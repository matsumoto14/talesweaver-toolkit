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

export interface RegisteredCharacter {
  id: number;
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
}

export interface NewCharacter {
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
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
}

