<script lang="ts">
  // 数値をひとつ入れる欄。§07 の 5 形態のうち **1(自動)・4(ステッパー)・5(自由入力)**
  // をこれ 1 つで持つ。上の 2 形態(段階選択・チップ)は ui/Choose.svelte。
  //
  //   [ 値 /上限  ← セル底に進捗バー ]  ＋ −  MAX  [注記]
  //
  // **形態は呼ぶ側が選ばない。渡したものが決める**:
  //
  //   max を渡した       → 形態 4。上限を値の隣に常設し、バー・＋ − ・MAX が付く
  //   max が無い         → 形態 5。青枠 + 破線チップで「例外として許した入力」だと見せる
  //   autoNote を渡した  → 形態 1。出どころがある値なので、ふだんは読み取りの文字に見せる
  //
  // §07 は形態 4 を「連続値だが上限があり、刻みが決まっているとき。＋ / − と MAX で動かす。
  // 上限は値の隣に常設し、届いたらバー・枠・MAX が金になる」と **1 つの形**として書いている。
  // バーだけ・＋ − だけを外す選択肢は規格に無いので、部品にも置かない(段階 8)。
  // 上限が無い値は「上限を語る部分を消した形態 4」ではなく、形態 5(例外)である。
  //
  // 見た目は app.css の `.numfield`(§07 実演をそのまま写した共通部品)。
  // ここには**振る舞いだけ**を置く — 見た目を部品の中に持つと、規格を写し直すたびにずれる。
  //
  // 数値欄のテキスト確定ロジック:
  // text($state) と value(bindable) を分離し、oninput で確定できる間だけ value を書き換え、
  // onblur で最終確定・範囲内にクランプする。外部から value が変わったときだけ $effect で
  // text を同期する(lastSyncedValue で比較。Number("") === 0 になる罠を避けるため
  // value との比較ではなく専用変数で判定する)。
  import { bump } from "./motion.svelte";
  import { fmtInt } from "../format";

  interface Props {
    /** 何の欄か。読み上げ用(aria-label)。見えるラベルは呼ぶ側の行が持つ */
    label: string;
    value: number;
    min?: number;
    /**
     * この値の上限。**渡すと形態 4**(バー・/上限・＋ − ・MAX)になり、外から上限が変わったら
     * 現在値を直ちにクランプする。上限が無い値・上限が未収録の値には渡さない(形態 5 になる)。
     */
    max?: number;
    step?: number;
    format?: (value: number) => string;
    /**
     * よく使う値を 1 押しで入れるボタン(§12「MAX を 1 タップで置く」と同じ思想)。
     * 上限まで盛らないのが普通の値(神鳥の聖物の +200 = 20 段階など)は、
     * MAX だけ置いても押されない。実際に多い値を隣に出す。
     */
    presets?: { value: number; label: string }[];
    /** 現在値へ加算する定型値。エンチャントカード(+12/+14/+17/+20)のような反復入力に使う。 */
    increments?: number[];
    /**
     * この値の出どころ(「カタログの値」など)。**渡すと形態 1** —
     * 出どころがある値は入力欄に見せず、読み取りの文字にする。押すと同じ寸法の入力面になり、
     * 自動値を上書きできる(§07 形態 1「MR による個体差があるので上書きはできる」)。
     */
    autoNote?: string;
    /**
     * 形態 5 の理由(§07「ここまで降りたら理由を書く」)。上限が無い欄には必ず理由が出る —
     * 渡さなかったときは既定の文言になるので、**理由の無い自由入力は作れない**。
     */
    reason?: string;
    /**
     * 入る値の最大桁数。上限のある欄は上限の桁でセル幅が決まるが、上限を持たない
     * 形態 5 は桁の情報が無いので、ここで与える。省略すると 74px の既定幅で、
     * 7 桁以上が右端で切れる(実測ダメージで起きた。§09 規則 4「あとから幅が変わらない」)。
     */
    digits?: number;
  }
  let {
    label, value = $bindable(), min = 0, max, step = 1, format, presets = [], increments = [], autoNote, reason, digits,
  }: Props = $props();

  /** 形態 4 か。上限が動かせる幅を持っているときだけ「上限のある値」と言える */
  const capped = $derived(max !== undefined && max > min);
  /** 形態 1 か。出どころのある値はふだん読み取りの文字 */
  const isAuto = $derived(autoNote !== undefined);
  /**
   * 形態 5 の理由。出どころがある値(形態 1)には出さない — その値にとっては
   * 出どころそのものが理由なので、青枠と破線チップを重ねると例外が二重になる。
   */
  const why = $derived(capped || isAuto ? null : (reason ?? "上限なし · 手入力"));
  /** 桁区切りのカンマを含めた文字数(セル幅の根拠) */
  const chars = $derived(digits === undefined ? null : digits + Math.floor((digits - 1) / 3));
  /** 形態 5 の 0 は「まだ入れていない」なので、0 と読ませず空表示にする(押せば 0 が選択された編集に入る) */
  const blank = $derived(why !== null && value === 0);

  let text = $state(String(value));
  let lastSyncedValue = value;

  $effect(() => {
    // 上限が外から変わったら現在値を直ちに寄せる(装備を替えて上限が下がった、など)
    if (capped && value > max!) value = Math.max(min, max!);
    if (value !== lastSyncedValue) {
      lastSyncedValue = value;
      text = String(value);
    }
  });

  function clamp(n: number): number {
    if (n < min) return min;
    if (capped && n > max!) return max!;
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

  /** ＋ / − ・MAX ・よく使う値。編集中でも読取のままでも同じように効く */
  function commit(next: number) {
    if (next === value) return;
    value = next;
    lastSyncedValue = next;
    text = String(next);
  }

  const hint = $derived(format ? format(value) : null);
  const full = $derived(capped && value >= max!);
  /**
   * 上限に対する進捗。負の範囲(調整の加算 -3,000〜3,000)は「上限に対してどこまで」が
   * 成り立たないのでバーを出さない
   */
  const pct = $derived(
    !capped || min < 0
      ? null
      : value <= min
        ? 0
        : Math.min(100, Math.max(3, ((value - min) / (max! - min)) * 100)),
  );

  /** 編集中か。既定は読み取り表示(§08 フィールド) */
  let editing = $state(false);
  /**
   * Escape / Enter で編集を閉じたら、**押した読み取り面にフォーカスを戻す**(§00 ③)。
   * 入力欄が消えるとフォーカスは body に落ち、Tab の続きが画面の先頭に戻ってしまう —
   * キーボードだけで触っている人には「押した場所」が消える。
   * 読み取りの button は閉じたあとに作り直されるので、生まれた時に戻す($state ではない —
   * 画面に出る値ではなく、次の 1 回だけ使う合図)。
   */
  let refocus = false;
</script>

<!-- §08「フィールド — 表示が既定・編集は例外」。初期値は常に埋まっているので、
     ふだんは読み取り表示。入力欄は**自動値を上書きする例外操作**なので、押して初めて出す。
     編集に入っても「適用」は挟まない — 触った瞬間に結果が動く(§07)。 -->
<div
  class="numfield"
  class:full
  class:auto={isAuto}
  class:free={why !== null}
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
  {#if capped}
    <button type="button" class="step" onclick={() => commit(clamp(value - step))} disabled={value <= min} aria-label="{label} を減らす">−</button>
  {/if}
  <!-- 値と上限は**同じセルに同居**する(§07「値・上限・進捗・MAX がひとつのセルに同居」)。
       上限を行の右端に飛ばすと、値の隣に無いので「何に対しての上限か」が読めない。
       読取(button)と編集(input)でセルの寸法は同じ。押しても値が動かない(§09 規則 1) -->
  <div class="cell" class:editing class:bare={!capped} class:sized={chars !== null} style:--chars={chars}>
    {#if pct !== null}<span class="fill" style:width="{pct}%"></span>{/if}
    {#if editing}
      <input
        class="num val"
        type="number"
        value={text}
        oninput={handleInput}
        onblur={handleBlur}
        onkeydown={(e) => {
          if (e.key !== "Escape" && e.key !== "Enter") return;
          // 既定動作を止めるのは必須。止めないと —— Enter: 閉じた直後に読み取り button へ
          // フォーカスを戻すので、この keydown の既定動作(フォーカス中のボタンを押す)が
          // その button に当たり、一瞬で編集に戻る(実機 2026-09-17)。
          // Escape: この欄が <dialog>(ui/Modal)の中にあると、欄を閉じるつもりの Escape が
          // モーダルごと閉じてしまう
          e.preventDefault();
          refocus = true;
          editing = false;
        }}
        {min}
        max={capped ? max : undefined}
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
        class:blank
        aria-label="{label} を編集"
        title={autoNote}
        use:bump={() => value}
        onclick={() => (editing = true)}
        {@attach (node) => {
          // Escape / Enter で閉じた直後だけ戻す。それ以外(初期表示・値の外部更新)では奪わない
          if (!refocus) return;
          refocus = false;
          node.focus({ preventScroll: true });
        }}
      >{blank ? "—" : fmtInt(value)}</button>
    {/if}
    {#if capped}<span class="cap num">/{fmtInt(max!)}</span>{/if}
  </div>
  <!-- 形態 5 の理由チップ(§07「ここまで降りたら理由を書く」)。値の隣に常設し、出たり消えたりしない -->
  {#if why !== null}<span class="why">{why}</span>{/if}
  {#if capped}
    <button type="button" class="step" onclick={() => commit(clamp(value + step))} disabled={value >= max!} aria-label="{label} を増やす">＋</button>
  {/if}
  <!-- よく使う値。MAX と同じく常設する -->
  {#each presets as p (p.value)}
    <button
      type="button"
      class="preset"
      class:on={value === p.value}
      onclick={() => commit(clamp(p.value))}
    >{p.label}</button>
  {/each}
  {#each increments as amount (amount)}
    <button
      type="button"
      class="increment num"
      onclick={() => commit(clamp(value + amount))}
      disabled={full}
      aria-label="{label}に{amount}加算"
    >+{amount}</button>
  {/each}
  <!-- MAX は**常設**。押して編集に入ってからでは 2 タップになる(§12「MAX を 1 タップで置く」) -->
  {#if capped}
    <button type="button" class="max" onclick={() => commit(max!)} disabled={full}>MAX</button>
  {/if}
  <!-- format を渡された欄は**値が 0 でも場所を確保する**。出たり消えたりすると、
       その行だけ入力欄の幅が変わる(§09 規則 4「あとから幅が変わらない」) -->
  {#if format}<span class="hint dim fixed">{hint ?? ""}</span>{/if}
</div>
