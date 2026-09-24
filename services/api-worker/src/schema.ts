/**
 * LLM の構造化出力のスキーマ(段階 1)。Zod v4 で定義し、`zodOutputFormat` で
 * Claude API の `output_config.format` に渡す(`src/claude.ts`)。
 *
 * enum は毎回同じ固定スロット(u01〜u40、p01〜p10)。候補 ID から毎回スキーマを
 * 作らない(新しいスキーマは初回にコンパイルが走り、24 時間のキャッシュは同一
 * スキーマにしか効かない)。件数・長さの上限はスキーマでは縛れない(API 側が
 * minItems/maxItems を受け付けない)ので、`verify.ts` がコードで切る。
 */
import { z } from "zod";

/** 手順の最大数。 */
export const MAX_STEPS = 4;
/** 1 手順あたりのユニット最大数(塊で数える)。 */
export const MAX_UNITS_PER_STEP = 4;
/** 安い道が 1 回に渡す候補の上限。 */
export const MAX_CANDIDATES = 20;
/** 札の総数(回す道はツールを重ねて最大 40 まで積む)。enum はこちら 1 種類。 */
export const MAX_SLOTS = 40;
/** 質問文の続きへ加点する、理解のページ候補の上限。 */
export const MAX_PAGE_SLOTS = 10;

/** u01〜u40。どちらの道でも同じ 1 種類。 */
export const SLOTS = Array.from(
  { length: MAX_SLOTS },
  (_, i) => `u${String(i + 1).padStart(2, "0")}`,
) as [string, ...string[]];

/** p01〜p10。理解のページ候補。 */
export const PAGE_SLOTS = Array.from(
  { length: MAX_PAGE_SLOTS },
  (_, i) => `p${String(i + 1).padStart(2, "0")}`,
) as [string, ...string[]];

/** 聞かれた観点。理解と選択の missing/aspects で共有する。 */
export const ASPECTS = ["what", "how", "materials", "where", "condition", "numbers"] as const;
export type Aspect = (typeof ASPECTS)[number];

/** 理解(1 回目)の出力。画面には出ない(検索にしか使わない)ので、内容の検証は要らない。 */
/**
 * 理解の欄は段階 3 で 7 つになった(Issue #36「気分・困りごとの型は段階 3」)。観点はまだ足していない。
 */
export const UnderstandSchema = z.object({
  kind: z.enum(["wiki", "smalltalk", "other"]),
  /** 候補(p01〜p10)から 0〜3 件。 */
  pages: z.array(z.enum(PAGE_SLOTS)),
  /** 最大 6 語・各 20 字。超過はコードが切る。検索にしか使わない。 */
  terms: z.array(z.string()),
  /** 1 回の検索で済むか、たどる必要があるか。multi は回す道へ(段階 2 では記録だけ)。 */
  hops: z.enum(["single", "multi"]),
  /** 直前の質問・ページの続きか。true かつ直前のページが実在すれば候補集めで加点する。 */
  followup: z.boolean(),
  /** 気分。trouble なら答えの前にコードの定型文で一言寄り添う(画面には出ない欄そのものは出さない)。 */
  mood: z.enum(["ask", "trouble"]),
  /** 困りごとの型。cant_win なら端末が打ち手(Playbook)を描く。画面には出ない。 */
  trouble: z.enum(["none", "cant_win"]),
});
export type Understand = z.infer<typeof UnderstandSchema>;

/** 何もしない・空の理解。1 回目が落ちたときのコード経路が使う。 */
export const NONE_UNDERSTAND: Understand = {
  kind: "wiki",
  pages: [],
  terms: [],
  hops: "single",
  followup: false,
  mood: "ask",
  trouble: "none",
};

/** 選択(2 回目)の 1 手順ぶん。 */
export const SelectionStepSchema = z.object({
  /** 候補外は API 側で出せない(enum 制約)。未使用スロットは verify が落とす。 */
  units: z.array(z.enum(SLOTS)),
  /** enum にしない(表ごとに違う)。実在は verify が見る。 */
  columns: z.array(z.string()),
  /** units と同じ順で、行のキー列の値。verify が元行と突合する。 */
  key_check: z.array(z.string()),
});
export type SelectionStep = z.infer<typeof SelectionStepSchema>;

/** 選択(2 回目)の出力。§選択の契約(s4)/ 実装(s12)のスキーマそのまま。 */
export const SelectionSchema = z.object({
  /** true なら「該当なし」経路。 */
  none: z.boolean(),
  /** はい/いいえで答えられる質問なら yes / no / depends。それ以外は none。 */
  verdict: z.enum(["yes", "no", "depends", "none"]),
  /** 結論の根拠にしたスロット。選んだユニットの中から。 */
  basis: z.array(z.enum(SLOTS)),
  /** 聞かれた観点のうち、候補に答えが無かったもの。 */
  missing: z.array(z.enum(ASPECTS)),
  /** 答え全体で 1 つ。ゼリッピの口調。表の値は {{スロット.列}} で参照。計算した値は式と結果を地の文に書いてよい
   *  (合計・確率の掛け算など。段階 4、ADR-020)。書いたら `computed` を true にする。 */
  lead: z.string(),
  /** lead に LLM が計算した値(参照の言い換えでない、掛け算・合計などの結果)が含まれるか。
   *  画面はこれが true のとき「AI の計算」の印を出す(サーバーは式を再計算しない)。 */
  computed: z.boolean(),
  steps: z.array(SelectionStepSchema),
});
export type Selection = z.infer<typeof SelectionSchema>;

/** 該当なし(LLM が none、または理解 1 回目が落ちてコード経路に進むとき)。 */
export const NONE_SELECTION: Selection = {
  none: true,
  verdict: "none",
  basis: [],
  missing: [],
  lead: "",
  computed: false,
  steps: [],
};
