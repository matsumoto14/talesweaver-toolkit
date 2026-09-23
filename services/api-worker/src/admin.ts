/**
 * 運営者の監視画面(admin.tw-context.dev)。ビルド無し: 1 枚の静的 HTML(admin-html.ts)+ JSON API。
 * 全リクエストを Cloudflare Access(access.ts)で検証してから応答する。
 */
import { verifyAccess } from "./access";
import type { AccessEnv } from "./access";
import { costUsd } from "./pricing";
import { ADMIN_HTML } from "./admin-html";

export interface AdminEnv extends AccessEnv {
  WIKI: D1Database;
}

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
  });
}

function html(body: string): Response {
  return new Response(body, { status: 200, headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" } });
}

export async function handleAdmin(request: Request, env: AdminEnv, url: URL): Promise<Response> {
  if (!(await verifyAccess(request, env))) return json({ error: "forbidden" }, 403);

  if (url.pathname === "/" && request.method === "GET") return html(ADMIN_HTML);
  if (url.pathname === "/api/logs" && request.method === "GET") return json(await listLogs(env.WIKI, url.searchParams));
  if (url.pathname.startsWith("/api/logs/") && request.method === "GET") {
    const id = Number(url.pathname.slice("/api/logs/".length));
    if (!Number.isInteger(id) || id <= 0) return json({ error: "invalid id" }, 400);
    const detail = await getLogDetail(env.WIKI, id);
    if (!detail) return json({ error: "not found" }, 404);
    return json(detail);
  }
  if (url.pathname === "/api/summary" && request.method === "GET") return json(await getSummary(env.WIKI, url.searchParams));
  return json({ error: "not found" }, 404);
}

// --- 小物 -----------------------------------------------------------------------

function placeholders(n: number): string {
  return Array.from({ length: n }, () => "?").join(",");
}

function clampInt(raw: string | null, fallback: number, min: number, max: number): number {
  // Number(null) と Number("") は 0 になるので、指定なしは先に既定値へ
  if (raw === null || raw.trim() === "") return fallback;
  const n = Number(raw);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(max, Math.max(min, Math.trunc(n)));
}

interface CallRow {
  ask_log_id: number;
  seq: number;
  kind: string;
  model: string;
  input_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  output_tokens: number;
  ms: number;
}

function callCost(c: { model: string; input_tokens: number; cache_read_tokens: number; cache_creation_tokens: number; output_tokens: number }): number | null {
  return costUsd({
    model: c.model,
    inputTokens: c.input_tokens,
    cacheReadTokens: c.cache_read_tokens,
    cacheCreationTokens: c.cache_creation_tokens,
    outputTokens: c.output_tokens,
  });
}

/** 費用の合計。1 件でも単価不明のモデルがあれば null(0 円に化かさない)。 */
function sumCost(costs: (number | null)[]): number | null {
  if (costs.length === 0) return 0;
  if (costs.some((c) => c === null)) return null;
  return costs.reduce((a: number, b) => a + (b ?? 0), 0);
}

// --- GET /api/logs ---------------------------------------------------------------

interface LogRow {
  id: number;
  at: string;
  question: string;
  kind: string;
  reason: string | null;
  route: string | null;
  ms: number;
  answer_id: string | null;
  missing_n: number | null;
}

/**
 * 直すべきもの = エラー / 見つからない(雑談・範囲外は除く)/ 一部だけ(missing あり)/ 👎・値の誤り。
 * 雑談・範囲外の誤分類は機械では決められないので、ここには入れず一覧の分類で見せる。
 */
const PROBLEM_SQL = `(al.kind = 'error'
  OR (al.kind = 'none' AND al.reason IN ('llm_none', 'verification_failed', 'no_terms'))
  OR (al.kind = 'answer' AND json_array_length(json_extract(al.body, '$.missing')) > 0)
  OR EXISTS (SELECT 1 FROM reaction r WHERE r.answer_id = al.answer_id AND r.kind IN ('wrong', 'value_wrong')))`;

async function listLogs(db: D1Database, params: URLSearchParams): Promise<unknown> {
  const from = params.get("from");
  const to = params.get("to");
  const kind = params.get("kind");
  const route = params.get("route");
  const reaction = params.get("reaction");
  const problem = params.get("problem") === "1";
  const before = params.get("before");
  const limit = clampInt(params.get("limit"), 50, 1, 200);

  const conditions: string[] = [];
  const args: unknown[] = [];
  if (from) { conditions.push("al.at >= ?"); args.push(from); }
  if (to) { conditions.push("al.at <= ?"); args.push(to); }
  if (kind) { conditions.push("al.kind = ?"); args.push(kind); }
  if (route) { conditions.push("al.route = ?"); args.push(route); }
  if (before) { conditions.push("al.id < ?"); args.push(clampInt(before, Number.MAX_SAFE_INTEGER, 1, Number.MAX_SAFE_INTEGER)); }
  if (problem) conditions.push(PROBLEM_SQL);
  if (reaction) {
    conditions.push("EXISTS (SELECT 1 FROM reaction r WHERE r.answer_id = al.answer_id AND r.kind = ?)");
    args.push(reaction);
  }
  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";

  const rows = await db
    .prepare(
      `SELECT al.id, al.at, al.question, al.kind, al.reason, al.route, al.ms, al.answer_id,
              json_array_length(json_extract(al.body, '$.missing')) AS missing_n
       FROM ask_log al ${where} ORDER BY al.id DESC LIMIT ${placeholders(1)}`,
    )
    .bind(...args, limit)
    .all<LogRow>();
  const logs = rows.results ?? [];

  const ids = logs.map((r) => r.id);
  const calls = ids.length > 0
    ? (
        await db
          .prepare(
            `SELECT ask_log_id, model, input_tokens, cache_read_tokens, cache_creation_tokens, output_tokens
             FROM ask_call WHERE ask_log_id IN (${placeholders(ids.length)})`,
          )
          .bind(...ids)
          .all<Omit<CallRow, "seq" | "kind" | "ms">>()
      ).results ?? []
    : [];
  const callsByLog = new Map<number, typeof calls>();
  for (const c of calls) {
    const list = callsByLog.get(c.ask_log_id) ?? [];
    list.push(c);
    callsByLog.set(c.ask_log_id, list);
  }

  const answerIds = [...new Set(logs.map((r) => r.answer_id).filter((v): v is string => !!v))];
  const reactionCounts = answerIds.length > 0
    ? (
        await db
          .prepare(
            `SELECT answer_id, kind, COUNT(*) AS n FROM reaction
             WHERE answer_id IN (${placeholders(answerIds.length)}) GROUP BY answer_id, kind`,
          )
          .bind(...answerIds)
          .all<{ answer_id: string; kind: string; n: number }>()
      ).results ?? []
    : [];
  const reactionsByAnswer = new Map<string, Record<string, number>>();
  for (const r of reactionCounts) {
    const rec = reactionsByAnswer.get(r.answer_id) ?? {};
    rec[r.kind] = r.n;
    reactionsByAnswer.set(r.answer_id, rec);
  }

  return {
    logs: logs.map((r) => {
      const rowCalls = callsByLog.get(r.id) ?? [];
      return {
        id: r.id,
        at: r.at,
        question: r.question,
        kind: r.kind,
        reason: r.reason,
        route: r.route,
        ms: r.ms,
        answer_id: r.answer_id,
        calls: rowCalls.length,
        cost_usd: sumCost(rowCalls.map(callCost)),
        reactions: r.answer_id ? reactionsByAnswer.get(r.answer_id) ?? {} : {},
      };
    }),
    next: logs.length === limit ? String(logs[logs.length - 1]!.id) : null,
  };
}

// --- GET /api/logs/:id -------------------------------------------------------------

interface LogDetailRow {
  id: number;
  at: string;
  user: string;
  question: string;
  qkey: string | null;
  prev_page: string | null;
  state: string;
  kind: string;
  reason: string | null;
  route: string | null;
  answer_id: string | null;
  lead: string | null;
  body: string;
  ms: number;
  understanding: string | null;
  candidates: string | null;
}

interface NeighborRow {
  id: number;
  at: string;
  question: string;
  kind: string;
  reason: string | null;
  missing_n: number | null;
}

async function getLogDetail(db: D1Database, id: number): Promise<unknown | null> {
  const log = await db.prepare("SELECT * FROM ask_log WHERE id = ?").bind(id).first<LogDetailRow>();
  if (!log) return null;

  const callRows = (
    await db
      .prepare(
        `SELECT ask_log_id, seq, kind, model, input_tokens, cache_read_tokens, cache_creation_tokens, output_tokens, ms
         FROM ask_call WHERE ask_log_id = ? ORDER BY seq ASC`,
      )
      .bind(id)
      .all<CallRow>()
  ).results ?? [];

  const reactions = (
    await db.prepare("SELECT * FROM reaction WHERE answer_id = ?").bind(log.answer_id ?? "").all()
  ).results ?? [];

  const before = (
    await db
      .prepare("SELECT id, at, question, kind, reason, json_array_length(json_extract(body, '$.missing')) AS missing_n FROM ask_log WHERE user = ? AND id < ? ORDER BY id DESC LIMIT 5")
      .bind(log.user, id)
      .all<NeighborRow>()
  ).results ?? [];
  const after = (
    await db
      .prepare("SELECT id, at, question, kind, reason, json_array_length(json_extract(body, '$.missing')) AS missing_n FROM ask_log WHERE user = ? AND id > ? ORDER BY id ASC LIMIT 5")
      .bind(log.user, id)
      .all<NeighborRow>()
  ).results ?? [];

  return {
    log: {
      id: log.id,
      at: log.at,
      user: log.user,
      question: log.question,
      qkey: log.qkey,
      prev_page: log.prev_page,
      state: parseJson(log.state),
      kind: log.kind,
      reason: log.reason,
      route: log.route,
      answer_id: log.answer_id,
      lead: log.lead,
      body: parseJson(log.body),
      ms: log.ms,
      understanding: parseJson(log.understanding),
      candidates: parseJson(log.candidates) ?? [],
    },
    calls: callRows.map((c) => ({
      seq: c.seq,
      kind: c.kind,
      model: c.model,
      input_tokens: c.input_tokens,
      cache_read_tokens: c.cache_read_tokens,
      cache_creation_tokens: c.cache_creation_tokens,
      output_tokens: c.output_tokens,
      ms: c.ms,
      cost_usd: callCost(c),
    })),
    reactions,
    neighbors: { before: before.reverse(), after },
  };
}

function parseJson(raw: string | null): unknown {
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

// --- GET /api/costs ----------------------------------------------------------------

interface CostRow {
  ask_log_id: number;
  at: string;
  user: string;
  model: string;
  input_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  output_tokens: number;
}

/** 1 問の結果の分類。画面の積み上げ棒と指標はこの 5 つで数える。 */
export type Outcome = "answered" | "partial" | "not_found" | "off" | "error";

export function outcomeOf(kind: string, reason: string | null, missingN: number | null): Outcome {
  if (kind === "error") return "error";
  if (kind === "answer") return (missingN ?? 0) > 0 ? "partial" : "answered";
  if (reason === "smalltalk" || reason === "other") return "off";
  return "not_found";
}

async function getSummary(db: D1Database, params: URLSearchParams): Promise<unknown> {
  const days = clampInt(params.get("days"), 30, 1, 365);
  const since = new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();

  const rows = (
    await db
      .prepare(
        `SELECT ac.ask_log_id, al.at, al.user, ac.model, ac.input_tokens, ac.cache_read_tokens,
                ac.cache_creation_tokens, ac.output_tokens
         FROM ask_call ac JOIN ask_log al ON al.id = ac.ask_log_id
         WHERE al.at >= ?`,
      )
      .bind(since)
      .all<CostRow>()
  ).results ?? [];

  interface DayAgg {
    date: string;
    questionIds: Set<number>;
    calls: number;
    inputTokens: number;
    cacheReadTokens: number;
    cacheCreationTokens: number;
    outputTokens: number;
    costs: (number | null)[];
  }
  const byDay = new Map<string, DayAgg>();

  // 結果の分類と反応(費用とは別に、呼び出しの無い質問 = 雑談・エラーも数える)
  const logRows = (
    await db
      .prepare(
        `SELECT id, at, kind, reason, ms, json_array_length(json_extract(body, '$.missing')) AS missing_n
         FROM ask_log WHERE at >= ?`,
      )
      .bind(since)
      .all<{ id: number; at: string; kind: string; reason: string | null; ms: number; missing_n: number | null }>()
  ).results ?? [];
  const reactionRows = (
    await db
      .prepare(
        `SELECT al.at, r.kind FROM reaction r JOIN ask_log al ON al.answer_id = r.answer_id
         WHERE al.at >= ? AND al.kind = 'answer'`,
      )
      .bind(since)
      .all<{ at: string; kind: string }>()
  ).results ?? [];
  type OutcomeAgg = Record<Outcome, number> & { ms: number; helpful: number; wrong: number; value_wrong: number };
  const emptyOutcome = (): OutcomeAgg =>
    ({ answered: 0, partial: 0, not_found: 0, off: 0, error: 0, ms: 0, helpful: 0, wrong: 0, value_wrong: 0 });
  const outcomeByDay = new Map<string, OutcomeAgg>();
  for (const r of logRows) {
    const o = outcomeByDay.get(r.at.slice(0, 10)) ?? emptyOutcome();
    o[outcomeOf(r.kind, r.reason, r.missing_n)] += 1;
    o.ms += r.ms;
    outcomeByDay.set(r.at.slice(0, 10), o);
  }
  for (const r of reactionRows) {
    const o = outcomeByDay.get(r.at.slice(0, 10)) ?? emptyOutcome();
    if (r.kind === "helpful" || r.kind === "wrong" || r.kind === "value_wrong") o[r.kind] += 1;
    outcomeByDay.set(r.at.slice(0, 10), o);
  }
  const byUser = new Map<string, { questionIds: Set<number>; costs: (number | null)[] }>();

  for (const r of rows) {
    const date = r.at.slice(0, 10);
    const day = byDay.get(date) ?? {
      date, questionIds: new Set(), calls: 0, inputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 0, costs: [],
    };
    day.questionIds.add(r.ask_log_id);
    day.calls += 1;
    day.inputTokens += r.input_tokens;
    day.cacheReadTokens += r.cache_read_tokens;
    day.cacheCreationTokens += r.cache_creation_tokens;
    day.outputTokens += r.output_tokens;
    const cost = costUsd({
      model: r.model, inputTokens: r.input_tokens, cacheReadTokens: r.cache_read_tokens,
      cacheCreationTokens: r.cache_creation_tokens, outputTokens: r.output_tokens,
    });
    day.costs.push(cost);
    byDay.set(date, day);

    const user = byUser.get(r.user) ?? { questionIds: new Set(), costs: [] };
    user.questionIds.add(r.ask_log_id);
    user.costs.push(cost);
    byUser.set(r.user, user);
  }

  const dates = [...new Set([...byDay.keys(), ...outcomeByDay.keys()])].sort().reverse();
  const daysOut = dates
    .map((date) => {
      const d = byDay.get(date) ?? {
        date, questionIds: new Set<number>(), calls: 0, inputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 0, costs: [],
      };
      const o = outcomeByDay.get(date) ?? emptyOutcome();
      const cost = sumCost(d.costs);
      const questions = o.answered + o.partial + o.not_found + o.off + o.error;
      return {
        date,
        questions,
        outcomes: { answered: o.answered, partial: o.partial, not_found: o.not_found, off: o.off, error: o.error },
        reactions: { helpful: o.helpful, wrong: o.wrong, value_wrong: o.value_wrong },
        avg_ms: questions > 0 ? Math.round(o.ms / questions) : null,
        calls: d.calls,
        input_tokens: d.inputTokens,
        cache_read_tokens: d.cacheReadTokens,
        cache_creation_tokens: d.cacheCreationTokens,
        output_tokens: d.outputTokens,
        cost_usd: cost,
        // 雑談・エラーなど呼び出しの無い質問も分母に入れる(1 問の実費)
        avg_cost_usd: cost !== null && questions > 0 ? cost / questions : null,
      };
    });

  const topUsers = [...byUser.entries()]
    .map(([user, u]) => ({ user, questions: u.questionIds.size, cost_usd: sumCost(u.costs) }))
    .sort((a, b) => (b.cost_usd ?? 0) - (a.cost_usd ?? 0))
    .slice(0, 20);

  return {
    days: daysOut,
    total_cost_usd: sumCost(daysOut.map((d) => d.cost_usd)),
    top_users: topUsers,
  };
}
