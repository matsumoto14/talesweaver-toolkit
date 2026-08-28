// ステータスの表示名と並び順。順序は Rust の StatKind::ALL に合わせる。
import type {
  CoreRegion, CoreType, Element, EquipmentAbilityFamily, PartSlot, PetSkillTier, SienaAuras,
  RandomOptionRank, SkillDependency, StatKind, StatLayer, UltimateSkill,
} from "./api/types";
import { limits } from "./limits.svelte";

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

// 装備補正 9 種(crates/domain/src/equipment.rs の EquipmentValues)の表示名・並び順(wiki Item ページの列順)。
export const EQUIPMENT_STAT_KINDS = [
  "thrust", "slash", "physical_defense", "magic_attack", "magic_defense",
  "accuracy", "critical", "evasion", "agility",
] as const;
export type EquipmentStatKind = (typeof EQUIPMENT_STAT_KINDS)[number];
// 唯一の正は Rust の EquipmentValues::fields()(StatLimits.equipment_stat_labels 経由)。
// CoreType(テシスコア)の表示名も同じテーブルを引く(装備補正とテシスコアで敏捷度補正の表記が食い違っていた事故の再発防止)。
export const EQUIPMENT_STAT_LABELS: Record<EquipmentStatKind, string> = Object.fromEntries(
  limits.equipment_stat_labels.map((e) => [e.kind, e.label]),
) as Record<EquipmentStatKind, string>;
// 表・部位行など幅の狭いところ用の短縮名。
export const EQUIPMENT_STAT_SHORT: Record<EquipmentStatKind, string> = {
  thrust: "突き", slash: "斬り", physical_defense: "物防", magic_attack: "魔攻", magic_defense: "魔防",
  accuracy: "命中", critical: "Cri", evasion: "回避", agility: "敏捷",
};

// 属性 8 種(crates/domain/src/element.rs の Element)。wiki 属性システムの並び。
export const ELEMENTS: Element[] = ["fire", "water", "wind", "earth", "thunder", "white", "black", "neutral"];
export const ELEMENT_LABELS: Record<Element, string> = {
  fire: "火", water: "水", wind: "風", earth: "土", thunder: "雷",
  white: "白", black: "黒", neutral: "無",
};
// 装備に付与できるのは無属性以外(wiki: 装備システム/属性強化「1属性のみ装着可能(火、水、風、土、雷、白、黒)」)。
export const EQUIPMENT_ELEMENTS: Element[] = ELEMENTS.filter((e) => e !== "neutral");
// 装備部位ごとの枠数・可否ルールと部位の並び順(crates/domain/src/equipment.rs の PartSlot の鏡像)。
// 唯一の正は Rust(StatLimits.part_slot_rules)。以下は全てそこからの導出 — ここに新しい判定を足さない。
export const PART_SLOTS: PartSlot[] = limits.part_slot_rules.map((r) => r.slot);
export const PART_SLOT_LABELS: Record<PartSlot, string> = Object.fromEntries(
  limits.part_slot_rules.map((r) => [r.slot, r.label]),
) as Record<PartSlot, string>;
// 装備強化(+1〜+15)を持てる部位(wiki: 装備システム/装備強化。武器・鎧のみ)。
export const ENHANCE_ALLOWED_SLOTS: PartSlot[] =
  limits.part_slot_rules.filter((r) => r.allows_enhance).map((r) => r.slot);
// 装着アビリティ表がある部位(wiki: 装備システム/アビリティ、新装着アビリティ)。
export const ABILITY_ALLOWED_SLOTS: PartSlot[] =
  limits.part_slot_rules.filter((r) => r.allows_ability).map((r) => r.slot);
// 属性強化を持てる部位(wiki: 装備システム冒頭の表「属性強化」行。盾+・レリックは対象外)。
export const ELEMENT_ALLOWED_SLOTS: PartSlot[] =
  limits.part_slot_rules.filter((r) => r.allows_element).map((r) => r.slot);
// ランダムオプションを持てる部位(wiki: 装備システム冒頭の表「転移」行。効果・AF は対象外)。
export const RANDOM_OPTION_ALLOWED_SLOTS: PartSlot[] =
  limits.part_slot_rules.filter((r) => r.allows_random_option).map((r) => r.slot);
// 武器アビリティの系統(crates/domain/src/equipment.rs の EquipmentAbilityFamily)。
// 表示順は加算先(突き / 斬り / 魔攻 / 魔防)の並びに合わせる。
export const ABILITY_FAMILIES: EquipmentAbilityFamily[] = [
  "pointed_blade", "sharp_blade", "intelligence", "magic_resistance", "weapon_delay",
  "armor_polish", "vitality", "mana", "evasion", "shield_polish", "critical",
  "accuracy", "element", "agility", "skill_attack",
];
export const ABILITY_FAMILY_LABELS: Record<EquipmentAbilityFamily, string> = {
  pointed_blade: "尖った刃(突き)",
  sharp_blade: "鋭い刃(斬り)",
  intelligence: "知力(魔攻)",
  magic_resistance: "耐魔力(魔防)",
  weapon_delay: "武器ディレイ",
  armor_polish: "鎧研磨",
  vitality: "生命力",
  mana: "マナ",
  evasion: "機敏(回避)",
  shield_polish: "盾研磨",
  critical: "致命打",
  accuracy: "的中剣",
  element: "属性",
  agility: "敏捷",
  skill_attack: "スキル攻撃力",
};
// シエナのオーラを発現できる部位(wiki: 装備システム冒頭の表「オーラ」行。8 部位)。
export type SienaPartSlot = keyof SienaAuras;
export const SIENA_ALLOWED_SLOTS: SienaPartSlot[] =
  limits.part_slot_rules.filter((r) => r.allows_siena).map((r) => r.slot as SienaPartSlot);
// シエナのオーラの能力値が装備補正(強化能力値)になる部位(wiki: 能力値一覧(武器/盾))。
// それ以外の部位はステの最終固定値増加になる。
export const SIENA_EQUIPMENT_VALUE_SLOTS: SienaPartSlot[] = limits.part_slot_rules
  .filter((r) => r.siena_counts_as_equipment)
  .map((r) => r.slot as SienaPartSlot);

// ランダムオプションのランク(wiki 一覧表の列)。左ほど下位。
export const RANDOM_OPTION_RANKS: RandomOptionRank[] = [
  "normal", "valuable", "rare", "special", "s_true",
];
export const RANDOM_OPTION_RANK_LABELS: Record<RandomOptionRank, string> = {
  normal: "Normal",
  valuable: "Valuable",
  rare: "Rare",
  special: "Special",
  s_true: "S・真",
};
// スキル依存種別(crates/domain/src/skill.rs の SkillDependency)。ランダムOP の効き先表示に使う。
export const SKILL_DEPENDENCY_LABELS: Record<SkillDependency, string> = {
  stab: "突き(STAB依存)",
  hack: "斬り(HACK依存)",
  int: "魔法(INT依存)",
  mr: "神聖(MR依存)",
  stab_hack: "物理複合(STAB+HACK依存)",
  hack_int: "魔法斬り(HACK+INT依存)",
};

// 極限スキル(crates/domain/src/ultimate_skill.rs の UltimateSkill)。wiki Skill/極限 の表順。
export const ULTIMATE_SKILLS: UltimateSkill[] = ["scope_eye", "full_throttle", "wide_focus"];
export const ULTIMATE_SKILL_LABELS: Record<UltimateSkill, string> = {
  scope_eye: "スコープアイ",
  full_throttle: "フルスロットル",
  wide_focus: "ワイドフォーカス",
};
/** 何に効くか(火力に効かないものはそう分かる文言にする)。 */
export const ULTIMATE_SKILL_EFFECTS: Record<UltimateSkill, string> = {
  scope_eye: "クリティカルダメージ増加(非クリには乗りません)",
  full_throttle: "中ディレイ減少 + 単体チャネリングスキルの段数",
  wide_focus: "スキル範囲(火力には効きません)",
};

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
// CoreType は EquipmentStatKind から critical を除いた 8 種と同じ文字列(crates/domain/src/thesis_core.rs
// の CoreType::label が EquipmentValues の表示名をそのまま引く)。表記ゆれ防止のため同じテーブルを使う。
export const CORE_TYPE_LABELS: Record<CoreType, string> = EQUIPMENT_STAT_LABELS;
// テシスコアの装着枠数(wiki: テシスコア効果「装着位置」1〜6)。
export const CORE_SLOT_COUNT = 6;
