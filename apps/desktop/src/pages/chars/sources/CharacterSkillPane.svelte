<script lang="ts">
  // 「skills」補正源のペイン。マスタリー(段ごとに 1 つ)と、自分・味方のスキル。
  import type { CharacterSkillEffectsView, MasteryDef } from "../../../api/types";
  import {
    allySkills, effectLabel, enemySkills, ownSkills, resolvedEffectsOf, singleEffectLabel, toggleCharacterSkill,
  } from "../../../characterSkills";
  import type { Draft } from "../../../draft";
  import { app } from "../../../state.svelte";
  import Icon from "../../../ui/Icon.svelte";
  import ToggleRow from "../../../ui/ToggleRow.svelte";

  interface Props {
    draft: Draft;
    resolvedSkillEffects: CharacterSkillEffectsView[];
  }
  let { draft, resolvedSkillEffects }: Props = $props();

  // --- キャラスキル(wiki: 各キャラの Skill ページ / ステータスの各カテゴリ表)-------
  // カタログはキャラを問わず全件持っているので、このキャラのぶんだけ出す。
  // 味方から受けるスキルは誰でも ON にできる。
  const ownCharacterSkills = $derived(ownSkills(app.characterSkills, draft.gameCharacterId));
  const allyCharacterSkills = $derived(allySkills(app.characterSkills));
  const enemyCharacterSkills = $derived(enemySkills(app.characterSkills));
  const skillChecked = (id: string) => draft.statSources.character_skills.skill_ids.includes(id);
  function toggleCharSkill(id: string, on: boolean) {
    draft.statSources.character_skills.skill_ids = toggleCharacterSkill(
      draft.statSources.character_skills.skill_ids,
      id,
      on,
      app.characterSkills,
    );
  }

  // --- マスタリー(wiki: 各キャラの Skill ページ。段ごとに 1 つ)-----------
  // 同じ段の選択肢は効き先がばらばら(中ディレイ / カテゴリX / ステ / 未収録)なので、
  // カタログは 1 つにまとめて domain 側が効き先ごとに振り分ける。
  const masteryTiers = $derived.by(() => {
    const mine = app.masteries.filter((m) => m.game_character_id === draft.gameCharacterId);
    const tiers = [...new Set(mine.map((m) => m.tier))].sort((a, b) => a - b);
    return tiers.map((tier) => ({ tier, options: mine.filter((m) => m.tier === tier) }));
  });
  const pickedMastery = (tier: number) =>
    app.masteries.find(
      (m) => m.tier === tier && draft.statSources.masteries.picked.includes(m.id),
    ) ?? null;
  /** その段の選択を差し替える(段ごとに 1 つ。同じものを押したら外す) */
  function pickMastery(tier: number, id: string | null) {
    const others = draft.statSources.masteries.picked.filter(
      (picked) => app.masteries.find((m) => m.id === picked)?.tier !== tier,
    );
    draft.statSources.masteries.picked = id === null ? others : [...others, id];
  }
  /** 効き先の要約。記録のみは wiki の効果だけ出し、未収録の理由は title に回す
      (カードは 3 列なので、理由まで入れると行が伸びて段の高さがそろわない) */
  const masteryEffectLabel = (m: MasteryDef): string => singleEffectLabel(m.effect) ?? m.note.split(" — ")[0];
  const masteryIsModeled = (m: MasteryDef): boolean => m.effect !== "record_only";
</script>

<!-- マスタリーは**段ごとに 1 つ**(wiki: スキル表の (M1)〜(M4))。同じ段の選択肢は
     効き先がばらばら(中ディレイ / カテゴリX / ステ / 未収録)なので、
     チェックの列ではなく段の選択にする(§07 形態 2)。グレーは記録するだけ -->
<div class="card">
  <div class="card-title inline">
    マスタリー
    <span class="dim normal">段ごとに 1 つ ・ もう一度押すと外す</span>
  </div>
  {#if masteryTiers.length === 0}
    <p class="empty dim">このキャラのマスタリーは未収録です(wiki の Skill ページから取り込み予定)。</p>
  {:else}
    {#each masteryTiers as t (t.tier)}
      {@const picked = pickedMastery(t.tier)}
      <!-- どの段も 3 択。列をそろえて横に並べる(§00 01)。ゲームに「未取得」という
           選択肢は無いので出さず、選んだものをもう一度押して外す -->
      <div class="mastery-row">
        <span class="mastery-tier num">M{t.tier}</span>
        <div class="mastery-options">
          {#each t.options as m (m.id)}
            <button
              type="button"
              class="mastery-option"
              class:on={picked?.id === m.id}
              class:record-only={!masteryIsModeled(m)}
              title={m.note}
              onclick={() => pickMastery(t.tier, picked?.id === m.id ? null : m.id)}
            >
              <Icon kind="mastery" id={m.id} size={28} label={m.name} />
              <span class="mastery-text">
                <span class="mastery-name">{m.name}</span>
                <span class="mastery-effect num">{masteryEffectLabel(m)}</span>
              </span>
            </button>
          {/each}
        </div>
      </div>
    {/each}
  {/if}
</div>
<div class="card">
  <div class="card-title">このキャラのスキル</div>
  <p class="hint dim">
    スキルの効果は<b>取っているマスタリーで変わります</b>(wiki の各カテゴリ表がその形)。
    上のマスタリーを選び直すと、ここの値も一緒に動きます。
  </p>
  <!-- 自分のスキルは行チップ(ToggleRow)。ゲーム内と同じスキルアイコンを名前の左に置き、
       効果はマスタリー込みの実数を右端に出す。適用中かどうかは面の色だけで言う(§07) -->
  <div class="toggle-list">
    {#if ownCharacterSkills.length === 0}
      <p class="empty dim">このキャラのスキルデータは未収録です。</p>
    {/if}
    {#each ownCharacterSkills as def (def.id)}
      {@const label = effectLabel(resolvedEffectsOf(def.id, resolvedSkillEffects))}
      {@const checked = skillChecked(def.id)}
      <ToggleRow
        name={def.name}
        cond={def.note || undefined}
        value={label ?? "マスタリー未取得"}
        title={def.note || undefined}
        on={checked}
        onToggle={() => toggleCharSkill(def.id, !checked)}
      >
        {#snippet icon()}<Icon kind="skill" id={def.id} size={20} label={def.name} />{/snippet}
      </ToggleRow>
    {/each}
  </div>
  <div class="card-title space">味方から受けるスキル</div>
  <div class="toggle-list">
    {#if allyCharacterSkills.length === 0}
      <p class="empty dim">味方から受けるスキルデータは未収録です。</p>
    {/if}
    {#each allyCharacterSkills as def (def.id)}
      {@const label = effectLabel(resolvedEffectsOf(def.id, resolvedSkillEffects))}
      {@const sourceCharacter = app.gameCharacters.find((c) => c.id === def.game_character_id)}
      {@const checked = skillChecked(def.id)}
      <ToggleRow
        name={def.name}
        cond={def.note || undefined}
        value={label ?? "—"}
        title={def.note || undefined}
        on={checked}
        onToggle={() => toggleCharSkill(def.id, !checked)}
      >
        {#snippet icon()}
          <Icon kind="character" id={def.game_character_id} size={20} label={sourceCharacter?.name ?? def.game_character_id} />
        {/snippet}
      </ToggleRow>
    {/each}
  </div>
  <div class="card-title space">敵にかけるデバフ</div>
  <p class="hint dim">同行者が敵にかけている前提なので、誰でも ON にできます(敵の被ダメージが増える = 自分の火力が上がる)。</p>
  <div class="toggle-list">
    {#if enemyCharacterSkills.length === 0}
      <p class="empty dim">敵にかけるデバフのデータは未収録です。</p>
    {/if}
    {#each enemyCharacterSkills as def (def.id)}
      {@const label = effectLabel(resolvedEffectsOf(def.id, resolvedSkillEffects))}
      {@const sourceCharacter = app.gameCharacters.find((c) => c.id === def.game_character_id)}
      {@const checked = skillChecked(def.id)}
      <ToggleRow
        name={def.name}
        cond={def.note || undefined}
        value={label ?? "—"}
        title={def.note || undefined}
        on={checked}
        onToggle={() => toggleCharSkill(def.id, !checked)}
      >
        {#snippet icon()}
          <Icon kind="character" id={def.game_character_id} size={20} label={sourceCharacter?.name ?? def.game_character_id} />
        {/snippet}
      </ToggleRow>
    {/each}
  </div>
</div>
