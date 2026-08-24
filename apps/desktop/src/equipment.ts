// 装備値の共通ヘルパー(部位の基本値レンジ中央・エンチャント上限クランプ・表示用合計)。
// 計算・判定ロジックは Rust 側(crates/domain/src/equipment.rs)にあり、ここは表示・編集用の
// 単純な値組み立てのみ(CLAUDE.md「計算・判定は Rust 側」)。
import type {
  CoreRegion, CoreSet, CoreType, Equipment, EquipmentPart, EquipmentValues, SienaAura,
  SienaStatBonus, ThesisCores,
} from "./api/types";
import {
  CORE_POWER_TYPES, CORE_REGIONS, CORE_SLOT_COUNT, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT,
  PART_SLOTS, STAT_KINDS,
} from "./labels";

const EQUIPMENT_VALUE_KEYS = EQUIPMENT_STAT_KINDS;

export const zeroValues = (): EquipmentValues =>
  Object.fromEntries(EQUIPMENT_VALUE_KEYS.map((k) => [k, 0])) as unknown as EquipmentValues;

/** カタログのレンジ(min/max)の中央値(floor)。カタログ選択時の基本能力値の既定セットに使う。 */
export const midpointValues = (min: EquipmentValues, max: EquipmentValues): EquipmentValues =>
  Object.fromEntries(
    EQUIPMENT_VALUE_KEYS.map((k) => [k, Math.floor((min[k] + max[k]) / 2)]),
  ) as unknown as EquipmentValues;

/** 値をカタログの上限(エンチャント上限等)まで clamp する。 */
export const clampToCaps = (values: EquipmentValues, caps: EquipmentValues): EquipmentValues =>
  Object.fromEntries(
    EQUIPMENT_VALUE_KEYS.map((k) => [k, Math.min(values[k], caps[k])]),
  ) as unknown as EquipmentValues;

/** 9 値の合計(候補の「上位品」判定など、大小比較の目安にのみ使う)。 */
export const sumValues = (v: EquipmentValues): number =>
  EQUIPMENT_VALUE_KEYS.reduce((s, k) => s + v[k], 0);

/** 値が大きい上位 2 種の要約(部位行の見出し用)。武器なら「突き122 / 斬り315」、鎧なら「物防270 / 魔防245」。 */
export const valuesSummary = (v: EquipmentValues): string => {
  const top = EQUIPMENT_VALUE_KEYS.filter((k) => v[k] > 0).sort((a, b) => v[b] - v[a]).slice(0, 2);
  return top.length === 0 ? "—" : top.map((k) => `${EQUIPMENT_STAT_SHORT[k]}${v[k]}`).join(" / ");
};

/** カタログ候補のレンジ要約。max が大きい上位 2 種を「物防260-280 / 魔防230-260」の形で出す。 */
export const rangeSummary = (min: EquipmentValues, max: EquipmentValues): string => {
  const top = EQUIPMENT_VALUE_KEYS.filter((k) => max[k] > 0).sort((a, b) => max[b] - max[a]).slice(0, 2);
  return top.length === 0
    ? "—"
    : top.map((k) => `${EQUIPMENT_STAT_SHORT[k]}${min[k]}-${max[k]}`).join(" / ");
};

export const zeroStatBonus = (): SienaStatBonus =>
  Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as unknown as SienaStatBonus;

export const neutralSienaAura = (): SienaAura => ({
  stage: 0,
  values: zeroValues(),
  stats: zeroStatBonus(),
  all_stats: 0,
  attack_rate_percent: 0,
});

export const cloneSienaAura = (src: SienaAura): SienaAura => ({
  stage: src.stage,
  values: { ...src.values },
  stats: { ...src.stats },
  all_stats: src.all_stats,
  attack_rate_percent: src.attack_rate_percent,
});

/** 部位ごとのステ加算合計(能力値スロット + 全ステータス増加 × 7 ステ)。表示用 */
export const sienaPartStatTotal = (siena: SienaAura): number =>
  STAT_KINDS.reduce((sum, k) => sum + siena.stats[k] + siena.all_stats, 0);

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
  item_id: null,
  custom_name: null,
  base: zeroValues(),
  enchant: zeroValues(),
  enhance_level: 0,
  enhance_added_damage: null,
  abilities: [],
  siena: neutralSienaAura(),
});

export const cloneEquipmentPart = (src: EquipmentPart): EquipmentPart => ({
  item_id: src.item_id,
  custom_name: src.custom_name,
  base: { ...src.base },
  enchant: { ...src.enchant },
  enhance_level: src.enhance_level,
  enhance_added_damage: src.enhance_added_damage,
  abilities: [...src.abilities],
  siena: cloneSienaAura(src.siena),
});

/** テシスコアの補正値(wiki: 進化強化表)。判定・計算は Rust 側。ここは表示用。 */
const CORE_POWER_BONUS: number[][] = [
  [1, 2, 3, 4, 5],
  [6, 7, 8, 9, 10],
  [12, 14, 16, 18, 20],
  [23, 26, 29, 32, 35],
  [40, 50, 60, 70, 80],
];
/** 補助タイプは進化4 の強化1 以降だけ火力と値が分かれる。 */
const CORE_SUPPORT_BONUS: number[][] = [
  [1, 2, 3, 4, 5],
  [6, 7, 8, 9, 10],
  [12, 14, 16, 18, 20],
  [23, 26, 29, 32, 35],
  [40, 45, 50, 55, 60],
];

/** コア 1 個の補正値(表示用) */
export const coreBonus = (type: CoreType, evolution: number, enhancement: number): number => {
  const table = CORE_POWER_TYPES.includes(type) ? CORE_POWER_BONUS : CORE_SUPPORT_BONUS;
  return table[evolution]?.[enhancement] ?? 0;
};

/** 6 枠の補正値合計(入場条件「コア N」と同じ値。表示用) */
export const coreSetTotalBonus = (set: CoreSet): number =>
  set.slots.reduce((sum, core) => sum + (core ? coreBonus(core.core_type, core.evolution, core.enhancement) : 0), 0);

/** 6 枠の補助タイプの合計(装備値 9 種のうち 物防/回避/敏捷/命中)。表示用 */
export const coreSetSupportValues = (set: CoreSet): EquipmentValues => {
  const total = zeroValues();
  for (const core of set.slots) {
    if (!core || CORE_POWER_TYPES.includes(core.core_type)) continue;
    total[core.core_type] += coreBonus(core.core_type, core.evolution, core.enhancement);
  }
  return total;
};

/** 全地域のコア合計のうち最大(補正源リストのサマリ表示用) */
export const thesisCoresBestTotal = (cores: ThesisCores): number =>
  Math.max(0, ...CORE_REGIONS.map((r: CoreRegion) => coreSetTotalBonus(cores[r])));

function sumParts(equipment: Equipment, pick: (p: EquipmentPart) => EquipmentValues): EquipmentValues {
  const total = zeroValues();
  for (const slot of PART_SLOTS) {
    const v = pick(equipment.parts[slot]);
    for (const k of EQUIPMENT_VALUE_KEYS) total[k] += v[k];
  }
  return total;
}

/** Σ part.base(表示用。実際の集計は Rust 側 Equipment::base_totals がアビリティ込みで行う)。 */
export const equipmentBaseTotal = (equipment: Equipment): EquipmentValues => sumParts(equipment, (p) => p.base);
/** Σ part.enchant(表示用。実際の集計は Rust 側 Equipment::enhanced_totals)。 */
export const equipmentEnchantTotal = (equipment: Equipment): EquipmentValues => sumParts(equipment, (p) => p.enchant);

/** シエナのオーラの攻撃力増加(New1)の合計 %(表示用)。 */
export const sienaAttackRatePercent = (equipment: Equipment): number =>
  PART_SLOTS.reduce((sum, slot) => sum + equipment.parts[slot].siena.attack_rate_percent, 0);

/** シエナのオーラのステ加算の合計(全部位・全ステ。表示用)。 */
export const sienaStatTotal = (equipment: Equipment): number =>
  PART_SLOTS.reduce((sum, slot) => sum + sienaPartStatTotal(equipment.parts[slot].siena), 0);

/** シエナのオーラを発現している部位数(表示用)。 */
export const sienaPartCount = (equipment: Equipment): number =>
  PART_SLOTS.filter((slot) => equipment.parts[slot].siena.stage > 0).length;
