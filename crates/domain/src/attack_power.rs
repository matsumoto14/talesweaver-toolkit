//! 攻撃力(wiki: カテゴリA)と攻撃力乱数部分(カテゴリB)の計算。docs/damage-formula.md §4。

use serde::{Deserialize, Serialize};

use crate::rounding::{floor_int, trunc2};
use crate::stats::{EffectiveStats, StatKind};

/// ステ由来攻撃力の係数。`primary.1 * stat(primary.0) + secondary.1 * stat(secondary.0)`。
/// 値はスキル依存種別ごとに gamedata が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttackCoefficients {
    pub primary: (StatKind, f64),
    pub secondary: (StatKind, f64),
}

/// ステ由来攻撃力(切捨て前)。
pub fn stat_attack_power(stats: &EffectiveStats, c: &AttackCoefficients) -> f64 {
    stats.get(c.primary.0) as f64 * c.primary.1 + stats.get(c.secondary.0) as f64 * c.secondary.1
}

/// 攻撃力(wiki: カテゴリA)。
///
/// `[ステ攻撃力 + 装備攻撃力] + [装備攻撃力/25 * 装備補正強化係数] * 25`
pub fn attack_power(stat_attack: f64, equipment_attack: f64, equipment_enhance_rate: f64) -> i64 {
    floor_int(stat_attack + equipment_attack)
        + floor_int(equipment_attack / 25.0 * equipment_enhance_rate) * 25
}

/// 攻撃力(A)の内訳。`attack_power` と同じ経路で作る(計算を二重に書かない)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttackPowerBreakdown {
    /// ステ由来攻撃力(切捨て前)
    pub stat_attack: f64,
    /// 装備の基本能力値に係数を掛けた分
    pub equipment_base_attack: f64,
    /// 装備の強化能力値(エンチャント + シエナのオーラ + テシスコア)に係数を掛けた分
    pub equipment_enhanced_attack: f64,
    /// 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン)
    pub enhance_rate: f64,
    /// 攻撃力(A)
    pub value: i64,
}

impl AttackPowerBreakdown {
    /// 装備攻撃力(基本 + 強化)。
    pub fn equipment_attack(&self) -> f64 {
        self.equipment_base_attack + self.equipment_enhanced_attack
    }
}

/// 攻撃力(A)を内訳付きで出す。`attack_power` の唯一の呼び出し口にする。
pub fn attack_power_breakdown(
    stat_attack: f64,
    equipment_base_attack: f64,
    equipment_enhanced_attack: f64,
    enhance_rate: f64,
) -> AttackPowerBreakdown {
    let equipment_attack = equipment_base_attack + equipment_enhanced_attack;
    AttackPowerBreakdown {
        stat_attack,
        equipment_base_attack,
        equipment_enhanced_attack,
        enhance_rate,
        value: attack_power(stat_attack, equipment_attack, enhance_rate),
    }
}

/// 攻撃力乱数部分の最大値(wiki: カテゴリB)。最小は 1。
///
/// `{(ステ由来攻撃力 + DEX*3)/18} + 1`
pub fn random_part_max(stat_attack: f64, dex: i64) -> f64 {
    trunc2((stat_attack + dex as f64 * 3.0) / 18.0) + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ステ由来攻撃力は係数の線形和() {
        let stats = EffectiveStats { stab: 100, hack: 200, ..Default::default() };
        let c = AttackCoefficients { primary: (StatKind::Stab, 2.1), secondary: (StatKind::Hack, 1.08) };
        // 100 * 2.1 + 200 * 1.08 = 210 + 216 = 426
        assert!((stat_attack_power(&stats, &c) - 426.0).abs() < 1e-9);
    }

    #[test]
    fn 攻撃力は装備なしならステ由来攻撃力の切捨て() {
        assert_eq!(attack_power(426.7, 0.0, 0.0), 426);
    }

    #[test]
    fn 攻撃力は装備補正強化係数を25刻みで加算する() {
        // [400.5 + 100] = 500、[100/25 * 0.05 = 0.2] = 0 → 500
        assert_eq!(attack_power(400.5, 100.0, 0.05), 500);
        // [100/25 * 0.5 = 2] * 25 = 50 → 550
        assert_eq!(attack_power(400.5, 100.0, 0.5), 550);
    }

    #[test]
    fn 乱数部分最大は小数2位切捨てプラス1() {
        // (426 + 50*3)/18 = 32.0 → 33.0
        assert!((random_part_max(426.0, 50) - 33.0).abs() < 1e-9);
        // (100 + 0)/18 = 5.555.. → 5.55 + 1 = 6.55
        assert!((random_part_max(100.0, 0) - 6.55).abs() < 1e-9);
    }
}
