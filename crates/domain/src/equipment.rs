//! 装備補正(wiki: カテゴリA の内訳「装備攻撃力」)。docs/damage-formula.md §4 A。
//!
//! 装備品を部位ごとに登録するのではなく、ゲーム内ステータス画面に表示される
//! 「基本能力値」「強化能力値」の合計値のみを持つ(docs/claude/goals/2026-08-22-equipment-attack.md)。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 装備補正 4 種(突き/斬り/魔攻/魔防)。基本能力値・強化能力値のどちらも同じ形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EquipmentValues {
    #[serde(default)]
    pub thrust: i64,
    #[serde(default)]
    pub slash: i64,
    #[serde(default)]
    pub magic_attack: i64,
    #[serde(default)]
    pub magic_defense: i64,
}

/// 装備補正 4 値の値域上限(wiki に明記なし。実用上の安全域として暫定採用)`[仮]`。
pub const EQUIPMENT_VALUE_MAX: i64 = 9999;
/// ストロングウェポンの Lv 上限(wiki Skill/共通: Lv1〜6)。
pub const STRONG_WEAPON_LEVEL_MAX: u8 = 6;

impl EquipmentValues {
    fn validate(&self) -> Result<(), EquipmentError> {
        for (field, value) in [
            ("突き攻撃力", self.thrust),
            ("斬り攻撃力", self.slash),
            ("魔法攻撃力", self.magic_attack),
            ("魔法防御力", self.magic_defense),
        ] {
            if !(0..=EQUIPMENT_VALUE_MAX).contains(&value) {
                return Err(EquipmentError::ValueOutOfRange { field, value, max: EQUIPMENT_VALUE_MAX });
            }
        }
        Ok(())
    }
}

/// 装備補正の値域違反。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum EquipmentError {
    #[error("装備補正の{field}は 0〜{max} の範囲で指定してください(指定値 {value})")]
    ValueOutOfRange { field: &'static str, value: i64, max: i64 },
    #[error("ストロングウェポンの Lv は 0〜{max} です(指定値 {value})")]
    StrongWeaponLevelOutOfRange { value: u8, max: u8 },
}

/// キャラの装備補正一式(基本能力値/強化能力値/装備攻撃力強化バフ)。
///
/// 「基本能力値」= エンチャント値を除いた素の装備補正・キャラスキルによる値・アビリティ。
/// 「強化能力値」= エンチャント・テシスコア・アバター強化剤等の期間制能力値・シエナのオーラ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Equipment {
    #[serde(default)]
    pub base: EquipmentValues,
    #[serde(default)]
    pub enhanced: EquipmentValues,
    /// パワーウェポン(wiki Skill/共通: 自身の装備補正を2%増加。Lv1 のみ、ストロングウェポンと重複可)
    #[serde(default)]
    pub power_weapon: bool,
    /// ストロングウェポンの Lv(0 = 未使用、1〜6 = 該当 Lv。wiki Skill/共通: 3/6/9/12/15/18%)
    #[serde(default)]
    pub strong_weapon_level: u8,
}

impl Equipment {
    pub fn validate(&self) -> Result<(), EquipmentError> {
        self.base.validate()?;
        self.enhanced.validate()?;
        if self.strong_weapon_level > STRONG_WEAPON_LEVEL_MAX {
            return Err(EquipmentError::StrongWeaponLevelOutOfRange {
                value: self.strong_weapon_level,
                max: STRONG_WEAPON_LEVEL_MAX,
            });
        }
        Ok(())
    }

    /// 装備攻撃力強化倍率(wiki: カテゴリA の内訳)。
    /// パワーウェポン(+2%)+ ストロングウェポン Lv × 3%。
    pub fn enhance_rate(&self) -> f64 {
        let power_weapon_rate = if self.power_weapon { 0.02 } else { 0.0 };
        power_weapon_rate + f64::from(self.strong_weapon_level) * 0.03
    }
}

/// 装備補正 4 種それぞれに掛かる係数(基本/強化のどちらか片方)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentRates {
    pub thrust: f64,
    pub slash: f64,
    pub magic_attack: f64,
    pub magic_defense: f64,
}

/// 装備攻撃力の係数(基本能力値用/強化能力値用)。スキル依存種別ごとに gamedata が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentCoefficients {
    pub base: EquipmentRates,
    pub enhanced: EquipmentRates,
}

/// 装備攻撃力(wiki: カテゴリA の内訳)。`Σ(基本値 × 基本係数) + Σ(強化値 × 強化係数)`。
pub fn equipment_attack_power(eq: &Equipment, c: &EquipmentCoefficients) -> f64 {
    eq.base.thrust as f64 * c.base.thrust
        + eq.base.slash as f64 * c.base.slash
        + eq.base.magic_attack as f64 * c.base.magic_attack
        + eq.base.magic_defense as f64 * c.base.magic_defense
        + eq.enhanced.thrust as f64 * c.enhanced.thrust
        + eq.enhanced.slash as f64 * c.enhanced.slash
        + eq.enhanced.magic_attack as f64 * c.enhanced.magic_attack
        + eq.enhanced.magic_defense as f64 * c.enhanced.magic_defense
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coefficients() -> EquipmentCoefficients {
        EquipmentCoefficients {
            base: EquipmentRates { thrust: 14.5, slash: 14.5, magic_attack: 0.0, magic_defense: 0.0 },
            enhanced: EquipmentRates { thrust: 28.75, slash: 28.75, magic_attack: 0.0, magic_defense: 0.0 },
        }
    }

    #[test]
    fn 装備攻撃力は基本と強化の合計() {
        let eq = Equipment {
            base: EquipmentValues { thrust: 150, slash: 150, ..Default::default() },
            enhanced: EquipmentValues { thrust: 60, slash: 60, ..Default::default() },
            ..Default::default()
        };
        // 150*14.5*2 + 60*28.75*2 = 4350 + 3450 = 7800
        assert!((equipment_attack_power(&eq, &coefficients()) - 7800.0).abs() < 1e-9);
    }

    #[test]
    fn 装備なしなら装備攻撃力は0() {
        assert_eq!(equipment_attack_power(&Equipment::default(), &coefficients()), 0.0);
    }

    #[test]
    fn 強化倍率はパワーウェポンとストロングウェポンの合計() {
        assert_eq!(Equipment::default().enhance_rate(), 0.0);
        let pw = Equipment { power_weapon: true, ..Default::default() };
        assert!((pw.enhance_rate() - 0.02).abs() < 1e-12);
        let sw6 = Equipment { strong_weapon_level: 6, ..Default::default() };
        assert!((sw6.enhance_rate() - 0.18).abs() < 1e-12);
        let both = Equipment { power_weapon: true, strong_weapon_level: 6, ..Default::default() };
        assert!((both.enhance_rate() - 0.20).abs() < 1e-12);
    }

    #[test]
    fn 値域違反は拒否する() {
        let mut eq = Equipment::default();
        eq.base.thrust = EQUIPMENT_VALUE_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.enhanced.magic_defense = -1;
        assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::StrongWeaponLevelOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.base.thrust = EQUIPMENT_VALUE_MAX;
        eq.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX;
        assert!(eq.validate().is_ok());
    }
}
