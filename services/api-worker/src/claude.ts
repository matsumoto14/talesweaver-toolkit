/**
 * Claude 呼び出し。理解(1 回目)と選択(2 回目)。テストではこのモジュールを丸ごと
 * 差し替える(`vi.mock("../src/claude")`)。env から client を作る関数はここ 1 か所
 * (`createClient`)にまとめる。
 *
 * モデルは両方 claude-haiku-4-5、thinking なし、キャッシュなし。AI Gateway 経由なら
 * `cf-aig-collect-log: false` を毎回付ける(質問文をログに残さない)。
 */
import Anthropic from "@anthropic-ai/sdk";
import { zodOutputFormat } from "@anthropic-ai/sdk/helpers/zod";

import { SYSTEM_RULES, renderCandidates, renderUnderstandPrompt } from "./prompt";
import type { PrevTurn } from "./prompt";
import type { Candidate } from "./retrieve";
import { NONE_SELECTION, NONE_UNDERSTAND, SelectionSchema, UnderstandSchema } from "./schema";
import type { Selection, Understand } from "./schema";
import { assignSlots } from "./tools";

export interface ClaudeEnv {
  ANTHROPIC_API_KEY: string;
  AI_GATEWAY_URL?: string;
  SELECT_MODEL: string;
  UNDERSTAND_MODEL: string;
}

const AI_GATEWAY_HEADERS = { "cf-aig-collect-log": "false" };
const UNDERSTAND_TIMEOUT_MS = 4_000;
const SELECT_TIMEOUT_MS = 15_000;
const UNDERSTAND_MAX_TOKENS = 1024;
const SELECT_MAX_TOKENS = 4096;

/** env から Anthropic クライアントを作る、唯一の場所。 */
export function createClient(env: ClaudeEnv): Anthropic {
  return new Anthropic({ apiKey: env.ANTHROPIC_API_KEY, baseURL: env.AI_GATEWAY_URL || undefined });
}

/** 1 回の呼び出しの記録(管理画面の費用・時間表示用)。応答が来た(refusal も含む)ときだけ作る。 */
export interface CallInfo {
  model: string;
  ms: number;
  inputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  outputTokens: number;
}

function callInfoOf(model: string, usage: Anthropic.Usage | undefined, startedAt: number): CallInfo {
  return {
    model,
    ms: Date.now() - startedAt,
    inputTokens: usage?.input_tokens ?? 0,
    cacheReadTokens: usage?.cache_read_input_tokens ?? 0,
    cacheCreationTokens: usage?.cache_creation_input_tokens ?? 0,
    outputTokens: usage?.output_tokens ?? 0,
  };
}

export interface UnderstandResult {
  understanding: Understand;
  call: CallInfo;
}

/**
 * 理解(1 回目)。落ちても止まらない: エラー・時間切れ・`max_tokens` 切れは `null`(呼び元はコード
 * 経路で進む。1 回だけの呼び出しで、再試行はしない)。拒否(`refusal`)は経路の故障ではないので、
 * 空の理解(何も選ばず kind: "wiki" のまま)を返す。
 */
export async function understand(
  env: ClaudeEnv,
  question: string,
  prev: PrevTurn | null,
  pageCandidates: { slot: string; page: string }[],
): Promise<UnderstandResult | null> {
  const client = createClient(env);
  const startedAt = Date.now();
  try {
    const response = await client.messages.parse(
      {
        model: env.UNDERSTAND_MODEL,
        max_tokens: UNDERSTAND_MAX_TOKENS,
        system: "wiki への質問かどうかと、検索の手がかりを内部用に決めるだけ。ここで書く文字は画面に出ない。",
        messages: [{ role: "user", content: renderUnderstandPrompt(question, prev, pageCandidates) }],
        output_config: { format: zodOutputFormat(UnderstandSchema) },
      },
      { headers: AI_GATEWAY_HEADERS, timeout: UNDERSTAND_TIMEOUT_MS },
    );
    const call = callInfoOf(env.UNDERSTAND_MODEL, response.usage, startedAt);
    if (response.stop_reason === "refusal") return { understanding: NONE_UNDERSTAND, call };
    if (!response.parsed_output) return null;
    return { understanding: response.parsed_output, call };
  } catch {
    return null;
  }
}

export interface SelectResult {
  selection: Selection;
  slotToId: Map<string, string>;
  call: CallInfo;
}

/**
 * 選択(2 回目)。`null` は経路の故障(JSON が壊れている・`max_tokens` で切れた・API エラー)。
 * 呼び元が 1 回だけ再試行し、それでも `null` なら 502(該当なしには化かさない)。
 * 拒否(`refusal`)は経路の故障ではないので、該当なし(`NONE_SELECTION`)を返す。
 */
export async function select(
  env: ClaudeEnv,
  question: string,
  state: Record<string, number>,
  candidates: Candidate[],
  columnNotes: Record<string, string>,
): Promise<SelectResult | null> {
  const { slotToId, idToSlot } = assignSlots(candidates);
  const slotOf = (id: string): string => idToSlot.get(id) ?? id;

  const client = createClient(env);
  const startedAt = Date.now();
  try {
    const response = await client.messages.parse(
      {
        model: env.SELECT_MODEL,
        max_tokens: SELECT_MAX_TOKENS,
        system: SYSTEM_RULES,
        messages: [
          { role: "user", content: renderCandidates(question, state, candidates, slotOf, columnNotes) },
        ],
        output_config: { format: zodOutputFormat(SelectionSchema) },
      },
      { headers: AI_GATEWAY_HEADERS, timeout: SELECT_TIMEOUT_MS },
    );
    const call = callInfoOf(env.SELECT_MODEL, response.usage, startedAt);
    if (response.stop_reason === "refusal") return { selection: NONE_SELECTION, slotToId, call };
    if (!response.parsed_output) return null;
    return { selection: resolveSlots(response.parsed_output, slotToId), slotToId, call };
  } catch {
    return null;
  }
}

/**
 * スロットを安定 ID に戻す。units も `basis` も lead の `{{u02.列}}` も。未使用スロット
 * (候補が 12 件のときの u13 以降など)はそのまま残り、`verify()` が `unknown_id` で落とす。
 */
export function resolveSlots(sel: Selection, slotToId: Map<string, string>): Selection {
  const id = (slot: string): string => slotToId.get(slot) ?? slot;
  return {
    ...sel,
    lead: sel.lead.replace(/\{\{(u\d\d)\./g, (_match, slot: string) => `{{${id(slot)}.`),
    basis: sel.basis.map(id),
    steps: sel.steps.map((s) => ({ ...s, units: s.units.map(id) })),
  };
}
