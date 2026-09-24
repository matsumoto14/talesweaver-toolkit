<script lang="ts" module>
  import type { SourceId } from "../sourceId";

  export interface ExternalSource {
    /** 押すと移れる補正源。**無いものもある** — ペット・カード・ルーンの属性値のように、
     *  その補正源のペインでは入力しない値(ここでしか触れない)は移る先が無い */
    id?: SourceId;
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
  // (移る先が無い供給源は `id` を持たない。その行はチップを出さない)
  import Chip from "../../../ui/Chip.svelte";
  import Value from "../../../ui/Value.svelte";
  import { t } from "../../../i18n";

  interface Props {
    rows: ExternalSource[];
    title: string;
    onOpenSource: (id: SourceId) => void;
  }
  let { rows, title, onOpenSource }: Props = $props();
</script>

<style>
  .ext-no-open { visibility: hidden; }
</style>

{#if rows.length > 0}
  <div class="card">
    <div class="card-title">{title}</div>
    {#each rows as r ((r.id ?? "") + r.name)}
      <div class="ext-row" class:empty={r.value === 0}>
        <span class="ext-name">
          {r.name}
          {#if r.note}<span class="ext-note">{r.note}</span>{/if}
        </span>
        <!-- 0 は「−0%」「×1.00」ではなく — で出す(入っていないことを値の形で言わない) -->
        <Value class="ext-value" motion={() => r.value} value={r.value === 0 ? "—" : r.format(r.value)} />
        {#if r.id}
          <Chip class="quiet" onclick={() => onOpenSource(r.id!)}>{t("開く ›")}</Chip>
        {:else}
          <!-- 移る先が無い行。同じチップを `visibility: hidden` で置くと幅が完全に一致するので、
               値の列が行をまたいで揃う(§00 01)。非表示なのでフォーカスも当たらない -->
          <span class="ext-no-open" aria-hidden="true">
            <Chip class="quiet" onclick={() => {}}>{t("開く ›")}</Chip>
          </span>
        {/if}
      </div>
    {/each}
  </div>
{/if}
