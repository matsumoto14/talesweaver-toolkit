<script lang="ts">
  /**
   * モーダル面。`<dialog>.showModal()` のトップレイヤーに載せる。
   *
   * 呼ぶ側が決めるのは「面の名前」と「中に何を出すか」だけ。Escape で閉じること・フォーカスが外へ
   * 出ないこと・閉じたら開いたボタンに戻ること・背面が押せないこと・重ね順は、
   * ブラウザ(トップレイヤー)が持つ(§00 ③ 押した場所は動かない)。
   *
   * **見出し(面の名前 + 閉じる)もこの部品が持つ**(ADR-015 段階 6)。置き換え前は `.panel-header` と
   * `.part-detail-header` の 2 流儀があり、`label` と同じ文字列を呼ぶ側が 2 回書いていた。
   * 見出しは面の作法(名前があり、閉じられる)であって画面ごとの決定ではないので部品に入れた。
   *
   * 面そのもの(`.modal-surface`)もこの部品が描く。呼ぶ側は幅だけを `class` で決める
   * (`panel` 560px / `part-detail` 980px)。中身は見出しの下の枠に入り、そこがスクロールする。
   * 背景を押しても閉じない(`<dialog>` の既定の挙動)。閉じる操作は見出しの閉じるボタンと Escape だけ。
   */
  import type { Snippet } from "svelte";

  let {
    label,
    class: klass = "",
    closeDisabled = false,
    onClose,
    children,
  }: {
    /** 面の名前。見出しに出し、読み上げ名にもなる */
    label: string;
    /** 面の幅など、その画面での見た目(`panel` / `part-detail`) */
    class?: string;
    /** 進行中で閉じさせたくない間だけ true。Escape も効かなくなる(閉じるボタンと揃える) */
    closeDisabled?: boolean;
    onClose: () => void;
    children: Snippet;
  } = $props();

  let el = $state<HTMLDialogElement | null>(null);

  // マウントされた時点で開く。呼ぶ側は `{#if open}<Modal>` のままでよい
  $effect(() => {
    el?.showModal();
  });
</script>

<dialog
  bind:this={el}
  class="modal-root"
  aria-label={label}
  oncancel={(event) => { if (closeDisabled) event.preventDefault(); }}
  onclose={onClose}
>
  <div class="modal-surface pane-in {klass}">
    <div class="modal-head">
      <b>{label}</b>
      <button type="button" class="btn modal-close" disabled={closeDisabled} onclick={() => el?.close()}
        >閉じる <span aria-hidden="true">×</span></button>
    </div>
    <div class="modal-body">{@render children()}</div>
  </div>
</dialog>

<style>
  /* 暗幕。トップレイヤーに載るので z-index は要らない */
  .modal-root {
    position: fixed; inset: 0;
    width: 100vw; height: 100vh; max-width: none; max-height: none;
    margin: 0; border: 0; padding: 3vh max(14px, 6vw);
    background: none; color: inherit;
    display: flex; justify-content: center; align-items: flex-start;
  }
  /* display を上書きしているので、開く前の 1 フレームで出てしまわないよう自分で消す */
  .modal-root:not([open]) { display: none; }
  .modal-root::backdrop { background: rgba(27, 35, 49, 0.62); }

  /* 見出しは天井に固定し、中身の枠だけがスクロールする。position: sticky は要らない
     (flex 列で、伸びるのは中身の枠だけ) */
  .modal-head {
    flex-shrink: 0; display: flex; align-items: center; gap: 12px;
    padding: 9px 12px; border-bottom: 1px solid var(--border-strong);
    background: var(--bg-rail); border-radius: var(--r-window) var(--r-window) 0 0;
  }
  .modal-head b { min-width: 0; font-size: var(--t-heading); color: var(--fg-head); }
  .modal-close { margin-left: auto; min-width: 88px; justify-content: center; border-color: var(--border-strong); background: var(--bg-field); font-weight: 700; }
  .modal-close span { font-size: 15px; line-height: 1; }
  .modal-body { min-height: 0; display: flex; flex-direction: column; }
</style>
