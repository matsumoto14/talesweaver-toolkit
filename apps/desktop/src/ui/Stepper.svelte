<script lang="ts">
  // −/+ の微調整ボタン + range スライダー。自由入力欄は設けない(CLAUDE.md UX 方針)。
  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    slider?: boolean;
  }
  let { label, value = $bindable(), min, max, slider = true }: Props = $props();

  const clamp = (n: number) => Math.min(max, Math.max(min, n));
  function step(delta: number) {
    value = clamp(value + delta);
  }
</script>

<div class="stepper">
  <span class="label">{label}</span>
  {#if slider}
    <input type="range" {min} {max} step="1" bind:value aria-label={label} />
  {/if}
  <div class="ctl">
    <button type="button" onclick={() => step(-1)} disabled={value <= min} aria-label="{label} を1減らす">−</button>
    <span class="val num">{value}</span>
    <button type="button" onclick={() => step(1)} disabled={value >= max} aria-label="{label} を1増やす">+</button>
  </div>
</div>

<style>
  .stepper { display: flex; align-items: center; gap: 10px; }
  .label { font-size: 12px; color: var(--fg-muted); width: 44px; flex-shrink: 0; }
  input[type="range"] {
    flex-grow: 1; min-width: 0; height: 2px; appearance: none;
    background: var(--border-strong); accent-color: var(--accent);
  }
  input[type="range"]::-webkit-slider-thumb {
    appearance: none; width: 10px; height: 14px; background: var(--accent); border: 0; cursor: pointer;
  }
  .ctl { display: flex; align-items: stretch; border: 1px solid var(--border); flex-shrink: 0; }
  button {
    width: 26px; display: flex; align-items: center; justify-content: center;
    background: var(--bg-field); border: 0; color: var(--fg-muted);
  }
  button:hover:not(:disabled) { color: var(--fg); }
  .val { width: 56px; text-align: center; padding: 5px 0; background: var(--bg-panel); font-weight: 500; }
</style>
