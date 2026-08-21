//! ステータス(素ステ 7 種)と能力値(実効ステータス)の計算。docs/damage-formula.md §1・§2。

use serde::{Deserialize, Serialize};

use crate::rounding::floor_int;

/// ステータスの種別(wiki §1 の 7 種)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatKind {
    Stab,
    Hack,
    Int,
    Def,
    Mr,
    Dex,
    Agi,
}

impl StatKind {
    pub const ALL: [StatKind; 7] = [
        StatKind::Stab,
        StatKind::Hack,
        StatKind::Int,
        StatKind::Def,
        StatKind::Mr,
        StatKind::Dex,
        StatKind::Agi,
    ];
}

/// 素ステータス(オリジナル)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BaseStats {
    pub stab: u32,
    pub hack: u32,
    pub int: u32,
    pub def: u32,
    pub mr: u32,
    pub dex: u32,
    pub agi: u32,
}

impl BaseStats {
    pub fn get(&self, kind: StatKind) -> u32 {
        match kind {
            StatKind::Stab => self.stab,
            StatKind::Hack => self.hack,
            StatKind::Int => self.int,
            StatKind::Def => self.def,
            StatKind::Mr => self.mr,
            StatKind::Dex => self.dex,
            StatKind::Agi => self.agi,
        }
    }
}

/// 1 つのステータスに掛かる補正の 5 レイヤー(wiki §2)。
///
/// ```text
/// 基本能力値 = [(素ステ + Σ[素ステ * 割合増加] + 固定値) * Π倍率A]
/// 最終能力値 = 基本能力値 + [基本能力値 * 倍率B] + 最終固定値
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatModifiers {
    /// 割合増加(素ステ比)。バフごとに切捨ててから加算する。0.1 = +10%
    pub percent_of_base: Vec<f64>,
    /// 固定値増加/減少
    pub fixed: i64,
    /// 能力値倍率A。乗算で重なる(1.1 = 1.1倍)
    pub multiplier_a: Vec<f64>,
    /// 能力値倍率B。初期 0、下限 -0.30
    pub multiplier_b: f64,
    /// 最終固定値増加/減少
    pub final_fixed: i64,
}

impl StatModifiers {
    /// 補正なし(素ステがそのまま最終能力値になる)。
    pub fn neutral() -> Self {
        Self {
            percent_of_base: Vec::new(),
            fixed: 0,
            multiplier_a: Vec::new(),
            multiplier_b: 0.0,
            final_fixed: 0,
        }
    }
}

impl Default for StatModifiers {
    fn default() -> Self {
        Self::neutral()
    }
}

/// 倍率B の下限(wiki §2)。
const MULTIPLIER_B_MIN: f64 = -0.30;

/// 能力値計算の中間値。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatTrace {
    pub kind: StatKind,
    pub base: u32,
    /// Σ[素ステ * 割合増加]
    pub percent_of_base_total: i64,
    pub fixed: i64,
    /// Π倍率A
    pub multiplier_a: f64,
    /// 基本能力値
    pub basic: i64,
    /// 下限適用後の倍率B
    pub multiplier_b: f64,
    /// [基本能力値 * 倍率B]
    pub multiplier_b_bonus: i64,
    pub final_fixed: i64,
    /// 最終能力値
    pub effective: i64,
}

/// 1 ステータス分の能力値計算(wiki §2)。
pub fn effective_stat(kind: StatKind, base: u32, m: &StatModifiers) -> (i64, StatTrace) {
    let percent_of_base_total: i64 = m
        .percent_of_base
        .iter()
        .map(|rate| floor_int(f64::from(base) * rate))
        .sum();
    let multiplier_a: f64 = m.multiplier_a.iter().product();
    let before_multiplier = i64::from(base) + percent_of_base_total + m.fixed;
    let basic = floor_int(before_multiplier as f64 * multiplier_a);
    let multiplier_b = m.multiplier_b.max(MULTIPLIER_B_MIN);
    let multiplier_b_bonus = floor_int(basic as f64 * multiplier_b);
    let effective = basic + multiplier_b_bonus + m.final_fixed;
    let trace = StatTrace {
        kind,
        base,
        percent_of_base_total,
        fixed: m.fixed,
        multiplier_a,
        basic,
        multiplier_b,
        multiplier_b_bonus,
        final_fixed: m.final_fixed,
        effective,
    };
    (effective, trace)
}

/// 7 ステータスそれぞれの補正。`Default` が中立(補正なし)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StatModifierSet {
    pub stab: StatModifiers,
    pub hack: StatModifiers,
    pub int: StatModifiers,
    pub def: StatModifiers,
    pub mr: StatModifiers,
    pub dex: StatModifiers,
    pub agi: StatModifiers,
}

impl StatModifierSet {
    pub fn get(&self, kind: StatKind) -> &StatModifiers {
        match kind {
            StatKind::Stab => &self.stab,
            StatKind::Hack => &self.hack,
            StatKind::Int => &self.int,
            StatKind::Def => &self.def,
            StatKind::Mr => &self.mr,
            StatKind::Dex => &self.dex,
            StatKind::Agi => &self.agi,
        }
    }
}

/// 最終能力値(実効ステータス)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EffectiveStats {
    pub stab: i64,
    pub hack: i64,
    pub int: i64,
    pub def: i64,
    pub mr: i64,
    pub dex: i64,
    pub agi: i64,
}

impl EffectiveStats {
    pub fn get(&self, kind: StatKind) -> i64 {
        match kind {
            StatKind::Stab => self.stab,
            StatKind::Hack => self.hack,
            StatKind::Int => self.int,
            StatKind::Def => self.def,
            StatKind::Mr => self.mr,
            StatKind::Dex => self.dex,
            StatKind::Agi => self.agi,
        }
    }

    fn set(&mut self, kind: StatKind, value: i64) {
        match kind {
            StatKind::Stab => self.stab = value,
            StatKind::Hack => self.hack = value,
            StatKind::Int => self.int = value,
            StatKind::Def => self.def = value,
            StatKind::Mr => self.mr = value,
            StatKind::Dex => self.dex = value,
            StatKind::Agi => self.agi = value,
        }
    }
}

/// 7 ステータスすべての能力値計算。トレースは `StatKind::ALL` の順。
pub fn effective_stats(
    base: &BaseStats,
    modifiers: &StatModifierSet,
) -> (EffectiveStats, Vec<StatTrace>) {
    let mut stats = EffectiveStats::default();
    let mut traces = Vec::with_capacity(StatKind::ALL.len());
    for kind in StatKind::ALL {
        let (value, trace) = effective_stat(kind, base.get(kind), modifiers.get(kind));
        stats.set(kind, value);
        traces.push(trace);
    }
    (stats, traces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(base: u32, m: &StatModifiers) -> i64 {
        effective_stat(StatKind::Stab, base, m).0
    }

    #[test]
    fn 中立補正では素ステがそのまま最終能力値になる() {
        let m = StatModifiers::neutral();
        assert_eq!(stat(0, &m), 0);
        assert_eq!(stat(1, &m), 1);
        assert_eq!(stat(777, &m), 777);
        assert_eq!(stat(1500, &m), 1500);
    }

    #[test]
    fn 割合増加はバフごとに切捨ててから加算する() {
        // 335 * 0.1 = 33.5 → 33、335 * 0.05 = 16.75 → 16。合計 49
        // (まとめて 15% なら 50.25 → 50 になるので、バフごと切捨ての差が出る)
        let m = StatModifiers { percent_of_base: vec![0.1, 0.05], ..StatModifiers::neutral() };
        assert_eq!(stat(335, &m), 335 + 33 + 16);
    }

    #[test]
    fn 固定値は倍率aの前に加算される() {
        let m = StatModifiers { fixed: 50, multiplier_a: vec![1.1], ..StatModifiers::neutral() };
        // (100 + 50) * 1.1 = 165
        assert_eq!(stat(100, &m), 165);
    }

    #[test]
    fn 倍率aは乗算で重なり結果を切捨てる() {
        let m = StatModifiers { multiplier_a: vec![1.1, 1.1], ..StatModifiers::neutral() };
        // 101 * 1.21 = 122.21 → 122
        assert_eq!(stat(101, &m), 122);
    }

    #[test]
    fn 倍率bは基本能力値に掛けて切捨て下限はマイナス30パーセント() {
        let m = StatModifiers { multiplier_b: 0.25, ..StatModifiers::neutral() };
        // 101 + [101 * 0.25 = 25.25] = 126
        assert_eq!(stat(101, &m), 126);
        let m = StatModifiers { multiplier_b: -0.5, ..StatModifiers::neutral() };
        // 下限 -0.30: 100 + [100 * -0.3] = 70
        assert_eq!(stat(100, &m), 70);
    }

    #[test]
    fn 最終固定値は最後に加算される() {
        let m = StatModifiers { multiplier_a: vec![2.0], final_fixed: -7, ..StatModifiers::neutral() };
        assert_eq!(stat(10, &m), 13);
    }

    #[test]
    fn 全レイヤー複合() {
        let m = StatModifiers {
            percent_of_base: vec![0.2],
            fixed: 10,
            multiplier_a: vec![1.1],
            multiplier_b: 0.1,
            final_fixed: 5,
        };
        // 基本 = [(200 + 40 + 10) * 1.1] = [275.0] = 275
        // 最終 = 275 + [27.5] + 5 = 307
        let (value, trace) = effective_stat(StatKind::Int, 200, &m);
        assert_eq!(value, 307);
        assert_eq!(trace.basic, 275);
        assert_eq!(trace.multiplier_b_bonus, 27);
        assert_eq!(trace.kind, StatKind::Int);
    }

    #[test]
    fn 七種すべてを計算しトレースを返す() {
        let base = BaseStats { stab: 1, hack: 2, int: 3, def: 4, mr: 5, dex: 6, agi: 7 };
        let (stats, traces) = effective_stats(&base, &StatModifierSet::default());
        assert_eq!(stats, EffectiveStats { stab: 1, hack: 2, int: 3, def: 4, mr: 5, dex: 6, agi: 7 });
        assert_eq!(traces.len(), 7);
        for kind in StatKind::ALL {
            assert_eq!(stats.get(kind), i64::from(base.get(kind)));
        }
    }
}
