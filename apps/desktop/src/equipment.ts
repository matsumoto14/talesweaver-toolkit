// 装備値の共通ヘルパー(部位の基本値レンジ中央・エンチャント上限クランプ・表示用合計)。
// 計算・判定ロジックは Rust 側(crates/domain/src/equipment.rs)にあり、ここは表示・編集用の
// 単純な値組み立てのみ(CLAUDE.md「計算・判定は Rust 側」)。
import type { Equipment, EquipmentPart, EquipmentValues } from "./api/types";
import { PART_SLOTS } from "./labels";

const EQUIPMENT_VALUE_KEYS = ["thrust", "slash", "magic_attack", "magic_defense"] as const;

export const zeroValues = (): EquipmentValues => ({ thrust: 0, slash: 0, magic_attack: 0, magic_defense: 0 });

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

/** 4 値の合計(候補の「上位品」判定など、大小比較の目安にのみ使う)。 */
export const sumValues = (v: EquipmentValues): number =>
  EQUIPMENT_VALUE_KEYS.reduce((s, k) => s + v[k], 0);

export const neutralEquipmentPart = (): EquipmentPart => ({
  item_id: null,
  custom_name: null,
  base: zeroValues(),
  enchant: zeroValues(),
  enhance_level: 0,
  enhance_added_damage: null,
  abilities: [],
});

export const cloneEquipmentPart = (src: EquipmentPart): EquipmentPart => ({
  item_id: src.item_id,
  custom_name: src.custom_name,
  base: { ...src.base },
  enchant: { ...src.enchant },
  enhance_level: src.enhance_level,
  enhance_added_damage: src.enhance_added_damage,
  abilities: [...src.abilities],
});

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
