import type { StatLayer } from "./api/types";

export const fmtInt = (n: number) => n.toLocaleString("ja-JP");

/** ISO8601(YYYY-MM-DD)を MM-DD に。ホームの「最後の強化」とお知らせの公開日で使う */
export const fmtMonthDay = (iso: string) => {
  const d = new Date(iso);
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

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

export interface TopRows {
  /** 上位 `max` 件のラベル */
  shown: string[];
  /** 割愛した件数(0 なら全件出している) */
  restCount: number;
  /** 割愛した行そのもの。ラベルだけでは分からない「ほか n の中身」を辿るのに使う */
  restRows: { label: string; value: number }[];
}

/**
 * 効きの大きい順に並べ、上位 `max` 件と「割愛した残り」に分ける。
 * 全ステに乗るバフのチップ・カードは 7 件そのまま並べると幅からはみ出す
 * (design-review A4)。バフタブ(BuffsPage)・計算タブ(CalcPage)の両方でここから出し、
 * 出す量そのものを減らすことで一貫させる(幅や折り返しで症状だけ消さない)。
 * 値の出どころ(どの行を上位とみなすか)を 1 本にするための共通ロジック本体。
 */
export function topRows(rows: { label: string; value: number }[], max = 2): TopRows {
  const sorted = [...rows].sort((a, b) => Math.abs(b.value) - Math.abs(a.value));
  return { shown: sorted.slice(0, max).map((r) => r.label), restCount: Math.max(0, sorted.length - max), restRows: sorted.slice(max) };
}

/** `topRows` を「上位 `max` 件 + 『ほか n』」の 1 行テキストに畳んだもの。計算タブ右ペインなど、
 *  割愛した中身を辿る手段が要らない(狭い・一覧性より要約優先の)場所で使う。 */
export function topRowsText(rows: { label: string; value: number }[], max = 2): string {
  if (rows.length === 0) return "";
  const { shown, restCount } = topRows(rows, max);
  const parts = [...shown];
  if (restCount > 0) parts.push(`ほか ${restCount}`);
  return parts.join(" / ");
}
