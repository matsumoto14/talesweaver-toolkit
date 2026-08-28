<script lang="ts">
  // 「skills」補正源のペイン。マスタリー(段ごとに 1 つ)と、自分・味方のスキル。
  import type { MasteryDef } from "../../../api/types";
  import { allySkills, damageCategoryLabel, effectLabel, ownSkills, toggleCharacterSkill } from "../../../characterSkills";
  import type { Draft } from "../../../draft";
  import { STAT_LABELS } from "../../../labels";
  import { app } from "../../../state.svelte";
  import { flash } from "../../../ui/motion.svelte";
  import Icon from "../../../ui/Icon.svelte";

  interface Props {
    draft: Draft;
  }
  let { draft }: Props = $props();

  // --- キャラスキル(wiki: 各キャラの Skill ページ / ステータスの各カテゴリ表)-------
  // カタログはキャラを問わず全件持っているので、このキャラのぶんだけ出す。
  // 味方から受けるスキルは誰でも ON にできる。
  const ownCharacterSkills = $derived(ownSkills(app.characterSkills, draft.gameCharacterId));
  const allyCharacterSkills = $derived(allySkills(app.characterSkills));
  const skillChecked = (id: string) => draft.statSources.character_skills.skill_ids.includes(id);
  function toggleCharSkill(id: string, on: boolean) {
    draft.statSources.character_skills.skill_ids = toggleCharacterSkill(
      draft.statSources.character_skills.skill_ids,
      id,
      on,
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
  const masteryEffectLabel = (m: MasteryDef): string => {
    const e = m.effect;
    if (e === "record_only") return m.note.split(" — ")[0];
    if ("stat_rate" in e) {
      return `${e.stat_rate.stats.map((k) => STAT_LABELS[k]).join(" / ")} +${e.stat_rate.percent}%`;
    }
    if ("actual_delay" in e) return `中ディレイ −${e.actual_delay.percent}%`;
    const sign = e.damage.percent < 0 ? "−" : "+";
    return `${damageCategoryLabel(e.damage.category)} ${sign}${Math.abs(e.damage.percent)}%`;
  };
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
  <!-- 自分のスキルはバフのチップではなく、マスタリーと同じ「アイコン + 名前 + 効果」の
       スキルカードで出す。選択状態は面の色だけにせず、固定幅の状態バッジでも言い切る。 -->
  <div class="character-skill-grid">
    {#if ownCharacterSkills.length === 0}
      <p class="empty dim">このキャラのスキルデータは未収録です。</p>
    {/if}
    {#each ownCharacterSkills as def (def.id)}
      {@const label = effectLabel(def, draft.statSources.masteries.picked)}
      {@const checked = skillChecked(def.id)}
      <label class="character-skill-card" class:on={checked} title={def.note}>
        <input
          type="checkbox"
          checked={checked}
          onchange={(e) => toggleCharSkill(def.id, e.currentTarget.checked)}
        />
        <Icon kind="skill" id={def.id} size={28} label={def.name} />
        <span class="character-skill-text">
          <span class="character-skill-head">
            <span class="character-skill-name">{def.name}</span>
            <span
              class="character-skill-state"
              class:on={checked}
              use:flash={() => checked ? "適用中" : "未適用"}
            >{checked ? "適用中" : "未適用"}</span>
          </span>
          <span
            class="character-skill-effect num"
            class:unknown={label === null}
            use:flash={() => label ?? ""}
          >{label ?? "マスタリー未取得"}</span>
          {#if def.note}<span class="character-skill-note dim">{def.note}</span>{/if}
        </span>
      </label>
    {/each}
  </div>
  <div class="card-title space">味方から受けるスキル</div>
  <div class="buff-list">
    {#if allyCharacterSkills.length === 0}
      <p class="empty dim">味方から受けるスキルデータは未収録です。</p>
    {/if}
    {#each allyCharacterSkills as def (def.id)}
      {@const label = effectLabel(def, draft.statSources.masteries.picked)}
      {@const sourceCharacter = app.gameCharacters.find((c) => c.id === def.game_character_id)}
      <label class="check">
        <input
          type="checkbox"
          checked={skillChecked(def.id)}
          onchange={(e) => toggleCharSkill(def.id, e.currentTarget.checked)}
        />
        <Icon
          kind="character"
          id={def.game_character_id}
          size={20}
          label={sourceCharacter?.name ?? def.game_character_id}
        />
        <span>{def.name}</span>
        <span class="fixed-value dim">{label ?? "—"}</span>
        {#if def.note}<span class="dim note">{def.note}</span>{/if}
      </label>
    {/each}
  </div>
</div>
