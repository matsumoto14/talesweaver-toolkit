<script lang="ts">
  // ペット/ルーン/クラウン/モンスターカード/聖物で共通の「8(または対応)ステ 1 行ずつ」の
  // 並び(`.stat-rows` > `.stat-row` × 各ステ)。行の中身(入力欄・値表示)はペインごとに違うので
  // snippet で受け取る。値が変わったときに枠だけ光らせたい行(クラウン)向けに flashValue も持つ。
  import type { StatKind } from "../../../api/types";
  import type { Snippet } from "svelte";
  import { STAT_LABELS } from "../../../labels";
  import { flash } from "../../../ui/motion.svelte";

  interface Props {
    kinds: readonly StatKind[];
    twoCol?: boolean;
    row: Snippet<[StatKind]>;
    flashValue?: (k: StatKind) => number;
  }
  let { kinds, twoCol = false, row, flashValue }: Props = $props();
</script>

<div class="stat-rows" class:two={twoCol}>
  {#each kinds as k (k)}
    <div class="stat-row" use:flash={() => (flashValue ? String(flashValue(k)) : k)}>
      <span class="k">{STAT_LABELS[k]}</span>
      {@render row(k)}
    </div>
  {/each}
</div>
