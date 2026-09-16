<script lang="ts">
  // 小さな的 1 つ(design-system §07 形態 3 / §08 チップ)。**アプリ中の的はこれ 1 つ**。
  //
  // 何になるかは `on` が渡っているかで決まる —— 見た目の選択肢ではない:
  //
  //   - `on` 無し  … 押すと何かが起きる的。`<button>`(「未装備にする」「開く ›」「+ 追加」)
  //   - `on` あり  … なっている / なっていないを表す的。`<input type=checkbox>` + `<label>`
  //   - `on` + `name` … 並んだ中の 1 つ。`<input type=radio>`(`Choose` だけが渡す)
  //
  // **input を素の checkbox / radio にしてあるのは、キーボードをブラウザに持たせるため。**
  // 同じ `name` の radio は、矢印キーでの移動・Tab ストップが 1 つだけになること・
  // 読み上げの「n 個中 m 個目」を既定で持つ。自作の `aria-pressed` ボタンには何も無い。
  // input は見えないが `display: none` にはしない(消すとフォーカスも消える)。
  //
  // 見た目は app.css の `.chip`。**段・タブの見た目は入れ物側**(`.seg .chip` / `.tabs .chip`)が
  // 上書きするので、ここでは持たない。
  import type { Snippet } from "svelte";

  interface Props {
    /** 渡すと状態を持つ的になる。渡さなければ押すと何かが起きる的 */
    on?: boolean;
    /** 並んだ中の 1 つになる(radio 群の名前)。`Choose` だけが渡す */
    name?: string;
    /** radio / checkbox の値。読み上げと form のため */
    value?: string;
    /** app.css の修飾(`quiet` / `add` / `todo` / 属性色など)。`chip` は付け直さない */
    class?: string;
    disabled?: boolean;
    title?: string;
    /** 状態を持つ的を押したとき。`on` を渡すなら要る */
    onToggle?: () => void;
    /** 押すと何かが起きる的を押したとき。`on` を渡さないなら要る */
    onclick?: () => void;
    children: Snippet;
  }
  let {
    on, name, value, class: cls = "", disabled = false, title, onToggle, onclick, children,
  }: Props = $props();
</script>

{#if on === undefined}
  <button type="button" class="chip {cls}" {disabled} {title} {onclick}>{@render children()}</button>
{:else}
  <label class="chip {cls}" class:on class:disabled {title}>
    <input
      type={name === undefined ? "checkbox" : "radio"}
      {name}
      {value}
      {disabled}
      checked={on}
      onchange={() => onToggle?.()}
    />
    {@render children()}
  </label>
{/if}
