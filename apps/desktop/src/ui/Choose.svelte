<script lang="ts" module>
  // 並んだ中から選ぶ(design-system §07 形態 2「段階選択」/ 形態 3「チップ」/ §08 タブ)。
  // **段・チップ・タブはこれ 1 つ**。3 つは「並んだ中から選ぶ」という同じことで、
  // 違うのは見た目と、それが何を切り替えるかだけ。部品を分けると呼ぶ側に
  // 「チップならこっち、タブならこっち」を毎回決めさせることになる。
  //
  // 1 つ選ぶか、いくつか選ぶかは**持っているものが決める** —— `value`(1 つ)を渡せば
  // radio 群、`values`(いくつか)を渡せば checkbox 群。見た目の選択肢ではない。
  //
  // **キーボードは 1 行も書いていない。**同じ `name` の radio が、矢印キーでの移動・
  // Tab ストップが群れで 1 つになること・読み上げの「n 個中 m 個目」を既定で持つ。
  // 段は `Chip` が描く(`ReadRow` が `Value` の上に載っているのと同じ形)。
  //
  // 見た目は入れ物(段・粒・タブ)そのものが app.css に持つので、この部品は <style> を持たない。
  //
  // 押した瞬間に結果が動く(「適用」を挟まない)。押した段は同じ位置に残る(§09 規則 1)。
  // 押せない段は**消さない** — 消すと段の数が変わって幅が動く(§09 規則 4)。
  export interface ChooseOption {
    value: string;
    label: string;
  }
  /** radio 群の名前。群れごとに違えば中身は何でもいい */
  let seq = 0;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import Chip from "./Chip.svelte";

  interface Props {
    /**
     * この群が何を選ばせるか。**読み上げ用(aria-label)だけ**で、画面には出ない —
     * 見える見出しは呼ぶ側の行が持つ(`NumberField` / `TextField` / `Picker` と同じ契約。
     * app.css の `.field` / `.field-label` がその型)。段階 9 で揃えた。
     */
    label: string;
    /** 1 つだけ選ぶ。radio 群になる */
    value?: string;
    /** いくつか選ぶ。checkbox 群になる。並びは呼ぶ側が決める(押した順で入れ替わらないように) */
    values?: string[];
    /** `values` を渡したときの反映先 */
    onToggle?: (value: string, next: boolean) => void;
    options: ChooseOption[];
    /**
     * 入れ物の見た目。app.css が持つ 3 つ —— `seg`(段が地続き)/ `chiprow`(粒が離れて並ぶ)/
     * `tabs`(下の面と地続き)。段階 2〜4 と同じ「その画面の事実」の線
     */
    class?: string;
    disabled?: boolean;
    /** 幅いっぱいを段の数で割る(§08 `.seg.full`)。段が折り返すのを防ぐ */
    full?: boolean;
    /** 列を固定して並べる(§08 `.seg.cols`)。折り返しても行をまたいで幅が揃う */
    cols?: number;
    /**
     * 1 段の幅(px)。**行ごとに段の数が違うとき**に使う — 幅いっぱいに割ると
     * 1 段の大きさが行ごとに変わって、そろって見えない
     */
    cell?: number;
    /** 段ごとに足すクラス(属性のやんわりした面など)。`value -> class` */
    tone?: (value: string) => string | undefined;
    /** 押せない段。段そのものは消さない。理由は `titleFor` で読ませる */
    disabledValues?: string[];
    /** 段ごとの説明(hover)。押せない段の理由を出すのに使う */
    titleFor?: (value: string) => string | undefined;
    /**
     * 同時に選べる数の上限(`values` のときだけ)。上限に達した段は押せなくなる。
     * **「n/上限」の表示は呼ぶ側の見出しが持つ**(§07「上限は値の隣に常設する」)——
     * 見出しを呼ぶ側に渡した以上、その隣に置く数もそこにある。
     */
    max?: number;
    /** 段の中身を自分で描く(件数の `<Value>` を添えるときなど)。既定はラベルだけ */
    item?: Snippet<[ChooseOption]>;
  }
  let {
    label, value = $bindable(), values, onToggle, options, class: cls = "seg",
    disabled = false, full = false, cols, cell, tone, disabledValues = [], titleFor, max, item,
  }: Props = $props();

  const multiple = $derived(values !== undefined);
  const name = `choose-${seq++}`;
  const atMax = $derived(max !== undefined && (values?.length ?? 0) >= max);
</script>

<div
  class={cls}
  class:full
  class:cols={cols !== undefined}
  class:fixed={cell !== undefined}
  style={cols === undefined
    ? undefined
    : `--seg-cols: ${cols}${cell === undefined ? "" : `; --seg-cell: ${cell}px`}`}
  role={multiple ? "group" : "radiogroup"}
  aria-label={label}
>
  {#each options as o (o.value)}
    {@const on = multiple ? (values ?? []).includes(o.value) : o.value === value}
    <Chip
      {on}
      name={multiple ? undefined : name}
      value={o.value}
      class={tone?.(o.value) ?? ""}
      disabled={disabled || disabledValues.includes(o.value) || (atMax && !on)}
      title={titleFor?.(o.value)}
      onToggle={() => (multiple ? onToggle?.(o.value, !on) : (value = o.value))}
    >
      {#if item}{@render item(o)}{:else}{o.label}{/if}
    </Chip>
  {/each}
</div>
