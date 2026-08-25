<script lang="ts">
  // 上限のある数値の唯一の入力部品(§07 形態 4)。
  //
  //   [ラベル]  [ 値 /上限  ← セル底に進捗バー ]  [MAX]  [注記]
  //
  // v4(TW Toolkit Prototype v4.dc.html 570-587)の実物は `− [値/上限] ＋ MAX` だが、
  // ＋ / − は置かない。ステータスの刻みは 1 で上限が 310〜3,000 あり、1 ずつ押して
  // 動かす操作が実用にならない。代わりに **セルを押すと手入力**に入る。
  // 形態 4 のうち「上限に対していまどのあたりか」を見せる部分(値と上限の同居・
  // 進捗バー・MAX 1 タップ・上限到達で面が変わる)を採る。
  //
  // 上限到達は金(§03 予約色「上限到達(「満」)」)。§07 実演の「紫になる」は
  // v4 のエンチャント欄がラベンダー系統の画面にあるための記述で、色の規則ではない。
  //
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
     * 上限に対する進捗を見せるか(§07 形態 4)。**上限まで盛るもの**だけ true —
     * エンチャント・ランダムOP・能力値のように「どこまで届いたか」に意味がある値。
     *
     * false は §12 の形態 1(自動)。装備の基本値のように、上限が入力ミスを防ぐための
     * 一律値でしかないものは、進捗を出すと「1,000 まで盛れる」と読めてしまう。
     */
    gauge?: boolean;
  }
  let { label, value = $bindable(), min, max, step = 1, format, gauge = true }: Props = $props();

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
    // 上限の情報が無いとき(max <= min)は上限で縛らない。縛ると手入力が min に落ちるだけになる
    if (max > min && n > max) return max;
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
    // min にフォールバックすると、min が負の項目(例: 調整「加算」の -3,000)で
    // 空欄化しただけの操作が -3,000 になってしまう。範囲外は端に寄せる。
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
    if (value === max) return;
    value = max;
    lastSyncedValue = max;
    text = String(max);
  }

  const hint = $derived(format ? format(value) : null);
  const full = $derived(value >= max);
  /**
   * 上限を語る部分(進捗バー・/上限・MAX)を出すか。
   * `max <= min` は動かせる幅が無い = 上限の情報が無いということなので、上限を語らない。
   * ただし**手入力は残す** — gamedata が未収録・誤っているときの逃げ道が無くなる。
   */
  const showCap = $derived(gauge && max > min);
  /**
   * 上限に対する進捗。負の範囲(調整の加算 -3,000〜3,000)は「上限に対してどこまで」が
   * 成り立たないのでバーを出さない
   */
  const pct = $derived(
    min < 0 || max <= min
      ? null
      : value <= min
        ? 0
        : Math.min(100, Math.max(3, ((value - min) / (max - min)) * 100)),
  );

  /** 編集中か。既定は読み取り表示(§08 フィールド) */
  let editing = $state(false);
</script>

<!-- §08「フィールド — 表示が既定・編集は例外」。初期値は常に埋まっているので、
     ふだんは読み取り表示。入力欄は**自動値を上書きする例外操作**なので、押して初めて出す。
     編集に入っても「適用」は挟まない — 触った瞬間に結果が動く(§07)。 -->
<div
  class="stat-input"
  class:full
  onfocusout={(e) => {
    // 編集の中で入力欄 → MAX と移る間は閉じない。relatedTarget は再描画のタイミングで
    // null になることがあるので、次のフレームで「いまフォーカスがこの部品の外にあるか」を見る
    if (!editing) return;
    const root = e.currentTarget as HTMLElement;
    setTimeout(() => {
      if (!root.contains(document.activeElement)) editing = false;
    }, 0);
  }}
>
  {#if label}<span class="label">{label}</span>{/if}
  <!-- 値と上限は**同じセルに同居**する(§07「値・上限・進捗・MAX がひとつのセルに同居」)。
       上限を行の右端に飛ばすと、値の隣に無いので「何に対しての上限か」が読めない。
       読取(button)と編集(input)でセルの寸法は同じ。押しても値が動かない(§09 規則 1) -->
  <div class="cell" class:editing class:bare={!showCap}>
    {#if showCap && pct !== null}<span class="fill" style:width="{pct}%"></span>{/if}
    {#if editing}
      <input
        class="num val"
        type="number"
        value={text}
        oninput={handleInput}
        onblur={handleBlur}
        onkeydown={(e) => { if (e.key === "Escape" || e.key === "Enter") editing = false; }}
        {min}
        max={max > min ? max : undefined}
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
        class="num val read"
        aria-label="{label} を編集"
        onclick={() => (editing = true)}
      >{value.toLocaleString("ja-JP")}</button>
    {/if}
    {#if showCap}<span class="cap num">/{max.toLocaleString("ja-JP")}</span>{/if}
  </div>
  <!-- MAX は**常設**。押して編集に入ってからでは 2 タップになる(§12「MAX を 1 タップで置く」) -->
  {#if showCap}
    <button type="button" class="max-btn" onclick={setMax} disabled={full}>MAX</button>
  {/if}
  {#if hint}<span class="hint dim">{hint}</span>{/if}
</div>

<style>
  .stat-input { display: flex; align-items: center; gap: 6px; min-width: 0; flex-wrap: wrap; }
  .label { font-size: 12px; color: var(--fg-muted); min-width: 44px; flex-shrink: 0; white-space: nowrap; }
  /* 値・上限・進捗が入るセル。読取でも編集でも同じ寸法 */
  .cell {
    position: relative; overflow: hidden; flex-shrink: 0;
    display: flex; align-items: baseline; justify-content: flex-end; gap: 2px;
    width: 104px; padding: 4px 6px 5px; border-radius: var(--r-panel);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
  }
  /* 白 = 編集できる面(§01) */
  .cell.editing { background: var(--bg-field); border-color: var(--accent); }
  .cell:not(.editing):hover { border-color: var(--accent); }
  /* 上限を語らない形(形態 1)。/上限 と MAX が無いぶん詰める */
  .cell.bare { width: 74px; }
  /* 上限に対していまどのあたりか。セル底の 2px(v4) */
  .fill {
    position: absolute; left: 0; bottom: 0; height: 2px;
    background: linear-gradient(90deg, var(--border-strong), var(--accent));
    pointer-events: none;
  }
  .val {
    min-width: 0; flex: 1 1 auto; text-align: right; font-size: 12.5px; font-weight: 700;
    font-variant-numeric: tabular-nums; color: var(--fg);
    background: none; border: none; padding: 0;
  }
  .val:focus { outline: none; }
  /* スピナー(▲▼)は押した瞬間に幅を食って値を横にずらす(§09 規則 1)。
     ＋ / − を置かないので、刻みで動かす手段としても要らない */
  .val::-webkit-inner-spin-button, .val::-webkit-outer-spin-button { appearance: none; margin: 0; }
  input.val { appearance: textfield; }
  .val.read:focus-visible { outline: 1px solid var(--accent); outline-offset: 2px; }
  /* 上限は値のすぐ右、小さく。「1 〜 310」のような範囲表記にはしない —
     値が別にあるぶん、範囲で書くと何の数字か読めない */
  .cap { flex: none; font-size: 8.5px; color: var(--fg-faint); font-variant-numeric: tabular-nums; }
  /* 上限に届いたら面の色が変わる(§07 / §12)。「満」は文字ではなく面で伝える */
  .stat-input.full .cell { background: var(--state-edge-bg); border-color: var(--gold); }
  .stat-input.full .fill { background: var(--gold); }
  .stat-input.full .cap { color: var(--state-edge-fg); }
  .max-btn {
    flex-shrink: 0; width: 30px; padding: 4px 0; text-align: center;
    border-radius: var(--r-chip); background: var(--bg-field);
    border: 1px solid var(--border); color: var(--fg-muted);
    font-size: 8px; font-weight: 700;
  }
  .max-btn:hover:not(:disabled) { color: var(--fg); border-color: var(--border-strong); }
  /* 満のときは沈めずに反転させる。「もう上限だ」を MAX 自身が言う(v4) */
  .max-btn:disabled { background: var(--state-edge-bg); border-color: var(--gold); color: var(--state-edge-fg); }
  .hint { flex-shrink: 0; font-size: 11px; white-space: nowrap; }
</style>
