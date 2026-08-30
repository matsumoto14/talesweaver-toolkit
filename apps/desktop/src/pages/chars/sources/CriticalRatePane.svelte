<script lang="ts">
  // 「criticalRate」補正源のペイン。ペット会心・極のルーン・致命打・設計者の研究室と、
  // ほかの補正源から自動で入ってくる分(装備クリティカル補正・AGI・主軸スキルの Cri値・シエナ)。
  // 合算・研究室段階の換算・シエナの %→倍率換算はすべて Rust 側(preview_effective_stats)が
  // 解決済みなので、ここは preview の値を表示用に整形するだけ。
  import type { Skill, StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { limits } from "../../../limits.svelte";
  import { fmtInt } from "../../../format";
  import { bump } from "../../../ui/motion.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";
  import type { SourceId } from "../sourceId";
  import ExternalSourceList, { type ExternalSource } from "./ExternalSourceList.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, skills, onOpenSource }: Props = $props();

  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);

  /** 設計者の研究室ぶんのクリティカル率増加(研究段階 × 1 段階あたりの増加量)。正は
   *  CriticalRateSources::architect_lab_bonus(preview.critical_rate_bonus.architect_lab_bonus) */
  const architectLabBonus = $derived(preview?.critical_rate_bonus.architect_lab_bonus ?? 0);
  /** 研究段階の選択肢。0〜10 段階を「N 段階(+3N%)」で並べる(§07 形態 2) */
  const architectLabOptions = $derived(
    Array.from({ length: limits.architect_lab_stage_max + 1 }, (_, i) => ({
      value: String(i),
      label: String(i),
    })),
  );
  /** クリティカル率増加の合計(上限を掛ける前。計算は Rust 側) */
  const criticalRateBonus = $derived(preview?.critical_rate_bonus.raw ?? 0);
  /** 装備クリティカル補正の合計(基本 + 強化。バッチ 2 で preview に入った合計をそのまま使う) */
  const equipmentCriticalTotal = $derived(
    (preview?.equipment_base_total.critical ?? 0) + (preview?.equipment_enhanced_total.critical ?? 0),
  );

  /** クリティカル率に、この補正源の外から入ってくる分 */
  const criticalFromOthers = $derived<ExternalSource[]>([
    {
      id: "equipment",
      name: "装備クリティカル補正",
      value: equipmentCriticalTotal,
      format: (v) => `+${fmtInt(v)}`,
      note: "(装備クリティカル補正 + 1) × 2 の項",
    },
    {
      id: "status",
      name: "AGI(最終能力値)",
      value: preview?.stats.agi ?? 0,
      format: (v) => fmtInt(v),
      note: "AGI /(AGI + 対象のAGI)の項",
    },
    {
      id: "status",
      name: "主軸スキルの Cri値",
      value: mainSkill?.critical_rate ?? 0,
      format: (v) => `+${v}%`,
      note: mainSkill
        ? mainSkill.critical_rate === null
          ? `${mainSkill.name} は wiki 未記載`
          : mainSkill.name
        : "主軸スキル未選択",
    },
    {
      id: "siena",
      name: "シエナのオーラのクリティカル確率",
      value: preview?.siena_critical_rate ?? 0,
      format: (v) => `×${(1 + v).toFixed(2)}`,
      note: "AGI 由来の項に乗算。下の合計には入らない",
    },
  ]);
</script>

<div class="card">
  <p class="hint dim">
    wiki「計算式まとめ <b>#CriticalChance</b>」。クリティカル率は
    <b>(装備クリティカル補正 + 1) × 2 × (AGI / (AGI + 対象のAGI)) × ペット会心
    ＋ スキルの Cri値 ＋ クリティカル率増加 ＋ 対象のクリティカル被撃率</b>で、下限 {limits.critical_rate_min}% / 上限 {limits.critical_rate_max}%。
    装備クリティカル補正・AGI・スキルの Cri値は登録済みのデータから自動で入るので、
    ここで選ぶのは<b>ペット会心と「クリティカル率増加」</b>だけです(自動で入る分は下に出します)。
    対象のAGI とクリティカル被撃率は wiki 狩り場情報一覧に値がある敵だけに入っているので、
    計算タブでは<b>その敵を選んだときだけ</b>クリティカル率が出ます。
  </p>
  <div class="buff-list">
    <label class="check">
      <input type="checkbox" bind:checked={draft.statSources.critical_rate.pet} />
      <span>ペット会心</span>
      <span class="fixed-value dim">×{limits.pet_critical_rate}</span>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={draft.statSources.critical_rate.ultimate_rune} />
      <span>極のルーン</span>
      <span class="fixed-value dim">+{limits.ultimate_rune_bonus_max}%</span>
      <span class="dim note">最大レベル時</span>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={draft.statSources.critical_rate.deadly_blow} />
      <span>致命打</span>
      <span class="fixed-value dim">+{limits.deadly_blow_bonus_max}%</span>
    </label>
  </div>
  <!-- 設計者の研究室だけは段階制(wiki: B グループは最大 10 段階・1 段階 +3)。
       オン/オフだと 1〜9 段階の人が入力できない。チェックの列は割らずに下へ置く -->
  <div class="lab-field">
    <span class="lab-label">
      設計者の研究室
      <span class="lab-note">B グループの研究段階(0 = 未研究)・ 1 段階 +{limits.architect_lab_per_stage}%</span>
    </span>
    <StepSelect
      label=""
      options={architectLabOptions}
      cols={architectLabOptions.length}
      cell={34}
      bind:value={
        () => String(draft.statSources.critical_rate.architect_lab_stage),
        (v) => (draft.statSources.critical_rate.architect_lab_stage = Number(v))
      }
    />
    <span class="lab-value num" use:bump={() => architectLabBonus}>
      {architectLabBonus > 0 ? `+${architectLabBonus}%` : "—"}
    </span>
  </div>
  <p class="hint dim">
    クリティカル率増加の合計: <b>+{Math.min(limits.critical_rate_bonus_max, criticalRateBonus)}%</b>
    {#if criticalRateBonus > limits.critical_rate_bonus_max}(上限 +{limits.critical_rate_bonus_max}% で頭打ち){/if}
    <br />
    値が不定の「バフ」、被撃率B(対人)、最終クリティカル率増加は未収録です。
  </p>
</div>
<ExternalSourceList rows={criticalFromOthers} title="ほかの補正源から自動で入る分" {onOpenSource} />
