<script lang="ts">
  // 結論文(LLM · 検証済)。地の文はそのまま、参照 `{{…}}` の値だけ `<Value>`(数値書体)で出す
  // (design-system §08「値は Value 1 つ」)。値は wiki の色のまま(ゼリッピの言葉に見せない)。
  import type { Correction, LeadSeg, Unit } from "../../ask";
  import Value from "../../ui/Value.svelte";
  import { renderLead } from "./leadLine";

  interface Props {
    segments: LeadSeg[];
    unitsById: Map<string, Unit>;
    corrections: Correction[];
  }
  let { segments, unitsById, corrections }: Props = $props();

  const parts = $derived(renderLead(segments, unitsById, corrections));
</script>

{#each parts as part, i (i)}
  {#if part.text}{part.text}{:else if part.value !== undefined}<Value value={part.value} class="lead-value" />{/if}
{/each}
