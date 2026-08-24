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

/// 回避P の定数項と AGI 係数(wiki `#EvasionPoint`:
/// `回避P = [15 + (AGI + 装備回避率)*1.2 + 装備敏捷度/7 + 回避P増加 + 攻撃タイプに応じた回避P増加]`)。
const EVASION_POINT_BASE: f64 = 15.0;
const EVASION_POINT_AGI_RATE: f64 = 1.2;
/// 攻撃タイプ別 回避P増加の共通除数(wiki `#EvasionPoint`)。
const EVASION_TYPE_DIVISOR: f64 = 7.0;
/// 物理の回避P増加に入る `[(STAB+HACK)/100]` の除数。
const EVASION_PHYSICAL_ATTACK_DIVISOR: f64 = 100.0;

/// 命中率の下限 %(wiki `#HitRateCap`: `15 + 攻撃者の最小命中率補正 − 対象の最小回避率補正`。
/// 最小命中率/最小回避率の補正は未収録なので定数 15 のまま)。
const HIT_RATE_MIN_PERCENT: f64 = 15.0;
/// 命中率の上限 %(wiki `#HitRateCap`: モンスターが行う攻撃は 100)。
const HIT_RATE_MAX_PERCENT: f64 = 100.0;

/// 通常回避の上限(wiki ステータス「命中率/回避率」: 基本上限 85% = 命中率下限 15% の裏返し)。
pub const NORMAL_EVASION_CAP: f64 = 1.0 - HIT_RATE_MIN_PERCENT / 100.0;

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
    /// 攻撃タイプ別の回避P。通常回避率は敵の命中Pとの差で決まるので、能力値だけで
    /// 出せるのはここまで(`通常回避 = 1 − (敵命中P − 回避P)/100`、下限 0% / 上限 85%)
    pub evasion_point: EvasionPoints,
    /// 通常回避の上限(85%)。敵命中P ≦ 回避P + 15 のときこの値になる
    pub normal_evasion_cap: f64,
    /// 上限回避時の最終被弾率 `(1 − 85%) × (1 − 特殊回避)`。
    /// 敵命中Pが上がるほどこの値より悪化する(上限は `1 − 特殊回避`)
    pub hit_taken_rate_at_cap: f64,
    /// 装備物防。装備モデル(`EquipmentValues`)が持たないので `None`。
    /// 物理・複合の防御力とカット率はこの分だけ下振れする
    pub equipment_physical_defense: Option<i64>,
    /// 装備魔防(基本能力値 + 強化能力値の合計)
    pub equipment_magic_defense: i64,
    /// 装備回避率・装備敏捷度。装備モデルが持たないので `None`。回避Pはこの分だけ下振れする
    pub equipment_evasion: Option<i64>,
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
/// 装備回避率・装備敏捷度・回避P増加(バフ)は装備モデル/バフカタログが持たないので 0。
fn evasion_point(stats: &EffectiveStats, type_bonus: f64) -> i64 {
    floor_int(EVASION_POINT_BASE + stats.agi as f64 * EVASION_POINT_AGI_RATE + type_bonus)
}

/// 通常回避率(wiki `#HitRate` + `#HitRateCap`)。
///
/// `命中率 = 敵命中P − 回避P`(下限 15% / 上限 100%)の裏返しなので、
/// 通常回避は下限 0% / 上限 85%。敵の命中Pはコンテンツごとに異なる
/// (wiki 狩り場情報一覧「上限回避P」= 敵命中P − 15。現状すべて未記載)。
pub fn normal_evasion(evasion_point: i64, enemy_accuracy_point: i64) -> f64 {
    let hit_rate =
        ((enemy_accuracy_point - evasion_point) as f64).clamp(HIT_RATE_MIN_PERCENT, HIT_RATE_MAX_PERCENT);
    1.0 - hit_rate / 100.0
}

/// 最終被弾率 `(1 − 通常回避) × (1 − 特殊回避)`(wiki ステータス「命中率/回避率」の例と同じ合成)。
pub fn hit_taken_rate(normal_evasion: f64, combo_evasion: f64) -> f64 {
    (1.0 - normal_evasion) * (1.0 - combo_evasion)
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
    let combo_evasion = combo_evasion_percent / 100.0;

    let physical_type_bonus = (def * 2.0
        + floor_int((stats.stab + stats.hack) as f64 / EVASION_PHYSICAL_ATTACK_DIVISOR) as f64)
        / EVASION_TYPE_DIVISOR;

    DefenseProfile {
        physical_defense: floor_int(def * 3.0 + eq_physical * 6.0),
        magic_defense: floor_int(mr * 3.0 + eq_magic * 6.0),
        composite_defense: floor_int((def + mr) * 1.5 + (eq_physical + eq_magic) * 3.0),
        physical_cut_rate: cut_rate(cut_rate_a(stats.def, 10.0)),
        magic_cut_rate: cut_rate(cut_rate_a(stats.mr + equipment_magic_defense, 10.0)),
        composite_cut_rate: cut_rate(cut_rate_a(stats.def + stats.mr + equipment_magic_defense, 20.0)),
        combo_evasion,
        evasion_point: EvasionPoints {
            physical: evasion_point(stats, physical_type_bonus),
            magic: evasion_point(stats, mr * 2.0 / EVASION_TYPE_DIVISOR),
            composite: evasion_point(stats, (def + mr) / EVASION_TYPE_DIVISOR),
        },
        normal_evasion_cap: NORMAL_EVASION_CAP,
        hit_taken_rate_at_cap: hit_taken_rate(NORMAL_EVASION_CAP, combo_evasion),
        equipment_physical_defense: None,
        equipment_magic_defense,
        equipment_evasion: None,
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
    fn 回避Pは15足すAGI1_2倍足す攻撃タイプ別増加() {
        // DEF200 / MR150 / AGI100、STAB+HACK は 0
        let p = defense_profile(&stats(200, 150, 100), 0);
        // 物理: 15 + 120 + (400 + 0)/7 = 135 + 57.142.. = 192.14.. → 192
        assert_eq!(p.evasion_point.physical, 192);
        // 魔法: 15 + 120 + 300/7 = 135 + 42.857.. → 177
        assert_eq!(p.evasion_point.magic, 177);
        // 複合: 15 + 120 + 350/7 = 135 + 50 = 185
        assert_eq!(p.evasion_point.composite, 185);
    }

    #[test]
    fn 物理の回避P増加は突き足す斬りを100で割って切捨ててから足す() {
        let s = EffectiveStats { def: 0, mr: 0, agi: 0, stab: 250, hack: 260, ..Default::default() };
        let p = defense_profile(&s, 0);
        // [(250+260)/100] = 5 → 15 + 0 + 5/7 = 15.714.. → 15
        assert_eq!(p.evasion_point.physical, 15);
    }

    #[test]
    fn 通常回避は敵命中Pとの差で下限0上限85() {
        // 敵命中P が 回避P + 15 以下なら上限 85%
        assert!((normal_evasion(1000, 1010) - 0.85).abs() < 1e-9);
        assert!((normal_evasion(1000, 1015) - 0.85).abs() < 1e-9);
        // 差 40 → 命中率 40% → 回避 60%
        assert!((normal_evasion(1000, 1040) - 0.60).abs() < 1e-9);
        // 敵命中P が 回避P + 100 以上なら 0%(必中)
        assert!(normal_evasion(1000, 1100).abs() < 1e-9);
        assert!(normal_evasion(1000, 9999).abs() < 1e-9);
    }

    #[test]
    fn 上限回避時の最終被弾率は特殊回避と合成する() {
        let p = defense_profile(&stats(0, 150, 200), 0);
        // 特殊回避 46.666..% → (1 − 0.85) × (1 − 0.46666..) = 0.15 × 0.53333.. = 0.08
        assert!((p.hit_taken_rate_at_cap - 0.08).abs() < 1e-9);
        assert!((p.normal_evasion_cap - 0.85).abs() < 1e-9);
    }

    #[test]
    fn 未実装項目は0ではなくnone() {
        let p = defense_profile(&stats(200, 150, 100), 0);
        assert_eq!(p.equipment_physical_defense, None);
        assert_eq!(p.equipment_evasion, None);
    }
}
