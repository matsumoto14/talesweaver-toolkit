<script lang="ts">
  // 文字欄(design-system §07「文字欄」)。アプリ全体の文字入力はこれ 1 つ。
  // 素の input[type=text] / textarea / placeholder は使わない。
  //
  // 形は 3 つ。どれも高さ 28px / 角丸 r-panel で全ページ同一:
  // - 呼び名(`auto` あり): 既定は読み取り面に自動値(キャラ名など)を埋め、押すと入力面。
  //   §08 フィールドと同じ「表示が既定・編集は例外」。placeholder に頼らない
  // - 検索(`search`): 左に ⌕、右に件数を常設(「範囲は入力欄が知っている」の文字版)
  // - 自由記述(`multi`): 同じ枠を縦に伸ばすだけ。右下に文字数
  // それ以外(ラベル欄)は入力面が既定で、右端に文字数を出す。
  import { bump } from "./motion.svelte";
  import { fmtInt } from "../format";

  interface Props {
    /** 何の欄か。見えるラベルは呼び出し側が置くので、ここでは aria-label にだけ使う */
    label: string;
    value: string;
    /** 値が空のときに埋める自動値。指定すると「呼び名」の形(既定は読み取り面) */
    auto?: string;
    /** 自動値を使っていることの注記(例: キャラ名を使用) */
    autoNote?: string;
    /** 検索の形。⌕ と件数を常設する */
    search?: boolean;
    /** 検索の件数(search のとき必須) */
    count?: number;
    /** 自由記述の形(textarea) */
    multi?: boolean;
    rows?: number;
    /** 文字数の上限。指定すると右端に「n/上限」を出す */
    max?: number;
    disabled?: boolean;
    /** blur / Enter で確定した値を受け取る(1 文字ごとに保存したくない欄で使う) */
    onCommit?: (value: string) => void;
    /** Enter で実行する操作(作成ボタンの代わり) */
    onEnter?: () => void;
  }
  let {
    label, value = $bindable(), auto, autoNote = "自動値を使用", search = false, count, multi = false, rows = 5, max,
    disabled = false, onCommit, onEnter,
  }: Props = $props();

  /** 呼び名の形は読み取りが既定。押したときだけ入力面になる */
  let editing = $state(false);
  const named = $derived(auto !== undefined);
  const shown = $derived(value === "" && auto !== undefined ? auto : value);

  function commit() {
    // 自動値のまま離れたら空に戻す。実値にしてしまうと「キャラ名を使用」の注記が消え、
    // 上書きしたのか自動のままなのかが読めなくなる
    if (auto !== undefined && value.trim() === auto) value = "";
    onCommit?.(value);
  }
  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !multi) {
      commit();
      onEnter?.();
      if (named) editing = false;
    } else if (e.key === "Escape" && named) {
      editing = false;
    }
  }
  function blur() {
    commit();
    if (named) editing = false;
  }
  /** 入力面に入ったら自動値を実値として埋める。空のまま編集に入ると placeholder 頼みになる */
  function startEdit() {
    if (value === "" && auto !== undefined) value = auto;
    editing = true;
  }
  const focusOnMount = (node: HTMLInputElement) => {
    // preventScroll: 押した場所は既に見えている。動かすと視点が飛ぶ(§09「押した場所は動かない」)
    node.focus({ preventScroll: true });
    node.select();
  };
</script>

{#if multi}
  <label class="tfield multi" class:disabled>
    <textarea bind:value {rows} maxlength={max} {disabled} aria-label={label} onblur={blur}></textarea>
    {#if max !== undefined}<span class="cnt num" use:bump={() => value.length}>{value.length}/{max}</span>{/if}
  </label>
{:else if named && !editing}
  <button type="button" class="tfield read" {disabled} aria-label="{label} を編集" onclick={startEdit}>
    <span class="text" title={shown}>{shown}</span>
    {#if value === ""}<span class="auto">{autoNote}</span>{/if}
  </button>
{:else}
  <label class="tfield" class:search class:disabled>
    {#if search}<span class="lens" aria-hidden="true">⌕</span>{/if}
    {#if named}
      <input type="text" bind:value maxlength={max} {disabled} aria-label={label} onblur={blur} onkeydown={keydown} {@attach focusOnMount} />
    {:else}
      <input type="text" bind:value maxlength={max} {disabled} aria-label={label} onblur={blur} onkeydown={keydown} />
    {/if}
    {#if search && count !== undefined}
      <span class="cnt num" use:bump={() => count ?? 0}>{fmtInt(count)} 件</span>
    {:else if max !== undefined}
      <span class="cnt num" use:bump={() => value.length}>{value.length}/{max}</span>
    {/if}
  </label>
{/if}

<style>
  .tfield {
    display: flex; align-items: center; gap: 6px; box-sizing: border-box;
    /* 器に合わせて伸びる。width: 100% にすると横並びの隣(作成ボタン・バッジ)を潰す */
    flex: 1 1 auto; min-width: 0; height: 28px; padding: 0 9px; margin: 0;
    border-radius: var(--r-panel); border: 1px solid var(--border); background: var(--bg-field);
    font: inherit; font-size: 12px; color: var(--fg); text-align: left;
    transition: border-color 0.15s ease, background 0.15s ease;
  }
  .tfield:focus-within { border-color: var(--accent); }
  .tfield.disabled { opacity: 0.5; }
  input, textarea {
    flex: 1; min-width: 0; margin: 0; padding: 0; border: 0; background: none;
    font: inherit; color: inherit; outline: none;
  }
  /* 高さは rows で決める。つまみを出すと右下の文字数と重なる */
  textarea { resize: none; line-height: 1.6; }
  /* 読み取り面: 枠を消して周りの読み取り値と同じ見た目。触れると枠が戻り、押せることが分かる */
  .read { background: transparent; border-color: transparent; color: var(--fg-sub); cursor: text; }
  .read:hover:not(:disabled) { border-color: var(--border); background: var(--bg-field); }
  .read:focus-visible { outline: 1px solid var(--accent); outline-offset: 2px; }
  .read:disabled { cursor: default; opacity: 0.5; }
  .text { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .auto { margin-left: auto; flex: none; font-size: 9px; color: var(--fg-dim); white-space: nowrap; }
  .lens { flex: none; color: var(--fg-dim); font-size: 12px; }
  /* 件数・文字数は右端の固定幅。桁が増えても欄の幅が変わらない */
  .cnt { margin-left: auto; flex: none; min-width: 34px; text-align: right; font-size: 8.5px; color: var(--fg-dim); font-variant-numeric: tabular-nums; white-space: nowrap; }
  /* 自由記述は同じ枠を縦に伸ばすだけ。文字数は右下 — textarea の下の行に置く。
     枠の中に重ねると、行数が rows を超えたときスクロールバーの下に潜る */
  .multi { height: auto; flex-direction: column; align-items: stretch; gap: 2px; padding: 6px 9px 4px; }
  .multi .cnt { margin-left: 0; align-self: flex-end; }
</style>
