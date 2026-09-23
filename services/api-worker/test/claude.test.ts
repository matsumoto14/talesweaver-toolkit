// resolveSlots(): スロットを安定 ID に戻す(§実装 s12)。LLM は呼ばない純関数のテスト。
// understand() / select() の usage 抽出は Anthropic SDK 自体をモックして見る(下の describe)。
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockParse = vi.fn();
vi.mock("@anthropic-ai/sdk", () => ({
  default: class {
    messages = { parse: mockParse };
  },
}));

import { resolveSlots, select, understand } from "../src/claude";
import type { ClaudeEnv } from "../src/claude";
import { NONE_SELECTION, NONE_UNDERSTAND } from "../src/schema";
import type { Selection } from "../src/schema";

describe("resolveSlots", () => {
  it("units・basis・lead の {{u02.列}} をすべて安定 ID に戻す", () => {
    const slotToId = new Map([["u01", "p:テシスコア/top/1"], ["u02", "r:テシスコア/h2_1/1/進0-強0"]]);
    const sel: Selection = {
      none: false, verdict: "yes", basis: ["u02"], missing: [],
      lead: "進化 {{u02.進化}} の今は成功率 {{u02.成功率}} ッピ。",
      steps: [{ units: ["u01", "u02"], columns: ["成功率"], key_check: ["", "進0-強0"] }],
    };

    const resolved = resolveSlots(sel, slotToId);

    expect(resolved.basis).toEqual(["r:テシスコア/h2_1/1/進0-強0"]);
    expect(resolved.steps[0]?.units).toEqual(["p:テシスコア/top/1", "r:テシスコア/h2_1/1/進0-強0"]);
    expect(resolved.lead).toBe(
      "進化 {{r:テシスコア/h2_1/1/進0-強0.進化}} の今は成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} ッピ。",
    );
  });

  it("未使用スロット(候補外)はそのまま残る(verify が unknown_id で落とす)", () => {
    const slotToId = new Map([["u01", "p:テシスコア/top/1"]]);
    const sel: Selection = {
      none: false, verdict: "none", basis: [], missing: [], lead: "",
      steps: [{ units: ["u01", "u13"], columns: [], key_check: [] }],
    };

    const resolved = resolveSlots(sel, slotToId);

    expect(resolved.steps[0]?.units).toEqual(["p:テシスコア/top/1", "u13"]);
  });
});

const ENV: ClaudeEnv = { ANTHROPIC_API_KEY: "sk-ant-test", SELECT_MODEL: "claude-haiku-4-5", UNDERSTAND_MODEL: "claude-haiku-4-5" };

beforeEach(() => {
  mockParse.mockReset();
});

describe("understand() の usage 抽出", () => {
  it("成功時は response.usage から call を組む", async () => {
    mockParse.mockResolvedValue({
      stop_reason: "end_turn",
      parsed_output: { ...NONE_UNDERSTAND, kind: "wiki" },
      usage: { input_tokens: 120, output_tokens: 40, cache_read_input_tokens: 10, cache_creation_input_tokens: 5 },
    });

    const result = await understand(ENV, "テシスコアって?", null, []);

    expect(result).not.toBeNull();
    expect(result!.understanding.kind).toBe("wiki");
    expect(result!.call).toEqual({
      model: "claude-haiku-4-5", ms: expect.any(Number),
      inputTokens: 120, cacheReadTokens: 10, cacheCreationTokens: 5, outputTokens: 40,
    });
  });

  it("refusal は空の理解(NONE_UNDERSTAND)+ call を返す(経路の故障ではない)", async () => {
    mockParse.mockResolvedValue({ stop_reason: "refusal", parsed_output: null, usage: { input_tokens: 30, output_tokens: 1 } });

    const result = await understand(ENV, "x", null, []);

    expect(result).not.toBeNull();
    expect(result!.understanding).toEqual(NONE_UNDERSTAND);
    expect(result!.call.inputTokens).toBe(30);
  });

  it("API エラーは null(call も作らない)", async () => {
    mockParse.mockRejectedValue(new Error("timeout"));
    expect(await understand(ENV, "x", null, [])).toBeNull();
  });
});

describe("select() の usage 抽出", () => {
  it("成功時は response.usage から call を組む", async () => {
    mockParse.mockResolvedValue({
      stop_reason: "end_turn",
      parsed_output: { none: true, verdict: "none", basis: [], missing: [], lead: "", steps: [] },
      usage: { input_tokens: 500, output_tokens: 80, cache_read_input_tokens: 0, cache_creation_input_tokens: 200 },
    });

    const result = await select(ENV, "x", {}, [], {});

    expect(result).not.toBeNull();
    expect(result!.call).toEqual({
      model: "claude-haiku-4-5", ms: expect.any(Number),
      inputTokens: 500, cacheReadTokens: 0, cacheCreationTokens: 200, outputTokens: 80,
    });
  });

  it("refusal は該当なし(NONE_SELECTION)+ call を返す", async () => {
    mockParse.mockResolvedValue({ stop_reason: "refusal", parsed_output: null, usage: { input_tokens: 10, output_tokens: 1 } });

    const result = await select(ENV, "x", {}, [], {});

    expect(result).not.toBeNull();
    expect(result!.selection).toEqual(NONE_SELECTION);
    expect(result!.call.inputTokens).toBe(10);
  });
});
