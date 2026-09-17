<script lang="ts">
  // 値を出す 1 つの部品(design-system §08 数値 / §10 型 1)。**値に関する作法をこれが全部持つ**:
  // 数値書体と tabular-nums(`num`)・未収録の「?」・変わったら跳ねる・増減の差分枠。
  //
  // 呼ぶ側は「どう動かすか」を決めない。`motion`(値の元になる数)を渡せば上下が分かるので跳ね、
  // 渡せない値(文字・要約)は書式済みの文字が変わったら光る — 選ぶのではなく、数があるかで決まる。
  // `ui/ReadRow.svelte` はこの上に載っている(行に載る値)。行に載らない値がこれを直接使う。
  import type { Snippet } from "svelte";
  import { bump, changed, delta as deltaAction } from "./motion.svelte";

  interface Props {
    /** 書式済みの値(format.ts を通したもの)。null = 未収録 → 「?」を出す。0 や空白にしない(§00)。
     *  省略すると値そのものを出さない — 値が別の場所にあって、ここには差分枠だけを置きたいとき
     *  (計算タブの鎖: 主役の数字の下の行に「↑1,234」だけを出す) */
    value?: string | null;
    /** 値の元になる数。あれば増減が分かるので跳ねる。無ければ書式済みの文字の変化で光る */
    motion?: () => number | null;
    /** 値の右に増減(↑1,234)を出す。`motion` があるときだけ意味を持つ */
    delta?: { unit?: string; digits?: number } | null;
    /** 値の色。up = 伸びた(緑) / down = 減った(赤) / sim = 試し変更(ラベンダー) */
    tone?: "up" | "down" | "sim" | null;
    /** その画面での見た目(大きさ・色・幅)。作法ではなく画面の事実 */
    class?: string;
    /** 差分枠に付ける見た目(「変わったところを開く」の下線など) */
    deltaClass?: string;
    /** 差分枠を押したときの行き先(計算タブの followChange) */
    onDelta?: (e: MouseEvent) => void;
    title?: string;
    /** 値の中に別の書体・色の部分があるときだけ(「12 / 30」の分母など)。
     *  渡したときも `value` は残す — 光るかどうかを見る「変わったか」の合図として使う */
    children?: Snippet;
  }
  let {
    value,
    motion,
    delta = null,
    tone = null,
    class: klass = "",
    deltaClass = "",
    onDelta,
    title,
    children,
  }: Props = $props();

  // 跳ねるか光るかは選ばせない。数が渡っていれば上下が言えるので跳ね、言えなければ光る
  const text = $derived(value ?? "");
</script>

{#if value === undefined && !children}
  <!-- 値はここに無い。差分枠だけを置く -->
{:else if value === null && !children}
  <span class={klass} {title}><span class="badge unknown">?</span></span>
{:else if motion}
  <span
    class="num {klass}"
    class:tone-up={tone === "up"}
    class:tone-down={tone === "down"}
    class:sim-value={tone === "sim"}
    {title}
    use:bump={motion}>{#if children}{@render children()}{:else}{value}{/if}</span>
{:else}
  <span
    class="num {klass}"
    class:tone-up={tone === "up"}
    class:tone-down={tone === "down"}
    class:sim-value={tone === "sim"}
    {title}
    use:changed={() => text}>{#if children}{@render children()}{:else}{value}{/if}</span>
{/if}
{#if delta && motion}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <span
    class={deltaClass}
    title={onDelta ? "変わったところを開く" : undefined}
    onclick={onDelta}
    use:deltaAction={{ get: motion, unit: delta.unit, digits: delta.digits }}></span>
{/if}

<style>
  /* 呼ぶ側が渡す `class`(その画面の見た目)より必ず強くする。
     要素名を足して詳細度を 1 段上げておかないと、行側の `.readrow .v { color }` に負ける。
     クラス名を `tone-*` にしてあるのは、差分枠(`use:delta` が付ける `num` + `up` / `down`)と
     ぶつかるため —— 素の `.up` / `.down` だと、この scoped 指定のほうが強くて
     差分枠の色(§10 の増減)を横取りしていた(実機 2026-09-17) */
  span.num.tone-up { color: var(--good); }
  span.num.tone-down { color: var(--danger); }
  span.num.sim-value { color: var(--sim-fg); }
</style>
