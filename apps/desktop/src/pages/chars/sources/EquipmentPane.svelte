<script lang="ts">
  import { t } from "../../../i18n";
  // 「equipment」補正源のペイン。部位一覧 ⇄ 部位詳細のドリルダウン(§09 規則 2)。
  import type {
    EnchantPlan, EnchantPlanRow, EquipmentAbilityAdditionalKind, EquipmentAbilityCandidate,
    EquipmentAbilityDef, EquipmentCandidates,
    EquipmentItem, EquipmentPart, PartSlot, Skill, StatPreview, WeaponClass,
  } from "../../../api/types";
  import {
    applyCatalogItem as applyCatalogItemCommand,
    listEnchantPlans, listEquipmentAbilityCandidates, listEquipmentCandidates,
    setAbilityForCategory as setAbilityForCategoryCommand,
    setEnhanceLevel as setEnhanceLevelCommand,
    toggleAbility as toggleAbilityCommand,
  } from "../../../api/commands";
  import { draftToPayload } from "../../../draft";
  import Chip from "../../../ui/Chip.svelte";
  import Disclosure from "../../../ui/Disclosure.svelte";
  import { latest } from "../../../ui/latest.svelte";
  import { damageCategoryLabel } from "../../../characterSkills";
  import { equipmentAttackKindsFor } from "../summaries";
  import type { Draft } from "../../../draft";
  import {
    cloneEquipmentPart, equipmentIconId, neutralEquipmentPart, rangeSummary, valuesSummary, zeroValues,
  } from "../../../equipment";
  import { reportError } from "../../../toast.svelte";
  import { errorMessage } from "../../../api/commands";
  import { fmtInt, fmtRate, fmtSigned } from "../../../format";
  import {
    ABILITY_ALLOWED_SLOTS, ELEMENT_ALLOWED_SLOTS, ENHANCE_ALLOWED_SLOTS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT, OTHER_EQUIPMENT_STATS, PART_SLOTS, PART_SLOT_LABELS, PRIMARY_EQUIPMENT_STATS, RANDOM_OPTION_ALLOWED_SLOTS,
  } from "../../../labels";
  import type { EquipmentStatKind } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { tables } from "../../../tables.svelte";
  import { app, equipmentFocus, equipmentPartFocus } from "../../../state.svelte";
  import { changed, DUR, motionDuration, reveal } from "../../../ui/motion.svelte";
  import Modal from "../../../ui/Modal.svelte";
  import Drill from "../../../ui/Drill.svelte";
  import Icon from "../../../ui/Icon.svelte";
  import Value from "../../../ui/Value.svelte";
  import { dropHalfIndex, moveItem } from "../../../ui/reorder.svelte";
  import Picker, { type PickerOption } from "../../../ui/Picker.svelte";
  import Choose from "../../../ui/Choose.svelte";
  import NumberField from "../../../ui/NumberField.svelte";
  import ToggleRow from "../../../ui/ToggleRow.svelte";
  import TextField from "../../../ui/TextField.svelte";
  import { slide } from "svelte/transition";
  import { tick, untrack } from "svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
  }
  let { draft, preview, skills }: Props = $props();

  /** 通常エンチャントを持つ全部位。成長装備の盾+とレリックは別の入力モデル。 */
  const ENCHANT_PLAN_SLOTS = new Set<PartSlot>([
    "weapon", "armor", "helm", "shield", "head", "body", "hand", "leg", "effect", "artifact",
  ]);

  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);
  /** 部位一覧に出す補正。主軸スキルの依存能力から決まる(見出しの要約と同じ規則)。
      主軸が無いときは undefined = 値が大きい上位 2 種に任せる。 */
  const summaryKinds = $derived(
    mainSkill === null ? undefined : equipmentAttackKindsFor(mainSkill.dependency),
  );
  const iconId = (itemId: string | null) => equipmentIconId(itemId, app.equipmentCatalog);

  /** 双剣Sub(wrist_type)。この盾は武器アビリティも合算候補になる
      (wiki: 装備システム/アビリティ #subarms。domain::WristType::weapon_ability_system と同じ対象)。 */
  const DUAL_BLADE_WRIST_TYPES = new Set(["dual_blade_physical", "dual_blade_magic"]);
  /** 武器と同じ「カテゴリー枠セレクタ」UI を使う部位か(武器そのもの、または双剣Subの盾)。 */
  const usesWeaponAbilityUi = (slot: PartSlot): boolean =>
    slot === "weapon" || DUAL_BLADE_WRIST_TYPES.has(equippedItem(slot)?.wrist_type ?? "");

  // --- 装備ドリルダウン(部位一覧 ⇄ 部位詳細) --------------------------------
  let openPart = $state<PartSlot | null>(null);
  let itemQuery = $state("");
  let showOtherEquipmentStats = $state(false);
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
    next.label = t("装備 {n}", { n: list.registered.length + 1 });
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
    equipmentRegistrationDropAt = { slot, index: dropHalfIndex(rect, event.clientX, index, "x") };
  };
  const dropEquipmentRegistration = (event: DragEvent, slot: PartSlot) => {
    event.preventDefault();
    const dragging = draggedEquipmentRegistration;
    const dropAt = equipmentRegistrationDropAt;
    draggedEquipmentRegistration = null;
    equipmentRegistrationDropAt = null;
    if (dragging?.slot !== slot || dropAt?.slot !== slot) return;
    const list = draft.equipment.parts[slot];
    const from = list.registered.findIndex((part) => part.id === dragging.id);
    if (from === -1) return;
    list.registered = moveItem(list.registered, from, dropAt.index);
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
  // 候補の絞り込み規則(依存能力 → 武器系統、スキル → 実用武器種、キャラの装備可能区分、
  // サブアームの物理 / 魔法)は Rust(domain::EquipmentFitRule)が持つ。ここは返ってきた
  // 適合度で分けて、帯の文言を組むだけにする。
  let equipmentCandidates = $state<EquipmentCandidates>({ items: [], criterion: null });
  const candidatesLatest = latest();
  $effect(() => {
    const slot = openPart;
    const gameCharacterId = draft.gameCharacterId;
    const mainSkillId = draft.mainSkillId === "" ? null : draft.mainSkillId;
    if (slot === null) {
      equipmentCandidates = { items: [], criterion: null };
      return;
    }
    candidatesLatest.run((isCurrent) =>
      listEquipmentCandidates(gameCharacterId, mainSkillId, slot)
        .then((result) => { if (isCurrent()) equipmentCandidates = result; })
        .catch(() => { if (isCurrent()) equipmentCandidates = { items: [], criterion: null }; }),
    );
    return () => candidatesLatest.cancel();
  });
  const selectedGameCharacter = $derived(
    app.gameCharacters.find((character) => character.id === draft.gameCharacterId) ?? null,
  );
  const weaponClassLabel = (weaponClass: WeaponClass): string => {
    const labels: Partial<Record<WeaponClass, string>> = { katana: t("刀"), tachi: t("太刀"), great_sword: t("大剣") };
    return labels[weaponClass] ?? weaponClass;
  };
  /** 何で絞ったかの帯。文言は画面のもの(Rust は絞り込みの構造だけを返す)。 */
  const equipmentFilterLabel = $derived.by(() => {
    const criterion = equipmentCandidates.criterion;
    if (criterion === null) return null;
    const characterName = selectedGameCharacter?.name ?? null;
    const skillName = mainSkill?.name ?? null;
    switch (criterion.kind) {
      case "weapon_classes":
        return skillName === null
          ? null
          : `${skillName} → ${criterion.classes.map(weaponClassLabel).join("・")}`;
      case "weapon_systems":
        if (characterName !== null && skillName !== null) return t("{character}・{skill}向け", { character: characterName, skill: skillName });
        return skillName === null ? null : t("{skill}の依存能力に合う武器", { skill: skillName });
      case "wrist_types":
        if (characterName === null) return null;
        return skillName === null ? t("{character}が装備可能", { character: characterName }) : t("{character}・{skill}向け", { character: characterName, skill: skillName });
      case "character_usable":
        return characterName === null ? null : t("{character}が装備可能", { character: characterName });
      case "dependency":
        return skillName === null ? null : t("{skill}の依存能力に合うAF", { skill: skillName });
    }
  });
  const filteredCatalog = $derived.by(() => {
    const query = itemQuery.trim();
    const items = equipmentCandidates.items;
    const candidates = query === ""
      ? items
      : items.filter((i) => i.name.includes(query) || t(i.name).includes(query));
    const matched = candidates.filter((i) => i.fit === "recommended");
    if (matched.length === 0) return candidates;
    return showAllEquipmentCandidates
      ? [...matched, ...candidates.filter((i) => i.fit !== "recommended")]
      : matched;
  });
  const equippedItem = (slot: PartSlot): EquipmentItem | null => {
    const itemId = selectedPartOrNull(slot)?.item_id;
    return itemId ? (app.equipmentCatalog.find((i) => i.id === itemId) ?? null) : null;
  };
  const isRelicSlot = (slot: PartSlot): boolean => slot === "relic_pendant" || slot === "relic_bracelet";
  const relicKindOptions = [
    { value: "godbird", label: t("神鳥") },
    { value: "lunaria", label: t("ルナリア") },
  ];
  const relicLevelOptions = Array.from({ length: 10 }, (_, i) => ({
    value: String(i + 1),
    label: `+${i + 1}`,
  }));
  // レリックの系列・段はカタログの属性(gamedata: EquipmentItem::relic)。id 文字列は解析しない。
  const relicKindFor = (slot: PartSlot): string => equippedItem(slot)?.relic?.kind ?? "";
  const relicLevelFor = (slot: PartSlot): string => {
    const level = equippedItem(slot)?.relic?.level;
    return level === undefined ? "" : String(level);
  };
  /** 部位ごとの枠数ルール(domain: PartSlot::ability_slots / random_option_slots)。 */
  const partSlotRule = (slot: PartSlot) => tables.part_slot_rules.find((r) => r.slot === slot) ?? null;
  const currentAbilitySlotCount = (slot: PartSlot) =>
    equippedItem(slot)?.ability_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.ability_slots ?? 0));
  /** 攻撃・耐久の装着時効果の要約。効果なしは null。装備補正値と違って部位の数値には出ないので、
      選ぶ前も選んだ後も文字で見せる。`short` は部位行(3 ペインで幅が狭い)用で、
      カテゴリ名を出すと装備名を押し出してしまうので短い効果名だけにする */
  const itemDamageLabel = (item: EquipmentItem | null, short = false): string | null => {
    if (!item) return null;
    const labels: string[] = item.damage_effects
      .map((e) => {
        if (typeof e === "string" || !("damage" in e)) return null;
        const head = short ? t("与ダメ") : damageCategoryLabel(e.damage.category);
        return `${head} ${fmtSigned(e.damage.percent, { max: 2 }, "%")}`;
      })
      .filter((x): x is string => x !== null);
    for (const effect of item.survival_effects) {
      if ("damage_mitigation" in effect) {
        labels.push(t("緩和 {v}", { v: fmtSigned(effect.damage_mitigation.percent, { max: 2 }, "%") }));
      } else if ("defense_rate" in effect) {
        labels.push(t("防御 {v}", { v: fmtSigned(effect.defense_rate.percent, { max: 2 }, "%") }));
      } else if ("defense_fixed" in effect) {
        labels.push(t("防御 {v}", { v: fmtSigned(effect.defense_fixed.value) }));
      }
    }
    return labels.length === 0 ? null : labels.join(" ・ ");
  };
  const partDisplayName = (slot: PartSlot): string => {
    const item = equippedItem(slot);
    if (item) return item.name;
    const custom = selectedPartOrNull(slot)?.custom_name;
    return custom ? t("{name} [仮]", { name: custom }) : t("未装備");
  };

  /** 部位に返ってきた結果を当てる。Rust から返る部位はそのまま保存できる形になっている。 */
  async function applyPart(slot: PartSlot, next: Promise<EquipmentPart | null>) {
    try {
      const part = await next;
      if (part) Object.assign(selectedPart(slot), part);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }
  // カタログ品を当てる規則(基本能力値・エンチャント・枠数の切り詰め)は Rust の
  // EquipmentPart::apply_catalog_item 1 本だけ。レリックの段送りも同じ関数を通る。
  function pickCatalogItem(slot: PartSlot, item: EquipmentItem, keepPickerOpen = false) {
    itemQuery = "";
    itemPickerOpen = keepPickerOpen;
    void applyPart(slot, applyCatalogItemCommand(cloneEquipmentPart(selectedPart(slot)), item.id));
  }
  function pickRelic(slot: PartSlot, kind: string, level: string) {
    const item = app.equipmentCatalog.find((candidate) =>
      candidate.slot === slot && candidate.relic?.kind === kind && candidate.relic.level === Number(level));
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

  // 上限までのエンチャント案(巻物の組み合わせ・案内する補正の選び方)は Rust
  // (domain::enchant_plan / enchant_plan_stats)が持つ。ここは返ってきた行を出すだけ。
  let enchantPlans = $state<EnchantPlanRow[]>([]);
  const enchantPlansLatest = latest({ debounce: 120 });
  $effect(() => {
    const character = draftToPayload(draft);
    enchantPlansLatest.run((isCurrent) =>
      listEnchantPlans(character)
        .then((rows) => { if (isCurrent()) enchantPlans = rows; })
        .catch(() => { if (isCurrent()) enchantPlans = []; }),
    );
    return () => enchantPlansLatest.cancel();
  });
  const enchantPlanStatsFor = (slot: PartSlot): EquipmentStatKind[] =>
    enchantPlans.filter((row) => row.slot === slot).map((row) => row.stat);
  const enchantPlanFor = (slot: PartSlot, stat: EquipmentStatKind): EnchantPlan | null =>
    enchantPlans.find((row) => row.slot === slot && row.stat === stat)?.plan ?? null;
  function enchantCompletionLabel(plan: EnchantPlan): string {
    const parts: string[] = [];
    if (plan.twenty_count > 0) parts.push(`20 × ${plan.twenty_count}`);
    if (plan.seventeen_count > 0) parts.push(`17 × ${plan.seventeen_count}`);
    if (plan.remainder > 0) parts.push(t("端数 {n}", { n: plan.remainder }));
    return parts.length === 0 ? "" : `+${parts.join(" + ")}`;
  }

  /** その部位の攻撃力(A)への寄与(外すと減る量)。主軸スキル未選択なら null */
  const partContribution = (slot: PartSlot): number | null =>
    preview?.attack?.part_contributions.find((c) => c.slot === slot)?.value ?? null;

  // 武器アビリティは3スロット。同じカテゴリーは1つまでだが、同じ攻撃系統でも
  // カテゴリー1「下級斬り」とカテゴリー4「夜星の鋭い刃」は併用できる。
  const abilityDef = (id: string) => app.equipmentAbilities.find((a) => a.id === id) ?? null;
  /**
   * 装着アビリティを差し替え、そこで外れたアビリティの本体値・追加値も一緒に落とす。
   * 親が選ばれていない値(古いデータの残骸)は Rust が保存時に落とすので、画面は気にしない
   * (`Equipment::prune_ability_extras`。計算にも入らない)。
   */
  function replaceAbilities(part: EquipmentPart, next: string[]) {
    const removed = part.abilities.filter((id) => !next.includes(id));
    part.abilities = next;
    if (removed.length === 0) return;
    part.ability_values = (part.ability_values ?? []).filter((v) => !removed.includes(v.ability_id));
    part.ability_additions = (part.ability_additions ?? []).filter((a) => !removed.includes(a.ability_id));
  }
  // 武器の系統の解決・系統ごとの装着可否・候補の並び・等級での畳み方は、すべて Rust
  // (domain::ability_candidates)が持つ。ここは返ってきた順に並べ、`default_shown` で
  // 既定表示と「ほかの等級」に分けるだけ。
  interface AbilityGroup { shown: EquipmentAbilityCandidate[]; folded: EquipmentAbilityCandidate[] }
  const EMPTY_ABILITY_GROUP: AbilityGroup = { shown: [], folded: [] };
  /** 武器の 3 枠(ゲーム内と同じ順)。 */
  const WEAPON_ABILITY_ROWS = [
    { category: 1, label: t("基本補正"), note: t("従来アビリティ") },
    { category: 4, label: t("新装着"), note: t("追加効果2枠") },
    { category: 3, label: t("武器ディレイ"), note: t("任意・計算対象外") },
  ];
  /** カテゴリー枠セレクタを使う部位の実際の枠(その部位の枠数ぶんだけ先頭から使う)。
      武器は3枠で全部出る。双剣Subの盾は2枠なので、カテゴリー3(武器ディレイ・計算対象外)は出ない。 */
  const abilityRowsFor = (slot: PartSlot) => WEAPON_ABILITY_ROWS.slice(0, currentAbilitySlotCount(slot));
  let abilityGroups = $state<Record<string, AbilityGroup>>({});
  const abilityCandidatesLatest = latest();
  $effect(() => {
    const slot = openPart;
    const part = slot === null ? null : selectedPartOrNull(slot);
    if (slot === null || part === null || !ABILITY_ALLOWED_SLOTS.includes(slot)) {
      abilityGroups = {};
      return;
    }
    // 候補を決めるのはこの 3 つだけ(武器系統 = item_id / enhance_type、選択中 = abilities)。
    // ここだけを読んで、エンチャント等の編集で問い合わせ直さない
    const payload: EquipmentPart = {
      ...neutralEquipmentPart(),
      item_id: part.item_id,
      enhance_type: part.enhance_type,
      abilities: [...part.abilities],
    };
    const keys: { key: string; category: number | null }[] = usesWeaponAbilityUi(slot)
      ? abilityRowsFor(slot).map((row) => ({ key: `weapon-${row.category}`, category: row.category }))
      : [{ key: slot, category: null }];
    abilityCandidatesLatest.run(async (isCurrent) => {
      try {
        const rows = await Promise.all(
          keys.map((k) => listEquipmentAbilityCandidates(payload, slot, k.category)),
        );
        if (!isCurrent()) return;
        abilityGroups = Object.fromEntries(keys.map((k, index) => [k.key, {
          shown: rows[index].filter((a) => a.default_shown),
          folded: rows[index].filter((a) => !a.default_shown),
        }]));
      } catch {
        if (isCurrent()) abilityGroups = {};
      }
    });
    return () => abilityCandidatesLatest.cancel();
  });
  const abilityGroup = (key: string): AbilityGroup => abilityGroups[key] ?? EMPTY_ABILITY_GROUP;
  /** 武器アビリティ 1 カテゴリの候補列。shown(上位等級)を固定チップ、folded(下位等級)を候補面に */
  const weaponAbilityOptions = (grades: AbilityGroup): PickerOption[] => [
    { value: "", name: t("装着しない"), meta: t("空き枠"), pinned: true },
    ...[...grades.shown, ...grades.folded].map((ability, i) => ({
      value: ability.id, name: t(ability.name), meta: t(ability.effect_summary),
      iconId: ability.id, iconKind: "equipment" as const,
      pinned: i < grades.shown.length,
      tone: ability.record_only ? "record-only" : undefined,
    })),
  ];
  const abilityIdForCategory = (slot: PartSlot, category: number): string =>
    selectedPart(slot).abilities.find((id) => abilityDef(id)?.category === category) ?? "";
  function setAbilityForCategory(slot: PartSlot, category: number, id: string) {
    if (abilityIdForCategory(slot, category) === id) return;
    void applyPart(slot, setAbilityForCategoryCommand(
      cloneEquipmentPart(selectedPart(slot)), slot, category, id === "" ? null : id,
    ));
  }
  /** この部位に収録されているアビリティ(「記録のみ」の注記を出すかの判定にだけ使う)。 */
  const slotAbilities = (slot: PartSlot): EquipmentAbilityDef[] =>
    app.equipmentAbilities.filter((ability) => ability.slot === slot);

  /** 下位等級を開いている候補リスト。部位(武器はカテゴリ行)ごとに覚える。 */
  /** 付け外し(同系統の置換・枠が埋まっているときの入れ替え・本体値の既定)は Rust の
      EquipmentPart::toggle_ability が持つ。ここは結果を当てるだけ。 */
  function toggleNonWeaponAbility(slot: PartSlot, ability: EquipmentAbilityDef) {
    void applyPart(slot, toggleAbilityCommand(
      cloneEquipmentPart(selectedPart(slot)), slot, ability.id,
    ));
  }
  const additionalKindLabel = (kind: EquipmentAbilityAdditionalKind): string => ({
    fixed_damage: t("ダメージ増加"), damage_rate: t("ダメージ増加率"),
    thrust: t("突き攻撃力"), slash: t("斬り攻撃力"), magic_attack: t("魔法攻撃力"),
    magic_defense: t("魔法防御力"), hp_recovery: t("HP自然回復力"),
    mp_recovery: t("MP自然回復力"), accuracy: t("命中率補正"),
    physical_defense: t("物理防御力"), critical: t("クリティカル補正"), evasion: t("回避率補正"),
    damage_resistance: t("ダメージ耐性"), physical_damage_reduction: t("物理被害減少"),
    magic_damage_reduction: t("魔法被害減少"), sp_recovery: t("SP自然回復力"), evasion_rate: t("回避率"),
    fire_element: t("火属性"), water_element: t("水属性"), wind_element: t("風属性"), earth_element: t("土属性"),
    lightning_element: t("雷属性"), white_element: t("白属性"), dark_element: t("黒属性"),
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
    // 減少側は負の値として渡し、符号は書式関数に任せる(「−」を直書きしない)
    const signed = (v: number) => fmtSigned(option.kind === "physical_damage_reduction" || option.kind === "magic_damage_reduction" ? -v : v);
    return option.min === option.max ? signed(option.max) : `${signed(option.min)}〜${fmtInt(option.max)}`;
  };
  const additionalAt = (slot: PartSlot, abilityId: string, index: number) => additionsFor(slot, abilityId)[index] ?? null;
  function setAdditionalValue(slot: PartSlot, abilityId: string, index: number, value: number) {
    const current = additionalAt(slot, abilityId, index);
    if (current) current.value = value;
  }
  /** まだ付いていない追加候補。どの部位でどの種類が出るかは Rust(カタログ)が絞って返す。 */
  const addableAdditionalOptions = (slot: PartSlot, abilityId: string) => {
    const used = new Set(additionsFor(slot, abilityId).map((addition) => addition.kind));
    return (abilityDef(abilityId)?.additional_options ?? []).filter(
      (option) => !used.has(option.kind),
    );
  };
  function addAdditional(slot: PartSlot, abilityId: string, kind: EquipmentAbilityAdditionalKind) {
    const part = selectedPart(slot);
    const current = additionsFor(slot, abilityId);
    const max = abilityDef(abilityId)?.additional_slots ?? 0;
    if (current.length >= max || current.some((addition) => addition.kind === kind)) return;
    const option = abilityDef(abilityId)?.additional_options.find((candidate) => candidate.kind === kind);
    if (!option) return;
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
      [t("突き"), "thrust"], [t("斬り"), "slash"], [t("物防"), "physical_defense"],
      [t("魔攻"), "magic_attack"], [t("魔防"), "magic_defense"], [t("命中"), "accuracy"],
      ["Cri", "critical"], [t("回避"), "evasion"], [t("敏捷"), "agility"],
    ] as const;
    const pieces = stats.flatMap(([label, kind]) => {
      const value = partAbilityValues(slot)[kind];
      return value === 0 ? [] : [`${label} ${fmtSigned(value)}`];
    });
    const additions = part.ability_additions ?? [];
    const fixed = additions.filter((a) => a.kind === "fixed_damage").reduce((sum, a) => sum + a.value, 0);
    const rate = additions.filter((a) => a.kind === "damage_rate").reduce((sum, a) => sum + a.value, 0);
    if (fixed !== 0) pieces.push(t("固定 {v}", { v: fmtSigned(fixed) }));
    if (rate !== 0) pieces.push(t("ダメージ {v}", { v: fmtSigned(rate, { max: 2 }, "%") }));
    if (pieces.length === 0) {
      const modeled = part.abilities.map(abilityDef).filter((def) => def && !def.record_only).map((def) => def!.effect_summary);
      return modeled.length > 0 ? modeled.join(" / ") : (part.abilities.length > 0 ? t("記録のみ") : t("未装備"));
    }
    return pieces.join(" / ");
  };
  /** 基本能力値のうち、この部位の装備アビリティ由来の分(表示用の内訳)。計算は Rust 側(preview) */
  const partAbilityValues = (slot: PartSlot) =>
    preview?.part_ability_values.find((p) => p.slot === slot)?.values ?? zeroValues();
  /** この部位の補正値の合計(装備本体 + エンチャント + 装備アビリティ)。計算は Rust 側(preview) */
  const partTotalValues = (slot: PartSlot) =>
    preview?.part_total_values.find((p) => p.slot === slot)?.values ?? zeroValues();
  /** 強化能力値のうち、この部位のエンチャント分(part.enchant そのもの)。計算は Rust 側(preview) */
  const partEnchantValues = (slot: PartSlot) =>
    preview?.part_enchant_values.find((p) => p.slot === slot)?.values ?? zeroValues();
  /** 装備強化の追加効果(武器 = 追加固定ダメージ / 鎧 = 追加HP)。計算は Rust 側(preview) */
  const partEnhance = (slot: PartSlot) =>
    preview?.part_enhance.find((p) => p.slot === slot) ?? null;
  /** 部位詳細を開いたときに、旧データの同カテゴリー重複を1つへ畳む。 */
  function openPartDetail(slot: PartSlot) {
    const part = selectedPartOrNull(slot);
    showAllEquipmentCandidates = false;
    if (!part) { openPart = slot; itemQuery = ""; itemPickerOpen = false; return; }
    const seen = new Set<string>();
    // 双剣Subの盾は武器defも正当(usesWeaponAbilityUi)。それ以外はこの部位の def だけを残す。
    const weaponUi = usesWeaponAbilityUi(slot);
    const normalized = part.abilities.filter((id) => {
      const defSlot = abilityDef(id)?.slot;
      if (defSlot !== slot && !(weaponUi && defSlot === "weapon")) return false;
      const category = abilityDef(id)?.category;
      if (category === undefined || (weaponUi && seen.has(String(category)))) return false;
      seen.add(String(category));
      return true;
    }).slice(0, currentAbilitySlotCount(slot));
    replaceAbilities(part, normalized);
    openPart = slot;
    itemPickerOpen = false;
    showOtherEquipmentStats = false;
  }
  // --- エラー帯からの「ここを開く」 -------------------------------------
  // 帯が指した部位を開き、該当アビリティ行を光らせて見える位置まで送る(§00 ④)。
  let detailEl = $state<HTMLElement | null>(null);
  let focusedAbilityId = $state<string | null>(null);
  let focusSeq = $state(0);
  /** 光らせる対象だけ値が変わるトークン。`use:changed` はこれの変化で動く */
  const focusToken = (abilityId: string) => (focusedAbilityId === abilityId ? String(focusSeq) : "");
  async function revealFocused(abilityId: string | null) {
    await tick();
    if (abilityId === null) return;
    const row = detailEl?.querySelector(`[data-ability-id="${CSS.escape(abilityId)}"]`);
    reveal(row, "center");
  }
  $effect(() => {
    const request = equipmentFocus.request;
    if (!request || request.randomOptionId !== null) return;
    untrack(() => {
      const list = draft.equipment.parts[request.slot];
      if (list.registered.some((p) => p.id === request.partId)) list.selected_id = request.partId;
      openPartDetail(request.slot);
      focusedAbilityId = request.abilityId;
      focusSeq = request.seq;
      equipmentFocus.request = null;
      void revealFocused(request.abilityId);
    });
  });

  // --- ホームの部位タイルから「この部位を開く」だけの要求(光らせる行は無い) ---
  $effect(() => {
    const request = equipmentPartFocus.request;
    if (!request) return;
    untrack(() => {
      openPartDetail(request.slot);
      equipmentPartFocus.request = null;
    });
  });

  const randomOptionSlots = (slot: PartSlot) =>
    equippedItem(slot)?.random_option_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.random_option_slots ?? 0));

  // 装備種別は順序が無いので Picker(§07「1 つ選ぶ」)。候補行の値は選ぶのに要るもの —
  // 武器はその系統の該当武器、鎧は強化補正の式(wiki「装備システム/装備強化」系統表、2026-09-15 取得。
  // 系統の分類の正は crates/domain/src/equipment_class.rs の WeaponClass::system)
  const weaponEnhanceTypeOptions: PickerOption[] = [
    { value: "", name: t("未選択"), meta: t("追加固定ダメージを出さない") },
    { value: "weapon_stab", name: t("突き系"), meta: t("細剣・短剣・槍・スモールソード・物理銃・クロー・ハンドランチャー") },
    { value: "weapon_stab_hack", name: t("物理複合系"), meta: t("長剣・太刀・戦杖・短刀・棒・連接棍") },
    { value: "weapon_hack", name: t("斬り系"), meta: t("刀・斧・鞭・カーラ・物理双剣・サイズ・アーミングソード") },
    { value: "weapon_int", name: t("魔法系"), meta: t("魔杖・ワンド・魔法銃・セプター・トーテム") },
    { value: "weapon_int_hack", name: t("魔剣系"), meta: t("大剣") },
    { value: "weapon_mr", name: t("魔法防御系"), meta: t("聖杖・ハンドベル・魔法双剣・ハンマー") },
  ];
  const armorEnhanceTypeOptions: PickerOption[] = [
    { value: "", name: t("未選択"), meta: t("追加HPを出さない") },
    { value: "armor_light", name: t("軽鎧"), meta: t("物防 ×3.90 + 魔防 ×4.00") },
    { value: "armor_heavy", name: t("重鎧"), meta: t("物防 ×3.10 + 魔防 ×3.80") },
    { value: "armor_magic", name: t("マジックアーマー"), meta: t("物防 ×3.80 + 魔防 ×4.00") },
    { value: "armor_suit", name: t("スーツ"), meta: t("物防 ×7.80") },
    { value: "armor_robe", name: t("ローブ"), meta: t("物防 ×4.00 + 魔防 ×3.80") },
  ];
  const enhanceLevelOptions = $derived(
    tables.enhance_level_candidates.map((lv) => ({
      value: String(lv), label: lv === 0 ? t("強化なし") : `+${lv}`,
    })),
  );
  const enhanceGradeOptions = [
    { value: "lowest", label: t("最下") }, { value: "low", label: t("下") },
    { value: "middle", label: t("中") }, { value: "high", label: t("上") },
    { value: "highest", label: t("最上") },
  ];
  // 強化 Lv と等級の不変条件(+12 以上は等級必須)は Rust の EquipmentPart::set_enhance_level。
  function setEnhanceLevel(slot: PartSlot, level: number) {
    void applyPart(slot, setEnhanceLevelCommand(cloneEquipmentPart(selectedPart(slot)), level));
  }
</script>



{#snippet nonWeaponAbilityChip(slot: PartSlot, ability: EquipmentAbilityDef, selectedIds: string[], full: boolean, fresh: boolean)}
  {@const selected = selectedIds.includes(ability.id)}
  <!-- 複数選択の防具アビリティは行チップ(§07)。キャラに保存されるので tone="saved" -->
  <div class:swap-in={fresh}>
    <ToggleRow
      name={t(ability.name)}
      value={t(ability.effect_summary)}
      cond={ability.record_only ? t("記録のみ") : undefined}
      on={selected}
      tone="saved"
      disabled={!selected && full}
      onToggle={() => toggleNonWeaponAbility(slot, ability)}
    >
      {#snippet icon()}<Icon kind="equipment" id={ability.id} size={20} label={t(ability.name)} />{/snippet}
    </ToggleRow>
  </div>
{/snippet}

{#snippet partSwitchList(slot: PartSlot, registeredList: EquipmentPart[], selectedId: number | null)}
  {#each registeredList as registered, index (registered.id)}
    <button
      type="button"
      class:on={registered.id === selectedId}
      class:dragging={draggedEquipmentRegistration?.slot === slot && draggedEquipmentRegistration.id === registered.id}
      class:drop-before={equipmentRegistrationDropAt?.slot === slot && equipmentRegistrationDropAt.index === index}
      class:drop-after={equipmentRegistrationDropAt?.slot === slot && equipmentRegistrationDropAt.index === index + 1 && index === registeredList.length - 1}
      draggable="true"
      onclick={() => selectEquipmentRegistration(slot, registered.id)}
      ondragstart={(event) => startEquipmentRegistrationDrag(event, slot, registered.id)}
      ondragover={(event) => dragEquipmentRegistrationOver(event, slot, index)}
      ondrop={(event) => dropEquipmentRegistration(event, slot)}
      ondragend={() => { draggedEquipmentRegistration = null; equipmentRegistrationDropAt = null; }}
    >
      <span class="registration-grip" aria-hidden="true">⠿</span>
      <Icon kind="equipment" id={iconId(registered.item_id)} size={20} label={registered.label || t("装備 {n}", { n: registered.id })} />
      {registered.label || t(app.equipmentCatalog.find((i) => i.id === registered.item_id)?.name ?? "") || t("装備 {n}", { n: registered.id })}
    </button>
  {/each}
{/snippet}

<!-- ドリルダウンは置き換えではなく、右にペインを足す(§09 規則 2)。押した部位行は
     その場に残り、別の部位を押せばそのまま横に移れる — 戻るのに「戻る」が要らない -->
<div class="part-split" class:open={openPart !== null}>
  <div class="part-list">
{#each PART_SLOTS as slot (slot)}
  {@const part = selectedPartOrNull(slot)}
  {@const list = draft.equipment.parts[slot]}
  {@const canEnhance = ENHANCE_ALLOWED_SLOTS.includes(slot)}
  {@const damageLabel = itemDamageLabel(equippedItem(slot), true)}
  <Drill open={openPart === slot} onOpen={() => openPartDetail(slot)}>
  {#snippet line()}
    <Icon kind="equipment" id={iconId(part?.item_id ?? null)} size={28} label={partDisplayName(slot)} />
    <span class="part-main">
      <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
      <span class="part-item">{partDisplayName(slot)}</span>
      <Value class="part-abi" motion={() => list.registered.length} value={t("登録 {n}", { n: list.registered.length })} />
      <!-- 強化バッジの枠は常に確保する。出ても行の中身がずれない(§12) -->
      {#if canEnhance}
        <span class="part-plus" class:on={(part?.enhance_level ?? 0) > 0}
        >{(part?.enhance_level ?? 0) > 0 ? `+${part!.enhance_level}` : ""}</span>
      {/if}
      {#if (part?.abilities.length ?? 0) > 0}
        <span class="part-abi">{t("アビリティ {n}", { n: part!.abilities.length })}</span>
      {/if}
      <!-- 装着時効果は装備補正値の列に出ないので、行にバッジで残す(§00 ②/⑤) -->
      {#if damageLabel !== null}
        <Value class="part-dmg" value={damageLabel} />
      {/if}
      {#if (part?.random_options.length ?? 0) > 0}
        <span class="part-abi">OP {part!.random_options.length}</span>
      {/if}
    </span>
    <!-- 主軸スキルが実際に使う補正だけを出す(部位をまたいで同じ並びなので縦に目が動かない。
         §00 ①)。主軸未選択のときだけ「値が大きい上位 2 種」に落ちる -->
    <span class="part-vals num dim"
    >{part ? valuesSummary(partTotalValues(slot), partEnchantValues(slot), summaryKinds) : "—"}</span>
  {/snippet}
  </Drill>
  {#if list.registered.length > 1}
    <div class="part-switches">
      {@render partSwitchList(slot, list.registered, list.selected_id)}
    </div>
  {/if}
{/each}
  </div>
  {#if openPart !== null}
    {@const slot = openPart}
    {@const part = selectedPartOrNull(slot)}
    {@const item = equippedItem(slot)}
    {@const contribution = partContribution(slot)}
    <Modal label={t("{part}の装備登録", { part: openPartLabel })} class="part-detail" onClose={() => (openPart = null)}>
    <div class="part-detail-body" bind:this={detailEl}>
    {#if draft.equipment.parts[slot].registered.length > 1}
      <div class="part-switches registration-order inset" aria-label={t("装備登録の並び順")}>
        {@render partSwitchList(slot, draft.equipment.parts[slot].registered, draft.equipment.parts[slot].selected_id)}
      </div>
    {/if}
    {#if part === null}
      <div class="card empty"><p class="hint dim">{t("この部位にはまだ装備が登録されていません。")}</p>
        <button type="button" class="btn primary" onclick={() => createEquipmentRegistration(slot)}>{t("＋ 新しい装備を登録")}</button>
      </div>
    {:else}
    <div class="part-actions">
      <span class="editing-registration badge">{t("編集中: {name}", { name: part.label || t("装備 {n}", { n: part.id }) })}</span>
      <button type="button" class="btn" onclick={() => createEquipmentRegistration(slot)}>{t("＋ 新しい装備を登録")}</button>
      <button
        type="button"
        class="btn delete-registration"
        class:confirm={confirmEquipmentDeleteId === part.id}
        onclick={() => removeSelectedEquipmentRegistration(slot)}
      >{confirmEquipmentDeleteId === part.id ? t("もう一度押すと削除します") : t("この登録を削除")}</button>
    </div>
    <div class="card registration-name-card">
      <label class="text custom-name">
        <span class="label">{t("登録名")} <span class="dim">{t("同じ装備を複数持つときの見分け方")}</span></span>
        <TextField label={t("登録名")} bind:value={part.label} max={40} />
      </label>
    </div>

    <div class="card equipment-choice-card">
      <div class="selected-equipment">
        <Icon kind="equipment" id={iconId(part.item_id)} size={28} label={partDisplayName(slot)} />
        <!-- 値ではなく「選択中の装備」という面。装備名を数値書体にしないので <Value> に入れない -->
        <span class="selected-equipment-copy" use:changed={() => partDisplayName(slot)}>
          <small class="dim">{t("選択中の装備")}</small>
          <b>{partDisplayName(slot)}</b>
        </span>
        {#if contribution !== null}<Value class="contrib-inline" value={String(contribution)}>{#snippet children()}{t("寄与 {v}", { v: fmtInt(contribution) })}{/snippet}</Value>{/if}
        <button type="button" class="btn" onclick={() => (itemPickerOpen = !itemPickerOpen)}>
          {itemPickerOpen ? t("候補を閉じる") : t("装備を変更")}
        </button>
      </div>
      {#if itemPickerOpen}
        <div class="equipment-picker pane-in">
          {#if isRelicSlot(slot)}
            <div class="relic-selector">
              <div class="field">
                <span class="field-label">{t("種別")}</span>
                <Choose
                  label={t("レリックの種別")}
                  options={relicKindOptions}
                  full
                  bind:value={() => relicKindFor(slot), (value) => pickRelicKind(slot, value)}
                />
              </div>
              <div class="field">
                <span class="field-label">{t("強化段階")}</span>
                <Choose
                  label={t("レリックの強化段階")}
                  options={relicLevelOptions}
                  cols={5}
                  disabled={relicKindFor(slot) === ""}
                  bind:value={() => relicLevelFor(slot), (value) => pickRelicLevel(slot, value)}
                />
              </div>
              <div class="relic-picker-actions">
                <Chip class="quiet" onclick={() => pickUnequipped(slot)}>{t("未装備")}</Chip>
                <Chip class="quiet" onclick={() => pickCustom(slot)}>{t("カタログ外")}</Chip>
              </div>
            </div>
          {:else}
          <div class="picker-tools">
            <TextField label={t("装備名で探す")} count={filteredCatalog.length} bind:value={itemQuery} />
            {#if equipmentFilterLabel !== null}
              <span class="equipment-filter badge">{equipmentFilterLabel}</span>
              <Chip class="quiet" onclick={() => (showAllEquipmentCandidates = !showAllEquipmentCandidates)}>
                {showAllEquipmentCandidates ? t("候補だけ見る") : t("すべて見る")}
              </Chip>
            {/if}
          </div>
          <div class="item-list" class:effectful={slot === "artifact"}>
            <button type="button" class="item-row" class:on={part.item_id === null && part.custom_name === null} onclick={() => pickUnequipped(slot)}>
              <Icon kind="equipment" id={null} size={28} label={t("未装備")} />
              <span class="item-copy"><span class="item-name">{t("未装備")}</span></span>
            </button>
            {#each filteredCatalog as candidate (candidate.id)}
              <!-- 候補カードは名前の識別が主目的。カテゴリ名は詳細へ譲り、短い効果量だけを置く -->
              {@const candidateDamage = itemDamageLabel(candidate, true)}
              <button type="button" class="item-row" class:on={part.item_id === candidate.id} onclick={() => pickCatalogItem(slot, candidate)}>
                <Icon kind="equipment" id={iconId(candidate.id)} size={28} label={t(candidate.name)} />
                <span class="item-copy">
                  <span class="item-name">{t(candidate.name)}</span>
                  <span class="item-vals num dim">{rangeSummary(candidate.values_min, candidate.values_max)}</span>
                </span>
                {#if candidateDamage !== null}<span class="part-dmg">{candidateDamage}</span>{/if}
              </button>
            {/each}
            <button type="button" class="item-row" class:on={part.item_id === null && part.custom_name !== null} onclick={() => pickCustom(slot)}>
              <Icon kind="equipment" id={null} size={28} label={t("カスタム")} />
              <span class="item-copy"><span class="item-name">{t("カスタム")}</span><span class="item-vals dim">{t("カタログ外")}</span></span>
            </button>
          </div>
          {/if}
          {#if part.item_id === null && part.custom_name !== null}
            <label class="text custom-name">
              <span class="label">{t("装備名")} <span class="dim">{t("[仮] カタログ外")}</span></span>
              <TextField label={t("装備名")} bind:value={part.custom_name} max={40} />
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
        <span>{t("装備補正")}</span>
        <span class="badge num">{t("成長値")}</span>
      </div>
      <p class="hint dim">{t("この段階の実値を入力します。下限は直前段階の完成値、上限はこの段階のMAXです。エンチャント枠はありません。")}</p>
      <div class="stat-rows growth-equipment-values">
        {#each EQUIPMENT_STAT_KINDS.filter((k) => item.growth_caps![k] > 0) as k (k)}
          <div class="stat-row">
            <span class="k">{EQUIPMENT_STAT_LABELS[k]}</span>
            <NumberField
              label={t("{name}の装備補正", { name: EQUIPMENT_STAT_LABELS[k] })}
              min={item.values_min[k]}
              max={item.growth_caps[k]}
              presets={item.id === "rising-holic-cuffs" ? [{ value: 140, label: "140" }] : []}
              bind:value={part.base[k]}
            />
          </div>
        {/each}
      </div>
    </div>
    {:else}
    {@const enchantPlanStats = enchantPlanStatsFor(slot)}
    <div class="card enchant-card">
      <div class="card-title inline">
        <span>{t("エンチャント")}</span>
      </div>
      <p class="hint dim">{t("通常は突き・斬り・魔攻・魔防の4補正だけ入力します。")}</p>
      <div class="base-value-toolbar">
        <span class="base-value-copy" title={item === null ? t("カタログ外のため入力") : t("数値を押すと例外編集")}><b>{t("装備本体")}</b><small>{item === null ? t("入力") : t("自動")}</small></span>
      </div>
      {#if item === null}
        {@const capStats = EQUIPMENT_STAT_KINDS.filter((k) => part.enchant[k] > 0 || (part.enchant_caps?.[k] ?? 0) > 0)}
        <div class="card custom-enchant-caps">
          <div class="card-title inline">
            <span>{t("エンチャント上限")}</span>
            <span class="dim small">{t("カタログ外は自動で分からないため実測値を入力")}</span>
          </div>
          {#if capStats.length === 0}
            <p class="hint dim">{t("下でエンチャント値を入れると、ここにその補正の上限入力が出ます。")}</p>
          {:else}
            <div class="stat-rows custom-enchant-cap-rows">
              {#each capStats as k (k)}
                <div class="stat-row">
                  <span class="k">{EQUIPMENT_STAT_SHORT[k]}</span>
                  <!-- 上限そのものを入れる欄。これ自身に上限は無いので形態 5(§07) -->
                  <NumberField
                    label={t("{name}のエンチャント上限", { name: EQUIPMENT_STAT_LABELS[k] })}
                    reason={t("カタログ外 · 実測")}
                    bind:value={
                      () => part.enchant_caps?.[k] ?? 0,
                      (v) => { part.enchant_caps = { ...(part.enchant_caps ?? zeroValues()), [k]: v }; }
                    }
                  />
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      <div class="values-paired enchant-first">
        {#each PRIMARY_EQUIPMENT_STATS as k (k)}
          {@render equationRow(k)}
        {/each}
        <!-- 主要でない 5 補正。トリガは**行より先**に置く(後ろだと開いた瞬間に押した場所が下へ動く・§00 03)。
             <details> を段に溶かして(display: contents)、行がそのまま values-paired の子になるようにする -->
        <Disclosure class="equipment-more" summaryClass="enchant-more-toggle" bind:open={showOtherEquipmentStats}>
          {#snippet summary(open)}
            <span class="more-label"><b>{t("物防・命中など5補正")}</b><small>{t("物防 / 命中 / Cri / 回避 / 敏捷")}</small></span>
            <span class="toggle-state">{open ? t("閉じる") : t("開く")}</span>
          {/snippet}
          {#each OTHER_EQUIPMENT_STATS as k (k)}
            {@render equationRow(k)}
          {/each}
        </Disclosure>
      </div>
      <p class="hint dim">{t("シエナのオーラとテシスコアは各専用欄から自動合流します。")}</p>
      {#snippet equationRow(k: EquipmentStatKind)}
          {@const cap = item ? item.enchant_caps[k] : (part.enchant_caps?.[k] ?? null)}
          <!-- 上限 0 = この装備でそのステはエンチャントできない。「上限が未収録」の破線欄を出すと
               入れられない値を入れる場所に見える(§00 ⑤ 考えさせない)。既に値が入っている行だけは
               直せるように欄を残す -->
          {@const noEnchant = cap !== null && cap <= 0 && part.enchant[k] === 0}
          {@const abilityValue = partAbilityValues(slot)[k]}
          <!-- 合計は Rust が部位ごとに出したもの(`part_total_values`)。画面側で足し直さない -->
          {@const displayTotal = partTotalValues(slot)[k]}
          {@const completionPlan = enchantPlanFor(slot, k)}
          <div
            class="value-pair"
            class:plan-stat={enchantPlanStats.includes(k)}
            class:secondary-stat={!PRIMARY_EQUIPMENT_STATS.includes(k)}
          >
            <b>{EQUIPMENT_STAT_SHORT[k]}</b>
            <div class="value-equation">
              <strong class="value-total num">
                <Value class="total-main" motion={() => displayTotal} value={String(displayTotal)} />
                <Value class="enchant-part" motion={() => part.enchant[k]} value={`（＋${part.enchant[k]}）`} />
              </strong>
              {#if abilityValue !== 0}
                <span class="ability-part meta-pill">{t("アビ{v}", { v: abilityValue })}</span>
              {:else}
                <span class="ability-spacer" aria-hidden="true"></span>
              {/if}
              <div class="equation-enchant">
                {#if !noEnchant}
                  <NumberField
                    label={t("{name}のエンチャント", { name: EQUIPMENT_STAT_LABELS[k] })}
                    max={cap ?? undefined}
                    reason={t("上限が未収録")}
                    increments={[12, 14, 17, 20]}
                    bind:value={part.enchant[k]}
                  />
                {/if}
              </div>
              <div class="equation-base">
                <NumberField
                  label={t("{name}の装備本体補正", { name: EQUIPMENT_STAT_LABELS[k] })}
                  autoNote={item === null ? undefined : t("カタログの値")}
                  reason={t("カタログ外 · 入力")}
                  bind:value={part.base[k]}
                />
              </div>
            </div>
            {#if completionPlan !== null && enchantPlanStats.includes(k)}
              <div
                class="enchant-plan inset"
                class:complete={completionPlan.remaining === 0}
                use:changed={() => `${completionPlan.remaining}:${completionPlan.twenty_count}:${completionPlan.seventeen_count}:${completionPlan.remainder}`}
              >
                <span class="plan-remaining"><small>{t("上限まであと")}</small><b class="num">{completionPlan.remaining}</b></span>
                {#if completionPlan.remaining > 0}
                  <span class="plan-recipe num">{enchantCompletionLabel(completionPlan)}</span>
                  <span class="badge num">{t("{n}回", { n: completionPlan.count })}</span>
                {:else}
                  <span class="plan-recipe">{t("強化完了")}</span>
                  <span class="badge">MAX</span>
                {/if}
              </div>
            {/if}
          </div>
      {/snippet}
    </div>
    {/if}

    {#if ABILITY_ALLOWED_SLOTS.includes(slot) && currentAbilitySlotCount(slot) > 0}
      <div class="card ability-card">
        <div class="card-title inline">
          <span>{t("アビリティ")}</span><span class="badge">{part.abilities.length} / {currentAbilitySlotCount(slot)}</span>
          <strong class="ability-impact num"><Value value={abilityImpactSummary(slot)} /></strong>
        </div>
        {#if usesWeaponAbilityUi(slot)}
        <p class="hint dim">
          {slot === "weapon"
            ? t("ゲーム内の3枠と同じ順です。装備中の武器系統に合う候補を押して選びます。")
            : t("双剣Subは武器アビリティと盾アビリティを合わせて2枠まで装着できます。")}
        </p>

        <div class="ability-fixed-list">
          {#each abilityRowsFor(slot) as row (row.category)}
            {@const selectedAbilityId = abilityIdForCategory(slot, row.category)}
            {@const selectedAbility = abilityDef(selectedAbilityId)}
            {@const gradeKey = `weapon-${row.category}`}
            {@const grades = abilityGroup(gradeKey)}
            <div
              class="ability-fixed-row"
              data-ability-id={selectedAbilityId}
              use:changed={() => focusToken(selectedAbilityId)}
            >
              <div class="ability-fixed-label">
                <b>{row.label}</b>
                <span>{row.note} ・ {t("カテゴリ{n}", { n: row.category })}</span>
              </div>
              <!-- カテゴリごとに 1 つ選ぶ(§07「1 つ選ぶ」)。上位等級と「装着しない」はチップで手前に固定、
                   下位等級は候補面へ。候補行の値は効果の要約、絵はゲーム内のアイテム(月石・研磨…) -->
              <div class="ability-choice-list" aria-label={t("{name}の候補", { name: row.label })}>
                <Picker
                  label={t("{name}のアビリティ", { name: row.label })}
                  options={weaponAbilityOptions(grades)}
                  bind:value={() => selectedAbilityId, (v) => setAbilityForCategory(slot, row.category, v)}
                />
              </div>
            </div>

            {#if row.category === 4 && selectedAbility?.additional_slots}
              <div class="ability-additional-panel swap-in">
                <div class="ability-additional-head">
                  <b>{t("ランダム追加")}</b>
                  <span class="badge">{additionsFor(slot, selectedAbility.id).length} / 2</span>
                  <span class="dim">{t("付いている種類を押して足し、実測値を合わせます")}</span>
                </div>
                {#if additionsFor(slot, selectedAbility.id).length < 2}
                  <div class="ro-add-row ability-additional-candidates">
                    {#each addableAdditionalOptions(slot, selectedAbility.id) as option (option.kind)}
                      <Chip class="add" onclick={() => addAdditional(slot, selectedAbility.id, option.kind)}>
                        ＋ {additionalKindLabel(option.kind)}
                        <span class="num dim">
                          {additionalRangeLabel(option)}
                        </span>
                      </Chip>
                    {/each}
                  </div>
                {/if}
                {#each additionsFor(slot, selectedAbility.id) as additional, additionalIndex (`${additional.kind}-${additionalIndex}`)}
                  {@const additionalDef = selectedAbility.additional_options.find((option) => option.kind === additional?.kind)}
                  {#if additionalDef}
                    <div class="siena-row swap-in">
                      <span class="ro-name">{additionalKindLabel(additional.kind)}</span>
                      <NumberField
                        label={t("{name}の値", { name: additionalKindLabel(additional.kind) })}
                        min={additionalDef.min}
                        max={additionalDef.max}
                        bind:value={() => additional.value, (value) => setAdditionalValue(slot, selectedAbility.id, additionalIndex, value)}
                      />
                      <button type="button" class="clear" onclick={() => removeAdditional(slot, selectedAbility.id, additionalIndex)}>{t("外す")}</button>
                    </div>
                  {/if}
                {/each}
                <span class="additional-note dim">{t("固定ダメージ・割合・攻撃補正・命中を保存し、計算へ反映します。")}</span>
              </div>
            {/if}
          {/each}
        </div>
        {:else}
          <p class="hint dim">
            {t("{n}枠まで装着できます。範囲値とランダム追加は選択後に実測値を合わせます。", { n: currentAbilitySlotCount(slot) })}
          </p>
          {@const grades = abilityGroup(slot)}
          {@const full = part.abilities.length >= currentAbilitySlotCount(slot)}
          <div class="ability-choice-list non-weapon-ability-list" aria-label={t("{name}アビリティの候補", { name: PART_SLOT_LABELS[slot] })}>
            {#each grades.shown as ability (ability.id)}
              {@render nonWeaponAbilityChip(slot, ability, part.abilities, full, false)}
            {/each}
            <!-- 畳みボタンは候補の後ろに置き、開いた分はさらに後ろへ足す。
                 押した場所(§00 03)が動かないのはこの順のときだけ。
                 <details> は候補の段に溶かす(display: contents) -->
            {#if grades.folded.length > 0}
              <Disclosure class="lower-grades" summaryClass="chip add lower-grade-toggle">
                {#snippet summary(open)}
                  {open ? t("ほかの等級を畳む") : t("ほかの等級も出す")}
                  <span class="num dim">{grades.folded.length}</span>
                {/snippet}
                {#each grades.folded as ability (ability.id)}
                  {@render nonWeaponAbilityChip(slot, ability, part.abilities, full, true)}
                {/each}
              </Disclosure>
            {/if}
          </div>
          {#each part.abilities as abilityId (abilityId)}
            {@const ability = abilityDef(abilityId)}
            {#if ability?.value_option}
              <div
                class="siena-row ability-value-row swap-in"
                data-ability-id={ability.id}
                use:changed={() => focusToken(ability.id)}
              >
                <span class="ro-name">{t(ability.name)}</span>
                <NumberField
                  label={t("{name}の実測値", { name: t(ability.name) })}
                  min={ability.value_option.min}
                  max={ability.value_option.max}
                  bind:value={() => abilityRecordedValue(slot, ability.id), (value) => setAbilityRecordedValue(slot, ability.id, value)}
                />
                <span class="num dim">/{ability.value_option.max}</span>
              </div>
            {/if}
          {/each}
          {#each part.abilities as abilityId (`addition-${abilityId}`)}
            {@const ability = abilityDef(abilityId)}
            {#if ability && ability.additional_slots > 0}
              <div
                class="ability-additional-panel non-weapon-additional-panel swap-in"
                data-ability-id={ability.id}
                use:changed={() => focusToken(ability.id)}
              >
                <div class="ability-additional-head">
                  <b>{t("{name}のランダム追加", { name: ability.name })}</b>
                  <span class="badge">{additionsFor(slot, ability.id).length} / {ability.additional_slots}</span>
                </div>
                {#if additionsFor(slot, ability.id).length < ability.additional_slots}
                  <div class="ro-add-row ability-additional-candidates">
                    {#each addableAdditionalOptions(slot, ability.id) as option (option.kind)}
                      <Chip class="add" onclick={() => addAdditional(slot, ability.id, option.kind)}>
                        ＋ {additionalKindLabel(option.kind)}
                        <span class="num dim">{additionalRangeLabel(option)}</span>
                      </Chip>
                    {/each}
                  </div>
                {/if}
                {#each additionsFor(slot, ability.id) as additional, additionalIndex (`${additional.kind}-${additionalIndex}`)}
                  {@const option = ability.additional_options.find((candidate) => candidate.kind === additional.kind)}
                  {#if option}
                    <div class="siena-row swap-in">
                      <span class="ro-name">{additionalKindLabel(additional.kind)}</span>
                      <NumberField
                        label={t("{name}の実測値", { name: additionalKindLabel(additional.kind) })}
                        min={option.min} max={option.max}
                        bind:value={() => additional.value, (value) => setAdditionalValue(slot, ability.id, additionalIndex, value)}
                      />
                      <button type="button" class="clear" onclick={() => removeAdditional(slot, ability.id, additionalIndex)}>{t("外す")}</button>
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          {/each}
          {#if slotAbilities(slot).some((ability) => ability.record_only)}
            <span class="additional-note dim">{t("破線の候補は効果を保存しますが、現在の計算項目にない値は合計へ加えません。防御率・回避率の追加効果も同じく記録だけです。")}</span>
          {/if}
        {/if}

      </div>
    {/if}

    {#if ELEMENT_ALLOWED_SLOTS.includes(slot)}
      <p class="hint dim">{t("属性強化はキャラで選択中の属性を自動で +9 反映します。")}</p>
    {/if}

    {#if ENHANCE_ALLOWED_SLOTS.includes(slot)}
      <div class="card">
        <div class="card-title">{t("装備強化")}</div>
        {#if part.item_id === null && part.custom_name !== null}
          <div class="field">
            <span class="field-label">{t("装備種別")}</span>
            <Picker
              label={t("装備種別")}
              options={slot === "weapon" ? weaponEnhanceTypeOptions : armorEnhanceTypeOptions}
              bind:value={() => part.enhance_type ?? "", (v) => (part.enhance_type = v === "" ? null : v as typeof part.enhance_type)}
            />
          </div>
          <p class="hint dim">
            {slot === "weapon" ? t("追加固定ダメージの補正式に使います。") : t("追加HPの算出条件として保存します。")}
          </p>
        {/if}
        <div class="field">
          <span class="field-label">{t("強化 Lv")}</span>
          <Choose
            label={t("強化 Lv")}
            options={enhanceLevelOptions}
            bind:value={() => String(part.enhance_level), (v) => setEnhanceLevel(slot, Number(v))}
          />
        </div>
        {#if part.enhance_level > 0 && part.enhance_type === null}
          <p class="preview-error">
            {slot === "weapon"
              ? t("装備種別を選ぶと追加固定ダメージを計算できます。")
              : t("装備種別を選んでください(追加HPの算出条件に使用します)。")}
          </p>
        {/if}
        {#if part.enhance_level >= 12}
          <div class="field">
            <span class="field-label">{t("等級")}</span>
            <Choose
              label={t("強化の等級")}
              options={enhanceGradeOptions}
              bind:value={() => part.enhance_grade ?? "highest", (v) => (part.enhance_grade = v as typeof part.enhance_grade)}
            />
          </div>
          <p class="hint dim">{t("等級内の上限値を使用します。倍率の端数は四捨五入します。")}</p>
        {:else if part.enhance_level > 0}
          {#if part.enhance_type !== null || item}
            <p class="hint dim">
              {slot === "weapon"
                ? t("追加固定ダメージは自動計算されます(ダメージ計算タブのトレースに表示)。")
                : t("追加HPの算出条件として保存します。現在はHP表示へ反映せず、与ダメージにも加算しません。")}
            </p>
          {:else}
            <p class="hint dim">
              {slot === "weapon"
                ? t("装備種別を選ぶと追加固定ダメージを自動計算します。")
                : t("装備種別を選んでください(追加HPの算出条件に使用します)。")}
            </p>
          {/if}
        {/if}
        <!-- 強化 Lv・等級を押した結果がその場で出る面(§00 ④)。ソウルリンク7/8 の倍率も
             ここで一緒に見せる — 別のペインまで見に行かないと最終値が分からないのを避ける -->
        {#if partEnhance(slot)}
          {@const enhance = partEnhance(slot)!}
          <div class="enhance-readout inset num">
            <span class="enhance-term">
              <span class="dim">{slot === "weapon" ? t("追加ダメージ") : t("追加HP")}</span>
              <b><Value motion={() => enhance.added} value={fmtInt(enhance.added)} /></b>
            </span>
            <span class="enhance-op" aria-hidden="true">×</span>
            <span class="enhance-term">
              <span class="dim">{t("ソウルリンク")}</span>
              <b><Value motion={() => enhance.soul_link_multiplier} value={fmtRate(enhance.soul_link_multiplier)} /></b>
            </span>
            <span class="enhance-op" aria-hidden="true">=</span>
            <span class="enhance-term">
              <span class="dim">{t("合計")}</span>
              <strong><Value motion={() => enhance.total} value={fmtInt(enhance.total)} /></strong>
            </span>
          </div>
        {/if}
      </div>
    {/if}

    {#if RANDOM_OPTION_ALLOWED_SLOTS.includes(slot) && randomOptionSlots(slot) > 0}
      <div class="card">
        <div class="card-title">{t("ランダムオプション")}</div>
        <p class="hint dim">{t("登録済み {n}件。ランダムオプションは専用の入力エリアで設定します。", { n: part.random_options.length })}</p>
      </div>
    {/if}
    </div>
    {/key}
    {/if}

    {/if}
    </div>
    </Modal>
  {/if}
</div>
