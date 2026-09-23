/**
 * /ask の記録(D1 `ask_log` + `ask_call`)。質問と答えを運営が見返して、外した質問の傾向と
 * 1 人あたりの費用を把握する。user は端末 ID のハッシュ(auth.ts userHash)。IP も装備の中身も入らない。
 * 書き込みの失敗は答えに影響させない(呼び元は waitUntil で投げっぱなし)。
 *
 * トレース(理解の出力・LLM に見せた候補・呼び出しごとの usage)は管理画面(admin.ts)専用で、
 * クライアント向けの応答 body には混ざらない。`ask_call` は `ask_log` の INSERT が返す
 * `meta.last_row_id` を使って batch で書く(失敗は握りつぶす。今までと同じ)。
 */
import type { CallInfo } from "./claude";
import type { PrevTurn } from "./prompt";
import type { RunAskResult } from "./index";

/** 1 回の LLM 呼び出しの記録。kind で理解・選択・回す道の往復を区別する。 */
export interface AskCallTrace extends CallInfo {
  kind: "understand" | "select" | "loop";
}

export interface AskTrace {
  /** 理解(1 回目)の出力 JSON。落ちてコード経路に進んだときもその値を残す。 */
  understanding: unknown;
  /** LLM に見せた候補(安い道は select、回す道は答えに使った候補)。 */
  candidates: { id: string; page: string; section: string }[];
  calls: AskCallTrace[];
}

export interface AskLogInput {
  user: string;
  question: string;
  qkey: string | null;
  prev: PrevTurn | null;
  state: Record<string, number>;
  result: RunAskResult | { status: 500; body: { error: string } };
  ms: number;
  trace?: AskTrace;
}

export async function logAsk(db: D1Database, input: AskLogInput): Promise<void> {
  const { result } = input;
  let kind: "answer" | "none" | "error";
  let reason: string | null = null;
  let route: string | null = null;
  let answerId: string | null = null;
  let lead: string | null = null;
  if (result.status !== 200) {
    kind = "error";
    reason = result.body.error;
  } else if (result.body.kind === "answer") {
    kind = "answer";
    route = result.body.route;
    answerId = result.body.answer_id;
    lead = result.body.lead?.map((seg) => ("t" in seg ? seg.t : `{{${seg.ref}.${seg.col}}}`)).join("") ?? null;
  } else {
    kind = "none";
    reason = result.body.reason;
    const routeNote = result.body.dropped.find((d) => d.what === "route" && d.why.startsWith("loop:"));
    route = routeNote ? "loop" : null;
  }
  try {
    const inserted = await db
      .prepare(
        `INSERT INTO ask_log (at, user, question, qkey, prev_page, state, kind, reason, route, answer_id, lead, body, ms, understanding, candidates)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)`,
      )
      .bind(
        new Date().toISOString(), input.user, input.question, input.qkey, input.prev?.page ?? null, JSON.stringify(input.state),
        kind, reason, route, answerId, lead, JSON.stringify(result.body), input.ms,
        input.trace ? JSON.stringify(input.trace.understanding) : null,
        input.trace ? JSON.stringify(input.trace.candidates) : null,
      )
      .run();

    const askLogId = inserted.meta.last_row_id;
    if (input.trace && input.trace.calls.length > 0 && askLogId) {
      const statements = input.trace.calls.map((call, seq) =>
        db
          .prepare(
            `INSERT INTO ask_call (ask_log_id, seq, kind, model, input_tokens, cache_read_tokens, cache_creation_tokens, output_tokens, ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)`,
          )
          .bind(askLogId, seq, call.kind, call.model, call.inputTokens, call.cacheReadTokens, call.cacheCreationTokens, call.outputTokens, call.ms),
      );
      await db.batch(statements);
    }
  } catch (error) {
    console.error("ask_log の書き込みに失敗", error);
  }
}
