<script lang="ts">
  // 中央 + 右カラムの 2 カラム構成(v4 の「行ける?」/「どこまで通るか」の共通シェル)。
  // HomePage / CalcPage で .layout grid + Splitter + .head-bar + .scroll が重複していたのを部品化。
  // Workspace は左レール込みの 3 カラムで構造が違うため、無理にここへは寄せない。
  import type { Snippet } from "svelte";
  import { persisted } from "./persistedState.svelte";
  import Splitter from "./Splitter.svelte";

  interface Props {
    midTitle: string;
    midNote?: string;
    rightTitle: string;
    rightNote?: string;
    /** localStorage キー(ページごとに固有)。右カラム幅の永続化に使う */
    persistKey: string;
    defaultRight: number;
    minMid: number;
    minRight: number;
    splitterLabel: string;
    /** 中央 .scroll への追加 style(scrollbar-gutter など、ページごとの微差の吸収) */
    midScrollStyle?: string;
    /** 右 .scroll.pad への追加 style(padding / gap のページごとの微差の吸収) */
    rightScrollStyle?: string;
    mid: Snippet;
    right: Snippet;
  }
  let {
    midTitle, midNote = "", rightTitle, rightNote = "",
    persistKey, defaultRight, minMid, minRight, splitterLabel,
    midScrollStyle = "", rightScrollStyle = "",
    mid, right,
  }: Props = $props();

  const layoutWidths = persisted(persistKey, { right: defaultRight });
  const gridTemplateColumns = $derived(
    `minmax(${minMid}px, 1fr) 6px minmax(${minRight}px, ${layoutWidths.value.right ?? defaultRight}px)`,
  );
</script>

<div class="layout" style="grid-template-columns: {gridTemplateColumns};">
  <section class="mid">
    <div class="head-bar">
      <span class="title">{midTitle}</span>
      <span class="note">{midNote}</span>
    </div>
    <div class="scroll" style={midScrollStyle}>
      {@render mid()}
    </div>
  </section>

  <Splitter
    bind:value={layoutWidths.value.right}
    min={minRight}
    defaultValue={defaultRight}
    controls="next"
    label={splitterLabel}
  />

  <section class="right">
    <div class="head-bar">
      <span class="title">{rightTitle}</span>
      <span class="note">{rightNote}</span>
    </div>
    <div class="scroll pad" style={rightScrollStyle}>
      {@render right()}
    </div>
  </section>
</div>

<style>
  .layout { flex: 1; min-height: 0; display: grid; }
  section { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  section.mid { background: var(--bg-mid); }
  section.right { background: var(--bg-rail); border-left: 1px solid var(--border-strong); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 13px 16px 18px; }
  .scroll.pad { display: flex; flex-direction: column; padding: 12px; gap: 9px; }
</style>
