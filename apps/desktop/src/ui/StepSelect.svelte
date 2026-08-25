<script lang="ts">
  // 段階選択(design-system §07 形態 2)。**選択肢が有限で、順序があるとき**に使う。
  // 段階そのものを見せるのが要点で、「3」という数字を打たせない・畳んで隠さない。
  // ドロップダウン(Select.svelte)は 5 形態のどれでもないので、段階として並べられる
  // ものはこちらを使う。並べると横に溢れる長さ(Lv 0〜100 のような)は段階ではないので
  // Select のまま置き、§14 に保留として挙げてある。
  //
  // 押した瞬間に結果が動く(「適用」を挟まない)。押した段は同じ位置に残る(§09 規則 1)。
  interface Option {
    value: string;
    label: string;
  }
  interface Props {
    label?: string;
    value: string;
    options: Option[];
    disabled?: boolean;
  }
  let { label, value = $bindable(), options, disabled = false }: Props = $props();
</script>

<div class="step-select">
  {#if label}<span class="label">{label}</span>{/if}
  <div class="steps" role="radiogroup" aria-label={label ?? ""}>
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
  /* 見た目は design-system の .seg に合わせる — 1 つの枠の中にボタンが並び、
     間は弱い区切り線 1 本。選択中は水色のグラデ(このアプリの「選ばれている」の色) */
  .step-select { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  .steps {
    display: inline-flex; flex-wrap: wrap; align-self: flex-start; min-width: 0;
    border: 1px solid var(--border); border-radius: var(--r-panel); overflow: hidden;
  }
  .step {
    padding: 5px 12px; border: 0; border-right: 1px solid var(--border-soft);
    background: var(--bg-field); color: var(--fg-muted);
    font-size: 11px; font-weight: 500; white-space: nowrap;
    transition: background 0.15s ease;
  }
  .step:last-child { border-right: 0; }
  .step:hover:not(:disabled):not(.on) { background: var(--bg-rail); }
  .step.on {
    background: linear-gradient(180deg, #CCF7FF, #90D7FF);
    color: #123047; font-weight: 700;
  }
  .step:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .step:disabled { cursor: not-allowed; opacity: 0.5; }
</style>
