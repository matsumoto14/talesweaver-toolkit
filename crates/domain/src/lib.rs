//! ドメインモデルと計算(I/O 無し・決定的)。
//!
//! パイプライン(docs/damage-formula.md §8):
//! ①能力値計算(`stats`) → ②カテゴリ集計(`category`) → ③与ダメージ式(`damage`) → ④段数。

pub mod attack_power;
pub mod awakening;
pub mod category;
pub mod damage;
pub mod enemy;
pub mod rounding;
pub mod skill;
pub mod stat_sources;
pub mod stats;

pub use attack_power::{attack_power, random_part_max, stat_attack_power, AttackCoefficients};
pub use awakening::Awakening;
pub use category::{CategoryCap, CategoryKind, CategoryTotals, CategoryTrace, DamageCategory};
pub use damage::{
    calculate_damage, evaluate, DamageInput, DamageResult, DamageTrace, DamageTriple, FormulaStep,
};
pub use enemy::Enemy;
pub use rounding::{floor_int, trunc2};
pub use skill::{Skill, SkillDependency};
pub use stat_sources::{
    apply_pins, apply_temporary_adjustments, build_modifiers, merge_pins, preview_effective_stats,
    Adjustments, BuffCatalog, BuffChoice, BuffDefinition, BuffGroup, BuffSelection, BuffTarget,
    BuffValue, Crown, PetSkillTier, PetSkills, RuneLevels, SacredRelic, StatAdjustment,
    StatContribution, StatLayer, StatPreview, StatSourceError, StatSources,
};
pub use stats::{
    effective_stat, effective_stats, BaseStats, BaseStatsError, EffectiveStats, StatKind,
    StatModifierSet, StatModifiers, StatTrace, BASE_STAT_MAX,
};
