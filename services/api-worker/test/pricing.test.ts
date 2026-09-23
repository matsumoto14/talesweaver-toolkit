// 単価計算(src/pricing.ts)。既知モデルの計算式と、未知モデルは null(0 円に化かさない)を見る。
import { describe, expect, it } from "vitest";

import { costUsd, PRICING } from "../src/pricing";

describe("costUsd", () => {
  it("既知モデルは 4 つの単価をトークン数で按分して合計する", () => {
    const usd = costUsd({
      model: "claude-haiku-4-5",
      inputTokens: 1_000_000,
      cacheReadTokens: 1_000_000,
      cacheCreationTokens: 1_000_000,
      outputTokens: 1_000_000,
    });
    const p = PRICING["claude-haiku-4-5"]!;
    expect(usd).toBeCloseTo(p.input + p.cacheRead + p.cacheWrite + p.output, 9);
  });

  it("トークン 0 なら 0 円", () => {
    expect(costUsd({ model: "claude-haiku-4-5", inputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 0 })).toBe(0);
  });

  it("未知モデルは null(0 円に化かさない)", () => {
    expect(costUsd({ model: "gpt-9", inputTokens: 100, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 10 })).toBeNull();
  });
});
