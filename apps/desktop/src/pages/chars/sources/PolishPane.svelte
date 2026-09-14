<script lang="ts">
  // 「polish」補正源のペイン。装備研磨(部位ごとに能力値 1 つを上げる消耗品。種類 3 つ:
  // ピカピカ = 素の補正の3%切り上げ、職人 = 5%切り上げ、聖なる = 固定値(武器・鎧+4/それ以外+2)。
  // 「研磨剤」(武器・鎧)/「ワックス」(それ以外)の呼び分けは部位から決まる。
  //
  // 並びは装備ペイン・アバター強化ペインと同じ: 部位を縦に並べ、開いた部位だけ編集する
  // (AvatarPane.svelte を踏襲)。1 部位に同時 1 つ。レリックは段階成長の別モデルなので対象外
  // (POLISH_ALLOWED_SLOTS)。実際に効くにはバフ「装備研磨」を ON にする必要がある(記録は常設)。
  import type { EquipmentPolish, EquipmentStatKind, PartSlot, PolishKind, SkillDependency, StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { polishAmount, polishProductLabel, selectedEquipmentPart, selectedEquipmentPartOrNeutral, zeroValues } from "../../../equipment";
  import { fmtInt } from "../../../format";
  import {
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT, OTHER_EQUIPMENT_STATS, PART_SLOT_LABELS,
    POLISH_ALLOWED_SLOTS, POLISH_KIND_LABELS, PRIMARY_EQUIPMENT_STATS,
  } from "../../../labels";
  import { bump, flash } from "../../../ui/motion.svelte";
  import { equipmentAttackKindsFor } from "../summaries";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    /** 主軸スキルの依存種別。研磨の種類を初めて選んだときの対象能力値の自動決めに使う */
    dependency: SkillDependency | null;
  }
  let { draft, preview, dependency }: Props = $props();

  const POLISH_KINDS: PolishKind[] = ["sparkle", "artisan", "holy"];

  const entryOf = (slot: PartSlot): EquipmentPolish | undefined =>
    draft.equipment.polish.entries.find((e) => e.slot === slot);

  const isEquipped = (slot: PartSlot): boolean => selectedEquipmentPart(draft.equipment.parts[slot]) !== null;

  const amountOf = (entry: EquipmentPolish): number => {
    const part = selectedEquipmentPartOrNeutral(draft.equipment.parts[entry.slot]);
    return polishAmount(entry.kind, entry.slot, part.base[entry.stat]);
  };

  // --- 効いている量(結果。バフ ON/OFF に関わらず記録の合計を出す) -------------
  const totals = $derived.by(() => {
    const sum = zeroValues();
    for (const entry of draft.equipment.polish.entries) sum[entry.stat] += amountOf(entry);
    return sum;
  });
  const totalsLabel = $derived(
    EQUIPMENT_STAT_KINDS.filter((k) => totals[k] > 0)
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]} +${fmtInt(totals[k])}`)
      .join(" ・ ") || "未使用",
  );
  /** いまバフ「装備研磨」が ON か(いつものバフでの判定。正は Rust 側 `equipment_polish_active`) */
  const polishActive = $derived(preview?.equipment_polish_active ?? false);

  // --- 部位の行 -----------------------------------------------------------
  function kindChipLabel(kind: PolishKind, slot: PartSlot): string {
    if (kind === "sparkle") return "ピカピカ 3%";
    if (kind === "artisan") return "職人 5%";
    return `聖なる +${polishAmount("holy", slot, 0)}`;
  }

  function partRowSummary(slot: PartSlot): string {
    const entry = entryOf(slot);
    if (!entry) return isEquipped(slot) ? "未使用" : "装備なし";
    const amount = amountOf(entry);
    const kindLabel = POLISH_KIND_LABELS[entry.kind];
    const statLabel = EQUIPMENT_STAT_SHORT[entry.stat];
    const percent = entry.kind === "sparkle" ? "+3%" : entry.kind === "artisan" ? "+5%" : null;
    return percent
      ? `${kindLabel} ${statLabel} ${percent} → +${fmtInt(amount)}`
      : `${kindLabel} ${statLabel} → +${fmtInt(amount)}`;
  }

  let openSlot = $state<PartSlot | null>(null);
  let showOtherStats = $state(false);
  function openPartRow(slot: PartSlot) {
    openSlot = openSlot === slot ? null : slot;
    const entry = entryOf(slot);
    showOtherStats = entry !== undefined && OTHER_EQUIPMENT_STATS.includes(entry.stat);
  }
  const visibleStats = $derived(showOtherStats ? [...PRIMARY_EQUIPMENT_STATS, ...OTHER_EQUIPMENT_STATS] : PRIMARY_EQUIPMENT_STATS);

  /** 種類チップを押した瞬間に反映する(「適用」ボタンは挟まない)。「なし」で記録を外す。
   *  初めて種類を選んだときは、対象能力値を主軸スキルの依存から自動で埋める。 */
  function selectKind(slot: PartSlot, kind: PolishKind | null) {
    const entries = draft.equipment.polish.entries;
    const index = entries.findIndex((e) => e.slot === slot);
    if (kind === null) {
      if (index >= 0) entries.splice(index, 1);
      return;
    }
    if (index >= 0) {
      entries[index].kind = kind;
      return;
    }
    const defaultStat = equipmentAttackKindsFor(dependency)[0] ?? "thrust";
    entries.push({ slot, kind, stat: defaultStat });
  }

  function selectStat(slot: PartSlot, stat: EquipmentStatKind) {
    const entry = entryOf(slot);
    if (entry) entry.stat = stat;
  }
</script>

<div class="result-value num">
  <span class="dim tiny">研磨 効いている量(基本能力値へ合流)</span>
  <span class="strong" use:flash={() => totalsLabel}>{totalsLabel}</span>
  {#if preview !== null}
    <p class="hint dim">
      {#if polishActive}
        いつものバフで「装備研磨」が ON なので、この量が効いています。
      {:else}
        効かせるにはバフ「装備研磨」を ON にしてください(いつものバフ / 計算の材料)。
      {/if}
    </p>
  {/if}
</div>

<div class="card">
  <details class="fold">
    <summary>この画面の読み方</summary>
    <div class="fold-body">
      <p class="hint dim">
        装備研磨は消耗品で、選んだ装備の能力値 1 つを上げます。種類は 3 つ:
        <b>ピカピカ</b> = 素の補正(エンチャントを除く実測値)の 3% 切り上げ、
        <b>職人</b> = 5% 切り上げ、<b>聖なる</b> = 固定値(武器・鎧 +4 / それ以外 +2)。
        1 部位に同時 1 つで、期限はありません。レリックは段階成長の別モデルなので対象外です。
      </p>
      <p class="hint dim">「研磨剤」(武器・鎧)/「ワックス」(それ以外)の呼び名は部位から決まります。</p>
    </div>
  </details>

  <div class="part-list">
    {#each POLISH_ALLOWED_SLOTS as slot (slot)}
      {@const summary = partRowSummary(slot)}
      {@const entry = entryOf(slot)}
      {@const equipped = isEquipped(slot)}
      <button type="button" class="part-row" class:on={openSlot === slot} onclick={() => openPartRow(slot)}>
        <span class="part-main">
          <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
          <span class="part-item" class:dim={summary === "未使用" || summary === "装備なし"} use:flash={() => summary}>
            {summary}
          </span>
        </span>
        <span class="chev dim">›</span>
      </button>
      {#if openSlot === slot}
        <div class="avatar-editor" aria-label={`${PART_SLOT_LABELS[slot]}の研磨`}>
          <div class="polish-kind-row" role="radiogroup" aria-label={`${PART_SLOT_LABELS[slot]}の${polishProductLabel(slot)}`}>
            <span class="dim tiny">{polishProductLabel(slot)}</span>
            <button
              type="button"
              class="chip"
              class:on={entry === undefined}
              role="radio"
              aria-checked={entry === undefined}
              onclick={() => selectKind(slot, null)}
            >なし</button>
            {#each POLISH_KINDS as kind (kind)}
              <button
                type="button"
                class="chip"
                class:on={entry?.kind === kind}
                role="radio"
                aria-checked={entry?.kind === kind}
                onclick={() => selectKind(slot, kind)}
              >{kindChipLabel(kind, slot)}</button>
            {/each}
          </div>
          {#if !equipped}
            <p class="hint dim">この部位は未装備です。加算値は装備してから決まります(いまは 0)。</p>
          {/if}
          {#if entry}
            {#each visibleStats as kind (kind)}
              <div class="polish-stat-row" class:secondary-stat={!PRIMARY_EQUIPMENT_STATS.includes(kind)}>
                <button
                  type="button"
                  class="chip"
                  class:on={entry.stat === kind}
                  role="radio"
                  aria-checked={entry.stat === kind}
                  onclick={() => selectStat(slot, kind)}
                >{EQUIPMENT_STAT_SHORT[kind]}</button>
                {#if entry.stat === kind}
                  <span class="stat-total num" use:bump={() => amountOf(entry)}>+{fmtInt(amountOf(entry))}</span>
                {/if}
              </div>
            {/each}
            <button
              type="button"
              class="enchant-more-toggle"
              aria-expanded={showOtherStats}
              onclick={() => (showOtherStats = !showOtherStats)}
            >
              <span><b>物防・命中など5補正</b><small>物防 / 命中 / Cri / 回避 / 敏捷</small></span>
              <span class="toggle-state">{showOtherStats ? "閉じる ︿" : "開く ﹀"}</span>
            </button>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .part-list { margin-top: 9px; }
  .avatar-editor {
    margin: -2px 0 4px 18px; padding: 6px 8px 2px;
    border-left: 2px solid var(--accent); background: var(--surface-inset); border-radius: 0 var(--r-inset) var(--r-inset) 0;
    display: flex; flex-direction: column; gap: 6px;
  }
  .polish-kind-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .polish-stat-row { display: flex; align-items: center; gap: 10px; }
  .polish-stat-row.secondary-stat .chip { color: var(--fg-sub); font-weight: 500; }
  .stat-total { min-width: 52px; font-size: 9.5px; color: var(--accent-hover); }
</style>
