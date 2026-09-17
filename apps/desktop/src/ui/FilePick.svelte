<script lang="ts">
  // ファイルを選ぶ口。アプリ中でファイルを受け取るのはこれ 1 つ。
  //
  // §07 の 5 形態の外(値を決める操作ではなく、外から中身を持ってくる操作)。それでも
  // **生の `<input type="file">` を画面側に置かない** — 素の口は OS ごとに見た目が違い、
  // 「選択されていません」の文字が勝手に付いて行の幅を変える。的は `<Chip>` 1 つで、
  // 入力そのものは隠して押したときに開く(置き換え前の 2 箇所はどちらもこの形を手で書いていた)。
  //
  // 選んだあとは値を持たない(同じファイルをもう一度選べるように毎回 value を空にする)。
  import type { Snippet } from "svelte";
  import Chip from "./Chip.svelte";

  interface Props {
    /** 受け取る種類(input の accept そのまま) */
    accept: string;
    disabled?: boolean;
    /** 的に出す文字 */
    children: Snippet;
    /** 的の見た目(Chip と同じ passthrough) */
    class?: string;
    onPick: (file: File) => void;
  }
  let { accept, disabled = false, children, class: klass = "", onPick }: Props = $props();

  let input: HTMLInputElement | undefined = $state();
</script>

<Chip class={klass} {disabled} onclick={() => input?.click()}>{@render children()}</Chip>
<input
  bind:this={input}
  type="file"
  {accept}
  {disabled}
  onchange={(e) => {
    const el = e.currentTarget as HTMLInputElement;
    const file = el.files?.[0];
    // 同じファイルを選び直しても change が来るように、読んだら口を空にする
    el.value = "";
    if (file) onPick(file);
  }}
/>

<style>
  /* 口は隠すが display: none にはしない — フォーカスの当たる要素が消えると、
     キーボードで進んだときに的(Chip)から次へ飛ぶ順番が環境で変わる */
  input {
    position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px;
    overflow: hidden; clip-path: inset(50%); white-space: nowrap; border: 0;
  }
</style>
