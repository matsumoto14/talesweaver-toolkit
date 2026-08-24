//! 防御側の戦闘能力値(docs/damage-formula.md §6)、カット率 J(§4 カテゴリJ)、回避(§7)。
//!
//! 与ダメージ式(`damage`)とは別の経路。ここで出すのは「自分がどれだけ耐えるか」で、
//! 攻撃力(A)と違って与ダメージには入らない。
//!
//! 未実装で値を出せない項目は `Option` の `None` にする。0 と区別できないと
//! 「防御力 0」なのか「まだ計算していない」なのか画面で判断できないため。

use serde::{Deserialize, Serialize};

use crate::rounding::floor_int;
use crate::stats::EffectiveStats;

/// 装備防御力倍率の初期値(wiki §6: 初期 100%。リンゴの島・ベリネンルミでは常に 100%)。
/// コートアーマー等による増加は未収録なのでこの値で固定する。
const EQUIPMENT_DEFENSE_RATE: f64 = 1.0;

/// カット率 J の分母定数(wiki カテゴリJ: `r = 1 − a/(a+80)`)。
const CUT_RATE_DENOMINATOR: f64 = 80.0;

/// 特殊回避(コンボ回避)の下限・上限 %(wiki §7)。
const COMBO_EVASION_MIN_PERCENT: f64 = 20.0;
const COMBO_EVASION_MAX_PERCENT: f64 = 63.0;

/// 防御側の戦闘能力値一式。割合(カット率・回避)は小数表現(50% → 0.5)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DefenseProfile {
    /// 物理防御力 `[DEF*3 + 装備物防 * 倍率 * 6]`
    pub physical_defense: i64,
    /// 魔法防御力 `[MR*3 + 装備魔防 * 倍率 * 6]`
    pub magic_defense: i64,
    /// 複合防御力 `[(DEF+MR)*1.5 + (装備物防*倍率 + 装備魔防*倍率) * 3]`
    pub composite_defense: i64,
    /// カット率 J(物理)`r = 1 − a/(a+80)`、`a = 3 + [(DEF+装備物防−1)/10]`
    pub physical_cut_rate: f64,
    /// カット率 J(魔法)。`a` は MR 版
    pub magic_cut_rate: f64,
    /// カット率 J(複合)。`a = 3 + [(DEF+装備物防+MR+装備魔防−1)/20]` `[仮]`
    pub composite_cut_rate: f64,
    /// 特殊回避(コンボ回避)`(10 + MR/15 + AGI/7.5)%`、下限 20% / 上限 63%
    pub combo_evasion: f64,
    /// 通常回避。AGI 等からの算出式が未取込(wiki 計算式まとめ#HitRate)なので `None`
    pub normal_evasion: Option<f64>,
    /// 最終被弾率 `(1 − 通常回避) × (1 − 特殊回避)`。通常回避が未実装なので `None`
    pub hit_taken_rate: Option<f64>,
    /// 装備物防。装備モデル(`EquipmentValues`)が持たないので `None`。
    /// 物理・複合の防御力とカット率はこの分だけ下振れする
    pub equipment_physical_defense: Option<i64>,
    /// 装備魔防(基本能力値 + 強化能力値の合計)
    pub equipment_magic_defense: i64,
}

/// カット率 J。`a` から `r = 1 − a/(a+80)`。
fn cut_rate(a: f64) -> f64 {
    1.0 - a / (a + CUT_RATE_DENOMINATOR)
}

/// カット率 J の `a`。`3 + [(合計 − 1) / 除数]`。
fn cut_rate_a(sum: i64, divisor: f64) -> f64 {
    3.0 + floor_int((sum - 1) as f64 / divisor) as f64
}

/// 防御側の戦闘能力値を出す。
///
/// `equipment_magic_defense` は装備の魔法防御力の合計(基本 + 強化)。呼び出し側が
/// `Equipment::base_totals` / `enhanced_totals` から足して渡す(domain は gamedata の
/// アビリティカタログを持たないため)。装備物防は装備モデルに無いので 0 として扱う。
pub fn defense_profile(stats: &EffectiveStats, equipment_magic_defense: i64) -> DefenseProfile {
    let def = stats.def as f64;
    let mr = stats.mr as f64;
    let eq_magic = equipment_magic_defense as f64 * EQUIPMENT_DEFENSE_RATE;
    // 装備物防は未収録。0 のまま式に入れる(UI 側で「未実装」と示す)
    let eq_physical = 0.0;

    let combo_evasion_percent = (10.0 + mr / 15.0 + stats.agi as f64 / 7.5)
        .clamp(COMBO_EVASION_MIN_PERCENT, COMBO_EVASION_MAX_PERCENT);

    DefenseProfile {
        physical_defense: floor_int(def * 3.0 + eq_physical * 6.0),
        magic_defense: floor_int(mr * 3.0 + eq_magic * 6.0),
        composite_defense: floor_int((def + mr) * 1.5 + (eq_physical + eq_magic) * 3.0),
        physical_cut_rate: cut_rate(cut_rate_a(stats.def, 10.0)),
        magic_cut_rate: cut_rate(cut_rate_a(stats.mr + equipment_magic_defense, 10.0)),
        composite_cut_rate: cut_rate(cut_rate_a(stats.def + stats.mr + equipment_magic_defense, 20.0)),
        combo_evasion: combo_evasion_percent / 100.0,
        normal_evasion: None,
        hit_taken_rate: None,
        equipment_physical_defense: None,
        equipment_magic_defense,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(def: i64, mr: i64, agi: i64) -> EffectiveStats {
        EffectiveStats { def, mr, agi, ..Default::default() }
    }

    #[test]
    fn 防御力はステ3倍と装備魔防6倍() {
        let p = defense_profile(&stats(200, 150, 0), 40);
        assert_eq!(p.physical_defense, 600); // 200*3(装備物防は未収録で 0)
        assert_eq!(p.magic_defense, 690); // 150*3 + 40*6
        // (200+150)*1.5 + (0 + 40)*3 = 525 + 120 = 645
        assert_eq!(p.composite_defense, 645);
    }

    #[test]
    fn カット率は1マイナスaを80足したaで割った値() {
        let p = defense_profile(&stats(200, 150, 0), 0);
        // a = 3 + [(200-1)/10] = 3 + 19 = 22 → 1 − 22/102
        assert!((p.physical_cut_rate - (1.0 - 22.0 / 102.0)).abs() < 1e-9);
        // a = 3 + [(150-1)/10] = 3 + 14 = 17 → 1 − 17/97
        assert!((p.magic_cut_rate - (1.0 - 17.0 / 97.0)).abs() < 1e-9);
        // a = 3 + [(200+150-1)/20] = 3 + 17 = 20 → 1 − 20/100
        assert!((p.composite_cut_rate - 0.8).abs() < 1e-9);
    }

    #[test]
    fn 特殊回避は下限20上限63に収まる() {
        // MR/AGI が 0 なら 10% → 下限 20%
        assert!((defense_profile(&stats(0, 0, 0), 0).combo_evasion - 0.20).abs() < 1e-9);
        // 10 + 150/15 + 200/7.5 = 10 + 10 + 26.666.. = 46.666..%
        let p = defense_profile(&stats(0, 150, 200), 0);
        assert!((p.combo_evasion - 0.4666666666666667).abs() < 1e-9);
        // 上限 63%
        assert!((defense_profile(&stats(0, 310, 310), 0).combo_evasion - 0.63).abs() < 1e-9);
    }

    #[test]
    fn 未実装項目は0ではなくnone() {
        let p = defense_profile(&stats(200, 150, 100), 0);
        assert_eq!(p.normal_evasion, None);
        assert_eq!(p.hit_taken_rate, None);
        assert_eq!(p.equipment_physical_defense, None);
    }
}
