<script lang="ts">
  /**
   * モーダル面。`<dialog>.showModal()` のトップレイヤーに載せる。
   *
   * 呼ぶ側が決めるのは「中に何を出すか」だけ。Escape で閉じること・フォーカスが外へ
   * 出ないこと・閉じたら開いたボタンに戻ること・背面が押せないこと・重ね順は、
   * ブラウザ(トップレイヤー)が持つ(§00 ③ 押した場所は動かない)。
   *
   * この要素はオーバーレイ(暗幕)そのもの。面は中身側に置く(`.modal-surface`)。
   * 背景を押しても閉じない(`<dialog>` の既定の挙動)。閉じる操作は面内の明示ボタンだけ。
   */
  import type { Snippet } from "svelte";

  let {
    label,
    closeDisabled = false,
    onClose,
    children,
  }: {
    label: string;
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
  {@render children()}
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
</style>
