//! ドメインモデルと計算(I/O 無し・決定的)。
//!
//! パイプライン(docs/damage-formula.md §13):
//! ①能力値計算(`stats`) → ②カテゴリ集計(`category`) → ③与ダメージ式(`damage`) → ④段数。

pub mod actual_delay;
pub mod attack_power;
pub mod awakening;
pub mod candidate;
pub mod category;
pub mod character;
pub mod character_skill;
pub mod common_skill;
pub mod content;
pub mod content_evaluation;
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
pub mod soul_link;
pub mod stat_sources;
pub mod stats;
pub mod thesis_core;
pub mod title;
pub mod ultimate_skill;
pub mod validation;

pub use actual_delay::{
    actual_delay, ActualDelay, ActualDelayContribution, SkillUsesTable, ACTUAL_DELAY_MIN,
    ACTUAL_DELAY_REDUCTION_MAX, SECONDS_PER_MINUTE,
};
pub use attack_power::{
    attack_power, attack_power_breakdown, random_part_max, stat_attack_power, AttackCoefficients,
    AttackPowerBreakdown,
};
pub use awakening::{Awakening, AwakeningCaps};
pub use candidate::{
    aura_candidates, enchant_candidates, enchant_dependency_keys, enhance_candidates,
    list_candidate_changes, quick_win_candidates, rank_candidates, CandidateChange, CandidateCost,
    RankedCandidate,
};
pub use category::{CategoryCap, CategoryKind, CategoryTotals, CategoryTrace, DamageCategory};
pub use character::NewCharacter;
pub use character_skill::{
    damage_contributions, CharacterSkillCatalog, CharacterSkillDef, CharacterSkillError,
    CharacterSkills, MasteryOverride, SkillAudience, SkillEffect,
};
pub use common_skill::{
    CommonSkillError, CommonSkills, DefenseRates, RateContribution, UnleashSlot,
    AUGMENT_LEVEL_MAX, KAI_PROTECT_ARMOR_LEVEL_MAX, PROTECT_ARMOR_LEVEL_MAX, REINFORCE_LEVEL_MAX,
    SHARPNESS_VISION_LEVEL_MAX, STRONG_WEAPON_LEVEL_MAX, UNLEASH_LEVEL_MAX, UNLEASH_SLOTS,
};
pub use content::{
    evaluate_content, BestSkillDamage, Content, ContentArea, ContentEvaluation, ContentRequirement,
    ContentSeries, GameRegion, RequirementCheck,
};
pub use content_evaluation::{
    evaluate_contents_for_character, DamageMaterial, DependencyCoefficients, SkillEvaluationInput,
    WristBonusMaterial,
};
pub use critical_rate::{
    critical_rate, CriticalRate, CriticalRateError, CriticalRateSourceId, CriticalRateSources,
    ARCHITECT_LAB_PER_STAGE, ARCHITECT_LAB_STAGE_MAX, CRITICAL_RATE_BONUS_MAX,
};
pub use damage::{
    calculate_damage, calculate_damage_with_combo, evaluate, ComboCycle, DamageContribution,
    DamageInput, DamageResult, DamageTrace, DamageTriple, DpsTriple, FormulaStep,
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
    armor_added_hp, equipment_attack_parts, equipment_attack_power, equipment_values_attack,
    sum_equipment_value_sources, weapon_added_damage, wrist_base_bonus, EnhanceGrade,
    EnhanceRates, Equipment, EquipmentAbilityAdditional, EquipmentAbilityAdditionalDef,
    EquipmentAbilityAdditionalKind, EquipmentAbilityDef, EquipmentAbilityFamily,
    EquipmentAttackLayer, EquipmentAttackPart, EquipmentAttackSource, EquipmentCatalogEntry,
    EquipmentCoefficients, EquipmentEnhanceType, EquipmentError, EquipmentPart,
    EquipmentPartList, EquipmentParts, EquipmentRates, EquipmentValueKind, EquipmentValueSource,
    EquipmentValues, PartEquipmentValues, PartSlot, PartSlotRule, SienaStatBonus, WristBonusRule,
    ENHANCE_LEVEL_MAX, ENHANCE_LEVEL_RANDOM_RANGE_MIN, EQUIPMENT_VALUE_MAX, WEAPON_ABILITY_SLOTS,
};
pub use mastery::{Masteries, MasteryCatalog, MasteryDef, MasteryError};
pub use random_option::{
    DependencyRates, RandomOptionDef, RandomOptionEffect, RandomOptionError, RandomOptionRank,
    RandomOptionSlot, RandomOptionTier, RandomOptionTotals, RANDOM_OPTION_VALUE_MAX,
};
pub use rounding::{floor_int, round_int, trunc2, trunc_int};
pub use siena::{
    siena_catalog, RegisteredSienaAura, SienaAura, SienaAuraList, SienaAuras, SienaCatalog,
    SienaError, SienaExtraKind, SienaExtraKindDef, SienaExtraSlot, SienaSlot, SienaValueKind,
    SienaValueKindDef, SIENA_EXTRA_UNLOCK_STAGES, SIENA_STAGE_MAX,
};
pub use skill::{
    ComboSkillType, ComboSkillTypeError, ComboSkillVariant, Skill, SkillDependency, SkillTarget,
};
pub use soul_link::{SoulLinkError, SoulLinkPreview, SoulLinkStatus};
pub use stat_sources::{
    apply_character_skills, apply_masteries, apply_pins, apply_temporary_adjustments,
    apply_unleash, buff_target_stat_gains, build_modifiers, contribution_source_effects,
    group_source_effects,
    preview_effective_stats, stat_limits,
    summarize_buff_selection, Adjustments, AttackPowerCoefficients, AttackPreview, BuffCatalog, BuffChoice,
    BuffDamageEffect, BuffDamageSummary, BuffDefinition, BuffOrigin, BuffPurpose, BuffSelection, BuffTarget,
    BuffTargetStatGain,
    BuffValue, CommonSkillPreview, CriticalRateBonusPreview, Crown, MonsterCards, PartAttackContribution,
    PetSkillTier, PetSkillTierBonus, PetSkills, RuneLevels, SacredRelic, StatAdjustment, StatContribution,
    StatGroupEffect, StatLayer, StatLimits, StatPreview, StatSourceError, StatSourceEffect,
    StatSourceGroup, StatSources, ThesisCoreRegionPreview,
    UltimateSkillPreview,
};
pub use stats::{
    effective_stat, effective_stats, BaseStats, BaseStatsError, EffectiveStats, StatKind,
    StatModifierSet, StatModifiers, StatTrace, BASE_STAT_MAX,
};
pub use thesis_core::{
    CoreRegion, CoreSet, CoreSetBonus, CoreSetGroup, CoreType, ThesisCore, ThesisCoreError,
    ThesisCores, CORE_ENHANCEMENT_MAX, CORE_EVOLUTION_MAX, CORE_SLOT_COUNT, POWER_BONUS,
    SUPPORT_BONUS,
};
pub use title::{
    title_added_damage_rate, title_attack_damage_rate, title_values, AddedDamageCondition,
    ConditionalAddedDamage, TitleDef, TitleError, TitleKind,
};
pub use ultimate_skill::{
    UltimateSkill, UltimateSkillError, UltimateSkills, HYPER_LIMIT_LEVEL_MAX, ULTIMATE_SKILL_SLOTS,
};
pub use validation::{ValidationError, ValidationLocation};
