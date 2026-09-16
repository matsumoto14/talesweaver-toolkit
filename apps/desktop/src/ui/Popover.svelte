<script lang="ts">
  /**
   * トリガの直下に重なるもの(design-system §09 規則 3)。`popover` 属性のトップレイヤーに載せ、
   * 置き場所は CSS Anchor Positioning(`anchor-name` / `position-area` / `position-try-fallbacks`)
   * が決める。見た目は app.css の `.popover`。
   *
   * 呼ぶ側が決めるのは「中に何を出すか」だけ。**外を押すと閉じる(light dismiss)/ Escape で
   * 閉じる / 重ね順 / 入らなければ上に開く / 祖先の overflow に切られない / 中身が伸びたら
   * 置き直す**はブラウザが持つ。開いた状態を覚える変数も、外側クリックの除外 selector も要らない。
   *
   * トリガもこの部品が持つ(`popovertarget` で結ぶので、呼ぶ側に id を配らせない)。
   * トリガは開いても閉じても動かない(§09 規則 1)。
   */
  import type { Snippet } from "svelte";

  interface Props {
    /** 面の名前(読み上げ)。その面固有の事実であって作法の選択肢ではない */
    label: string;
    /** トリガの的の形(`rest-link` など、その画面での見た目) */
    triggerClass?: string;
    /** トリガの読み上げ名。文字だけでは何の的か決まらないとき(行が 30 並ぶ「設定」など) */
    triggerLabel?: string;
    /** 面の幅など、その画面での見た目 */
    panelClass?: string;
    disabled?: boolean;
    /** 開いた / 閉じたを呼ぶ側にも知らせる。開いたときにだけ走らせたい計算がある面だけ */
    onToggle?: (open: boolean) => void;
    /** トリガの中身。開いているかどうかを受け取る(キャレットの向き) */
    trigger: Snippet<[boolean]>;
    /** 面の中身。閉じる操作を受け取る(選んだ瞬間に閉じる・明示の「閉じる」) */
    children: Snippet<[() => void]>;
  }
  let { label, triggerClass = "", triggerLabel, panelClass = "", disabled = false, onToggle, trigger, children }: Props = $props();

  const uid = $props.id();
  const id = `pop-${uid}`;
  let panel = $state<HTMLElement | null>(null);
  let open = $state(false);
  const close = () => panel?.hidePopover();
</script>

<button
  type="button"
  class={triggerClass}
  {disabled}
  popovertarget={id}
  aria-label={triggerLabel}
  aria-expanded={open}
  style="anchor-name: --{id};"
>{@render trigger(open)}</button>

<div
  bind:this={panel}
  {id}
  popover="auto"
  class="popover pop-in {panelClass}"
  role="dialog"
  aria-label={label}
  style="position-anchor: --{id};"
  ontoggle={(event) => { open = event.newState === "open"; onToggle?.(open); }}
>{@render children(close)}</div>
