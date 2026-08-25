<script lang="ts" module>
  export type SourceId =
    | "status"
    | "equipment"
    | "element"
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
    CoreRegion, CoreType, Element, ElementPreview, EquipmentAbilityFamily, EquipmentItem, PartSlot,
    PetSkillTier, RandomOptionDef, RandomOptionRank, Skill, StatKind, StatPreview, TitleDef,
    UltimateSkill,
  } from "../../api/types";
  import { isAllySkill, isCharacterSkillFor, isFixedValue, toggleBuff } from "../../buffs";
  import { previewElements } from "../../api/commands";
  import { draftToPayload, type Draft } from "../../draft";
  import {
    clampToCaps, coreBonus, coreSetSupportValues, coreSetTotalBonus, midpointValues,
    neutralEquipmentPart, neutralSienaAura, randomOptionEffectLabel, randomOptionIsApplied,
    randomOptionValue, rangeSummary, sienaPartStatTotal, valuesSummary,
  } from "../../equipment";
  import { fmtInt, formatLayerValue } from "../../format";
  import {
    ABILITY_ALLOWED_SLOTS, ABILITY_FAMILIES, ABILITY_FAMILY_LABELS, CORE_POWER_TYPES, CORE_REGION_LABELS, CORE_REGIONS, CORE_SLOT_COUNT,
    CORE_SUPPORT_TYPES, CORE_TYPE_LABELS, ELEMENT_ALLOWED_SLOTS, ELEMENT_LABELS, ELEMENTS,
    ENHANCE_ALLOWED_SLOTS,
    EQUIPMENT_ELEMENTS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT,
    PART_SLOT_LABELS, PART_SLOTS, PET_SKILL_TIER_LABELS,
    RANDOM_OPTION_ALLOWED_SLOTS, RANDOM_OPTION_RANK_LABELS, RANDOM_OPTION_RANKS,
    SIENA_ALLOWED_SLOTS,
    SIENA_EQUIPMENT_VALUE_SLOTS, STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS,
    ULTIMATE_SKILLS, ULTIMATE_SKILL_EFFECTS, ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { app } from "../../state.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import Select from "../../ui/Select.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import StatInput from "../../ui/StatInput.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    previewError: string | null;
    /** 主軸スキルの選択肢(キャラ種のスキル一覧)。親が引く */
    skills: Skill[];
    sourceId: SourceId;
  }
  let { draft, preview, previewError, skills, sourceId }: Props = $props();

  const STAT_MIN = 1;

  // --- 中ディレイ減少(wiki: ステータス「中ディレイ倍率B」)---------------------
  // カタログはキャラを問わず全件持っているので、このキャラのぶんだけ出す。
  const delaySkills = $derived(
    app.actualDelaySkills.filter((d) => d.game_character_id === draft.gameCharacterId),
  );
  function toggleDelaySkill(id: string, on: boolean) {
    const rest = draft.statSources.actual_delay_skills.skill_ids.filter((x) => x !== id);
    draft.statSources.actual_delay_skills.skill_ids = on ? [...rest, id] : rest;
  }
  /** クリティカル率増加の合計(上限を掛ける前) */
  const criticalRateBonus = $derived(
    (draft.statSources.critical_rate.ultimate_rune ? 20 : 0) +
      (draft.statSources.critical_rate.architect_lab ? 30 : 0) +
      (draft.statSources.critical_rate.deadly_blow ? 100 : 0),
  );

  /** このキャラのパッシブぶんの中ディレイ減少 %(共通の供給源は含まない) */
  const delaySkillPercent = $derived(
    draft.statSources.actual_delay_skills.skill_ids.reduce(
      (n, id) => n + (app.actualDelaySkills.find((d) => d.id === id)?.percent ?? 0),
      0,
    ),
  );
  const characterOptions = $derived(app.gameCharacters.map((c) => ({ value: c.id, label: c.name })));

  // 主軸スキル。未収録のキャラがあるので未選択("")を許す。
  // キャラ種を変えたら前キャラのスキル id が残らないよう同期的に外す(保存時に Rust 側が弾く値)。
  const mainSkillOptions = $derived([
    { value: "", label: "未選択(攻撃力を出さない)" },
    ...skills.map((s) => ({ value: s.id, label: s.name })),
  ]);
  function setGameCharacterId(id: string) {
    if (id === draft.gameCharacterId) return;
    draft.gameCharacterId = id;
    draft.mainSkillId = "";
  }
  const stageOptions = Array.from({ length: 6 }, (_, i) => ({ value: String(i), label: `${i} 段階` }));
  // wiki エタの意志「エタの成長」は Lv100(MAX)まで
  const eternalOptions = $derived(
    Array.from({ length: limits.eternal_level_max + 1 }, (_, i) => ({ value: String(i), label: `Lv ${i}` })),
  );

  const PET_TIERS: PetSkillTier[] = ["basic", "true_lv1", "true_lv2", "true_lv3", "true_lv4"];
  const petSkillOptions = [
    { value: "", label: "なし" },
    ...PET_TIERS.map((t) => ({ value: t, label: PET_SKILL_TIER_LABELS[t] })),
  ];
  const petSkillValue = (k: StatKind) => draft.statSources.pet_skills[k] ?? "";
  const setPetSkillValue = (k: StatKind, v: string) => {
    draft.statSources.pet_skills[k] = (v === "" ? null : v) as PetSkillTier | null;
  };

  const strongWeaponOptions = $derived([
    { value: "0", label: "なし" },
    ...Array.from({ length: limits.strong_weapon_level_max }, (_, i) => {
      const lv = i + 1;
      return { value: String(lv), label: `Lv${lv}(+${lv * 3}%)` };
    }),
  ]);

  const ownSkillBuffs = $derived(app.catalog.filter((d) => isCharacterSkillFor(d, draft.gameCharacterId)));
  const allySkillBuffs = $derived(app.catalog.filter(isAllySkill));
  const buffChecked = (buffId: string) => draft.statSources.buffs.choices.some((c) => c.buff_id === buffId);

  // --- 装備ドリルダウン(部位一覧 ⇄ 部位詳細) --------------------------------
  let openPart = $state<PartSlot | null>(null);
  let itemQuery = $state("");
  const openPartLabel = $derived(openPart ? PART_SLOT_LABELS[openPart] : "");
  const catalogFor = $derived(
    openPart ? app.equipmentCatalog.filter((i) => i.slot === openPart) : [],
  );
  const filteredCatalog = $derived(
    itemQuery.trim() === "" ? catalogFor : catalogFor.filter((i) => i.name.includes(itemQuery.trim())),
  );
  const equippedItem = (slot: PartSlot): EquipmentItem | null => {
    const itemId = draft.equipment.parts[slot].item_id;
    return itemId ? (app.equipmentCatalog.find((i) => i.id === itemId) ?? null) : null;
  };
  const partDisplayName = (slot: PartSlot): string => {
    const item = equippedItem(slot);
    if (item) return item.name;
    const custom = draft.equipment.parts[slot].custom_name;
    return custom ? `${custom} [仮]` : "未装備";
  };

  function pickCatalogItem(slot: PartSlot, item: EquipmentItem) {
    const part = draft.equipment.parts[slot];
    part.item_id = item.id;
    part.custom_name = null;
    part.base = midpointValues(item.values_min, item.values_max);
    part.enchant = clampToCaps(part.enchant, item.enchant_caps);
    itemQuery = "";
  }
  function pickUnequipped(slot: PartSlot) {
    draft.equipment.parts[slot] = neutralEquipmentPart();
    itemQuery = "";
  }
  function pickCustom(slot: PartSlot) {
    const part = draft.equipment.parts[slot];
    part.item_id = null;
    if (part.custom_name === null) part.custom_name = "";
    itemQuery = "";
  }

  /** その部位の攻撃力(A)への寄与(外すと減る量)。主軸スキル未選択なら null */
  const partContribution = (slot: PartSlot): number | null =>
    preview?.attack?.part_contributions.find((c) => c.slot === slot)?.value ?? null;

  // アビリティは系統(尖った刃 / 鋭い刃 / 知力 / 耐魔力)ごとに 1 つだけ。Select 4 行にして
  // 排他を構造で保証する(段違いの重複チェックが通ってしまう問題の解消。storage 側も検証する)。
  const abilityDef = (id: string) => app.equipmentAbilities.find((a) => a.id === id) ?? null;
  const abilityOptions = (family: EquipmentAbilityFamily) => [
    { value: "", label: "なし" },
    ...app.equipmentAbilities.filter((a) => a.family === family).map((a) => ({ value: a.id, label: a.name })),
  ];
  /** その部位でこの系統に選ばれているアビリティ id(未選択は "") */
  const abilityOf = (slot: PartSlot, family: EquipmentAbilityFamily): string =>
    draft.equipment.parts[slot].abilities.find((id) => abilityDef(id)?.family === family) ?? "";
  /** 同じ系統の既存選択を必ず 1 つに置き換える(旧データの重複もここで解消される) */
  function setAbility(slot: PartSlot, family: EquipmentAbilityFamily, id: string) {
    const part = draft.equipment.parts[slot];
    const others = part.abilities.filter((a) => abilityDef(a)?.family !== family);
    part.abilities = id === "" ? others : [...others, id];
  }
  /** 部位詳細を開いたときに、旧データの同系統重複を 1 つへ畳む(保存時に弾かれる値を残さない) */
  function openPartDetail(slot: PartSlot) {
    const part = draft.equipment.parts[slot];
    const seen = new Set<string>();
    const normalized = part.abilities.filter((id) => {
      const family = abilityDef(id)?.family;
      if (family === undefined || seen.has(family)) return false;
      seen.add(family);
      return true;
    });
    if (normalized.length !== part.abilities.length) part.abilities = normalized;
    openPart = slot;
  }

  // --- 称号 ---------------------------------------------------------------
  // 装備枠 1 つ。表示中の 1 件だけが効く(所持ぶんの累積ではない。wiki: 称号システム)。
  let titleQuery = $state("");
  const selectedTitle = $derived(app.titles.find((t) => t.id === draft.equipment.title) ?? null);
  const filteredTitles = $derived.by(() => {
    const q = titleQuery.trim();
    if (q === "") return app.titles;
    return app.titles.filter((t) => t.name.includes(q) || t.group.includes(q));
  });
  /** 称号の補正値の要約(値が入っている列だけ)。 */
  const titleSummary = (t: TitleDef): string =>
    EQUIPMENT_STAT_KINDS.filter((k) => t.values[k] !== 0)
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]}${t.values[k]}`)
      .join(" ");

  // --- ランダムオプション -------------------------------------------------
  // 効果値の上限は wiki の一覧表のレンジそのもの。枠数は wiki に記載が無く、代わりに
  // 「同じカテゴリーは 1 部位に 1 つまで(カテゴリー 0 は除く)」で縛る(転移の説明)。
  const randomOptionDef = (id: string): RandomOptionDef | undefined =>
    app.randomOptions.find((d) => d.id === id);

  /** その部位に足せる OP(未選択 かつ カテゴリーが空いているもの) */
  function addableRandomOptions(slot: PartSlot) {
    const part = draft.equipment.parts[slot];
    const takenIds = new Set(part.random_options.map((o) => o.option_id));
    const takenCategories = new Set(
      part.random_options.map((o) => randomOptionDef(o.option_id)?.category).filter((c) => c !== undefined && c !== 0),
    );
    return [
      { value: "", label: "追加する OP を選ぶ" },
      ...app.randomOptions
        .filter((d) => d.slot === slot && !takenIds.has(d.id) && !takenCategories.has(d.category))
        .map((d) => ({ value: d.id, label: d.name })),
    ];
  }

  function addRandomOption(slot: PartSlot, id: string) {
    if (id === "") return;
    const def = randomOptionDef(id);
    if (!def || def.tiers.length === 0) return;
    // 既定は一覧のいちばん上位のランク(手持ちがそれ未満なら下げてもらう)
    const rank = def.tiers[def.tiers.length - 1].rank;
    draft.equipment.parts[slot].random_options = [
      ...draft.equipment.parts[slot].random_options,
      { option_id: id, rank, value: null },
    ];
  }

  function removeRandomOption(slot: PartSlot, index: number) {
    const part = draft.equipment.parts[slot];
    part.random_options = part.random_options.filter((_, i) => i !== index);
  }

  const rankOptions = (def: RandomOptionDef) =>
    RANDOM_OPTION_RANKS.filter((r) => def.tiers.some((t) => t.rank === r)).map((r) => ({
      value: r,
      label: RANDOM_OPTION_RANK_LABELS[r],
    }));

  /** ランクを変えるとレンジが変わるので、実測の上書きは外して既定(レンジ上限)へ戻す */
  function setRandomOptionRank(slot: PartSlot, index: number, rank: RandomOptionRank) {
    const option = draft.equipment.parts[slot].random_options[index];
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
  const UNLEASH_RATES = [1, 2, 3, 4, 5, 8, 11, 14, 17, 20];
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
  const unleashStatOptions = (slotIndex: number) => {
    const other = draft.commonSkills.unleash[1 - slotIndex].stat;
    return [
      { value: "", label: "未使用" },
      ...STAT_KINDS.filter((k) => k !== other).map((k) => ({ value: k, label: STAT_LABELS[k] })),
    ];
  };
  const unleashLevelOptions = $derived(
    Array.from({ length: Math.min(limits.unleash_level_max, reinforceGate) + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? "未習得" : `Lv${lv}(+${UNLEASH_RATES[lv - 1]}%)`,
    })),
  );

  const augmentOptions = $derived(
    Array.from({ length: limits.augment_level_max + 1 }, (_, i) => ({
      value: String(i),
      label: i === 0 ? "未習得" : `Lv${i}`,
    })),
  );
  /** オーグメントで解放されている Lv までを選択肢にする */
  const gatedLevelOptions = (max: number, label: (lv: number) => string) =>
    Array.from({ length: max + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? "未習得" : label(lv),
      disabled: lv > augmentGate,
    })).filter((o) => !o.disabled);
  const protectArmorOptions = $derived(
    gatedLevelOptions(
      limits.protect_armor_level_max,
      (lv) => `Lv${lv}(物+${[36, 45, 54, 63, 72, 81][lv - 1]}% / 魔+${[24, 30, 36, 42, 48, 54][lv - 1]}%)`,
    ),
  );
  const kaiProtectArmorOptions = Array.from({ length: 6 }, (_, lv) => ({
    value: String(lv),
    label: lv === 0 ? "未習得" : `Lv${lv}(物+${lv * 9}% / 魔+${lv * 6}%)`,
  }));
  const sharpnessVisionOptions = Array.from({ length: 11 }, (_, lv) => ({
    value: String(lv),
    label: lv === 0 ? "未習得" : `Lv${lv}(+${[5, 10, 15, 20, 25, 28, 31, 34, 37, 40][lv - 1]}%)`,
  }));
  /** 装備防御力倍率(共通スキル + シエナのオーラの防御力増加)。表示用 */
  const sienaDefenseRate = $derived(
    PART_SLOTS.reduce((n, slot) => n + draft.equipment.parts[slot].siena.defense_rate_percent, 0),
  );
  const defenseRatePercent = $derived.by(() => {
    const c = draft.commonSkills;
    const pa = c.protect_armor_level;
    const kai = c.kai_protect_armor_level;
    const physical =
      (c.coat_armor ? 18 : 0) +
      (pa > 0 ? [36, 45, 54, 63, 72, 81][pa - 1] : 0) +
      kai * 9 +
      sienaDefenseRate;
    const magic =
      (c.coat_armor ? 12 : 0) +
      (pa > 0 ? [24, 30, 36, 42, 48, 54][pa - 1] : 0) +
      kai * 6 +
      sienaDefenseRate;
    return { physical: 100 + physical, magic: 100 + magic };
  });

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
    return [
      { value: "", label: "未習得" },
      ...ULTIMATE_SKILLS.filter((u) => u !== other).map((u) => ({
        value: u,
        label: ULTIMATE_SKILL_LABELS[u],
      })),
    ];
  }
  function setUltimate(slotIndex: number, value: string) {
    draft.commonSkills.ultimate.slots[slotIndex] = value === "" ? null : (value as UltimateSkill);
  }
  /** 選択中の極限スキルの効果値(表示用。計算は Rust 側) */
  const ultimateEffects = $derived.by(() => {
    const u = draft.commonSkills.ultimate;
    const superLimit = u.super_limit;
    const lv = u.hyper_limit_level;
    const hyper = (table: number[]) => (lv === 0 ? 0 : table[lv - 1]);
    const out: string[] = [];
    if (u.slots.includes("scope_eye")) {
      out.push(`クリティカルダメージ +${20 + (superLimit ? 3 : 0) + hyper([7, 9, 11, 13, 15, 17])}%`);
    }
    if (u.slots.includes("full_throttle")) {
      const hits = hyper([0, 0, 0, 1, 2, 3]);
      out.push(`中ディレイ −${25 + (superLimit ? 3 : 0) + hyper([7, 9, 11, 13, 15, 17])}%`);
      out.push(`単体チャネリング段数 +${hits}`);
    }
    if (u.slots.includes("wide_focus")) {
      out.push(`スキル範囲 +${4 + (superLimit ? 2 : 0) + hyper([4, 6, 8, 10, 12, 14])}`);
    }
    return out;
  });

  const enhanceRatePercent = $derived(
    (draft.commonSkills.power_weapon ? 2 : 0) + draft.commonSkills.strong_weapon_level * 3,
  );

  const enhanceLevelOptions = $derived(
    Array.from({ length: limits.enhance_level_max + 1 }, (_, lv) => ({
      value: String(lv), label: lv === 0 ? "強化なし" : `+${lv}`,
    })),
  );
  function setEnhanceLevel(slot: PartSlot, level: number) {
    const part = draft.equipment.parts[slot];
    part.enhance_level = level;
    // +11 以下は追加固定ダメージの上書きを許可しない(domain 側の制約に合わせる)
    if (level < 12) part.enhance_added_damage = null;
  }

  // --- シエナのオーラ(部位ごと) ------------------------------------------
  let openSienaPart = $state<PartSlot | null>(null);
  const sienaStageOptions = $derived(
    Array.from({ length: limits.siena_stage_max + 1 }, (_, i) => ({
      value: String(i),
      label: i === 0 ? "未発現" : `${i} 段階(能力値 ${i} 枠)`,
    })),
  );
  /** 段階 0 に戻したらその部位のオーラを丸ごと中立に戻す(値だけ残る幽霊状態を作らない) */
  function setSienaStage(slot: PartSlot, stage: number) {
    if (stage === 0) {
      draft.equipment.parts[slot].siena = neutralSienaAura();
      return;
    }
    draft.equipment.parts[slot].siena.stage = stage;
  }
  const sienaIsEquipmentValues = (slot: PartSlot) => SIENA_EQUIPMENT_VALUE_SLOTS.includes(slot);

  // --- 属性(装備の属性強化以外の供給源) ----------------------------------
  const elementSourceDefs = $derived(app.elementSources);
  // 内訳は Rust 側で出す(キャラ基礎属性値は gamedata にしか無い)。開いている間だけ引く
  let elementPreview = $state<ElementPreview | null>(null);
  let elementSeq = 0;
  $effect(() => {
    if (sourceId !== "element") return;
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

  // --- 属性強化(部位ごとに 1 属性) --------------------------------------
  const elementOptions = [
    { value: "", label: "属性なし" },
    ...EQUIPMENT_ELEMENTS.map((e) => ({ value: e, label: ELEMENT_LABELS[e] })),
  ];
  function setPartElement(slot: PartSlot, value: string) {
    const part = draft.equipment.parts[slot];
    part.element = value === "" ? null : (value as Element);
    // 属性を外したら値も消す(0 と「属性なし」を食い違わせない)
    if (part.element === null) part.element_value = 0;
  }
  const sienaSummary = (slot: PartSlot): string => {
    const siena = draft.equipment.parts[slot].siena;
    if (siena.stage === 0) return "未発現";
    const parts: string[] = [`${siena.stage} 段階`];
    if (sienaIsEquipmentValues(slot)) {
      const v = siena.values;
      if (v.thrust || v.slash || v.magic_attack || v.magic_defense) {
        parts.push(`突${fmtInt(v.thrust)} / 斬${fmtInt(v.slash)}`);
      }
    }
    const statTotal = sienaPartStatTotal(siena);
    if (statTotal > 0) parts.push(`ステ +${fmtInt(statTotal)}`);
    if (siena.attack_rate_percent > 0) parts.push(`攻撃力 +${siena.attack_rate_percent}%`);
    return parts.join(" ・ ");
  };

  // --- テシスコア(地域ごとに 6 枠) ---------------------------------------
  let coreRegion = $state<CoreRegion>("abyss");
  const coreSlotIndexes = Array.from({ length: CORE_SLOT_COUNT }, (_, i) => i);
  // 火力タイプと補助タイプはラベルで区別する(補助は装備攻撃力に入らない)
  const coreTypeOptions = [
    { value: "", label: "未装着" },
    ...CORE_POWER_TYPES.map((t) => ({ value: t, label: CORE_TYPE_LABELS[t] })),
    ...CORE_SUPPORT_TYPES.map((t) => ({ value: t, label: `${CORE_TYPE_LABELS[t]}(補助)` })),
  ];
  const coreStageOptions = (max: number, prefix: string) =>
    Array.from({ length: max + 1 }, (_, i) => ({ value: String(i), label: `${prefix}${i}` }));
  const coreEvolutionOptions = $derived(coreStageOptions(limits.core_evolution_max, "進化"));
  const coreEnhancementOptions = $derived(coreStageOptions(limits.core_enhancement_max, "強化"));
  const coreAt = (index: number) => draft.equipment.thesis_cores[coreRegion].slots[index] ?? null;
  function setCoreType(index: number, value: string) {
    const slots = draft.equipment.thesis_cores[coreRegion].slots;
    slots[index] = value === "" ? null : { core_type: value as CoreType, evolution: 0, enhancement: 0 };
  }
  function setCoreStage(index: number, field: "evolution" | "enhancement", value: number) {
    const core = draft.equipment.thesis_cores[coreRegion].slots[index];
    if (core) core[field] = value;
  }
  const coreRegionTotal = (region: CoreRegion) =>
    coreSetTotalBonus(draft.equipment.thesis_cores[region]);
  // 補助タイプは与ダメージ(攻撃力)には効かないが、装備値 9 種として防御側・回避Pに効く
  const coreSupport = $derived(coreSetSupportValues(draft.equipment.thesis_cores[coreRegion]));
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
    status: { title: "キャラステータス", note: "素ステ・覚醒・エタの意志" },
    equipment: { title: "装備", note: "部位ごとのアイテム・エンチャント・強化" },
    element: { title: "属性", note: "装備の属性強化以外の供給源(乗せる属性を選ぶ)" },
    pet: { title: "ペット S スキル", note: "ステごとに 1 段階" },
    rune: { title: "ルーンスキル", note: `0–${limits.rune_level_max}` },
    crown: { title: "クラウン", note: `0–${limits.crown_max}` },
    monsterCard: { title: "モンスターカード", note: `装着カードのステータス(0–${limits.monster_card_max})` },
    relic: { title: "神鳥の聖物", note: `0–${limits.sacred_relic_stage_max} 段階(実加算は段階×10)` },
    siena: { title: "シエナのオーラ", note: "Lv310 の 8 部位・増幅段階と能力値" },
    randomOption: { title: "ランダムOP", note: "部位ごとの追加効果(同じカテゴリーは 1 部位 1 つ)" },
    title: { title: "称号", note: "表示中の 1 件だけが装備の基本能力値に乗る" },
    commonSkill: { title: "共通スキル", note: "キャラ横断のパッシブ(オーグメントが Lv の前提)" },
    thesis: { title: "テシスコア", note: "地域ごとに 6 枠(能力値は対象地域内のみ有効)" },
    skills: { title: "キャラスキル", note: "自分のスキルと味方から受けるスキル" },
    actualDelay: { title: "中ディレイ減少", note: "このキャラ固有のパッシブ・マスタリー(倍率B)" },
    criticalRate: { title: "クリティカル率", note: `ペット会心と増加(上限 +${limits.critical_rate_bonus_max}%)` },
    adjust: { title: "調整", note: "検証・仮定用の例外操作" },
  };

  const traceFor = (k: StatKind) => preview?.traces.find((t) => t.kind === k) ?? null;
  const signed = (n: number) => `${n >= 0 ? "+" : ""}${fmtInt(n)}`;
</script>

<!-- ランダムオプションの編集(装備の部位詳細と「ランダムOP」ペインで共有する) -->
{#snippet randomOptionEditor(slot: PartSlot)}
  {@const part = draft.equipment.parts[slot]}
  {#each part.random_options as option, index (option.option_id)}
    {@const def = randomOptionDef(option.option_id)}
    {#if def}
      {@const t = tierOf(def, option.rank)}
      <div class="ro-row" class:record-only={!randomOptionIsApplied(def.effect)}>
        <div class="ro-head">
          <span class="ro-name">{def.name}</span>
          <span class="ro-cat dim">カテゴリー{def.category}</span>
          <span class="ro-effect dim">{randomOptionEffectLabel(def.effect)}</span>
          <button type="button" class="ro-remove" onclick={() => removeRandomOption(slot, index)}>外す</button>
        </div>
        <div class="fields">
          <StepSelect
            label="ランク"
            options={rankOptions(def)}
            bind:value={
              () => option.rank,
              (v) => setRandomOptionRank(slot, index, v as RandomOptionRank)
            }
          />
          <StatInput
            label="効果値"
            min={t ? t.min : 0}
            max={t ? t.max : limits.random_option_value_max}
            step={0.5}
            format={t ? () => `wiki ${t.min}–${t.max}` : undefined}
            bind:value={() => randomOptionValue(option, def), (v) => (option.value = v)}
          />
        </div>
        {#if def.note}<p class="hint dim">{def.note}</p>{/if}
      </div>
    {/if}
  {/each}
  <div class="fields">
    <Select
      label="OP を追加"
      options={addableRandomOptions(slot)}
      bind:value={() => "", (v) => addRandomOption(slot, v)}
    />
  </div>
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
        <Select
          label="キャラ"
          options={characterOptions}
          bind:value={() => draft.gameCharacterId, setGameCharacterId}
        />
        <div class="two">
          <StepSelect label="覚醒段階" bind:value={draft.stage} options={stageOptions} />
          <Select label="エタの意志 Lv" bind:value={draft.eternalLevel} options={eternalOptions} />
        </div>
        <Select label="主軸スキル" options={mainSkillOptions} bind:value={draft.mainSkillId} />
      </div>
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
          <thead><tr><th>ステ</th><th class="n">素</th><th class="n">補正</th><th class="n">最終</th></tr></thead>
          <tbody>
            {#each STAT_KINDS as k (k)}
              {@const trace = traceFor(k)}
              {@const diff = preview ? preview.stats[k] - draft.baseStats[k] : null}
              <tr>
                <td>{STAT_LABELS[k]}</td>
                <td class="n stat-cell">
                  <StatInput label="" min={STAT_MIN} max={limits.base_stat_max} bind:value={draft.baseStats[k]} />
                </td>
                <td class="n muted">{diff === null ? "—" : signed(diff)}</td>
                <td class="n final">
                  <span class="strong">{preview ? fmtInt(preview.stats[k]) : "—"}</span>
                  {#if trace?.pinned_from !== null && trace?.pinned_from !== undefined}
                    <span class="pin-badge" title={`固定前: ${fmtInt(trace.pinned_from)}`}>固定</span>
                  {/if}
                  {#if trace && trace.capped_loss > 0}
                    <span
                      class="cap-badge"
                      title={`上限 ${fmtInt(trace.stat_cap)} で ${fmtInt(trace.capped_loss)} 捨てています。上限は覚醒段階とエタの意志 Lv で上がります`}
                    >上限</span>
                  {/if}
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
            <table class="grid">
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
      {@const part = draft.equipment.parts[slot]}
      {@const canEnhance = ENHANCE_ALLOWED_SLOTS.includes(slot)}
      <button type="button" class="part-row" class:on={openPart === slot} onclick={() => openPartDetail(slot)}>
        <span class="part-main">
          <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
          <span class="part-item">{partDisplayName(slot)}</span>
          {#if canEnhance && part.enhance_level > 0}
            <span class="part-plus">+{part.enhance_level}</span>
          {/if}
          {#if part.abilities.length > 0}
            <span class="part-abi">アビリティ {part.abilities.length}</span>
          {/if}
          {#if part.random_options.length > 0}
            <span class="part-abi">OP {part.random_options.length}</span>
          {/if}
          {#if part.element !== null}
            <span class="part-elem">{ELEMENT_LABELS[part.element]}{part.element_value}</span>
          {/if}
        </span>
        <span class="part-vals num dim">{valuesSummary(part.base)}</span>
        <span class="chev dim">›</span>
      </button>
    {/each}
      </div>
      {#if openPart !== null}
        {@const slot = openPart}
        {@const part = draft.equipment.parts[slot]}
        {@const item = equippedItem(slot)}
        {@const contribution = partContribution(slot)}
        <div class="part-detail pane-in">
        <button type="button" class="close-detail" onclick={() => (openPart = null)}>✕ この部位を閉じる</button>
        <div class="contrib-card" class:empty={contribution === null}>
          <span class="contrib-label">この枠の寄与</span>
          {#if contribution === null}
            <span class="contrib-note dim">「キャラステータス」で主軸スキルを選ぶと出ます</span>
          {:else}
            <span class="contrib-value num">−{fmtInt(contribution)}</span>
            <span class="contrib-note dim">外すと攻撃力がこれだけ減ります(テシスコアの地域分を除く)</span>
          {/if}
        </div>
        <div class="card">
          <div class="card-title">{openPartLabel}: アイテム選択</div>
          <input
            class="item-search"
            type="text"
            placeholder="装備名で探す"
            bind:value={itemQuery}
          />
          <div class="item-list">
            <button type="button" class="item-row" class:on={part.item_id === null && part.custom_name === null} onclick={() => pickUnequipped(slot)}>
              <span class="item-name">未装備</span>
            </button>
            {#each filteredCatalog as candidate (candidate.id)}
              <button type="button" class="item-row" class:on={part.item_id === candidate.id} onclick={() => pickCatalogItem(slot, candidate)}>
                <span class="item-name">{candidate.name}</span>
                <span class="item-vals num dim">
                  {rangeSummary(candidate.values_min, candidate.values_max)}
                </span>
              </button>
            {/each}
            <button type="button" class="item-row" class:on={part.item_id === null && part.custom_name !== null} onclick={() => pickCustom(slot)}>
              <span class="item-name">カスタム(カタログ外)</span>
            </button>
          </div>
          {#if part.item_id === null && part.custom_name !== null}
            <label class="text custom-name">
              <span class="label">表示名 <span class="dim">[仮] 例外操作(カタログ外)</span></span>
              <input type="text" bind:value={part.custom_name} maxlength="40" placeholder="装備名" />
            </label>
          {/if}
        </div>
  
        <div class="card">
          <div class="values-cols">
            <div class="values-col">
              <div class="card-title">基本(装備品ごとに固定)</div>
              <p class="hint dim">
                {#if item}wiki レンジ(MR で個体差あり)。上書きは例外操作
                {:else}カタログ外のため手入力(例外操作)
                {/if}
              </p>
              <div class="fields">
                {#each EQUIPMENT_STAT_KINDS as k (k)}
                  <StatInput
                    label={EQUIPMENT_STAT_LABELS[k]}
                    min={0}
                    max={limits.equipment_value_max}
                    bind:value={part.base[k]}
                    format={item ? () => `wiki ${item.values_min[k]}–${item.values_max[k]}` : undefined}
                  />
                {/each}
              </div>
            </div>
            <div class="values-col">
              <div class="card-title">エンチャント(呪文書で伸ばす)</div>
              <p class="hint dim">上限はアイテム個別(カタログ外は{fmtInt(limits.equipment_value_max)})</p>
              <div class="fields">
                {#each EQUIPMENT_STAT_KINDS as k (k)}
                  {@const cap = item ? item.enchant_caps[k] : limits.equipment_value_max}
                  <StatInput
                    label={EQUIPMENT_STAT_LABELS[k]}
                    min={0}
                    max={cap}
                    bind:value={part.enchant[k]}
                    capGauge
                  />
                {/each}
              </div>
            </div>
          </div>
          <p class="hint dim">
            エンチャントにシエナのオーラとテシスコアの分は含めないでください(それぞれの補正源で入力すると
            強化能力値に自動で合流します。ここにも入れると二重計上になります)。
          </p>
        </div>
  
        {#if ELEMENT_ALLOWED_SLOTS.includes(slot)}
          <div class="card">
            <div class="card-title">属性強化</div>
            <p class="hint dim">1 部位につき 1 属性。無属性は付与できません(wiki: 装備システム/属性強化)</p>
            <div class="fields">
              <Select
                label="属性"
                options={elementOptions}
                bind:value={() => part.element ?? "", (v) => setPartElement(slot, v)}
              />
              {#if part.element !== null}
                <StatInput
                  label="属性値"
                  min={0}
                  max={limits.equipment_element_value_max}
                  bind:value={part.element_value}
                />
              {/if}
            </div>
          </div>
        {/if}
  
        {#if ENHANCE_ALLOWED_SLOTS.includes(slot)}
          <div class="card">
            <div class="card-title">装備強化</div>
            <StepSelect
              label="強化 Lv"
              options={enhanceLevelOptions}
              bind:value={() => String(part.enhance_level), (v) => setEnhanceLevel(slot, Number(v))}
            />
            {#if slot === "armor"}
              <p class="hint dim">鎧の強化は最大 HP のみに効きます(火力計算には反映されません)。</p>
            {:else if part.enhance_level >= 12}
              <label class="check">
                <input
                  type="checkbox"
                  checked={part.enhance_added_damage !== null}
                  onchange={(e) => (part.enhance_added_damage = e.currentTarget.checked ? 0 : null)}
                />
                <span>追加固定ダメージ(ゲーム内表示値)を実測で上書き(未チェックはレンジ上限で自動計算)</span>
              </label>
              {#if part.enhance_added_damage !== null}
                <div class="fields">
                  <StatInput
                    label="追加固定ダメージ"
                    min={0}
                    max={limits.enhance_added_damage_max}
                    bind:value={
                      () => part.enhance_added_damage ?? 0,
                      (v) => (part.enhance_added_damage = v)
                    }
                  />
                </div>
              {/if}
            {:else if part.enhance_level > 0}
              {#if item}
                <p class="hint dim">追加固定ダメージは自動計算されます(ダメージ計算タブのトレースに表示)。</p>
              {:else}
                <p class="hint dim">カタログ外アイテムは追加固定ダメージを自動計算できません(+12 以上にすると実測値を入力できます)。</p>
              {/if}
            {/if}
          </div>
        {/if}
  
        {#if RANDOM_OPTION_ALLOWED_SLOTS.includes(slot)}
          <div class="card">
            <div class="card-title">ランダムオプション</div>
            <p class="hint dim">
              同じカテゴリーの OP は 1 部位に 1 つだけです(wiki: 転移)。効果値は触らなければレンジ上限で計算します。
              収録しているのは火力・命中・回避に関係する OP だけで、グレーの枠は<b>記録するだけ</b>で計算には入りません。
            </p>
            {@render randomOptionEditor(slot)}
          </div>
        {/if}
  
        {#if ABILITY_ALLOWED_SLOTS.includes(slot)}
          <div class="card">
            <div class="card-title">アビリティ</div>
            <p class="hint dim">装備攻撃力に効く 4 系統。同じ系統は段が違っても 1 つだけ付きます(武器のみ)</p>
            <div class="fields">
              {#each ABILITY_FAMILIES as family (family)}
                <Select
                  label={ABILITY_FAMILY_LABELS[family]}
                  options={abilityOptions(family)}
                  bind:value={() => abilityOf(slot, family), (id) => setAbility(slot, family, id)}
                />
              {/each}
            </div>
          </div>
        {/if}
        </div>
      {/if}
    </div>
  {:else if sourceId === "element"}
    <div class="card">
      <p class="hint dim">
        wiki「属性システム」。与ダメージに効くのは<b>攻撃側の属性値 − 敵の属性値</b>の差だけで、
        差 +1 ごとに +0.625%、+80 で上限 +50% です(カテゴリI)。装備の属性強化は「装備」の部位ごとに、
        キャラの基礎属性値は wiki 由来で自動です。
      </p>
      <div class="fields">
        {#each elementSourceDefs as def (def.id)}
          <Select
            label={`${def.name}(+${def.value})`}
            options={elementOptions}
            bind:value={
              () => draft.statSources.elements[def.id] ?? "",
              (v) => (draft.statSources.elements[def.id] = v === "" ? null : (v as Element))
            }
          />
        {/each}
      </div>
    </div>
    {#if elementPreview}
      <div class="card">
        <div class="card-title">属性値の内訳</div>
        <table class="eq-table num inset">
          <thead>
            <tr>
              <th></th>
              {#each ELEMENTS as e (e)}<th class="n">{ELEMENT_LABELS[e]}</th>{/each}
            </tr>
          </thead>
          <tbody>
            <tr>
              <th class="rh">キャラ基礎</th>
              {#each ELEMENTS as e (e)}<td class="n">{fmtInt(elementPreview.base[e])}</td>{/each}
            </tr>
            <tr>
              <th class="rh">装備の属性強化</th>
              {#each ELEMENTS as e (e)}<td class="n">{fmtInt(elementPreview.equipment[e])}</td>{/each}
            </tr>
            <tr>
              <th class="rh">この画面の供給源</th>
              {#each ELEMENTS as e (e)}<td class="n">{fmtInt(elementPreview.sources[e])}</td>{/each}
            </tr>
            <tr class="total">
              <th class="rh">合計(上限 {fmtInt(limits.element_value_max)})</th>
              {#each ELEMENTS as e (e)}<td class="n">{fmtInt(elementPreview.total[e])}</td>{/each}
            </tr>
          </tbody>
        </table>
        <p class="hint dim">
          敵の属性値は 120 / 125(狩り場情報一覧)。合計がそれを +80 上回ると属性差ボーナスが上限(+50%)に届きます。
        </p>
      </div>
    {/if}
  {:else if sourceId === "pet"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StepSelect
            label={STAT_LABELS[k]}
            options={petSkillOptions}
            bind:value={() => petSkillValue(k), (v) => setPetSkillValue(k, v)}
          />
        {/each}
      </div>
    </div>
  {:else if sourceId === "rune"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput label={STAT_LABELS[k]} min={0} max={limits.rune_level_max} bind:value={draft.statSources.rune_levels[k]} />
        {/each}
      </div>
    </div>
  {:else if sourceId === "crown"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput label={STAT_LABELS[k]} min={0} max={limits.crown_max} bind:value={draft.statSources.crown[k]} />
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
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput
            label={STAT_LABELS[k]}
            min={0}
            max={limits.monster_card_max}
            bind:value={draft.statSources.monster_cards[k]}
          />
        {/each}
      </div>
    </div>
  {:else if sourceId === "relic"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput
            label={STAT_LABELS[k]}
            min={0}
            max={limits.sacred_relic_stage_max}
            bind:value={draft.statSources.sacred_relic[k]}
            format={(v) => `${v} 段階 (+${v * 10})`}
          />
        {/each}
      </div>
    </div>
  {:else if sourceId === "siena"}
    {#if openSienaPart === null}
      <div class="card">
        <p class="hint dim">
          wiki「装備システム/シエナのオーラ」。Lv310 の 8 部位(兜/鎧/武器/盾/頭/体/手/足)に発現でき、
          増幅段階の数だけ能力値スロットが解放されます。中身は再抽選のランダム値なので wiki から自動では
          決まりません。部位ごとに実測の合計値を入れてください。
        </p>
        <p class="hint dim">
          武器・盾の能力値はエンチャント扱い(強化能力値)、その他の部位はステの最終固定値、
          追加オプション「攻撃力増加」は与ダメージ割合増加(New1)として計算に入ります。
        </p>
      </div>
      <div class="part-list">
        {#each SIENA_ALLOWED_SLOTS as slot (slot)}
          {@const siena = draft.equipment.parts[slot].siena}
          <button type="button" class="part-row" onclick={() => (openSienaPart = slot)}>
            <span class="part-main">
              <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
              <span class="part-item">{partDisplayName(slot)}</span>
              {#if siena.stage > 0}<span class="part-plus">{siena.stage} 段階</span>{/if}
            </span>
            <span class="part-vals num dim">{sienaSummary(slot)}</span>
            <span class="chev dim">›</span>
          </button>
        {/each}
      </div>
    {:else}
      {@const slot = openSienaPart}
      {@const siena = draft.equipment.parts[slot].siena}
      <button type="button" class="back-link" onclick={() => (openSienaPart = null)}>‹ 部位一覧へ</button>
      <div class="card">
        <div class="card-title">{PART_SLOT_LABELS[slot]}: 増幅段階</div>
        <StepSelect
          label="段階"
          options={sienaStageOptions}
          bind:value={() => String(siena.stage), (v) => setSienaStage(slot, Number(v))}
        />
        <p class="hint dim">
          段階ごとに能力値スロットが 1 個ずつ解放されます(段階 3/7/10 で追加オプションが 1/2/3 個)。
          段階を「未発現」に戻すとこの部位の入力値は消えます。
        </p>
      </div>

      {#if siena.stage > 0}
        {#if sienaIsEquipmentValues(slot)}
          <div class="card">
            <div class="card-title">能力値(装備補正の合計)</div>
            <p class="hint dim">解放済みスロットに出ている装備補正の合計。強化能力値として装備攻撃力に入ります</p>
            <div class="fields">
              {#each EQUIPMENT_STAT_KINDS as k (k)}
                <StatInput
                  label={EQUIPMENT_STAT_LABELS[k]}
                  min={0}
                  max={limits.equipment_value_max}
                  bind:value={siena.values[k]}
                />
              {/each}
            </div>
            <p class="hint dim">
              物理複合攻撃力・魔法斬り攻撃力は wiki の内訳(例: 物理複合 5 = 突き 3 + 斬り 2)に分けて入れてください。
            </p>
          </div>
        {:else}
          <div class="card">
            <div class="card-title">能力値スロットのステータス加算</div>
            <p class="hint dim">解放済みスロットに出ている STAB〜AGI の合計(最終固定値)。全ステータス増加は下の追加オプションへ</p>
            <div class="fields">
              {#each STAT_KINDS as k (k)}
                <StatInput
                  label={STAT_LABELS[k]}
                  min={0}
                  max={limits.siena_stat_bonus_max}
                  bind:value={siena.stats[k]}
                />
              {/each}
            </div>
          </div>
        {/if}

        <div class="card">
          <div class="card-title">追加オプション</div>
          <p class="hint dim">同じ種類のオプションは同じ装備に 1 個までしか付きません(部位は問いません)</p>
          <div class="fields">
            <StatInput
              label="攻撃力増加"
              min={0}
              max={limits.siena_attack_rate_percent_max}
              step={0.5}
              bind:value={siena.attack_rate_percent}
              format={(v) => `+${v}%`}
            />
            <StatInput
              label="全ステータス増加"
              min={0}
              max={limits.siena_all_stats_bonus_max}
              bind:value={siena.all_stats}
              format={(v) => (v > 0 ? `STAB〜AGI に +${v}` : "なし")}
            />
            <StatInput
              label="防御力増加"
              min={0}
              max={limits.siena_defense_rate_percent_max}
              bind:value={siena.defense_rate_percent}
              format={(v) => `+${v}%`}
            />
            <StatInput
              label="中ディレイ減少"
              min={0}
              max={limits.siena_actual_delay_percent_max}
              step={0.5}
              bind:value={siena.actual_delay_percent}
              format={(v) => `−${v}%`}
            />
            <StatInput
              label="クリティカル確率"
              min={0}
              max={limits.siena_critical_rate_percent_max}
              bind:value={siena.critical_rate_percent}
              format={(v) => `+${v}%`}
            />
          </div>
          <p class="hint dim">
            「攻撃力増加」は与ダメージ割合増加(New1)、「全ステータス増加」は 7 ステすべてに同じ値が乗ります。
            「防御力増加」は装備防御力倍率(防御タブ)、「中ディレイ減少」は中ディレイ倍率B(計算タブの 1 秒あたり)、
            「クリティカル確率」はクリティカル率の AGI 由来の項に<b>乗算</b>で合流します。
            防御無視攻撃確率は確率発動(x% で防御力 15% 無視)なので未収録、HP/MP/SP は与ダメージ式に入りません。
          </p>
        </div>
      {/if}
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
    {#each RANDOM_OPTION_ALLOWED_SLOTS as slot (slot)}
      {#if app.randomOptions.some((d) => d.slot === slot)}
        <div class="card">
          <div class="card-title">{PART_SLOT_LABELS[slot]}</div>
          {@render randomOptionEditor(slot)}
        </div>
      {/if}
    {/each}
  {:else if sourceId === "title"}
    <div class="card">
      <p class="hint dim">
        wiki「称号システム」。<b>表示中の 1 件だけ</b>が効きます(持っている称号の合計ではありません)。
        補正は<b>装備の基本能力値</b>に乗るので、装備値の合計と同じ列に足されます。
        収録は<b>主要称号のみ</b>(補正値 9 種の合計 15 以上 = {app.titles.length} 件)。
        備考の条件付き効果(特定マップで追加ダメージ +20% など)とグループボーナスは計算に入りません。
      </p>
      <input class="item-search" type="text" placeholder="称号名・グループで探す" bind:value={titleQuery} />
      <div class="item-list">
        <button
          type="button"
          class="item-row"
          class:on={draft.equipment.title === null}
          onclick={() => (draft.equipment.title = null)}
        >
          <span class="item-name">未装備</span>
        </button>
        {#each filteredTitles as t (t.id)}
          <button
            type="button"
            class="item-row"
            class:on={draft.equipment.title === t.id}
            onclick={() => (draft.equipment.title = t.id)}
          >
            <span class="item-name">{t.name}</span>
            <span class="item-vals num dim">{titleSummary(t)}</span>
          </button>
        {/each}
      </div>
    </div>
    {#if selectedTitle}
      <div class="card">
        <div class="card-title">{selectedTitle.name}</div>
        <div class="values-grid">
          {#each EQUIPMENT_STAT_KINDS as k (k)}
            <span class="val-cell">
              <span class="dim">{EQUIPMENT_STAT_SHORT[k]}</span>
              <span class="num strong">{signed(selectedTitle.values[k])}</span>
            </span>
          {/each}
        </div>
        <p class="hint dim">
          {selectedTitle.group}{selectedTitle.level !== null ? ` ・ 習得 Lv${selectedTitle.level}` : ""}
          {#if selectedTitle.note}<br />{selectedTitle.note}{/if}
        </p>
      </div>
    {/if}
  {:else if sourceId === "commonSkill"}
    <div class="card">
      <p class="hint dim">
        wiki「Skill/共通」。キャラを問わず習得するパッシブです。効き先は
        <b>装備攻撃力強化倍率</b>(パワーウェポン / ストロングウェポン)、
        <b>装備防御力倍率</b>(コートアーマー / プロテクトアーマー)、
        <b>割合追加ダメージ</b>(シャープネスビジョン)の 3 つ。
        <b>オーグメント</b>はストロングウェポン・プロテクトアーマー・ハイパーリミットを Lv2 以上に
        するための前提スキルなので、先に上げないと上の Lv が選べません
        (下げると、それに縛られる Lv も一緒に下がります)。
      </p>
      <div class="fields">
        <StepSelect
          label="オーグメント(前提スキル)"
          options={augmentOptions}
          bind:value={
            () => String(draft.commonSkills.augment_level),
            (v) => setAugmentLevel(Number(v))
          }
        />
        <StepSelect
          label="レインフォース(前提スキル)"
          options={reinforceOptions}
          bind:value={
            () => String(draft.commonSkills.reinforce_level),
            (v) => setReinforceLevel(Number(v))
          }
        />
      </div>
    </div>

    <div class="card">
      <div class="card-title inline">
        アンリーシュ(能力解放)
        <span class="num strong">
          {draft.commonSkills.unleash
            .filter((u) => u.stat !== null && u.level > 0)
            .map((u) => `${STAT_LABELS[u.stat!]} +${UNLEASH_RATES[u.level - 1]}%`)
            .join(" ・ ") || "未使用"}
        </span>
      </div>
      <p class="hint dim">
        選んだステが<b>能力値倍率B</b>で増えます(<b>バフ込みの基本能力値 × 倍率</b>なので、
        バフを盛るほど効きます)。<b>2 ステまで</b>で、同じステは 2 枠に入れられません。
        Lv6 以降は<b>レインフォース</b>の Lv が要ります(下げると Lv も一緒に下がります)。
      </p>
      <div class="fields">
        {#each draft.commonSkills.unleash as slot, i (i)}
          <Select
            label={`枠 ${i + 1} のステ`}
            options={unleashStatOptions(i)}
            bind:value={
              () => slot.stat ?? "",
              (v) => {
                slot.stat = v === "" ? null : (v as StatKind);
                if (slot.stat === null) slot.level = 0;
              }
            }
          />
          <StepSelect
            label={`枠 ${i + 1} の Lv`}
            options={unleashLevelOptions}
            disabled={slot.stat === null}
            bind:value={() => String(slot.level), (v) => (slot.level = Number(v))}
          />
        {/each}
      </div>
    </div>

    <div class="card">
      <div class="card-title inline">装備攻撃力強化 <span class="num strong">+{enhanceRatePercent}%</span></div>
      <p class="hint dim">ほぼ全員が取っているので既定で入れてあります。取っていない・Lv が違うときだけ触ってください。</p>
      <div class="fields">
        <label class="check">
          <input type="checkbox" bind:checked={draft.commonSkills.power_weapon} />
          <span>パワーウェポン(+2%)</span>
        </label>
        <StepSelect
          label="ストロングウェポン"
          options={strongWeaponOptions.filter((o) => Number(o.value) <= augmentGate)}
          bind:value={
            () => String(draft.commonSkills.strong_weapon_level),
            (v) => (draft.commonSkills.strong_weapon_level = Number(v))
          }
        />
      </div>
    </div>

    <div class="card">
      <div class="card-title inline">
        装備防御力倍率
        <span class="num strong">物 {fmtInt(defenseRatePercent.physical)}% / 魔 {fmtInt(defenseRatePercent.magic)}%</span>
      </div>
      <p class="hint dim">
        装備の物防・魔防に掛かります(防御タブの防御力に効きます)。
        コートアーマーとプロテクトアーマーは<b>重複可</b>で、シエナのオーラの「防御力増加」もここに加算されます。
        <b>リンゴの島・ベリネンルミでは常に 100%</b>(wiki 計算式まとめ §防御力)。
      </p>
      <div class="fields">
        <label class="check">
          <input type="checkbox" bind:checked={draft.commonSkills.coat_armor} />
          <span>コートアーマー(物+18% / 魔+12%)</span>
        </label>
        <Select
          label="プロテクトアーマー"
          options={protectArmorOptions}
          bind:value={
            () => String(draft.commonSkills.protect_armor_level),
            (v) => (draft.commonSkills.protect_armor_level = Number(v))
          }
        />
        <Select
          label="改・プロテクトアーマー"
          options={kaiProtectArmorOptions}
          bind:value={
            () => String(draft.commonSkills.kai_protect_armor_level),
            (v) => (draft.commonSkills.kai_protect_armor_level = Number(v))
          }
        />
      </div>
      {#if sienaDefenseRate > 0}
        <p class="hint dim">シエナのオーラの防御力増加 +{sienaDefenseRate}% を含んでいます。</p>
      {/if}
    </div>

    <div class="card">
      <div class="card-title inline">極限スキル(2 枠)</div>
      <p class="hint dim">
        wiki「Skill/極限」。ゲージスキルのうち<b>2 つ</b>がパッシブとして常時適用されます。
        効果値は <b>基本 + スーパーリミット + ハイパーリミット Lv</b> の加算です
        (スーパーリミット = ハイパーアタック、ハイパーリミット = エクストリームアタックの極限形)。
        ハイパーリミットも Lv2 以降はオーグメントが要ります。
      </p>
      <div class="fields">
        {#each [0, 1] as slotIndex (slotIndex)}
          <Select
            label={`枠 ${slotIndex + 1}`}
            options={ultimateOptions(slotIndex)}
            bind:value={
              () => draft.commonSkills.ultimate.slots[slotIndex] ?? "",
              (v) => setUltimate(slotIndex, v)
            }
          />
        {/each}
        <label class="check">
          <input type="checkbox" bind:checked={draft.commonSkills.ultimate.super_limit} />
          <span>スーパーリミット(ハイパーアタックの極限形)</span>
        </label>
        <Select
          label="ハイパーリミット"
          options={hyperLimitOptions}
          bind:value={
            () => String(draft.commonSkills.ultimate.hyper_limit_level),
            (v) => (draft.commonSkills.ultimate.hyper_limit_level = Number(v))
          }
        />
      </div>
      {#each draft.commonSkills.ultimate.slots.filter((u) => u !== null) as picked (picked)}
        <p class="hint dim">{ULTIMATE_SKILL_LABELS[picked]}: {ULTIMATE_SKILL_EFFECTS[picked]}</p>
      {/each}
      {#if ultimateEffects.length > 0}
        <p class="hint dim">いまの効果: {ultimateEffects.join(" ・ ")}</p>
      {/if}
    </div>

    <div class="card">
      <div class="card-title inline">
        割合追加ダメージ <span class="num strong">+{fmtInt(draft.commonSkills.sharpness_vision_level === 0 ? 0 : [5, 10, 15, 20, 25, 28, 31, 34, 37, 40][draft.commonSkills.sharpness_vision_level - 1])}%</span>
      </div>
      <p class="hint dim">
        シャープネスビジョン。<b>合計ダメージ</b>に乗ります(1 発ごとではありません)。
        Lv6 以上は各 Lv の習得スクロールが要ります。
      </p>
      <div class="fields">
        <Select
          label="シャープネスビジョン"
          options={sharpnessVisionOptions}
          bind:value={
            () => String(draft.commonSkills.sharpness_vision_level),
            (v) => (draft.commonSkills.sharpness_vision_level = Number(v))
          }
        />
      </div>
    </div>
  {:else if sourceId === "thesis"}
    <div class="card">
      <div class="card-title">地域</div>
      <div class="region-tabs">
        {#each CORE_REGIONS as region (region)}
          <button
            type="button"
            class="region-tab"
            class:on={coreRegion === region}
            onclick={() => (coreRegion = region)}
          >
            <span>{CORE_REGION_LABELS[region]}</span>
            <span class="num dim">{fmtInt(coreRegionTotal(region))}</span>
          </button>
        {/each}
      </div>
      <p class="hint dim">
        wiki「テシスコア」。コアの能力値増加は対象ダンジョン内でのみ有効なので、計算対象のコンテンツに
        対応する地域のコアだけが装備攻撃力に入ります。セット効果(最終ダメージ)は全地域で発動します。
      </p>
      <p class="hint dim">
        補助タイプ(物防/回避/敏捷/命中)も装着状態として記録できます。与ダメージ式の装備係数が 0 なので
        装備攻撃力には入らず、入場条件「コア N」の合計と防御タブ(防御力・カット率・回避P)に効きます。
        経験値タイプのみのシオカンヘイムコアは火力にもセット効果にも効かないため地域を持ちません。
      </p>
    </div>
    <div class="card">
      <div class="card-title">
        {CORE_REGION_LABELS[coreRegion]} の 6 枠
        <span class="dim normal">補正値 合計 {fmtInt(coreRegionTotal(coreRegion))}</span>
      </div>
      {#if coreSupportSummary}
        <p class="hint dim">
          このうち補助タイプ({coreSupportSummary})は装備攻撃力には入らず、防御タブの防御力・カット率・回避Pに効きます。
        </p>
      {/if}
      <div class="core-list">
        {#each coreSlotIndexes as index (index)}
          {@const core = coreAt(index)}
          <div class="core-row">
            <span class="core-slot dim">{index + 1}</span>
            <Select
              label="タイプ"
              options={coreTypeOptions}
              bind:value={() => core?.core_type ?? "", (v) => setCoreType(index, v)}
            />
            <Select
              label="進化"
              options={coreEvolutionOptions}
              disabled={core === null}
              bind:value={
                () => String(core?.evolution ?? 0),
                (v) => setCoreStage(index, "evolution", Number(v))
              }
            />
            <Select
              label="強化"
              options={coreEnhancementOptions}
              disabled={core === null}
              bind:value={
                () => String(core?.enhancement ?? 0),
                (v) => setCoreStage(index, "enhancement", Number(v))
              }
            />
            <span class="core-bonus num" class:support={core !== null && !CORE_POWER_TYPES.includes(core.core_type)}>
              {core ? `+${fmtInt(coreBonus(core.core_type, core.evolution, core.enhancement))}` : "—"}
            </span>
          </div>
        {/each}
      </div>
      <p class="hint dim">
        入場条件の「コア N」はこの 6 枠の合計と同じ値です(火力の進化1強化4 ×6 = 60、進化4強化4 ×6 = 480。
        補助タイプは進化4強化4 でも 60 なので 6 枠でも 360 止まり)。
        セット効果は強化 4 段階のコアが 3 個以上そろうと発動します(タイプは問いません)。
      </p>
    </div>
  {:else if sourceId === "actualDelay"}
    <div class="card">
      <p class="hint dim">
        wiki「ステータス」の<b>中ディレイ倍率B</b>。中ディレイは
        <b>基本中ディレイ × (1 − 減少値) ×(2 コンボ以上なら 0.5)</b>で、下限 0.3s・減少値の上限 70%。
        ここで選ぶのは<b>このキャラ固有のパッシブ・マスタリー</b>だけです。
        フルスロットル(共通スキル)・カフスのランダムOP・シエナのオーラの「中ディレイ減少」は
        それぞれの補正源で設定した値がそのまま合流します。
        中ディレイと 1 秒あたりの火力は計算タブに出ます。
      </p>
      <div class="buff-list">
        {#if delaySkills.length === 0}
          <p class="empty dim">このキャラには中ディレイ減少のパッシブがありません(wiki の表に記載なし)。</p>
        {/if}
        {#each delaySkills as def (def.id)}
          <label class="check">
            <input
              type="checkbox"
              checked={draft.statSources.actual_delay_skills.skill_ids.includes(def.id)}
              onchange={(e) => toggleDelaySkill(def.id, e.currentTarget.checked)}
            />
            <span>{def.name}</span>
            <span class="fixed-value dim">−{def.percent}%</span>
            {#if def.note}<span class="dim note">{def.note}</span>{/if}
          </label>
        {/each}
      </div>
      {#if delaySkillPercent > 0}
        <p class="hint dim">このキャラのパッシブぶん: <b>−{delaySkillPercent}%</b></p>
      {/if}
    </div>
  {:else if sourceId === "criticalRate"}
    <div class="card">
      <p class="hint dim">
        wiki「計算式まとめ <b>#CriticalChance</b>」。クリティカル率は
        <b>(装備クリティカル補正 + 1) × 2 × (AGI / (AGI + 対象のAGI)) × ペット会心
        ＋ スキルの Cri値 ＋ クリティカル率増加 ＋ 対象のクリティカル被撃率</b>で、下限 0% / 上限 100%。
        装備クリティカル補正・AGI・スキルの Cri値は登録済みのデータから自動で入るので、
        ここで選ぶのは<b>ペット会心と「クリティカル率増加」</b>だけです。
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
          <span class="fixed-value dim">+20%</span>
          <span class="dim note">最大レベル時</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.statSources.critical_rate.architect_lab} />
          <span>設計者の研究室</span>
          <span class="fixed-value dim">+30%</span>
          <span class="dim note">最大レベル時</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft.statSources.critical_rate.deadly_blow} />
          <span>致命打</span>
          <span class="fixed-value dim">+100%</span>
        </label>
      </div>
      <p class="hint dim">
        クリティカル率増加の合計: <b>+{Math.min(limits.critical_rate_bonus_max, criticalRateBonus)}%</b>
        {#if criticalRateBonus > limits.critical_rate_bonus_max}(上限 +{limits.critical_rate_bonus_max}% で頭打ち){/if}
        <br />
        値が不定の「バフ」、シエナのオーラのクリティカル確率、被撃率B(対人)、最終クリティカル率増加は未収録です。
      </p>
    </div>
  {:else if sourceId === "skills"}
    <div class="card">
      <div class="card-title">このキャラのスキル</div>
      <div class="buff-list">
        {#if ownSkillBuffs.length === 0}
          <p class="empty dim">このキャラのスキルデータは未収録です。</p>
        {/if}
        {#each ownSkillBuffs as def (def.id)}
          {@const checked = buffChecked(def.id)}
          <label class="check">
            <input
              type="checkbox"
              {checked}
              onchange={(e) => (draft.statSources.buffs.choices = toggleBuff(draft.statSources.buffs.choices, def, e.currentTarget.checked))}
            />
            <span>{def.name}</span>
            {#if isFixedValue(def.value)}<span class="fixed-value dim">{formatLayerValue(def.layer, def.value.fixed)}</span>{/if}
            {#if def.note}<span class="dim note">{def.note}</span>{/if}
          </label>
        {/each}
      </div>
      <div class="card-title space">味方から受けるスキル</div>
      <div class="buff-list">
        {#if allySkillBuffs.length === 0}
          <p class="empty dim">味方から受けるスキルデータは未収録です。</p>
        {/if}
        {#each allySkillBuffs as def (def.id)}
          {@const checked = buffChecked(def.id)}
          <label class="check">
            <input
              type="checkbox"
              {checked}
              onchange={(e) => (draft.statSources.buffs.choices = toggleBuff(draft.statSources.buffs.choices, def, e.currentTarget.checked))}
            />
            <span>{def.name}</span>
            {#if isFixedValue(def.value)}<span class="fixed-value dim">{formatLayerValue(def.layer, def.value.fixed)}</span>{/if}
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
  .fields { margin-top: 9px; display: flex; flex-direction: column; gap: 9px; }
  .two { display: flex; gap: 10px; }
  .two > :global(*) { flex: 1; }
  .text { display: flex; flex-direction: column; gap: 6px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  input[type="text"] {
    padding: 8px 10px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); }

  .tbl { margin-top: 8px; overflow-x: auto; border: 1px solid var(--border-soft); border-radius: var(--r-panel); background: var(--bg-field); }
  table.grid td.stat-cell { min-width: 180px; }
  .stat-cell :global(.stat-input) { justify-content: flex-end; flex-wrap: nowrap; }
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
  .cap-badge {
    font-size: 9px; letter-spacing: 0.05em; color: #B5443A; border: 1px solid #B5443A;
    border-radius: var(--r-inset); padding: 1px 4px; cursor: default;
  }
  .fixed-value { font-size: 11px; font-weight: 500; }

  /* 装備ドリルダウン: 部位一覧 */
  /* 装備のドリルダウン(§09 規則 2)。掘るたびに右へペインが増え、前の階層は消えない。
     詳細を開いているときだけ一覧を細くし、値サマリを畳む — 狭いときだけ左から畳む、の形 */
  .part-split { display: flex; align-items: flex-start; gap: 10px; min-width: 0; }
  .part-list { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 6px; }
  .part-split.open .part-list { flex: 0 0 232px; }
  .part-split.open .part-vals { display: none; }
  .part-detail { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 9px; }
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
    flex-shrink: 0; padding: 0 6px; border-radius: var(--r-pill); background: var(--state-short-bg); border: 1px solid var(--mob);
    font-size: 9px; font-weight: 700; color: #7A4B45;
  }
  .part-vals { flex-shrink: 0; font-size: 9.5px; }
  .chev { flex-shrink: 0; font-size: 11px; }

  /* 装備ドリルダウン: 部位詳細。一覧が消えないので「戻る」は要らない — 閉じるだけ */
  .close-detail { align-self: flex-start; padding: 2px 2px; font-size: var(--t-label); color: var(--fg-muted); }
  .close-detail:hover { color: var(--accent); text-decoration: underline; }
  .card-title.inline { margin: 0; display: flex; align-items: baseline; gap: 8px; }
  .card-title.inline .strong { font-size: 13px; font-weight: 700; }
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

  /* 称号の補正値グリッド */
  .values-grid { display: flex; flex-wrap: wrap; gap: 4px 12px; margin-bottom: 6px; }
  .val-cell { display: flex; align-items: baseline; gap: 4px; font-size: 11px; }

  /* ランダムオプションの 1 枠 */
  .ro-row {
    margin-top: 8px; padding: 8px 10px;
    border: 1px solid var(--border); border-radius: var(--r-panel); background: var(--surface-inset);
  }
  /* 計算に入らない(記録するだけの)枠は破線 + 塗りなしで見分ける */
  .ro-row.record-only { border-style: dashed; background: var(--bg-rail); }
  .ro-row.record-only .ro-name { color: var(--fg-muted); }
  .ro-head { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; margin-bottom: 6px; }
  .ro-name { font-size: 11px; font-weight: 700; }
  .ro-cat { flex-shrink: 0; font-size: 9px; }
  .ro-effect { font-size: 9px; }
  .ro-remove {
    margin-left: auto; flex-shrink: 0; padding: 1px 8px;
    font-size: 9px; color: var(--fg-muted);
    border: 1px solid var(--border); border-radius: var(--r-pill); background: var(--bg-field);
  }
  .ro-remove:hover { border-color: var(--accent); color: var(--accent); }

  .contrib-card {
    margin-top: 8px; display: flex; align-items: baseline; gap: 9px; flex-wrap: wrap;
    padding: 8px 11px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #EFF5FD); border: 1px solid #9FB4D0;
  }
  .contrib-card.empty { background: var(--bg-rail); border-style: dashed; border-color: var(--border); }
  .contrib-label { font-size: 10px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); }
  .contrib-value { font-size: 17px; font-weight: 700; }
  .contrib-note { font-size: 9px; line-height: 1.5; }
  .item-search {
    margin-top: 8px; width: 100%; box-sizing: border-box; padding: 7px 9px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg); font-size: 11px;
  }
  .item-search:focus { outline: none; border-color: var(--accent); }
  .item-list { margin-top: 7px; max-height: 220px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
  .item-row {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    padding: 7px 9px; border-radius: var(--r-panel); background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .item-row:hover { border-color: var(--accent); }
  .item-row.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); }
  .item-name { min-width: 0; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item-vals { flex-shrink: 0; font-size: 9.5px; }
  .custom-name { margin-top: 9px; }

  .region-tabs { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 8px; }
  .region-tab {
    display: flex; align-items: baseline; gap: 6px; padding: 6px 10px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border); font-size: 11px;
  }
  .region-tab.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); font-weight: 700; }
  .core-list { display: flex; flex-direction: column; gap: 8px; }
  .core-row {
    display: grid; grid-template-columns: 18px minmax(110px, 1.4fr) minmax(84px, 1fr) minmax(84px, 1fr) 48px;
    gap: 8px; align-items: end;
  }
  .core-slot { font-size: 11px; padding-bottom: 8px; text-align: center; }
  .core-bonus { font-size: 12px; font-weight: 700; padding-bottom: 8px; text-align: right; }
  .core-bonus.support { color: var(--fg-muted); font-weight: 400; }
</style>