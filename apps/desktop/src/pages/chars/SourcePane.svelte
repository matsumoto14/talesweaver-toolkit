<script lang="ts" module>
  export type SourceId =
    | "status"
    | "equipment"
    | "pet"
    | "rune"
    | "crown"
    | "monsterCard"
    | "relic"
    | "siena"
    | "randomOption"
    | "title"
    | "commonSkill"
    | "actualDelay"
    | "criticalRate"
    | "thesis"
    | "skills"
    | "adjust";
</script>

<script lang="ts">
  // 選択した補正源の編集ペイン。draft($state プロキシ)のネストしたプロパティを直接書き換える。
  // 専門用語(層名など)は「補正の内訳」以外に出さない(既存決定を踏襲)。
  import type {
    CoreRegion, CoreType, Element, ElementPreview, EquipmentAbilityAdditionalKind, EquipmentAbilityDef, EquipmentAbilityFamily, EquipmentItem, EquipmentPart, PartSlot,
    MasteryDef, PetSkillTier, RandomOptionDef, RandomOptionRank, SienaAuraList, SienaExtraKind, SienaValueKind,
    Skill, SkillDependency, WeaponClass, WeaponSystem, WristType,
    StatKind, StatPreview, TitleDef, UltimateSkill,
  } from "../../api/types";
  import { isFixedValue, toggleBuff } from "../../buffs";
  import {
    actualDelayPercent, allySkills, damageCategoryLabel, effectLabel, ownSkills,
    toggleCharacterSkill,
  } from "../../characterSkills";
  import { previewElements } from "../../api/commands";
  import { draftToPayload, type Draft } from "../../draft";
  import {
    clampToCaps, midpointValues,
    neutralEquipmentPart, neutralSienaAura, randomOptionEffectLabel, randomOptionIsApplied,
    randomOptionActualDelayPercent, randomOptionValue, randomOptionValueLabel, rangeSummary,
    sienaExtraCapacity, sienaExtraTotal, sienaExtraValue,
    selectedSienaAura, selectedSienaAuraRegistration,
    sienaPartStatTotal, sienaStage, sumValues, valuesSummary, zeroValues,
  } from "../../equipment";
  import { fmtInt, formatLayerValue } from "../../format";
  import {
    ABILITY_ALLOWED_SLOTS, CORE_POWER_TYPES, CORE_REGION_LABELS, CORE_REGIONS, CORE_SLOT_COUNT,
    CORE_SUPPORT_TYPES, CORE_TYPE_LABELS, ELEMENT_ALLOWED_SLOTS, ELEMENT_LABELS, ELEMENTS,
    ENHANCE_ALLOWED_SLOTS,
    EQUIPMENT_ELEMENTS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT,
    PART_SLOT_LABELS, PART_SLOTS, PET_SKILL_TIER_LABELS,
    RANDOM_OPTION_ALLOWED_SLOTS, RANDOM_OPTION_RANK_LABELS, RANDOM_OPTION_RANKS,
    SIENA_ALLOWED_SLOTS,
    SIENA_EQUIPMENT_VALUE_SLOTS, STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS,
    ULTIMATE_SKILLS, ULTIMATE_SKILL_EFFECTS, ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import type { EquipmentStatKind, SienaPartSlot } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { app } from "../../state.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Picker, { type PickerOption } from "../../ui/Picker.svelte";
  import Select from "../../ui/Select.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import { ETERNAL_MILESTONES } from "../../draft";
  import StatInput from "../../ui/StatInput.svelte";
  import { slide } from "svelte/transition";

  /** 2 列のステ入力は、ゲーム内で対応を見る組み合わせを同じ段に置く。 */
  const PAIRED_STAT_KINDS: StatKind[] = ["stab", "def", "hack", "dex", "int", "agi", "mr"];
  /** 装備で日常的にエンチャントする4補正。ゲーム内の呼び方どおり S/H/I/M を先に並べる。 */
  const PRIMARY_EQUIPMENT_STATS: EquipmentStatKind[] = ["thrust", "slash", "magic_attack", "magic_defense"];
  const OTHER_EQUIPMENT_STATS: EquipmentStatKind[] = EQUIPMENT_STAT_KINDS.filter(
    (kind) => !PRIMARY_EQUIPMENT_STATS.includes(kind),
  );
  /** 通常エンチャントを持つ全部位。成長装備の盾+とレリックは別の入力モデル。 */
  const ENCHANT_PLAN_SLOTS = new Set<PartSlot>([
    "weapon", "armor", "helm", "shield", "head", "body", "hand", "leg", "effect", "artifact",
  ]);

  /** ほかの補正源から入ってくる分の 1 行。押すとその補正源へ飛ぶ */
  interface ExternalSource {
    id: SourceId;
    name: string;
    value: number;
    format: (value: number) => string;
    note?: string;
  }

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

  const STAT_MIN = 1;
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
  /** 中ディレイ減少を持つキャラスキル(中ディレイのペインに出す) */
  const delaySkills = $derived(
    ownCharacterSkills.filter((d) =>
      [...d.effects, ...d.mastery_overrides.flatMap((o) => o.effects)].some(
        (e) => e !== "record_only" && "actual_delay" in e,
      ),
    ),
  );
  /** 設計者の研究室ぶんのクリティカル率増加(研究段階 × 1 段階あたりの増加量) */
  const architectLabBonus = $derived(
    draft.statSources.critical_rate.architect_lab_stage * limits.architect_lab_per_stage,
  );
  /** 研究段階の選択肢。0〜10 段階を「N 段階(+3N%)」で並べる(§07 形態 2) */
  const architectLabOptions = $derived(
    Array.from({ length: limits.architect_lab_stage_max + 1 }, (_, i) => ({
      value: String(i),
      label: String(i),
    })),
  );
  /** クリティカル率増加の合計(上限を掛ける前。計算は Rust 側) */
  const criticalRateBonus = $derived(preview?.critical_rate_bonus.raw ?? 0);

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
  /** マスタリーぶんの中ディレイ減少 %(中ディレイペインの「ほかから入る分」に出す) */
  const masteryDelayPercent = $derived.by(() => {
    let sum = 0;
    for (const id of draft.statSources.masteries.picked) {
      const e = app.masteries.find((m) => m.id === id)?.effect;
      if (e !== undefined && e !== "record_only" && "actual_delay" in e) sum += e.actual_delay.percent;
    }
    return sum;
  });

  /** このキャラのスキルぶんの中ディレイ減少 %(共通の供給源は含まない) */
  const delaySkillPercent = $derived(
    actualDelayPercent(
      draft.statSources.character_skills.skill_ids,
      app.characterSkills,
      draft.statSources.masteries.picked,
    ),
  );
  // キャラは名前で探すより顔で選ぶほうが速い(ゲーム内も顔で選ぶ)。§06 の 40px。
  // 名前は必ず併記する(アイコン単独表示は禁止)
  const characterOptions = $derived(app.gameCharacters.map((c) => ({ value: c.id, label: c.name })));
  const gameCharacterName = $derived(
    app.gameCharacters.find((c) => c.id === draft.gameCharacterId)?.name ?? "未選択",
  );
  /** キャラは登録時に決めるもの。ふだんは畳んでおく */
  let charPickOpen = $state(false);

  // 主軸スキル。未収録のキャラがあるので未選択("")を許す。
  // キャラ種を変えたら前キャラのスキル id が残らないよう同期的に外す(保存時に Rust 側が弾く値)。
  /** 火力の高い順。主軸に選ばれるのはほぼこの上位なので、候補として先に出す */
  const skillPower = (s: Skill) => s.multiplier * Math.max(1, s.hit_count);
  /** 一覧でも名前だけにしない。単 / 範・段数・属性を名前の後ろに付ける */
  const skillMeta = (s: Skill) =>
    `${s.target === null ? "?" : s.target === "single" ? "単" : "範"} ・ ${s.hit_count} 段 ・ ${ELEMENT_LABELS[s.element]}`;
  const mainSkillOptions = $derived<PickerOption[]>([
    { value: "", name: "未選択(攻撃力を出さない)", iconId: null },
    ...[...skills]
      .sort((a, b) => skillPower(b) - skillPower(a))
      .map((s) => ({ value: s.id, name: s.name, meta: skillMeta(s), iconId: s.id, iconKind: "skill" as const })),
  ]);
  const topSkills = $derived([...skills].sort((a, b) => skillPower(b) - skillPower(a)).slice(0, 3));
  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);
  /** 候補にない主軸を選んでいるとき、または自分で開いたときだけ全部出す */
  let skillListOpen = $state(false);
  const skillPickedOutside = $derived(
    draft.mainSkillId !== "" && !topSkills.some((s) => s.id === draft.mainSkillId),
  );
  function setGameCharacterId(id: string) {
    if (id === draft.gameCharacterId) return;
    draft.gameCharacterId = id;
    draft.mainSkillId = "";
  }
  // 段の名前は数字だけにして、幅いっぱいを 6 で割る(§08 `.seg.full`)。
  // 「0 段階」…「5 段階」だと欄の幅で折り返して 2 段になり、段の並びが読めなくなる
  const stageOptions = Array.from({ length: 6 }, (_, i) => ({ value: String(i), label: String(i) }));
  // エタの意志 Lv は 0〜100 の**数値**。101 個を並べても段階にならないので、
  // 節目(20 / 40 / 60 / 80 / 90)を選べる形 + 数値の微調整にする。
  // 節目はそこを超えると上限の増え方が一段上がる地点で、育成の目標地点そのもの。
  const eternalMilestoneOptions = $derived(
    ETERNAL_MILESTONES.filter((lv) => lv <= limits.eternal_level_max).map((lv) => ({
      value: String(lv),
      label: String(lv),
    })),
  );
  /** 覚醒 5 でないとエタの意志は効かない(gamedata: 段階 0〜4 は STAGE_CAPS を引く) */
  const eternalActive = $derived(Number(draft.stage) >= 5);
  /** エタの意志は覚醒 5 の先にあるものなので、触った時点で覚醒は 5 で確定する */
  function setEternalLevel(level: string) {
    draft.eternalLevel = level;
    if (Number(level) > 0) draft.stage = "5";
  }
  // 覚醒段階は 4 と 5 しか使わない(このツールの対象)。それ以外は開いたときだけ出す
  const stageMainOptions = [4, 5].map((i) => ({ value: String(i), label: String(i) }));
  let stageAllOpen = $state(false);
  const stageIsLow = $derived(Number(draft.stage) < 4);

  // 属性は主軸スキルで決まる。無属性のスキルのときだけ、乗せる属性を選ばせる
  // (アンプルで属性を足す運用が多い)
  const skillElement = $derived(mainSkill?.element ?? null);
  const elementFromSkill = $derived(skillElement !== null && skillElement !== "neutral");
  let elementPickOpen = $state(false);
  /**
   * 属性は主軸スキルで決まるので、スキルを選んだら供給源もその属性に合わせる(自動値)。
   * 自分で「別の属性を乗せる」を開いたときは触らない — 例外操作を上書きしない
   */
  $effect(() => {
    if (!elementFromSkill || elementPickOpen) return;
    if (mainElement === skillElement) return;
    setMainElement(skillElement as string);
  });

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

  const strongWeaponOptions = $derived([
    { value: "0", label: "なし" },
    ...Array.from({ length: limits.strong_weapon_level_max }, (_, i) => {
      const lv = i + 1;
      const percent = Math.round(lv * limits.strong_weapon_rate_per_level * 100);
      return { value: String(lv), label: `Lv${lv}(+${percent}%)` };
    }),
  ]);
  const relicKindOptions = [
    { value: "godbird", label: "神鳥" },
    { value: "lunaria", label: "ルナリア" },
  ];
  const relicLevelOptions = Array.from({ length: 10 }, (_, i) => ({
    value: String(i + 1),
    label: `+${i + 1}`,
  }));


  // --- 装備ドリルダウン(部位一覧 ⇄ 部位詳細) --------------------------------
  let openPart = $state<PartSlot | null>(null);
  let itemQuery = $state("");
  let showOtherEquipmentStats = $state(false);
  const visibleEquipmentStats = $derived(
    showOtherEquipmentStats ? [...PRIMARY_EQUIPMENT_STATS, ...OTHER_EQUIPMENT_STATS] : PRIMARY_EQUIPMENT_STATS,
  );
  let showAllEquipmentCandidates = $state(false);
  let itemPickerOpen = $state(false);
  let draggedEquipmentRegistration = $state<{ slot: PartSlot; id: number } | null>(null);
  let equipmentRegistrationDropAt = $state<{ slot: PartSlot; index: number } | null>(null);
  let confirmEquipmentDeleteId = $state<number | null>(null);
  let confirmEquipmentDeleteTimer: ReturnType<typeof setTimeout> | null = null;
  const selectedPartOrNull = (slot: PartSlot) => {
    const list = draft.equipment.parts[slot];
    return list.registered.find((p) => p.id === list.selected_id) ?? null;
  };
  const closeEquipmentOnEscape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    if (openSienaPart !== null) openSienaPart = null;
    else if (openPart !== null) openPart = null;
  };
  const selectedPart = (slot: PartSlot) => {
    const list = draft.equipment.parts[slot];
    let part = list.registered.find((p) => p.id === list.selected_id);
    if (!part) {
      part = neutralEquipmentPart();
      part.id = Math.max(0, ...list.registered.map((p) => p.id)) + 1;
      list.registered.push(part);
      list.selected_id = part.id;
    }
    return part;
  };
  const createEquipmentRegistration = (slot: PartSlot) => {
    const list = draft.equipment.parts[slot];
    const next = neutralEquipmentPart();
    next.id = Math.max(0, ...list.registered.map((p) => p.id)) + 1;
    next.label = `装備 ${list.registered.length + 1}`;
    list.registered.push(next); list.selected_id = next.id;
    itemQuery = "";
    showOtherEquipmentStats = false;
    showAllEquipmentCandidates = false;
    itemPickerOpen = true;
  };
  const selectEquipmentRegistration = (slot: PartSlot, id: number) => {
    draft.equipment.parts[slot].selected_id = id;
    confirmEquipmentDeleteId = null;
  };
  const startEquipmentRegistrationDrag = (event: DragEvent, slot: PartSlot, id: number) => {
    draggedEquipmentRegistration = { slot, id };
    event.dataTransfer?.setData("text/plain", String(id));
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  };
  const dragEquipmentRegistrationOver = (event: DragEvent, slot: PartSlot, index: number) => {
    if (draggedEquipmentRegistration?.slot !== slot) return;
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    equipmentRegistrationDropAt = { slot, index: index + (event.clientX < rect.left + rect.width / 2 ? 0 : 1) };
  };
  const dropEquipmentRegistration = (event: DragEvent, slot: PartSlot) => {
    event.preventDefault();
    const dragging = draggedEquipmentRegistration;
    const dropAt = equipmentRegistrationDropAt;
    draggedEquipmentRegistration = null;
    equipmentRegistrationDropAt = null;
    if (dragging?.slot !== slot || dropAt?.slot !== slot) return;
    const registered = draft.equipment.parts[slot].registered;
    const from = registered.findIndex((part) => part.id === dragging.id);
    if (from === -1) return;
    const [part] = registered.splice(from, 1);
    const target = from < dropAt.index ? dropAt.index - 1 : dropAt.index;
    registered.splice(Math.max(0, Math.min(registered.length, target)), 0, part);
  };
  const removeSelectedEquipmentRegistration = (slot: PartSlot) => {
    const list = draft.equipment.parts[slot];
    const id = list.selected_id;
    if (id === null) return;
    if (confirmEquipmentDeleteId !== id) {
      confirmEquipmentDeleteId = id;
      if (confirmEquipmentDeleteTimer !== null) clearTimeout(confirmEquipmentDeleteTimer);
      confirmEquipmentDeleteTimer = setTimeout(() => (confirmEquipmentDeleteId = null), 4000);
      return;
    }
    if (confirmEquipmentDeleteTimer !== null) clearTimeout(confirmEquipmentDeleteTimer);
    confirmEquipmentDeleteTimer = null;
    confirmEquipmentDeleteId = null;
    const index = list.registered.findIndex((part) => part.id === id);
    if (index === -1) return;
    list.registered.splice(index, 1);
    list.selected_id = list.registered[Math.min(index, list.registered.length - 1)]?.id ?? null;
    itemPickerOpen = false;
  };
  const openPartLabel = $derived(openPart ? PART_SLOT_LABELS[openPart] : "");
  const catalogFor = $derived(
    openPart
      ? app.equipmentCatalog
          .filter((i) => i.slot === openPart)
          .sort((a, b) =>
            sumValues(b.values_max) - sumValues(a.values_max)
            || sumValues(b.enchant_caps) - sumValues(a.enchant_caps)
            || a.name.localeCompare(b.name, "ja"))
      : [],
  );
  const weaponSystemsFor = (dependency: SkillDependency | null): WeaponSystem[] => {
    if (dependency === "stab") return ["stab", "stab_hack"];
    if (dependency === "hack") return ["hack", "stab_hack", "int_hack"];
    if (dependency === "int") return ["int", "int_hack"];
    if (dependency === "mr") return ["mr"];
    if (dependency === "stab_hack") return ["stab_hack"];
    if (dependency === "hack_int") return ["int_hack"];
    return [];
  };
  /** 依存能力だけでは刀/太刀/大剣を区別できないスキルの実用武器。 */
  const weaponClassesForSkill = (skillId: string | undefined): WeaponClass[] => {
    if (skillId === "boris_continuous") return ["katana"];
    if (skillId === "boris_blur_sword") return ["tachi"];
    if (skillId === "boris_ice_attack_sword") return ["great_sword"];
    return [];
  };
  const weaponClassLabel = (weaponClass: WeaponClass): string => {
    const labels: Partial<Record<WeaponClass, string>> = { katana: "刀", tachi: "太刀", great_sword: "大剣" };
    return labels[weaponClass] ?? weaponClass;
  };
  const weaponFilterLabel = $derived.by(() => {
    const classes = weaponClassesForSkill(mainSkill?.id);
    if (classes.length > 0) return `${mainSkill!.name} → ${classes.map(weaponClassLabel).join("・")}`;
    const systems = weaponSystemsFor(mainSkill?.dependency ?? null);
    return systems.length > 0 && mainSkill ? `${mainSkill.name}の依存能力に合う武器` : null;
  });
  const selectedGameCharacter = $derived(
    app.gameCharacters.find((character) => character.id === draft.gameCharacterId) ?? null,
  );
  /** 同じキャラで物理・魔法の専用サブアームが分かれる場合だけ、主軸スキルでも狭める。 */
  const wristTypesForSelection = (types: WristType[], dependency: SkillDependency | null): WristType[] => {
    if (dependency === null) return types;
    if (types.includes("physical_magazine") && types.includes("magic_magazine")) {
      return dependency === "int" || dependency === "mr" || dependency === "hack_int"
        ? ["magic_magazine"]
        : ["physical_magazine"];
    }
    if (types.includes("dual_blade_physical") && types.includes("dual_blade_magic")) {
      return dependency === "int" || dependency === "mr" || dependency === "hack_int"
        ? ["dual_blade_magic"]
        : ["dual_blade_physical"];
    }
    if (types.includes("spellbook") && types.includes("crystal_ball")) {
      return dependency === "int" || dependency === "mr" || dependency === "hack_int"
        ? ["spellbook"]
        : ["crystal_ball"];
    }
    return types;
  };
  const equipmentFilterLabel = $derived.by(() => {
    if (openPart === "weapon") return weaponFilterLabel;
    if (openPart === "artifact" && mainSkill) return `${mainSkill.name}の依存能力に合うAF`;
    if (openPart !== "armor" && openPart !== "shield") return null;
    if (selectedGameCharacter === null) return null;
    return openPart === "shield" && mainSkill
      ? `${selectedGameCharacter.name}・${mainSkill.name}向け`
      : `${selectedGameCharacter.name}が装備可能`;
  });
  const filteredCatalog = $derived.by(() => {
    let candidates = itemQuery.trim() === "" ? catalogFor : catalogFor.filter((i) => i.name.includes(itemQuery.trim()));
    let matched: EquipmentItem[] = [];
    if (openPart === "weapon") {
      const classes = weaponClassesForSkill(mainSkill?.id);
      const systems = weaponSystemsFor(mainSkill?.dependency ?? null);
      matched = classes.length > 0
        ? candidates.filter((i) => i.weapon_class !== null && classes.includes(i.weapon_class))
        : candidates.filter((i) => i.weapon_system !== null && systems.includes(i.weapon_system));
    } else if (openPart === "armor" && selectedGameCharacter !== null) {
      matched = candidates.filter((item) =>
        item.armor_class !== null && selectedGameCharacter!.armor_classes.includes(item.armor_class),
      );
    } else if (openPart === "shield" && selectedGameCharacter !== null) {
      const wristTypes = wristTypesForSelection(
        selectedGameCharacter.wrist_types,
        mainSkill?.dependency ?? null,
      );
      matched = candidates.filter((item) => item.wrist_type !== null && wristTypes.includes(item.wrist_type));
    } else if (openPart === "artifact" && mainSkill !== null) {
      matched = candidates.filter((item) => item.recommended_dependency === mainSkill!.dependency);
    } else {
      return candidates;
    }
    if (matched.length === 0) return candidates;
    return showAllEquipmentCandidates ? [...matched, ...candidates.filter((i) => !matched.includes(i))] : matched;
  });
  const equippedItem = (slot: PartSlot): EquipmentItem | null => {
    const itemId = selectedPartOrNull(slot)?.item_id;
    return itemId ? (app.equipmentCatalog.find((i) => i.id === itemId) ?? null) : null;
  };
  const isRelicSlot = (slot: PartSlot): boolean => slot === "relic_pendant" || slot === "relic_bracelet";
  const relicKindFor = (slot: PartSlot): string => {
    const id = equippedItem(slot)?.id ?? "";
    if (id.startsWith("godbird-")) return "godbird";
    if (id.startsWith("lunaria-")) return "lunaria";
    return "";
  };
  const relicLevelFor = (slot: PartSlot): string => {
    const match = equippedItem(slot)?.id.match(/-plus(\d+)$/);
    return match?.[1] ?? "";
  };
  /** 部位ごとの枠数ルール(domain: PartSlot::ability_slots / random_option_slots)。 */
  const partSlotRule = (slot: PartSlot) => limits.part_slot_rules.find((r) => r.slot === slot) ?? null;
  const currentAbilitySlotCount = (slot: PartSlot) =>
    equippedItem(slot)?.ability_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.ability_slots ?? 0));
  /** 攻撃・耐久の装着時効果の要約。効果なしは null。装備補正値と違って部位の数値には出ないので、
      選ぶ前も選んだ後も文字で見せる。`short` は部位行(3 ペインで幅が狭い)用で、
      カテゴリ名を出すと装備名を押し出してしまうので短い効果名だけにする */
  const itemDamageLabel = (item: EquipmentItem | null, short = false): string | null => {
    if (!item) return null;
    const labels: string[] = item.damage_effects
      .map((e) => {
        if (e === "record_only" || !("damage" in e)) return null;
        const sign = e.damage.percent < 0 ? "−" : "+";
        const head = short ? "与ダメ" : damageCategoryLabel(e.damage.category);
        return `${head} ${sign}${Math.abs(e.damage.percent)}%`;
      })
      .filter((x): x is string => x !== null);
    for (const effect of item.survival_effects) {
      if ("damage_mitigation" in effect) {
        labels.push(`緩和 +${effect.damage_mitigation.percent}%`);
      } else if ("defense_rate" in effect) {
        labels.push(`防御 +${effect.defense_rate.percent}%`);
      } else if ("defense_fixed" in effect) {
        labels.push(`防御 +${effect.defense_fixed.value}`);
      }
    }
    return labels.length === 0 ? null : labels.join(" ・ ");
  };
  const partDisplayName = (slot: PartSlot): string => {
    const item = equippedItem(slot);
    if (item) return item.name;
    const custom = selectedPartOrNull(slot)?.custom_name;
    return custom ? `${custom} [仮]` : "未装備";
  };

  function pickCatalogItem(slot: PartSlot, item: EquipmentItem, keepPickerOpen = false) {
    const part = selectedPart(slot);
    part.item_id = item.id;
    part.custom_name = null;
    part.base = { ...item.values_max };
    part.enchant = clampToCaps(part.enchant, item.enchant_caps);
    part.enhance_type = item.enhance_type;
    part.abilities = part.abilities.slice(0, item.ability_slots);
    if (item.ability_slots === 0) {
      part.ability_values = [];
      part.ability_additions = [];
    }
    part.random_options = part.random_options.slice(0, item.random_option_slots ?? 0);
    itemQuery = "";
    itemPickerOpen = keepPickerOpen;
  }
  function pickRelic(slot: PartSlot, kind: string, level: string) {
    const partName = slot === "relic_pendant" ? "pendant" : "bracelet";
    const item = app.equipmentCatalog.find((candidate) => candidate.id === `${kind}-${partName}-plus${level}`);
    if (item) pickCatalogItem(slot, item, true);
  }
  function pickRelicKind(slot: PartSlot, kind: string) {
    pickRelic(slot, kind, relicLevelFor(slot) || "1");
  }
  function pickRelicLevel(slot: PartSlot, level: string) {
    const kind = relicKindFor(slot);
    if (kind) pickRelic(slot, kind, level);
  }
  function pickUnequipped(slot: PartSlot) {
    draft.equipment.parts[slot].selected_id = null;
    itemQuery = "";
    itemPickerOpen = false;
  }
  function pickCustom(slot: PartSlot) {
    const part = selectedPart(slot);
    const wasCatalogItem = part.item_id !== null;
    part.item_id = null;
    if (part.custom_name === null) part.custom_name = "";
    if (wasCatalogItem) part.enhance_type = null;
    itemQuery = "";
  }

  /** 上限までのエンチャント案。+17を基本に、端数18/19を避けつつ1回減らせる最小個数だけ+20を使う。 */
  function enchantCompletionPlan(remainingValue: number) {
    const remaining = Math.max(0, Math.trunc(remainingValue));
    if (remaining === 0) return { remaining, twentyCount: 0, seventeenCount: 0, remainder: 0, count: 0 };
    const baseCount = Math.ceil(remaining / 17);
    const reducedCount = baseCount - 1;
    // まず「+17だけ」より1回少ない組み合わせを探す。+20の個数を外側から増やすことで、
    // 見つかったものが+20を最小限にした案になる。同数なら+17が多い案を優先する。
    for (let twentyCount = 0; twentyCount <= reducedCount; twentyCount += 1) {
      for (let seventeenCount = reducedCount - twentyCount; seventeenCount >= 0; seventeenCount -= 1) {
        const remainderSlots = reducedCount - twentyCount - seventeenCount;
        if (remainderSlots > 1) continue;
        const remainder = remaining - twentyCount * 20 - seventeenCount * 17;
        if ((remainderSlots === 0 && remainder === 0)
          || (remainderSlots === 1 && remainder >= 1 && remainder <= 16)) {
          return { remaining, twentyCount, seventeenCount, remainder, count: reducedCount };
        }
      }
    }
    const seventeenCount = Math.floor(remaining / 17);
    const remainder = remaining % 17;
    return { remaining, twentyCount: 0, seventeenCount, remainder, count: seventeenCount + (remainder > 0 ? 1 : 0) };
  }
  function enchantCompletionLabel(plan: ReturnType<typeof enchantCompletionPlan>): string {
    const parts: string[] = [];
    if (plan.twentyCount > 0) parts.push(`20 × ${plan.twentyCount}`);
    if (plan.seventeenCount > 0) parts.push(`17 × ${plan.seventeenCount}`);
    if (plan.remainder > 0) parts.push(`端数 ${plan.remainder}`);
    return parts.length === 0 ? "" : `+${parts.join(" + ")}`;
  }
  function enchantPlanStatsFor(item: EquipmentItem | null): EquipmentStatKind[] {
    if (item === null || !ENCHANT_PLAN_SLOTS.has(item.slot)) return [];
    if (item?.weapon_class === "katana") return ["thrust", "slash"];
    // 武器以外は選んだ装備の専用系統(AF等)を優先し、汎用品は主軸スキルの係数へ合わせる。
    const dependency = item.weapon_system ?? item.recommended_dependency ?? mainSkill?.dependency ?? null;
    const dependencyStats: EquipmentStatKind[] = dependency === "stab" ? ["thrust"]
      : dependency === "hack" ? ["slash"]
      : dependency === "stab_hack" ? ["thrust", "slash"]
      : dependency === "int" ? ["magic_attack"]
      : dependency === "mr" ? ["magic_defense"]
      : dependency === "int_hack" ? ["slash", "magic_attack"]
      : dependency === "hack_int" ? ["slash", "magic_attack"]
      : [];
    const supportedDependencyStats = dependencyStats.filter((kind) => item.enchant_caps[kind] > 0);
    if (supportedDependencyStats.length > 0) return supportedDependencyStats;

    // 鎧・盾は攻撃補正を持たない品がある。火力係数が当たらない場合は、その部位で
    // 実際に伸ばせる耐久の主要補正へ案内する(物防・魔防・回避は詳細を開いた行に表示)。
    if (item.slot === "armor" || item.slot === "shield") {
      return (["physical_defense", "magic_defense", "evasion"] as EquipmentStatKind[])
        .filter((kind) => item.enchant_caps[kind] > 0);
    }

    // 主軸スキル未選択でも案内自体を消さない。効果は最大枠の系統、その他の汎用品は
    // エンチャント可能なSHIMを候補にする。
    const supportedPrimary = PRIMARY_EQUIPMENT_STATS.filter((kind) => item.enchant_caps[kind] > 0);
    if (item.slot === "effect" && supportedPrimary.length > 0) {
      const maxCap = Math.max(...supportedPrimary.map((kind) => item.enchant_caps[kind]));
      return supportedPrimary.filter((kind) => item.enchant_caps[kind] === maxCap);
    }
    return supportedPrimary;
  }

  /** その部位の攻撃力(A)への寄与(外すと減る量)。主軸スキル未選択なら null */
  const partContribution = (slot: PartSlot): number | null =>
    preview?.attack?.part_contributions.find((c) => c.slot === slot)?.value ?? null;

  // 武器アビリティは3スロット。同じカテゴリーは1つまでだが、同じ攻撃系統でも
  // カテゴリー1「下級斬り」とカテゴリー4「夜星の鋭い刃」は併用できる。
  const abilityDef = (id: string) => app.equipmentAbilities.find((a) => a.id === id) ?? null;
  const abilityFitsWeapon = (family: EquipmentAbilityFamily, system: WeaponSystem | null): boolean => {
    if (family === "weapon_delay" || system === null) return true;
    if (system === "stab") return family === "pointed_blade";
    if (system === "hack") return family === "sharp_blade";
    if (system === "stab_hack") return family === "pointed_blade" || family === "sharp_blade";
    if (system === "int") return family === "intelligence";
    if (system === "int_hack") return family === "sharp_blade" || family === "intelligence";
    return family === "magic_resistance";
  };
  const abilityWeaponSystem = (slot: PartSlot): WeaponSystem | null => {
    const catalogSystem = equippedItem(slot)?.weapon_system;
    if (catalogSystem) return catalogSystem;
    switch (selectedPartOrNull(slot)?.enhance_type) {
      case "weapon_stab": return "stab";
      case "weapon_stab_hack": return "stab_hack";
      case "weapon_hack": return "hack";
      case "weapon_int": return "int";
      case "weapon_int_hack": return "int_hack";
      case "weapon_mr": return "mr";
      default: return null;
    }
  };
  /** 収録候補を武器系統で絞る。カスタム武器で系統不明なら、選べなくしないため全系統を出す。 */
  const abilityCandidates = (slot: PartSlot, category: number) => {
    const system = abilityWeaponSystem(slot);
    const candidates = app.equipmentAbilities.filter((ability) => ability.slot === slot
      && ability.category === category
      && (slot !== "weapon" || abilityFitsWeapon(ability.family, system)));
    const preferred = [
      "storm-blade", "gale-blade", "soft-wind-blade", "breeze-blade", "silence-blade",
    ];
    const score = (ability: EquipmentAbilityDef) => Math.max(
      ability.values.thrust, ability.values.slash, ability.values.magic_attack, ability.values.magic_defense,
    );
    return candidates.sort((a, b) => category === 3
      ? preferred.indexOf(a.id) - preferred.indexOf(b.id)
      : score(b) - score(a));
  };
  const abilityIdForCategory = (slot: PartSlot, category: number): string =>
    selectedPart(slot).abilities.find((id) => abilityDef(id)?.category === category) ?? "";
  function setAbilityForCategory(slot: PartSlot, category: number, id: string) {
    const part = selectedPart(slot);
    if (abilityIdForCategory(slot, category) === id) return;
    const previousIds = part.abilities.filter((current) => abilityDef(current)?.category === category);
    part.abilities = part.abilities.filter((current) => abilityDef(current)?.category !== category);
    part.ability_values = (part.ability_values ?? []).filter(
      (value) => !previousIds.includes(value.ability_id),
    );
    part.ability_additions = (part.ability_additions ?? []).filter(
      (addition) => !previousIds.includes(addition.ability_id),
    );
    const def = abilityDef(id);
    if (def?.slot === slot && def.category === category && (slot !== "weapon" || abilityFitsWeapon(def.family, abilityWeaponSystem(slot)))) {
      part.abilities = [...part.abilities, id];
    }
  }
  const nonWeaponAbilityCandidates = (slot: PartSlot): EquipmentAbilityDef[] =>
    app.equipmentAbilities.filter((ability) => ability.slot === slot);
  function toggleNonWeaponAbility(slot: PartSlot, ability: EquipmentAbilityDef) {
    const part = selectedPart(slot);
    if (part.abilities.includes(ability.id)) {
      part.abilities = part.abilities.filter((id) => id !== ability.id);
      part.ability_values = (part.ability_values ?? []).filter((a) => a.ability_id !== ability.id);
      part.ability_additions = (part.ability_additions ?? []).filter((a) => a.ability_id !== ability.id);
      return;
    }
    const replacedIds = part.abilities.filter((id) => abilityDef(id)?.exclusive_group === ability.exclusive_group);
    if (replacedIds.length > 0) {
      part.abilities = part.abilities.filter((id) => !replacedIds.includes(id));
      part.ability_values = (part.ability_values ?? []).filter((value) => !replacedIds.includes(value.ability_id));
      part.ability_additions = (part.ability_additions ?? []).filter((addition) => !replacedIds.includes(addition.ability_id));
    }
    const max = currentAbilitySlotCount(slot);
    if (part.abilities.length >= max) {
      if (max === 1) {
        part.abilities = [];
        part.ability_values = [];
        part.ability_additions = [];
      }
      else return;
    }
    part.abilities = [...part.abilities, ability.id];
    if (ability.value_option) {
      part.ability_values = [...(part.ability_values ?? []), {
        ability_id: ability.id,
        kind: ability.value_option.kind,
        value: ability.value_option.max,
      }];
    }
  }
  const additionalKindLabel = (kind: EquipmentAbilityAdditionalKind): string => ({
    fixed_damage: "ダメージ増加", damage_rate: "ダメージ増加率",
    thrust: "突き攻撃力", slash: "斬り攻撃力", magic_attack: "魔法攻撃力",
    magic_defense: "魔法防御力", hp_recovery: "HP自然回復力",
    mp_recovery: "MP自然回復力", accuracy: "命中率補正",
    physical_defense: "物理防御力", critical: "クリティカル補正", evasion: "回避率補正",
    damage_resistance: "ダメージ耐性", physical_damage_reduction: "物理被害減少",
    magic_damage_reduction: "魔法被害減少", sp_recovery: "SP自然回復力", evasion_rate: "回避率",
    fire_element: "火属性", water_element: "水属性", wind_element: "風属性", earth_element: "土属性",
    lightning_element: "雷属性", white_element: "白属性", dark_element: "黒属性",
  }[kind]);
  const abilityRecordedValue = (slot: PartSlot, abilityId: string): number =>
    (selectedPart(slot).ability_values ?? []).find((value) => value.ability_id === abilityId)?.value ?? 0;
  function setAbilityRecordedValue(slot: PartSlot, abilityId: string, value: number) {
    const current = (selectedPart(slot).ability_values ?? []).find((roll) => roll.ability_id === abilityId);
    if (current) current.value = value;
  }
  const additionsFor = (slot: PartSlot, abilityId: string) =>
    (selectedPart(slot).ability_additions ?? []).filter((a) => a.ability_id === abilityId);
  const additionalRangeLabel = (option: { kind: EquipmentAbilityAdditionalKind; min: number; max: number }): string => {
    const sign = option.kind === "physical_damage_reduction" || option.kind === "magic_damage_reduction" ? "−" : "+";
    return option.min === option.max ? `${sign}${option.max.toLocaleString()}` : `${sign}${option.min}〜${option.max}`;
  };
  const additionalAt = (slot: PartSlot, abilityId: string, index: number) => additionsFor(slot, abilityId)[index] ?? null;
  function setAdditionalValue(slot: PartSlot, abilityId: string, index: number, value: number) {
    const current = additionalAt(slot, abilityId, index);
    if (current) current.value = value;
  }
  const addableAdditionalOptions = (slot: PartSlot, abilityId: string) => {
    const used = new Set(additionsFor(slot, abilityId).map((addition) => addition.kind));
    return (abilityDef(abilityId)?.additional_options ?? []).filter(
      (option) => (slot !== "weapon" || (option.kind !== "hp_recovery" && option.kind !== "mp_recovery")) && !used.has(option.kind),
    );
  };
  function addAdditional(slot: PartSlot, abilityId: string, kind: EquipmentAbilityAdditionalKind) {
    const part = selectedPart(slot);
    const current = additionsFor(slot, abilityId);
    const max = abilityDef(abilityId)?.additional_slots ?? 0;
    if (current.length >= max || current.some((addition) => addition.kind === kind)) return;
    const option = abilityDef(abilityId)?.additional_options.find((candidate) => candidate.kind === kind);
    if (!option || (slot === "weapon" && (option.kind === "hp_recovery" || option.kind === "mp_recovery"))) return;
    current.push({ ability_id: abilityId, kind: option.kind, value: option.max });
    part.ability_additions = (part.ability_additions ?? []).filter((a) => a.ability_id !== abilityId).concat(current.slice(0, max));
  }
  function removeAdditional(slot: PartSlot, abilityId: string, index: number) {
    const part = selectedPart(slot);
    const current = additionsFor(slot, abilityId);
    current.splice(index, 1);
    part.ability_additions = (part.ability_additions ?? []).filter((a) => a.ability_id !== abilityId).concat(current);
  }
  const abilityImpactSummary = (slot: PartSlot): string => {
    const part = selectedPart(slot);
    const stats = [
      ["突き", "thrust"], ["斬り", "slash"], ["物防", "physical_defense"],
      ["魔攻", "magic_attack"], ["魔防", "magic_defense"], ["命中", "accuracy"],
      ["Cri", "critical"], ["回避", "evasion"], ["敏捷", "agility"],
    ] as const;
    const pieces = stats.flatMap(([label, kind]) => {
      const value = partAbilityValues(slot)[kind];
      return value === 0 ? [] : [`${label} +${value}`];
    });
    const additions = part.ability_additions ?? [];
    const fixed = additions.filter((a) => a.kind === "fixed_damage").reduce((sum, a) => sum + a.value, 0);
    const rate = additions.filter((a) => a.kind === "damage_rate").reduce((sum, a) => sum + a.value, 0);
    if (fixed !== 0) pieces.push(`固定 +${fixed.toLocaleString()}`);
    if (rate !== 0) pieces.push(`ダメージ +${rate}%`);
    if (pieces.length === 0) {
      const modeled = part.abilities.map(abilityDef).filter((def) => def && !def.record_only).map((def) => def!.effect_summary);
      return modeled.length > 0 ? modeled.join(" / ") : (part.abilities.length > 0 ? "記録のみ" : "未装着");
    }
    return pieces.join(" / ");
  };
  /** 基本能力値のうち、この部位の装備アビリティ由来の分(表示用の内訳)。計算は Rust 側(preview) */
  const partAbilityValues = (slot: PartSlot) =>
    preview?.part_ability_values.find((p) => p.slot === slot)?.values ?? zeroValues();
  /** 部位詳細を開いたときに、旧データの同カテゴリー重複を1つへ畳む。 */
  function openPartDetail(slot: PartSlot) {
    const part = selectedPartOrNull(slot);
    showAllEquipmentCandidates = false;
    if (!part) { openPart = slot; itemQuery = ""; itemPickerOpen = false; return; }
    const seen = new Set<string>();
    const normalized = part.abilities.filter((id) => {
      if (abilityDef(id)?.slot !== slot) return false;
      const category = abilityDef(id)?.category;
      if (category === undefined || (slot === "weapon" && seen.has(String(category)))) return false;
      seen.add(String(category));
      return true;
    }).slice(0, currentAbilitySlotCount(slot));
    if (normalized.length !== part.abilities.length) part.abilities = normalized;
    part.ability_values = (part.ability_values ?? []).filter((value) => normalized.includes(value.ability_id));
    part.ability_additions = (part.ability_additions ?? []).filter((addition) => normalized.includes(addition.ability_id));
    openPart = slot;
    itemPickerOpen = false;
    showOtherEquipmentStats = false;
  }

  // --- 称号 ---------------------------------------------------------------
  // 装備枠 1 つ。表示中の 1 件だけが効く(所持ぶんの累積ではない。wiki: 称号システム)。
  let titleQuery = $state("");
  const selectedTitle = $derived(app.titles.find((t) => t.id === draft.equipment.title) ?? null);
  /** 普段使う称号。無条件ダメージ +20%以上と、地域称号のうち実用される最上位 2 件。 */
  const titleIsCommon = (t: TitleDef): boolean =>
    t.attack_damage_percent >= 20 || t.id === "eclipse" || t.id === "shinchou_no_negura";
  /** 依存違いの一部だけに追加効果が書かれている場合も、同名の変種はまとめて常設する。 */
  const commonTitleBases = $derived(
    new Set(app.titles.filter(titleIsCommon).map((t) => t.name.split(" - ")[0])),
  );
  const commonTitles = $derived(app.titles.filter((t) => commonTitleBases.has(t.name.split(" - ")[0])));
  const otherTitles = $derived(app.titles.filter((t) => !commonTitleBases.has(t.name.split(" - ")[0])));
  const filteredOtherTitles = $derived.by(() => {
    const q = titleQuery.trim();
    if (q === "") return otherTitles;
    return otherTitles.filter((t) => t.name.includes(q) || t.group.includes(q));
  });
  /** 称号の補正値の要約(値が入っている列だけ)。 */
  const titleSummary = (t: TitleDef): string =>
    EQUIPMENT_STAT_KINDS.filter((k) => t.values[k] !== 0)
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]}${t.values[k]}`)
      .join(" ");
  /** 「緋馬の怪火 - 突き」の「緋馬の怪火」。同じ称号の依存違いをまとめる単位 */
  const titleBase = (t: TitleDef): string => t.name.split(" - ")[0];
  /** 同じ称号の変種(突き / 斬り / 魔攻 …)を 1 行にまとめる。
      カタログの並び(ダメージ増加の大きい順)は最初に出てきた変種の位置で保つ */
  const groupTitles = (titles: TitleDef[]) => {
    const groups = new Map<string, TitleDef[]>();
    for (const t of titles) {
      const base = titleBase(t);
      const list = groups.get(base);
      if (list) list.push(t);
      else groups.set(base, [t]);
    }
    return [...groups].map(([base, items]) => ({ base, items }));
  };
  const commonTitleGroups = $derived(groupTitles(commonTitles));
  const otherTitleGroups = $derived(groupTitles(filteredOtherTitles));

  // --- ランダムオプション -------------------------------------------------
  // 効果値の上限は wiki の一覧表のレンジそのもの。枠数は wiki に記載が無く、代わりに
  // 「同じカテゴリーは 1 部位に 1 つまで(カテゴリー 0 は除く)」で縛る(転移の説明)。
  const randomOptionDef = (id: string): RandomOptionDef | undefined =>
    app.randomOptions.find((d) => d.id === id);

  /** その部位に足せる OP(未選択 かつ カテゴリーが空いているもの) */
  function addableRandomOptions(slot: PartSlot) {
    const part = selectedPart(slot);
    const takenIds = new Set(part.random_options.map((o) => o.option_id));
    const takenCategories = new Set(
      part.random_options.map((o) => randomOptionDef(o.option_id)?.category).filter((c) => c !== undefined && c !== 0),
    );
    // 段階選択に並べるので、プレースホルダは入れない。押せない選択肢を混ぜると
    // 「選ばれているのに何も起きない」項目になる(§00 意味のないものを置かない)
    return [
      ...app.randomOptions
        .filter((d) => d.slot === slot && !takenIds.has(d.id) && !takenCategories.has(d.category))
        .map((d) => ({ value: d.id, label: d.name })),
    ];
  }

  /** ランダムOP のドリルダウン。装備と同じく、押した部位は残して右にペインを足す(§09 規則 2) */
  let openRandomPart = $state<PartSlot | null>(null);
  /** その部位に付いている OP の要約(行に出す) */
  const randomOptionSummary = (slot: PartSlot): string => {
    const options = selectedPart(slot).random_options;
    if (options.length === 0) return NEUTRAL_RO;
    return options
      .map((o) => randomOptionDef(o.option_id)?.name ?? o.option_id)
      .join(" ・ ");
  };
  const NEUTRAL_RO = "なし";
  /** その部位に足せる OP のうち、実際によく付けるもの(gamedata の common) */
  const addableDefs = (slot: PartSlot): RandomOptionDef[] =>
    addableRandomOptions(slot)
      .map((o) => randomOptionDef(o.value))
      .filter((d): d is RandomOptionDef => d !== undefined);
  /**
   * よく使う OP。**主軸スキルの依存に合う「◯◯攻撃力が増加」を先頭**に出す —
   * ここで実際に選ばれるのはほぼそれで、攻撃ダメージ増加はその次(ユーザー確認 2026-08-26)
   */
  const commonAddable = (slot: PartSlot) => {
    const dependency = mainSkill?.dependency ?? null;
    const rank = (d: RandomOptionDef) => {
      const effect = d.effect;
      if (typeof effect === "object") {
        return effect.dependency_damage_rate === dependency ? 0 : 1;
      }
      return 2;
    };
    return addableDefs(slot)
      .filter((d) => d.common)
      .sort((a, b) => rank(a) - rank(b));
  };
  const otherAddable = (slot: PartSlot) => addableDefs(slot).filter((d) => !d.common);
  const otherPickerOptions = (slot: PartSlot): PickerOption[] =>
    otherAddable(slot).map((d) => ({
      value: d.id,
      name: d.name,
      meta: `カテゴリー${d.category} ・ ${randomOptionEffectLabel(d.effect)}`,
      iconId: undefined,
    }));
  /** 候補の面に出す形。名前だけでは選べないので、カテゴリーと効き先を並べる */
  const addablePickerOptions = (slot: PartSlot): PickerOption[] =>
    addableRandomOptions(slot).map((o) => {
      const def = randomOptionDef(o.value);
      return {
        value: o.value,
        name: def?.name ?? o.label,
        meta: def ? `カテゴリー${def.category} ・ ${randomOptionEffectLabel(def.effect)}` : undefined,
        iconId: undefined,
      };
    });
  function addRandomOption(slot: PartSlot, id: string) {
    if (id === "") return;
    const def = randomOptionDef(id);
    if (!def || def.tiers.length === 0) return;
    // 既定は一覧のいちばん上位のランク(手持ちがそれ未満なら下げてもらう)
    const rank = def.tiers[def.tiers.length - 1].rank;
    selectedPart(slot).random_options = [
      ...selectedPart(slot).random_options,
      { option_id: id, rank, value: null },
    ];
  }

  function removeRandomOption(slot: PartSlot, index: number) {
    const part = selectedPart(slot);
    part.random_options = part.random_options.filter((_, i) => i !== index);
  }

  /** その部位に付けられる枠の数(domain: PartSlot::random_option_slots)。武器だけ 3 枠 */
  const randomOptionSlots = (slot: PartSlot) =>
    equippedItem(slot)?.random_option_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.random_option_slots ?? 0));
  const rankOptions = (def: RandomOptionDef) =>
    RANDOM_OPTION_RANKS.filter((r) => def.tiers.some((t) => t.rank === r)).map((r) => ({
      value: r,
      label: RANDOM_OPTION_RANK_LABELS[r],
    }));
  // **実際に使うのは Special と S・真だけ**(Normal / Valuable / Rare はほぼ付けない。
  // ユーザー確認 2026-08-26)。下位は開いたときだけ出す
  const MAIN_RANKS: RandomOptionRank[] = ["special", "s_true"];
  let rankAllOpen = $state(false);
  const rankOptionsNow = (def: RandomOptionDef, rank: RandomOptionRank) => {
    const all = rankOptions(def);
    if (rankAllOpen || !MAIN_RANKS.includes(rank)) return all;
    const main = all.filter((o) => MAIN_RANKS.includes(o.value));
    return main.length > 0 ? main : all;
  };
  const hasLowerRanks = (def: RandomOptionDef) =>
    rankOptions(def).some((o) => !MAIN_RANKS.includes(o.value));

  /** ランクを変えるとレンジが変わるので、実測の上書きは外して既定(レンジ上限)へ戻す */
  function setRandomOptionRank(slot: PartSlot, index: number, rank: RandomOptionRank) {
    const option = selectedPart(slot).random_options[index];
    option.rank = rank;
    option.value = null;
  }

  const tierOf = (def: RandomOptionDef, rank: RandomOptionRank) =>
    def.tiers.find((t) => t.rank === rank);

  // --- 共通スキル(wiki: Skill/共通)---------------------------------------
  // オーグメントはストロングウェポン / プロテクトアーマー / ハイパーリミットの Lv2 以降の前提スキル。
  // 上限を超える Lv は保存時に Rust 側が弾くので、選択肢の側で先に絞る。
  const augmentGate = $derived(draft.commonSkills.augment_level + 1);
  /**
   * オーグメント Lv を下げたら、それに縛られる Lv(ストロングウェポン / プロテクトアーマー /
   * ハイパーリミット)も一緒に下げる。放置すると選択肢に無い値が残り、保存だけが失敗する。
   */
  function setAugmentLevel(level: number) {
    const c = draft.commonSkills;
    c.augment_level = level;
    const max = level + 1;
    c.strong_weapon_level = Math.min(c.strong_weapon_level, max);
    c.protect_armor_level = Math.min(c.protect_armor_level, max);
    c.ultimate.hyper_limit_level = Math.min(c.ultimate.hyper_limit_level, max);
  }
  // アンリーシュ(能力解放)。効き先は能力値倍率B。Lv6 以降はレインフォース(Lv5 まで)が前提。
  // 正は crates/domain/src/common_skill.rs の UNLEASH。limits.unleash_rates(Σ% の小数表現)経由で引く
  const UNLEASH_RATES = $derived(limits.unleash_rates.map((r) => Math.round(r * 100)));
  const reinforceGate = $derived(draft.commonSkills.reinforce_level + 5);
  /** レインフォース Lv を下げたら、それに縛られるアンリーシュの Lv も一緒に下げる */
  function setReinforceLevel(level: number) {
    const c = draft.commonSkills;
    c.reinforce_level = level;
    for (const slot of c.unleash) slot.level = Math.min(slot.level, level + 5);
  }
  const reinforceOptions = $derived(
    Array.from({ length: limits.reinforce_level_max + 1 }, (_, i) => ({
      value: String(i),
      label: i === 0 ? `未習得(アンリーシュ Lv5 まで)` : `Lv${i}(アンリーシュ Lv${i + 5} まで)`,
    })),
  );
  /** もう一方の枠で選んでいるステは選べない(同じステの 2 枠は不可) */
  /** 段はいつも 7 つ。もう片方の枠で使っているステは**押せなくするだけ**(消すと幅が動く) */
  const unleashStatOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));
  const unleashDisabled = (slotIndex: number) => {
    const other = draft.commonSkills.unleash[1 - slotIndex].stat;
    return other === null ? [] : [other];
  };
  /** いま取れるアンリーシュの上限(レインフォース Lv + 5、最大 10) */
  const unleashCap = $derived(Math.min(limits.unleash_level_max, reinforceGate));
  /** ステを選んだら Lv は上限で入れる。ここは「どのステに乗せるか」だけを決める場所 */
  function setUnleashStat(slotIndex: number, value: string) {
    const slot = draft.commonSkills.unleash[slotIndex];
    slot.stat = value === "" ? null : (value as StatKind);
    slot.level = slot.stat === null ? 0 : unleashCap;
  }
  const unleashLevelOptions = $derived(
    Array.from({ length: Math.min(limits.unleash_level_max, reinforceGate) + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? "未習得" : `Lv${lv}(+${UNLEASH_RATES[lv - 1]}%)`,
    })),
  );

  // 「未習得」は段に入れない。ほぼ選ばれないものに 1 列を渡さず、外すのは行末の小さな 1 押しで足りる
  const augmentOptions = $derived(
    Array.from({ length: limits.augment_level_max }, (_, i) => ({
      value: String(i + 1),
      label: `Lv${i + 1}`,
    })),
  );
  /** オーグメントで解放されている Lv までを選択肢にする */
  const gatedLevelOptions = (max: number, label: (lv: number) => string) =>
    Array.from({ length: max + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? "未習得" : label(lv),
      disabled: lv > augmentGate,
    })).filter((o) => !o.disabled);
  // 段階選択に並べるのでラベルは Lv だけ。効果値は選択中のものを注記で出す
  // (全段の効果を並べると 7 段で 4 行になり、1 画面の情報量が減る)
  // 正は crates/domain/src/common_skill.rs の PROTECT_ARMOR_PHYSICAL / _MAGIC
  const PROTECT_ARMOR_RATES = $derived(limits.protect_armor_physical_rates.map((r) => Math.round(r * 100)));
  const PROTECT_ARMOR_MAGIC = $derived(limits.protect_armor_magic_rates.map((r) => Math.round(r * 100)));
  // 畳んだ中の段も上と同じ形にする。名前は Lv の数字だけ、効いている値は行の右
  const levelChoices = (max: number) =>
    Array.from({ length: max }, (_, i) => ({ value: String(i + 1), label: String(i + 1) }));
  const strongWeaponLevels = $derived(levelChoices(limits.strong_weapon_level_max));
  const protectArmorLevels = $derived(levelChoices(limits.protect_armor_level_max));
  const kaiProtectArmorLevels = $derived(levelChoices(limits.kai_protect_armor_level_max));
  const hyperLimitLevels = $derived(levelChoices(limits.hyper_limit_level_max));
  const reinforceLevels = $derived(levelChoices(limits.reinforce_level_max));
  const unleashLevelChoices = $derived(levelChoices(limits.unleash_level_max));
  const protectArmorOptions = $derived(gatedLevelOptions(limits.protect_armor_level_max, (lv) => `Lv${lv}`));
  const protectArmorNote = $derived.by(() => {
    const lv = draft.commonSkills.protect_armor_level;
    return lv === 0 ? "未習得" : `物 +${PROTECT_ARMOR_RATES[lv - 1]}% / 魔 +${PROTECT_ARMOR_MAGIC[lv - 1]}%`;
  });
  const kaiProtectArmorOptions = Array.from({ length: 6 }, (_, lv) => ({
    value: String(lv),
    label: lv === 0 ? "未習得" : `Lv${lv}`,
  }));
  const kaiProtectArmorNote = $derived.by(() => {
    const lv = draft.commonSkills.kai_protect_armor_level;
    if (lv === 0) return "未習得";
    const physical = Math.round(limits.kai_protect_armor_physical_rates[lv - 1] * 100);
    const magic = Math.round(limits.kai_protect_armor_magic_rates[lv - 1] * 100);
    return `物 +${physical}% / 魔 +${magic}%`;
  });
  // 正は crates/domain/src/common_skill.rs の SHARPNESS_VISION
  const SHARPNESS_RATES = $derived(limits.sharpness_vision_rates.map((r) => Math.round(r * 100)));
  // 段の名前は Lv だけ、効いている値は行の右に出す(段に「Lv6(+28%)」と書くと折り返す)。
  // **Lv5 まではほぼ全員が同じ**(そこで止まる)なので、ふだんは 5〜10 だけ出す
  const sharpnessVisionOptions = Array.from({ length: 10 }, (_, i) => ({
    value: String(i + 1),
    label: String(i + 1),
  }));
  const sharpnessMainOptions = sharpnessVisionOptions.slice(4);
  let sharpnessAllOpen = $state(false);
  const sharpnessIsLow = $derived(
    draft.commonSkills.sharpness_vision_level > 0 && draft.commonSkills.sharpness_vision_level < 5,
  );
  const sharpnessOptionsNow = $derived(
    sharpnessAllOpen || sharpnessIsLow ? sharpnessVisionOptions : sharpnessMainOptions,
  );
  const sharpnessVisionNote = $derived.by(() => {
    const lv = draft.commonSkills.sharpness_vision_level;
    return lv === 0 ? "未習得" : `割合追加ダメージ +${SHARPNESS_RATES[lv - 1]}%`;
  });
  /** 装備防御力倍率(共通スキル + シエナのオーラの防御力増加)。表示用 */
  const sienaDefenseRate = $derived(sienaExtraTotal(draft.equipment, "defense_rate"));

  // --- 極限スキル(wiki: Skill/極限)---------------------------------------
  // 3 択から 2 つ。効果値は 基本 + スーパーリミット + ハイパーリミット Lv の加算。
  const hyperLimitOptions = $derived(
    Array.from({ length: limits.hyper_limit_level_max + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? "未習得" : `Lv${lv}`,
    })).filter((o) => Number(o.value) <= augmentGate),
  );
  /** その枠で選べる極限スキル(もう片方の枠で選ばれているものは出さない) */
  function ultimateOptions(slotIndex: number) {
    const other = draft.commonSkills.ultimate.slots[1 - slotIndex];
    // 「未習得」は段に入れない(行末の小さな 1 押しで外せる)
    return ULTIMATE_SKILLS.filter((u) => u !== other).map((u) => ({
      value: u,
      label: ULTIMATE_SKILL_LABELS[u],
    }));
  }
  function setUltimate(slotIndex: number, value: string) {
    draft.commonSkills.ultimate.slots[slotIndex] = value === "" ? null : (value as UltimateSkill);
  }
  /** 極限は「3 つのうち 2 つ」。枠に分けず、押して入れる / 押して外す(§07 形態 3) */
  const ultimatePickedCount = $derived(
    draft.commonSkills.ultimate.slots.filter((u) => u !== null).length,
  );
  function toggleUltimate(skill: UltimateSkill) {
    const slots = draft.commonSkills.ultimate.slots;
    const at = slots.indexOf(skill);
    if (at !== -1) {
      slots[at] = null;
      return;
    }
    const empty = slots.indexOf(null);
    if (empty !== -1) slots[empty] = skill;
  }
  /** 選択中の極限スキルの効果値(表示用。計算は Rust 側 = preview.common_skill.ultimate) */
  const ultimateEffects = $derived.by(() => {
    const u = draft.commonSkills.ultimate;
    const effects = preview?.common_skill.ultimate;
    const out: string[] = [];
    if (u.slots.includes("scope_eye")) {
      out.push(`クリティカルダメージ +${Math.round((effects?.critical_damage_rate ?? 0) * 100)}%`);
    }
    if (u.slots.includes("full_throttle")) {
      out.push(`中ディレイ −${fullThrottlePercent}%`);
      out.push(`単体チャネリング段数 +${effects?.added_hit_count ?? 0}`);
    }
    if (u.slots.includes("wide_focus")) {
      out.push(`スキル範囲 +${effects?.skill_range_bonus ?? 0}`);
    }
    return out;
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

  /** クリティカル率に、この補正源の外から入ってくる分 */
  const criticalFromOthers = $derived<ExternalSource[]>([
    {
      id: "equipment",
      name: "装備クリティカル補正",
      value: PART_SLOTS.reduce((n, slot) => n + (selectedPartOrNull(slot)?.base.critical ?? 0) + (selectedPartOrNull(slot)?.enchant.critical ?? 0), 0),
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
      value: sienaExtraTotal(draft.equipment, "critical_rate"),
      format: (v) => `×${(1 + v / 100).toFixed(2)}`,
      note: "AGI 由来の項に乗算。下の合計には入らない",
    },
  ]);

  /** 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン)。計算は Rust 側 */
  const enhanceRatePercent = $derived(Math.round((preview?.common_skill.equipment_attack_rate ?? 0) * 100));

  const enhanceLevelOptions = $derived(
    [0, 10, 11, 12, 13, 14, 15].map((lv) => ({
      value: String(lv), label: lv === 0 ? "強化なし" : `+${lv}`,
    })),
  );
  const enhanceGradeOptions = [
    { value: "lowest", label: "最下" }, { value: "low", label: "下" },
    { value: "middle", label: "中" }, { value: "high", label: "上" },
    { value: "highest", label: "最上" },
  ];
  const weaponEnhanceTypeOptions = [
    { value: "", label: "種別を選択" },
    { value: "weapon_stab", label: "突き系" }, { value: "weapon_stab_hack", label: "物理複合系" },
    { value: "weapon_hack", label: "斬り系" }, { value: "weapon_int", label: "魔法系" },
    { value: "weapon_int_hack", label: "魔剣系" }, { value: "weapon_mr", label: "魔法防御系" },
  ];
  const armorEnhanceTypeOptions = [
    { value: "", label: "種別を選択" }, { value: "armor_light", label: "軽鎧" },
    { value: "armor_heavy", label: "重鎧" }, { value: "armor_magic", label: "魔鎧" },
    { value: "armor_suit", label: "スーツ" }, { value: "armor_robe", label: "ローブ" },
  ];
  function setEnhanceLevel(slot: PartSlot, level: number) {
    const part = selectedPart(slot);
    part.enhance_level = level;
    part.enhance_grade = level >= 12 ? (part.enhance_grade ?? "highest") : null;
  }

  // --- シエナのオーラ(部位ごと) ------------------------------------------
  // **段階は入力しない。**能力値スロットを 1 個ずつ足した数がそのまま段階になる
  // (wiki: 段階ごとに能力値スロットが 1 個解放)。追加オプションの枠も段階から出る。
  let openSienaPart = $state<SienaPartSlot | null>(null);
  const sienaList = (slot: SienaPartSlot): SienaAuraList => draft.equipment.siena[slot];
  const sienaRegistration = (slot: SienaPartSlot) => selectedSienaAuraRegistration(sienaList(slot));
  const sienaForDisplay = (slot: SienaPartSlot) => selectedSienaAura(sienaList(slot)) ?? neutralSienaAura();
  const createSienaRegistration = (slot: SienaPartSlot) => {
    const list = sienaList(slot);
    const id = Math.max(0, ...list.registered.map((entry) => entry.id)) + 1;
    list.registered.push({ id, label: `オーラ ${list.registered.length + 1}`, aura: neutralSienaAura() });
    list.selected_id = id;
  };
  const removeSelectedSienaRegistration = (slot: SienaPartSlot) => {
    const list = sienaList(slot);
    const index = list.registered.findIndex((entry) => entry.id === list.selected_id);
    if (index < 0) return;
    list.registered.splice(index, 1);
    list.selected_id = list.registered[0]?.id ?? null;
  };
  const sienaIsEquipmentValues = (slot: SienaPartSlot) => SIENA_EQUIPMENT_VALUE_SLOTS.includes(slot);
  /** その部位に出る能力値の種類。武器/盾とその他の部位で一覧が丸ごと違う */
  const sienaValueDefs = (slot: SienaPartSlot) =>
    app.siena.values
      .filter((d) => d.is_equipment_value === sienaIsEquipmentValues(slot))
      // 記録するだけのものは後ろへ。ふだん選ぶのは計算に入るほう(§00 02)
      .sort((a, b) => Number(b.is_modeled) - Number(a.is_modeled));
  const sienaValueDef = (kind: SienaValueKind) => app.siena.values.find((d) => d.kind === kind);
  const sienaExtraDef = (kind: SienaExtraKind) => app.siena.extras.find((d) => d.kind === kind);
  const sienaCapacity = (slot: SienaPartSlot) =>
    sienaExtraCapacity(sienaForDisplay(slot), app.siena.extra_unlock_stages);
  /** まだ付いていない追加オプション(wiki: 同じ種類は同じ装備の別スロットには出ない) */
  const sienaAddableExtras = (slot: SienaPartSlot) => {
    const used = new Set(sienaForDisplay(slot).extras.map((e) => e.kind));
    return app.siena.extras
      .filter((d) => !used.has(d.kind))
      .sort((a, b) => Number(b.is_modeled) - Number(a.is_modeled));
  };
  /** 取りうる値が連番かどうか。連番ならステッパー、飛び飛び(中ディレイ)は段階選択 */
  const sienaChoicesAreRun = (choices: number[]) =>
    choices.every((c, i) => c === choices[0] + i);
  /** 足した直後の値はレンジ上限(再抽選で振り直せるので想定値は最上値。ランダムOP と同じ) */
  function addSienaSlot(slot: SienaPartSlot, kind: SienaValueKind) {
    const def = sienaValueDef(kind);
    const siena = selectedSienaAura(sienaList(slot));
    if (!def || !siena) return;
    siena.slots.push({ kind, value: def.max });
  }
  function removeSienaSlot(slot: SienaPartSlot, index: number) {
    const siena = selectedSienaAura(sienaList(slot));
    if (!siena) return;
    siena.slots.splice(index, 1);
    // 段階が下がって枠が閉じたら、はみ出た追加オプションも落とす(値だけ残る幽霊状態を作らない)
    const capacity = sienaExtraCapacity(siena, app.siena.extra_unlock_stages);
    if (siena.extras.length > capacity) siena.extras.length = capacity;
  }
  function addSienaExtra(slot: SienaPartSlot, kind: SienaExtraKind) {
    const def = sienaExtraDef(kind);
    const siena = selectedSienaAura(sienaList(slot));
    if (!def || def.choices.length === 0 || !siena) return;
    siena.extras.push({
      kind,
      value: def.choices[def.choices.length - 1],
    });
  }
  const removeSienaExtra = (slot: SienaPartSlot, index: number) =>
    selectedSienaAura(sienaList(slot))?.extras.splice(index, 1);

  // --- 主属性 -------------------------------------------------------------
  // 供給源(ペット / モンスターカード / ルーンスキル / 頭・カフスのアビリティ)は、
  // 実際には**全部同じ属性に振る**。だから供給源ごとに聞かず、主属性を 1 回選ばせて
  // まとめて乗せる(§00「要らないものを見せない」)
  const elementSourceDefs = $derived(app.elementSources);
  const elementOptions = [
    { value: "", label: "なし" },
    ...ELEMENTS.map((e) => ({ value: e, label: ELEMENT_LABELS[e] })),
  ];
  /** 供給源が全部同じ属性ならそれが主属性。ばらけていたら "" を返す */
  const mainElement = $derived.by(() => {
    const picked = elementSourceDefs.map((def) => draft.statSources.elements[def.id] ?? null);
    const first = picked[0] ?? null;
    return first !== null && picked.every((e) => e === first) ? first : "";
  });
  function setMainElement(value: string) {
    for (const def of elementSourceDefs) {
      draft.statSources.elements[def.id] = value === "" ? null : (value as Element);
    }
  }
  const elementSourceTotal = $derived(elementSourceDefs.reduce((n, def) => n + def.value, 0));
  // 内訳は Rust 側で出す(キャラ基礎属性値は gamedata にしか無い)。開いている間だけ引く
  let elementPreview = $state<ElementPreview | null>(null);
  let elementSeq = 0;
  $effect(() => {
    if (sourceId !== "status") return;
    const payload = draftToPayload(draft);
    const seq = ++elementSeq;
    previewElements(payload)
      .then((p) => {
        if (seq === elementSeq) elementPreview = p;
      })
      .catch(() => {
        if (seq === elementSeq) elementPreview = null;
      });
  });

  /** 能力値スロットの装備補正合計(武器/盾)。計算は Rust 側(preview) */
  const sienaPartValues = (slot: SienaPartSlot) =>
    preview?.siena_part_values.find((p) => p.slot === slot)?.values ?? zeroValues();
  /** 部位の行に出す要約。段階はバッジで出しているので、ここでは効き先の合計だけ */
  const sienaSummary = (slot: SienaPartSlot): string => {
    const siena = sienaForDisplay(slot);
    if (sienaList(slot).selected_id === null) return "未装着";
    if (sienaStage(siena) === 0) return "未発現";
    const parts: string[] = [];
    if (sienaIsEquipmentValues(slot)) {
      const v = sienaPartValues(slot);
      const top = EQUIPMENT_STAT_KINDS.filter((k) => v[k] > 0)
        .map((k) => `${EQUIPMENT_STAT_SHORT[k]}${fmtInt(v[k])}`);
      if (top.length > 0) parts.push(top.join(" / "));
    }
    const statTotal = sienaPartStatTotal(siena);
    if (statTotal > 0) parts.push(`ステ +${fmtInt(statTotal)}`);
    const attack = sienaExtraValue(siena, "attack_rate");
    if (attack > 0) parts.push(`攻撃力 +${attack}%`);
    return parts.length > 0 ? parts.join(" ・ ") : "—";
  };
  /** 行に出すバッジ。段階 10 だと 13 個になるので上位だけ出し、残りは「+N」で畳む(§00 01) */
  const SIENA_BADGE_MAX = 4;
  const sienaBadges = (slot: SienaPartSlot) => {
    const siena = sienaForDisplay(slot);
    const rows: { key: string; text: string; title: string; modeled: boolean }[] = [];
    siena.slots.forEach((s, i) => {
      const def = sienaValueDef(s.kind);
      if (def) rows.push({
        key: `v${i}`, text: `${def.short}${s.value}${def.unit}`,
        title: `${def.label} +${s.value}${def.unit}`, modeled: def.is_modeled,
      });
    });
    siena.extras.forEach((e, i) => {
      const def = sienaExtraDef(e.kind);
      if (def) rows.push({
        key: `e${i}`, text: `${def.short}${e.value}${def.unit}`,
        title: `${def.label} +${e.value}${def.unit}`, modeled: def.is_modeled,
      });
    });
    return rows;
  };

  // --- テシスコア(地域ごとに 6 枠) ---------------------------------------
  let coreRegion = $state<CoreRegion>("abyss");
  const coreSlotIndexes = Array.from({ length: CORE_SLOT_COUNT }, (_, i) => i);
  // 選ぶのは火力 4 タイプがほとんど。補助タイプ(物防・回避・敏捷・命中)は装備攻撃力に
  // 入らないので、ふだんは畳んでおく(§00「要らないものを見せない」)。
  // 既に補助タイプが入っている枠があるときは、畳んだままだと選択中の段が消えるので開く
  let coreShowSupport = $state(false);
  // 「未装着」は段に入れない。ほぼ選ばれないものが常に 1 列を占めるのは割に合わない。
  // 外すのは行末の小さな × 1 つで足りる(§00 02「要らないものを見せない」)
  const corePowerOptions = CORE_POWER_TYPES.map((t) => ({ value: t, label: CORE_TYPE_LABELS[t] }));
  // ラベルに「(補助)」を付けない。段が分かれていて、上にも注記がある — 4 回同じ語を読ませない
  const coreSupportOptions = CORE_SUPPORT_TYPES.map((t) => ({
    value: t,
    label: CORE_TYPE_LABELS[t],
  }));
  const coreSupportInUse = $derived(
    draft.equipment.thesis_cores[coreRegion].slots.some(
      (c) => c !== null && !CORE_POWER_TYPES.includes(c.core_type),
    ),
  );
  /** 補助の段を閉じるときは、補助タイプの枠も外す。
      見えない段に選択が残ると、画面に出ていない値が効き続ける */
  function toggleCoreSupport() {
    if (!(coreShowSupport || coreSupportInUse)) {
      coreShowSupport = true;
      return;
    }
    const slots = draft.equipment.thesis_cores[coreRegion].slots;
    slots.forEach((core, i) => {
      if (core && !CORE_POWER_TYPES.includes(core.core_type)) slots[i] = null;
    });
    coreShowSupport = false;
  }
  const coreTypeOptions = $derived(
    coreShowSupport || coreSupportInUse ? [...corePowerOptions, ...coreSupportOptions] : corePowerOptions,
  );
  // 進化と強化は別々に選ばせず「4-4」で 1 回で決める(5×5 = 25 通り)。
  // 押した枠の下に重ねて出すので、他の枠は動かない(§09 規則 3)
  let openCoreStage = $state<number | null>(null);
  const coreStagePairs = $derived(
    Array.from({ length: limits.core_evolution_max + 1 }, (_, ev) =>
      Array.from({ length: limits.core_enhancement_max + 1 }, (_, en) => ({ ev, en })),
    ),
  );
  /** コア 1 個の補正値(表示用)。テーブル自体は Rust 側のデータ(limits.core_*_bonus_table)。 */
  const coreBonus = (type: CoreType, evolution: number, enhancement: number): number => {
    const table = CORE_POWER_TYPES.includes(type) ? limits.core_power_bonus_table : limits.core_support_bonus_table;
    return table[evolution]?.[enhancement] ?? 0;
  };
  const coreAt = (index: number) => draft.equipment.thesis_cores[coreRegion].slots[index] ?? null;
  function setCoreType(index: number, value: string) {
    const slots = draft.equipment.thesis_cores[coreRegion].slots;
    slots[index] = value === "" ? null : { core_type: value as CoreType, evolution: 0, enhancement: 0 };
  }
  function setCoreStagePair(index: number, evolution: number, enhancement: number) {
    const core = draft.equipment.thesis_cores[coreRegion].slots[index];
    if (!core) return;
    core.evolution = evolution;
    core.enhancement = enhancement;
  }
  // 補助タイプは与ダメージ(攻撃力)には効かないが、装備値 9 種として防御側・回避Pに効く
  // 地域ごとのコアセット効果はタブが持つ(ゲーム内 UI の地域カードと同じ)。
  // 全地域の合計は「いまの実力」に出す — 結果を入力エリアに積まない。計算は Rust 側(preview)
  const coreRegionPreview = (region: CoreRegion) =>
    preview?.thesis_cores.find((r) => r.region === region) ?? null;
  const coreRegionTotal = (region: CoreRegion) => coreRegionPreview(region)?.total_bonus ?? 0;
  const coreSetOf = (region: CoreRegion) => coreRegionPreview(region);
  /** その地域のコアセット効果(タブに出す短い形)。進化段階ごとの分は合算済み */
  const coreSetLabelOf = (region: CoreRegion) => {
    const e = coreSetOf(region);
    if (!e || e.set_groups.length === 0) return "";
    const parts: string[] = [];
    if (e.set_bonus.final_damage_rate > 0) parts.push(`+${Math.round(e.set_bonus.final_damage_rate * 100)}%`);
    if (e.set_bonus.final_damage_fixed > 0) parts.push(`+${fmtInt(e.set_bonus.final_damage_fixed)}`);
    return parts.join(" ");
  };
  const coreSupport = $derived(coreRegionPreview(coreRegion)?.values ?? zeroValues());
  const coreSupportSummary = $derived(
    [
      ["物防", coreSupport.physical_defense],
      ["回避", coreSupport.evasion],
      ["敏捷", coreSupport.agility],
      ["命中", coreSupport.accuracy],
    ]
      .filter(([, v]) => (v as number) > 0)
      .map(([label, v]) => `${label} +${fmtInt(v as number)}`)
      .join(" ・ "),
  );

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

  const traceFor = (k: StatKind) => preview?.traces.find((t) => t.kind === k) ?? null;
  const signed = (n: number) => `${n >= 0 ? "+" : ""}${fmtInt(n)}`;
</script>

<svelte:window onkeydown={closeEquipmentOnEscape} />

<!-- ランダムオプションの編集(装備の部位詳細と「ランダムOP」ペインで共有する) -->
<!-- ほかの補正源から入ってくる分。**0 の行も出す** — ここは「この値がどこから来るか」の
     地図でもあるので、入っていない供給源を消すと存在に気づけない。0 の行は薄くする。
     押すとその補正源へ移る -->
{#snippet fromOthers(rows: ExternalSource[], title: string)}
  {#if rows.length > 0}
    <div class="card">
      <div class="card-title">{title}</div>
      {#each rows as r (r.id + r.name)}
        <div class="ext-row" class:empty={r.value === 0}>
          <span class="ext-name">
            {r.name}
            {#if r.note}<span class="ext-note">{r.note}</span>{/if}
          </span>
          <!-- 0 は「−0%」「×1.00」ではなく — で出す(入っていないことを値の形で言わない) -->
          <span class="ext-value num" use:bump={() => r.value}>{r.value === 0 ? "—" : r.format(r.value)}</span>
          <button type="button" class="chip quiet" onclick={() => onOpenSource(r.id)}>開く ›</button>
        </div>
      {/each}
    </div>
  {/if}
{/snippet}

{#snippet randomOptionEditor(slot: PartSlot)}
  {@const part = selectedPartOrNull(slot)}
  {#if part === null}
    <div class="empty-note">
      <span>先にこの部位の装備を登録してください。</span>
      <button type="button" class="chip" onclick={() => onOpenSource("equipment")}>装備へ ›</button>
    </div>
  {:else}
  {#each part.random_options as option, index (option.option_id)}
    {@const def = randomOptionDef(option.option_id)}
    {#if def}
      {@const t = tierOf(def, option.rank)}
      <!-- 1 OP 1 行。名前 / ランク / 効果値 / 外す を列でそろえる(§00 01) -->
      <div class="ro-row" class:record-only={!randomOptionIsApplied(def.effect)}>
        <span class="ro-name" title={def.name}>{def.name}</span>
        <button type="button" class="clear" onclick={() => removeRandomOption(slot, index)}>外す</button>
        <!-- ランクは言葉なので幅は中身なり。ふだんは Special / S・真 だけ -->
        <span class="ro-rank">
          <StepSelect
            label=""
            options={rankOptionsNow(def, option.rank)}
            bind:value={
              () => option.rank,
              (v) => setRandomOptionRank(slot, index, v as RandomOptionRank)
            }
          />
          {#if hasLowerRanks(def) && MAIN_RANKS.includes(option.rank)}
            <button
              type="button"
              class="chip quiet"
              class:on={rankAllOpen}
              onclick={() => (rankAllOpen = !rankAllOpen)}
            >{rankAllOpen ? "上位だけ" : "下位も"}</button>
          {/if}
        </span>
        <StatInput
          label=""
          min={t ? t.min : 0}
          max={t ? t.max : limits.random_option_value_max}
          step={t && Number.isInteger(t.min) && Number.isInteger(t.max) ? 1 : 0.5}
          format={t ? () => `wiki ${t.min}–${t.max}` : undefined}
          bind:value={() => randomOptionValue(option, def), (v) => (option.value = v)}
          stepper
        />
      </div>
      {#if def.note}<p class="hint dim ro-note">{def.note}</p>{/if}
    {/if}
  {/each}
  {/if}
{/snippet}

<!-- 称号候補。依存違いだけの変種は 1 行にまとめ、選ぶのに必要な効果値を同じ行に出す。 -->
{#snippet titleRows(groups: { base: string; items: TitleDef[] }[])}
  {#each groups as g (g.base)}
    {#if g.items.length === 1}
      {@const t = g.items[0]}
      <button
        type="button"
        class="item-row"
        class:on={draft.equipment.title === t.id}
        onclick={() => (draft.equipment.title = t.id)}
      >
        <span class="item-name">{t.name}</span>
        {#if t.attack_damage_percent > 0}
          <span class="title-dmg num">ダメ +{t.attack_damage_percent}%</span>
        {:else if t.note.includes("追加ダメージ")}
          <span class="title-extra">条件付き追加ダメ</span>
        {/if}
      </button>
    {:else}
      {@const picked = g.items.find((t) => t.id === draft.equipment.title) ?? null}
      <div class="item-row group" class:on={picked !== null}>
        <span class="item-name">{g.base}</span>
        {#if g.items[0].attack_damage_percent > 0}
          <span class="title-dmg num">ダメ +{g.items[0].attack_damage_percent}%</span>
        {:else if g.items.some((t) => t.note.includes("追加ダメージ"))}
          <span class="title-extra">条件付き追加ダメ</span>
        {/if}
        <span class="title-variants">
          {#each g.items as t (t.id)}
            <button
              type="button"
              class="chip"
              class:on={draft.equipment.title === t.id}
              title="{t.name} — {titleSummary(t)}"
              onclick={() => (draft.equipment.title = t.id)}
            >{t.name.slice(g.base.length + 3)}</button>
          {/each}
        </span>
      </div>
    {/if}
  {/each}
{/snippet}

{#key sourceId}
<div class="pane pane-in">
  <div class="pane-head">
    <span class="pane-title">{TITLES[sourceId].title}</span>
    <span class="dim">{TITLES[sourceId].note}</span>
  </div>

  {#if previewError}<p class="preview-error">{previewError}</p>{/if}

  {#if sourceId === "status"}
    <div class="card">
      <div class="fields">
        <label class="text">
          <span class="label">名前</span>
          <input type="text" bind:value={draft.name} maxlength="32" placeholder="表示名" />
        </label>
        <!-- キャラは登録のときに決めて、ふだんは変えない。いまのキャラだけ出して、
             変えるときに顔を並べる(§00 02)。名前はアイコンに必ず併記する -->
        <div class="wide">
          <span class="label">キャラ</span>
          <div class="char-now">
            <Icon kind="character" id={draft.gameCharacterId} size={40} label={gameCharacterName} />
            <span class="char-name">{gameCharacterName}</span>
            <button type="button" class="chip quiet" class:on={charPickOpen} onclick={() => (charPickOpen = !charPickOpen)}>
              {charPickOpen ? "閉じる" : "変更"}
            </button>
          </div>
          {#if charPickOpen}
            <div class="pick-grid open-in">
              {#each app.gameCharacters as c (c.id)}
                <button
                  type="button"
                  class="pick"
                  class:on={c.id === draft.gameCharacterId}
                  onclick={() => { setGameCharacterId(c.id); charPickOpen = false; }}
                >
                  <Icon kind="character" id={c.id} size={40} label={c.name} />
                  <span class="pick-name">{c.name}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
        <!-- エタの意志は覚醒 5 の先にあるもの。**選んだ時点で覚醒は 5 で確定する**ので、
             覚醒より先に置く(§00 01 決める順に並べる) -->
        <div class="wide">
          <span class="label">エタの意志 Lv</span>
          <div class="eternal-row">
            <StatInput
              label=""
              min={0}
              max={limits.eternal_level_max}
              bind:value={
                () => Number(draft.eternalLevel),
                (v) => setEternalLevel(String(v))
              }
            />
            <StepSelect
              label=""
              options={eternalMilestoneOptions}
              cols={eternalMilestoneOptions.length}
              bind:value={() => draft.eternalLevel, setEternalLevel}
            />
          </div>
          <p class="hint dim">節目(20 / 40 / 60 / 80 / 90)を超えると、ダメージ上限・防御力上限・能力値上限の伸びが一段上がります。Lv を入れると覚醒は 5 段階になります。</p>
        </div>
        <!-- 覚醒段階は 4 と 5 しか使わない。それ以外は開いたときだけ出す(§00 02) -->
        <div class="stage-field wide">
          <span class="label">覚醒段階</span>
          <div class="stage-row">
            <StepSelect
              label=""
              options={stageAllOpen || stageIsLow ? stageOptions : stageMainOptions}
              cols={stageAllOpen || stageIsLow ? stageOptions.length : stageMainOptions.length}
              bind:value={draft.stage}
            />
            {#if !stageIsLow}
              <button type="button" class="chip quiet" class:on={stageAllOpen} onclick={() => (stageAllOpen = !stageAllOpen)}>
                {stageAllOpen ? "4 / 5 だけ" : "それ以外"}
              </button>
            {/if}
          </div>
        </div>
        <!-- 主軸に選ばれるのはほぼ火力上位。3 つを候補に出し、それ以外は開いたときだけ -->
        <div class="wide">
          <span class="label">主軸スキル</span>
          <div class="skill-row">
            {#each topSkills as sk (sk.id)}
              <!-- スキルは名前だけでは選べない。単 / 範・段数・属性を名前の隣に出す。
                   対象指定が wiki と突き合わせできていないものは `?`(0 や「単体」で埋めない) -->
              <button
                type="button"
                class="chip skill-chip"
                class:on={draft.mainSkillId === sk.id}
                onclick={() => (draft.mainSkillId = sk.id)}
              >
                <Icon kind="skill" id={sk.id} size={20} label={sk.name} />
                <span class="skill-name">{sk.name}</span>
                <span class="skill-meta num" class:unknown={sk.target === null}>
                  {sk.target === null ? "?" : sk.target === "single" ? "単" : "範"}
                </span>
                <span class="skill-meta num">{sk.hit_count} 段</span>
                <span class="skill-meta num elem-{sk.element}">{ELEMENT_LABELS[sk.element]}</span>
              </button>
            {/each}
            {#if skills.length > topSkills.length}
              <button
                type="button"
                class="chip quiet"
                class:on={skillListOpen || skillPickedOutside}
                onclick={() => (skillListOpen = !skillListOpen)}
              >ほかのスキル</button>
            {/if}
          </div>
          {#if skillListOpen || skillPickedOutside}
            <!-- open-in は overflow: hidden なので、重ねて出す候補が切れる。
                 ここは面が現れるだけなので swap-in(§10 型 3b) -->
            <div class="skill-all swap-in">
              <Picker
                options={mainSkillOptions}
                note="火力の高い順(倍率 × 段数)"
                placeholder="スキルを選ぶ"
                bind:value={draft.mainSkillId}
              />
            </div>
          {/if}
        </div>
        <!-- 属性はふつう主軸スキルで決まる。無属性のときだけ「何を乗せるか」を選ばせる -->
        <div class="wide">
          <span class="label">属性</span>
          {#if elementFromSkill && !elementPickOpen}
            <p class="element-auto">
              <b>{ELEMENT_LABELS[skillElement!]}</b>
              <span class="dim">— 主軸スキル「{mainSkill?.name}」で決まります</span>
              <button type="button" class="chip quiet" onclick={() => (elementPickOpen = true)}>別の属性を乗せる</button>
            </p>
          {:else}
            {#if skillElement === "neutral"}
              <p class="hint dim">主軸スキルが無属性なので、アンプルなどで乗せる属性を選びます。</p>
            {/if}
            <StepSelect
              label=""
              options={elementOptions}
              cols={elementOptions.length}
              tone={(v) => (v === "" ? undefined : `elem-${v}`)}
              bind:value={() => mainElement, setMainElement}
            />
            <p class="hint dim">ペット・カード・ルーン・アビリティの +{elementSourceTotal} をまとめて乗せます。</p>
          {/if}
        </div>
      </div>
      {#if elementPreview}
        <p class="hint dim">
          属性値
          {#each ELEMENTS.filter((e) => elementPreview!.total[e] > 0) as e (e)}
            <b>{ELEMENT_LABELS[e]} {fmtInt(elementPreview.total[e])}</b>
            <span class="dim">(キャラ {fmtInt(elementPreview.base[e])} + 装備 {fmtInt(elementPreview.equipment[e])} + 主属性 {fmtInt(elementPreview.sources[e])})</span>
          {:else}
            まだどの属性も乗っていません
          {/each}
          。与ダメージに効くのは<b>攻撃側 − 敵</b>の差で、差 +1 ごとに +0.625%、+80 で上限 +50%(敵は 120 / 125)。
        </p>
      {/if}
      <p class="hint dim">
        {#if skills.length === 0}
          このキャラのスキルはまだ未収録です。収録されるまで攻撃力は出せません。
        {:else if draft.mainSkillId === ""}
          主軸スキルを選ぶと攻撃力が出ます。スキルの依存種別(突き / 斬り / 魔攻 / 魔防 / 複合)で装備の係数が変わるためです。
        {:else}
          攻撃力はこのスキルの依存種別で計算します。ダメージ計算タブは選んだスキルごとに計算します。
        {/if}
      </p>
    </div>
    <div class="card">
      <div class="card-title">能力値 <span class="dim normal">設定を触ると即時更新</span></div>
      <div class="tbl">
        <table class="grid">
          <thead><tr><th>ステ</th><th class="n">素</th><th class="n">補正</th><th>素ステ → 最終</th><th class="n">最終</th></tr></thead>
          <tbody>
            {#each STAT_KINDS as k (k)}
              {@const trace = traceFor(k)}
              {@const diff = preview ? preview.stats[k] - draft.baseStats[k] : null}
              {@const cap = trace?.stat_cap ?? 0}
              {@const basePct = cap > 0 ? Math.min(100, (draft.baseStats[k] / cap) * 100) : 0}
              {@const addPct = cap > 0 && diff !== null ? Math.max(0, Math.min(100 - basePct, (diff / cap) * 100)) : 0}
              <tr>
                <td>{STAT_LABELS[k]}</td>
                <td class="n stat-cell">
                  <StatInput label="" min={STAT_MIN} max={limits.base_stat_max} bind:value={draft.baseStats[k]} />
                </td>
                <td class="n muted ro">{diff === null ? "—" : signed(diff)}</td>
                <!-- 素ステ → 最終を 1 本のバーで(§11)。数字の羅列ではなく「どれだけ伸びたか」を見せる。
                     灰が素ステ(振り分け)、青が補正で乗った分。長さは最終能力値の上限に対する割合 -->
                <td class="ro">
                  <span
                    class="grow"
                    title={cap > 0 ? `上限 ${fmtInt(cap)}(覚醒段階 + エタの意志 Lv)` : "上限は計算中"}
                  >
                    <i class="base" style="width: {basePct.toFixed(1)}%"></i>
                    <i class="add" style="width: {addPct.toFixed(1)}%"></i>
                  </span>
                </td>
                <td class="n final ro">
                  <span class="strong">{preview ? fmtInt(preview.stats[k]) : "—"}</span>
                  {#if trace?.pinned_from !== null && trace?.pinned_from !== undefined}
                    <span class="pin-badge" title={`固定前: ${fmtInt(trace.pinned_from)}`}>固定</span>
                  {/if}
                  <!-- 「満」の枠は常に確保する。出たときに行がずれない(§09 規則 4 / §11) -->
                  <span
                    class="cap-badge"
                    class:on={trace !== null && trace !== undefined && trace.capped_loss > 0}
                    title={trace && trace.capped_loss > 0
                      ? `上限 ${fmtInt(trace.stat_cap)} で ${fmtInt(trace.capped_loss)} 捨てています。上限は覚醒段階とエタの意志 Lv で上がります`
                      : ""}
                  >{trace && trace.capped_loss > 0 ? "満" : ""}</span>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <details class="contrib">
        <summary>補正の内訳 <span class="dim">{preview ? preview.contributions.length : 0} 件</span></summary>
        {#if !preview || preview.contributions.length === 0}
          <p class="empty dim">補正源なし(素ステのみ)</p>
        {:else}
          <div class="tbl">
            <table class="grid ro">
              <thead><tr><th>ステ</th><th>出典</th><th>層</th><th class="n">値</th></tr></thead>
              <tbody>
                {#each STAT_KINDS.flatMap((k) => preview!.contributions.filter((c) => c.kind === k)) as c, i (i)}
                  <tr>
                    <td>{STAT_LABELS[c.kind]}</td>
                    <td class="muted">{c.source}</td>
                    <td class="muted">{STAT_LAYER_LABELS[c.layer]}</td>
                    <td class="n">{formatLayerValue(c.layer, c.value)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </details>
    </div>
  {:else if sourceId === "equipment"}
    <!-- ドリルダウンは置き換えではなく、右にペインを足す(§09 規則 2)。押した部位行は
         その場に残り、別の部位を押せばそのまま横に移れる — 戻るのに「戻る」が要らない -->
    <div class="part-split" class:open={openPart !== null}>
      <div class="part-list">
    {#each PART_SLOTS as slot (slot)}
      {@const part = selectedPartOrNull(slot)}
      {@const list = draft.equipment.parts[slot]}
      {@const canEnhance = ENHANCE_ALLOWED_SLOTS.includes(slot)}
      {@const damageLabel = itemDamageLabel(equippedItem(slot), true)}
      <button type="button" class="part-row" class:on={openPart === slot} onclick={() => openPartDetail(slot)}>
        <Icon kind="equipment" id={part?.item_id ?? null} size={28} label={partDisplayName(slot)} />
        <span class="part-main">
          <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
          <span class="part-item">{partDisplayName(slot)}</span>
          <span class="part-abi" use:bump={() => list.registered.length}>登録 {list.registered.length}</span>
          <!-- 強化バッジの枠は常に確保する。出ても行の中身がずれない(§12) -->
          {#if canEnhance}
            <span class="part-plus" class:on={(part?.enhance_level ?? 0) > 0}
            >{(part?.enhance_level ?? 0) > 0 ? `+${part!.enhance_level}` : ""}</span>
          {/if}
          {#if (part?.abilities.length ?? 0) > 0}
            <span class="part-abi">アビリティ {part!.abilities.length}</span>
          {/if}
          <!-- 装着時効果は装備補正値の列に出ないので、行にバッジで残す(§00 ②/⑤) -->
          {#if damageLabel !== null}
            <span class="part-dmg" use:flash={() => damageLabel}>{damageLabel}</span>
          {/if}
          {#if (part?.random_options.length ?? 0) > 0}
            <span class="part-abi">OP {part!.random_options.length}</span>
          {/if}
        </span>
        <span class="part-vals num dim">{part ? valuesSummary(part.base) : "—"}</span>
        <span class="chev dim">›</span>
      </button>
      {#if list.registered.length > 1}
        <div class="part-switches">
          {#each list.registered as registered, index (registered.id)}
            <button
              type="button"
              class:on={registered.id === list.selected_id}
              class:dragging={draggedEquipmentRegistration?.slot === slot && draggedEquipmentRegistration.id === registered.id}
              class:drop-before={equipmentRegistrationDropAt?.slot === slot && equipmentRegistrationDropAt.index === index}
              class:drop-after={equipmentRegistrationDropAt?.slot === slot && equipmentRegistrationDropAt.index === index + 1 && index === list.registered.length - 1}
              draggable="true"
              onclick={() => selectEquipmentRegistration(slot, registered.id)}
              ondragstart={(event) => startEquipmentRegistrationDrag(event, slot, registered.id)}
              ondragover={(event) => dragEquipmentRegistrationOver(event, slot, index)}
              ondrop={(event) => dropEquipmentRegistration(event, slot)}
              ondragend={() => { draggedEquipmentRegistration = null; equipmentRegistrationDropAt = null; }}
            >
              <span class="registration-grip" aria-hidden="true">⠿</span>
              <Icon kind="equipment" id={registered.item_id} size={20} label={registered.label || `装備 ${registered.id}`} />
              {registered.label || app.equipmentCatalog.find((i) => i.id === registered.item_id)?.name || `装備 ${registered.id}`}
            </button>
          {/each}
        </div>
      {/if}
    {/each}
      </div>
      {#if openPart !== null}
        {@const slot = openPart}
        {@const part = selectedPartOrNull(slot)}
        {@const item = equippedItem(slot)}
        {@const contribution = partContribution(slot)}
        <div class="equipment-overlay modal-overlay" role="presentation">
        <div class="part-detail modal-surface pane-in" role="dialog" aria-modal="true" aria-label={`${openPartLabel}の装備登録`}>
        <div class="part-detail-header">
          <b>{openPartLabel}の装備登録</b>
          <button type="button" class="btn close-equipment" onclick={() => (openPart = null)}>閉じる <span aria-hidden="true">×</span></button>
        </div>
        {#if draft.equipment.parts[slot].registered.length > 1}
          <div class="part-switches registration-order" aria-label="装備登録の並び順">
            {#each draft.equipment.parts[slot].registered as registered, index (registered.id)}
              <button
                type="button"
                class:on={registered.id === draft.equipment.parts[slot].selected_id}
                class:dragging={draggedEquipmentRegistration?.slot === slot && draggedEquipmentRegistration.id === registered.id}
                class:drop-before={equipmentRegistrationDropAt?.slot === slot && equipmentRegistrationDropAt.index === index}
                class:drop-after={equipmentRegistrationDropAt?.slot === slot && equipmentRegistrationDropAt.index === index + 1 && index === draft.equipment.parts[slot].registered.length - 1}
                draggable="true"
                onclick={() => selectEquipmentRegistration(slot, registered.id)}
                ondragstart={(event) => startEquipmentRegistrationDrag(event, slot, registered.id)}
                ondragover={(event) => dragEquipmentRegistrationOver(event, slot, index)}
                ondrop={(event) => dropEquipmentRegistration(event, slot)}
                ondragend={() => { draggedEquipmentRegistration = null; equipmentRegistrationDropAt = null; }}
              >
                <span class="registration-grip" aria-hidden="true">⠿</span>
                <Icon kind="equipment" id={registered.item_id} size={20} label={registered.label || `装備 ${registered.id}`} />
                {registered.label || app.equipmentCatalog.find((item) => item.id === registered.item_id)?.name || `装備 ${registered.id}`}
              </button>
            {/each}
          </div>
        {/if}
        {#if part === null}
          <div class="card empty"><p class="hint dim">この部位にはまだ装備が登録されていません。</p>
            <button type="button" class="btn primary" onclick={() => createEquipmentRegistration(slot)}>＋ 新しい装備を登録</button>
          </div>
        {:else}
        <div class="part-actions">
          <span class="editing-registration badge">編集中: {part.label || `装備 ${part.id}`}</span>
          <button type="button" class="btn" onclick={() => createEquipmentRegistration(slot)}>＋ 新しい装備を登録</button>
          <button
            type="button"
            class="btn delete-registration"
            class:confirm={confirmEquipmentDeleteId === part.id}
            onclick={() => removeSelectedEquipmentRegistration(slot)}
          >{confirmEquipmentDeleteId === part.id ? "もう一度押すと削除します" : "この登録を削除"}</button>
        </div>
        <div class="card registration-name-card">
          <label class="text custom-name">
            <span class="label">登録名 <span class="dim">同じ装備を複数持つときの見分け方</span></span>
            <input type="text" bind:value={part.label} maxlength="40" placeholder="例: ボス用" />
          </label>
        </div>

        <div class="card equipment-choice-card">
          <div class="selected-equipment">
            <Icon kind="equipment" id={part.item_id} size={28} label={partDisplayName(slot)} />
            <span class="selected-equipment-copy" use:flash={() => partDisplayName(slot)}>
              <small class="dim">選択中の装備</small>
              <b>{partDisplayName(slot)}</b>
            </span>
            {#if contribution !== null}<span class="contrib-inline num" use:flash={() => String(contribution)}>寄与 {fmtInt(contribution)}</span>{/if}
            <button type="button" class="btn" onclick={() => (itemPickerOpen = !itemPickerOpen)}>
              {itemPickerOpen ? "候補を閉じる" : "装備を変更"}
            </button>
          </div>
          {#if itemPickerOpen}
            <div class="equipment-picker pane-in">
              {#if isRelicSlot(slot)}
                <div class="relic-selector">
                  <StepSelect
                    label="種別"
                    options={relicKindOptions}
                    full
                    bind:value={() => relicKindFor(slot), (value) => pickRelicKind(slot, value)}
                  />
                  <StepSelect
                    label="強化段階"
                    options={relicLevelOptions}
                    cols={5}
                    disabled={relicKindFor(slot) === ""}
                    bind:value={() => relicLevelFor(slot), (value) => pickRelicLevel(slot, value)}
                  />
                  <div class="relic-picker-actions">
                    <button type="button" class="chip quiet" onclick={() => pickUnequipped(slot)}>未装備</button>
                    <button type="button" class="chip quiet" onclick={() => pickCustom(slot)}>カタログ外</button>
                  </div>
                </div>
              {:else}
              <div class="picker-tools">
                <input class="item-search" type="text" placeholder="装備名で探す" bind:value={itemQuery} />
                {#if equipmentFilterLabel !== null}
                  <span class="equipment-filter badge">{equipmentFilterLabel}</span>
                  <button type="button" class="chip quiet" onclick={() => (showAllEquipmentCandidates = !showAllEquipmentCandidates)}>
                    {showAllEquipmentCandidates ? "候補だけ見る" : "すべて見る"}
                  </button>
                {/if}
              </div>
              <div class="item-list" class:effectful={slot === "artifact"}>
                <button type="button" class="item-row" class:on={part.item_id === null && part.custom_name === null} onclick={() => pickUnequipped(slot)}>
                  <Icon kind="equipment" id={null} size={28} label="未装備" />
                  <span class="item-copy"><span class="item-name">未装備</span></span>
                </button>
                {#each filteredCatalog as candidate (candidate.id)}
                  <!-- 候補カードは名前の識別が主目的。カテゴリ名は詳細へ譲り、短い効果量だけを置く -->
                  {@const candidateDamage = itemDamageLabel(candidate, true)}
                  <button type="button" class="item-row" class:on={part.item_id === candidate.id} onclick={() => pickCatalogItem(slot, candidate)}>
                    <Icon kind="equipment" id={candidate.id} size={28} label={candidate.name} />
                    <span class="item-copy">
                      <span class="item-name">{candidate.name}</span>
                      <span class="item-vals num dim">{rangeSummary(candidate.values_min, candidate.values_max)}</span>
                    </span>
                    {#if candidateDamage !== null}<span class="part-dmg">{candidateDamage}</span>{/if}
                  </button>
                {/each}
                <button type="button" class="item-row" class:on={part.item_id === null && part.custom_name !== null} onclick={() => pickCustom(slot)}>
                  <Icon kind="equipment" id={null} size={28} label="カスタム" />
                  <span class="item-copy"><span class="item-name">カスタム</span><span class="item-vals dim">カタログ外</span></span>
                </button>
              </div>
              {/if}
              {#if part.item_id === null && part.custom_name !== null}
                <label class="text custom-name">
                  <span class="label">装備名 <span class="dim">[仮] カタログ外</span></span>
                  <input type="text" bind:value={part.custom_name} maxlength="40" placeholder="装備名" />
                </label>
              {/if}
            </div>
          {/if}
        </div>

        {#if part.item_id !== null || part.custom_name !== null}
        {#key item?.id ?? "custom"}
        <div class="equipment-item-content swap-in">
        {#if item?.growth_caps}
        <div class="card growth-equipment-card">
          <div class="card-title inline growth-equipment-head">
            <span>装備補正</span>
            <span class="badge num">成長値</span>
          </div>
          <p class="hint dim">この段階の実値を入力します。下限は直前段階の完成値、上限はこの段階のMAXです。エンチャント枠はありません。</p>
          <div class="stat-rows growth-equipment-values">
            {#each EQUIPMENT_STAT_KINDS.filter((k) => item.growth_caps![k] > 0) as k (k)}
              <div class="stat-row">
                <span class="k">{EQUIPMENT_STAT_LABELS[k]}</span>
                <StatInput
                  label="{EQUIPMENT_STAT_LABELS[k]}の装備補正"
                  hideLabel
                  min={item.values_min[k]}
                  max={item.growth_caps[k]}
                  strictMax
                  stepper
                  presets={item.id === "rising-holic-cuffs" ? [{ value: 140, label: "140" }] : []}
                  bind:value={part.base[k]}
                />
              </div>
            {/each}
          </div>
        </div>
        {:else}
        {@const enchantPlanStats = enchantPlanStatsFor(item)}
        <div class="card enchant-card">
          <div class="card-title inline">
            <span>エンチャント</span>
          </div>
          <p class="hint dim">通常は突き・斬り・魔攻・魔防の4補正だけ入力します。</p>
          <div class="base-value-toolbar">
            <span class="base-value-copy" title={item === null ? "カタログ外のため入力" : "数値を押すと例外編集"}><b>装備本体</b><small>{item === null ? "入力" : "自動"}</small></span>
          </div>
          <div class="values-paired enchant-first">
            {#each visibleEquipmentStats as k, index (k)}
              {@const cap = item ? item.enchant_caps[k] : limits.equipment_value_max}
              {@const abilityValue = partAbilityValues(slot)[k]}
              {@const displayTotal = part.base[k] + part.enchant[k] + abilityValue}
              {@const completionPlan = enchantCompletionPlan(cap - part.enchant[k])}
              <div
                class="value-pair"
                class:plan-stat={enchantPlanStats.includes(k)}
                class:secondary-stat={!PRIMARY_EQUIPMENT_STATS.includes(k)}
                transition:slide={{ duration: PRIMARY_EQUIPMENT_STATS.includes(k) ? 0 : 220 }}
              >
                <b>{EQUIPMENT_STAT_SHORT[k]}</b>
                <div class="value-equation">
                  <strong class="value-total num" use:bump={() => displayTotal}>
                    <span class="total-main">{displayTotal}</span>
                    <span class="enchant-part">（＋{part.enchant[k]}）</span>
                  </strong>
                  {#if abilityValue !== 0}
                    <span class="ability-part">アビ{abilityValue}</span>
                  {:else}
                    <span class="ability-spacer" aria-hidden="true"></span>
                  {/if}
                  <div class="equation-enchant">
                    <StatInput label="{EQUIPMENT_STAT_LABELS[k]}のエンチャント" hideLabel min={0} max={cap} strictMax={item !== null} increments={[12, 14, 17, 20]} bind:value={part.enchant[k]} />
                  </div>
                  <div class="equation-base">
                    <StatInput label="{EQUIPMENT_STAT_LABELS[k]}の装備本体補正" hideLabel min={0} max={item?.growth_cap ?? limits.equipment_value_max} gauge={false} readAsText={item !== null} bind:value={part.base[k]} />
                  </div>
                </div>
                {#if item !== null && enchantPlanStats.includes(k)}
                  <div
                    class="enchant-plan"
                    class:complete={completionPlan.remaining === 0}
                    use:flash={() => `${completionPlan.remaining}:${completionPlan.twentyCount}:${completionPlan.seventeenCount}:${completionPlan.remainder}`}
                  >
                    <span class="plan-remaining"><small>上限まであと</small><b class="num">{completionPlan.remaining}</b></span>
                    {#if completionPlan.remaining > 0}
                      <span class="plan-recipe num">{enchantCompletionLabel(completionPlan)}</span>
                      <span class="badge num">{completionPlan.count}回</span>
                    {:else}
                      <span class="plan-recipe">強化完了</span>
                      <span class="badge">MAX</span>
                    {/if}
                  </div>
                {/if}
              </div>
              {#if index === PRIMARY_EQUIPMENT_STATS.length - 1}
                <button
                  type="button"
                  class="enchant-more-toggle"
                  aria-expanded={showOtherEquipmentStats}
                  onclick={() => (showOtherEquipmentStats = !showOtherEquipmentStats)}
                >
                  <span><b>物防・命中など5補正</b><small>物防 / 命中 / Cri / 回避 / 敏捷</small></span>
                  <span class="toggle-state">{showOtherEquipmentStats ? "閉じる ︿" : "開く ﹀"}</span>
                </button>
              {/if}
            {/each}
          </div>
          <p class="hint dim">シエナのオーラとテシスコアは各専用欄から自動合流します。</p>
        </div>
        {/if}

        {#if ABILITY_ALLOWED_SLOTS.includes(slot) && currentAbilitySlotCount(slot) > 0}
          <div class="card ability-card">
            <div class="card-title inline">
              <span>アビリティ</span><span class="badge">{part.abilities.length} / {currentAbilitySlotCount(slot)}</span>
              <strong class="ability-impact num" use:flash={() => abilityImpactSummary(slot)}>{abilityImpactSummary(slot)}</strong>
            </div>
            {#if slot === "weapon"}
            <p class="hint dim">ゲーム内の3枠と同じ順です。装備中の武器系統に合う候補を押して選びます。</p>

            <div class="ability-fixed-list">
              {#each [
                { category: 1, label: "基本補正", note: "従来アビリティ" },
                { category: 4, label: "新装着", note: "追加効果2枠" },
                { category: 3, label: "武器ディレイ", note: "任意・計算対象外" },
              ] as row (row.category)}
                {@const selectedAbilityId = abilityIdForCategory(slot, row.category)}
                {@const selectedAbility = abilityDef(selectedAbilityId)}
                <div class="ability-fixed-row">
                  <div class="ability-fixed-label">
                    <b>{row.label}</b>
                    <span>{row.note} ・ カテゴリ{row.category}</span>
                  </div>
                  <div class="ability-choice-list" aria-label="{row.label}の候補">
                    <button
                      type="button"
                      class:on={selectedAbilityId === ""}
                      class="chip ability-choice-none"
                      aria-pressed={selectedAbilityId === ""}
                      onclick={() => setAbilityForCategory(slot, row.category, "")}
                    >装着しない</button>
                    {#each abilityCandidates(slot, row.category) as ability (ability.id)}
                      <button
                        type="button"
                        class:on={selectedAbilityId === ability.id}
                        class:record-only={ability.record_only}
                        class="chip ability-choice"
                        aria-pressed={selectedAbilityId === ability.id}
                        onclick={() => setAbilityForCategory(slot, row.category, ability.id)}
                      >
                        <span>{ability.name}</span>
                        <span class="ability-choice-effect num">{ability.effect_summary}</span>
                      </button>
                    {/each}
                  </div>
                </div>

                {#if row.category === 4 && selectedAbility?.additional_slots}
                  <div class="ability-additional-panel swap-in">
                    <div class="ability-additional-head">
                      <b>ランダム追加</b>
                      <span class="badge">{additionsFor("weapon", selectedAbility.id).length} / 2</span>
                      <span class="dim">付いている種類を押して足し、実測値を合わせます</span>
                    </div>
                    {#if additionsFor("weapon", selectedAbility.id).length < 2}
                      <div class="ro-add-row ability-additional-candidates">
                        {#each addableAdditionalOptions("weapon", selectedAbility.id) as option (option.kind)}
                          <button type="button" class="chip add" onclick={() => addAdditional("weapon", selectedAbility.id, option.kind)}>
                            ＋ {additionalKindLabel(option.kind)}
                            <span class="num dim">
                              {additionalRangeLabel(option)}
                            </span>
                          </button>
                        {/each}
                      </div>
                    {/if}
                    {#each additionsFor("weapon", selectedAbility.id) as additional, additionalIndex (`${additional.kind}-${additionalIndex}`)}
                      {@const additionalDef = selectedAbility.additional_options.find((option) => option.kind === additional?.kind)}
                      {#if additionalDef}
                        <div class="siena-row swap-in">
                          <span class="ro-name">{additionalKindLabel(additional.kind)}</span>
                          <StatInput
                            label="{additionalKindLabel(additional.kind)}の値"
                            hideLabel
                            min={additionalDef.min}
                            max={additionalDef.max}
                            gauge={false}
                            stepper
                            bind:value={() => additional.value, (value) => setAdditionalValue("weapon", selectedAbility.id, additionalIndex, value)}
                          />
                          <button type="button" class="clear" onclick={() => removeAdditional("weapon", selectedAbility.id, additionalIndex)}>外す</button>
                        </div>
                      {/if}
                    {/each}
                    <span class="additional-note dim">固定ダメージ・割合・攻撃補正・命中を保存し、計算へ反映します。</span>
                  </div>
                {/if}
              {/each}
            </div>
            {:else}
              <p class="hint dim">
                {currentAbilitySlotCount(slot)}枠まで装着できます。範囲値とランダム追加は選択後に実測値を合わせます。
              </p>
              <div class="ability-choice-list non-weapon-ability-list" aria-label="{PART_SLOT_LABELS[slot]}アビリティの候補">
                {#each nonWeaponAbilityCandidates(slot) as ability (ability.id)}
                  {@const selected = part.abilities.includes(ability.id)}
                  <button
                    type="button"
                    class:on={selected}
                    class:record-only={ability.record_only}
                    class="chip ability-choice"
                    aria-pressed={selected}
                    disabled={!selected && part.abilities.length >= currentAbilitySlotCount(slot)}
                    onclick={() => toggleNonWeaponAbility(slot, ability)}
                  >
                    <span>{ability.name}</span>
                    <span class="ability-choice-effect num">{ability.effect_summary}</span>
                  </button>
                {/each}
              </div>
              {#each part.abilities as abilityId (abilityId)}
                {@const ability = abilityDef(abilityId)}
                {#if ability?.value_option}
                  <div class="siena-row ability-value-row swap-in">
                    <span class="ro-name">{ability.name}</span>
                    <StatInput
                      label="{ability.name}の実測値"
                      hideLabel
                      min={ability.value_option.min}
                      max={ability.value_option.max}
                      gauge={false}
                      stepper
                      bind:value={() => abilityRecordedValue(slot, ability.id), (value) => setAbilityRecordedValue(slot, ability.id, value)}
                    />
                    <span class="num dim">/{ability.value_option.max}</span>
                  </div>
                {/if}
              {/each}
              {#each part.abilities as abilityId (`addition-${abilityId}`)}
                {@const ability = abilityDef(abilityId)}
                {#if ability && ability.additional_slots > 0}
                  <div class="ability-additional-panel non-weapon-additional-panel swap-in">
                    <div class="ability-additional-head">
                      <b>{ability.name}のランダム追加</b>
                      <span class="badge">{additionsFor(slot, ability.id).length} / {ability.additional_slots}</span>
                    </div>
                    {#if additionsFor(slot, ability.id).length < ability.additional_slots}
                      <div class="ro-add-row ability-additional-candidates">
                        {#each addableAdditionalOptions(slot, ability.id) as option (option.kind)}
                          <button type="button" class="chip add" onclick={() => addAdditional(slot, ability.id, option.kind)}>
                            ＋ {additionalKindLabel(option.kind)}
                            <span class="num dim">{additionalRangeLabel(option)}</span>
                          </button>
                        {/each}
                      </div>
                    {/if}
                    {#each additionsFor(slot, ability.id) as additional, additionalIndex (`${additional.kind}-${additionalIndex}`)}
                      {@const option = ability.additional_options.find((candidate) => candidate.kind === additional.kind)}
                      {#if option}
                        <div class="siena-row swap-in">
                          <span class="ro-name">{additionalKindLabel(additional.kind)}</span>
                          <StatInput
                            label="{additionalKindLabel(additional.kind)}の実測値"
                            hideLabel min={option.min} max={option.max} gauge={false} stepper
                            bind:value={() => additional.value, (value) => setAdditionalValue(slot, ability.id, additionalIndex, value)}
                          />
                          <button type="button" class="clear" onclick={() => removeAdditional(slot, ability.id, additionalIndex)}>外す</button>
                        </div>
                      {/if}
                    {/each}
                  </div>
                {/if}
              {/each}
              {#if nonWeaponAbilityCandidates(slot).some((ability) => ability.record_only)}
                <span class="additional-note dim">破線の候補は効果を保存しますが、現在の計算項目にない値は合計へ加えません。</span>
              {/if}
            {/if}
          </div>
        {/if}

        {#if ELEMENT_ALLOWED_SLOTS.includes(slot)}
          <p class="hint dim">属性強化はキャラで選択中の属性を自動で +9 反映します。</p>
        {/if}

        {#if ENHANCE_ALLOWED_SLOTS.includes(slot)}
          <div class="card">
            <div class="card-title">装備強化</div>
            {#if part.item_id === null && part.custom_name !== null}
              <Select
                label="装備種別"
                options={slot === "weapon" ? weaponEnhanceTypeOptions : armorEnhanceTypeOptions}
                bind:value={() => part.enhance_type ?? "", (v) => (part.enhance_type = v === "" ? null : v as typeof part.enhance_type)}
              />
              <p class="hint dim">固定ダメージの補正式にだけ使います。</p>
            {/if}
            <StepSelect
              label="強化 Lv"
              options={enhanceLevelOptions}
              bind:value={() => String(part.enhance_level), (v) => setEnhanceLevel(slot, Number(v))}
            />
            {#if part.enhance_level > 0 && part.enhance_type === null}
              <p class="preview-error">装備種別を選ぶと固定ダメージを計算できます。</p>
            {/if}
            {#if part.enhance_level >= 12}
              <StepSelect
                label="等級"
                options={enhanceGradeOptions}
                bind:value={() => part.enhance_grade ?? "highest", (v) => (part.enhance_grade = v as typeof part.enhance_grade)}
              />
              <p class="hint dim">等級内の上限値を使用します。倍率の端数は四捨五入します。</p>
            {:else if part.enhance_level > 0}
              {#if part.enhance_type !== null || item}
                <p class="hint dim">追加固定ダメージは自動計算されます(ダメージ計算タブのトレースに表示)。</p>
              {:else}
                <p class="hint dim">装備種別を選ぶと追加固定ダメージを自動計算します。</p>
              {/if}
            {/if}
          </div>
        {/if}
  
        {#if RANDOM_OPTION_ALLOWED_SLOTS.includes(slot) && randomOptionSlots(slot) > 0}
          <div class="card">
            <div class="card-title">ランダムオプション</div>
            <p class="hint dim">登録済み {part.random_options.length}件。ランダムオプションは専用の入力エリアで設定します。</p>
          </div>
        {/if}
        </div>
        {/key}
        {/if}
  
        {/if}
        </div>
        </div>
      {/if}
    </div>
  {:else if sourceId === "pet"}
    <div class="card">
      <!-- 8 ステが同じ形で並ぶので 1 ステ 1 行。段は列を固定して行をまたいで揃える(§00 01) -->
      <div class="stat-rows">
        {#each STAT_KINDS as k (k)}
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
            <StepSelect
              label=""
              options={petSkillOptions}
              cols={petSkillOptions.length}
              bind:value={() => petSkillValue(k), (v) => setPetSkillValue(k, v)}
            />
            <span class="v num" use:bump={() => petSkillBonus(k)}>
              {petSkillBonus(k) > 0 ? `+${fmtInt(petSkillBonus(k))}` : "—"}
            </span>
          </div>
        {/each}
      </div>
    </div>
  {:else if sourceId === "rune"}
    <div class="card">
      <div class="stat-rows two">
        {#each PAIRED_STAT_KINDS as k (k)}
          <!-- Lv は段階。1 押しに意味があるので ＋ / − を置く(§07 形態 4) -->
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
            <StatInput
              label=""
              min={0}
              max={limits.rune_level_max}
              stepper
              bind:value={draft.statSources.rune_levels[k]}
            />
          </div>
        {/each}
      </div>
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
      <div class="stat-rows two">
        {#each PAIRED_STAT_KINDS as k (k)}
          <div class="stat-row" use:flash={() => String(crownMax(k))}>
            <span class="k">{STAT_LABELS[k]}</span>
            <StatInput
              label=""
              min={0}
              max={crownMax(k)}
              step={limits.crown_step}
              stepper
              bind:value={draft.statSources.crown[k]}
            />
          </div>
        {/each}
      </div>
    </div>
  {:else if sourceId === "monsterCard"}
    <div class="card">
      <p class="hint dim">
        wiki「ステータス」の固定値増加にある<b>カード装着</b>。装着したカードのステータスが
        そのまま乗ります(ステごと 0〜{limits.monster_card_max})。
        <b>固定値層</b>なので、能力値倍率A(テイルズウィーバーのエネルギー等)の影響を受けます。
      </p>
      <div class="stat-rows two">
        {#each PAIRED_STAT_KINDS as k (k)}
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
            <StatInput
              label=""
              min={0}
              max={limits.monster_card_max}
              stepper
              bind:value={draft.statSources.monster_cards[k]}
            />
          </div>
        {/each}
      </div>
    </div>
  {:else if sourceId === "relic"}
    <div class="card">
      <div class="stat-rows two">
        {#each PAIRED_STAT_KINDS as k (k)}
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
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
          </div>
        {/each}
      </div>
    </div>
  {:else if sourceId === "siena"}
    <div class="card">
      <p class="hint dim">
        wiki「装備システム/シエナのオーラ」。オーラは装備から抽出して、同じ部位の別装備へ注入できます。
        そのため装備とは別に登録し、<b>部位ごとに装着中の 1 件だけ</b>を計算へ反映します。
        中身は再抽選のランダム値なので、<b>スロットに出ているものを 1 個ずつ選んで足します</b>。
        <b>増幅段階は足したスロットの数</b>で、段階 3/7/10 で追加オプションの枠が 1/2/3 個開きます。
        効果値は触らなければレンジ上限で計算します(再抽選で振り直せるため)。
        グレーの枠は<b>記録するだけ</b>(防御側・HP/MP/SP など未収録の概念)で計算には入りません。
      </p>
    </div>
    <div class="part-list">
      {#each SIENA_ALLOWED_SLOTS as slot (slot)}
        {@const list = sienaList(slot)}
        {@const current = sienaRegistration(slot)}
        {@const siena = sienaForDisplay(slot)}
        {@const stage = sienaStage(siena)}
        {@const badges = sienaBadges(slot)}
        <button type="button" class="part-row" class:on={openSienaPart === slot} onclick={() => (openSienaPart = slot)}>
          <span class="siena-mark" class:off={current === null} aria-hidden="true">◆</span>
          <span class="part-main">
            <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
            <span class="part-item">{current?.label || (current ? `オーラ ${current.id}` : "未装着")}</span>
            <span class="part-abi" use:bump={() => list.registered.length}>登録 {list.registered.length}</span>
            <span class="part-plus wide" class:on={stage > 0}>{stage > 0 ? `${stage} 段階` : ""}</span>
          </span>
          <span class="ro-badges">
            {#each badges.slice(0, SIENA_BADGE_MAX) as b (b.key)}
              <span class="ro-badge" class:record-only={!b.modeled} title={b.title}>{b.text}</span>
            {/each}
            {#if badges.length > SIENA_BADGE_MAX}
              <span class="ro-badge more">+{badges.length - SIENA_BADGE_MAX}</span>
            {/if}
          </span>
          <span class="part-vals num dim" use:flash={() => `${list.selected_id}:${sienaSummary(slot)}`}>{sienaSummary(slot)}</span>
          <span class="chev dim">›</span>
        </button>
        {#if list.registered.length > 0}
          <div class="part-switches siena-quick-switches" aria-label={`${PART_SLOT_LABELS[slot]}のオーラ切替`}>
            <button type="button" class:on={list.selected_id === null} onclick={() => (list.selected_id = null)}>未装着</button>
            {#each list.registered as entry (entry.id)}
              <button type="button" class:on={entry.id === list.selected_id} onclick={() => (list.selected_id = entry.id)}>
                <span class="siena-mini-mark" aria-hidden="true">◆</span>{entry.label || `オーラ ${entry.id}`}
              </button>
            {/each}
          </div>
        {/if}
      {/each}
    </div>
    {#if openSienaPart !== null}
      {@const slot = openSienaPart}
      {@const list = sienaList(slot)}
      {@const registration = sienaRegistration(slot)}
      {@const siena = sienaForDisplay(slot)}
      {@const stage = sienaStage(siena)}
      {@const capacity = sienaCapacity(slot)}
      <div class="equipment-overlay modal-overlay" role="presentation">
        <div class="part-detail modal-surface pane-in" role="dialog" aria-modal="true" aria-label={`${PART_SLOT_LABELS[slot]}のシエナのオーラ`}>
          <div class="part-detail-header">
            <b>{PART_SLOT_LABELS[slot]}のシエナのオーラ</b>
            <button type="button" class="btn close-equipment" onclick={() => (openSienaPart = null)}>閉じる <span aria-hidden="true">×</span></button>
          </div>
          <div class="part-actions siena-registration-actions">
            <div class="part-switches" aria-label="装着するオーラ">
              <button type="button" class:on={list.selected_id === null} onclick={() => (list.selected_id = null)}>未装着</button>
              {#each list.registered as entry (entry.id)}
                <button type="button" class:on={entry.id === list.selected_id} onclick={() => (list.selected_id = entry.id)}>
                  <span class="siena-mini-mark" aria-hidden="true">◆</span>{entry.label || `オーラ ${entry.id}`}
                </button>
              {/each}
            </div>
            <button type="button" class="btn primary" onclick={() => createSienaRegistration(slot)}>＋ 新しいオーラを登録</button>
          </div>
          {#if registration === null}
            <div class="card empty siena-unattached">
              <p class="hint dim">この部位は未装着です。登録済みのオーラを選ぶか、新しく登録してください。</p>
            </div>
          {:else}
            <div class="card registration-name-card">
              <label class="text custom-name">
                <span class="label">登録名 <span class="dim">同じ部位のオーラを見分ける名前</span></span>
                <input type="text" bind:value={registration.label} maxlength="40" placeholder="例: 火力用" />
              </label>
              <button type="button" class="chip quiet siena-delete" onclick={() => removeSelectedSienaRegistration(slot)}>この登録を削除</button>
            </div>
            <div class="card">
              <div class="card-title inline">
                {PART_SLOT_LABELS[slot]}: 能力値スロット
                <span class="dim normal num" use:bump={() => stage}>{stage} / {app.siena.stage_max} 段階</span>
            </div>
            <!-- 足す場所は**行より上**。下に置くと、1 個足すたびに押したチップが
                 行の高さぶん下へ逃げる(§09 規則 1)。足したものは真下に増える -->
            {#if stage < app.siena.stage_max}
              <div class="ro-add-row">
                {#each sienaValueDefs(slot) as def (def.kind)}
                  <button
                    type="button"
                    class="chip add"
                    class:record-only={!def.is_modeled}
                    title={def.note}
                    onclick={() => addSienaSlot(slot, def.kind)}
                  >＋ {def.label}</button>
                {/each}
              </div>
            {:else}
              <p class="hint dim">段階 {app.siena.stage_max} まで埋まりました。変えるときは外してから足します。</p>
            {/if}
            {#each siena.slots as s, index (index)}
              {@const def = sienaValueDef(s.kind)}
              {#if def}
                <!-- 1 スロット 1 行。種類 / 値 / 外す を列でそろえる(§00 01)。
                     効き先の但し書きは title に入れ、行は増やさない -->
                <div class="siena-row swap-in" class:record-only={!def.is_modeled}>
                  <span class="ro-name" title="{def.label}{def.note ? ` — ${def.note}` : ''}">{def.label}</span>
                  <StatInput
                    label=""
                    min={def.min}
                    max={def.max}
                    format={def.min > 1 ? () => `wiki ${def.min}–${def.max}${def.unit}` : undefined}
                    bind:value={() => s.value, (v) => (s.value = v)}
                    stepper
                  />
                  <button type="button" class="clear" onclick={() => removeSienaSlot(slot, index)}>外す</button>
                </div>
              {/if}
            {/each}
            </div>
            <div class="card">
            <div class="card-title inline">
              追加オプション
              <span class="dim normal num" use:bump={() => capacity}>
                {siena.extras.length} / {capacity} 枠
              </span>
            </div>
            {#if capacity === 0}
              <p class="hint dim">
                段階 {app.siena.extra_unlock_stages[0]} で 1 枠目が開きます(いま段階 {stage})。
              </p>
            {:else}
              {#if siena.extras.length < capacity}
                <div class="ro-add-row">
                  {#each sienaAddableExtras(slot) as def (def.kind)}
                    <button
                      type="button"
                      class="chip add"
                      class:record-only={!def.is_modeled}
                      title={def.note}
                      onclick={() => addSienaExtra(slot, def.kind)}
                    >＋ {def.label}</button>
                  {/each}
                </div>
              {:else}
                <p class="hint dim">
                  いまの段階で開いている {capacity} 枠は埋まりました。次は段階
                  {app.siena.extra_unlock_stages[capacity] ?? app.siena.stage_max} で開きます。
                </p>
              {/if}
              {#each siena.extras as e, index (index)}
                {@const def = sienaExtraDef(e.kind)}
                {#if def}
                  <div class="siena-row swap-in" class:record-only={!def.is_modeled}>
                    <span class="ro-name" title="{def.label} — {def.note}">
                      {def.label}
                      <span class="siena-to">{def.note}</span>
                    </span>
                    {#if sienaChoicesAreRun(def.choices)}
                      <StatInput
                        label=""
                        min={def.choices[0]}
                        max={def.choices[def.choices.length - 1]}
                        format={def.choices[0] > 1
                          ? () => `wiki ${def.choices[0]}–${def.choices[def.choices.length - 1]}${def.unit}`
                          : undefined}
                        bind:value={() => e.value, (v) => (e.value = v)}
                        stepper
                      />
                    {:else}
                      <!-- 飛び飛びの値(中ディレイ 0.5 / 1 / 2%)はステッパーだと無い値を作れてしまう -->
                      <StepSelect
                        label=""
                        options={def.choices.map((c) => ({ value: String(c), label: `${c}${def.unit}` }))}
                        bind:value={() => String(e.value), (v) => (e.value = Number(v))}
                      />
                    {/if}
                    <button type="button" class="clear" onclick={() => removeSienaExtra(slot, index)}>外す</button>
                  </div>
                {/if}
              {/each}
            {/if}
            </div>
          {/if}
        </div>
      </div>
    {/if}
  {:else if sourceId === "randomOption"}
    <div class="card">
      <p class="hint dim">
        wiki「ランダムオプション」。装備補正 9 値には乗らず、与ダメージ式のカテゴリ(依存別の与ダメージ増加・
        攻撃ダメージ増加)や命中P・回避P に直接効きます。<b>同じカテゴリーの OP は 1 部位に 1 つだけ</b>です(wiki: 転移)。
        効果値は触らなければレンジ上限で計算します(オプション変化石で振り直せるため)。
        <b>収録しているのは火力・命中・回避に関係する OP だけ</b>で、HP・移動速度・経験値などは入れていません。
        グレーの枠は<b>記録するだけ</b>(発動条件付き・未実装の概念)で計算には入りません。
      </p>
    </div>
    <!-- 装備と同じドリルダウン。押した部位はその場に残り、右にペインが増える(§09 規則 2) -->
    <div class="part-split" class:open={openRandomPart !== null}>
      <div class="part-list">
        {#each RANDOM_OPTION_ALLOWED_SLOTS as slot (slot)}
          {#if app.randomOptions.some((d) => d.slot === slot) && randomOptionSlots(slot) > 0}
            {@const count = selectedPartOrNull(slot)?.random_options.length ?? 0}
            <button
              type="button"
              class="part-row"
              class:on={openRandomPart === slot}
              onclick={() => (openRandomPart = slot)}
            >
              <span class="part-main">
                <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
              </span>
              <!-- 付いている OP を短い名前のバッジで並べる。名前をそのまま出すと 1 行に入らない -->
              <span class="ro-badges">
                {#each selectedPartOrNull(slot)?.random_options ?? [] as o (o.option_id)}
                  {@const def = randomOptionDef(o.option_id)}
                  {#if def}
                    <!-- バッジは「何が付いているか」だけ。いくら効いているかは行の要約で出す -->
                    <span
                      class="ro-badge"
                      class:record-only={!randomOptionIsApplied(def.effect)}
                      title="{def.name}({randomOptionEffectLabel(def.effect)} {randomOptionValueLabel(o, def)})"
                    >{def.short}</span>
                  {/if}
                {/each}
                {#if count === 0}<span class="dim">なし</span>{/if}
              </span>
              <span class="chev dim">›</span>
            </button>
          {/if}
        {/each}
      </div>
      {#if openRandomPart !== null}
        {@const slot = openRandomPart}
        <div class="part-detail pane-in">
          <button type="button" class="close-detail" onclick={() => (openRandomPart = null)}>✕ この部位を閉じる</button>
          <div class="card">
            <div class="card-title">{PART_SLOT_LABELS[slot]}</div>
            {@render randomOptionEditor(slot)}
            <!-- 枠は 1 装備 2 つ。**1 つ目を決めたら 2 つ目の候補を出す** —
                 候補を 2 枠ぶん並べても、実際に選べるのは順番に 1 つずつ(§00 02) -->
            {#if selectedPartOrNull(slot) !== null && (selectedPartOrNull(slot)?.random_options.length ?? 0) < randomOptionSlots(slot)}
              <div class="ro-next swap-in">
                <span class="ro-next-label">
                  枠 {(selectedPartOrNull(slot)?.random_options.length ?? 0) + 1}
                  <span class="dim">/ {randomOptionSlots(slot)}</span>
                </span>
                {#if commonAddable(slot).length > 0}
                  <div class="ro-common">
                    {#each commonAddable(slot) as o (o.id)}
                      <button type="button" class="chip add" onclick={() => addRandomOption(slot, o.id)}>
                        ＋ {o.name}
                      </button>
                    {/each}
                  </div>
                {/if}
                {#if otherAddable(slot).length > 0}
                  <div class="ro-add">
                    <Picker
                      options={otherPickerOptions(slot)}
                      note="ほかの OP(同じカテゴリーは 1 つまで)"
                      placeholder="ほかの OP から選ぶ"
                      bind:value={() => "", (v) => { if (v !== "") addRandomOption(slot, v); }}
                    />
                  </div>
                {/if}
              </div>
            {:else}
              <p class="hint dim">枠は {randomOptionSlots(slot)} つまで。変えるときは外してから足します。</p>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  {:else if sourceId === "title"}
    <div class="card">
      <div class="card-title">選択中</div>
      <div
        class="contrib-card title-current"
        class:empty={selectedTitle === null}
        use:flash={() => selectedTitle?.id ?? "none"}
      >
        <span class="item-name strong">{selectedTitle?.name ?? "未選択"}</span>
        {#if selectedTitle && titleSummary(selectedTitle) !== ""}
          <span class="item-vals num dim" title={titleSummary(selectedTitle)}>{titleSummary(selectedTitle)}</span>
        {/if}
        {#if selectedTitle?.attack_damage_percent}
          <span class="title-dmg num">ダメージ +{selectedTitle.attack_damage_percent}%</span>
        {:else if selectedTitle?.note.includes("追加ダメージ")}
          <span class="title-extra">条件付き追加ダメージ</span>
        {/if}
        {#if selectedTitle}
          <button type="button" class="chip quiet" onclick={() => (draft.equipment.title = null)}>外す</button>
        {/if}
      </div>
    </div>
    <div class="card">
      <p class="hint dim">
        <b>表示中の 1 件だけ</b>が効きます。普段使う候補だけを先に出し、それ以外は下の「その他」から選べます。
      </p>
      <details class="fold">
        <summary>称号の補正の入り方</summary>
        <div class="fold-body">
          <p class="hint dim">
            wiki「称号システム」。補正値は<b>装備の基本能力値</b>に乗り、<b>ダメージ n% 増加</b>はカテゴリX(攻撃ダメージ)に入ります。
            収録は主要称号のみ({app.titles.length} 件)。並びは<b>ダメージ増加 → 与ダメージに効く 1 値の大きさ</b>の順です。
            条件付き効果とグループボーナスは記録だけで、計算には入りません。
          </p>
        </div>
      </details>
      <div class="card-title space">
        よく使う称号 <span class="normal dim">ダメ +20%以上 / エクリプス / 神鳥の塒</span>
      </div>
      <div class="item-list title-list">
        {@render titleRows(commonTitleGroups)}
      </div>
      <details class="fold">
        <summary>その他の称号から選ぶ({otherTitles.length} 件)</summary>
        <div class="fold-body">
          <input class="item-search" type="text" placeholder="称号名・グループで探す" bind:value={titleQuery} />
          {#if otherTitleGroups.length > 0}
            <div class="item-list title-list">
              {@render titleRows(otherTitleGroups)}
            </div>
          {:else}
            <p class="hint dim">該当する称号はありません。</p>
          {/if}
        </div>
      </details>
    </div>
    {#if selectedTitle}
      {@const filled = EQUIPMENT_STAT_KINDS.filter((k) => selectedTitle.values[k] !== 0)}
      <div class="card">
        <div class="card-title inline">
          選択中の補正 <span class="normal dim">{selectedTitle.name}</span>
        </div>
        {#if selectedTitle.attack_damage_percent > 0}
          <p class="hint dim">
            ダメージ増加は<b>カテゴリX(攻撃ダメージ)</b>の X3 基本発動に入ります(wiki: ステータス。X3 は上限 +80%)。
          </p>
        {/if}
        {#if filled.length > 0}
          <div class="values-grid">
            {#each filled as k (k)}
              <span class="val-cell">
                <span class="dim">{EQUIPMENT_STAT_SHORT[k]}</span>
                <span class="num strong">{signed(selectedTitle.values[k])}</span>
              </span>
            {/each}
          </div>
        {:else}
          <p class="hint dim">補正値はありません(ダメージ増加だけの称号)。</p>
        {/if}
        <p class="hint dim">
          {selectedTitle.group}{selectedTitle.level !== null ? ` ・ 習得 Lv${selectedTitle.level}` : ""}
          {#if selectedTitle.note}<br />{selectedTitle.note}{/if}
        </p>
      </div>
    {/if}
  {:else if sourceId === "commonSkill"}
    <!-- ほぼ全員が同じ設定なので、**人によって違うところを先に置く**。
         残りは既定で最大まで入れて畳む(§00 02)。結果(装備攻撃力・防御力・追加ダメージ)は
         「いまの実力」が持つので、ここには入力だけを置く -->
    <div class="card">
      <div class="card-title inline">
        まず決める <span class="dim normal">人によって違うのはここ</span>
      </div>
      <div class="skill-fields">
        <div class="skill-field">
          <span class="k">オーグメント</span>
          <StepSelect
            label=""
            options={augmentOptions}
            cols={augmentOptions.length}
              cell={36}
            bind:value={
              () => String(draft.commonSkills.augment_level),
              (v) => setAugmentLevel(Number(v))
            }
          />
          <span class="skill-actions">
            <button
              type="button"
              class="clear"
              disabled={draft.commonSkills.augment_level === 0}
              onclick={() => setAugmentLevel(0)}
            >未習得</button>
          </span>
          <span></span>
        </div>
        <div class="skill-field">
          <span class="k">極限スキル</span>
          <div class="ultimate-row">
            {#each ULTIMATE_SKILLS as u (u)}
              {@const on = draft.commonSkills.ultimate.slots.includes(u)}
              <button
                type="button"
                class="chip"
                class:on
                disabled={!on && ultimatePickedCount >= 2}
                onclick={() => toggleUltimate(u)}
              >{ULTIMATE_SKILL_LABELS[u]}</button>
            {/each}
          </div>
          <span class="v num">{ultimatePickedCount} / 2</span>
        </div>
        <!-- 選んだ数で行数が変わると、下にあるものが上下する。**2 件ぶんの場所を先に取る**
             (§09 規則 4「あとから寸法が変わらない」) -->
        <div class="ultimate-notes">
          {#each [0, 1] as i (i)}
            {@const picked = draft.commonSkills.ultimate.slots[i]}
            <p class="hint dim skill-note">
              {#if picked !== null}{ULTIMATE_SKILL_LABELS[picked]}: {ULTIMATE_SKILL_EFFECTS[picked]}{/if}
            </p>
          {/each}
        </div>
      </div>
      <p class="hint dim">
        いまの効果:
        <b use:flash={() => ultimateEffects.join(" ・ ")}>{ultimateEffects.length > 0 ? ultimateEffects.join(" ・ ") : "—"}</b>
      </p>
      <p class="hint dim">
        wiki「Skill/共通」「Skill/極限」。<b>オーグメント</b>はストロングウェポン・プロテクトアーマー・
        ハイパーリミットを Lv2 以上にするための前提で、下げるとそれに縛られる Lv も一緒に下がります。
      </p>
    </div>

    <div class="card">
      <div class="card-title inline">アンリーシュ(能力解放)</div>
      <div class="skill-fields">
        {#each draft.commonSkills.unleash as slot, i (i)}
          <div class="skill-field">
            <span class="k">枠 {i + 1}</span>
            <StepSelect
              label=""
              options={unleashStatOptions}
              cols={unleashStatOptions.length}
              disabledValues={unleashDisabled(i)}
              bind:value={() => slot.stat ?? "", (v) => setUnleashStat(i, v)}
            />
            <span class="skill-actions">
              <button
                type="button"
                class="clear"
                disabled={slot.stat === null}
                onclick={() => setUnleashStat(i, "")}
              >未使用</button>
            </span>
            <span class="v num" use:flash={() => (slot.stat === null ? "-" : `${UNLEASH_RATES[slot.level - 1]}`)}>
              {slot.stat === null ? "—" : `+${UNLEASH_RATES[slot.level - 1]}%`}
            </span>
          </div>
        {/each}
      </div>
      <p class="hint dim">
        選んだステが<b>能力値倍率B</b>で増えます(<b>バフ込みの基本能力値 × 倍率</b>なので、
        バフを盛るほど効きます)。<b>2 ステまで</b>で、同じステは 2 枠に入れられません。
        Lv は取れる上限(いまは <b>Lv{unleashCap}</b> = +{UNLEASH_RATES[unleashCap - 1]}%)で入ります。
      </p>
    </div>

    <div class="card">
      <div class="card-title inline">シャープネスビジョン</div>
      <div class="skill-fields">
        <div class="skill-field">
          <span class="k">Lv</span>
          <StepSelect
            label=""
            options={sharpnessOptionsNow}
            cols={sharpnessOptionsNow.length}
              cell={36}
            bind:value={
              () => String(draft.commonSkills.sharpness_vision_level),
              (v) => (draft.commonSkills.sharpness_vision_level = Number(v))
            }
          />
          <span class="skill-actions">
            {#if !sharpnessIsLow}
              <button
                type="button"
                class="chip quiet"
                class:on={sharpnessAllOpen}
                onclick={() => (sharpnessAllOpen = !sharpnessAllOpen)}
              >{sharpnessAllOpen ? "5 以上" : "1〜4"}</button>
            {/if}
            <button
              type="button"
              class="clear"
              disabled={draft.commonSkills.sharpness_vision_level === 0}
              onclick={() => (draft.commonSkills.sharpness_vision_level = 0)}
            >未習得</button>
          </span>
          <span
            class="v num"
            use:bump={() => (draft.commonSkills.sharpness_vision_level === 0
              ? null
              : SHARPNESS_RATES[draft.commonSkills.sharpness_vision_level - 1])}
          >
            {draft.commonSkills.sharpness_vision_level === 0
              ? "—"
              : `+${SHARPNESS_RATES[draft.commonSkills.sharpness_vision_level - 1]}%`}
          </span>
        </div>
      </div>
      <p class="hint dim">
        割合追加ダメージは<b>合計ダメージ</b>に乗ります(1 発ごとではありません)。
        Lv6 以上は各 Lv の習得スクロールが要ります。
      </p>
    </div>

    <div class="card">
      <div class="card-title inline">
        ほぼ全員が同じ設定 <span class="dim normal">取り切っている前提で入れてあります</span>
      </div>
      <details class="fold">
        <summary>取っていない・Lv が違うときだけ開く(8 項目)</summary>
        <!-- 開いた先も上と同じ形。ラベル / 段 / 操作 / 効いている値の 4 列でそろえる -->
        <div class="fold-body skill-fields">
          <div class="skill-field">
            <span class="k">パワーウェポン</span>
            <span class="chip-row">
              <button
                type="button"
                class="chip"
                class:on={draft.commonSkills.power_weapon}
                onclick={() => (draft.commonSkills.power_weapon = !draft.commonSkills.power_weapon)}
              >取っている</button>
            </span>
            <span class="skill-actions"></span>
            <span class="v num">{draft.commonSkills.power_weapon ? "+2%" : "—"}</span>
          </div>
          <div class="skill-field">
            <span class="k">ストロングウェポン</span>
            <StepSelect
              label=""
              options={strongWeaponLevels}
              cols={strongWeaponLevels.length}
              cell={36}
              disabledValues={strongWeaponLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
              bind:value={
                () => String(draft.commonSkills.strong_weapon_level),
                (v) => (draft.commonSkills.strong_weapon_level = Number(v))
              }
            />
            <span class="skill-actions">
              <button
                type="button"
                class="clear"
                disabled={draft.commonSkills.strong_weapon_level === 0}
                onclick={() => (draft.commonSkills.strong_weapon_level = 0)}
              >未習得</button>
            </span>
            <span class="v num" use:bump={() => draft.commonSkills.strong_weapon_level * 3}>
              {draft.commonSkills.strong_weapon_level === 0 ? "—" : `+${draft.commonSkills.strong_weapon_level * 3}%`}
            </span>
          </div>
          <div class="skill-field">
            <span class="k">コートアーマー</span>
            <span class="chip-row">
              <button
                type="button"
                class="chip"
                class:on={draft.commonSkills.coat_armor}
                onclick={() => (draft.commonSkills.coat_armor = !draft.commonSkills.coat_armor)}
              >取っている</button>
            </span>
            <span class="skill-actions"></span>
            <span class="v num">
              {draft.commonSkills.coat_armor
                ? `物${Math.round(limits.coat_armor_physical_rate * 100)} / 魔${Math.round(limits.coat_armor_magic_rate * 100)}%`
                : "—"}
            </span>
          </div>
          <div class="skill-field">
            <span class="k">プロテクトアーマー</span>
            <StepSelect
              label=""
              options={protectArmorLevels}
              cols={protectArmorLevels.length}
              cell={36}
              disabledValues={protectArmorLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
              bind:value={
                () => String(draft.commonSkills.protect_armor_level),
                (v) => (draft.commonSkills.protect_armor_level = Number(v))
              }
            />
            <span class="skill-actions">
              <button
                type="button"
                class="clear"
                disabled={draft.commonSkills.protect_armor_level === 0}
                onclick={() => (draft.commonSkills.protect_armor_level = 0)}
              >未習得</button>
            </span>
            <span class="v num" use:bump={() => draft.commonSkills.protect_armor_level}>
              {draft.commonSkills.protect_armor_level === 0
                ? "—"
                : `物${PROTECT_ARMOR_RATES[draft.commonSkills.protect_armor_level - 1]} / 魔${PROTECT_ARMOR_MAGIC[draft.commonSkills.protect_armor_level - 1]}%`}
            </span>
          </div>
          <div class="skill-field">
            <span class="k">改・プロテクト</span>
            <StepSelect
              label=""
              options={kaiProtectArmorLevels}
              cols={kaiProtectArmorLevels.length}
              cell={36}
              bind:value={
                () => String(draft.commonSkills.kai_protect_armor_level),
                (v) => (draft.commonSkills.kai_protect_armor_level = Number(v))
              }
            />
            <span class="skill-actions">
              <button
                type="button"
                class="clear"
                disabled={draft.commonSkills.kai_protect_armor_level === 0}
                onclick={() => (draft.commonSkills.kai_protect_armor_level = 0)}
              >未習得</button>
            </span>
            <span class="v num" use:bump={() => draft.commonSkills.kai_protect_armor_level * 9}>
              {draft.commonSkills.kai_protect_armor_level === 0
                ? "—"
                : `物${draft.commonSkills.kai_protect_armor_level * 9} / 魔${draft.commonSkills.kai_protect_armor_level * 6}%`}
            </span>
          </div>
          <div class="skill-field">
            <span class="k">スーパーリミット</span>
            <span class="chip-row">
              <button
                type="button"
                class="chip"
                class:on={draft.commonSkills.ultimate.super_limit}
                onclick={() => (draft.commonSkills.ultimate.super_limit = !draft.commonSkills.ultimate.super_limit)}
              >取っている</button>
            </span>
            <span class="skill-actions"></span>
            <span class="v num">{draft.commonSkills.ultimate.super_limit ? "極限に加算" : "—"}</span>
          </div>
          <div class="skill-field">
            <span class="k">ハイパーリミット</span>
            <StepSelect
              label=""
              options={hyperLimitLevels}
              cols={hyperLimitLevels.length}
              cell={36}
              disabledValues={hyperLimitLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
              bind:value={
                () => String(draft.commonSkills.ultimate.hyper_limit_level),
                (v) => (draft.commonSkills.ultimate.hyper_limit_level = Number(v))
              }
            />
            <span class="skill-actions">
              <button
                type="button"
                class="clear"
                disabled={draft.commonSkills.ultimate.hyper_limit_level === 0}
                onclick={() => (draft.commonSkills.ultimate.hyper_limit_level = 0)}
              >未習得</button>
            </span>
            <span class="v num">{draft.commonSkills.ultimate.hyper_limit_level === 0 ? "—" : `Lv${draft.commonSkills.ultimate.hyper_limit_level}`}</span>
          </div>
          <div class="skill-field">
            <span class="k">レインフォース</span>
            <StepSelect
              label=""
              options={reinforceLevels}
              cols={reinforceLevels.length}
              cell={36}
              bind:value={
                () => String(draft.commonSkills.reinforce_level),
                (v) => setReinforceLevel(Number(v))
              }
            />
            <span class="skill-actions">
              <button
                type="button"
                class="clear"
                disabled={draft.commonSkills.reinforce_level === 0}
                onclick={() => setReinforceLevel(0)}
              >未習得</button>
            </span>
            <span class="v num">Lv{unleashCap} まで</span>
          </div>
          {#each draft.commonSkills.unleash as slot, i (i)}
            {#if slot.stat !== null}
              <div class="skill-field">
                <span class="k">解放 {i + 1} の Lv</span>
                <StepSelect
                  label=""
                  options={unleashLevelChoices}
                  cols={unleashLevelChoices.length}
              cell={36}
                  disabledValues={unleashLevelChoices.filter((o) => Number(o.value) > unleashCap).map((o) => o.value)}
                  bind:value={() => String(slot.level), (v) => (slot.level = Number(v))}
                />
                <span class="skill-actions"></span>
                <span class="v num">{STAT_LABELS[slot.stat]} +{UNLEASH_RATES[slot.level - 1]}%</span>
              </div>
            {/if}
          {/each}
          <p class="hint dim">
            オーグメントで解放されていない段は押せません。
            {#if sienaDefenseRate > 0}装備防御力にはシエナのオーラの +{sienaDefenseRate}% を含みます。{/if}
            <b>リンゴの島・ベリネンルミでは装備防御力は常に 100%</b>(wiki 計算式まとめ §防御力)。
          </p>
        </div>
      </details>
    </div>
  {:else if sourceId === "thesis"}
    <div class="card">
      <div class="card-title">地域</div>
      <!-- 地域は「同じ形の 6 枠を切り替える」ので §08 のタブ。選んだ地域の下と地続きになる -->
      <div class="tabs">
        {#each CORE_REGIONS as region (region)}
          <button
            type="button"
            class="tab"
            class:on={coreRegion === region}
            onclick={() => (coreRegion = region)}
          >
            {CORE_REGION_LABELS[region]}
            <span class="num dim" use:bump={() => coreRegionTotal(region)}>{fmtInt(coreRegionTotal(region))}</span>
            {#if (coreSetOf(region)?.set_groups.length ?? 0) > 0}
              <span class="tab-set num" use:flash={() => coreSetLabelOf(region)}>{coreSetLabelOf(region)}</span>
            {:else if coreRegionTotal(region) > 0}
              <span class="tab-set off num">あと {3 - (coreSetOf(region)?.ready ?? 0)}</span>
            {/if}
          </button>
        {/each}
      </div>
      <div class="tab-rule"></div>
      <!-- 説明は毎回読むものではない。畳んで、入力の場所を押し下げないようにする(§00 02) -->
      <details class="fold">
        <summary>この画面の読み方(wiki テシスコア)</summary>
        <div class="fold-body">
          <p class="hint dim">
            コアの能力値増加は対象ダンジョン内でのみ有効なので、計算対象のコンテンツに
            対応する地域のコアだけが装備攻撃力に入ります。コアセット効果(最終ダメージ)は全地域で発動し、
            地域ごとの発動分が足されます。
          </p>
          <p class="hint dim">
            補助タイプ(物防/回避/敏捷/命中)も装着状態として記録できます。与ダメージ式の装備係数が 0 なので
            装備攻撃力には入らず、入場条件「コア N」の合計と防御タブ(防御力・カット率・回避P)に効きます。
            経験値タイプのみのシオカンヘイムコアは火力にもセット効果にも効かないため地域を持ちません。
          </p>
          <p class="hint dim">
            入場条件の「コア N」はこの 6 枠の合計と同じ値です(火力の進化1強化4 ×6 = 60、進化4強化4 ×6 = 480。
            補助タイプは進化4強化4 でも 60 なので 6 枠でも 360 止まり)。
            コアセット効果は強化 4 段階のコアが 3 個以上そろうと発動します(タイプは問いません)。
          </p>
        </div>
      </details>
    </div>
    {#key coreRegion}
    <div class="card swap-in">
      <div class="card-title inline">
        {CORE_REGION_LABELS[coreRegion]} の 6 枠
        <!-- 「補助も出す」は段の見え方を変える操作なので、段より先に目に入る位置に置く。
             控えめなチップ 1 つ(§07 形態 3)。下に置くと、段を見たあとで見え方が変わって読み直しになる -->
        <button
          type="button"
          class="chip quiet"
          class:on={coreShowSupport || coreSupportInUse}
          onclick={toggleCoreSupport}
        >{coreShowSupport || coreSupportInUse ? "補助タイプを閉じる" : "補助タイプも出す"}</button>
      </div>
      <!-- この画面で知りたいのは「いくつになったか」と「セット効果が出ているか」の 2 つ。
           小さな注記ではなく、段より先に読める場所に出す -->
      {#if coreSupportSummary}
        <p class="hint dim">
          このうち補助タイプ({coreSupportSummary})は装備攻撃力には入らず、防御タブの防御力・カット率・回避Pに効きます。
        </p>
      {/if}
      <!-- 列の名前は 1 回だけ。行ごとにラベルを置くと、6 回同じ言葉を読ませることになる -->
      <div class="core-head">
        <span></span><span>タイプ</span><span></span><span class="lead">進化 - 強化</span><span class="r">コア効果</span>
      </div>
      <div class="core-list">
        {#each coreSlotIndexes as index (index)}
          {@const core = coreAt(index)}
          <div class="core-row">
            <span class="core-slot dim">{index + 1}</span>
            <!-- 6 枠が同じ列で並ぶように列を固定する。行ごとに幅が違うと端を探し直す(§00 01)。
                 補助タイプは別の段にする — 1 つの段に 9 個入れると列が余って空きセルが出る -->
            <span class="core-types">
              <StepSelect
                label=""
                options={corePowerOptions}
                cols={4}
                bind:value={() => core?.core_type ?? "", (v) => setCoreType(index, v)}
              />
              {#if coreShowSupport || coreSupportInUse}
                <!-- 段が増えるのは「開いた」なので下に伸ばす(§10 型 6) -->
                <div class="open-in">
                  <StepSelect
                    label=""
                    options={coreSupportOptions}
                    cols={4}
                    bind:value={() => core?.core_type ?? "", (v) => setCoreType(index, v)}
                  />
                </div>
              {/if}
            </span>
            <!-- 進化 - 強化。押すと 5×5 が重なって出るので、押した枠は動かない(§09 規則 3) -->
            <button
              type="button"
              class="core-clear"
              disabled={core === null}
              onclick={() => setCoreType(index, "")}
            >外す</button>
            <span class="core-stage">
              <button
                type="button"
                class="stage-trigger num"
                disabled={core === null}
                aria-label="進化と強化"
                onclick={() => (openCoreStage = openCoreStage === index ? null : index)}
              >
                <span use:flash={() => (core ? `${core.evolution}-${core.enhancement}` : "-")}>
                  {core ? `${core.evolution}-${core.enhancement}` : "—"}
                </span>
              </button>
              {#if openCoreStage === index && core}
                <button type="button" class="stage-overlay" aria-label="閉じる" onclick={() => (openCoreStage = null)}></button>
                <!-- 下の枠は上に開く。下に開くとペインの外へ出て、選ぶのにスクロールが要る -->
                <div class="stage-pop pop-in" class:up={index >= 3}>
                  <div class="stage-pop-h">進化 - 強化</div>
                  <div class="stage-grid">
                    {#each coreStagePairs as row (row[0].ev)}
                      {#each row as p (p.en)}
                        <!-- 段の名前だけだと「それでいくつになるのか」を毎回考えることになる。
                             結果(補正値)をその場に小さく乗せる(§00 05「考えさせない」) -->
                        <button
                          type="button"
                          class="stage-cell"
                          class:on={core.evolution === p.ev && core.enhancement === p.en}
                          onclick={() => { setCoreStagePair(index, p.ev, p.en); openCoreStage = null; }}
                        >
                          <b class="num">{p.ev}-{p.en}</b>
                          <span class="cell-bonus num">+{fmtInt(coreBonus(core.core_type, p.ev, p.en))}</span>
                        </button>
                      {/each}
                    {/each}
                  </div>
                </div>
              {/if}
            </span>
            <span
              class="core-bonus num"
              class:support={core !== null && !CORE_POWER_TYPES.includes(core.core_type)}
              use:bump={() => (core ? coreBonus(core.core_type, core.evolution, core.enhancement) : null)}
            >
              {core ? `+${fmtInt(coreBonus(core.core_type, core.evolution, core.enhancement))}` : "—"}
            </span>
          </div>
        {/each}
      </div>

    </div>
    {/key}
  {:else if sourceId === "actualDelay"}
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
    {@render fromOthers(delayFromOthers, "ほかの補正源から入る分")}
  {:else if sourceId === "criticalRate"}
    <div class="card">
      <p class="hint dim">
        wiki「計算式まとめ <b>#CriticalChance</b>」。クリティカル率は
        <b>(装備クリティカル補正 + 1) × 2 × (AGI / (AGI + 対象のAGI)) × ペット会心
        ＋ スキルの Cri値 ＋ クリティカル率増加 ＋ 対象のクリティカル被撃率</b>で、下限 0% / 上限 100%。
        装備クリティカル補正・AGI・スキルの Cri値は登録済みのデータから自動で入るので、
        ここで選ぶのは<b>ペット会心と「クリティカル率増加」</b>だけです(自動で入る分は下に出します)。
        対象のAGI とクリティカル被撃率は wiki 狩り場情報一覧に値がある敵だけに入っているので、
        計算タブでは<b>その敵を選んだときだけ</b>クリティカル率が出ます。
      </p>
      <div class="buff-list">
        <label class="check">
          <input type="checkbox" bind:checked={draft.statSources.critical_rate.pet} />
          <span>ペット会心</span>
          <span class="fixed-value dim">×1.1</span>
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
    {@render fromOthers(criticalFromOthers, "ほかの補正源から自動で入る分")}
  {:else if sourceId === "skills"}
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

<style>
  .pane { display: flex; flex-direction: column; gap: 9px; padding-bottom: 10px; }
  .pane-head { display: flex; align-items: baseline; gap: 8px; padding: 0 2px; }
  .pane-title { font-size: var(--t-label); font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); }
  .pane-head .dim { margin-left: auto; font-size: 9px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .preview-error { margin: 0; padding: 0 2px; font-size: 11px; color: var(--warm); }

  .card-title .normal { font-weight: 400; font-size: 9px; }
  .card-title.space { margin-top: 12px; }
  .hint { margin: 6px 0 0; font-size: 9.5px; line-height: 1.5; }
  /* エタの意志。値の欄と節目を横に並べる(どちらも同じことを決める場所なので) */
  .eternal-row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .eternal-row > :global(.stat-input), .eternal-row > :global(.stepper) { flex: 0 0 150px; }
  .eternal-row > :global(.step-select) { flex: 1 1 260px; min-width: 0; }
  /* Lv の段階選択 + 選択中の効果値 */
  .lv { display: flex; flex-direction: column; gap: 4px; min-width: 0; }

  /* 入力セルは親の幅まで伸びる(§07 実演の .ctrl が 1fr)。1 列で受けると値が
     右端まで離れて読めないので、232px の段に割って伸び先を絞る */
  .add-row { margin-top: 9px; display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .fields {
    margin-top: 9px; display: grid; gap: 9px 16px;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
  }
  .text { display: flex; flex-direction: column; gap: 6px; }
  /* 段が多くて折り返す欄は 1 行を占める。折り返した段は一度の視線で読めない(§00 01) */
  .fields > :global(.wide) { grid-column: 1 / -1; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  input[type="text"] {
    padding: 8px 10px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); }

  .tbl { margin-top: 8px; overflow-x: auto; border: 1px solid var(--border-soft); border-radius: var(--r-panel); background: var(--surface-inset); }
  table.grid td.stat-cell { min-width: 180px; }
  .stat-cell :global(.stepper) { justify-content: flex-end; }
  .final { white-space: nowrap; }
  .strong { font-weight: 700; }
  .pin-badge {
    margin-left: 6px; vertical-align: middle;
    font-size: 9px; letter-spacing: 0.05em; color: var(--accent); border: 1px solid var(--accent);
    border-radius: var(--r-inset); padding: 1px 4px; cursor: default;
  }
  details.contrib { margin-top: 8px; }
  details.contrib summary { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-muted); cursor: pointer; user-select: none; }
  details.contrib summary:hover { color: var(--fg); }
  .empty { margin: 6px 0 0; padding: 4px 0; font-size: 11px; }

  /* オン / オフはチップで出す(§07 形態 3)。素のチェックボックスは 5 形態のどれでもない。
     値はチップの中に書く — 選ぶ = 値が確定する。計算タブのバフチップと同じ形にそろえる */
  .check {
    display: inline-flex; align-items: center; flex-wrap: wrap; gap: 6px;
    padding: 4px 11px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    color: var(--fg-sub); font-size: 11px; font-weight: 500; cursor: pointer;
  }
  .check:hover { border-color: var(--accent); }
  .check:has(input:checked) {
    background: linear-gradient(180deg, #CCF7FF, #90D7FF);
    border-color: #687287; color: #123047; font-weight: 700;
  }
  .check:has(input:focus-visible) { outline: 1px solid var(--accent); outline-offset: 2px; }
  /* チェックボックスそのものは出さない。選ばれているかは面の色が言う */
  .check input { position: absolute; opacity: 0; width: 0; height: 0; }
  .check .fixed-value, .check .note { color: inherit; opacity: 0.75; }
  /* チップは横に並べる(1 行 1 個だと 30 行の一覧になる) */
  .buff-list { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 6px; }
  .note { font-size: 10px; }
  /* 「満」は上限に届いた印。金 = 上限(§03 予約色)。枠は常に確保して行をずらさない */
  .cap-badge {
    display: inline-block; min-width: 20px; text-align: center;
    font-size: 9px; letter-spacing: 0.05em; color: transparent; border: 1px solid transparent;
    border-radius: var(--r-inset); padding: 1px 4px; cursor: default;
  }
  .cap-badge.on { color: var(--state-edge-fg); border-color: var(--gold); background: var(--state-edge-bg); }
  /* 素ステ → 最終。灰が素ステ、青が補正で乗った分(§11) */
  .grow {
    display: flex; width: 100%; min-width: 74px; height: 7px; overflow: hidden;
    border-radius: var(--r-inset); background: var(--surface-inset); border: 1px solid var(--border-soft);
  }
  .grow > i { display: block; transition: width 0.38s cubic-bezier(0.4, 0, 0.2, 1); }
  .grow > .base { background: var(--flow-base); }
  .grow > .add { background: var(--accent); }
  .fixed-value { font-size: 11px; font-weight: 500; }
  /* マスタリー待ちで値が決まらないものは破線で出す(0 や空白で埋めない。§03) */
  .fixed-value.unknown {
    padding: 0 4px; border: 1px dashed var(--border); border-radius: 4px; opacity: 0.6;
  }

  /* 装備ドリルダウン: 部位一覧 */
  /* 装備のドリルダウン(§09 規則 2)。掘るたびに右へペインが増え、前の階層は消えない。
     詳細を開いているときだけ一覧を細くし、値サマリを畳む — 狭いときだけ左から畳む、の形 */
  .part-split { min-width: 0; }
  .part-list { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 6px; }
  /* シエナは一覧の上に説明カードがあるので、まとめて 1 列にする */
  .equipment-overlay { z-index: 90; padding: 3vh max(14px, 6vw); display: flex; justify-content: center; align-items: flex-start; }
  .part-detail { width: min(980px, 100%); max-height: 94vh; overflow: auto; padding: 0 8px 8px; display: flex; flex-direction: column; gap: 5px; }
  .part-detail > .card { padding: 7px 9px; }
  .equipment-item-content { display: flex; flex-direction: column; gap: 5px; }
  .equipment-item-content > .card { padding: 7px 9px; }
  .part-detail-header { position: sticky; top: 0; z-index: 4; min-height: 42px; margin: 0 -8px; padding: 6px 9px 6px 12px; display: flex; align-items: center; justify-content: space-between; background: var(--bg-rail); border-bottom: 1px solid var(--border-strong); box-shadow: 0 4px 9px rgba(18, 27, 42, .09); }
  .part-detail-header > b { font-size: 12px; color: var(--fg-sub); }
  .close-equipment { min-width: 88px; justify-content: center; border-color: var(--border-strong); background: var(--bg-field); font-weight: 700; }
  .close-equipment span { font-size: 15px; line-height: 1; }
  .part-row {
    display: flex; align-items: center; gap: 10px; padding: 9px 11px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .part-row:hover { border-color: var(--accent); }
  /* いま右に開いている部位。押した行がその場に残っていることを面で示す */
  .part-row.on { background: var(--bg-active); border-color: var(--accent); }
  .part-main { min-width: 0; flex: 1; display: flex; align-items: baseline; gap: 7px; }
  .part-name { flex-shrink: 0; font-size: 11px; font-weight: 700; }
  .part-item { min-width: 0; font-size: 10px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .part-plus {
    flex-shrink: 0; min-width: 30px; text-align: center;
    padding: 0 6px; border-radius: var(--r-pill);
    background: transparent; border: 1px solid transparent;
    font-size: 9px; font-weight: 700; color: transparent;
  }
  .part-plus.on { background: var(--state-short-bg); border-color: var(--mob); color: #7A4B45; }
  .part-plus.wide { min-width: 46px; }
  .part-vals { flex-shrink: 0; font-size: 9.5px; }
  .chev { flex-shrink: 0; font-size: 11px; }

  /* 装備ドリルダウン: 部位詳細。一覧が消えないので「戻る」は要らない — 閉じるだけ */
  .close-detail { align-self: flex-start; padding: 2px 2px; font-size: var(--t-label); color: var(--fg-muted); }
  .close-detail:hover { color: var(--accent); text-decoration: underline; }
  .card-title.inline { margin: 0; display: flex; align-items: baseline; gap: 8px; }
  /* 見え方を変えるスイッチは右端に控えめに置く(主役は 6 枠そのもの) */
  /* 押すと言葉が変わる(出す ↔ 閉じる)。幅を先に取っておかないと、押した瞬間に
     押した場所そのものが動く(§00 03「押した場所は動かない」) */
  .card-title.inline .chip.quiet {
    margin-left: auto; align-self: center; min-width: 112px; justify-content: center;
  }
  .values-cols { display: flex; flex-wrap: wrap; gap: 10px 16px; }
  .values-col { flex: 1 1 260px; min-width: 0; }
  .part-elem {
    flex-shrink: 0; padding: 1px 6px; font-size: 9px; font-weight: 700;
    color: var(--fg-muted); background: var(--surface-inset);
    border: 1px solid var(--border); border-radius: var(--r-pill);
  }
  .part-abi {
    flex-shrink: 0; font-size: 8.5px; font-weight: 700; color: var(--fg-muted);
    border: 1px solid var(--border); border-radius: var(--r-pill); padding: 0 6px;
  }
  /* 装着時効果は攻撃・耐久とも装備で得ている効果なので、状態色 §03 の「足りている」を使う */
  .part-dmg {
    flex-shrink: 0; font-size: 8.5px; font-weight: 700;
    color: var(--state-met-fg); background: var(--state-met-bg);
    border: 1px solid var(--state-met-bd); border-radius: var(--r-pill); padding: 0 6px;
  }
  /* 変種も同じ 1 行に置く。単独称号と高さをそろえ、一覧をリズムよく追えるようにする */
  .item-row.group {
    display: flex; align-items: center; gap: 7px; cursor: default;
  }
  .item-row.group:hover { border-color: var(--border); }
  .title-list > .item-row { height: 40px; min-height: 40px; overflow: hidden; }
  .title-list .item-name { flex: 1; }
  .title-variants { margin-left: auto; display: flex; flex-wrap: nowrap; gap: 5px; flex-shrink: 0; }

  /* マスタリーの 1 段。段名 + 3 択のカード。どの段も 3 択なので列を固定してそろえる */
  .mastery-row {
    display: grid; grid-template-columns: 24px minmax(0, 1fr);
    gap: 9px; align-items: start;
    padding: 6px 0; border-bottom: 1px dashed var(--border-soft);
  }
  .mastery-row:last-of-type { border-bottom: none; }
  .mastery-tier { padding-top: 9px; font-size: 10px; font-weight: 700; color: var(--fg-dim); }
  .mastery-options { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 6px; }
  /* 1 択 = アイコン + 名前 + 効果。効果を出さないと「どれを選ぶか」を別画面で調べることになる(§00 05) */
  .mastery-option {
    display: flex; align-items: center; gap: 7px; min-width: 0; text-align: left;
    padding: 6px 8px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
    cursor: pointer;
  }
  .mastery-option:hover { border-color: var(--accent); }
  .mastery-option.on {
    background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent);
  }
  /* 計算に入らない(記録するだけの)選択肢 */
  .mastery-option.record-only { border-style: dashed; color: var(--fg-muted); }
  .mastery-option.record-only.on { border-style: solid; }
  .mastery-text { min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .mastery-name { font-size: 11px; font-weight: 700; line-height: 1.25; }
  /* 効果は 2 行で打ち切る。長い 1 択(騎士道・冬を降らせる子)に合わせて段の高さが
     ばらつくと、3 列そろえた意味が無くなる。全文は title に出る */
  .mastery-effect {
    font-size: 9px; color: var(--fg-muted); line-height: 1.25;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .mastery-option.on .mastery-effect { color: var(--fg-sub); }

  /* 設計者の研究室(段階制)。チェックの列に混ぜず 1 段だけ独立させる */
  .lab-field {
    margin: 7px 0; display: flex; align-items: center; gap: 9px; flex-wrap: wrap;
  }
  .lab-label { font-size: 11.5px; font-weight: 700; }
  .lab-note { display: block; font-size: 8.5px; font-weight: 500; color: var(--fg-muted); }
  .lab-value { min-width: 52px; font-size: 12.5px; font-weight: 700; }

  /* ほかの補正源から入る分の 1 行。名前 / 値 / その補正源へ移るボタン */
  .ext-row {
    display: grid; grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 9px; align-items: center;
    padding: 3px 0; border-bottom: 1px dashed var(--border-soft);
  }
  .ext-row:last-of-type { border-bottom: none; }
  /* まだ入っていない供給源。消さずに薄くする(どこから来るかの地図として残す) */
  .ext-row.empty { opacity: 0.55; }
  .ext-name { min-width: 0; font-size: 11.5px; font-weight: 700; }
  .ext-note { display: block; font-size: 8.5px; font-weight: 500; color: var(--fg-muted); }
  .ext-value { font-size: 12.5px; font-weight: 700; min-width: 56px; text-align: right; }

  /* 称号のダメージ増加。補正値と効き方が違うので値の要約と分けて出す */
  .title-dmg {
    flex-shrink: 0; padding: 0 6px; font-size: 9px; font-weight: 700;
    color: var(--sel-fg); background: var(--sel);
    border: 1px solid var(--sel-bd); border-radius: var(--r-pill);
  }
  .title-extra {
    flex-shrink: 0; padding: 0 6px; font-size: 9px; font-weight: 700;
    color: var(--state-edge-fg); background: var(--state-edge-bg);
    border: 1px dashed var(--state-edge-bd); border-radius: var(--r-pill);
  }
  .title-current { min-height: 40px; flex-wrap: nowrap; }
  .title-current .item-name { flex: 1; }
  .title-current .item-vals {
    min-width: 0; max-width: 42%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  /* 称号の補正値グリッド */
  .values-grid { display: flex; flex-wrap: wrap; gap: 4px 12px; margin-bottom: 6px; }
  .val-cell { display: flex; align-items: baseline; gap: 4px; font-size: 11px; }

  /* ランダムオプションの 1 枠 */
  /* 1 OP 1 枠。名前 + 外すが上、ランクと効果値が下。
     右のペインは狭いので、ランクの段(言葉 4 つ)を名前と同じ行に置くと折り返す */
  .ro-row {
    display: grid; grid-template-columns: minmax(0, 1fr) 40px;
    gap: 5px 9px; align-items: center;
    padding: 7px 0; border-bottom: 1px dashed var(--border-soft);
  }
  .ro-rank { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 7px; }
  .ro-row > :global(.stat-input), .ro-row > :global(.stepper) { grid-column: 1 / -1; max-width: 240px; }
  .ro-row.record-only { opacity: 0.75; }
  .ro-name { min-width: 0; font-size: 11.5px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ro-row .clear {
    padding: 3px 4px; border-radius: var(--r-inset);
    background: none; border: 0; color: var(--fg-dim); font-size: 9.5px; line-height: 1;
  }
  .ro-row .clear:hover { background: var(--state-short-bg); color: var(--danger); }
  .ro-note { margin: 0 0 6px; }
  .ro-next { margin-top: 10px; padding-top: 9px; border-top: 1px dashed var(--border-soft); }
  .ro-next-label { font-size: 9px; letter-spacing: 0.1em; color: var(--fg-dim); }
  .ro-common { margin-top: 7px; display: flex; flex-wrap: wrap; gap: 6px; }
  /* シエナのオーラの 1 枠。名前 / 値 / 外す が 1 行に収まる
     (ランダムOP と違ってランクの段が無いので 2 行いらない) */
  .siena-row {
    display: grid; grid-template-columns: minmax(0, 132px) minmax(0, 1fr) 40px;
    gap: 9px; align-items: center;
    padding: 3px 0; border-bottom: 1px dashed var(--border-soft);
  }
  .siena-row:last-of-type { border-bottom: none; }
  .non-weapon-additional-panel { margin: 8px 0 0; }
  .siena-row.record-only { opacity: 0.75; }
  .siena-row .clear {
    padding: 1px 5px; font-size: 9px; color: var(--fg-dim);
    background: none; border: 1px solid var(--border-soft); border-radius: var(--r-pill);
  }
  .siena-row .clear:hover { background: var(--state-short-bg); color: var(--danger); }
  /* 効き先。名前の下に小さく置く(行は増やさない) */
  .siena-to {
    display: block; font-size: 8.5px; font-weight: 500; color: var(--fg-muted);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  /* 「足す」を行より上に常設する列。押しても動かない位置に置く(§09 規則 1) */
  .ro-add-row {
    display: flex; flex-wrap: wrap; gap: 6px;
    margin-bottom: 9px; padding-bottom: 9px; border-bottom: 1px dashed var(--border-soft);
  }
  .ro-add { margin-top: 8px; max-width: 300px; }
  /* 部位の行に「何が付いているか」を短い名前のバッジで並べる。
     名前をそのまま出すと 1 行に入らないので、gamedata の short を使う */
  .ro-badges {
    min-width: 0; display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 4px;
  }
  .ro-badge {
    padding: 1px 7px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg-sub);
    font-size: 9px; font-weight: 700; white-space: nowrap;
  }
  /* 計算に入らない(記録するだけの)枠は破線 + 塗りなし */
  .ro-badge.record-only { background: none; border-style: dashed; color: var(--fg-muted); }
  /* 畳んだ残り数。数そのものは主役ではないので面を持たせない */
  .ro-badge.more { background: none; border-color: var(--border-soft); color: var(--fg-dim); }

  .contrib-card {
    margin-top: 8px; display: flex; align-items: baseline; gap: 9px; flex-wrap: wrap;
    padding: 8px 11px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #EFF5FD); border: 1px solid #9FB4D0;
  }
  .contrib-card.empty { background: var(--bg-rail); border-style: dashed; border-color: var(--border); }
  .contrib-label { font-size: 10px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); }
  .contrib-value { font-size: 15px; font-weight: 700; }
  .contrib-note { font-size: 9px; line-height: 1.5; }
  .item-search {
    margin-top: 8px; width: 100%; box-sizing: border-box; padding: 7px 9px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg); font-size: 11px;
  }
  .item-search:focus { outline: none; border-color: var(--accent); }
  /* 変種をまとめた行は 2 段ぶんの高さがあるので、220px だと 3 行しか入らず
     チップが毎回途中で切れる。ペインの下は空いているので伸ばす */
  .item-list { margin-top: 5px; max-height: 300px; overflow-y: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 4px; }
  /* AF は攻撃/耐久バッジも判断材料。カードを細分化して名前を潰さず、2列程度で読む。 */
  .item-list.effectful { grid-template-columns: repeat(auto-fill, minmax(min(430px, 100%), 1fr)); }
  .item-row {
    display: flex; align-items: center; gap: 7px;
    min-width: 0; min-height: 42px; padding: 5px 7px; border-radius: var(--r-panel); background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .item-row:hover { border-color: var(--accent); }
  .item-row.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); }
  .item-name { min-width: 0; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item-vals { flex-shrink: 0; font-size: 9.5px; }
  .item-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 1px; }
  .item-copy .item-vals { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item-row .part-dmg { flex-shrink: 0; }
  .custom-name { margin-top: 6px; }
  .registration-name-card .custom-name { margin: 0; display: grid; grid-template-columns: minmax(180px, .7fr) minmax(260px, 1.3fr); align-items: center; gap: 10px; }
  .selected-equipment { display: flex; align-items: center; gap: 8px; min-height: 36px; }
  .selected-equipment-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; line-height: 1.15; }
  .selected-equipment-copy b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .selected-equipment .btn { flex-shrink: 0; }
  .contrib-inline { flex-shrink: 0; color: var(--fg-sub); font-size: 10px; }
  .equipment-picker { margin-top: 6px; padding: 6px; border-radius: var(--r-inset); }
  .picker-tools { display: flex; align-items: center; gap: 6px; }
  .picker-tools .item-search { flex: 1; }
  .relic-selector { display: grid; gap: 14px; max-width: 480px; }
  .relic-picker-actions { display: flex; gap: 8px; padding-top: 2px; }

  /* 地域タブの見た目は app.css の `.tabs` / `.tab`(§08)。ここには置き場所だけ */
  .tabs { margin-top: 2px; }
  .tab .num { font-size: 9.5px; margin-left: 5px; }
  /* ゲームの地域カードと同じく、地域ごとの「コアセット効果」をその地域に出す */
  .tab-set {
    margin-left: 4px; padding: 0 5px; border-radius: var(--r-pill);
    background: var(--state-met-bg); border: 1px solid var(--good-soft);
    font-size: 9px; font-weight: 700; color: var(--good);
  }
  .tab-set.off { background: var(--state-edge-bg); border-color: var(--gold); color: var(--state-edge-fg); }
  .tab-rule { margin-bottom: 9px; }

  /* 同じ形のステが 8 個並ぶ場所の共通形。ラベルを左に置いて 1 ステ 1 行にする —
     ラベルを上に置くと 1 件で 2 行使い、8 件で画面が埋まる(§00 01 / 02) */
  /* キャラの顔で選ぶ場所(登録ペインと同じ形。見た目は app.css の .pick に置いてある) */
  .pick-grid { margin-top: 7px; display: flex; flex-wrap: wrap; gap: 6px; }
  .char-now { display: flex; align-items: center; gap: 9px; }
  .char-now .char-name { font-size: 13px; font-weight: 700; }
  .char-now .chip { margin-left: 4px; }
  .stage-field { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .stage-row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .stage-row > :global(.chip) { flex: none; }
  /* 段が 2 つしか無いと、幅なりでは細すぎて押す場所に見えない。段の大きさを先に決める */
  .stage-row > :global(.step-select) { flex: none; }
  .stage-row :global(.seg button) { min-width: 46px; padding: 7px 0; font-size: 13px; }
  /* 共通スキルの欄。ラベル・段・外す・効いている値を 1 行に、列をそろえて並べる */
  .skill-fields { margin-top: 9px; display: flex; flex-direction: column; gap: 7px; }
  /* 列: ラベル / 段 / 操作(見え方のスイッチ・外す) / 効いている値。
     操作が無い行でも列は残すので、どの行も同じ位置で並ぶ */
  .skill-field { display: grid; grid-template-columns: 100px minmax(0, 1fr) 104px 100px; gap: 8px; align-items: center; }
  .skill-actions { display: flex; align-items: center; justify-content: flex-end; gap: 6px; white-space: nowrap; }
  .chip-row { display: flex; align-items: center; gap: 6px; }
  .skill-field .k { font-size: 10px; letter-spacing: 0.06em; color: var(--fg-muted); }
  .skill-field .v { white-space: nowrap; }
  .skill-field .v { text-align: right; font-size: 12.5px; font-weight: 700; color: var(--fg-sub); }
  /* ほぼ押さない操作。段に 1 列渡さず、言葉のまま小さく置く(記号にしない) */
  .skill-field .clear {
    padding: 3px 4px; border-radius: var(--r-inset);
    background: none; border: 0; color: var(--fg-dim); font-size: 9.5px; line-height: 1;
  }
  .skill-field .clear:hover:not(:disabled) { background: var(--state-short-bg); color: var(--danger); }
  .skill-field .clear:disabled { color: var(--border-soft); }
  .skill-note { margin: 0 0 0 113px; }
  /* 3 つのチップは段 1 つぶんの幅に収まらないので、操作の列まで使う */
  .ultimate-row { grid-column: 2 / 4; display: flex; flex-wrap: wrap; gap: 6px; }
  /* 2 件ぶんの高さを先に取る。選んだ数で下が上下しない */
  .ultimate-notes { min-height: 30px; }
  .ultimate-notes .skill-note { min-height: 15px; }

  .skill-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .skill-chip .skill-name { font-weight: 700; }
  .skill-chip .skill-meta {
    padding: 0 5px; border-radius: var(--r-pill);
    background: var(--surface-inset); font-size: 9px; color: var(--fg-muted);
  }
  .skill-chip.on .skill-meta { background: rgba(255, 255, 255, 0.7); color: var(--sel-fg); }
  /* 未収録は破線 + ? で、空白や「単体」で埋めない(§00) */
  .skill-chip .skill-meta.unknown { background: none; border: 1px dashed var(--border); }
  .skill-all { margin-top: 7px; max-width: 320px; }
  .element-auto { margin: 0; display: flex; align-items: center; gap: 8px; font-size: 12px; }
  .part-switches, .part-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 5px; padding: 2px 4px; }
  .part-switches button { position: relative; cursor: grab; user-select: none; -webkit-user-drag: element; display: inline-flex; align-items: center; gap: 5px; border: 1px solid var(--border-soft); border-radius: var(--r-pill); background: var(--bg-panel); padding: 2px 7px 2px 3px; font-size: 10px; }
  .part-switches button:active { cursor: grabbing; }
  .part-switches button.on { border-color: var(--accent); color: var(--accent-hover); background: var(--state-goal-bg); }
  .part-switches button.dragging { opacity: .45; }
  .part-switches button.drop-before::before, .part-switches button.drop-after::after {
    content: ""; position: absolute; top: -3px; bottom: -3px; width: 2px;
    background: var(--accent); border-radius: var(--r-pill);
  }
  .part-switches button.drop-before::before { left: -4px; }
  .part-switches button.drop-after::after { right: -4px; }
  .registration-grip { color: var(--fg-off); font-size: 10px; line-height: 1; }
  .part-switches button:hover .registration-grip, .part-switches button.dragging .registration-grip { color: var(--accent); }
  .registration-order { margin: 7px 4px 3px; padding: 7px; border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--surface-inset); }
  .delete-registration { width: 174px; color: var(--danger); border-color: var(--state-short-bd); }
  .delete-registration.confirm { background: var(--state-short-bg); font-weight: 700; }
  .editing-registration { background: var(--state-temp-bg); border-color: var(--state-temp-bd); color: var(--state-temp-fg); }
  .values-paired { width: 100%; display: grid; grid-template-columns: minmax(0, 1fr); gap: 3px; margin-top: 4px; }
  .value-pair { width: min(100%, 520px); display: grid; grid-template-columns: 44px 464px; gap: 6px; align-items: center; }
  .value-pair.plan-stat { width: min(100%, 800px); grid-template-columns: 44px 464px minmax(220px, 1fr); }
  .value-pair.secondary-stat { opacity: .82; }
  .value-equation { display: grid; grid-template-columns: 102px 52px 214px 62px; gap: 6px; align-items: center; }
  .equation-base { width: 62px; display: grid; align-items: center; }
  .equation-base :global(.stepper .cell.bare) { width: 62px; min-width: 62px; }
  .equation-enchant { width: 214px; display: grid; align-items: center; }
  .enchant-plan {
    min-width: 0; min-height: 30px; padding: 3px 7px; display: grid;
    grid-template-columns: 75px minmax(0, 1fr) 34px; align-items: center; gap: 7px;
    border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--surface-inset);
  }
  .plan-remaining { display: flex; align-items: baseline; gap: 5px; white-space: nowrap; }
  .plan-remaining small { color: var(--fg-muted); font-size: 8.5px; }
  .plan-remaining b { color: var(--accent-hover); font-size: 12.5px; }
  .plan-recipe { min-width: 0; color: var(--fg-sub); font-size: 9.5px; font-weight: 700; white-space: nowrap; }
  .enchant-plan > .badge { min-width: 34px; text-align: center; }
  .enchant-plan.complete .plan-remaining b, .enchant-plan.complete .plan-recipe { color: var(--state-edge-fg); }
  .base-value-toolbar { width: min(100%, 520px); min-height: 29px; margin-top: 5px; padding-bottom: 5px; border-bottom: 1px dashed var(--border-soft); display: grid; grid-template-columns: 44px 102px 52px 214px 62px; column-gap: 6px; align-items: center; }
  .base-value-copy { grid-column: 5; display: flex; align-items: baseline; justify-content: center; gap: 3px; white-space: nowrap; }
  .base-value-copy b { font-size: 9.5px; color: var(--fg-sub); }
  .base-value-copy small { color: var(--fg-muted); font-size: 9px; }
  .growth-equipment-card { border-color: var(--accent); box-shadow: inset 3px 0 0 var(--accent); }
  .growth-equipment-head { align-items: center; }
  .growth-equipment-values { width: min(100%, 460px); }
  .growth-equipment-values .stat-row { grid-template-columns: 76px minmax(0, 1fr); }
  .growth-equipment-values .stat-row :global(.stepper .cell) { flex: 0 1 104px; }
  .enchant-more-toggle {
    width: min(100%, 520px); grid-column: 1 / -1; min-height: 29px; margin-top: 3px; padding: 4px 7px;
    border: 0; border-top: 1px dashed var(--border-soft); background: none; color: var(--fg-sub);
    display: flex; align-items: center; justify-content: space-between; text-align: left;
  }
  .enchant-more-toggle:hover { color: var(--accent-hover); }
  .enchant-more-toggle > span:first-child { display: flex; align-items: baseline; gap: 8px; }
  .enchant-more-toggle b { font-size: 10px; }
  .enchant-more-toggle small { color: var(--fg-muted); font-size: 9px; }
  .enchant-more-toggle .toggle-state { width: 56px; text-align: right; color: var(--accent-hover); font-size: 9.5px; }
  .value-total { width: 102px; display: grid; grid-template-columns: 38px 62px; gap: 2px; align-items: baseline; white-space: nowrap; color: var(--accent-hover); }
  .total-main { width: 38px; font-size: 13px; text-align: right; }
  .enchant-part { width: 62px; color: var(--fg-muted); font-size: 9px; font-weight: 500; text-align: left; }
  .ability-part { width: 52px; padding: 1px 4px; border-radius: var(--r-pill); background: var(--surface-inset); color: var(--accent-hover); font-size: 8.5px; font-weight: 700; text-align: center; white-space: nowrap; }
  .ability-spacer { width: 52px; }
  .enchant-card { border-color: var(--accent); box-shadow: inset 3px 0 0 var(--accent); }
  .enchant-card .hint { margin: 4px 0 0; }
  .equipment-filter { white-space: nowrap; }
  @media (max-width: 850px) {
    .values-paired, .base-value-toolbar { width: min(100%, 508px); }
    .base-value-toolbar { grid-template-columns: 40px 94px 48px 214px 58px; }
    .value-pair { grid-template-columns: 40px 462px; }
    .value-pair.plan-stat { width: min(100%, 508px); grid-template-columns: 40px 462px; }
    .enchant-plan { grid-column: 2; margin-top: 2px; }
    .value-equation { grid-template-columns: 94px 48px 214px 58px; }
    .equation-base { width: 58px; }
    .equation-base :global(.stepper .cell.bare) { width: 58px; min-width: 58px; }
    .equation-enchant { width: 214px; }
    .value-total { width: 94px; grid-template-columns: 36px 56px; }
    .total-main { width: 36px; }
    .enchant-part { width: 56px; }
    .ability-part, .ability-spacer { width: 48px; }
  }
  .element-auto b { font-size: 13px; }

  .stat-rows { margin-top: 8px; display: grid; grid-template-columns: 1fr; gap: 5px; }
  /* 2 列にするのは中身が収まるときだけ。狭いと右の列にはみ出して隣の行に重なる */
  .stat-rows.two { grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 5px 16px; }
  .crown-choice {
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding-bottom: 8px; border-bottom: 1px dashed var(--border-soft);
  }
  .crown-choice-label { font-size: 10px; letter-spacing: 0.08em; color: var(--fg-muted); }
  .crown-choice-stats { display: flex; gap: 5px; flex-wrap: wrap; }
  .crown-presets { display: flex; gap: 5px; margin-left: auto; }
  .crown-choice .hint { flex-basis: 100%; }
  .stat-row {
    display: grid; grid-template-columns: 42px minmax(0, 1fr) auto; gap: 9px; align-items: center;
    padding-bottom: 5px; border-bottom: 1px dashed var(--border-soft);
  }
  .stat-row .k { font-size: 10px; letter-spacing: 0.06em; color: var(--fg-muted); }
  .stat-row > :global(.stepper) { min-width: 0; }
  /* 入力欄は数値 2〜4 桁ぶんあれば足りる。行いっぱいに伸ばすと、数字と ＋ / MAX が離れて
     視線が横に流れる(§00 01)。全行が同じ幅なので端もそろう */
  .stat-rows.two .stat-row :global(.stepper .cell) { flex: 0 1 104px; }
  .stat-row .v { min-width: 44px; text-align: right; font-size: 12px; font-weight: 700; color: var(--fg-sub); }

  .core-list { display: flex; flex-direction: column; gap: 7px; }
  /* 読み取り専用の要約なのでインセット。面の色ではなくバッジで状態を言う
     (意味のある帯・面は 1 画面 2 種まで。§02) */
  .core-head {
    display: grid; grid-template-columns: 16px minmax(0, 1fr) 34px 118px 52px; gap: 9px;
    margin-bottom: 5px; font-size: 9px; letter-spacing: 0.1em; color: var(--fg-dim);
  }
  .core-head .lead { text-align: center; color: var(--fg-muted); font-weight: 700; }
  .core-head .r { text-align: right; }
  /* 1 枠 = 1 行。番号・タイプ・進化-強化・補正値が同じ高さに並ぶので、
     6 枠を上から一度の視線で読める(§00「視線を動かさない」) */
  .core-row {
    display: grid; grid-template-columns: 16px 1fr 34px 66px 48px;
    gap: 9px; align-items: center;
    padding-bottom: 7px; border-bottom: 1px dashed var(--border-soft);
  }
  .core-row:last-child { padding-bottom: 0; border-bottom: 0; }
  .core-slot { font-size: 11px; text-align: center; }
  .core-types { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  /* 外すのは「ほぼ押さない操作」なので、段ではなく小さな 1 つに落とす。
     ただし記号(×)にしない — 意味を読み取らせるより、言葉で言い切るほうが速い */
  .core-clear {
    padding: 3px 4px; border-radius: var(--r-inset);
    background: none; border: 0; color: var(--fg-dim); font-size: 9.5px; line-height: 1;
  }
  .core-clear:hover:not(:disabled) { background: var(--state-short-bg); color: var(--danger); }
  .core-clear:disabled { color: var(--border-soft); }
  .core-stage { position: relative; }
  /* この行の主役。どのコアかより「どこまで育てたか」を見に来ているので、
     ここだけ大きく・濃くする(§00「触る場所だけ大きく」) */
  .stage-trigger {
    width: 100%; padding: 9px 0; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 #fff;
    font-size: 17px; font-weight: 800; color: var(--fg); letter-spacing: 0.06em;
  }
  .stage-trigger:hover:not(:disabled) { border-color: var(--accent); background: var(--bg-active); }
  .stage-trigger:disabled { color: var(--fg-off); }
  /* 重なって出るので、開いても行は動かない(§09 規則 3) */
  .stage-overlay { position: fixed; inset: 0; z-index: 30; cursor: default; }
  /* 右端に近い場所から開くので、右をそろえて左へ広げる。中央合わせだと枠から溢れる */
  .stage-pop {
    position: absolute; z-index: 31; top: calc(100% + 5px); right: 0;
    padding: 8px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-strong);
    box-shadow: 0 10px 24px rgba(30, 44, 74, 0.28);
  }
  .stage-pop.up { top: auto; bottom: calc(100% + 5px); transform-origin: bottom center; }
  .stage-pop-h { margin-bottom: 5px; font-size: 9px; letter-spacing: 0.1em; color: var(--fg-dim); }
  .stage-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 4px; }
  .stage-cell {
    display: flex; flex-direction: column; align-items: center; gap: 1px;
    width: 60px; padding: 6px 0 5px; border-radius: var(--r-inset);
    background: var(--bg-rail); border: 1px solid var(--border-soft);
    white-space: nowrap;
  }
  .stage-cell b { font-size: 13px; font-weight: 800; color: var(--fg-sub); letter-spacing: 0.04em; }
  .stage-cell .cell-bonus { font-size: 9px; color: var(--fg-dim); }
  .stage-cell.on b, .stage-cell.on .cell-bonus { color: var(--sel-fg); }
  .stage-cell:hover:not(.on) { background: var(--bg-active); }
  .stage-cell.on { background: var(--sel); border-color: var(--sel-bd); color: var(--sel-fg); font-weight: 700; }
  .core-bonus { font-size: 12px; font-weight: 700; text-align: right; }
  .core-bonus.support { color: var(--fg-muted); font-weight: 400; }

</style>
