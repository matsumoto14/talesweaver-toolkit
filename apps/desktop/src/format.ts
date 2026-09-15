import type { StatLayer } from "./api/types";

// 数値書式はここだけが決める(design-system §08「数値の 3 段」)。画面側で toLocaleString / toFixed を
// 呼ばない(tools/design-audit/run.py R11 が見張る)。桁区切りは ja-JP、小数桁は呼び手が役割で指定する。

/** 小数桁。数値なら固定桁(1.50)、`{ max }` なら上限までで末尾の 0 を落とす(1.5 / 2) */
export type Digits = number | { max: number };

const formatters = new Map<string, Intl.NumberFormat>();
const formatter = (digits: Digits): Intl.NumberFormat => {
  const [min, max] = typeof digits === "number" ? [digits, digits] : [0, digits.max];
  const key = `${min}/${max}`;
  let f = formatters.get(key);
  if (!f) {
    f = new Intl.NumberFormat("ja-JP", { minimumFractionDigits: min, maximumFractionDigits: max });
    formatters.set(key, f);
  }
  return f;
};

/** 桁区切りつきの数。`unit` は値の直後に付ける(1.50s / 12.5%)。丸めて 0 になる負の値は「-0」にしない */
export const fmtNum = (n: number, digits: Digits = { max: 4 }, unit = ""): string => {
  const body = formatter(digits).format(n);
  return `${/^-0(?:\.0+)?$/.test(body) ? body.slice(1) : body}${unit}`;
};

/** 整数(件数・能力値・ダメージ)。小数が渡ったときは 3 桁まで出す(丸めない) */
export const fmtInt = (n: number): string => fmtNum(n, { max: 3 });

/** 割合(0–1)を % で。既に % 単位の値は fmtNum(v, digits, "%") */
export const fmtPct = (rate: number, digits: Digits = 0): string => fmtNum(rate * 100, digits, "%");

/** 符号つき(増減・補正値)。丸めた結果が 0 なら「+0」(-0.4 を「-0」にしない) */
export const fmtSigned = (n: number, digits: Digits = 0, unit = ""): string => {
  const body = fmtNum(n, digits, unit);
  return body.startsWith("-") ? body : `+${body}`;
};

/** 符号つきの割合(0–1)。「+12%」「-3.5%」 */
export const fmtSignedPct = (rate: number, digits: Digits = 0): string => fmtSigned(rate * 100, digits, "%");

/** 倍率。「×1.42」 */
export const fmtRate = (mult: number, digits: Digits = 2): string => `×${fmtNum(mult, digits)}`;

/** ISO8601(YYYY-MM-DD)を MM-DD に。ホームの「最後の強化」とお知らせの公開日で使う */
export const fmtMonthDay = (iso: string) => {
  const d = new Date(iso);
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

/**
 * 補正源のレイヤーに応じた値の整形。CharacterSettings のバフ選択肢表示・TracePanel の
 * 寄与内訳表示の両方で使う共通ロジック(2機能で使うため ui/ ではなくここに置く)。
 */
export function formatLayerValue(layer: StatLayer, raw: number): string {
  switch (layer) {
    case "percent_of_base":
    case "multiplier_b":
      return fmtSignedPct(raw);
    case "multiplier_a":
      return fmtRate(raw);
    case "fixed":
    case "final_fixed":
      return fmtSigned(raw);
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
