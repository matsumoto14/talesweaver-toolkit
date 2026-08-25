<script lang="ts">
  // 数値入力の唯一の部品(CLAUDE.md UX 方針)。ラベル | 値 | MAX | 範囲 の1行。
  // 値は読取(button)と編集(input)で同じ位置・同じ幅(§08 / §09 規則 1)。
  // 数値欄のテキスト確定ロジックは旧 NumberField.svelte を踏襲:
  // text($state) と value(bindable) を分離し、oninput で確定できる間だけ value を書き換え、
  // onblur で最終確定・範囲内にクランプする。外部から value が変わったときだけ $effect で
  // text を同期する(lastSyncedValue で比較。Number("") === 0 になる罠を避けるため
  // value との比較ではなく専用変数で判定する)。
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
  class:full={value >= max}
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
  <!-- 値は読取でも編集でも**同じ位置・同じ幅**。押しても値が動かない(§09 規則 1)。
       入れ替わるのは右側だけ — 「編集」→ スライダー + MAX(§08 のフィールドの図) -->
  {#if editing}
    <input
      class="num value-box"
      type="number"
      value={text}
      oninput={handleInput}
      onblur={handleBlur}
      onkeydown={(e) => { if (e.key === "Escape" || e.key === "Enter") editing = false; }}
      {min}
      {max}
      {step}
      aria-label={label}
      {@attach (node) => {
        // preventScroll: focus の既定はスクロールして要素を視界に入れる。押した場所は
        // 既に見えているので、動かすと視点がリセットされる(§09「押した場所は動かない」)
        node.focus({ preventScroll: true });
        node.select();
      }}
    />
  {:else}
    <button
      type="button"
      class="num value-box read"
      aria-label="{label} を編集"
      onclick={() => (editing = true)}
    >{value.toLocaleString("ja-JP")}</button>
  {/if}
  <!-- 右側。幅が違うので、この先の範囲表示は右端に固定して動かさない -->
  <span class="side">
    {#if editing}
      <button type="button" class="max-btn" onclick={setMax} disabled={value >= max}>MAX</button>
    {:else}
      <button type="button" class="edit" onclick={() => (editing = true)} tabindex="-1">編集</button>
    {/if}
  </span>
  {#if capGauge}
    <!-- 値は左の欄に出ているので、ここは**上限だけ**を言う。
         「1 〜 310」のような範囲表記だと、値が別にあるぶん何の数字か読めない -->
    <span class="cap num" class:full={value >= max}>上限 {max.toLocaleString("ja-JP")}</span>
    <!-- 「満」の枠は常に確保する。出たときに行がずれない(§09 規則 4 / §11) -->
    <span class="cap-badge" class:on={value >= max}>{value >= max ? "満" : ""}</span>
  {/if}
  {#if hint}<span class="hint dim">{hint}</span>{/if}
</div>

<style>
  .stat-input { display: flex; align-items: center; gap: 8px; min-width: 0; flex-wrap: wrap; }
  /* 上限に届いたら面の色が変わる(§07 / §12)。「満」は文字ではなく面でも伝える */
  .stat-input.full .value-box { background: var(--state-edge-bg); border-color: var(--gold); }
  .edit { flex-shrink: 0; font-size: 9px; color: var(--fg-dim); white-space: nowrap; }
  .edit:hover { color: var(--accent); text-decoration: underline; }
  .label { font-size: 12px; color: var(--fg-muted); min-width: 44px; flex-shrink: 0; white-space: nowrap; }
  /* 値の欄。読取(button)と編集(input)で同じ寸法にして、押しても値が動かないようにする */
  .value-box {
    width: 74px; flex-shrink: 0; padding: 5px 7px; border-radius: var(--r-panel);
    text-align: right; font-variant-numeric: tabular-nums; color: var(--fg);
    border: 1px solid var(--border);
  }
  /* 読取はインセット面、編集は白い面(§01 白 = 編集できる面) */
  .value-box.read { background: var(--surface-inset); border-color: var(--border-soft); }
  .value-box.read:hover { border-color: var(--accent); }
  .value-box.read:focus-visible { outline: 1px solid var(--accent); outline-offset: 2px; }
  input.value-box { background: var(--bg-field); }
  input.value-box:focus { outline: none; border-color: var(--accent); }
  /* 読取(「編集」)と編集(MAX)で必要幅が変わると、表の列幅が動いて値が横にずれる。
     広いほう(MAX)に合わせて最低幅を決めておく(§09 規則 4) */
  .side { flex: 1; min-width: 44px; display: flex; align-items: center; gap: 8px; }
  .max-btn {
    flex-shrink: 0; padding: 5px 8px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg-muted); font-size: 10px;
  }
  .max-btn:hover:not(:disabled) { color: var(--fg); border-color: var(--border-strong); }
  .hint { flex-shrink: 0; font-size: 11px; white-space: nowrap; }
  .cap { flex-shrink: 0; margin-left: auto; font-size: 11px; color: var(--fg-muted); white-space: nowrap; }
  .cap.full { color: var(--fg); font-weight: 700; }
  .cap-badge {
    flex-shrink: 0; min-width: 16px; text-align: center;
    font-size: 8.5px; font-weight: 700; color: transparent;
  }
  .cap-badge.on { color: var(--state-edge-fg); }
</style>
