// 「wiki に聞く」の索引・検索・理解・選択 API(段階 1)。inquiry-worker とは別 Worker・
// 別ホスト(api.tw-context.dev)。LLM を呼ぶのは understand() / select()(src/claude.ts)だけ。
import * as agent from "./agent";
import * as claude from "./claude";
import {
  attachCorrections,
  correctedValue,
  findPages,
  getAllPageNames,
  getColumnNotes,
  getAppDataCandidates,
  getCorrectionsForUnitIds,
  getMeta,
  getPageUrls,
  searchUnits,
  collectCandidates,
} from "./retrieve";
import type { Candidate, ColumnNote } from "./retrieve";
import { buildDict, fold, normalize, segment, toQuery } from "./segment";
import { ASPECT_VOCAB } from "./prompt";
import type { PrevTurn } from "./prompt";
import { PAGE_SLOTS } from "./schema";
import { wikiUrl } from "./wiki-url";
import { verify } from "./verify";
import type { Ctx, Dropped } from "./verify";
import { buildAnswer } from "./answer";
import type { AnswerResponse } from "./answer";
import {
  consumeRateLimit,
  issueChallenge,
  issueSessionToken,
  requireSession,
  userHash,
  verifyNonce,
  verifyProofOfWork,
  difficultyOf,
} from "./auth";
import { logAsk } from "./log";
import { getCachedAnswer, putCachedAnswer, questionKey, wikiVersion } from "./cache";
import { handleReact } from "./react";

export interface Env {
  WIKI: D1Database;
  /** nonce の使用済み記録・レート制限カウンタ・リアクションの重複防止。/health /search には要らない。 */
  API?: KVNamespace;
  ANTHROPIC_API_KEY?: string;
  NONCE_SECRET?: string;
  POW_DIFFICULTY_BITS?: string;
  AI_GATEWAY_URL?: string;
  SELECT_MODEL?: string;
  UNDERSTAND_MODEL?: string;
  RATE_LIMIT_PER_DAY?: string;
  ASK_BURST?: { limit: (opts: { key: string }) => Promise<{ success: boolean }> };
}

/** /challenge /session /ask /react が実際に必要とする形(KV は wrangler.toml のバインディングで常に付く)。 */
type EnvWithApi = Env & { API: KVNamespace };
/** /ask が実際に必要とする形(missingSecrets で確認済みの秘密 + モデル名 + KV)。 */
type AskEnv = EnvWithApi & {
  ANTHROPIC_API_KEY: string;
  NONCE_SECRET: string;
  SELECT_MODEL: string;
  UNDERSTAND_MODEL: string;
};

const SEARCH_QUERY_LIMIT = 200;
const QUESTION_LIMIT = 200;
const DEFAULT_LIMIT = 10;
const MAX_LIMIT = 20;
const NONE_SEARCH_LIMIT = 10;

/** `wrangler secret put` で入れる 2 つ。ルートごとに要る分だけ確認する。 */
function missingSecrets(env: Env, need: readonly ("ANTHROPIC_API_KEY" | "NONCE_SECRET")[]): string[] {
  return need.filter((name) => !env[name]);
}

/**
 * 辞書 = alias.name 全件 + 単独の訂正・静的データの名前(correction.subject、unit_id が NULL)+ BASE_WORDS。
 * 取込側(units.py の dict.txt)と同じ 2 系統で組むので、索引と質問の切れ目が揃う。
 * 起動中のインスタンスで使い回す(D1 は毎回読まない)。
 */
let aliasCache: {
  db: D1Database;
  words: string[];
  byName: Map<string, string[]>;
  /** 分かち書きの語(fold 済み)→ correction.subject の原文。静的データの完全一致に使う */
  subjectByToken: Map<string, string>;
} | null = null;

async function loadAliasIndex(
  db: D1Database,
): Promise<{ words: string[]; byName: Map<string, string[]>; subjectByToken: Map<string, string> }> {
  if (aliasCache && aliasCache.db === db) return aliasCache;
  const result = await db.prepare("SELECT name, page FROM alias").all<{ name: string; page: string }>();
  const byName = new Map<string, string[]>();
  const names: string[] = [];
  for (const row of result.results ?? []) {
    const normalized = normalize(row.name);
    if (!normalized) continue;
    names.push(normalized);
    const key = fold(normalized);
    const pages = byName.get(key) ?? [];
    if (!pages.includes(row.page)) pages.push(row.page);
    byName.set(key, pages);
  }
  const subjects = await db
    .prepare("SELECT DISTINCT subject FROM correction WHERE unit_id IS NULL")
    .all<{ subject: string }>();
  const subjectByToken = new Map<string, string>();
  for (const row of subjects.results ?? []) {
    const normalized = normalize(row.subject);
    if (!normalized) continue;
    names.push(normalized);
    if (!subjectByToken.has(fold(normalized))) subjectByToken.set(fold(normalized), row.subject);
  }
  const words = buildDict(names);
  aliasCache = { db, words, byName, subjectByToken };
  return aliasCache;
}

/** 全ページ名。結論文の固有名詞照合にだけ使う(起動中のインスタンスで使い回す)。 */
let pageNamesCache: { db: D1Database; names: string[] } | null = null;

async function loadPageNames(db: D1Database): Promise<string[]> {
  if (pageNamesCache && pageNamesCache.db === db) return pageNamesCache.names;
  const names = await getAllPageNames(db);
  pageNamesCache = { db, names };
  return names;
}

/** 同じページのユニットが上位を埋め尽くさないよう、1 ページあたりの件数を抑える。 */
const MAX_PER_PAGE = 8;

function capPerPage<T extends { page: string }>(hits: T[]): T[] {
  const counts = new Map<string, number>();
  const kept: T[] = [];
  const overflow: T[] = [];
  for (const h of hits) {
    const n = counts.get(h.page) ?? 0;
    if (n < MAX_PER_PAGE) { kept.push(h); counts.set(h.page, n + 1); } else overflow.push(h);
  }
  return [...kept, ...overflow];
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") return cors(new Response(null, { status: 204 }));

    try {
      if (url.pathname === "/health" && request.method === "GET") {
        return cors(await health(env));
      }
      if (url.pathname === "/search" && request.method === "GET") {
        return cors(await search(url, env));
      }
      if (url.pathname === "/challenge" && request.method === "GET") {
        const missing = missingSecrets(env, ["NONCE_SECRET"]);
        if (missing.length > 0) return cors(json({ error: `中継サーバーが未設定です: ${missing.join(", ")}` }, 503));
        return cors(json(await issueChallenge(env as EnvWithApi & { NONCE_SECRET: string })));
      }
      if (url.pathname === "/session" && request.method === "POST") {
        const missing = missingSecrets(env, ["NONCE_SECRET"]);
        if (missing.length > 0) return cors(json({ error: `中継サーバーが未設定です: ${missing.join(", ")}` }, 503));
        return cors(await session(request, env as EnvWithApi & { NONCE_SECRET: string }));
      }
      if (url.pathname === "/ask" && request.method === "POST") {
        const missing = missingSecrets(env, ["ANTHROPIC_API_KEY", "NONCE_SECRET"]);
        if (missing.length > 0) return cors(json({ error: `中継サーバーが未設定です: ${missing.join(", ")}` }, 503));
        return cors(await ask(request, env as AskEnv, ctx));
      }
      if (url.pathname === "/react" && request.method === "POST") {
        const missing = missingSecrets(env, ["NONCE_SECRET"]);
        if (missing.length > 0) return cors(json({ error: `中継サーバーが未設定です: ${missing.join(", ")}` }, 503));
        const envWithApi = env as EnvWithApi & { NONCE_SECRET: string };
        const authError = await requireSession(request, envWithApi);
        if (authError) return cors(json({ error: authError }, 401));
        const rateError = await consumeRateLimit(request, envWithApi);
        if (rateError) return cors(json({ error: rateError }, 429));
        return cors(await handleReact(request, envWithApi, (answerId) => ctx.waitUntil(rememberHelpfulAnswer(envWithApi, answerId))));
      }
    } catch (error) {
      console.error(error);
      return cors(json({ error: "サーバー側で問題が起きました。時間をおいて試してください。" }, 500));
    }

    return cors(json({ error: "not found" }, 404));
  },
};

// --- /health /search(段階 0) --------------------------------------------------

async function health(env: Env): Promise<Response> {
  const meta = await getMeta(env.WIKI);
  const syncedAt = meta.synced_at;
  const unitCount = meta.unit_count;
  const schemaVersion = meta.schema_version;
  if (!syncedAt || !unitCount || !schemaVersion) {
    return json({ ok: false, error: "索引が未投入です" }, 503);
  }
  return json({ ok: true, synced_at: syncedAt, unit_count: Number(unitCount), schema_version: schemaVersion });
}

async function search(url: URL, env: Env): Promise<Response> {
  const rawQ = (url.searchParams.get("q") ?? "").slice(0, SEARCH_QUERY_LIMIT);
  const limit = clampLimit(url.searchParams.get("limit"));
  const meta = await getMeta(env.WIKI);
  const syncedAt = meta.synced_at ?? null;

  const { words: dict, byName } = await loadAliasIndex(env.WIKI);
  const tokens = segment(rawQ, dict);
  const query = toQuery(tokens);

  if (!query) {
    return json({ kind: "none", reason: "no_terms", query: null, search: [], synced_at: syncedAt, dropped: [] });
  }

  const rawHits = await searchUnits(env.WIKI, query);
  const aliasPages = new Set(tokens.flatMap((t) => byName.get(t) ?? []));
  const boosted = aliasPages.size > 0
    ? [...rawHits].sort((a, b) => Number(aliasPages.has(b.page)) - Number(aliasPages.has(a.page)))
    : rawHits;
  const hits = capPerPage(boosted);
  const pageUrls = await getPageUrls(env.WIKI, hits.map((h) => h.page));

  return json({
    kind: "none",
    reason: "no_llm",
    query,
    search: hits.slice(0, limit).map((h) => ({
      id: h.id, kind: h.kind, page: h.page, section: h.section, anchor: h.anchor,
      snippet: h.text, truncated: h.truncated === 1, url: wikiUrl(pageUrls.get(h.page) ?? "", h.anchor),
    })),
    synced_at: syncedAt,
    dropped: [],
  });
}

function clampLimit(raw: string | null): number {
  if (raw === null || raw.trim() === "") return DEFAULT_LIMIT;
  const n = Number(raw);
  if (!Number.isFinite(n)) return DEFAULT_LIMIT;
  return Math.min(MAX_LIMIT, Math.max(1, Math.trunc(n)));
}

// --- /session -------------------------------------------------------------------

async function session(request: Request, env: EnvWithApi & { NONCE_SECRET: string }): Promise<Response> {
  const payload = (await request.json().catch(() => null)) as { nonce?: string; solution?: string } | null;
  const nonce = typeof payload?.nonce === "string" ? payload.nonce : "";
  const solution = typeof payload?.solution === "string" ? payload.solution : "";
  if (!nonce || !solution) return json({ error: "送信の準備ができていません" }, 400);

  const nonceError = await verifyNonce(env, nonce);
  if (nonceError) return json({ error: nonceError }, 400);
  if (!(await verifyProofOfWork(nonce, solution, difficultyOf(env)))) {
    return json({ error: "送信の検証に失敗しました" }, 400);
  }

  const { token, expiresInSeconds } = await issueSessionToken(env);
  return json({ token, expiresInSeconds });
}

// --- /ask -------------------------------------------------------------------

interface AskState { level?: number; evolution?: number }
interface AskPayload {
  question?: string;
  state?: AskState;
  prev?: { question: string; page: string } | null;
  /** `loop: false` で回す道を無効化(eval/run.ts --no-loop、安い道だけの数字を測る用)。 */
  debug?: { understand?: boolean; loop?: boolean; cache?: boolean };
}

function normalizeState(state: AskState | undefined): Record<string, number> {
  const out: Record<string, number> = {};
  if (state && typeof state.level === "number" && Number.isFinite(state.level)) out.level = state.level;
  if (state && typeof state.evolution === "number" && Number.isFinite(state.evolution)) out.evolution = state.evolution;
  return out;
}

const PREV_QUESTION_LIMIT = 200;
const PREV_PAGE_LIMIT = 100;

function normalizePrev(prev: AskPayload["prev"]): PrevTurn | null {
  if (!prev || typeof prev.question !== "string" || typeof prev.page !== "string") return null;
  return {
    question: prev.question.slice(0, PREV_QUESTION_LIMIT),
    page: prev.page.slice(0, PREV_PAGE_LIMIT),
  };
}

export interface NoneAnswer {
  kind: "none";
  reason: "llm_none" | "verification_failed" | "smalltalk" | "other" | "no_terms";
  search: { id: string; page: string; section: string; snippet: string; url: string }[];
  synced_at: string | null;
  dropped: { what: string; id?: string; why: string }[];
  /** 困りごとが「勝てない」の型なら cant_win。wiki に答えが無くても打ち手は出る(端末が描く)。 */
  playbook: "cant_win" | null;
}

async function noneAnswer(
  env: Env,
  reason: NoneAnswer["reason"],
  query: string | null,
  syncedAt: string | null,
  dropped: NoneAnswer["dropped"],
  playbook: "cant_win" | null = null,
): Promise<NoneAnswer> {
  if (!query) return { kind: "none", reason, search: [], synced_at: syncedAt, dropped, playbook };
  const hits = await searchUnits(env.WIKI, query);
  const pageUrls = await getPageUrls(env.WIKI, hits.map((h) => h.page));
  return {
    kind: "none",
    reason,
    search: hits.slice(0, NONE_SEARCH_LIMIT).map((h) => ({
      id: h.id, page: h.page, section: h.section, snippet: h.text,
      url: wikiUrl(pageUrls.get(h.page) ?? "", h.anchor),
    })),
    synced_at: syncedAt,
    dropped,
    playbook,
  };
}

/** 質問の語(2 文字以上)で索引に当たりがあるか(誤分類の歯止め)。挨拶は語が無いので false。 */
async function questionHitsIndex(db: D1Database, tokens: string[]): Promise<boolean> {
  const query = toQuery(tokens.filter((t) => t.length >= 2));
  if (!query) return false;
  return (await searchUnits(db, query)).length > 0;
}

/** 質問の全文が別名に完全一致するか(誤分類の歯止め)。 */
async function aliasExactMatch(db: D1Database, question: string): Promise<boolean> {
  const row = await db.prepare("SELECT 1 FROM alias WHERE name = ?1 LIMIT 1").bind(question.trim()).first();
  return row !== null;
}

type ProgressStep = "understand" | "search" | "select" | "app_data" | "outline" | "rows";
type ProgressFn = (step: ProgressStep) => void;
const noopProgress: ProgressFn = () => {};

export type RunAskResult =
  | { status: 200; body: NoneAnswer | AnswerResponse }
  | { status: 502; body: { error: string } };

async function ask(request: Request, env: AskEnv, ctx: ExecutionContext): Promise<Response> {
  const authError = await requireSession(request, env);
  if (authError) return json({ error: authError }, 401);

  // 本文の検査を上限の消費より先に(空の質問で 1 問ぶん減らさない)
  const payload = (await request.json().catch(() => null)) as AskPayload | null;
  const rawQuestion = typeof payload?.question === "string" ? payload.question : "";
  const question = rawQuestion.trim().slice(0, QUESTION_LIMIT);
  if (!question) return json({ error: "質問を入力してください" }, 400);

  const rateError = await consumeRateLimit(request, env);
  if (rateError) return json({ error: rateError }, 429);

  const state = normalizeState(payload?.state);
  const prev = normalizePrev(payload?.prev ?? null);
  const callUnderstand = payload?.debug?.understand !== false;
  const loopEnabled = payload?.debug?.loop !== false;

  // 質問と答えの記録(ask_log)。応答を待たせないよう waitUntil で。失敗しても答えは返す
  const startedAt = Date.now();
  const user = await userHash(request, env);
  // 答えのキャッシュのキー(cache.ts)。続きの質問は文脈依存なので作らない
  const qkey = prev ? null : questionKey(question, (await loadAliasIndex(env.WIKI)).words);
  const record = (result: RunAskResult | { status: 500; body: { error: string } }): void => {
    ctx.waitUntil(logAsk(env.WIKI, { user, question, qkey, prev, state, result, ms: Date.now() - startedAt }));
  };

  // 「役に立った」が付いた同じ意味の質問なら LLM を呼ばずに返す(wiki の版が同じときだけ)
  if (qkey && payload?.debug?.cache !== false) {
    const cached = await getCachedAnswer(env.API, qkey, wikiVersion(await getMeta(env.WIKI)), state);
    if (cached) {
      const result: RunAskResult = { status: 200, body: cached };
      record(result);
      return wantsSse(request) ? sseOnce("result", cached) : json(cached);
    }
  }

  if (!wantsSse(request)) {
    // 失敗も記録に残す(SSE 経路と同じ)。例外は 500 として記録してから投げ直す(呼び元が 500 を返す)
    let result: RunAskResult;
    try {
      result = await runAsk(env, question, state, prev, callUnderstand, loopEnabled, noopProgress);
    } catch (error) {
      record({ status: 500, body: { error: "サーバー側で問題が起きました。時間をおいて試してください。" } });
      throw error;
    }
    record(result);
    return json(result.body, result.status);
  }
  return askSse(env, question, state, prev, callUnderstand, loopEnabled, record);
}

function wantsSse(request: Request): boolean {
  return (request.headers.get("accept") ?? "").includes("text/event-stream");
}

/** SSE でイベントを 1 つ流して閉じる(キャッシュから即答するとき)。 */
function sseOnce(event: string, data: unknown): Response {
  return new Response(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`, {
    status: 200,
    headers: { "content-type": "text/event-stream", "cache-control": "no-store" },
  });
}

/** /react の helpful(1 回目)で、その答えを ask_log から引いてキャッシュに入れる。 */
async function rememberHelpfulAnswer(env: EnvWithApi, answerId: string): Promise<void> {
  try {
    const row = await env.WIKI
      .prepare("SELECT qkey, body, state FROM ask_log WHERE answer_id = ?1 AND kind = 'answer' AND qkey IS NOT NULL LIMIT 1")
      .bind(answerId)
      .first<{ qkey: string; body: string; state: string }>();
    if (!row) return;
    const body = JSON.parse(row.body) as AnswerResponse;
    const state = JSON.parse(row.state) as Record<string, number>;
    await putCachedAnswer(env.API, row.qkey, wikiVersion(await getMeta(env.WIKI)), body, state);
  } catch (error) {
    console.error("答えのキャッシュへの書き込みに失敗", error);
  }
}

/** `Accept: text/event-stream` のときだけ通る道。理解より前のエラー(401/429/503)は呼び元(ask/fetch)が
 * すでに返している。ここから先に起きるのは 502(Claude 呼び出し後の経路の故障)だけ。 */
function askSse(
  env: AskEnv,
  question: string,
  state: Record<string, number>,
  prev: PrevTurn | null,
  callUnderstand: boolean,
  loopEnabled: boolean,
  record: (result: RunAskResult | { status: 500; body: { error: string } }) => void,
): Response {
  const encoder = new TextEncoder();
  // クライアントが切断したら cancel() が呼ばれる。以後の enqueue は捨てる(Claude の呼び出しは
  // 途中で止めない。最長でも選択の 15 秒で終わる)
  let closed = false;
  const stream = new ReadableStream({
    async start(controller) {
      const send = (event: string, data: unknown): void => {
        if (closed) return;
        controller.enqueue(encoder.encode(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`));
      };
      try {
        const progress: ProgressFn = (step) => send("progress", { step });
        const result = await runAsk(env, question, state, prev, callUnderstand, loopEnabled, progress);
        record(result);
        if (result.status === 502) send("error", { status: result.status, error: result.body.error });
        else send("result", result.body);
      } catch (error) {
        console.error(error);
        const message = "サーバー側で問題が起きました。時間をおいて試してください。";
        record({ status: 500, body: { error: message } });
        send("error", { status: 500, error: message });
      } finally {
        if (!closed) { closed = true; controller.close(); }
      }
    },
    cancel() {
      closed = true;
    },
  });
  return new Response(stream, {
    status: 200,
    headers: { "content-type": "text/event-stream", "cache-control": "no-store" },
  });
}


/** 質問の語から「ページ名の候補」を最大 10 件。完全一致した別名のページ → 語を含む別名のページ(語ごとに LIKE)。 */
async function findPagesByTokens(
  db: D1Database,
  tokens: string[],
  byName: Map<string, string[]>,
): Promise<{ name: string; page: string }[]> {
  const out: { name: string; page: string }[] = [];
  const seen = new Set<string>();
  const push = (name: string, page: string): void => {
    if (seen.has(page)) return;
    seen.add(page);
    out.push({ name, page });
  };
  for (const t of tokens) for (const page of byName.get(t) ?? []) push(t, page);
  // 語ごとの LIKE は D1 のフルスキャンなので、走査する語は先頭 8 語まで
  for (const t of tokens.slice(0, 8)) {
    if (out.length >= PAGE_SLOTS.length) break;
    if (t.length < 2) continue;
    for (const hit of await findPages(db, t)) push(hit.name, hit.page);
  }
  return out.slice(0, PAGE_SLOTS.length);
}

/**
 * 回す道(段階 2、agent.ts)を 1 回試す。`null` は経路の故障・不使用(呼び元が安い道の結果へ)。
 * `route` は応答に載る値("loop" = hops:multi で最初から、"cheap_then_loop" = 安い道が駄目だったやり直し)。
 */
async function tryLoop(
  env: AskEnv,
  question: string,
  state: Record<string, number>,
  prev: PrevTurn | null,
  columnNotes: Record<string, ColumnNote>,
  dict: readonly string[],
  syncedAt: string | null,
  playbook: "cant_win" | null,
  pageNames: string[],
  columnDict: Record<string, string>,
  route: "loop" | "cheap_then_loop",
  hopsDropped: Dropped[],
  followupPage: string | null,
  progress: ProgressFn,
): Promise<RunAskResult | null> {
  const result = await agent.runAgentLoop(
    { db: env.WIKI, env, question, state, columnNotes, dict, prev },
    (step) => progress(step),
  );
  if (!result) return null;

  const routeDropped: Dropped = {
    what: "route",
    why: `loop:${result.toolCalls}回/${result.ms}ms/in${result.tokens.input}+cached${result.tokens.cached}+written${result.tokens.cacheWritten}/out${result.tokens.output}`,
  };
  const dropped = [...hopsDropped, routeDropped];

  if (result.selection.none) {
    return { status: 200, body: await noneAnswer(env, "llm_none", null, syncedAt, dropped, playbook) };
  }

  const ctx: Ctx = {
    candidates: result.candidates,
    state,
    columnDict,
    pageNames,
    candidateText: candidateTextOf([...result.candidates.values()]),
  };
  const verified = verify(result.selection, ctx);
  if (verified.steps.length === 0) {
    return {
      status: 200,
      body: await noneAnswer(env, "verification_failed", null, syncedAt, [...verified.dropped, ...dropped], playbook),
    };
  }

  const answer = await buildAnswer(env.WIKI, {
    steps: verified.steps,
    lead: verified.lead,
    dropped: [...verified.dropped, ...dropped],
    columnDict,
    state,
    syncedAt,
    model: env.SELECT_MODEL,
    route,
    playbook,
    missing: verified.missing,
    verdict: result.selection.verdict,
    basis: verified.lead
      ? result.selection.basis.filter((id) => verified.steps.some((st) => st.units.some((u) => u.id === id)))
      : [],
    followup: followupPage ? { page: followupPage } : null,
  });
  return { status: 200, body: answer };
}

/** `/ask` の中身(理解 → 候補収集 → 選択 → 検証 → 回答)。JSON 経路・SSE 経路の両方から呼ばれる。 */
async function runAsk(
  env: AskEnv,
  question: string,
  state: Record<string, number>,
  prev: PrevTurn | null,
  callUnderstand: boolean,
  loopEnabled: boolean,
  progress: ProgressFn,
): Promise<RunAskResult> {
  const meta = await getMeta(env.WIKI);
  const syncedAt = meta.synced_at ?? null;

  const { words: dict, byName, subjectByToken } = await loadAliasIndex(env.WIKI);
  // ページ名の候補: 質問の分かち書きの語が別名に完全一致したページを先に、次に語を含む別名のページ。
  // 質問文全体を LIKE に投げない(当たらないうえ、長い文は D1 が「pattern too complex」で拒む)
  const questionTokens = segment(question, dict);
  const pageHits = await findPagesByTokens(env.WIKI, questionTokens, byName);
  const pageSlots = pageHits.slice(0, PAGE_SLOTS.length).map((h, i) => ({ slot: PAGE_SLOTS[i]!, page: h.page }));

  progress("understand");
  const understanding = callUnderstand
    ? (await claude.understand(env, question, prev, pageSlots)) ?? codeFallbackUnderstand()
    : codeFallbackUnderstand();

  // 誤分類の歯止め 2 つ: 質問の全文が別名に一致する / 質問の語で索引に当たりがある(「聖水はどうやって稼ぐ?」を
  // 雑談と判定した実例 2026-09-23)。どちらかなら wiki として進める(挨拶は語が索引に無いので変わらない)
  let kind = understanding.kind;
  const kindDropped: Dropped[] = [];
  if (kind !== "wiki") {
    const rescued =
      (await aliasExactMatch(env.WIKI, question)) ||
      (await questionHitsIndex(env.WIKI, questionTokens));
    if (rescued) {
      kindDropped.push({ what: "kind", why: `${kind}→wiki(索引に当たりあり)` });
      kind = "wiki";
    }
  }
  // 困りごとの型「勝てない」。wiki の答えの有無に関わらず載せる(端末が打ち手をローカルで描く)。
  const playbook: "cant_win" | null = understanding.trouble === "cant_win" ? "cant_win" : null;

  if (kind === "smalltalk" || kind === "other") {
    return { status: 200, body: await noneAnswer(env, kind, null, syncedAt, [], playbook) };
  }

  // hops は記録だけ残す(評価が「回す道なら解けたかもしれない率」を数える材料)。hops:multi は下で
  // 最初から回す道を試す(§振り分け 1)。
  const hopsDropped: Dropped[] = [
    ...kindDropped,
    ...(understanding.hops === "multi" ? [{ what: "route", why: "hops:multi" } as Dropped] : []),
  ];
  // 1 問につき回す道は 1 回だけ(hops:multi で先に試したら、安い道が駄目でも二度目は試さない)。
  let loopTried = false;

  const pageNames = await loadPageNames(env.WIKI);
  const slotToPage = new Map(pageSlots.map((p) => [p.slot, p.page]));
  const understoodPages = understanding.pages.map((p) => slotToPage.get(p)).filter((p): p is string => !!p);
  const boostPages = understoodPages.length > 0 ? understoodPages : pageHits.slice(0, 3).map((h) => h.page);

  // 続き(followup): 直前のページが実在し、理解が続きと判定したときだけ加点し、応答に載せる。
  let followupPage: string | null = null;
  if (understanding.followup && prev && pageNames.includes(prev.page)) {
    followupPage = prev.page;
    if (!boostPages.includes(prev.page)) boostPages.push(prev.page);
  }

  const columnNoteRows = await getColumnNotes(env.WIKI);
  const columnDict: Record<string, string> = {};
  const columnNoteTexts: Record<string, string> = {};
  for (const [name, note] of Object.entries(columnNoteRows)) {
    columnNoteTexts[name] = note.note;
    if (note.state_key) columnDict[name] = note.state_key;
  }

  // hops:multi は最初から回す道(§振り分け 1)。安い道の search/select は呼ばない。
  // 回す道が駄目だった(null)ときだけ、下の安い道に進む。
  if (understanding.hops === "multi" && loopEnabled) {
    loopTried = true;
    const loopResult = await tryLoop(
      env, question, state, prev, columnNoteRows, dict, syncedAt, playbook,
      pageNames, columnDict, "loop", hopsDropped, followupPage, progress,
    );
    if (loopResult) return loopResult;
  }

  const termTokens = understanding.terms.slice(0, 6).flatMap((t) => segment(t.slice(0, 20), dict));
  const aspectTokens: string[] = []; // 観点(aspects)は段階 3 で理解の欄に戻す。それまで語彙の加点は無し
  const query = toQuery([...questionTokens, ...termTokens, ...aspectTokens]);
  if (!query) return { status: 200, body: await noneAnswer(env, "no_terms", null, syncedAt, hopsDropped, playbook) };

  progress("search");
  let candidates = await collectCandidates(env.WIKI, { query, boostPages, state, columnNotes: columnNoteRows });
  // 順序頑健性の測定用(ローカルの .dev.vars にだけ置く)。候補の並びを逆にして同じ評価を回し、選択の揺れを見る
  if ((env as { EVAL_REVERSE_CANDIDATES?: string }).EVAL_REVERSE_CANDIDATES === "1") candidates = [...candidates].reverse();
  // 静的データだけの項目(段階 3 spec B 7): 質問の語 + 理解の terms が完全一致した subject だけ足す
  // 静的データは語が名前に完全一致したときだけ。語 → 原文の名前に戻し、D1 の bind 上限(100)より手前で切る
  const subjects = [...new Set([...questionTokens, ...termTokens].map((t) => subjectByToken.get(t)).filter((v): v is string => !!v))].slice(0, 50);
  candidates.push(...(await getAppDataCandidates(env.WIKI, subjects)));
  const correctionRows = await getCorrectionsForUnitIds(env.WIKI, candidates.map((c) => c.id));
  attachCorrections(candidates, correctionRows);

  progress("select");
  let result = await claude.select(env, question, state, candidates, columnNoteTexts);
  if (!result) result = await claude.select(env, question, state, candidates, columnNoteTexts);
  if (!result) return { status: 502, body: { error: "回答サーバーが応答しません。時間をおいて試してください" } };

  const { selection } = result;

  /** 安い道が駄目だった(none / 検証で全滅)ときの、1 回だけの回す道リトライ。 */
  const retryWithLoop = async (): Promise<RunAskResult | null> => {
    if (loopTried || !loopEnabled) return null;
    loopTried = true;
    return tryLoop(
      env, question, state, prev, columnNoteRows, dict, syncedAt, playbook,
      pageNames, columnDict, "cheap_then_loop", hopsDropped, followupPage, progress,
    );
  };

  if (selection.none) {
    const loopResult = await retryWithLoop();
    if (loopResult) return loopResult;
    return { status: 200, body: await noneAnswer(env, "llm_none", query, syncedAt, hopsDropped, playbook) };
  }

  const ctx: Ctx = {
    candidates: new Map(candidates.map((c) => [c.id, c] as [string, Candidate])),
    state,
    columnDict,
    pageNames,
    candidateText: candidateTextOf(candidates),
  };
  const verified = verify(selection, ctx);
  if (verified.steps.length === 0) {
    const loopResult = await retryWithLoop();
    if (loopResult) return loopResult;
    return {
      status: 200,
      body: await noneAnswer(env, "verification_failed", query, syncedAt, [...verified.dropped, ...hopsDropped], playbook),
    };
  }

  const answer = await buildAnswer(env.WIKI, {
    steps: verified.steps,
    lead: verified.lead,
    dropped: [...verified.dropped, ...hopsDropped],
    columnDict,
    state,
    syncedAt,
    model: env.SELECT_MODEL,
    route: "cheap",
    playbook,
    missing: verified.missing,
    verdict: selection.verdict,
    basis: verified.lead ? selection.basis.filter((id) => verified.steps.some((st) => st.units.some((u) => u.id === id))) : [],
    followup: followupPage ? { page: followupPage } : null,
  });

  // missing(候補に答えが無かった観点)が空でなければ、1 回だけ回す道でやり直す(§振り分け 2)。
  if (answer.missing.length > 0) {
    const loopResult = await retryWithLoop();
    // 回す道が本物の答えを出したときだけ差し替える。none で戻ったら安い道の部分的な答えを残す(仕様: 回す道でも駄目なら安い道の結果)
    if (loopResult && "kind" in loopResult.body && loopResult.body.kind === "answer") return loopResult;
  }
  return { status: 200, body: answer };
}

/** LLM(理解)が落ちたときのコード経路: 何も選ばず wiki として進めるだけ(別名一致は呼び元が別途見る)。 */
function codeFallbackUnderstand(): {
  kind: "wiki"; pages: never[]; terms: never[]; hops: "single"; followup: false;
  mood: "ask"; trouble: "none";
} {
  return { kind: "wiki", pages: [], terms: [], hops: "single", followup: false, mood: "ask", trouble: "none" };
}

/** 結論文の固有名詞照合用に、候補の全文を寄せ集める(候補内に出る名前は許すため)。 */
function candidateTextOf(candidates: Candidate[]): string {
  return candidates
    .map((c) => {
      const cellsText = c.cells ? Object.entries(c.cells).map(([col]) => correctedValue(c, col) ?? "").join(" ") : "";
      return `${c.page} ${c.section} ${c.text} ${cellsText}`;
    })
    .join(" ");
}

// --- 小物 --------------------------------------------------------------------

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
  });
}

/** CORS は `*`(inquiry と同じ)。認証情報を載せない API。 */
function cors(response: Response): Response {
  const headers = new Headers(response.headers);
  headers.set("access-control-allow-origin", "*");
  headers.set("access-control-allow-methods", "GET, POST, OPTIONS");
  headers.set("access-control-allow-headers", "content-type, authorization, x-client-id");
  headers.set("access-control-max-age", "86400");
  return new Response(response.body, { status: response.status, headers });
}
