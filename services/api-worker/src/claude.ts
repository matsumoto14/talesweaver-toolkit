/**
 * Claude 呼び出し。理解(1 回目)・節選び(理解がページを選んだときだけ)・選択(2 回目)。テストではこのモジュールを丸ごと
 * 差し替える(`vi.mock("../src/claude")`)。env から client を作る関数はここ 1 か所
 * (`createClient`)にまとめる。
 *
 * モデルは wrangler.toml の UNDERSTAND_MODEL / SELECT_MODEL(既定は両方 claude-haiku-4-5)。キャッシュは理解のページ一覧だけ。AI Gateway 経由なら
 * `cf-aig-collect-log: false` を毎回付ける(質問文をログに残さない)。
 */
import Anthropic from "@anthropic-ai/sdk";
import { zodOutputFormat } from "@anthropic-ai/sdk/helpers/zod";

import { SYSTEM_RULES, renderCandidates, renderSectionPrompt, renderUnderstandPrompt } from "./prompt";
import type { PrevTurn } from "./prompt";
import type { Candidate } from "./retrieve";
import { NONE_SELECTION, NONE_UNDERSTAND, SectionPickSchema, SelectionSchema, UnderstandSchema } from "./schema";
import type { Selection, Understand } from "./schema";
import { assignSlots } from "./tools";

export interface ClaudeEnv {
  ANTHROPIC_API_KEY: string;
  AI_GATEWAY_URL?: string;
  SELECT_MODEL: string;
  UNDERSTAND_MODEL: string;
  /** 選択・回す道の effort(low〜max)。空なら送らない(Haiku 4.5 は effort を受け付けない)。 */
  SELECT_EFFORT?: string;
}

type Effort = "low" | "medium" | "high" | "xhigh" | "max";

/** SELECT_EFFORT を output_config に足す形で返す。未設定なら空。 */
export function effortConfig(env: ClaudeEnv): { effort?: Effort } {
  const e = env.SELECT_EFFORT;
  return e === "low" || e === "medium" || e === "high" || e === "xhigh" || e === "max" ? { effort: e } : {};
}

/**
 * tool_choice で特定のツールを強制できるモデルか。Haiku 4.5 だけ。思考が既定で走るモデル(Sonnet 5 以降)は
 * 強制と思考が両立せず、Opus 5.5 / Fable 5.1 は強制そのものを 400 で拒む。強制できないときはツールを
 * answer 1 本に絞って auto で呼ぶ(agent.ts)。
 */
export function supportsForcedToolChoice(model: string): boolean {
  return model.startsWith("claude-haiku-");
}

const AI_GATEWAY_HEADERS = { "cf-aig-collect-log": "false" };

/** 理解・節選びの system(eval/harness.test.ts もこれを使う。プロンプトを二重に持たない)。 */
export const UNDERSTAND_SYSTEM = "wiki への質問かどうかと、検索の手がかりを内部用に決めるだけ。ここで書く文字は画面に出ない。";
export const SECTION_SYSTEM = "wiki の目次から、質問の答えが書いてある節を内部用に選ぶだけ。ここで書く文字は画面に出ない。";
const UNDERSTAND_TIMEOUT_MS = 4_000;
const SELECT_TIMEOUT_MS = 30_000;
const UNDERSTAND_MAX_TOKENS = 1024;
const SECTION_MAX_TOKENS = 256;
// 思考が走るモデル(Sonnet 5 / Opus 5.5)は思考も max_tokens に数えるので余裕を持たせる
const SELECT_MAX_TOKENS = 16_000;

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
  pageMatches: string[],
  pageDirectory: string,
): Promise<UnderstandResult | null> {
  const client = createClient(env);
  const startedAt = Date.now();
  try {
    const response = await client.messages.parse(
      {
        model: env.UNDERSTAND_MODEL,
        max_tokens: UNDERSTAND_MAX_TOKENS,
        // ページ一覧は毎回同じなので system の末尾に置いてキャッシュする(取込で変わったら 1 回書き直すだけ)
        system: [
          { type: "text", text: UNDERSTAND_SYSTEM },
          { type: "text", text: pageDirectory, cache_control: { type: "ephemeral" } },
        ],
        messages: [{ role: "user", content: renderUnderstandPrompt(question, prev, pageMatches) }],
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

export interface PickSectionsResult {
  sections: { page: string; anchor: string }[];
  call: CallInfo;
}

/**
 * 節選び(理解がページを選んだときだけ)。落ちても止まらない: エラー・時間切れは `null`(呼び元は
 * 全文検索だけで進む)。目次の札を (page, anchor) に戻し、実在しない札は捨てる。最大 2 件。
 */
export async function pickSections(
  env: ClaudeEnv,
  question: string,
  outline: { page: string; anchor: string; section: string }[],
): Promise<PickSectionsResult | null> {
  if (outline.length === 0) return null;
  const slotted = outline.map((o, i) => ({ ...o, slot: `s${String(i + 1).padStart(2, "0")}` }));
  const bySlot = new Map(slotted.map((o) => [o.slot, o]));
  const client = createClient(env);
  const startedAt = Date.now();
  try {
    const response = await client.messages.parse(
      {
        model: env.UNDERSTAND_MODEL,
        max_tokens: SECTION_MAX_TOKENS,
        system: SECTION_SYSTEM,
        messages: [{ role: "user", content: renderSectionPrompt(question, slotted) }],
        output_config: { format: zodOutputFormat(SectionPickSchema) },
      },
      { headers: AI_GATEWAY_HEADERS, timeout: UNDERSTAND_TIMEOUT_MS },
    );
    const call = callInfoOf(env.UNDERSTAND_MODEL, response.usage, startedAt);
    if (response.stop_reason === "refusal" || !response.parsed_output) return { sections: [], call };
    const sections = response.parsed_output.sections
      .map((s) => bySlot.get(s))
      .filter((o): o is (typeof slotted)[number] => !!o)
      .slice(0, 2)
      .map((o) => ({ page: o.page, anchor: o.anchor }));
    return { sections, call };
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
        output_config: { format: zodOutputFormat(SelectionSchema), ...effortConfig(env) },
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
