/**
 * /ask の記録(D1 `ask_log`)。質問と答えを運営が見返して、外した質問の傾向と 1 人あたりの費用を把握する。
 * user は端末 ID のハッシュ(auth.ts userHash)。IP も装備の中身も入らない。
 * 書き込みの失敗は答えに影響させない(呼び元は waitUntil で投げっぱなし)。
 */
import type { PrevTurn } from "./prompt";
import type { RunAskResult } from "./index";

export interface AskLogInput {
  user: string;
  question: string;
  qkey: string | null;
  prev: PrevTurn | null;
  state: Record<string, number>;
  result: RunAskResult | { status: 500; body: { error: string } };
  ms: number;
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
    await db
      .prepare(
        `INSERT INTO ask_log (at, user, question, qkey, prev_page, state, kind, reason, route, answer_id, lead, body, ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)`,
      )
      .bind(
        new Date().toISOString(), input.user, input.question, input.qkey, input.prev?.page ?? null, JSON.stringify(input.state),
        kind, reason, route, answerId, lead, JSON.stringify(result.body), input.ms,
      )
      .run();
  } catch (error) {
    console.error("ask_log の書き込みに失敗", error);
  }
}
