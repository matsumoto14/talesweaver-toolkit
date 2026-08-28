<script lang="ts">
  // 「actualDelay」補正源のペイン。このキャラ固有の中ディレイ減少スキルと、
  // ほかの補正源から入ってくる分(共通スキル・マスタリー・ランダムOP・シエナ)の一覧。
  import type { StatPreview } from "../../../api/types";
  import { actualDelayPercent, effectLabel, ownSkills, toggleCharacterSkill } from "../../../characterSkills";
  import type { Draft } from "../../../draft";
  import { randomOptionActualDelayPercent, sienaExtraTotal } from "../../../equipment";
  import { app } from "../../../state.svelte";
  import { flash } from "../../../ui/motion.svelte";
  import type { SourceId } from "../sourceId";
  import ExternalSourceList, { type ExternalSource } from "./ExternalSourceList.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, onOpenSource }: Props = $props();

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
        (e) => e !== "record_only" && "actual_delay" in e,
      ),
    ),
  );
  /** このキャラのスキルぶんの中ディレイ減少 %(共通の供給源は含まない) */
  const delaySkillPercent = $derived(
    actualDelayPercent(
      draft.statSources.character_skills.skill_ids,
      app.characterSkills,
      draft.statSources.masteries.picked,
    ),
  );
  /** マスタリーぶんの中ディレイ減少 % */
  const masteryDelayPercent = $derived.by(() => {
    let sum = 0;
    for (const id of draft.statSources.masteries.picked) {
      const e = app.masteries.find((m) => m.id === id)?.effect;
      if (e !== undefined && e !== "record_only" && "actual_delay" in e) sum += e.actual_delay.percent;
    }
    return sum;
  });
  /** フルスロットル(共通スキル)の中ディレイ減少 %。0 = 未装着(計算は Rust 側) */
  const fullThrottlePercent = $derived(
    Math.round((preview?.common_skill.ultimate.actual_delay_reduction ?? 0) * 100),
  );

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
      value: randomOptionActualDelayPercent(draft.equipment, app.randomOptions),
      format: (v) => `−${v}%`,
    },
    { id: "siena", name: "シエナのオーラ", value: sienaExtraTotal(draft.equipment, "actual_delay"), format: (v) => `−${v}%` },
  ]);
</script>

<div class="card">
  <p class="hint dim">
    wiki「ステータス」の<b>中ディレイ倍率B</b>。中ディレイは
    <b>基本中ディレイ × (1 − 減少値) ×(2 コンボ以上なら 0.5)</b>で、下限 0.3s・減少値の上限 70%。
    ここで選ぶのは<b>このキャラのスキル</b>だけです
    (マスタリーは段ごとに 1 つで中ディレイ以外にも効くので、キャラスキルの欄にまとめてあります)。
    中ディレイと 1 秒あたりの火力は計算タブに出ます。
  </p>
  <div class="buff-list">
    {#if delaySkills.length === 0}
      <p class="empty dim">このキャラには中ディレイ減少のスキルがありません(wiki の表に記載なし)。</p>
    {/if}
    {#each delaySkills as def (def.id)}
      {@const label = effectLabel(def, draft.statSources.masteries.picked)}
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
