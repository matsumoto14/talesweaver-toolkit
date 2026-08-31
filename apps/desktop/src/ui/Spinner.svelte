<script lang="ts">
  // 「いま出ている数字はまだ確定していない」ことだけを示す印。**待たせるための演出ではない**
  // (design-system §10「待たせるための動きは 1 つも要らない」— そちらは読み込み中の飾りの話)。
  //
  // 置き場所は見出しの帯など、**レイアウトを押さないところ**に限る。本文の途中に出し入れすると
  // 下が上下して、待っているあいだ画面が落ち着かない(§09 規則 3・4)。
  // 枠は回っていないときも確保して、出た瞬間に隣がずれないようにする。
  //
  // 動き(回転)は app.css の `tw-spin`。prefers-reduced-motion では止まって、
  // 静止した円弧が残る(出ていること自体は opacity で伝わる)。
  interface Props {
    active: boolean;
    /** 読み上げ用。何を待っているかを言う(「集計しています」など) */
    label?: string;
  }
  let { active, label = "計算しています" }: Props = $props();
</script>

<span class="spinner" class:on={active} role="status" aria-label={active ? label : undefined}></span>

<style>
  .spinner {
    flex: none; width: 12px; height: 12px; border-radius: 50%;
    border: 2px solid currentColor; border-top-color: transparent;
    opacity: 0; transition: opacity 120ms linear;
  }
  .spinner.on { opacity: 0.55; animation: tw-spin 0.7s linear infinite; }
</style>
