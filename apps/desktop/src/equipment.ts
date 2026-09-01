// 装備値の共通ヘルパー(部位の装備本体レンジ中央・エンチャント許容量・表示用合計)。
// 計算・判定ロジックは Rust 側(crates/domain/src/equipment.rs)にあり、ここは表示・編集用の
// 単純な値組み立てのみ(CLAUDE.md「計算・判定は Rust 側」)。
import type {
  CoreSet, Equipment, EquipmentItem, EquipmentPart, EquipmentPartList, EquipmentValues,
  RandomOptionDef, RandomOptionEffect, RandomOptionSlot, RegisteredSienaAura,
  SienaAura, SienaAuraList, SienaAuras, SienaExtraKind, SkillDependency, ThesisCores,
} from "./api/types";
import {
  CORE_REGIONS, CORE_SLOT_COUNT, EQUIPMENT_STAT_KINDS,
  EQUIPMENT_STAT_SHORT, PART_SLOTS, SIENA_ALLOWED_SLOTS, SKILL_DEPENDENCY_LABELS,
} from "./labels";
import { limits } from "./limits.svelte";

const EQUIPMENT_VALUE_KEYS = EQUIPMENT_STAT_KINDS;

/**
 * 装備画像に使う ID。改・セイクリッドはゲーム内で通常版と同じ画像なので、
 * 対応する通常版の ID を返す。対応行が未収録なら自分の ID のまま `?` を表示する。
 */
export const equipmentIconId = (
  itemId: string | null,
  catalog: EquipmentItem[],
): string | null => {
  if (itemId === null) return null;
  const item = catalog.find((candidate) => candidate.id === itemId);
  if (!item?.name.startsWith("†改・セイクリッド")) return itemId;
  const normalName = item.name.replace("†改・", "†");
  return catalog.find((candidate) => candidate.name === normalName)?.id ?? itemId;
};

export const zeroValues = (): EquipmentValues =>
  Object.fromEntries(EQUIPMENT_VALUE_KEYS.map((k) => [k, 0])) as unknown as EquipmentValues;

/** カタログのレンジ(min/max)の中央値(floor)。カタログ選択時の基本能力値の既定セットに使う。 */
export const midpointValues = (min: EquipmentValues, max: EquipmentValues): EquipmentValues =>
  Object.fromEntries(
    EQUIPMENT_VALUE_KEYS.map((k) => [k, Math.floor((min[k] + max[k]) / 2)]),
  ) as unknown as EquipmentValues;

/** 値を指定された上限まで clamp する。 */
export const clampToCaps = (values: EquipmentValues, caps: EquipmentValues): EquipmentValues =>
  Object.fromEntries(
    EQUIPMENT_VALUE_KEYS.map((k) => [k, Math.min(values[k], caps[k])]),
  ) as unknown as EquipmentValues;

/**
 * カタログ品を部位へ適用した結果を返す(EquipmentPane の pickCatalogItem と同じ規則。
 * ホームの「今日の強化」レリック段数ステッパーがこの結果と一致させるために共有する)。
 * item_id / base(= values_max) / enchant(enchant_caps でクランプ) / enhance_type を差し替え、
 * アビリティ枠・ランダムオプション枠を新しい装備の枠数に切り詰める(枠外のアビリティの
 * 実測値・追加値も一緒に落とす)。
 */
export const applyCatalogItem = (part: EquipmentPart, item: EquipmentItem): EquipmentPart => {
  const abilities = part.abilities.slice(0, item.ability_slots);
  const droppedAbilityIds = part.abilities.filter((id) => !abilities.includes(id));
  return {
    ...part,
    item_id: item.id,
    custom_name: null,
    base: { ...item.values_max },
    enchant: clampToCaps(part.enchant, item.enchant_caps),
    enhance_type: item.enhance_type,
    // カタログ品はカタログの enchant_caps が正(resolve_enchant_caps)。カスタム時代の
    // 実測上限を残すとカスタムへ戻したときに古い値が復活するので消す。
    enchant_caps: null,
    abilities,
    ability_values: part.ability_values.filter((v) => !droppedAbilityIds.includes(v.ability_id)),
    ability_additions: part.ability_additions.filter((a) => !droppedAbilityIds.includes(a.ability_id)),
    random_options: part.random_options.slice(0, item.random_option_slots ?? 0),
  };
};

/**
 * 強化 Lv とその等級(enhance_grade)を、Rust 側の検証(crates/domain/src/equipment.rs)が
 * 要求する不変条件を保ったまま一緒に書き換える: Lv >= 12 は enhance_grade 必須(未設定なら
 * 「最上」を既定にする)、Lv < 12 は enhance_grade 禁止(null に戻す)。EquipmentPane の直接編集と
 * ホーム「今日の強化」のステッパー双方から使う(2 箇所に同じ規則を書かないため)。
 */
export const applyEnhanceLevel = (part: EquipmentPart, level: number): EquipmentPart => ({
  ...part,
  enhance_level: level,
  enhance_grade: level >= limits.enhance_grade_min_level ? (part.enhance_grade ?? "highest") : null,
});

/** 9 値の合計(候補の「上位品」判定など、大小比較の目安にのみ使う)。 */
export const sumValues = (v: EquipmentValues): number =>
  EQUIPMENT_VALUE_KEYS.reduce((s, k) => s + v[k], 0);

/**
 * 装備値 1 種の見せ方。**合計を出し、その横に括弧でエンチャント分**を添える
 * (ユーザー確定 2026-09-01「合計をだしてその横にかっこでプラスエンチャント値が正しい表現」)。
 * 括弧はエンチャントが 0 でも出す — 幅が動かないし、盛れる余地が残っていることが分かる。
 */
export const withEnchant = (base: number, enchant: number): string =>
  `${(base + enchant).toLocaleString()} (+${enchant.toLocaleString()})`;

/** 値が大きい上位 2 種の要約(部位行の見出し用)。武器なら「突き 315 (+120) / 斬り 122 (+0)」。 */
export const valuesSummary = (base: EquipmentValues, enchant: EquipmentValues): string => {
  const total = (k: (typeof EQUIPMENT_VALUE_KEYS)[number]) => base[k] + enchant[k];
  const top = EQUIPMENT_VALUE_KEYS.filter((k) => total(k) > 0)
    .sort((a, b) => total(b) - total(a))
    .slice(0, 2);
  return top.length === 0
    ? "—"
    : top.map((k) => `${EQUIPMENT_STAT_SHORT[k]} ${withEnchant(base[k], enchant[k])}`).join(" / ");
};

/** カタログ候補のレンジ要約。max が大きい上位 2 種を「物防260-280 / 魔防230-260」の形で出す。 */
export const rangeSummary = (min: EquipmentValues, max: EquipmentValues): string => {
  const top = EQUIPMENT_VALUE_KEYS.filter((k) => max[k] > 0).sort((a, b) => max[b] - max[a]).slice(0, 2);
  return top.length === 0
    ? "—"
    : top.map((k) => `${EQUIPMENT_STAT_SHORT[k]}${min[k]}-${max[k]}`).join(" / ");
};

export const neutralSienaAura = (): SienaAura => ({ slots: [], extras: [] });

export const cloneSienaAura = (src: SienaAura): SienaAura => ({
  slots: src.slots.map((s) => ({ ...s })),
  extras: src.extras.map((e) => ({ ...e })),
});
export const neutralSienaAuraList = (): SienaAuraList => ({ registered: [], selected_id: null });
export const neutralSienaAuras = (): SienaAuras =>
  Object.fromEntries(SIENA_ALLOWED_SLOTS.map((slot) => [slot, neutralSienaAuraList()])) as unknown as SienaAuras;
export const cloneRegisteredSienaAura = (src: RegisteredSienaAura): RegisteredSienaAura => ({
  id: src.id, label: src.label, aura: cloneSienaAura(src.aura),
});
export const cloneSienaAuras = (src: SienaAuras): SienaAuras =>
  Object.fromEntries(SIENA_ALLOWED_SLOTS.map((slot) => [slot, {
    selected_id: src[slot].selected_id,
    registered: src[slot].registered.map(cloneRegisteredSienaAura),
  }])) as unknown as SienaAuras;
export const selectedSienaAuraRegistration = (list: SienaAuraList): RegisteredSienaAura | null =>
  list.registered.find((entry) => entry.id === list.selected_id) ?? null;
export const selectedSienaAura = (list: SienaAuraList): SienaAura | null =>
  selectedSienaAuraRegistration(list)?.aura ?? null;

/** 増幅段階 = 能力値スロットの数(wiki: 段階ごとに 1 個解放)。 */
export const sienaStage = (siena: SienaAura): number => siena.slots.length;

/** いま解放されている追加オプションの枠数(段階 3/7/10 で 1/2/3)。 */
export const sienaExtraCapacity = (siena: SienaAura, unlockStages: readonly number[]): number =>
  unlockStages.filter((stage) => sienaStage(siena) >= stage).length;

/** 追加オプションの合計 %(同じ種類は 1 部位 1 個なので実質その値)。 */
export const sienaExtraValue = (siena: SienaAura, kind: SienaExtraKind): number =>
  siena.extras.filter((e) => e.kind === kind).reduce((sum, e) => sum + e.value, 0);

export const neutralCoreSet = (): CoreSet => ({ slots: Array(CORE_SLOT_COUNT).fill(null) });

export const cloneCoreSet = (src: CoreSet): CoreSet => ({
  slots: Array.from({ length: CORE_SLOT_COUNT }, (_, i) => {
    const core = src.slots[i] ?? null;
    return core ? { ...core } : null;
  }),
});

export const neutralThesisCores = (): ThesisCores =>
  Object.fromEntries(CORE_REGIONS.map((r) => [r, neutralCoreSet()])) as unknown as ThesisCores;

export const cloneThesisCores = (src: ThesisCores): ThesisCores =>
  Object.fromEntries(CORE_REGIONS.map((r) => [r, cloneCoreSet(src[r])])) as unknown as ThesisCores;

export const neutralEquipmentPart = (): EquipmentPart => ({
  id: 0,
  label: "",
  item_id: null,
  custom_name: null,
  base: zeroValues(),
  enchant: zeroValues(),
  enhance_level: 0,
  enhance_type: null,
  enhance_grade: null,
  abilities: [],
  ability_values: [],
  ability_additions: [],
  random_options: [],
  enchant_caps: null,
});

export const cloneEquipmentPart = (src: EquipmentPart): EquipmentPart => ({
  id: src.id,
  label: src.label,
  item_id: src.item_id,
  custom_name: src.custom_name,
  base: { ...src.base },
  enchant: { ...src.enchant },
  enhance_level: src.enhance_level,
  enhance_type: src.enhance_type,
  enhance_grade: src.enhance_grade,
  abilities: [...src.abilities],
  ability_values: (src.ability_values ?? []).map((a) => ({ ...a })),
  ability_additions: (src.ability_additions ?? []).map((a) => ({ ...a })),
  random_options: (src.random_options ?? []).map((o) => ({ ...o })),
  enchant_caps: src.enchant_caps ? { ...src.enchant_caps } : null,
});

export const selectedEquipmentPart = (list: EquipmentPartList): EquipmentPart | null =>
  list.registered.find((part) => part.id === list.selected_id) ?? null;

export const selectedEquipmentPartOrNeutral = (list: EquipmentPartList): EquipmentPart =>
  selectedEquipmentPart(list) ?? neutralEquipmentPart();

// --- 神鳥の聖物 -------------------------------------------------------------
// 段階↔実際の値の換算。正は crates/domain/src/stat_sources.rs の SacredRelic::value /
// stage_from_value(段階 × 1段あたりの値、逆算は端数切り捨て)。
// ステッパー1押しごとに押した瞬間へ反映する楽観更新(§00 04)のため IPC を挟めず、ここで同じ式をミラーする。

/** 段階 → 実際に増える値。 */
export const sacredRelicValue = (stage: number, valuePerStage: number): number => stage * valuePerStage;

/** 実際の値 → 段階(端数切り捨て・範囲外は clamp)。 */
export const sacredRelicStageFromValue = (
  value: number,
  stageMax: number,
  valuePerStage: number,
): number => Math.max(0, Math.min(stageMax, Math.floor(value / valuePerStage)));

// --- ランダムオプション ---------------------------------------------------
// 判定・集計は Rust 側(crates/domain/src/random_option.rs)。ここは表示・編集用。

/** この枠の効果値。上書きが無ければレンジ上限(Rust の RandomOptionSlot::value と同じ規則)。 */
export const randomOptionValue = (slot: RandomOptionSlot, def: RandomOptionDef): number =>
  slot.value ?? (def.tiers.find((t) => t.rank === slot.rank)?.max ?? 0);

/**
 * 一覧のバッジに出す値。効き先で単位が違う —
 * 割合(与ダメージ増加・攻撃ダメージ・追加ダメージ・耐性)は `%`、命中P/回避P は実数、
 * 中ディレイは減る側なので `−`。
 */
export const randomOptionValueLabel = (slot: RandomOptionSlot, def: RandomOptionDef): string => {
  const value = randomOptionValue(slot, def);
  const effect = def.effect;
  if (typeof effect === "object") return `+${value}%`;
  switch (effect) {
    case "accuracy_point":
    case "evasion_point":
    case "accuracy_and_evasion_point":
      return `+${value}`;
    case "actual_delay_reduction":
      return `−${value}%`;
    case "min_evasion_rate":
    default:
      return `+${value}%`;
  }
};

/** 効き先の表示名。「記録するだけ」の OP はそう分かる文言にする。 */
export const randomOptionEffectLabel = (effect: RandomOptionEffect): string => {
  if (typeof effect === "object") {
    return `与ダメージ増加 ${SKILL_DEPENDENCY_LABELS[effect.dependency_damage_rate]}`;
  }
  switch (effect) {
    case "attack_damage_rate": return "攻撃ダメージ増加";
    case "added_damage_rate": return "割合追加ダメージ";
    case "physical_added_damage_rate": return "割合追加ダメージ(物理依存のみ)";
    case "magic_added_damage_rate": return "割合追加ダメージ(魔法依存のみ)";
    case "physical_damage_amplify": return "ダメージ増幅(物理依存のみ)";
    case "magic_damage_amplify": return "ダメージ増幅(魔法依存のみ)";
    case "accuracy_point": return "命中P";
    case "evasion_point": return "回避P";
    case "accuracy_and_evasion_point": return "命中P・回避P";
    case "actual_delay_reduction": return "中ディレイ減少";
    case "min_evasion_rate": return "最小回避率補正";
    case "record_only": return "記録するだけ(計算に入りません)";
  }
};

export const randomOptionIsApplied = (effect: RandomOptionEffect): boolean => effect !== "record_only";

/** 選択スキルの依存種別で発動できる OP か。条件を持たない OP とスキル未選択時は候補に残す。 */
export const randomOptionMatchesDependency = (
  effect: RandomOptionEffect,
  dependency: SkillDependency | null,
): boolean => {
  if (dependency === null) return true;
  if (effect === "physical_added_damage_rate") {
    return dependency === "stab" || dependency === "hack" || dependency === "stab_hack";
  }
  if (effect === "magic_added_damage_rate") {
    return dependency === "int" || dependency === "mr" || dependency === "hack_int";
  }
  if (effect === "physical_damage_amplify") {
    return dependency === "stab" || dependency === "hack" || dependency === "stab_hack";
  }
  if (effect === "magic_damage_amplify") {
    return dependency === "int" || dependency === "mr" || dependency === "hack_int";
  }
  return true;
};

/** 全部位のランダムOP の枠数(補正源リストのサマリ用)。 */
export const randomOptionCount = (equipment: Equipment): number =>
  PART_SLOTS.reduce((n, slot) => n + selectedEquipmentPartOrNeutral(equipment.parts[slot]).random_options.length, 0);

/** 計算に入らない(記録するだけの)枠数。 */
export const randomOptionRecordOnlyCount = (equipment: Equipment, defs: RandomOptionDef[]): number =>
  PART_SLOTS.reduce(
    (n, slot) =>
      n +
      selectedEquipmentPartOrNeutral(equipment.parts[slot]).random_options.filter((o) => {
        const def = defs.find((d) => d.id === o.option_id);
        return def !== undefined && !randomOptionIsApplied(def.effect);
      }).length,
    0,
  );

/** シエナのオーラを発現している部位数(表示用)。 */
export const sienaPartCount = (equipment: Equipment): number =>
  SIENA_ALLOWED_SLOTS.filter((slot) => sienaStage(selectedSienaAura(equipment.siena[slot]) ?? neutralSienaAura()) > 0).length;
