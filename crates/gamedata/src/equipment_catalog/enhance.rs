//! 武器種と装備強化(wiki: 装備システム/装備強化)。強化 Lv・等級から倍率を引き、
//! 武器系統・鎧種別ごとの補正式を返す。**カタログを引かない**表だけを置く
//! (アイテム id からの解決は `items.rs`)。

use super::*;

/// 装備強化(装備システム/装備強化)の出典。
pub const ENHANCE_SOURCE: Source = Source {
    page: "装備システム/装備強化",
    retrieved_on: "2026-08-24",
    note: "武器系統ごとの補正式・倍率表。+12以上はレンジ内ランダム(MR)",
};

/// 武器系統ごとの強化補正式の係数(wiki: 装備システム/装備強化)。
/// 装着アビリティによる補正は含めない(補正は Item の実測 base 値のみで算出する)。
pub fn enhance_rates(class: WeaponClass) -> EnhanceRates {
    match class.system() {
        // 突き攻撃力 x 6.67 + 斬り攻撃力 x 1.00
        WeaponSystem::Stab => EnhanceRates {
            thrust: 6.67,
            slash: 1.00,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        // 突き攻撃力 x 4.55 + 斬り攻撃力 x 4.55
        WeaponSystem::StabHack => EnhanceRates {
            thrust: 4.55,
            slash: 4.55,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        // 斬り攻撃力 x 6.67 + 突き攻撃力 x 1.00
        WeaponSystem::Hack => EnhanceRates {
            thrust: 1.00,
            slash: 6.67,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        // 魔法攻撃力 x 6.95 + 魔法防御力 x 1.05
        WeaponSystem::Int => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 6.95,
            magic_defense: 1.05,
        },
        // 魔法攻撃力 x 4.55 + 斬り攻撃力 x 3.85
        WeaponSystem::IntHack => EnhanceRates {
            thrust: 0.0,
            slash: 3.85,
            magic_attack: 4.55,
            magic_defense: 0.0,
        },
        // 魔法防御力 x 7.70 + 魔法攻撃力 x 0.70
        WeaponSystem::Mr => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 0.70,
            magic_defense: 7.70,
        },
    }
}

pub fn enhance_rates_for_type(kind: EquipmentEnhanceType) -> Option<EnhanceRates> {
    Some(match kind {
        EquipmentEnhanceType::WeaponStab => EnhanceRates {
            thrust: 6.67,
            slash: 1.00,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponStabHack => EnhanceRates {
            thrust: 4.55,
            slash: 4.55,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponHack => EnhanceRates {
            thrust: 1.00,
            slash: 6.67,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponInt => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 6.95,
            magic_defense: 1.05,
        },
        EquipmentEnhanceType::WeaponIntHack => EnhanceRates {
            thrust: 0.0,
            slash: 3.85,
            magic_attack: 4.55,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponMr => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 0.70,
            magic_defense: 7.70,
        },
        _ => return None,
    })
}

/// 装備強化 +1〜+11 の確定倍率(wiki: 装備システム/装備強化)。範囲外は `None`。
pub fn enhance_multiplier(level: u8) -> Option<f64> {
    match level {
        1 => Some(0.4),
        2 => Some(1.0),
        3 => Some(1.8),
        4 => Some(3.0),
        5 => Some(4.6),
        6 => Some(6.8),
        7 => Some(9.6),
        8 => Some(14.2),
        9 => Some(20.6),
        10 => Some(28.8),
        11 => Some(40.0),
        _ => None,
    }
}

/// 装備強化 +12〜+15 のレンジ倍率(wiki: 装備システム/装備強化。MR で振り直し可)。範囲外は `None`。
pub fn enhance_multiplier_range(level: u8) -> Option<(f64, f64)> {
    match level {
        12 => Some((140.0, 280.0)),
        13 => Some((300.0, 460.0)),
        14 => Some((480.0, 660.0)),
        15 => Some((680.0, 880.0)),
        _ => None,
    }
}

/// 確率区分の上端に対応する倍率。倍率は整数へ四捨五入する。
pub fn enhance_grade_multiplier(level: u8, grade: EnhanceGrade) -> Option<f64> {
    let (min, max) = enhance_multiplier_range(level)?;
    Some(domain::round_int(min + (max - min) * grade.percentile()) as f64)
}

pub fn armor_enhance_multiplier(level: u8, grade: Option<EnhanceGrade>) -> Option<f64> {
    enhance_multiplier(level)
        .or_else(|| grade.and_then(|g| enhance_grade_multiplier(level, g)))
        .map(|v| v / 2.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmorEnhanceRates {
    pub physical_defense: f64,
    pub magic_defense: f64,
}

pub fn armor_enhance_rates(class: ArmorClass) -> ArmorEnhanceRates {
    match class {
        ArmorClass::Light => ArmorEnhanceRates {
            physical_defense: 3.90,
            magic_defense: 4.00,
        },
        ArmorClass::Heavy => ArmorEnhanceRates {
            physical_defense: 3.10,
            magic_defense: 3.80,
        },
        ArmorClass::Magic => ArmorEnhanceRates {
            physical_defense: 3.80,
            magic_defense: 4.00,
        },
        ArmorClass::Suit => ArmorEnhanceRates {
            physical_defense: 7.80,
            magic_defense: 0.00,
        },
        ArmorClass::Robe => ArmorEnhanceRates {
            physical_defense: 4.00,
            magic_defense: 3.80,
        },
    }
}

pub fn armor_class_for_type(kind: EquipmentEnhanceType) -> Option<ArmorClass> {
    match kind {
        EquipmentEnhanceType::ArmorLight => Some(ArmorClass::Light),
        EquipmentEnhanceType::ArmorHeavy => Some(ArmorClass::Heavy),
        EquipmentEnhanceType::ArmorMagic => Some(ArmorClass::Magic),
        EquipmentEnhanceType::ArmorSuit => Some(ArmorClass::Suit),
        EquipmentEnhanceType::ArmorRobe => Some(ArmorClass::Robe),
        _ => None,
    }
}
