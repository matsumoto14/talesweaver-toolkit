<script lang="ts" module>
  import type { SourceId } from "../sourceId";

  export interface ExternalSource {
    id: SourceId;
    name: string;
    value: number;
    format: (value: number) => string;
    note?: string;
  }
</script>

<script lang="ts">
  // ほかの補正源から入ってくる分の一覧(中ディレイ・クリティカル率ペインで共有)。
  // **0 の行も出す** — ここは「この値がどこから来るか」の地図でもあるので、
  // 入っていない供給源を消すと存在に気づけない。0 の行は薄くする。押すとその補正源へ移る
  import Num from "../../../ui/Num.svelte";

  interface Props {
    rows: ExternalSource[];
    title: string;
    onOpenSource: (id: SourceId) => void;
  }
  let { rows, title, onOpenSource }: Props = $props();
</script>

{#if rows.length > 0}
  <div class="card">
    <div class="card-title">{title}</div>
    {#each rows as r (r.id + r.name)}
      <div class="ext-row" class:empty={r.value === 0}>
        <span class="ext-name">
          {r.name}
          {#if r.note}<span class="ext-note">{r.note}</span>{/if}
        </span>
        <!-- 0 は「−0%」「×1.00」ではなく — で出す(入っていないことを値の形で言わない) -->
        <Num class="ext-value" motion={() => r.value} value={r.value === 0 ? "—" : r.format(r.value)} />
        <button type="button" class="chip quiet" onclick={() => onOpenSource(r.id)}>開く ›</button>
      </div>
    {/each}
  </div>
{/if}
