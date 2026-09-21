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
  import type { CharacterSkillEffectsView, PetSkillTier, Skill, StatKind, StatPreview } from "../../api/types";
  import type { Draft } from "../../draft";
  import { sacredRelicStageFromValue, sacredRelicValue, totalWithEnchant } from "../../equipment";
  import { limits } from "../../limits.svelte";
  import { tables } from "../../tables.svelte";
  import Chip from "../../ui/Chip.svelte";
  import Icon from "../../ui/Icon.svelte";
  import type { SourceId } from "./sourceId";
  import "./sources/pane-shared.css";
  import ActualDelayPane from "./sources/ActualDelayPane.svelte";
  import AvatarPane from "./sources/AvatarPane.svelte";
  import PolishPane from "./sources/PolishPane.svelte";
  import CharacterSkillPane from "./sources/CharacterSkillPane.svelte";
  import CommonSkillPane from "./sources/CommonSkillPane.svelte";
  import CriticalRatePane from "./sources/CriticalRatePane.svelte";
  import ElementPane from "./sources/ElementPane.svelte";
  import LuminaCorridorPane from "./sources/LuminaCorridorPane.svelte";
  import EquipmentPane from "./sources/EquipmentPane.svelte";
  import RandomOptionPane from "./sources/RandomOptionPane.svelte";
  import SienaPane from "./sources/SienaPane.svelte";
  import SoulLinkPane from "./sources/SoulLinkPane.svelte";
  import StatRows from "./sources/StatRows.svelte";
  import StatusPane from "./sources/StatusPane.svelte";
  import ThesisCorePane from "./sources/ThesisCorePane.svelte";
  import TitlePane from "./sources/TitlePane.svelte";
  import NumberField from "../../ui/NumberField.svelte";
  import Choose from "../../ui/Choose.svelte";
  import { fmtSigned } from "../../format";
  import { EQUIPMENT_STAT_SHORT, PET_SKILL_TIER_LABELS, STAT_KINDS, STAT_LABELS } from "../../labels";
  import Value from "../../ui/Value.svelte";
  import { equipmentAttackKindsFor } from "./summaries";

  /** 2 列のステ入力は、ゲーム内で対応を見る組み合わせを同じ段に置く。 */
  const PAIRED_STAT_KINDS: StatKind[] = ["stab", "def", "hack", "dex", "int", "agi", "mr"];

  interface Props {
    characterId: number;
    draft: Draft;
    preview: StatPreview | null;
    previewError: string | null;
    /** 主軸スキルの選択肢(キャラ種のスキル一覧)。親が引く */
    skills: Skill[];
    /** キャラスキル全件ぶんの、選んでいるマスタリーを踏まえた実際の効果(マスタリー解決は Rust 側) */
    resolvedSkillEffects: CharacterSkillEffectsView[];
    sourceId: SourceId;
    /** ほかの補正源へ飛ぶ(この値がどこから来ているかを追えるようにする) */
    onOpenSource: (id: SourceId) => void;
  }
  let { characterId, draft, preview, previewError, skills, resolvedSkillEffects, sourceId, onOpenSource }: Props = $props();

  // --- ペット S スキル ------------------------------------------------------
  // 段の名前だけだと「それでいくつ増えるのか」を毎回引くことになるので、値を段に書く
  // (正は crates/domain/src/stat_sources.rs の PetSkillTier::bonus。tables.pet_skill_tier_bonus 経由で引く)
  const petSkillBonusOf = (tier: PetSkillTier) =>
    tables.pet_skill_tier_bonus.find((b) => b.tier === tier)?.bonus ?? 0;
  const petSkillOptions = $derived([
    { value: "", label: "なし" },
    ...tables.pet_skill_tier_bonus.map((b) => ({
      value: b.tier,
      label: `${PET_SKILL_TIER_LABELS[b.tier]} ${fmtSigned(b.bonus)}`,
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
  /** よく使う値。上限が 280 と同じときは 1 つにまとめる(同じ的を 2 つ並べない) */
  const crownPresetOptions = $derived(
    [...new Set([260, 280, limits.crown_selected_max])].map((v) => ({
      value: String(v),
      label: v === limits.crown_selected_max ? "MAX" : String(v),
    })),
  );
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

  /**
   * 装備ペインの見出しに出す要約。主軸スキルが実際に使う補正について(魔法斬りなら 斬り・魔攻)、
   * **合計とその横に括弧でエンチャント分**を出す(equipment.ts の `withEnchant`。
   * ユーザー確定 2026-09-01)。テシスコアはテシスコアのペインが持つ — ここは装備だけを言う。
   */
  const equipmentMainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);
  const equipmentHeadNote = $derived.by(() => {
    // 計算前は 0 が並ぶだけなので、値が来るまでは元の説明文を出す(§00「0 で埋めない」)
    if (!preview) return TITLES.equipment.note;
    // ゲーム内の装備欄と見比べる数なので、ゲーム内に出ないソウルリンクだけを除いた合計を出す
    // (シエナのオーラ・アバター強化・称号はゲーム内でも乗る)。除いた分は横の注記が言う
    const total = preview.equipment_ingame_total;
    const enchant = preview.equipment_enhanced_total;
    return (
      equipmentAttackKindsFor(equipmentMainSkill?.dependency ?? null)
        .map((k) => `${EQUIPMENT_STAT_SHORT[k]} ${totalWithEnchant(total[k], enchant[k])}`)
        .join(" ・ ") || TITLES.equipment.note
    );
  });

  /** 見出しの合計から除いたソウルリンク分。0 なら除いたものが無いので注記ごと出さない */
  const equipmentSoulLinkNote = $derived.by(() => {
    if (!preview) return "";
    const excluded = equipmentAttackKindsFor(equipmentMainSkill?.dependency ?? null)
      .filter((k) => preview.soul_link.equipment_values[k] !== 0)
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]} ${fmtSigned(preview.soul_link.equipment_values[k])}`)
      .join(" ・ ");
    return excluded ? `ゲーム内の表示と同じ ・ ソウルリンク(${excluded})は含まない` : "";
  });

  const TITLES: Record<SourceId,{ title: string; note: string }> = {
    status: { title: "キャラステータス", note: "素ステ・覚醒・主軸スキル" },
    element: { title: "属性", note: "主属性と、装備から自動で入る属性値" },
    lumina: { title: "ルミナの回廊", note: "回廊効果(テイルズID 内の全キャラに効く恒常バフ)" },
    equipment: { title: "装備", note: "部位ごとのアイテム・エンチャント・強化" },
    soulLink: { title: "ソウルリンク", note: "全項目を計算に反映" },
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
    title: { title: "称号", note: "表示中の 1 件だけ有効" },
    commonSkill: { title: "共通スキル", note: "キャラ横断のパッシブ(オーグメントが Lv の前提)" },
    thesis: { title: "テシスコア", note: "地域ごとに 6 枠(能力値は対象地域内のみ有効)" },
    avatar: { title: "アバター", note: "兜・頭・体・脚・エフェクトの5部位。強化剤と補正付きアバター(末尾「Ａ」)" },
    polish: { title: "研磨", note: "部位ごとに能力値1つを研磨剤/ワックスで上げる" },
    skills: { title: "キャラスキル", note: "マスタリー(段ごとに 1 つ)と、自分・味方のスキル" },
    actualDelay: { title: "中ディレイ減少", note: "このキャラ固有のパッシブ・マスタリー(倍率B)" },
    criticalRate: { title: "クリティカル率", note: `ペット会心と増加(上限 ${fmtSigned(limits.critical_rate_bonus_max, { max: 2 }, "%")})` },
  };
</script>

{#key sourceId}
<div class="pane pane-in">
  <div class="pane-head">
    <Icon kind="source" id={sourceId} size={28} label={TITLES[sourceId].title} />
    <span class="pane-title">{TITLES[sourceId].title}</span>
    <!-- 装備だけは固定の説明文ではなく、いまの値を出す。装備は**強化合計(エンチャント)**で
         概ね認知できる(ユーザー判断 2026-09-01)ので、主軸スキルが使う補正だけを見出しに置き、
         ペインの中には表を持たない(縦を使わない) -->
    {#if sourceId === "equipment"}
      {#if equipmentSoulLinkNote}<span class="dim">{equipmentSoulLinkNote}</span>{/if}
      <Value class="head-value" value={equipmentHeadNote} />
    {:else}
      <span class="dim">{TITLES[sourceId].note}</span>
    {/if}
  </div>

  {#if previewError}<p class="preview-error">{previewError}</p>{/if}

  {#if sourceId === "status"}
    <StatusPane {characterId} {draft} {preview} {skills} />
  {:else if sourceId === "lumina"}
    <LuminaCorridorPane {draft} {preview} />
  {:else if sourceId === "element"}
    <ElementPane {draft} {preview} {skills} {onOpenSource} />
  {:else if sourceId === "equipment"}
    <EquipmentPane {draft} {preview} {skills} />
  {:else if sourceId === "soulLink"}
    <SoulLinkPane {draft} {preview} />
  {:else if sourceId === "pet"}
    <div class="card">
      <!-- 8 ステが同じ形で並ぶので 1 ステ 1 行。段は列を固定して行をまたいで揃える(§00 01) -->
      {#snippet petRow(k: StatKind)}
        <Choose
          label="{STAT_LABELS[k]}のペット S スキル"
          options={petSkillOptions}
          cols={petSkillOptions.length}
          bind:value={() => petSkillValue(k), (v) => setPetSkillValue(k, v)}
        />
        <Value class="v" motion={() => petSkillBonus(k)} value={petSkillBonus(k) > 0 ? fmtSigned(petSkillBonus(k)) : "—"} />
      {/snippet}
      <StatRows kinds={STAT_KINDS} row={petRow} />
    </div>
  {:else if sourceId === "rune"}
    <div class="card">
      {#snippet runeRow(k: StatKind)}
        <NumberField
          label="{STAT_LABELS[k]}のルーンスキル Lv"
          max={limits.rune_level_max}
          bind:value={draft.statSources.rune_levels[k]}
        />
      {/snippet}
      <StatRows kinds={PAIRED_STAT_KINDS} twoCol row={runeRow} />
    </div>
  {:else if sourceId === "crown"}
    <div class="card">
      <div class="crown-choice">
        <span class="crown-choice-label">選択報酬</span>
        <!-- 選んだ 1 つを押すと外せる(値は checkbox 群。radio は押し直しで外せない) -->
        <Choose
          label="選択報酬の能力値"
          class="chiprow crown-choice-stats"
          options={PAIRED_STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }))}
          values={draft.statSources.crown.selected_stat === null ? [] : [draft.statSources.crown.selected_stat]}
          onToggle={(v) => toggleCrownSelectedStat(v as StatKind)}
        />
        <div class="crown-presets" aria-label="選択報酬のよく使う値">
          <Chip
            class="num"
            disabled={draft.statSources.crown.selected_stat === null ||
              crownSelectedValue() === limits.crown_selected_max}
            onclick={() => addCrownSelected(20)}
          >+20</Chip>
          <Choose
            label="選択報酬のよく使う値"
            class="chiprow"
            options={crownPresetOptions}
            disabled={draft.statSources.crown.selected_stat === null}
            tone={() => "num"}
            bind:value={() => String(crownSelectedValue() ?? ""), (v) => setCrownPreset(Number(v))}
          />
        </div>
        <span class="hint dim">選んだ能力値だけ上限 {fmtSigned(limits.crown_selected_max)}。もう一度押すと外せます。</span>
      </div>
      {#snippet crownRow(k: StatKind)}
        <NumberField
          label="{STAT_LABELS[k]}の王冠"
          max={crownMax(k)}
          step={limits.crown_step}
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
        <NumberField
          label="{STAT_LABELS[k]}のカード装着"
          max={limits.monster_card_max}
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
        <NumberField
          label="{STAT_LABELS[k]}の神鳥の聖物"
          max={limits.sacred_relic_stage_max * limits.sacred_relic_value_per_stage}
          step={limits.sacred_relic_value_per_stage}
          presets={[{ value: 200, label: "200" }]}
          bind:value={
            () => sacredRelicValue(draft.statSources.sacred_relic[k]),
            (v) => (draft.statSources.sacred_relic[k] = sacredRelicStageFromValue(v))
          }
        />
      {/snippet}
      <StatRows kinds={PAIRED_STAT_KINDS} twoCol row={relicRow} />
    </div>
  {:else if sourceId === "siena"}
    <SienaPane {draft} {preview} />
  {:else if sourceId === "randomOption"}
    <RandomOptionPane {draft} {preview} {onOpenSource} />
  {:else if sourceId === "title"}
    <TitlePane {draft} {skills} />
  {:else if sourceId === "commonSkill"}
    <CommonSkillPane {draft} {preview} />
  {:else if sourceId === "thesis"}
    <ThesisCorePane {draft} {preview} />
  {:else if sourceId === "avatar"}
    <AvatarPane {draft} dependency={equipmentMainSkill?.dependency ?? null} />
  {:else if sourceId === "polish"}
    <PolishPane {draft} {preview} dependency={equipmentMainSkill?.dependency ?? null} />
  {:else if sourceId === "actualDelay"}
    <ActualDelayPane {draft} {preview} {resolvedSkillEffects} {onOpenSource} />
  {:else if sourceId === "criticalRate"}
    <CriticalRatePane {draft} {preview} {skills} {onOpenSource} />
  {:else if sourceId === "skills"}
    <CharacterSkillPane {draft} {resolvedSkillEffects} />
  {/if}
</div>
{/key}
