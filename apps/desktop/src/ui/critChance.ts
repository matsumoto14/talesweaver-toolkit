// クリ率の段階表示(6 段)。計算タブとホームのスポットライトで共有する。
// design-system の状態色の段に収める(新しい色は作らない)。
import type { StateKey } from "./states";

export const CRIT_CHANCE_STAGES: { max: number; label: string; state: StateKey }[] = [
  { max: 0, label: "出ない", state: "unknown" },
  { max: 25, label: "まれ", state: "short" },
  { max: 50, label: "ときどき", state: "edge" },
  { max: 75, label: "半分以上", state: "edge" },
  { max: 100, label: "ほぼ確定", state: "met" },
  { max: Infinity, label: "確定", state: "goal" },
];

/** クリ率(%)→ 段階。閾値は「言葉の実感」に合わせた区切りで、判定には使わない */
export const critChanceStage = (p: number) => {
  if (p <= 0) return CRIT_CHANCE_STAGES[0];
  if (p < 25) return CRIT_CHANCE_STAGES[1];
  if (p < 50) return CRIT_CHANCE_STAGES[2];
  if (p < 75) return CRIT_CHANCE_STAGES[3];
  if (p < 100) return CRIT_CHANCE_STAGES[4];
  return CRIT_CHANCE_STAGES[5];
};
