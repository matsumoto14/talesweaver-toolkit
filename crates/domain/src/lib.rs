//! ドメインモデルと計算(I/O 無し・決定的)。
//!
//! パイプライン(docs/damage-formula.md §8):
//! ①能力値計算(`stats`) → ②カテゴリ集計(`category`) → ③与ダメージ式(`damage`) → ④段数。

pub mod attack_power;
pub mod awakening;
pub mod category;
pub mod content;
pub mod damage;
pub mod enemy;
pub mod equipment;
pub mod rounding;
pub mod skill;
pub mod stat_sources;
pub mod stats;

pub use attack_power::{attack_power, random_part_max, stat_attack_power, AttackCoefficients};
pub use awakening::Awakening;
pub use category::{CategoryCap, CategoryKind, CategoryTotals, CategoryTrace, DamageCategory};
pub use content::{
    evaluate_content, BestSkillDamage, Content, ContentArea, ContentEvaluation,
    ContentRequirement, RequirementCheck,
};
pub use damage::{
    calculate_damage, evaluate, DamageInput, DamageResult, DamageTrace, DamageTriple, FormulaStep,
};
pub use enemy::Enemy;
pub use equipment::{
    equipment_attack_power, weapon_added_damage, EnhanceRates, Equipment, EquipmentAbilityDef,
    EquipmentCoefficients, EquipmentError, EquipmentPart, EquipmentParts, EquipmentRates,
    EquipmentValues, PartSlot, ENHANCE_LEVEL_MAX, ENHANCE_LEVEL_RANDOM_RANGE_MIN, EQUIPMENT_VALUE_MAX,
    STRONG_WEAPON_LEVEL_MAX,
};
pub use rounding::{floor_int, trunc2};
pub use skill::{Skill, SkillDependency};
pub use stat_sources::{
    apply_pins, apply_temporary_adjustments, build_modifiers, preview_effective_stats, stat_limits,
    Adjustments, BuffCatalog, BuffChoice, BuffDefinition, BuffGroup, BuffSelection, BuffTarget,
    BuffValue, Crown, PetSkillTier, PetSkills, RuneLevels, SacredRelic, StatAdjustment,
    StatContribution, StatLayer, StatLimits, StatPreview, StatSourceError, StatSources,
};
pub use stats::{
    effective_stat, effective_stats, BaseStats, BaseStatsError, EffectiveStats, PinSource, StatKind,
    StatModifierSet, StatModifiers, StatTrace, BASE_STAT_MAX,
};
