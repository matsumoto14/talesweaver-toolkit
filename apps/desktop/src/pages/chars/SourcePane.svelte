<script lang="ts" module>
  export type { SourceId } from "./sourceId";
</script>

<script lang="ts">
  // 選択した補正源の編集ペイン。draft($state プロキシ)のネストしたプロパティを直接書き換える。
  // 専門用語(層名など)は「補正の内訳」以外に出さない(既存決定を踏襲)。
  //
  // このファイルは「ペインの枠(head/戻る)+ sourceId → 子ペインへのディスパッチ + ペイン横断の
  // 共有状態」だけを持つ。各補正源固有の状態・markup・style は pages/chars/sources/*.svelte へ
  // 分割してある。ペインをまたいで使う CSS クラス(.stat-row / .check / .part-row など)は
  // sources/pane-shared.css にまとめてグローバル読み込みしている(理由はそのファイル冒頭を参照)。
  import type { PetSkillTier, Skill, StatKind, StatPreview } from "../../api/types";
  import type { Draft } from "../../draft";
  import { limits } from "../../limits.svelte";
  import type { SourceId } from "./sourceId";
  import "./sources/pane-shared.css";
  import ActualDelayPane from "./sources/ActualDelayPane.svelte";
  import CharacterSkillPane from "./sources/CharacterSkillPane.svelte";
  import CommonSkillPane from "./sources/CommonSkillPane.svelte";
  import CriticalRatePane from "./sources/CriticalRatePane.svelte";
  import EquipmentPane from "./sources/EquipmentPane.svelte";
  import RandomOptionPane from "./sources/RandomOptionPane.svelte";
  import SienaPane from "./sources/SienaPane.svelte";
  import StatRows from "./sources/StatRows.svelte";
  import StatusPane from "./sources/StatusPane.svelte";
  import ThesisCorePane from "./sources/ThesisCorePane.svelte";
  import TitlePane from "./sources/TitlePane.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import { fmtInt } from "../../format";
  import { PET_SKILL_TIER_LABELS, STAT_KINDS, STAT_LABELS } from "../../labels";
  import { bump } from "../../ui/motion.svelte";

  /** 2 列のステ入力は、ゲーム内で対応を見る組み合わせを同じ段に置く。 */
  const PAIRED_STAT_KINDS: StatKind[] = ["stab", "def", "hack", "dex", "int", "agi", "mr"];

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    previewError: string | null;
    /** 主軸スキルの選択肢(キャラ種のスキル一覧)。親が引く */
    skills: Skill[];
    sourceId: SourceId;
    /** ほかの補正源へ飛ぶ(この値がどこから来ているかを追えるようにする) */
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, previewError, skills, sourceId, onOpenSource }: Props = $props();

  // --- ペット S スキル ------------------------------------------------------
  // 段の名前だけだと「それでいくつ増えるのか」を毎回引くことになるので、値を段に書く
  // (正は crates/domain/src/stat_sources.rs の PetSkillTier::bonus。limits.pet_skill_tier_bonus 経由で引く)
  const petSkillBonusOf = (tier: PetSkillTier) =>
    limits.pet_skill_tier_bonus.find((b) => b.tier === tier)?.bonus ?? 0;
  const petSkillOptions = $derived([
    { value: "", label: "なし" },
    ...limits.pet_skill_tier_bonus.map((b) => ({
      value: b.tier,
      label: `${PET_SKILL_TIER_LABELS[b.tier]} +${b.bonus}`,
    })),
  ]);
  const petSkillValue = (k: StatKind) => draft.statSources.pet_skills[k] ?? "";
  const petSkillBonus = (k: StatKind) => {
    const tier = draft.statSources.pet_skills[k];
    return tier ? petSkillBonusOf(tier) : 0;
  };
  const setPetSkillValue = (k: StatKind, v: string) => {
    draft.statSources.pet_skills[k] = (v === "" ? null : v) as PetSkillTier | null;
  };

  // --- クラウン --------------------------------------------------------------
  const crownMax = (kind: StatKind): number =>
    draft.statSources.crown.selected_stat === kind
      ? limits.crown_selected_max
      : limits.crown_base_max;
  const crownSelectedValue = (): number | null => {
    const kind = draft.statSources.crown.selected_stat;
    return kind === null ? null : draft.statSources.crown[kind];
  };
  function toggleCrownSelectedStat(kind: StatKind) {
    const current = draft.statSources.crown.selected_stat;
    const next = current === kind ? null : kind;
    if (current !== null && current !== next) {
      draft.statSources.crown[current] = Math.min(
        draft.statSources.crown[current],
        limits.crown_base_max,
      );
    }
    draft.statSources.crown.selected_stat = next;
  }
  function setCrownPreset(value: number) {
    const kind = draft.statSources.crown.selected_stat;
    if (kind === null) return;
    draft.statSources.crown[kind] = Math.min(value, crownMax(kind));
  }
  function addCrownSelected(amount: number) {
    const kind = draft.statSources.crown.selected_stat;
    if (kind === null) return;
    draft.statSources.crown[kind] = Math.min(
      crownMax(kind),
      draft.statSources.crown[kind] + amount,
    );
  }

  const TITLES: Record<SourceId, { title: string; note: string }> = {
    status: { title: "キャラステータス", note: "素ステ・覚醒・エタの意志・主属性" },
    equipment: { title: "装備", note: "部位ごとのアイテム・エンチャント・強化" },
    pet: { title: "ペット S スキル", note: "ステごとに 1 段階" },
    rune: { title: "ルーンスキル", note: `スキル Lv がそのままステに乗る(Lv 0–${limits.rune_level_max})` },
    crown: {
      title: "クラウン",
      note: `10 きざみ・通常上限 ${limits.crown_base_max} / 選択報酬は ${limits.crown_selected_max}`,
    },
    monsterCard: { title: "モンスターカード", note: `装着カードのステータス(0–${limits.monster_card_max})` },
    relic: {
      title: "神鳥の聖物",
      note: `ステごとの加算(${limits.sacred_relic_value_per_stage} きざみ・0–${limits.sacred_relic_stage_max * limits.sacred_relic_value_per_stage})`,
    },
    siena: { title: "シエナのオーラ", note: "部位ごとに登録し、装着中の 1 件だけが反映" },
    randomOption: { title: "ランダムOP", note: "部位ごとの追加効果(同じカテゴリーは 1 部位 1 つ)" },
    title: { title: "称号", note: "表示中の 1 件だけが装備の基本能力値に乗る" },
    commonSkill: { title: "共通スキル", note: "キャラ横断のパッシブ(オーグメントが Lv の前提)" },
    thesis: { title: "テシスコア", note: "地域ごとに 6 枠(能力値は対象地域内のみ有効)" },
    skills: { title: "キャラスキル", note: "マスタリー(段ごとに 1 つ)と、自分・味方のスキル" },
    actualDelay: { title: "中ディレイ減少", note: "このキャラ固有のパッシブ・マスタリー(倍率B)" },
    criticalRate: { title: "クリティカル率", note: `ペット会心と増加(上限 +${limits.critical_rate_bonus_max}%)` },
    adjust: { title: "調整", note: "検証・仮定用の例外操作" },
  };
</script>

{#key sourceId}
<div class="pane pane-in">
  <div class="pane-head">
    <span class="pane-title">{TITLES[sourceId].title}</span>
    <span class="dim">{TITLES[sourceId].note}</span>
  </div>

  {#if previewError}<p class="preview-error">{previewError}</p>{/if}

  {#if sourceId === "status"}
    <StatusPane {draft} {preview} {skills} />
  {:else if sourceId === "equipment"}
    <EquipmentPane {draft} {preview} {skills} />
  {:else if sourceId === "pet"}
    <div class="card">
      <!-- 8 ステが同じ形で並ぶので 1 ステ 1 行。段は列を固定して行をまたいで揃える(§00 01) -->
      {#snippet petRow(k: StatKind)}
        <StepSelect
          label=""
          options={petSkillOptions}
          cols={petSkillOptions.length}
          bind:value={() => petSkillValue(k), (v) => setPetSkillValue(k, v)}
        />
        <span class="v num" use:bump={() => petSkillBonus(k)}>
          {petSkillBonus(k) > 0 ? `+${fmtInt(petSkillBonus(k))}` : "—"}
        </span>
      {/snippet}
      <StatRows kinds={STAT_KINDS} row={petRow} />
    </div>
  {:else if sourceId === "rune"}
    <div class="card">
      {#snippet runeRow(k: StatKind)}
        <!-- Lv は段階。1 押しに意味があるので ＋ / − を置く(§07 形態 4) -->
        <StatInput
          label=""
          min={0}
          max={limits.rune_level_max}
          stepper
          bind:value={draft.statSources.rune_levels[k]}
        />
      {/snippet}
      <StatRows kinds={PAIRED_STAT_KINDS} twoCol row={runeRow} />
    </div>
  {:else if sourceId === "crown"}
    <div class="card">
      <div class="crown-choice">
        <span class="crown-choice-label">選択報酬</span>
        <div class="crown-choice-stats" role="radiogroup" aria-label="クラウンの選択報酬">
          {#each PAIRED_STAT_KINDS as k (k)}
            <button
              type="button"
              class="chip"
              class:on={draft.statSources.crown.selected_stat === k}
              role="radio"
              aria-checked={draft.statSources.crown.selected_stat === k}
              onclick={() => toggleCrownSelectedStat(k)}
            >{STAT_LABELS[k]}</button>
          {/each}
        </div>
        <div class="crown-presets" aria-label="選択報酬のよく使う値">
          <button
            type="button"
            class="chip num"
            disabled={draft.statSources.crown.selected_stat === null ||
              crownSelectedValue() === limits.crown_selected_max}
            onclick={() => addCrownSelected(20)}
          >+20</button>
          {#each [260, 280] as value (value)}
            <button
              type="button"
              class="chip num"
              class:on={crownSelectedValue() === value}
              disabled={draft.statSources.crown.selected_stat === null}
              onclick={() => setCrownPreset(value)}
            >{value}</button>
          {/each}
          <button
            type="button"
            class="chip num"
            class:on={crownSelectedValue() === limits.crown_selected_max}
            disabled={draft.statSources.crown.selected_stat === null}
            onclick={() => setCrownPreset(limits.crown_selected_max)}
          >MAX</button>
        </div>
        <span class="hint dim">選んだ能力値だけ上限 +{limits.crown_selected_max}。もう一度押すと外せます。</span>
      </div>
      {#snippet crownRow(k: StatKind)}
        <StatInput
          label=""
          min={0}
          max={crownMax(k)}
          step={limits.crown_step}
          stepper
          bind:value={draft.statSources.crown[k]}
        />
      {/snippet}
      <StatRows kinds={PAIRED_STAT_KINDS} twoCol flashValue={crownMax} row={crownRow} />
    </div>
  {:else if sourceId === "monsterCard"}
    <div class="card">
      <p class="hint dim">
        wiki「ステータス」の固定値増加にある<b>カード装着</b>。装着したカードのステータスが
        そのまま乗ります(ステごと 0〜{limits.monster_card_max})。
        <b>固定値層</b>なので、能力値倍率A(テイルズウィーバーのエネルギー等)の影響を受けます。
      </p>
      {#snippet monsterCardRow(k: StatKind)}
        <StatInput
          label=""
          min={0}
          max={limits.monster_card_max}
          stepper
          bind:value={draft.statSources.monster_cards[k]}
        />
      {/snippet}
      <StatRows kinds={PAIRED_STAT_KINDS} twoCol row={monsterCardRow} />
    </div>
  {:else if sourceId === "relic"}
    <div class="card">
      {#snippet relicRow(k: StatKind)}
        <!-- 段階ではなく**実際に増える値**で入れる(1 段階 = +{limits.sacred_relic_value_per_stage} なので
             ＋ を押すとその値ずつ)。多くの人は 200 で止まるので、そこを 1 押しで置く。保存は段階のまま -->
        <StatInput
          label=""
          min={0}
          max={limits.sacred_relic_stage_max * limits.sacred_relic_value_per_stage}
          step={limits.sacred_relic_value_per_stage}
          stepper
          presets={[{ value: 200, label: "200" }]}
          bind:value={
            () => draft.statSources.sacred_relic[k] * limits.sacred_relic_value_per_stage,
            (v) => (draft.statSources.sacred_relic[k] = Math.round(v / limits.sacred_relic_value_per_stage))
          }
        />
      {/snippet}
      <StatRows kinds={PAIRED_STAT_KINDS} twoCol row={relicRow} />
    </div>
  {:else if sourceId === "siena"}
    <SienaPane {draft} {preview} />
  {:else if sourceId === "randomOption"}
    <RandomOptionPane {draft} {skills} {onOpenSource} />
  {:else if sourceId === "title"}
    <TitlePane {draft} />
  {:else if sourceId === "commonSkill"}
    <CommonSkillPane {draft} {preview} />
  {:else if sourceId === "thesis"}
    <ThesisCorePane {draft} {preview} />
  {:else if sourceId === "actualDelay"}
    <ActualDelayPane {draft} {preview} {onOpenSource} />
  {:else if sourceId === "criticalRate"}
    <CriticalRatePane {draft} {preview} {skills} {onOpenSource} />
  {:else if sourceId === "skills"}
    <CharacterSkillPane {draft} />
  {:else if sourceId === "adjust"}
    <div class="card">
      <p class="hint dim">「このステに +N」「最終能力値を N に固定」の 2 種。検証・未収録データ用の例外操作です。</p>
      <AdjustmentEditor
        adjustments={draft.statSources.adjustments}
        addMin={limits.adjustment_add_min}
        addMax={limits.adjustment_add_max}
        pinMin={limits.adjustment_pin_min}
        pinMax={limits.adjustment_pin_max}
        pinDefault={(k) => preview?.stats[k] ?? draft.baseStats[k]}
      />
    </div>
  {/if}
</div>
{/key}
