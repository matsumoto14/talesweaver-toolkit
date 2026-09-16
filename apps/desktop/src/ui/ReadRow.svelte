<script lang="ts">
  // 読み取り面の「ラベル + 値」1 行(design-system §08 ReadRow)。インセット面(.readrows.inset)に
  // 並べて使う。**行の形(ラベル・値の位置・固定幅)だけを持ち、値そのものは `ui/Num.svelte` に任せる** —
  // 跳ね・光り・差分枠・未収録の「?」を 2 か所に持たないため(ADR-015 段階 4)。
  import type { Snippet } from "svelte";
  import Num from "./Num.svelte";

  interface Props {
    label: string;
    /** 書式済みの値。null = 未収録 */
    value?: string | null;
    /** 値の元になる数。渡すと変化時に跳ねる。省略時は文字の変化で光る */
    motion?: () => number | null;
    /** 値の右に増減(↑12)を出す。unit は差分の単位 */
    delta?: { unit?: string; digits?: number } | null;
    /** 値の色。up = 伸びた(緑) / down = 減った(赤) / sim = 試し変更(ラベンダー) */
    tone?: "up" | "down" | "sim" | null;
    /** 値の左に添える小さな補足(素 + 強化 など) */
    sub?: Snippet;
    /** 値の右に続く注記(式・出典)。行の残り幅を使う */
    note?: Snippet;
    /** 値の代わりに置く中身(Picker・行チップ)。value より優先 */
    children?: Snippet;
  }
  let { label, value = null, motion, delta = null, tone = null, sub, note, children }: Props = $props();
</script>

<div class="readrow">
  <span class="k">{label}</span>
  {#if sub}<span class="sub num dim">{@render sub()}</span>{/if}
  {#if children}
    <span class="slot">{@render children()}</span>
  {:else}
    <Num {value} {motion} {delta} {tone} class="v" />
  {/if}
  {#if note}<span class="n dim">{@render note()}</span>{/if}
</div>

<style>
  .readrow { display: flex; align-items: baseline; gap: 8px; padding: 3px 0; min-width: 0; }
  .k { flex: none; min-width: 64px; font-size: var(--t-label); font-weight: 700; color: var(--fg-muted); white-space: nowrap; }
  .sub { flex: none; margin-left: auto; font-size: 8.5px; white-space: nowrap; }
  .sub + .slot { margin-left: 0; }
  /* 値は Num.svelte が描くので、行の側は `:global` で位置と幅だけを当てる
     (Svelte のスコープ付き CSS は子コンポーネントの中の要素に届かない) */
  .readrow :global(.v) { flex: none; margin-left: auto; min-width: 64px; text-align: right; font-size: var(--t-body); font-weight: 700; color: var(--fg); white-space: nowrap; }
  .sub + :global(.v) { margin-left: 0; }
  .slot { margin-left: auto; min-width: 0; display: flex; justify-content: flex-end; }
  /* 注記は値の右に続く。値の固定幅は保ったまま、残りの幅を注記が使う */
  .n { flex: 1 1 40%; min-width: 0; font-size: 9px; line-height: 1.5; }
  .readrow :global(.delta) { flex: none; min-width: 64px; font-size: 10px; }
</style>
