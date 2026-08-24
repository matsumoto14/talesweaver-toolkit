// ステータスの表示名と並び順。順序は Rust の StatKind::ALL に合わせる。
import type { CoreRegion, CoreType, PartSlot, PetSkillTier, StatKind, StatLayer } from "./api/types";

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

// 装備部位(crates/domain/src/equipment.rs の PartSlot)の表示名・並び順(wiki: 装備システム ページ冒頭の表)。
export const PART_SLOTS: PartSlot[] = [
  "weapon", "armor", "helm", "shield", "shield_plus",
  "head", "body", "hand", "leg", "effect", "artifact", "relic",
];
export const PART_SLOT_LABELS: Record<PartSlot, string> = {
  weapon: "武器",
  armor: "鎧",
  helm: "兜",
  shield: "盾",
  shield_plus: "盾+",
  head: "頭",
  body: "体",
  hand: "手",
  leg: "足",
  effect: "効果",
  artifact: "AF",
  relic: "レリック",
};
// 装備強化(+1〜+15)を持てる部位(wiki: 装備システム/装備強化。武器・鎧のみ)。
export const ENHANCE_ALLOWED_SLOTS: PartSlot[] = ["weapon", "armor"];
// 装備アビリティを持てる部位(wiki: 装備システム/アビリティ。武器のみが火力に効く)。
export const ABILITY_ALLOWED_SLOTS: PartSlot[] = ["weapon"];
// シエナのオーラを発現できる部位(wiki: 装備システム冒頭の表「オーラ」行。8 部位)。
export const SIENA_ALLOWED_SLOTS: PartSlot[] = [
  "weapon", "armor", "helm", "shield", "head", "body", "hand", "leg",
];
// シエナのオーラの能力値が装備補正(強化能力値)になる部位(wiki: 能力値一覧(武器/盾))。
// それ以外の部位はステの最終固定値増加になる。
export const SIENA_EQUIPMENT_VALUE_SLOTS: PartSlot[] = ["weapon", "shield"];

// テシスコアの地域(crates/domain/src/thesis_core.rs の CoreRegion)。順序は Rust の CoreRegion::ALL に合わせる。
export const CORE_REGIONS: CoreRegion[] = ["mercurial", "abyss", "eclipse", "rubicona"];
export const CORE_REGION_LABELS: Record<CoreRegion, string> = {
  mercurial: "マーキュリアル洞窟",
  abyss: "アビス",
  eclipse: "エクリプス",
  rubicona: "ルビコナ",
};
// テシスコアのタイプ。火力 4 種は強化能力値に入り、補助 4 種は記録と入場条件の合計にのみ効く
// (経験値タイプはシオカンヘイム専用なので持たない)。
export const CORE_POWER_TYPES: CoreType[] = ["thrust", "slash", "magic_attack", "magic_defense"];
export const CORE_SUPPORT_TYPES: CoreType[] = ["physical_defense", "evasion", "agility", "accuracy"];
export const CORE_TYPES: CoreType[] = [...CORE_POWER_TYPES, ...CORE_SUPPORT_TYPES];
export const CORE_TYPE_LABELS: Record<CoreType, string> = {
  thrust: "突き攻撃力",
  slash: "斬り攻撃力",
  magic_attack: "魔法攻撃力",
  magic_defense: "魔法防御力",
  physical_defense: "物理防御力",
  evasion: "回避率補正",
  agility: "敏捷性補正",
  accuracy: "命中率補正",
};
// テシスコアの装着枠数(wiki: テシスコア効果「装着位置」1〜6)。
export const CORE_SLOT_COUNT = 6;
