// 評価セットの実行。
//
// 既定(--ask 無し)は段階 0: 起動中の Worker(`npm run dev`、ローカル D1 に全件投入済み)の
// GET /search を叩き、各問の期待ユニット(ページ + 断片に含まれる文字列)が上位 20 件に
// 入っているかを数える。LLM は要らない。
//
// --ask を付けると段階 1: /challenge → PoW → /session → /ask を叩き、行の一致・none 率・
// lead の生存率と口調・verdict の分布・安い道で解けなかった率を出す(ANTHROPIC_API_KEY が
// 要る。`.dev.vars` に鍵を入れてから `npm run dev` すること)。--no-understand は /ask の
// 要求 JSON に `debug: { understand: false }` を足し、理解あり/なしの再現率を比べる。
// --no-loop は `debug: { loop: false }` を足し、回す道(段階 2、agent.ts)を無効化して
// 安い道だけの数字を測る(段階 1 の数字と比べるため)。既定(--no-loop 無し)は回す道あり。
//
// 実行: node --experimental-strip-types eval/run.ts [--base http://127.0.0.1:8787] [--limit 20]
//       [--ask] [--no-understand] [--no-loop] [--verbose]
import { readFileSync } from "node:fs";

interface Expect { page: string; text_contains?: string }
interface Question {
  id: string;
  kind: "wiki" | "smalltalk" | "other" | "followup" | "absent" | "damage_calc";
  question: string;
  paraphrase?: boolean;
  expect: Expect[];
  state?: Record<string, number>;
  prev?: { question: string; page: string };
}
interface Hit { id: string; kind: string; page: string; section: string; snippet: string }
interface SearchResponse { kind: string; reason: string; query: string | null; search: Hit[] }

function arg(name: string, fallback: string): string {
  const i = process.argv.indexOf(name);
  const v = i === -1 ? undefined : process.argv[i + 1];
  return v ?? fallback;
}
const base = arg("--base", "http://127.0.0.1:8787");
const limit = Number(arg("--limit", "20"));
const verbose = process.argv.includes("--verbose");
const askMode = process.argv.includes("--ask");
const noUnderstand = process.argv.includes("--no-understand");
const noLoop = process.argv.includes("--no-loop");

const file = new URL("./questions.json", import.meta.url);
const questions = (JSON.parse(readFileSync(file, "utf8")) as { questions: Question[] }).questions;

const matches = (hit: Hit, e: Expect): boolean =>
  hit.page === e.page && (e.text_contains === undefined || hit.snippet.includes(e.text_contains));

if (askMode) {
  await runAsk();
} else {
  await runSearch();
}

// --- 段階 1: /ask ----------------------------------------------------------------

interface AskUnit { id: string; kind: string; text?: string; cells?: Record<string, string> }
interface AskStep { units: AskUnit[] }
interface AskResponse {
  kind: "answer" | "none" | "error";
  reason?: string;
  verdict?: string;
  lead?: unknown[] | null;
  steps?: AskStep[];
  route?: string;
  missing?: string[];
  dropped?: { what: string; why: string }[];
}

async function solvePow(nonce: string, bits: number): Promise<string> {
  const encoder = new TextEncoder();
  for (let counter = 0; ; counter += 1) {
    const digest = new Uint8Array(
      await crypto.subtle.digest("SHA-256", encoder.encode(`${nonce}:${counter}`)),
    );
    let remaining = bits;
    let ok = true;
    for (const byte of digest) {
      if (remaining >= 8) {
        if (byte !== 0) { ok = false; break; }
        remaining -= 8;
        continue;
      }
      ok = byte >>> (8 - remaining) === 0;
      break;
    }
    if (ok) return String(counter);
  }
}

async function getSessionToken(): Promise<string> {
  const challenge = (await (await fetch(`${base}/challenge`)).json()) as {
    nonce: string; difficultyBits: number;
  };
  const solution = await solvePow(challenge.nonce, challenge.difficultyBits);
  const session = (await (
    await fetch(`${base}/session`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ nonce: challenge.nonce, solution }),
    })
  ).json()) as { token?: string; error?: string };
  if (!session.token) throw new Error(`/session に失敗: ${session.error ?? "unknown"}`);
  return session.token;
}

async function ask(token: string, q: Question): Promise<AskResponse> {
  const res = await fetch(`${base}/ask`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify({
      question: q.question,
      state: q.state ?? {},
      prev: q.prev ?? null,
      ...((noUnderstand || noLoop) ? { debug: { understand: !noUnderstand, loop: !noLoop } } : {}),
    }),
  });
  const body = (await res.json()) as AskResponse & { error?: string };
  if (!res.ok) {
    // HTTP エラー(429/502/503)は答えではない。集計に混ぜず、経路の故障として数える
    return { kind: "error", reason: `http_${res.status}`, error: body.error } as unknown as AskResponse;
  }
  return body;
}

function unitMatches(u: AskUnit, e: Expect): boolean {
  if (e.text_contains === undefined) return true;
  const text = u.text ?? Object.values(u.cells ?? {}).join(" ");
  return text.includes(e.text_contains);
}

async function runAsk(): Promise<void> {
  const token = await getSessionToken();
  const results: {
    q: Question; res: AskResponse; unitHit: boolean; pageHit: boolean;
  }[] = [];

  for (const q of questions) {
    const res = await ask(token, q);
    const steps = res.steps ?? [];
    const units = steps.flatMap((s) => s.units);
    const unitHit = q.expect.some((e) => units.some((u) => (e.text_contains === undefined ? true : unitMatches(u, e))));
    const pageHit = q.expect.length === 0 ? false : q.expect.some((e) =>
      units.some((u) => u.id.includes(`${e.page}/`) || u.id.includes(`:${e.page}/`)),
    );
    results.push({ q, res, unitHit, pageHit });
  }

  const withExpect = results.filter((r) => r.q.expect.length > 0);
  const withoutExpect = results.filter((r) => r.q.expect.length === 0 && r.q.kind !== "smalltalk" && r.q.kind !== "other" && r.q.kind !== "damage_calc");
  const noneRateOf = (rows: typeof results): string => {
    const n = rows.length;
    const noneCount = rows.filter((r) => r.res.kind === "none").length;
    return `${noneCount}/${n} (${n ? Math.round((noneCount / n) * 100) : 0}%)`;
  };

  console.log(`/ask 評価(${noUnderstand ? "理解なし" : "理解あり"}、${questions.length} 問)`);
  console.log(`  期待ありの行一致: ${withExpect.filter((r) => r.unitHit).length}/${withExpect.length}`);
  console.log(`  期待ありの none 率: ${noneRateOf(withExpect)}`);
  console.log(`  期待なし(absent)の none 率: ${noneRateOf(withoutExpect)}`);

  const errored = results.filter((r) => r.res.kind === "error");
  if (errored.length > 0) {
    const byReason = new Map<string, number>();
    for (const r of errored) byReason.set(r.res.reason ?? "?", (byReason.get(r.res.reason ?? "?") ?? 0) + 1);
    console.log(`  HTTP エラー(答えではない): ${errored.length}/${results.length} ${[...byReason].map(([k, v]) => `${k}=${v}`).join(" ")}`);
  }
  const answered = results.filter((r) => r.res.kind === "answer");
  const leadAlive = answered.filter((r) => Array.isArray(r.res.lead) && r.res.lead.length > 0);
  const leadText = (r: (typeof results)[number]): string =>
    (r.res.lead as { t?: string }[] | undefined)?.map((s) => s.t ?? "").join("") ?? "";
  const leadTone = leadAlive.filter((r) => leadText(r).trimEnd().endsWith("ッピ。") || leadText(r).trimEnd().endsWith("ッピ"));
  console.log(`  lead 生存率: ${leadAlive.length}/${answered.length}`);
  console.log(`  lead の口調(「ッピ」で終わる): ${leadTone.length}/${leadAlive.length}`);

  const verdicts = new Map<string, number>();
  for (const r of answered) verdicts.set(r.res.verdict ?? "?", (verdicts.get(r.res.verdict ?? "?") ?? 0) + 1);
  console.log(`  verdict の分布: ${[...verdicts.entries()].map(([k, v]) => `${k}=${v}`).join(" ")}`);

  // 安い道で解けなかった = none、または missing が空でない(部分的な答え)。verification_failed は none に含まれる
  const cheapMiss = withExpect.filter((r) => r.res.kind === "none" || (r.res.missing?.length ?? 0) > 0);
  console.log(`  安い道で解けなかった率(none または missing あり): ${cheapMiss.length}/${withExpect.length}`);

  // 雑談・範囲外・ダメージ計算は「reason がその kind そのもの」で返るのが正解(damage_calc は計算タブへ渡す)
  const classified = results.filter((r) => ["smalltalk", "other", "damage_calc"].includes(r.q.kind));
  const kindOk = classified.filter((r) => r.res.kind === "none" && r.res.reason === r.q.kind);
  console.log(`  雑談/範囲外/ダメージ計算の kind 正誤: ${kindOk.length}/${classified.length}`);

  // route ごとの件数・行一致・none 率・ツール回数と所要時間の平均(dropped の "route" 行から拾う。段階 2)。
  const routeOf = (r: AskResponse): string => r.route ?? (r.kind === "error" ? "error" : "(none)");
  const loopStatsOf = (r: AskResponse): { toolCalls: number; ms: number } | null => {
    const entry = r.dropped?.find((d) => d.what === "route" && d.why.startsWith("loop:"));
    const m = entry?.why.match(/^loop:(\d+)回\/(\d+)ms$/);
    return m ? { toolCalls: Number(m[1]), ms: Number(m[2]) } : null;
  };
  const routes = new Map<string, typeof results>();
  for (const r of results) {
    const key = routeOf(r.res);
    routes.set(key, [...(routes.get(key) ?? []), r]);
  }
  console.log(`  route ごとの件数: ${[...routes.entries()].map(([k, v]) => `${k}=${v.length}`).join(" ")}`);
  for (const [key, rows] of routes) {
    if (key === "cheap" || key === "error" || key === "(none)") continue;
    const stats = rows.map((r) => loopStatsOf(r.res)).filter((s): s is { toolCalls: number; ms: number } => s !== null);
    const withExpectRows = rows.filter((r) => r.q.expect.length > 0);
    const unitHit = withExpectRows.filter((r) => r.unitHit).length;
    const noneCount = rows.filter((r) => r.res.kind === "none").length;
    const avg = (nums: number[]): string => (nums.length ? (nums.reduce((a, b) => a + b, 0) / nums.length).toFixed(1) : "-");
    console.log(
      `    ${key}: 行一致 ${unitHit}/${withExpectRows.length}  none率 ${noneCount}/${rows.length}` +
        `  平均ツール回数 ${avg(stats.map((s) => s.toolCalls))}  平均秒 ${avg(stats.map((s) => s.ms / 1000))}`,
    );
  }

  if (verbose) {
    for (const r of results) {
      console.log(`  ${r.q.id}  kind=${r.res.kind} reason=${r.res.reason ?? ""} verdict=${r.res.verdict ?? ""} unitHit=${r.unitHit}`);
    }
  }
}

// --- 段階 0: /search --------------------------------------------------------------

async function runSearch(): Promise<void> {
  interface Row { id: string; ok: boolean; pageOk: boolean; rank: number | null; query: string | null; top: string }
  const rows: Row[] = [];
  for (const q of questions) {
    if (q.expect.length === 0) continue; // 雑談・範囲外・wiki に無い問は段階 1 の指標
    const res = await fetch(`${base}/search?q=${encodeURIComponent(q.question)}&limit=${limit}`);
    const body = (await res.json()) as SearchResponse;
    let rank: number | null = null;
    body.search.forEach((hit, i) => {
      if (rank === null && q.expect.some((e) => matches(hit, e))) rank = i + 1;
    });
    const top = body.search[0] ? `${body.search[0].page} › ${body.search[0].section}` : "(0 件)";
    const pageOk = body.search.some((hit) => q.expect.some((e) => e.page === hit.page));
    rows.push({ id: q.id, ok: rank !== null, pageOk, rank, query: body.query, top });
  }

  const byGroup = (pred: (q: Question) => boolean): { n: number; hit: number; page: number } => {
    const ids = new Set(questions.filter(pred).map((q) => q.id));
    const rs = rows.filter((r) => ids.has(r.id));
    return { n: rs.length, hit: rs.filter((r) => r.ok).length, page: rs.filter((r) => r.pageOk).length };
  };
  const groups: [string, (q: Question) => boolean][] = [
    ["wiki(ページ名あり)", (q) => q.kind === "wiki" && !q.paraphrase],
    ["wiki(言い換え)", (q) => q.kind === "wiki" && q.paraphrase === true],
    ["続きの質問", (q) => q.kind === "followup"],
    ["全体", (q) => q.expect.length > 0],
  ];
  const pct = (a: number, n: number): string => `${n ? Math.round((a / n) * 100) : 0}%`;
  console.log(`再現率(上位 ${limit} 件に 期待ユニット / 期待ページ が入る率)`);
  for (const [name, pred] of groups) {
    const { n, hit, page } = byGroup(pred);
    console.log(`  ${name}: ユニット ${hit}/${n} (${pct(hit, n)})  ページ ${page}/${n} (${pct(page, n)})`);
  }
  const ranks = rows.filter((r) => r.rank !== null).map((r) => r.rank as number);
  const mrr = rows.length ? rows.reduce((s, r) => s + (r.rank ? 1 / r.rank : 0), 0) / rows.length : 0;
  console.log(`  MRR: ${mrr.toFixed(3)}  1 位率: ${ranks.filter((r) => r === 1).length}/${rows.length}`);
  console.log("外した問:");
  for (const r of rows.filter((r) => !r.ok)) console.log(`  ${r.id}  query=${r.query}  top=${r.top}`);
  if (verbose) for (const r of rows.filter((r) => r.ok)) console.log(`  ok ${r.id} rank=${r.rank}`);
}
