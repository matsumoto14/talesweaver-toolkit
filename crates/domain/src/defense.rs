//! 防御側の戦闘能力値(docs/damage-formula.md §6)、カット率 J(§4 カテゴリJ)、回避(§7)。
//!
//! 与ダメージ式(`damage`)とは別の経路。ここで出すのは「自分がどれだけ耐えるか」で、
//! 攻撃力(A)と違って与ダメージには入らない。
//!
//! 回避の出典は wiki 計算式まとめ `#HitRate` / `#EvasionPoint` / `#HitRateCap`、
//! ステータス「命中率/回避率」(取得 2026-08-25)。
//!
//! 未実装で値を出せない項目は `Option` の `None` にする。0 と区別できないと
//! 「防御力 0」なのか「まだ計算していない」なのか画面で判断できないため。

use serde::{Deserialize, Serialize};

use crate::awakening::AwakeningCaps;
use crate::equipment::EquipmentValues;
use crate::rounding::floor_int;
use crate::stats::{EffectiveStats, StatKind};

/// 装備防御力倍率の初期値(wiki §6: 初期 100%。リンゴの島・ベリネンルミでは常に 100%)。
/// コートアーマー等による増加は未収録なのでこの値で固定する。
const EQUIPMENT_DEFENSE_RATE: f64 = 1.0;

/// カット率 J の分母定数(wiki カテゴリJ: `r = 1 − a/(a+80)`)。
const CUT_RATE_DENOMINATOR: f64 = 80.0;

/// 特殊回避(コンボ回避)の下限・上限 %(wiki §7)。
const COMBO_EVASION_MIN_PERCENT: f64 = 20.0;
const COMBO_EVASION_MAX_PERCENT: f64 = 63.0;

/// 回避P の定数項と AGI 係数(wiki `#EvasionPoint`:
/// `回避P = [15 + (AGI + 装備回避率)*1.2 + 装備敏捷度/7 + 回避P増加 + 攻撃タイプに応じた回避P増加]`)。
const EVASION_POINT_BASE: f64 = 15.0;
const EVASION_POINT_AGI_RATE: f64 = 1.2;
/// 攻撃タイプ別 回避P増加の共通除数(wiki `#EvasionPoint`)。
const EVASION_TYPE_DIVISOR: f64 = 7.0;
/// 物理の回避P増加に入る `[(STAB+HACK)/100]` の除数。
const EVASION_PHYSICAL_ATTACK_DIVISOR: f64 = 100.0;

/// 攻撃タイプ別の回避P(wiki `#EvasionPoint`)。敵の攻撃タイプに合わせた回避Pが要る。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvasionPoints {
    /// 物理。回避P増加 `(DEF*2 + [(STAB+HACK)/100]) / 7`
    pub physical: i64,
    /// 魔法。回避P増加 `(MR*2) / 7`
    pub magic: i64,
    /// 複合。回避P増加 `(DEF+MR) / 7`
    pub composite: i64,
}

/// 防御側の戦闘能力値一式。割合(カット率・回避)は小数表現(50% → 0.5)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DefenseProfile {
    /// 物理防御力 `[DEF*3 + 装備物防 * 倍率 * 6]`(上限適用後)
    pub physical_defense: i64,
    /// 魔法防御力 `[MR*3 + 装備魔防 * 倍率 * 6]`(上限適用後)
    pub magic_defense: i64,
    /// 複合防御力 `[(DEF+MR)*1.5 + (装備物防*倍率 + 装備魔防*倍率) * 3]`(上限適用後)
    pub composite_defense: i64,
    /// 防御力の上限(覚醒段階とエタの意志 Lv で決まる。wiki: Quest/覚醒クエスト・エタの意志)
    pub defense_cap: i64,
    /// 上限で捨てられた分(物理 / 魔法 / 複合)。すべて 0 なら上限に当たっていない
    pub physical_defense_loss: i64,
    pub magic_defense_loss: i64,
    pub composite_defense_loss: i64,
    /// カット率 J(物理)`r = 1 − a/(a+80)`、`a = 3 + [(DEF+装備物防−1)/10]`
    pub physical_cut_rate: f64,
    /// カット率 J(魔法)。`a` は MR 版
    pub magic_cut_rate: f64,
    /// カット率 J(複合)。`a = 3 + [(DEF+装備物防+MR+装備魔防−1)/20]`
    pub composite_cut_rate: f64,
    /// 特殊回避(コンボ回避)`(10 + MR/15 + AGI/7.5)%`、下限 20% / 上限 63%
    pub combo_evasion: f64,
    /// 攻撃タイプ別の回避P。通常回避「率」は敵の命中Pが要り、その入力(wiki 狩り場情報一覧の
    /// 「上限回避P」)が全行未記載なので出さない(ユーザー決定 2026-08-25)
    pub evasion_point: EvasionPoints,
    /// 装備物防(基本能力値 + 強化能力値の合計)
    pub equipment_physical_defense: i64,
    /// 装備魔防(基本能力値 + 強化能力値の合計)
    pub equipment_magic_defense: i64,
    /// 装備回避率補正(基本能力値 + 強化能力値の合計)
    pub equipment_evasion: i64,
    /// 装備敏捷度補正(基本能力値 + 強化能力値の合計)
    pub equipment_agility: i64,
}

/// カット率 J。`a` から `r = 1 − a/(a+80)`。
fn cut_rate(a: f64) -> f64 {
    1.0 - a / (a + CUT_RATE_DENOMINATOR)
}

/// カット率 J の `a`。`3 + [(合計 − 1) / 除数]`。
fn cut_rate_a(sum: i64, divisor: f64) -> f64 {
    3.0 + floor_int((sum - 1) as f64 / divisor) as f64
}

/// 回避P。`type_bonus` は攻撃タイプに応じた回避P増加。
///
/// 回避P増加(バフ)はバフカタログが「回避率+x%」のバフを持たないので 0。
fn evasion_point(stats: &EffectiveStats, equipment: &EquipmentValues, type_bonus: f64) -> i64 {
    floor_int(
        EVASION_POINT_BASE
            + (stats.agi + equipment.evasion) as f64 * EVASION_POINT_AGI_RATE
            + equipment.agility as f64 / EVASION_TYPE_DIVISOR
            + type_bonus,
    )
}

/// スキル依存種別ごとの命中P補正の係数(wiki 計算式まとめ の依存表「命中P補正」)。
/// 値は gamedata が持ち、ここは形と評価だけ。ペナルティは最大 2 ステの和(表のいちばん広い行が
/// `(INT+HACK)/250`)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccuracyCorrection {
    /// 依存ボーナス = `stat(kind) × rate`。ボーナスが無い依存は `None`
    pub bonus: Option<(StatKind, f64)>,
    /// 依存ペナルティ = `(stat(primary) + stat(secondary)) / divisor`
    pub penalty_primary: StatKind,
    pub penalty_secondary: Option<StatKind>,
    pub penalty_divisor: f64,
}

impl AccuracyCorrection {
    /// 依存ボーナス(切り捨て前)。
    pub fn bonus_value(&self, stats: &EffectiveStats) -> f64 {
        self.bonus.map_or(0.0, |(kind, rate)| stats.get(kind) as f64 * rate)
    }

    /// 依存ペナルティ(切り捨て前)。
    pub fn penalty_value(&self, stats: &EffectiveStats) -> f64 {
        let sum = stats.get(self.penalty_primary)
            + self.penalty_secondary.map_or(0, |k| stats.get(k));
        sum as f64 / self.penalty_divisor
    }
}

/// 命中P(wiki `#AccuracyPoint`)。
///
/// `命中P = [(DEX + 装備命中率補正 + スキル命中 + 依存ボーナス − 依存ペナルティ + 命中P増加)
///          × 命中P割合増加 × 命中P割合減少 + ランダムOP]`
///
/// 命中P増加(射手のルーン等のバフ)・割合増加(集中/的中剣)・割合減少(感電/雷電)・ランダムOP は
/// カタログに無いので中立値。スキル命中は wiki 表記 +15 済みの値を渡す。
pub fn accuracy_point(
    stats: &EffectiveStats,
    correction: &AccuracyCorrection,
    equipment_accuracy: i64,
    skill_accuracy: i64,
) -> i64 {
    stats.dex + equipment_accuracy + skill_accuracy + floor_int(correction.bonus_value(stats))
        - floor_int(correction.penalty_value(stats))
}

/// 防御側の戦闘能力値を出す。
///
/// `equipment` は装備補正 9 値の合計(基本 + 強化)。呼び出し側が `Equipment::base_totals` /
/// `enhanced_totals` を足して渡す(domain は gamedata のアビリティカタログを持たないため)。
/// `caps` は覚醒・エタの意志で開放される上限(表は gamedata)。防御力は上限に当たると
/// そこで頭打ちになり、以降の軽減はカット率 J が担う(wiki §6)。
pub fn defense_profile(
    stats: &EffectiveStats,
    equipment: &EquipmentValues,
    caps: AwakeningCaps,
) -> DefenseProfile {
    let def = stats.def as f64;
    let mr = stats.mr as f64;
    let eq_physical = equipment.physical_defense as f64 * EQUIPMENT_DEFENSE_RATE;
    let eq_magic = equipment.magic_defense as f64 * EQUIPMENT_DEFENSE_RATE;

    let combo_evasion_percent = (10.0 + mr / 15.0 + stats.agi as f64 / 7.5)
        .clamp(COMBO_EVASION_MIN_PERCENT, COMBO_EVASION_MAX_PERCENT);
    let combo_evasion = combo_evasion_percent / 100.0;

    let physical_type_bonus = (def * 2.0
        + floor_int((stats.stab + stats.hack) as f64 / EVASION_PHYSICAL_ATTACK_DIVISOR) as f64)
        / EVASION_TYPE_DIVISOR;

    let raw_physical = floor_int(def * 3.0 + eq_physical * 6.0);
    let raw_magic = floor_int(mr * 3.0 + eq_magic * 6.0);
    let raw_composite = floor_int((def + mr) * 1.5 + (eq_physical + eq_magic) * 3.0);
    let cap = |value: i64| value.min(caps.max_defense);

    DefenseProfile {
        physical_defense: cap(raw_physical),
        magic_defense: cap(raw_magic),
        composite_defense: cap(raw_composite),
        defense_cap: caps.max_defense,
        physical_defense_loss: raw_physical - cap(raw_physical),
        magic_defense_loss: raw_magic - cap(raw_magic),
        composite_defense_loss: raw_composite - cap(raw_composite),
        physical_cut_rate: cut_rate(cut_rate_a(stats.def + equipment.physical_defense, 10.0)),
        magic_cut_rate: cut_rate(cut_rate_a(stats.mr + equipment.magic_defense, 10.0)),
        composite_cut_rate: cut_rate(cut_rate_a(
            stats.def + equipment.physical_defense + stats.mr + equipment.magic_defense,
            20.0,
        )),
        combo_evasion,
        evasion_point: EvasionPoints {
            physical: evasion_point(stats, equipment, physical_type_bonus),
            magic: evasion_point(stats, equipment, mr * 2.0 / EVASION_TYPE_DIVISOR),
            composite: evasion_point(stats, equipment, (def + mr) / EVASION_TYPE_DIVISOR),
        },
        equipment_physical_defense: equipment.physical_defense,
        equipment_magic_defense: equipment.magic_defense,
        equipment_evasion: equipment.evasion,
        equipment_agility: equipment.agility,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(def: i64, mr: i64, agi: i64) -> EffectiveStats {
        EffectiveStats { def, mr, agi, ..Default::default() }
    }

    /// 上限に当たらない値(上限の挙動は専用テストで見る)
    fn no_caps() -> AwakeningCaps {
        AwakeningCaps { max_damage: i64::MAX, max_defense: i64::MAX, max_stat: i64::MAX }
    }

    #[test]
    fn 防御力はステ3倍と装備防御6倍() {
        let p = defense_profile(&stats(200, 150, 0), &EquipmentValues {
            physical_defense: 60,
            magic_defense: 40,
            ..Default::default()
        }, no_caps());
        assert_eq!(p.physical_defense, 960); // 200*3 + 60*6
        assert_eq!(p.magic_defense, 690); // 150*3 + 40*6
        // (200+150)*1.5 + (60 + 40)*3 = 525 + 300 = 825
        assert_eq!(p.composite_defense, 825);
        assert_eq!(p.equipment_physical_defense, 60);
        assert_eq!(p.equipment_magic_defense, 40);
    }

    #[test]
    fn カット率は1マイナスaを80足したaで割った値() {
        let p = defense_profile(&stats(200, 150, 0), &EquipmentValues::default(), no_caps());
        // a = 3 + [(200-1)/10] = 3 + 19 = 22 → 1 − 22/102
        assert!((p.physical_cut_rate - (1.0 - 22.0 / 102.0)).abs() < 1e-9);
        // a = 3 + [(150-1)/10] = 3 + 14 = 17 → 1 − 17/97
        assert!((p.magic_cut_rate - (1.0 - 17.0 / 97.0)).abs() < 1e-9);
        // a = 3 + [(200+150-1)/20] = 3 + 17 = 20 → 1 − 20/100
        assert!((p.composite_cut_rate - 0.8).abs() < 1e-9);
    }

    #[test]
    fn カット率の装備防御は生の値で足す() {
        let p = defense_profile(&stats(200, 150, 0), &EquipmentValues {
            physical_defense: 100,
            magic_defense: 50,
            ..Default::default()
        }, no_caps());
        // a = 3 + [(200+100-1)/10] = 3 + 29 = 32
        assert!((p.physical_cut_rate - (1.0 - 32.0 / 112.0)).abs() < 1e-9);
        // a = 3 + [(150+50-1)/10] = 3 + 19 = 22
        assert!((p.magic_cut_rate - (1.0 - 22.0 / 102.0)).abs() < 1e-9);
        // a = 3 + [(200+100+150+50-1)/20] = 3 + 24 = 27
        assert!((p.composite_cut_rate - (1.0 - 27.0 / 107.0)).abs() < 1e-9);
    }

    #[test]
    fn 特殊回避は下限20上限63に収まる() {
        // MR/AGI が 0 なら 10% → 下限 20%
        let zero = EquipmentValues::default();
        assert!((defense_profile(&stats(0, 0, 0), &zero, no_caps()).combo_evasion - 0.20).abs() < 1e-9);
        // 10 + 150/15 + 200/7.5 = 10 + 10 + 26.666.. = 46.666..%
        let p = defense_profile(&stats(0, 150, 200), &zero, no_caps());
        assert!((p.combo_evasion - 0.4666666666666667).abs() < 1e-9);
        // 上限 63%
        assert!((defense_profile(&stats(0, 310, 310), &zero, no_caps()).combo_evasion - 0.63).abs() < 1e-9);
    }

    #[test]
    fn 回避Pは15足すAGI1_2倍足す攻撃タイプ別増加() {
        // DEF200 / MR150 / AGI100、STAB+HACK は 0、装備なし
        let p = defense_profile(&stats(200, 150, 100), &EquipmentValues::default(), no_caps());
        // 物理: 15 + 120 + (400 + 0)/7 = 135 + 57.142.. = 192.14.. → 192
        assert_eq!(p.evasion_point.physical, 192);
        // 魔法: 15 + 120 + 300/7 = 135 + 42.857.. → 177
        assert_eq!(p.evasion_point.magic, 177);
        // 複合: 15 + 120 + 350/7 = 135 + 50 = 185
        assert_eq!(p.evasion_point.composite, 185);
    }

    #[test]
    fn 回避Pは装備回避率を1_2倍で装備敏捷度を7分の1で足す() {
        let p = defense_profile(&stats(200, 150, 100), &EquipmentValues {
            evasion: 50,
            agility: 70,
            ..Default::default()
        }, no_caps());
        // 複合: 15 + (100+50)*1.2 + 70/7 + 350/7 = 15 + 180 + 10 + 50 = 255
        assert_eq!(p.evasion_point.composite, 255);
        assert_eq!(p.equipment_evasion, 50);
        assert_eq!(p.equipment_agility, 70);
    }

    #[test]
    fn 物理の回避P増加は突き足す斬りを100で割って切捨ててから足す() {
        let s = EffectiveStats { def: 0, mr: 0, agi: 0, stab: 250, hack: 260, ..Default::default() };
        let p = defense_profile(&s, &EquipmentValues::default(), no_caps());
        // [(250+260)/100] = 5 → 15 + 0 + 5/7 = 15.714.. → 15
        assert_eq!(p.evasion_point.physical, 15);
    }
}
