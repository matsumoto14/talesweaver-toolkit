import type { StatLayer } from "./api/types";

export const fmtInt = (n: number) => n.toLocaleString("ja-JP");

/** 小数は最大4桁、整数は桁区切りのみ */
export const fmtNum = (n: number) =>
  n.toLocaleString("ja-JP", { maximumFractionDigits: 4 });

/**
 * 補正源のレイヤーに応じた値の整形。CharacterSettings のバフ選択肢表示・TracePanel の
 * 寄与内訳表示の両方で使う共通ロジック(2機能で使うため ui/ ではなくここに置く)。
 */
export function formatLayerValue(layer: StatLayer, raw: number): string {
  switch (layer) {
    case "percent_of_base":
    case "multiplier_b": {
      const pct = Math.round(raw * 100);
      return `${pct >= 0 ? "+" : ""}${pct}%`;
    }
    case "multiplier_a":
      return `×${raw.toFixed(2)}`;
    case "fixed":
    case "final_fixed": {
      const v = Math.round(raw);
      return `${v >= 0 ? "+" : ""}${v}`;
    }
  }
}
