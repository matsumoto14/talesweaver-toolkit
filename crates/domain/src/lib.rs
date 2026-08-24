//! ドメインモデルと計算(I/O 無し・決定的)。
//!
//! パイプライン(docs/damage-formula.md §8):
//! ①能力値計算(`stats`) → ②カテゴリ集計(`category`) → ③与ダメージ式(`damage`) → ④段数。

pub mod attack_power;
pub mod awakening;
pub mod category;
pub mod content;
pub mod damage;
pub mod defense;
pub mod enemy;
pub mod equipment;
pub mod rounding;
pub mod skill;
pub mod stat_sources;
pub mod stats;
pub mod thesis_core;

pub use attack_power::{
    attack_power, attack_power_breakdown, random_part_max, stat_attack_power, AttackCoefficients,
    AttackPowerBreakdown,
};
pub use awakening::Awakening;
pub use category::{CategoryCap, CategoryKind, CategoryTotals, CategoryTrace, DamageCategory};
pub use content::{
    evaluate_content, BestSkillDamage, Content, ContentArea, ContentEvaluation,
    ContentRequirement, ContentSeries, RequirementCheck,
};
pub use damage::{
    calculate_damage, evaluate, DamageInput, DamageResult, DamageTrace, DamageTriple, FormulaStep,
};
pub use defense::{
    defense_profile, hit_taken_rate, normal_evasion, DefenseProfile, EvasionPoints, NORMAL_EVASION_CAP,
};
pub use enemy::Enemy;
pub use equipment::{
    equipment_attack_power, equipment_values_attack, weapon_added_damage, EnhanceRates, Equipment,
    EquipmentAbilityDef, EquipmentAbilityFamily,
    EquipmentCoefficients, EquipmentError, EquipmentPart, EquipmentParts, EquipmentRates,
    EquipmentValues, PartSlot, SienaAura, SienaStatBonus, ENHANCE_LEVEL_MAX,
    ENHANCE_LEVEL_RANDOM_RANGE_MIN, EQUIPMENT_VALUE_MAX, SIENA_ATTACK_RATE_PERCENT_MAX,
    SIENA_ALL_STATS_BONUS_MAX, SIENA_STAGE_MAX, SIENA_STAT_BONUS_MAX, STRONG_WEAPON_LEVEL_MAX,
};
pub use rounding::{floor_int, trunc2};
pub use skill::{Skill, SkillDependency};
pub use stat_sources::{
    apply_pins, apply_temporary_adjustments, build_modifiers, preview_effective_stats, stat_limits,
    Adjustments, AttackPowerCoefficients, AttackPreview, BuffCatalog, BuffChoice, BuffDefinition,
    BuffGroup, BuffSelection, BuffTarget, BuffValue, Crown, PartAttackContribution, PetSkillTier,
    PetSkills, RuneLevels, SacredRelic, StatAdjustment, StatContribution, StatLayer, StatLimits,
    StatPreview, StatSourceError, StatSources,
};
pub use stats::{
    effective_stat, effective_stats, BaseStats, BaseStatsError, EffectiveStats, PinSource, StatKind,
    StatModifierSet, StatModifiers, StatTrace, BASE_STAT_MAX,
};
pub use thesis_core::{
    CoreRegion, CoreSet, CoreSetBonus, CoreType, ThesisCore, ThesisCoreError, ThesisCores,
    CORE_ENHANCEMENT_MAX, CORE_EVOLUTION_MAX, CORE_SLOT_COUNT,
};
