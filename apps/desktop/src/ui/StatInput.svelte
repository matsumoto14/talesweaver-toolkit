<script lang="ts">
  // 数値入力の唯一の部品(CLAUDE.md UX 方針)。ラベル | 数値欄 | スライダー | MAX の1行。
  // 数値欄のテキスト確定ロジックは旧 NumberField.svelte を踏襲:
  // text($state) と value(bindable) を分離し、oninput で確定できる間だけ value を書き換え、
  // onblur で最終確定・範囲内にクランプする。外部から value が変わったときだけ $effect で
  // text を同期する(lastSyncedValue で比較。Number("") === 0 になる罠を避けるため
  // value との比較ではなく専用変数で判定する)。
  // スライダーは text ではなく value(確定済みの数値)に束縛する。text に束縛すると、
  // 数値欄を空欄にした瞬間に range の value="" 扱いとなり、スライダーのつまみが
  // 中央付近へ飛んで見える不具合があった。
  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    format?: (value: number) => string;
  }
  let { label, value = $bindable(), min, max, step = 1, format }: Props = $props();

  let text = $state(String(value));
  let lastSyncedValue = value;

  $effect(() => {
    if (value !== lastSyncedValue) {
      lastSyncedValue = value;
      text = String(value);
    }
  });

  function clamp(n: number): number {
    if (n < min) return min;
    if (n > max) return max;
    return n;
  }

  function handleInput(e: Event) {
    text = (e.currentTarget as HTMLInputElement).value;
    const n = Number(text);
    if (text.trim() !== "" && Number.isFinite(n)) {
      const normalized = clamp(Number.isInteger(step) ? Math.round(n) : n);
      value = normalized;
      lastSyncedValue = normalized;
    }
  }

  function handleBlur() {
    const n = Number(text);
    // 空欄・無効値は直前の確定値(= 現在の value。handleInput は無効な text のときに
    // value を書き換えないため、常に「最後に確定した値」を保っている)にフォールバックする。
    // min にフォールバックすると、min が負の項目(例: 調整「加算」の -999)で
    // 空欄化しただけの操作が -999 になってしまう。範囲外は端に寄せる。
    const raw = text.trim() === "" || !Number.isFinite(n) ? value : n;
    const v = clamp(Number.isInteger(step) ? Math.round(raw) : raw);
    // この部品が最後に確定した値(lastSyncedValue)と同じなら setter を呼ばない。
    // `value` と比較すると、アンマウント時の blur で親が既にリセット済みの state を読んでしまい
    // 古い text を新しい state に書き戻す(キャラ切替で一時固定が次のキャラに漏れる)。
    if (v !== lastSyncedValue) {
      value = v;
    }
    lastSyncedValue = v;
    text = String(v);
  }

  function handleSlider(e: Event) {
    const n = Number((e.currentTarget as HTMLInputElement).value);
    value = n;
    lastSyncedValue = n;
    text = String(n);
  }

  function setMax() {
    value = max;
    lastSyncedValue = max;
    text = String(max);
  }

  const hint = $derived(format ? format(value) : null);
</script>

<div class="stat-input">
  {#if label}<span class="label">{label}</span>{/if}
  <input
    class="num-field"
    type="number"
    value={text}
    oninput={handleInput}
    onblur={handleBlur}
    {min}
    {max}
    {step}
    aria-label={label}
  />
  <input
    class="slider"
    type="range"
    {min}
    {max}
    {step}
    value={value}
    oninput={handleSlider}
    aria-label="{label} スライダー"
  />
  <button type="button" class="max-btn" onclick={setMax} disabled={value >= max}>MAX</button>
  {#if hint}<span class="hint dim">{hint}</span>{/if}
</div>

<style>
  .stat-input { display: flex; align-items: center; gap: 8px; min-width: 0; flex-wrap: wrap; }
  .label { font-size: 12px; color: var(--fg-muted); min-width: 44px; flex-shrink: 0; white-space: nowrap; }
  .num-field {
    width: 64px; flex-shrink: 0; padding: 5px 7px;
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  .num-field:focus { outline: none; border-color: var(--accent); }
  .slider {
    flex-grow: 1; flex-shrink: 1; min-width: 24px; height: 2px; appearance: none;
    background: var(--border-strong); accent-color: var(--accent);
  }
  .slider::-webkit-slider-thumb {
    appearance: none; width: 10px; height: 14px; background: var(--accent); border: 0; cursor: pointer;
  }
  .max-btn {
    flex-shrink: 0; padding: 5px 8px;
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg-muted); font-size: 10px;
  }
  .max-btn:hover:not(:disabled) { color: var(--fg); border-color: var(--border-strong); }
  .hint { flex-shrink: 0; font-size: 11px; white-space: nowrap; }
</style>
