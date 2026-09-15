<script lang="ts">
  // 「avatar」補正源のペイン。アバター強化(兜・頭・体・脚・エフェクトの5部位)。
  // 各部位は装備補正9値のどれにでも固定値を付与でき、同じ部位に複数の値を重ねられる
  // (兜に突き+12と命中+12の両方、等)。現行の強化剤は +10 / +12。旧品の +1 / +3 など
  // 3択外の値が既に入っているときは、その値をそのまま選べる状態にする(壊さない)。
  //
  // 並びは装備ペインと同じ: 部位を縦に並べ、開いた部位だけ編集する。補正は S/H/I/M の 4 つを
  // 先に出し、物防・命中など 5 補正は既定で畳む(必要なときだけ開く。値が入っていれば開いておく)。
  import type { AvatarPart, EquipmentStatKind } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { avatarEnhanceTotals } from "../../../equipment";
  import { fmtSigned } from "../../../format";
  import {
    AVATAR_PARTS, AVATAR_PART_LABELS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT,
    OTHER_EQUIPMENT_STATS, PRIMARY_EQUIPMENT_STATS,
  } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { bump, flash } from "../../../ui/motion.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";
  import { avatarEnhanceSummary } from "../summaries";

  interface Props {
    draft: Draft;
  }
  let { draft }: Props = $props();

  // 現行のアバター強化剤の値(クライアント DB dm_00000_0408/0425/0481)。旧品(+1/+3等)は
  // 既存データにあればセルの選択肢へその場で足す(cellOptions)。
  const CHOICES = [10, 12];

  // --- 効いている量(結果)。Workspace の行サブタイトルと同じ関数 ---------------
  const totalsLabel = $derived(avatarEnhanceSummary(draft));
  const totals = $derived(avatarEnhanceTotals(draft.equipment.avatar));

  /** 部位の行に出す要約(その部位の非 0 の値だけ)。 */
  function partSummary(part: AvatarPart): string {
    const values = draft.equipment.avatar[part];
    return (
      EQUIPMENT_STAT_KINDS.filter((k) => values[k] > 0)
        .map((k) => `${EQUIPMENT_STAT_SHORT[k]} ${fmtSigned(values[k])}`)
        .join(" ・ ") || "未使用"
    );
  }

  // --- 開いている部位と、畳んだ 5 補正 ----------------------------------------
  let openPart = $state<AvatarPart | null>("helm");
  let showOtherStats = $state(false);
  /** 畳んだ 5 補正に値が入っている部位は、開いたときに畳まない(入っている値を隠さない) */
  function openPartRow(part: AvatarPart) {
    openPart = openPart === part ? null : part;
    showOtherStats = OTHER_EQUIPMENT_STATS.some((k) => draft.equipment.avatar[part][k] > 0);
  }
  const visibleStats = $derived(showOtherStats ? [...PRIMARY_EQUIPMENT_STATS, ...OTHER_EQUIPMENT_STATS] : PRIMARY_EQUIPMENT_STATS);

  /** そのセルの選択肢(なし/+10/+12。既存値がそれ以外なら、その値も選べるよう足す)。 */
  function cellOptions(part: AvatarPart, kind: EquipmentStatKind) {
    const current = draft.equipment.avatar[part][kind];
    const values = [0, ...CHOICES];
    if (!values.includes(current)) values.push(current);
    values.sort((a, b) => a - b);
    return values.map((v) => ({ value: String(v), label: v === 0 ? "—" : fmtSigned(v) }));
  }
  function setCell(part: AvatarPart, kind: EquipmentStatKind, v: string) {
    draft.equipment.avatar[part][kind] = Number(v);
  }
</script>

<div class="result-value num">
  <span class="dim tiny">アバター強化 効いている量(強化能力値へ合流)</span>
  <span class="strong" use:flash={() => totalsLabel}>{totalsLabel}</span>
</div>

<div class="card">
  <!-- 説明は毎回読むものではない。畳んで、入力の場所を押し下げないようにする(§00 02) -->
  <details class="fold">
    <summary>この画面の読み方</summary>
    <div class="fold-body">
      <p class="hint dim">
        アバターは兜・頭・体・脚・エフェクトの5部位。各部位にアバター強化剤で装備補正
        {EQUIPMENT_STAT_KINDS.length}値のどれかを固定値で付与でき、<b>同じ部位に複数の値を重ねられます</b>
        (例: 兜に突き+{limits.avatar_enhance_max}と命中+{limits.avatar_enhance_max}の両方)。
        同じ値を5部位すべてに付けてもかまいません。加算後の値はエンチャント・テシスコアと
        同じ「強化能力値」に合流します。移動速度の強化剤は対象外です。
      </p>
      <p class="hint dim">
        現行のアバター強化剤は +10 / +12(期限つき)。期限はこのツールでは扱いません。
        旧品の +1 / +3 などが既に入っている部位はその値のまま選べます。
      </p>
    </div>
  </details>

  <div class="part-list">
    {#each AVATAR_PARTS as part (part)}
      {@const summary = partSummary(part)}
      <button type="button" class="part-row" class:on={openPart === part} onclick={() => openPartRow(part)}>
        <span class="part-main">
          <span class="part-name">{AVATAR_PART_LABELS[part]}</span>
          <span class="part-item" class:dim={summary === "未使用"} use:flash={() => summary}>{summary}</span>
        </span>
        <span class="chev dim">›</span>
      </button>
      {#if openPart === part}
        <div class="avatar-editor" aria-label={`${AVATAR_PART_LABELS[part]}のアバター強化`}>
          {#each visibleStats as kind (kind)}
            {@const options = cellOptions(part, kind)}
            <div class="avatar-stat-row" class:secondary-stat={!PRIMARY_EQUIPMENT_STATS.includes(kind)}>
              <b>{EQUIPMENT_STAT_SHORT[kind]}</b>
              <StepSelect
                label=""
                {options}
                cols={options.length}
                cell={34}
                bind:value={
                  () => String(draft.equipment.avatar[part][kind]),
                  (v) => setCell(part, kind, v)
                }
              />
              <span class="stat-total num" class:dim={totals[kind] === 0} use:bump={() => totals[kind]}>
                {totals[kind] === 0 ? "" : `5部位計 ${fmtSigned(totals[kind])}`}
              </span>
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
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .part-list { margin-top: 9px; }
  /* 開いた部位の編集面。行の直下に足す(押した行は動かない。§00 03) */
  .avatar-editor {
    margin: -2px 0 4px 18px; padding: 6px 8px 2px;
    border-left: 2px solid var(--accent); background: var(--surface-inset); border-radius: 0 var(--r-inset) var(--r-inset) 0;
    display: flex; flex-direction: column; gap: 5px;
  }
  .avatar-stat-row { display: grid; grid-template-columns: 34px max-content 1fr; align-items: center; gap: 10px; }
  .avatar-stat-row b { font-size: 11px; }
  .avatar-stat-row.secondary-stat b { color: var(--fg-sub); font-weight: 500; }
  /* 桁が増えても幅が動かない */
  .stat-total { min-width: 88px; font-size: 9.5px; font-variant-numeric: tabular-nums; color: var(--accent-hover); }
</style>
