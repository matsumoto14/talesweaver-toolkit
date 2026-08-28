//! クリティカル率(wiki: 計算式まとめ `#CriticalChance`。取得 2026-08-25)。docs/damage-formula.md §9。
//!
//! ```text
//! クリティカル確率 = ( (装備クリティカル補正 + 1) × 2 × (AGI / (AGI + 対象のAGI))
//!                      × ペット会心 × シエナのオーラ
//!                      + スキルクリティカル率 + クリティカル率増加 + 対象のクリティカル被撃率A )
//!                    × 対象のクリティカル被撃率B + 最終クリティカル率増加
//! ```
//!
//! 単位はパーセントポイント。下限 0% / 上限 100%。
//!
//! `ペット会心`(×1.1)と `シエナのオーラ`(追加オプション「クリティカル確率」)は
//! **AGI 由来の項に掛かる乗数**で、加算項ではない。
//!
//! **対象のAGI と クリティカル被撃率A が両方そろっている敵でしか出せない**。狩り場情報一覧は
//! 多くの行が `?` で、被撃率A は −250〜−930% と支配的なので、片方だけで出すと桁違いに外れる。
//! 未収録の入力(被撃率B(対人のみ)・最終クリティカル率増加)は中立値。

use serde::{Deserialize, Serialize};

/// クリティカル率増加の上限(wiki `#CriticalChance`「クリティカル率増加(上限+100%)」)。
pub const CRITICAL_RATE_BONUS_MAX: f64 = 100.0;
/// ペット会心(wiki: 「クリティカル率1.1倍」)。
const PET_CRITICAL_RATE: f64 = 1.1;

/// クリティカル率増加の供給源(wiki `#CriticalChance` の「クリティカル率増加」表)。
/// 「バフ」は値が `n` で不定なので入れず、値が確定している 3 件だけを持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticalRateSourceId {
    /// 極のルーン(最大レベル時 +20)
    UltimateRune,
    /// 設計者の研究室 B グループ「クリティカル率増加」(1 段階 +3・最大 10 段階 = +30)
    ArchitectLab,
    /// 致命打(+100)
    DeadlyBlow,
}

impl CriticalRateSourceId {
    pub const ALL: [CriticalRateSourceId; 3] = [
        CriticalRateSourceId::UltimateRune,
        CriticalRateSourceId::ArchitectLab,
        CriticalRateSourceId::DeadlyBlow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CriticalRateSourceId::UltimateRune => "極のルーン",
            CriticalRateSourceId::ArchitectLab => "設計者の研究室",
            CriticalRateSourceId::DeadlyBlow => "致命打",
        }
    }

    /// クリティカル率増加の**最大値**(パーセントポイント)。
    /// 設計者の研究室は段階制なので、実際に乗る値は `CriticalRateSources::architect_lab_bonus`。
    pub fn max_value(self) -> f64 {
        match self {
            CriticalRateSourceId::UltimateRune => 20.0,
            CriticalRateSourceId::ArchitectLab => {
                f64::from(ARCHITECT_LAB_STAGE_MAX) * ARCHITECT_LAB_PER_STAGE
            }
            CriticalRateSourceId::DeadlyBlow => 100.0,
        }
    }
}

/// 設計者の研究室の研究段階の上限(wiki: 設計者の研究室「A・B グループは最大 10 段階まで研究可能」)。
pub const ARCHITECT_LAB_STAGE_MAX: u8 = 10;
/// 設計者の研究室 B グループ「クリティカル率増加」の 1 段階あたり増加量
/// (wiki: 設計者の研究室 永続バフの表)。
pub const ARCHITECT_LAB_PER_STAGE: f64 = 3.0;

/// クリティカル率の供給源の選択。`Default` は全部オフ(中立)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CriticalRateSources {
    /// ペット会心(クリティカル率 ×1.1)
    #[serde(default)]
    pub pet: bool,
    #[serde(default)]
    pub ultimate_rune: bool,
    /// 設計者の研究室 B グループ「クリティカル率増加」の研究段階 0..=10(1 段階 +3)。
    /// 研究は**テイルズID 内の全キャラに適用**されるが、入力はキャラごとに持つ
    #[serde(default)]
    pub architect_lab_stage: u8,
    #[serde(default)]
    pub deadly_blow: bool,
}

impl CriticalRateSources {
    /// 設計者の研究室ぶんのクリティカル率増加(段階 × 3)。
    pub fn architect_lab_bonus(&self) -> f64 {
        f64::from(self.architect_lab_stage) * ARCHITECT_LAB_PER_STAGE
    }

    /// 供給源ごとの実際の値(0 = 未習得・未研究)。
    pub fn value_of(&self, id: CriticalRateSourceId) -> f64 {
        match id {
            CriticalRateSourceId::UltimateRune => {
                if self.ultimate_rune { id.max_value() } else { 0.0 }
            }
            CriticalRateSourceId::ArchitectLab => self.architect_lab_bonus(),
            CriticalRateSourceId::DeadlyBlow => {
                if self.deadly_blow { id.max_value() } else { 0.0 }
            }
        }
    }

    /// クリティカル率増加の合計(上限を掛ける前)。UI の「頭打ち」表示に使う。
    pub fn raw_bonus(&self) -> f64 {
        CriticalRateSourceId::ALL.iter().map(|id| self.value_of(*id)).sum()
    }

    /// クリティカル率増加の合計(上限 +100%)。ペット会心は倍率なのでここには入らない。
    pub fn bonus(&self) -> f64 {
        self.raw_bonus().min(CRITICAL_RATE_BONUS_MAX)
    }

    /// 研究段階が wiki の上限に収まっているか。
    pub fn validate(&self) -> Result<(), CriticalRateError> {
        if self.architect_lab_stage > ARCHITECT_LAB_STAGE_MAX {
            return Err(CriticalRateError::ArchitectLabStageOutOfRange {
                value: self.architect_lab_stage,
                max: ARCHITECT_LAB_STAGE_MAX,
            });
        }
        Ok(())
    }

    /// ペット会心の倍率(習得していなければ 1.0)。
    pub fn pet_rate(&self) -> f64 {
        if self.pet {
            PET_CRITICAL_RATE
        } else {
            1.0
        }
    }
}

/// クリティカル率の供給源の値域違反。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum CriticalRateError {
    #[error("設計者の研究室の研究段階は 0〜{max} です(指定値 {value})")]
    ArchitectLabStageOutOfRange { value: u8, max: u8 },
}

/// クリティカル率の内訳。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CriticalRate {
    /// 装備クリティカル補正(基本 + 強化の合計)
    pub equipment_critical: i64,
    /// キャラの AGI(最終能力値)
    pub agi: i64,
    /// 対象の AGI(wiki 狩り場情報一覧「敵AGI+固定値」の合計)
    pub target_agi: i64,
    /// AGI 由来の部分
    /// `(装備クリティカル補正 + 1) × 2 × (AGI / (AGI + 対象AGI)) × ペット会心 × シエナのオーラ`
    pub from_agi: f64,
    /// シエナのオーラの追加オプション「クリティカル確率」の Σ%(小数表現)。AGI 由来の項に乗る
    pub siena_rate: f64,
    /// スキルクリティカル率(wiki スキル性能一覧の Cri値)
    pub skill: f64,
    /// クリティカル率増加(上限 +100%)
    pub bonus: f64,
    /// 対象のクリティカル被撃率A(負値)
    pub target_taken_rate: f64,
    /// 下限 0% / 上限 100% を掛ける前の値
    pub raw: f64,
    /// クリティカル率(%)
    pub value: f64,
}

/// クリティカル率を出す。`target_agi` / `target_taken_rate` は wiki 狩り場情報一覧の値で、
/// **どちらかが未記載なら呼び出し側が `None` を返す**(この関数は両方そろっている前提)。
#[allow(clippy::too_many_arguments)]
pub fn critical_rate(
    equipment_critical: i64,
    agi: i64,
    target_agi: i64,
    skill_critical_rate: f64,
    sources: &CriticalRateSources,
    siena_rate: f64,
    target_taken_rate: f64,
) -> CriticalRate {
    let denominator = (agi + target_agi) as f64;
    // AGI も対象AGI も 0 なら 0 除算になる。その場合は AGI 由来の項を 0 にする
    let agi_ratio = if denominator == 0.0 { 0.0 } else { agi as f64 / denominator };
    let from_agi = (equipment_critical + 1) as f64
        * 2.0
        * agi_ratio
        * sources.pet_rate()
        * (1.0 + siena_rate);
    let bonus = sources.bonus();
    let raw = from_agi + skill_critical_rate + bonus + target_taken_rate;
    CriticalRate {
        equipment_critical,
        agi,
        target_agi,
        from_agi,
        siena_rate,
        skill: skill_critical_rate,
        bonus,
        target_taken_rate,
        raw,
        value: raw.clamp(0.0, 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // wiki `#CriticalChance` の式をそのまま:
    //   (300 + 1) × 2 × (1500 / (1500 + 1420)) = 602 × 0.51369... = 309.24...
    //   + スキル Cri値 13 + 増加 0 + 被撃率A −350 = −27.7 → 下限 0%
    #[test]
    fn 被撃率aが効いて下限0になる() {
        let r = critical_rate(300, 1500, 1420, 13.0, &CriticalRateSources::default(), 0.0, -350.0);
        assert!((r.from_agi - 602.0 * (1500.0 / 2920.0)).abs() < 1e-9);
        assert!(r.raw < 0.0);
        assert_eq!(r.value, 0.0);
    }

    #[test]
    fn 増加とペット会心が効く() {
        let sources = CriticalRateSources {
            pet: true,
            ultimate_rune: true,
            architect_lab_stage: ARCHITECT_LAB_STAGE_MAX,
            deadly_blow: false,
        };
        // 増加は 20 + 30 = 50
        assert_eq!(sources.bonus(), 50.0);
        let r = critical_rate(300, 1500, 1420, 13.0, &sources, 0.0, -350.0);
        let base = critical_rate(300, 1500, 1420, 13.0, &CriticalRateSources::default(), 0.0, -350.0);
        // ペット会心 ×1.1
        assert!((r.from_agi - base.from_agi * 1.1).abs() < 1e-9);
        assert!(r.raw > base.raw);
    }

    // wiki `#CriticalChance`: クリティカル率増加の上限は +100%
    #[test]
    fn 増加は100で頭打ち() {
        let sources = CriticalRateSources {
            pet: false,
            ultimate_rune: true,
            architect_lab_stage: ARCHITECT_LAB_STAGE_MAX,
            deadly_blow: true,
        };
        assert_eq!(sources.bonus(), CRITICAL_RATE_BONUS_MAX);
    }

    // wiki: 設計者の研究室「クリティカル率増加 1 段階 +3」「最大 10 段階」
    #[test]
    fn 設計者の研究室は段階ごとに3ずつ増える() {
        for stage in 0..=ARCHITECT_LAB_STAGE_MAX {
            let sources = CriticalRateSources { architect_lab_stage: stage, ..Default::default() };
            assert_eq!(sources.architect_lab_bonus(), f64::from(stage) * 3.0);
            assert_eq!(sources.bonus(), f64::from(stage) * 3.0);
            assert!(sources.validate().is_ok());
        }
        let over = CriticalRateSources {
            architect_lab_stage: ARCHITECT_LAB_STAGE_MAX + 1,
            ..Default::default()
        };
        assert!(matches!(
            over.validate(),
            Err(CriticalRateError::ArchitectLabStageOutOfRange { .. })
        ));
    }

    // wiki `#CriticalChance`: 下限 0% / 上限 100%
    #[test]
    fn 上限は100パーセント() {
        let r = critical_rate(3000, 2000, 100, 15.0, &CriticalRateSources::default(), 0.0, 0.0);
        assert!(r.raw > 100.0);
        assert_eq!(r.value, 100.0);
    }

    // wiki `#CriticalChance`: シエナのオーラは AGI 由来の項に掛かる乗数(加算項ではない)
    #[test]
    fn シエナのオーラはagi由来の項に乗算で効く() {
        let base = critical_rate(300, 1500, 1420, 13.0, &CriticalRateSources::default(), 0.0, -350.0);
        let siena = critical_rate(300, 1500, 1420, 13.0, &CriticalRateSources::default(), 0.80, -350.0);
        assert!((siena.from_agi - base.from_agi * 1.80).abs() < 1e-9);
        assert!((siena.siena_rate - 0.80).abs() < 1e-12);
        // 加算項(スキル Cri値・増加・被撃率)は変わらない
        assert_eq!(siena.skill, base.skill);
        assert_eq!(siena.target_taken_rate, base.target_taken_rate);
    }

    #[test]
    fn agiが両方0でも0除算しない() {
        let r = critical_rate(300, 0, 0, 10.0, &CriticalRateSources::default(), 0.0, 0.0);
        assert_eq!(r.from_agi, 0.0);
        assert_eq!(r.value, 10.0);
    }
}
