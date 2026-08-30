<script lang="ts">
  // 「title」補正源のペイン。装備枠 1 つ、表示中の 1 件だけが効く(wiki: 称号システム)。
  import type { TitleDef } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { fmtInt } from "../../../format";
  import { EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT } from "../../../labels";
  import { app } from "../../../state.svelte";
  import { flash } from "../../../ui/motion.svelte";

  interface Props {
    draft: Draft;
  }
  let { draft }: Props = $props();

  let titleQuery = $state("");
  const selectedTitle = $derived(app.titles.find((t) => t.id === draft.equipment.title) ?? null);
  /** 普段使う称号。無条件ダメージ +20%以上と、地域称号のうち実用される最上位 2 件。 */
  const titleIsCommon = (t: TitleDef): boolean =>
    t.attack_damage_percent >= 20 || t.id === "eclipse" || t.id === "shinchou_no_negura";
  /** 依存違いの一部だけに追加効果が書かれている場合も、同名の変種はまとめて常設する。 */
  const commonTitleBases = $derived(
    new Set(app.titles.filter(titleIsCommon).map((t) => t.name.split(" - ")[0])),
  );
  const commonTitles = $derived(app.titles.filter((t) => commonTitleBases.has(t.name.split(" - ")[0])));
  const otherTitles = $derived(app.titles.filter((t) => !commonTitleBases.has(t.name.split(" - ")[0])));
  const filteredOtherTitles = $derived.by(() => {
    const q = titleQuery.trim();
    if (q === "") return otherTitles;
    return otherTitles.filter((t) => t.name.includes(q) || t.group.includes(q));
  });
  /** 称号の補正値の要約(値が入っている列だけ)。 */
  const titleSummary = (t: TitleDef): string =>
    EQUIPMENT_STAT_KINDS.filter((k) => t.values[k] !== 0)
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]}${t.values[k]}`)
      .join(" ");
  /** 「緋馬の怪火 - 突き」の「緋馬の怪火」。同じ称号の依存違いをまとめる単位 */
  const titleBase = (t: TitleDef): string => t.name.split(" - ")[0];
  /** 同じ称号の変種(突き / 斬り / 魔攻 …)を 1 行にまとめる。
      カタログの並び(ダメージ増加の大きい順)は最初に出てきた変種の位置で保つ */
  const groupTitles = (titles: TitleDef[]) => {
    const groups = new Map<string, TitleDef[]>();
    for (const t of titles) {
      const base = titleBase(t);
      const list = groups.get(base);
      if (list) list.push(t);
      else groups.set(base, [t]);
    }
    return [...groups].map(([base, items]) => ({ base, items }));
  };
  const commonTitleGroups = $derived(groupTitles(commonTitles));
  const otherTitleGroups = $derived(groupTitles(filteredOtherTitles));

  const signed = (n: number) => `${n >= 0 ? "+" : ""}${fmtInt(n)}`;
</script>

<!-- 称号候補。依存違いだけの変種は 1 行にまとめ、選ぶのに必要な効果値を同じ行に出す。 -->
{#snippet titleRows(groups: { base: string; items: TitleDef[] }[])}
  {#each groups as g (g.base)}
    {#if g.items.length === 1}
      {@const t = g.items[0]}
      <button
        type="button"
        class="item-row"
        class:on={draft.equipment.title === t.id}
        onclick={() => (draft.equipment.title = t.id)}
      >
        <span class="item-name">{t.name}</span>
        {#if t.attack_damage_percent > 0}
          <span class="title-dmg num">ダメ +{t.attack_damage_percent}%</span>
        {:else if t.note.includes("追加ダメージ")}
          <span class="title-extra">条件付き追加ダメ</span>
        {/if}
      </button>
    {:else}
      {@const picked = g.items.find((t) => t.id === draft.equipment.title) ?? null}
      <div class="item-row group" class:on={picked !== null}>
        <span class="item-name">{g.base}</span>
        {#if g.items[0].attack_damage_percent > 0}
          <span class="title-dmg num">ダメ +{g.items[0].attack_damage_percent}%</span>
        {:else if g.items.some((t) => t.note.includes("追加ダメージ"))}
          <span class="title-extra">条件付き追加ダメ</span>
        {/if}
        <span class="title-variants">
          {#each g.items as t (t.id)}
            <button
              type="button"
              class="chip"
              class:on={draft.equipment.title === t.id}
              title="{t.name} — {titleSummary(t)}"
              onclick={() => (draft.equipment.title = t.id)}
            >{t.name.slice(g.base.length + 3)}</button>
          {/each}
        </span>
      </div>
    {/if}
  {/each}
{/snippet}

<div class="card">
  <div class="card-title">選択中</div>
  <div
    class="contrib-card title-current"
    class:empty={selectedTitle === null}
    use:flash={() => selectedTitle?.id ?? "none"}
  >
    <span class="item-name strong">{selectedTitle?.name ?? "未選択"}</span>
    {#if selectedTitle && titleSummary(selectedTitle) !== ""}
      <span class="item-vals num dim" title={titleSummary(selectedTitle)}>{titleSummary(selectedTitle)}</span>
    {/if}
    {#if selectedTitle?.attack_damage_percent}
      <span class="title-dmg num">ダメージ +{selectedTitle.attack_damage_percent}%</span>
    {:else if selectedTitle?.note.includes("追加ダメージ")}
      <span class="title-extra">条件付き追加ダメージ</span>
    {/if}
    {#if selectedTitle}
      <button type="button" class="chip quiet" onclick={() => (draft.equipment.title = null)}>外す</button>
    {/if}
  </div>
</div>
<div class="card">
  <p class="hint dim">
    <b>表示中の 1 件だけ</b>が効きます。普段使う候補だけを先に出し、それ以外は下の「その他」から選べます。
  </p>
  <details class="fold">
    <summary>称号の補正の入り方</summary>
    <div class="fold-body">
      <p class="hint dim">
        wiki「称号システム」。補正値は<b>装備の基本能力値</b>に乗り、<b>ダメージ n% 増加</b>はカテゴリX(攻撃ダメージ)に入ります。
        収録は主要称号のみ({app.titles.length} 件)。並びは<b>ダメージ増加 → 与ダメージに効く 1 値の大きさ</b>の順です。
        条件付き効果とグループボーナスは記録だけで、計算には入りません。
      </p>
    </div>
  </details>
  <div class="card-title space">
    よく使う称号 <span class="normal dim">ダメ +20%以上 / エクリプス / 神鳥の塒</span>
  </div>
  <div class="item-list title-list effectful">
    {@render titleRows(commonTitleGroups)}
  </div>
  <details class="fold">
    <summary>その他の称号から選ぶ({otherTitles.length} 件)</summary>
    <div class="fold-body">
      <input class="item-search" type="text" placeholder="称号名・グループで探す" bind:value={titleQuery} />
      {#if otherTitleGroups.length > 0}
        <div class="item-list title-list effectful">
          {@render titleRows(otherTitleGroups)}
        </div>
      {:else}
        <p class="hint dim">該当する称号はありません。</p>
      {/if}
    </div>
  </details>
</div>
{#if selectedTitle}
  {@const filled = EQUIPMENT_STAT_KINDS.filter((k) => selectedTitle.values[k] !== 0)}
  <div class="card">
    <div class="card-title inline">
      選択中の補正 <span class="normal dim">{selectedTitle.name}</span>
    </div>
    {#if selectedTitle.attack_damage_percent > 0}
      <p class="hint dim">
        ダメージ増加は<b>カテゴリX(攻撃ダメージ)</b>の X3 基本発動に入ります(wiki: ステータス。X3 は上限 +80%)。
      </p>
    {/if}
    {#if filled.length > 0}
      <div class="values-grid">
        {#each filled as k (k)}
          <span class="val-cell">
            <span class="dim">{EQUIPMENT_STAT_SHORT[k]}</span>
            <span class="num strong">{signed(selectedTitle.values[k])}</span>
          </span>
        {/each}
      </div>
    {:else}
      <p class="hint dim">補正値はありません(ダメージ増加だけの称号)。</p>
    {/if}
    <p class="hint dim">
      {selectedTitle.group}{selectedTitle.level !== null ? ` ・ 習得 Lv${selectedTitle.level}` : ""}
      {#if selectedTitle.note}<br />{selectedTitle.note}{/if}
    </p>
  </div>
{/if}
