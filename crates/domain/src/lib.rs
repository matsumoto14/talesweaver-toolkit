//! ドメインモデルと計算(I/O 無し・決定的)。
//!
//! パイプライン(docs/damage-formula.md §13):
//! ①能力値計算(`stats`) → ②カテゴリ集計(`category`) → ③与ダメージ式(`damage`) → ④段数。

pub mod actual_delay;
pub mod character_skill;
pub mod attack_power;
pub mod awakening;
pub mod category;
pub mod common_skill;
pub mod content;
pub mod critical_rate;
pub mod damage;
pub mod defense;
pub mod element;
pub mod enemy;
pub mod equipment;
pub mod mastery;
pub mod random_option;
pub mod rounding;
pub mod siena;
pub mod skill;
pub mod stat_sources;
pub mod stats;
pub mod thesis_core;
pub mod title;
pub mod ultimate_skill;

pub use actual_delay::{
    actual_delay, ActualDelay, ActualDelayContribution, SkillUsesTable, ACTUAL_DELAY_MIN,
    ACTUAL_DELAY_REDUCTION_MAX, SECONDS_PER_MINUTE,
};
pub use character_skill::{
    CharacterSkillCatalog, CharacterSkillDef, CharacterSkillError, CharacterSkills,
    MasteryOverride, SkillAudience, SkillEffect,
};
pub use attack_power::{
    attack_power, attack_power_breakdown, random_part_max, stat_attack_power, AttackCoefficients,
    AttackPowerBreakdown,
};
pub use awakening::{Awakening, AwakeningCaps};
pub use category::{CategoryCap, CategoryKind, CategoryTotals, CategoryTrace, DamageCategory};
pub use common_skill::{
    CommonSkillError, CommonSkills, DefenseRates, UnleashSlot, AUGMENT_LEVEL_MAX,
    KAI_PROTECT_ARMOR_LEVEL_MAX, PROTECT_ARMOR_LEVEL_MAX, REINFORCE_LEVEL_MAX,
    SHARPNESS_VISION_LEVEL_MAX, STRONG_WEAPON_LEVEL_MAX, UNLEASH_LEVEL_MAX, UNLEASH_SLOTS,
};
pub use content::{
    evaluate_content, BestSkillDamage, Content, ContentArea, ContentEvaluation,
    ContentRequirement, ContentSeries, RequirementCheck,
};
pub use critical_rate::{
    critical_rate, CriticalRate, CriticalRateError, CriticalRateSourceId, CriticalRateSources,
    ARCHITECT_LAB_PER_STAGE, ARCHITECT_LAB_STAGE_MAX, CRITICAL_RATE_BONUS_MAX,
};
pub use damage::{
    calculate_damage, evaluate, DamageInput, DamageResult, DamageTrace, DamageTriple, DpsTriple,
    FormulaStep,
};
pub use defense::{
    accuracy_point, defense_profile, AccuracyCorrection, DefenseProfile, EvasionPoints,
};
pub use element::{
    Element, ElementPreview, ElementSourceDef, ElementSourceId, ElementSources, ElementValues,
    EQUIPMENT_ELEMENT_VALUE_MAX,
};
pub use enemy::Enemy;
pub use equipment::{
    equipment_attack_power, equipment_values_attack, weapon_added_damage, EnhanceRates, Equipment,
    EquipmentAbilityDef, EquipmentAbilityFamily,
    EquipmentCoefficients, EquipmentError, EquipmentPart, EquipmentParts, EquipmentRates,
    EquipmentValues, PartSlot, SienaStatBonus, ENHANCE_LEVEL_MAX,
    ENHANCE_LEVEL_RANDOM_RANGE_MIN, EQUIPMENT_VALUE_MAX,
};
pub use mastery::{Masteries, MasteryCatalog, MasteryDef, MasteryError};
pub use random_option::{
    DependencyRates, RandomOptionDef, RandomOptionEffect, RandomOptionError, RandomOptionRank,
    RandomOptionSlot, RandomOptionTier, RandomOptionTotals, RANDOM_OPTION_VALUE_MAX,
};
pub use rounding::{floor_int, trunc2};
pub use siena::{
    siena_catalog, SienaAura, SienaCatalog, SienaError, SienaExtraKind, SienaExtraKindDef,
    SienaExtraSlot, SienaSlot, SienaValueKind, SienaValueKindDef, SIENA_EXTRA_UNLOCK_STAGES,
    SIENA_STAGE_MAX,
};
pub use skill::{Skill, SkillDependency, SkillTarget};
pub use stat_sources::{
    apply_character_skills, apply_masteries, apply_pins, apply_temporary_adjustments, apply_unleash, build_modifiers,
    preview_effective_stats, stat_limits,
    Adjustments, AttackPowerCoefficients, AttackPreview, BuffCatalog, BuffChoice, BuffDefinition,
    BuffSelection, BuffTarget, BuffValue, Crown, MonsterCards, PartAttackContribution,
    PetSkillTier,
    PetSkills, RuneLevels, SacredRelic, StatAdjustment, StatContribution, StatLayer, StatLimits,
    StatPreview, StatSourceError, StatSources,
};
pub use stats::{
    effective_stat, effective_stats, BaseStats, BaseStatsError, EffectiveStats, PinSource, StatKind,
    StatModifierSet, StatModifiers, StatTrace, BASE_STAT_MAX,
};
pub use title::{
    title_attack_damage_rate, title_values, TitleDef, TitleError, TitleKind,
};
pub use ultimate_skill::{
    UltimateSkill, UltimateSkillError, UltimateSkills, HYPER_LIMIT_LEVEL_MAX, ULTIMATE_SKILL_SLOTS,
};
pub use thesis_core::{
    CoreRegion, CoreSet, CoreSetBonus, CoreType, ThesisCore, ThesisCoreError, ThesisCores,
    CORE_ENHANCEMENT_MAX, CORE_EVOLUTION_MAX, CORE_SLOT_COUNT,
};
