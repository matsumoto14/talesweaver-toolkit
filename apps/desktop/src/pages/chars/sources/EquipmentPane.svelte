<script lang="ts">
  // 「equipment」補正源のペイン。部位一覧 ⇄ 部位詳細のドリルダウン(§09 規則 2)。
  import type {
    EquipmentAbilityAdditionalKind, EquipmentAbilityDef, EquipmentAbilityFamily, EquipmentItem, EquipmentPart, PartSlot,
    Skill, SkillDependency, StatPreview, WeaponClass, WeaponSystem, WristType,
  } from "../../../api/types";
  import { damageCategoryLabel } from "../../../characterSkills";
  import type { Draft } from "../../../draft";
  import {
    clampToCaps, equipmentIconId, neutralEquipmentPart, rangeSummary, sumValues, valuesSummary, zeroValues,
  } from "../../../equipment";
  import { fmtInt } from "../../../format";
  import {
    ABILITY_ALLOWED_SLOTS, ELEMENT_ALLOWED_SLOTS, ENHANCE_ALLOWED_SLOTS,
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT,
    PART_SLOT_LABELS, PART_SLOTS, RANDOM_OPTION_ALLOWED_SLOTS,
  } from "../../../labels";
  import type { EquipmentStatKind } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { app } from "../../../state.svelte";
  import { bump, flash } from "../../../ui/motion.svelte";
  import Icon from "../../../ui/Icon.svelte";
  import { dropHalfIndex, moveItem } from "../../../ui/reorder.svelte";
  import Select from "../../../ui/Select.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";
  import StatInput from "../../../ui/StatInput.svelte";
  import { slide } from "svelte/transition";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
  }
  let { draft, preview, skills }: Props = $props();

  /** 装備で日常的にエンチャントする4補正。ゲーム内の呼び方どおり S/H/I/M を先に並べる。 */
  const PRIMARY_EQUIPMENT_STATS: EquipmentStatKind[] = ["thrust", "slash", "magic_attack", "magic_defense"];
  const OTHER_EQUIPMENT_STATS: EquipmentStatKind[] = EQUIPMENT_STAT_KINDS.filter(
    (kind) => !PRIMARY_EQUIPMENT_STATS.includes(kind),
  );
  /** 通常エンチャントを持つ全部位。成長装備の盾+とレリックは別の入力モデル。 */
  const ENCHANT_PLAN_SLOTS = new Set<PartSlot>([
    "weapon", "armor", "helm", "shield", "head", "body", "hand", "leg", "effect", "artifact",
  ]);

  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);
  const iconId = (itemId: string | null) => equipmentIconId(itemId, app.equipmentCatalog);

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
    if (openPart !== null) openPart = null;
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
  const relicKindOptions = [
    { value: "godbird", label: "神鳥" },
    { value: "lunaria", label: "ルナリア" },
  ];
  const relicLevelOptions = Array.from({ length: 10 }, (_, i) => ({
    value: String(i + 1),
    label: `+${i + 1}`,
  }));
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
  const randomOptionSlots = (slot: PartSlot) =>
    equippedItem(slot)?.random_option_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.random_option_slots ?? 0));

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
  function setEnhanceLevel(slot: PartSlot, level: number) {
    const part = selectedPart(slot);
    part.enhance_level = level;
    part.enhance_grade = level >= 12 ? (part.enhance_grade ?? "highest") : null;
  }
</script>

<svelte:window onkeydown={closeEquipmentOnEscape} />

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
      <Icon kind="equipment" id={iconId(registered.item_id)} size={20} label={registered.label || `装備 ${registered.id}`} />
      {registered.label || app.equipmentCatalog.find((i) => i.id === registered.item_id)?.name || `装備 ${registered.id}`}
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
  <button type="button" class="part-row" class:on={openPart === slot} onclick={() => openPartDetail(slot)}>
    <Icon kind="equipment" id={iconId(part?.item_id ?? null)} size={28} label={partDisplayName(slot)} />
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
    <div class="equipment-overlay modal-overlay" role="presentation">
    <div class="part-detail modal-surface pane-in" role="dialog" aria-modal="true" aria-label={`${openPartLabel}の装備登録`}>
    <div class="part-detail-header">
      <b>{openPartLabel}の装備登録</b>
      <button type="button" class="btn close-equipment" onclick={() => (openPart = null)}>閉じる <span aria-hidden="true">×</span></button>
    </div>
    {#if draft.equipment.parts[slot].registered.length > 1}
      <div class="part-switches registration-order" aria-label="装備登録の並び順">
        {@render partSwitchList(slot, draft.equipment.parts[slot].registered, draft.equipment.parts[slot].selected_id)}
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
        <Icon kind="equipment" id={iconId(part.item_id)} size={28} label={partDisplayName(slot)} />
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
                <Icon kind="equipment" id={iconId(candidate.id)} size={28} label={candidate.name} />
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
                <span class="enchant-part" use:bump={() => part.enchant[k]}>（＋{part.enchant[k]}）</span>
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
