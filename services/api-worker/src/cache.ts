/**
 * 答えのキャッシュ(2026-09-23)。「役に立った」が付いた答えを、同じ意味の質問にそのまま返す
 * (LLM を呼ばない = 費用ゼロ・即答)。
 *
 * - キー(`qkey`): 質問を分かち書きして語を正規化・重複除去・ソートした集合。語順と
 *   助詞・空白・全半角の違いは吸収し、語が 1 つでも違えば別の質問。続きの質問(prev あり)は
 *   文脈依存なのでキーを作らない(貯めない・引かない)
 * - キャラ状態はキーに入れない(質問の多くは状態に依らない)。答えが状態で行を絞っていたとき
 *   (steps の filtered_by がある)だけ、その状態を一緒に貯め、同じ状態のときにだけ返す
 * - 書く: /react の helpful の 1 回目。ask_log から qkey と答えの JSON を引いて KV に置く
 * - 引く: /ask の理解より前。wiki の同期時刻(meta)が変わっていたら捨てる(訂正の投入も units.sql と一緒に来る)
 * - KV の TTL は 30 日。「違った」が付いても消さない(helpful が先に付いた答えだけが入っている)
 */
import { fold, normalize, segment } from "./segment";
import { sha256Hex } from "./auth";
import { newAnswerId } from "./answer";
import type { AnswerResponse } from "./answer";

const TTL_SECONDS = 60 * 60 * 24 * 30;

export function questionKey(question: string, dict: readonly string[]): string {
  return [...new Set(segment(normalize(question), dict).map(fold))].filter(Boolean).sort().join(" ");
}

/** wiki の版。これが変わったキャッシュは使わない */
export function wikiVersion(meta: Record<string, string | undefined>): string {
  return `${meta.synced_at ?? ""}/${meta.imported_at ?? ""}`;
}

interface Entry {
  version: string;
  body: AnswerResponse;
  /** 答えが行の絞り込みに使ったキャラ状態。使っていなければ null(誰にでも返す) */
  stateUsed: Record<string, number> | null;
}

/** 答えがキャラ状態で行を絞っていたか(answer.ts filteredByOf → steps[].filtered_by)。 */
function usesState(body: AnswerResponse): boolean {
  return body.steps.some((s) => s.filtered_by && Object.keys(s.filtered_by).length > 0);
}

async function kvKey(qkey: string): Promise<string> {
  return `answer:${await sha256Hex(qkey)}`;
}

export async function getCachedAnswer(
  kv: KVNamespace,
  qkey: string,
  version: string,
  state: Record<string, number>,
): Promise<AnswerResponse | null> {
  const raw = await kv.get(await kvKey(qkey));
  if (!raw) return null;
  let entry: Entry;
  try {
    entry = JSON.parse(raw) as Entry;
  } catch {
    return null;
  }
  if (entry.version !== version || entry.body?.kind !== "answer") return null;
  if (entry.stateUsed && Object.entries(entry.stateUsed).some(([k, v]) => state[k] !== v)) return null;
  return {
    ...entry.body,
    answer_id: newAnswerId(), // この人の反応は別に数える
    route: "cached",
    followup: null,
    dropped: [...entry.body.dropped, { what: "route", why: "cached" }],
  };
}

export async function putCachedAnswer(
  kv: KVNamespace,
  qkey: string,
  version: string,
  body: AnswerResponse,
  state: Record<string, number>,
): Promise<void> {
  const entry: Entry = {
    version,
    body: { ...body, route: body.route === "cached" ? "cheap" : body.route },
    stateUsed: usesState(body) ? state : null,
  };
  await kv.put(await kvKey(qkey), JSON.stringify(entry), { expirationTtl: TTL_SECONDS });
}
