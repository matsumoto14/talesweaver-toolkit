// ステータスの表示名と並び順。順序は Rust の StatKind::ALL に合わせる。
import type { PetSkillTier, StatKind, StatLayer } from "./api/types";

export const STAT_KINDS: StatKind[] = ["stab", "hack", "int", "def", "mr", "dex", "agi"];
export const STAT_LABELS: Record<StatKind, string> = {
  stab: "STAB", hack: "HACK", int: "INT", def: "DEF", mr: "MR", dex: "DEX", agi: "AGI",
};

export const STAT_LAYER_LABELS: Record<StatLayer, string> = {
  percent_of_base: "割合増加",
  fixed: "固定値",
  multiplier_a: "倍率A",
  multiplier_b: "倍率B",
  final_fixed: "最終固定値",
};

export const PET_SKILL_TIER_LABELS: Record<PetSkillTier, string> = {
  basic: "強化",
  true_lv1: "真Lv1",
  true_lv2: "真Lv2",
  true_lv3: "真Lv3",
  true_lv4: "真Lv4",
};

// 装備補正 4 種(crates/domain/src/equipment.rs の EquipmentValues)の表示名・並び順。
export const EQUIPMENT_STAT_KINDS = ["thrust", "slash", "magic_attack", "magic_defense"] as const;
export type EquipmentStatKind = (typeof EQUIPMENT_STAT_KINDS)[number];
export const EQUIPMENT_STAT_LABELS: Record<EquipmentStatKind, string> = {
  thrust: "突き攻撃力",
  slash: "斬り攻撃力",
  magic_attack: "魔法攻撃力",
  magic_defense: "魔法防御力",
};
