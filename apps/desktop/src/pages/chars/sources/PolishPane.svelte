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
  import { fmtSigned } from "../../../format";
  import {
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT, OTHER_EQUIPMENT_STATS, PART_SLOT_LABELS,
    POLISH_ALLOWED_SLOTS, POLISH_KIND_LABELS, PRIMARY_EQUIPMENT_STATS,
  } from "../../../labels";
  import { flash } from "../../../ui/motion.svelte";
  import Picker, { type PickerOption } from "../../../ui/Picker.svelte";
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
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]} ${fmtSigned(totals[k])}`)
      .join(" ・ ") || "未使用",
  );
  /** いまバフ「装備研磨」が ON か(いつものバフでの判定。正は Rust 側 `equipment_polish_active`) */
  const polishActive = $derived(preview?.equipment_polish_active ?? false);

  // --- 部位の行 -----------------------------------------------------------
  /** 種類を選んだときに乗る先の能力値。まだ記録が無ければ、選んだ瞬間に自動で埋まる値と同じ */
  const targetStatOf = (slot: PartSlot): EquipmentStatKind =>
    entryOf(slot)?.stat ?? equipmentAttackKindsFor(dependency)[0] ?? "thrust";
  const amountFor = (slot: PartSlot, kind: PolishKind, stat: EquipmentStatKind): number =>
    polishAmount(kind, slot, selectedEquipmentPartOrNeutral(draft.equipment.parts[slot]).base[stat]);
  /** 種類の候補(§07「1 つ選ぶ」)。「なし」も候補の 1 行。値は「その種類を選ぶと乗る量」 */
  const kindOptions = (slot: PartSlot): PickerOption[] => {
    const stat = targetStatOf(slot);
    const rate = (kind: PolishKind) => kind === "sparkle" ? "3% ・ " : kind === "artisan" ? "5% ・ " : "";
    return [
      { value: "", name: "なし", meta: "記録しない" },
      ...POLISH_KINDS.map((kind) => ({
        value: kind, name: POLISH_KIND_LABELS[kind],
        meta: `${rate(kind)}${EQUIPMENT_STAT_SHORT[stat]} ${fmtSigned(amountFor(slot, kind, stat))}`,
      })),
    ];
  };
  /** 乗せる能力値の候補。主要 4 補正はチップで手前に固定、物防・命中などは候補面(§07「1 つ選ぶ」) */
  const statOptions = (slot: PartSlot, entry: EquipmentPolish): PickerOption[] =>
    [...PRIMARY_EQUIPMENT_STATS, ...OTHER_EQUIPMENT_STATS].map((stat) => ({
      value: stat, name: EQUIPMENT_STAT_SHORT[stat],
      meta: fmtSigned(amountFor(slot, entry.kind, stat)),
      pinned: PRIMARY_EQUIPMENT_STATS.includes(stat),
    }));

  function partRowSummary(slot: PartSlot): string {
    const entry = entryOf(slot);
    if (!entry) return isEquipped(slot) ? "未使用" : "装備なし";
    const amount = amountOf(entry);
    const kindLabel = POLISH_KIND_LABELS[entry.kind];
    const statLabel = EQUIPMENT_STAT_SHORT[entry.stat];
    const percent = entry.kind === "sparkle" ? "+3%" : entry.kind === "artisan" ? "+5%" : null;
    return percent
      ? `${kindLabel} ${statLabel} ${percent} → ${fmtSigned(amount)}`
      : `${kindLabel} ${statLabel} → ${fmtSigned(amount)}`;
  }

  let openSlot = $state<PartSlot | null>(null);
  function openPartRow(slot: PartSlot) {
    openSlot = openSlot === slot ? null : slot;
  }

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
        効かせるには計算タブの材料「研磨」を ON にしてください(バフ「装備研磨」と同じスイッチ)。
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
          <div class="polish-kind-row" aria-label={`${PART_SLOT_LABELS[slot]}の${polishProductLabel(slot)}`}>
            <span class="dim tiny">{polishProductLabel(slot)}</span>
            <Picker
              options={kindOptions(slot)}
              bind:value={() => entry?.kind ?? "", (v) => selectKind(slot, v === "" ? null : v as PolishKind)}
            />
          </div>
          {#if !equipped}
            <p class="hint dim">この部位は未装備です。加算値は装備してから決まります(いまは 0)。</p>
          {/if}
          {#if entry}
            <div class="polish-stat-row">
              <span class="dim tiny">乗せる先</span>
              <Picker
                options={statOptions(slot, entry)}
                bind:value={() => entry.stat, (v) => selectStat(slot, v as EquipmentStatKind)}
              />
            </div>
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
  .polish-kind-row, .polish-stat-row { display: flex; align-items: flex-start; gap: 6px; }
  .polish-kind-row > .tiny, .polish-stat-row > .tiny { flex-shrink: 0; min-width: 56px; padding-top: 7px; }
  .polish-kind-row :global(.picker), .polish-stat-row :global(.picker) { min-width: 0; flex: 1; }
</style>
