// resolveSlots(): スロットを安定 ID に戻す(§実装 s12)。LLM は呼ばない純関数のテスト。
import { describe, expect, it } from "vitest";

import { resolveSlots } from "../src/claude";
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
