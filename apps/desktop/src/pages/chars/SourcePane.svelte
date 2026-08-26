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
    CoreRegion, CoreType, Element, ElementPreview, EquipmentAbilityFamily, EquipmentItem, PartSlot,
    PetSkillTier, RandomOptionDef, RandomOptionRank, Skill, StatKind, StatPreview, TitleDef,
    UltimateSkill,
  } from "../../api/types";
  import { isAllySkill, isCharacterSkillFor, isFixedValue, toggleBuff } from "../../buffs";
  import { previewElements } from "../../api/commands";
  import { draftToPayload, type Draft } from "../../draft";
  import {
    clampToCaps, coreBonus, coreSetEffect, coreSetSupportValues, coreSetTotalBonus, midpointValues,
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
  import { bump, flash } from "../../ui/motion.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Picker, { type PickerOption } from "../../ui/Picker.svelte";
  import Select from "../../ui/Select.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import { ETERNAL_MILESTONES } from "../../draft";
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

  const PET_TIERS: PetSkillTier[] = ["basic", "true_lv1", "true_lv2", "true_lv3", "true_lv4"];
  // 段の名前だけだと「それでいくつ増えるのか」を毎回引くことになるので、値を段に書く
  const PET_SKILL_BONUS: Record<PetSkillTier, number> = {
    basic: 20, true_lv1: 30, true_lv2: 40, true_lv3: 50, true_lv4: 60,
  };
  const petSkillOptions = [
    { value: "", label: "なし" },
    ...PET_TIERS.map((t) => ({ value: t, label: `${PET_SKILL_TIER_LABELS[t]} +${PET_SKILL_BONUS[t]}` })),
  ];
  const petSkillValue = (k: StatKind) => draft.statSources.pet_skills[k] ?? "";
  const petSkillBonus = (k: StatKind) => {
    const tier = draft.statSources.pet_skills[k];
    return tier ? PET_SKILL_BONUS[tier] : 0;
  };
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
    // 段階選択に並べるので、プレースホルダは入れない。押せない選択肢を混ぜると
    // 「選ばれているのに何も起きない」項目になる(§00 意味のないものを置かない)
    return [
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
  const PROTECT_ARMOR_RATES = [36, 45, 54, 63, 72, 81];
  const PROTECT_ARMOR_MAGIC = [24, 30, 36, 42, 48, 54];
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
    return lv === 0 ? "未習得" : `物 +${lv * 9}% / 魔 +${lv * 6}%`;
  });
  const SHARPNESS_RATES = [5, 10, 15, 20, 25, 28, 31, 34, 37, 40];
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

  // --- 属性強化(部位ごとに 1 属性) --------------------------------------
  const partElementOptions = [
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
  const coreRegionTotal = (region: CoreRegion) =>
    coreSetTotalBonus(draft.equipment.thesis_cores[region]);
  // 補助タイプは与ダメージ(攻撃力)には効かないが、装備値 9 種として防御側・回避Pに効く
  // 地域ごとのコアセット効果はタブが持つ(ゲーム内 UI の地域カードと同じ)。
  // 全地域の合計は「いまの実力」に出す — 結果を入力エリアに積まない
  const coreSetOf = (region: CoreRegion) => coreSetEffect(draft.equipment.thesis_cores[region]);
  /** その地域のコアセット効果(タブに出す短い形)。進化段階ごとの分は合算済み */
  const coreSetLabelOf = (region: CoreRegion) => {
    const e = coreSetOf(region);
    if (e.groups.length === 0) return "";
    const parts: string[] = [];
    if (e.rate > 0) parts.push(`+${Math.round(e.rate * 100)}%`);
    if (e.fixed > 0) parts.push(`+${fmtInt(e.fixed)}`);
    return parts.join(" ");
  };
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
    status: { title: "キャラステータス", note: "素ステ・覚醒・エタの意志・主属性" },
    equipment: { title: "装備", note: "部位ごとのアイテム・エンチャント・強化" },
    pet: { title: "ペット S スキル", note: "ステごとに 1 段階" },
    rune: { title: "ルーンスキル", note: `スキル Lv がそのままステに乗る(Lv 0–${limits.rune_level_max})` },
    crown: { title: "クラウン", note: `ステに乗る実値(0–${limits.crown_max})` },
    monsterCard: { title: "モンスターカード", note: `装着カードのステータス(0–${limits.monster_card_max})` },
    relic: { title: "神鳥の聖物", note: `ステごとの加算(10 きざみ・0–${limits.sacred_relic_stage_max * 10})` },
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
  <!-- 「1 つ選ぶ」ではなく「追加する」操作なので段階選択にしない。
       §08「追加用のチップは破線 + ＋。実在のチップと見た目で区別する」 -->
  <div class="add-row">
    <span class="label">OP を追加</span>
    {#each addableRandomOptions(slot) as o (o.value)}
      <button type="button" class="chip add" onclick={() => addRandomOption(slot, o.value)}>
        ＋ {o.label}
      </button>
    {/each}
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
      {@const part = draft.equipment.parts[slot]}
      {@const canEnhance = ENHANCE_ALLOWED_SLOTS.includes(slot)}
      <button type="button" class="part-row" class:on={openPart === slot} onclick={() => openPartDetail(slot)}>
        <span class="part-main">
          <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
          <span class="part-item">{partDisplayName(slot)}</span>
          <!-- 強化バッジの枠は常に確保する。出ても行の中身がずれない(§12) -->
          {#if canEnhance}
            <span class="part-plus" class:on={part.enhance_level > 0}
            >{part.enhance_level > 0 ? `+${part.enhance_level}` : ""}</span>
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
                  <!-- 基本値は §12 の形態 1(自動)。装備品を選んだ瞬間に確定する値で、
                       上限まで盛るものではない。上限(1,000)は入力ミスを防ぐための一律値なので
                       進捗も MAX も出さない — 出すと「1,000 まで盛れる」と読めてしまう。
                       MR による個体差で上書きできるので、目安として wiki レンジだけ添える -->
                  <StatInput
                    label={EQUIPMENT_STAT_LABELS[k]}
                    min={0}
                    max={limits.equipment_value_max}
                    gauge={false}
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
              <StepSelect
                label="属性"
                options={partElementOptions}
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
        {#each STAT_KINDS as k (k)}
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
      <div class="stat-rows two">
        {#each STAT_KINDS as k (k)}
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
            <StatInput label="" min={0} max={limits.crown_max} bind:value={draft.statSources.crown[k]} />
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
        {#each STAT_KINDS as k (k)}
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
            <StatInput
              label=""
              min={0}
              max={limits.monster_card_max}
              bind:value={draft.statSources.monster_cards[k]}
            />
          </div>
        {/each}
      </div>
    </div>
  {:else if sourceId === "relic"}
    <div class="card">
      <div class="stat-rows two">
        {#each STAT_KINDS as k (k)}
          <div class="stat-row">
            <span class="k">{STAT_LABELS[k]}</span>
            <!-- 段階ではなく**実際に増える値**で入れる(1 段階 = +10 なので ＋ を押すと 10 ずつ)。
                 多くの人は 200 で止まるので、そこを 1 押しで置く。保存は段階のまま -->
            <StatInput
              label=""
              min={0}
              max={limits.sacred_relic_stage_max * 10}
              step={10}
              stepper
              presets={[{ value: 200, label: "200" }]}
              bind:value={
                () => draft.statSources.sacred_relic[k] * 10,
                (v) => (draft.statSources.sacred_relic[k] = Math.round(v / 10))
              }
            />
          </div>
        {/each}
      </div>
    </div>
  {:else if sourceId === "siena"}
    <!-- ドリルダウンは置き換えではなく、右にペインを足す(§09 規則 2)。
         押した部位行はその場に残り、別の部位を押せばそのまま横に移れる -->
    <div class="part-split" class:open={openSienaPart !== null}>
      <div class="part-side">
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
          <button type="button" class="part-row" class:on={openSienaPart === slot} onclick={() => (openSienaPart = slot)}>
            <span class="part-main">
              <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
              <span class="part-item">{partDisplayName(slot)}</span>
              <span class="part-plus wide" class:on={siena.stage > 0}>{siena.stage > 0 ? `${siena.stage} 段階` : ""}</span>
            </span>
            <span class="part-vals num dim">{sienaSummary(slot)}</span>
            <span class="chev dim">›</span>
          </button>
        {/each}
      </div>
      </div>
      {#if openSienaPart !== null}
        {@const slot = openSienaPart}
        {@const siena = draft.equipment.parts[slot].siena}
        <div class="part-detail pane-in">
      <button type="button" class="close-detail" onclick={() => (openSienaPart = null)}>✕ この部位を閉じる</button>
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
        </div>
      {/if}
    </div>
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
            <span class="v num">{draft.commonSkills.coat_armor ? "物18 / 魔12%" : "—"}</span>
          </div>
          <div class="skill-field">
            <span class="k">プロテクトアーマー</span>
            <StepSelect
              label=""
              options={protectArmorLevels}
              cols={protectArmorLevels.length}
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
            {#if coreSetOf(region).groups.length > 0}
              <span class="tab-set num" use:flash={() => coreSetLabelOf(region)}>{coreSetLabelOf(region)}</span>
            {:else if coreRegionTotal(region) > 0}
              <span class="tab-set off num">あと {3 - coreSetOf(region).ready}</span>
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

  /* 装備ドリルダウン: 部位一覧 */
  /* 装備のドリルダウン(§09 規則 2)。掘るたびに右へペインが増え、前の階層は消えない。
     詳細を開いているときだけ一覧を細くし、値サマリを畳む — 狭いときだけ左から畳む、の形 */
  .part-split { display: flex; align-items: flex-start; gap: 10px; min-width: 0; }
  .part-list { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 6px; }
  /* シエナは一覧の上に説明カードがあるので、まとめて 1 列にする */
  .part-side { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 9px; }
  .part-split.open .part-list, .part-split.open .part-side { flex: 0 0 232px; }
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
  .contrib-value { font-size: 15px; font-weight: 700; }
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
    padding: 3px 4px; border-radius: var(--r-chip);
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
  .element-auto b { font-size: 13px; }

  .stat-rows { margin-top: 8px; display: grid; grid-template-columns: 1fr; gap: 5px; }
  /* 2 列にするのは中身が収まるときだけ。狭いと右の列にはみ出して隣の行に重なる */
  .stat-rows.two { grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 5px 16px; }
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
    padding: 3px 4px; border-radius: var(--r-chip);
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