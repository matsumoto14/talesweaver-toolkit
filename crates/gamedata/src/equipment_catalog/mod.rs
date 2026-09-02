//! 装備カタログ(部位別アイテム・武器系統・装備強化倍率・武器アビリティ)。
//!
//! 出典: 装備システム / 装備システム/エンチャント / 装備システム/装備強化 / 装備システム/アビリティ /
//! `Link/装備Item` から辿れる武器・防具・アクセサリ各ページ(取得 2026-08-27)。
//! docs/claude/goals/2026-08-24-equipment-parts.md「インファーナルより上位の全装備カタログ」節参照。

use domain::{
    BaseStats, DamageCategory, EnhanceGrade, EnhanceRates, Equipment,
    EquipmentAbilityAdditionalDef, EquipmentAbilityAdditionalKind, EquipmentAbilityDef,
    EquipmentAbilityFamily, EquipmentEnhanceType, EquipmentValues, PartSlot, SkillDependency,
    SkillEffect,
};
/// 装備の分類は domain のドメイン語彙(`domain::equipment_class`)。カタログはそれを
/// 各アイテムに付けるだけで、判定は domain 側が持つ。
pub use domain::{ArmorClass, RelicInfo, RelicKind, WeaponClass, WeaponSystem, WristType};

use crate::Source;

mod abilities;
mod enhance;
mod items;
#[cfg(test)]
mod tests;

pub use abilities::*;
pub use enhance::*;
pub use items::*;
