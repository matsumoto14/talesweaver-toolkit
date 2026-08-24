// Tauri コマンドの入出力の型。Rust の serde 構造体(crates/domain, storage, gamedata)の写し。
// 手で同期しているため、Rust 側の構造体を変えたらここも必ず変える。

export type StatKind = "stab" | "hack" | "int" | "def" | "mr" | "dex" | "agi";
export type BaseStats = Record<StatKind, number>;

export interface Awakening {
  stage: number;
  eternal_level: number;
}

export interface GameCharacter {
  id: string;
  name: string;
}

export type SkillDependency = "stab" | "hack" | "int" | "mr" | "stab_hack" | "hack_int";

export interface Skill {
  id: string;
  name: string;
  dependency: SkillDependency;
  multiplier: number;
  hit_count: number;
  critical_multiplier: number;
}

export interface Enemy {
  id: string;
  name: string;
  defense: number;
  damage_reduction: number;
  cut_rate_a: number;
  element_threshold: number;
}

// ペット S スキルの段階(wiki: PET)。crates/domain/src/stat_sources.rs の PetSkillTier(snake_case)。
export type PetSkillTier = "basic" | "true_lv1" | "true_lv2" | "true_lv3" | "true_lv4";
// ペット S スキル。ステごとに 1 つ(上位段階を選ぶと置き換わる)。未選択は null。
export type PetSkills = Record<StatKind, PetSkillTier | null>;
// ルーンスキル。ステごと 0..=20。
export type RuneLevels = Record<StatKind, number>;
// クラウン。ステごと 0..=300。
export type Crown = Record<StatKind, number>;
// 神鳥の聖物。ステごと 0..=40 段階(実加算値は段階×10)。
export type SacredRelic = Record<StatKind, number>;

// 能力値計算の5レイヤー(wiki §2)。crates/domain/src/stats.rs の StatLayer(snake_case)。
export type StatLayer = "percent_of_base" | "fixed" | "multiplier_a" | "multiplier_b" | "final_fixed";

// バフの対象ステ。crates/domain/src/stat_sources.rs の BuffTarget(rename_all snake_case、外部タグ付け)。
export type BuffTarget = "all_stats" | { stat: StatKind } | "user_selected" | { stats: StatKind[] };

// バフの値の決め方。crates/domain/src/stat_sources.rs の BuffValue(rename_all snake_case、外部タグ付け)。
export type BuffValue = { fixed: number } | { choice: number[] } | { user_input: { min: number; max: number } };

// バフの分類。crates/domain/src/stat_sources.rs の BuffGroup(rename_all snake_case、外部タグ付け)。
export type BuffGroup =
  | "consumable"
  | { character_skill: { game_character_id: string } }
  | "ally_skill";

export interface BuffDefinition {
  id: string;
  name: string;
  target: BuffTarget;
  layer: StatLayer;
  value: BuffValue;
  exclusive_slots: string[];
  source_url: string;
  note: string;
  /** BuffValue::UserInput の初期値。それ以外は null */
  default_value: number | null;
  group: BuffGroup;
}

export interface BuffChoice {
  buff_id: string;
  stat: StatKind | null;
  choice_index: number | null;
  value: number | null;
}

export interface BuffSelection {
  choices: BuffChoice[];
}

export interface StatAdjustment {
  /** このステに +N する(固定値層への加算) */
  add: number;
  /** Some のとき最終能力値をこの値に固定する */
  pin: number | null;
}
export type Adjustments = Record<StatKind, StatAdjustment>;

export interface StatSources {
  pet_skills: PetSkills;
  rune_levels: RuneLevels;
  crown: Crown;
  sacred_relic: SacredRelic;
  buffs: BuffSelection;
  adjustments: Adjustments;
}

export interface StatContribution {
  source: string;
  kind: StatKind;
  layer: StatLayer;
  value: number;
}

// 装備補正 4 種(突き/斬り/魔攻/魔防)。crates/domain/src/equipment.rs の EquipmentValues。
export interface EquipmentValues {
  thrust: number;
  slash: number;
  magic_attack: number;
  magic_defense: number;
}

// 装備部位。crates/domain/src/equipment.rs の PartSlot(snake_case)。
export type PartSlot =
  | "weapon" | "armor" | "helm" | "shield" | "shield_plus"
  | "head" | "body" | "hand" | "leg" | "effect" | "artifact" | "relic";

// 装備部位 1 つ。crates/domain/src/equipment.rs の EquipmentPart。
export interface EquipmentPart {
  /** カタログ参照(EquipmentItem.id)。null = 未装備またはカスタム */
  item_id: string | null;
  /** カタログ外アイテムの表示名 `[仮]` */
  custom_name: string | null;
  /** 実測の基本能力値 */
  base: EquipmentValues;
  /** エンチャント値(強化能力値) */
  enchant: EquipmentValues;
  /** 装備強化 Lv(0..=15)。武器・鎧のみ 0 超を許可 */
  enhance_level: number;
  /** +12 以上の追加固定ダメージ実測値の上書き。+11 以下は null 固定 */
  enhance_added_damage: number | null;
  /** 装備アビリティ id(武器のみ非空を許可) */
  abilities: string[];
}

// 12 部位。crates/domain/src/equipment.rs の EquipmentParts(named field)。
export interface EquipmentParts {
  weapon: EquipmentPart;
  armor: EquipmentPart;
  helm: EquipmentPart;
  shield: EquipmentPart;
  shield_plus: EquipmentPart;
  head: EquipmentPart;
  body: EquipmentPart;
  hand: EquipmentPart;
  leg: EquipmentPart;
  effect: EquipmentPart;
  artifact: EquipmentPart;
  relic: EquipmentPart;
}

// 装備補正一式(部位別装備 12 スロット + パワーウェポン/ストロングウェポン)。
// crates/domain/src/equipment.rs の Equipment。
export interface Equipment {
  parts: EquipmentParts;
  /** パワーウェポン(自身の装備補正を2%増加) */
  power_weapon: boolean;
  /** ストロングウェポンの Lv(0 = 未使用、1〜6) */
  strong_weapon_level: number;
}

// gamedata の出典。crates/gamedata/src/lib.rs の Source。
export interface Source {
  page: string;
  retrieved_on: string;
  note: string;
}

// 武器種(wiki: 装備システム/装備強化「系統」表)。crates/gamedata/src/equipment_catalog.rs の WeaponClass(snake_case)。
export type WeaponClass =
  | "rapier" | "dagger" | "spear" | "small_sword" | "physical_gun" | "claw" | "hand_launcher"
  | "long_sword" | "tachi" | "war_staff" | "short_sword" | "rod" | "nunchaku"
  | "katana" | "axe" | "whip" | "kara" | "dual_blade_physical" | "scythe" | "arming_sword"
  | "magic_wand" | "wand" | "magic_gun" | "scepter" | "totem"
  | "great_sword"
  | "holy_staff" | "handbell" | "dual_blade_magic" | "hammer";

// 装備カタログの 1 アイテム。crates/gamedata/src/equipment_catalog.rs の EquipmentItem。
export interface EquipmentItem {
  id: string;
  slot: PartSlot;
  name: string;
  /** 基本能力値のレンジ下限(wiki: Item ページの MR レンジ) */
  values_min: EquipmentValues;
  /** 基本能力値のレンジ上限 */
  values_max: EquipmentValues;
  /** エンチャント上限(エンチャント不可は全 0) */
  enchant_caps: EquipmentValues;
  /** 武器のみ非 null */
  weapon_class: WeaponClass | null;
  source: Source;
}

// 武器アビリティ定義。crates/domain/src/equipment.rs の EquipmentAbilityDef。
export interface EquipmentAbilityDef {
  id: string;
  name: string;
  /** 装備攻撃力(基本能力値)への加算値 */
  values: EquipmentValues;
}

export interface RegisteredCharacter {
  id: number;
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
  equipment: Equipment;
}

export interface NewCharacter {
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
  equipment: Equipment;
}

export type CategoryKind = "assigned" | "fixed" | "rate";

export interface CategoryCap {
  min: number | null;
  max: number | null;
}

export interface CategoryTrace {
  category: string;
  symbol: string;
  label: string;
  kind: CategoryKind;
  value: number;
  factor: number;
  cap: CategoryCap | null;
}

// pin(能力値の固定)の出所。crates/domain/src/stats.rs の PinSource(snake_case)。
export type PinSource = "saved" | "temporary";

export interface StatTrace {
  kind: StatKind;
  base: number;
  percent_of_base_total: number;
  fixed: number;
  multiplier_a: number;
  basic: number;
  multiplier_b: number;
  multiplier_b_bonus: number;
  final_fixed: number;
  effective: number;
  /** pin(能力値の固定)が適用された場合の上書き前の値。未適用は null */
  pinned_from: number | null;
  /** pin の出所。未適用は null */
  pin_source: PinSource | null;
}

// 7 ステータスすべての最終能力値。crates/domain/src/stats.rs の EffectiveStats。
export type EffectiveStats = Record<StatKind, number>;

// crates/domain/src/stat_sources.rs の StatPreview。preview_effective_stats コマンドの戻り値(保存しない)。
export interface StatPreview {
  stats: EffectiveStats;
  traces: StatTrace[];
  contributions: StatContribution[];
}

export interface FormulaStep {
  name: string;
  expression: string;
  value: number;
}

export interface DamageTriple {
  min: number;
  max: number;
  critical: number;
}

export interface DamageTrace {
  stats: StatTrace[];
  /** ステ補正源(ペット/ルーン/クラウン/聖物/バフ/調整値)の寄与内訳 */
  stat_contributions: StatContribution[];
  categories: CategoryTrace[];
  steps_min: FormulaStep[];
  steps_max: FormulaStep[];
  steps_critical: FormulaStep[];
}

export interface DamageResult {
  per_hit: DamageTriple;
  total: DamageTriple;
  hit_count: number;
  trace: DamageTrace;
}

// crates/domain/src/stat_sources.rs の StatLimits。get_stat_limits コマンドの戻り値。
export interface StatLimits {
  base_stat_max: number;
  rune_level_max: number;
  crown_max: number;
  sacred_relic_stage_max: number;
  adjustment_add_min: number;
  adjustment_add_max: number;
  adjustment_pin_min: number;
  adjustment_pin_max: number;
  equipment_value_max: number;
  strong_weapon_level_max: number;
  /** 装備強化 Lv 上限(wiki: 装備システム/装備強化。+1〜+15) */
  enhance_level_max: number;
  /** +12 以上の追加固定ダメージ実測値の上限(実用上の安全域)`[仮]` */
  enhance_added_damage_max: number;
}


// --- コンテンツ(crates/domain/src/content.rs) ---

// 入場条件。serde の外部タグ付け enum の写し。
// equipment_by_skill は「使うスキルの依存種別で比較先が決まる」条件(swiki の S/H/I・M・複合列)。
export type ContentRequirement =
  | { awakening_stage: number }
  | { eternal_level: number }
  | { equipment_by_skill: { single: number; mr: number; composite: number } };

export interface RequirementCheck {
  label: string;
  current: number;
  required: number;
  ok: boolean;
}

export interface Content {
  id: string;
  name: string;
  /** 敵データが無い(入場条件のみ判定する)コンテンツは null */
  enemy_id: string | null;
  /** 実用的に周回できる 1 ヒット(最大)の目安ダメージ。敵データが無ければ null */
  need_per_hit: number | null;
  requirements: ContentRequirement[];
  /** 判定対象外の入場条件の注記(ルーン Lv・共通スキル・コア等。表示専用) */
  entry_note: string | null;
  team_note: string | null;
}

export interface ContentArea {
  id: string;
  name: string;
  contents: Content[];
}

export interface BestSkillDamage {
  skill_id: string;
  per_hit_max: number;
  total_max: number;
}

export interface ContentEvaluation {
  content_id: string;
  /** スキル未収録キャラ・敵データなしコンテンツは null */
  damage: BestSkillDamage | null;
  checks: RequirementCheck[];
  entry_ok: boolean;
  /** 敵データなし(目安なし)は火力不問で true */
  reaches_need: boolean;
  clear: boolean;
}
