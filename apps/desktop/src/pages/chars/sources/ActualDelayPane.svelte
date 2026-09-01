<script lang="ts">
  // 「actualDelay」補正源のペイン。このキャラ固有の中ディレイ減少スキルと、
  // ほかの補正源から入ってくる分(共通スキル・マスタリー・ランダムOP・シエナ)の一覧。
  // 合算・上限適用は Rust 側(preview_effective_stats)で解決済みなので、ここは preview の値を
  // 表示用に % へ整形するだけ(供給源別の内訳は crates/domain/src/stat_sources.rs の StatPreview)。
  import type { CharacterSkillEffectsView, StatPreview } from "../../../api/types";
  import { effectLabel, ownSkills, resolvedEffectsOf, toggleCharacterSkill } from "../../../characterSkills";
  import type { Draft } from "../../../draft";
  import { limits } from "../../../limits.svelte";
  import { app } from "../../../state.svelte";
  import { flash } from "../../../ui/motion.svelte";
  import type { SourceId } from "../sourceId";
  import ExternalSourceList, { type ExternalSource } from "./ExternalSourceList.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    resolvedSkillEffects: CharacterSkillEffectsView[];
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, resolvedSkillEffects, onOpenSource }: Props = $props();

  /** Σ% の小数表現を表示用の % へ(四捨五入)。正の合算・上限適用は Rust 側 */
  const pct = (v: number) => Math.round(v * 100);

  const ownCharacterSkills = $derived(ownSkills(app.characterSkills, draft.gameCharacterId));
  const skillChecked = (id: string) => draft.statSources.character_skills.skill_ids.includes(id);
  function toggleCharSkill(id: string, on: boolean) {
    draft.statSources.character_skills.skill_ids = toggleCharacterSkill(
      draft.statSources.character_skills.skill_ids,
      id,
      on,
    );
  }
  /** 中ディレイ減少を持つキャラスキル */
  const delaySkills = $derived(
    ownCharacterSkills.filter((d) =>
      [...d.effects, ...d.mastery_overrides.flatMap((o) => o.effects)].some(
        (e) => typeof e !== "string" && "actual_delay" in e,
      ),
    ),
  );
  /** このキャラのスキルぶんの中ディレイ減少 %(共通の供給源は含まない)。
   *  供給源別の内訳は preview.character_skill_actual_delay(正は CharacterSkills::actual_delay_contributions) */
  const delaySkillPercent = $derived(
    pct((preview?.character_skill_actual_delay ?? []).reduce((sum, c) => sum + c.rate, 0)),
  );
  /** マスタリーぶんの中ディレイ減少 %。正は Masteries::actual_delay_reduction */
  const masteryDelayPercent = $derived(pct(preview?.mastery_actual_delay ?? 0));
  /** フルスロットル(共通スキル)の中ディレイ減少 %。0 = 未装着(計算は Rust 側) */
  const fullThrottlePercent = $derived(pct(preview?.common_skill.ultimate.actual_delay_reduction ?? 0));

  /** 中ディレイ減少に、この補正源の外から入ってくる分 */
  const delayFromOthers = $derived<ExternalSource[]>([
    { id: "commonSkill", name: "フルスロットル(共通スキル)", value: fullThrottlePercent, format: (v) => `−${v}%` },
    {
      id: "skills",
      name: "マスタリー(キャラスキル)",
      value: masteryDelayPercent,
      format: (v) => `−${v}%`,
      note: "段ごとに 1 つ。中ディレイ以外に効く選択肢もある",
    },
    {
      id: "randomOption",
      name: "ランダムOP(カフス)",
      value: pct(preview?.random_option_totals.actual_delay_reduction ?? 0),
      format: (v) => `−${v}%`,
    },
    { id: "siena", name: "シエナのオーラ", value: pct(preview?.siena_actual_delay_rate ?? 0), format: (v) => `−${v}%` },
  ]);
</script>

<div class="card">
  <p class="hint dim">
    wiki「ステータス」の<b>中ディレイ倍率B</b>。中ディレイは
    <b>基本中ディレイ × (1 − 減少値) ×(コンボするなら 0.5)</b>で、下限 {limits.actual_delay_min.toFixed(1)}s・減少値の上限 {Math.round(limits.actual_delay_reduction_max * 100)}%。
    ここで選ぶのは<b>このキャラのスキル</b>だけです
    (マスタリーは段ごとに 1 つで中ディレイ以外にも効くので、キャラスキルの欄にまとめてあります)。
    中ディレイと 1 秒あたりの火力は計算タブに出ます。
  </p>
  <div class="buff-list">
    {#if delaySkills.length === 0}
      <p class="empty dim">このキャラには中ディレイ減少のスキルがありません(wiki の表に記載なし)。</p>
    {/if}
    {#each delaySkills as def (def.id)}
      {@const label = effectLabel(resolvedEffectsOf(def.id, resolvedSkillEffects))}
      <label class="check">
        <input
          type="checkbox"
          checked={skillChecked(def.id)}
          onchange={(e) => toggleCharSkill(def.id, e.currentTarget.checked)}
        />
        <span>{def.name}</span>
        <span class="fixed-value dim num" use:flash={() => label ?? ""}>{label ?? "マスタリー未取得"}</span>
        {#if def.note}<span class="dim note">{def.note}</span>{/if}
      </label>
    {/each}
  </div>
  {#if delaySkillPercent > 0}
    <p class="hint dim">このキャラのスキルぶん: <b>−{delaySkillPercent}%</b></p>
  {/if}
</div>
<ExternalSourceList rows={delayFromOthers} title="ほかの補正源から入る分" {onOpenSource} />
