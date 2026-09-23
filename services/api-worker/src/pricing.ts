/**
 * Claude API の単価(管理画面の費用表示用)。確認済み: Anthropic 公式単価(2026-09-23)、
 * 百万トークンあたり USD。未知モデルは費用を出さない(0 円に化かさない。呼び元は null を「不明」と表示する)。
 */
export interface ModelPricing {
  /** 通常入力(キャッシュに当たらなかった分)。 */
  input: number;
  /** キャッシュ書き込み(5 分 TTL)。 */
  cacheWrite: number;
  /** キャッシュ読み取り。 */
  cacheRead: number;
  output: number;
}

export const PRICING: Record<string, ModelPricing> = {
  "claude-haiku-4-5": { input: 1.0, cacheWrite: 1.25, cacheRead: 0.1, output: 5.0 },
};

export interface UsageForCost {
  model: string;
  inputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  outputTokens: number;
}

/** USD。単価が未知のモデルは null(呼び元は「不明」と表示し、0 に丸めない)。 */
export function costUsd(usage: UsageForCost): number | null {
  const price = PRICING[usage.model];
  if (!price) return null;
  return (
    (usage.inputTokens * price.input +
      usage.cacheReadTokens * price.cacheRead +
      usage.cacheCreationTokens * price.cacheWrite +
      usage.outputTokens * price.output) /
    1_000_000
  );
}
