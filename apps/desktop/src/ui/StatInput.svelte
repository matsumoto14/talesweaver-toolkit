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
    /**
     * 範囲の表示(`値 / 上限` + 上限に達したら「満」)。**既定で出す** —
     * §07「範囲は入力欄が知っている。値の隣に常設し、範囲外を打てなくするのではなく
     * 範囲を見せる」。隣に別の形で範囲が出ているときだけ false にする。
     */
    capGauge?: boolean;
  }
  let { label, value = $bindable(), min, max, step = 1, format, capGauge = true }: Props = $props();

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

  /** 編集中か。既定は読み取り表示(§08 フィールド) */
  let editing = $state(false);
</script>

<!-- §08「フィールド — 表示が既定・編集は例外」。初期値は常に埋まっているので、
     ふだんは読み取り表示。入力欄は**自動値を上書きする例外操作**なので、押して初めて出す。
     編集に入っても「適用」は挟まない — 触った瞬間に結果が動く(§07)。 -->
<div
  class="stat-input"
  class:editing
  onfocusout={(e) => {
    // 編集の中で入力欄 → スライダー → MAX と移る間は閉じない。
    // relatedTarget は再描画のタイミングで null になることがあるので、次のフレームで
    // 「いまフォーカスがこの部品の外にあるか」を見る
    if (!editing) return;
    const root = e.currentTarget as HTMLElement;
    setTimeout(() => {
      if (!root.contains(document.activeElement)) editing = false;
    }, 0);
  }}
>
  {#if label}<span class="label">{label}</span>{/if}
  {#if editing}
    <input
      class="num-field"
      type="number"
      value={text}
      oninput={handleInput}
      onblur={handleBlur}
      onkeydown={(e) => { if (e.key === "Escape" || e.key === "Enter") editing = false; }}
      {min}
      {max}
      {step}
      aria-label={label}
      {@attach (node) => { node.focus(); node.select(); }}
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
  {:else}
    <button type="button" class="read" onclick={() => (editing = true)} aria-label="{label} を編集">
      <span class="num read-value">{value.toLocaleString("ja-JP")}</span>
      <span class="edit">編集</span>
    </button>
  {/if}
  {#if capGauge}
    <!-- 下限が 0 なら「値 / 上限」、0 以外(調整の ±999 など)は範囲そのものを出す -->
    <span class="cap num" class:full={value >= max}>
      {#if min === 0}{value.toLocaleString("ja-JP")} / {max.toLocaleString("ja-JP")}
      {:else}{min.toLocaleString("ja-JP")} 〜 {max.toLocaleString("ja-JP")}{/if}
    </span>
    <!-- 「満」の枠は常に確保する。出たときに行がずれない(§09 規則 4 / §11) -->
    <span class="cap-badge" class:on={value >= max}>{value >= max ? "満" : ""}</span>
  {/if}
  {#if hint}<span class="hint dim">{hint}</span>{/if}
</div>

<style>
  .stat-input { display: flex; align-items: center; gap: 8px; min-width: 0; flex-wrap: wrap; }
  /* 読み取り表示。インセット面に載せて「いまは編集していない」を面で伝える(§01) */
  .read {
    display: inline-flex; align-items: baseline; gap: 8px;
    padding: 5px 9px; border-radius: var(--r-inset);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
  }
  .read:hover { border-color: var(--accent); }
  .read:focus-visible { outline: 1px solid var(--accent); outline-offset: 2px; }
  /* 桁が増えても隣が動かない(§09 規則 4)。編集欄と同じ幅を予約しておく */
  .read-value { min-width: 50px; text-align: right; font-variant-numeric: tabular-nums; }
  .edit { font-size: 9px; color: var(--fg-dim); white-space: nowrap; }
  .read:hover .edit { color: var(--accent); }
  .label { font-size: 12px; color: var(--fg-muted); min-width: 44px; flex-shrink: 0; white-space: nowrap; }
  .num-field {
    width: 64px; flex-shrink: 0; padding: 5px 7px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  .num-field:focus { outline: none; border-color: var(--accent); }
  .slider {
    flex-grow: 1; flex-shrink: 1; min-width: 24px; height: 2px; appearance: none; border-radius: var(--r-pill);
    background: var(--border-strong); accent-color: var(--accent);
  }
  .slider::-webkit-slider-thumb {
    appearance: none; width: 10px; height: 14px; border-radius: var(--r-inset); background: var(--accent); border: 0; cursor: pointer;
  }
  .max-btn {
    flex-shrink: 0; padding: 5px 8px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg-muted); font-size: 10px;
  }
  .max-btn:hover:not(:disabled) { color: var(--fg); border-color: var(--border-strong); }
  .hint { flex-shrink: 0; font-size: 11px; white-space: nowrap; }
  .cap { flex-shrink: 0; font-size: 11px; color: var(--fg-muted); white-space: nowrap; }
  .cap.full { color: var(--fg); font-weight: 700; }
  .cap-badge {
    flex-shrink: 0; min-width: 16px; text-align: center;
    font-size: 8.5px; font-weight: 700; color: transparent;
  }
  .cap-badge.on { color: var(--state-edge-fg); }
</style>
