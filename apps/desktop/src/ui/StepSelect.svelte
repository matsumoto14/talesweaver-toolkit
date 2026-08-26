<script lang="ts">
  // 段階選択(design-system §07 形態 2)。**選択肢が有限で、順序があるとき**に使う。
  // 段階そのものを見せるのが要点で、「3」という数字を打たせない・畳んで隠さない。
  // ドロップダウン(Select.svelte)は 5 形態のどれでもないので、段階として並べられる
  // ものはこちらを使う。並べると横に溢れる長さ(Lv 0〜100 のような)は段階ではないので
  // Select のまま置き、§14 に保留として挙げてある。
  //
  // 押した瞬間に結果が動く(「適用」を挟まない)。押した段は同じ位置に残る(§09 規則 1)。
  //
  // 見た目は app.css の `.seg`(§08 をそのまま写した共通部品)。ここには振る舞いだけを置く。
  interface Option {
    value: string;
    label: string;
  }
  interface Props {
    label?: string;
    value: string;
    options: Option[];
    disabled?: boolean;
    /** 幅いっぱいを段の数で割る(§08 `.seg.full`)。段が折り返すのを防ぐ */
    full?: boolean;
  }
  let { label, value = $bindable(), options, disabled = false, full = false }: Props = $props();
</script>

<div class="step-select">
  {#if label}<span class="label">{label}</span>{/if}
  <div class="seg" class:full role="radiogroup" aria-label={label ?? ""}>
    {#each options as o (o.value)}
      <button
        type="button"
        class="step"
        class:on={o.value === value}
        role="radio"
        aria-checked={o.value === value}
        {disabled}
        onclick={() => (value = o.value)}
      >{o.label}</button>
    {/each}
  </div>
</div>

<style>
  .step-select { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
</style>
