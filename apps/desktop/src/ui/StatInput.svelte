<script lang="ts">
  // 上限のある数値の唯一の入力部品(§07 形態 4)。
  //
  //   [ラベル]  [ 値 /上限  ← セル底に進捗バー ]  [MAX]  [注記]
  //
  // 見た目は app.css の `.stepper`(§07 実演をそのまま写した共通部品)。
  // ここには**振る舞いだけ**を置く — 見た目を部品の中に持つと、規格を写し直すたびに
  // ずれる。実際、最初の実装は面の色も動きも規格と違っていた。
  //
  // ＋ / − は「1 押しに意味がある」欄だけに置く(stepper)。ステータスの刻みは 1 で
  // 上限が 310〜3,000 あり、1 ずつ押す操作にならないため。どの欄もセルを押せば手入力に入る。
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
     * よく使う値を 1 押しで入れるボタン(§12「MAX を 1 タップで置く」と同じ思想)。
     * 上限まで盛らないのが普通の値(神鳥の聖物の +200 = 20 段階など)は、
     * MAX だけ置いても押されない。実際に多い値を隣に出す。
     */
    presets?: { value: number; label: string }[];
    /**
     * 上限に対する進捗を見せるか(§07 形態 4)。**上限まで盛るもの**だけ true —
     * エンチャント・ランダムOP・能力値のように「どこまで届いたか」に意味がある値。
     *
     * false は §12 の形態 1(自動)。装備の基本値のように、上限が入力ミスを防ぐための
     * 一律値でしかないものは、進捗を出すと「1,000 まで盛れる」と読めてしまう。
     */
    gauge?: boolean;
    /**
     * ＋ / − を置くか(§07 形態 4「刻みが決まっているとき。＋ / − と MAX で動かす」)。
     * **1 押しに意味がある**もの、つまり段階を数で持っている値だけ true —
     * 神鳥の聖物(1 段階 = +10)やルーンスキル Lv。能力値の 1 は誤差なので置かない。
     */
    stepper?: boolean;
  }
  let {
    label, value = $bindable(), min, max, step = 1, format, presets = [], gauge = true, stepper = false,
  }: Props = $props();

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

  /** ＋ / − で 1 刻み動かす。編集中でも読取のままでも同じように効く */
  function nudge(dir: 1 | -1) {
    const next = clamp(value + dir * step);
    if (next === value) return;
    value = next;
    lastSyncedValue = next;
    text = String(next);
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
  class="stepper"
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
  {#if stepper}
    <button type="button" class="step" onclick={() => nudge(-1)} disabled={value <= min} aria-label="{label} を 1 減らす">−</button>
  {/if}
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
  {#if stepper}
    <button type="button" class="step" onclick={() => nudge(1)} disabled={value >= max} aria-label="{label} を 1 増やす">＋</button>
  {/if}
  <!-- よく使う値。MAX と同じく常設する -->
  {#each presets as p (p.value)}
    <button
      type="button"
      class="preset"
      class:on={value === p.value}
      onclick={() => { value = Math.min(max, Math.max(min, p.value)); text = String(value); }}
    >{p.label}</button>
  {/each}
  <!-- MAX は**常設**。押して編集に入ってからでは 2 タップになる(§12「MAX を 1 タップで置く」) -->
  {#if showCap}
    <button type="button" class="max" onclick={setMax} disabled={full}>MAX</button>
  {/if}
  <!-- format を渡された欄は**値が 0 でも場所を確保する**。出たり消えたりすると、
       その行だけ入力欄の幅が変わる(§09 規則 4「あとから幅が変わらない」) -->
  {#if format}<span class="hint dim fixed">{hint ?? ""}</span>{/if}
</div>
