//! 極限スキル(wiki: Skill/極限。取得 2026-08-25)。
//!
//! ゲージスキルの一部が極限後にパッシブとして常時適用される。**2 つまで**習得でき、
//! 変更には初期化スクロールが要る。
//!
//! 効果値は wiki の表の列がそのまま加算になっている:
//! `基本 + スーパーリミット + ハイパーリミット Lv`。合計が「最大値」列と一致する。
//! - スーパーリミット = 共通スキル「ハイパーアタック」が極限状態で変化したもの(Lv1 のみ)
//! - ハイパーリミット = 同「エクストリームアタック」(Lv1〜6)。**Lv2 以降はオーグメントの LvUp が要る**
//!
//! 収録するのは 3 種だけ(ユーザー決定 2026-08-25)。エレメンタルパワー・オーバードライブは入れない。

use serde::{Deserialize, Serialize};

/// ハイパーリミットの Lv 上限(wiki Skill/極限: Lv1〜6)。
pub const HYPER_LIMIT_LEVEL_MAX: u8 = 6;
/// 極限スキルの枠数(wiki Skill/極限: 2 つまで習得可能)。
pub const ULTIMATE_SKILL_SLOTS: usize = 2;

/// スコープアイのクリティカルダメージ増加(基本 / スーパーリミット / ハイパーリミット Lv1〜6)。
const SCOPE_EYE_BASE: f64 = 20.0;
const SCOPE_EYE_SUPER: f64 = 3.0;
const SCOPE_EYE_HYPER: [f64; 6] = [7.0, 9.0, 11.0, 13.0, 15.0, 17.0];

/// フルスロットルの中ディレイ減少 %(基本 25% = ×75%)。段階5 で使う。
const FULL_THROTTLE_DELAY_BASE: f64 = 25.0;
const FULL_THROTTLE_DELAY_SUPER: f64 = 3.0;
const FULL_THROTTLE_DELAY_HYPER: [f64; 6] = [7.0, 9.0, 11.0, 13.0, 15.0, 17.0];

/// フルスロットルの単体チャネリングスキル段数(ハイパーリミット Lv1〜6)。
/// 基本もスーパーリミットも +0 なので、ハイパーリミットの表だけ持つ。
const FULL_THROTTLE_HITS_HYPER: [u32; 6] = [0, 0, 0, 1, 2, 3];

/// ワイドフォーカスのスキル範囲(基本 / スーパーリミット / ハイパーリミット Lv1〜6)。
/// 火力には効かないので記録のみ。
const WIDE_FOCUS_BASE: f64 = 4.0;
const WIDE_FOCUS_SUPER: f64 = 2.0;
const WIDE_FOCUS_HYPER: [f64; 6] = [4.0, 6.0, 8.0, 10.0, 12.0, 14.0];

/// 選べる極限スキル(wiki の表のうち収録する 3 種)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UltimateSkill {
    /// スコープアイ: クリティカルダメージ増加(カテゴリG)
    ScopeEye,
    /// フルスロットル: 中ディレイ減少 + 単体チャネリングスキルの段数
    FullThrottle,
    /// ワイドフォーカス: スキル範囲。火力には効かない
    WideFocus,
}

impl UltimateSkill {
    pub const ALL: [UltimateSkill; 3] = [
        UltimateSkill::ScopeEye,
        UltimateSkill::FullThrottle,
        UltimateSkill::WideFocus,
    ];

    pub fn name(self) -> &'static str {
        match self {
            UltimateSkill::ScopeEye => "スコープアイ",
            UltimateSkill::FullThrottle => "フルスロットル",
            UltimateSkill::WideFocus => "ワイドフォーカス",
        }
    }
}

/// 極限スキル一式(2 枠 + 効果を底上げする 2 スキル)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct UltimateSkills {
    /// 選んだ極限スキル(最大 2 つ)。同じスキルは 2 枠に入れられない
    #[serde(default)]
    pub slots: [Option<UltimateSkill>; ULTIMATE_SKILL_SLOTS],
    /// スーパーリミット(共通スキル「ハイパーアタック」の極限形。Lv1 のみ)
    #[serde(default)]
    pub super_limit: bool,
    /// ハイパーリミットの Lv(0〜6)。Lv2 以降はオーグメントの Lv が要る
    #[serde(default)]
    pub hyper_limit_level: u8,
}

impl UltimateSkills {
    pub fn has(&self, skill: UltimateSkill) -> bool {
        self.slots.iter().any(|s| *s == Some(skill))
    }

    /// ハイパーリミット Lv に応じたテーブル引き。Lv0 なら 0。
    fn hyper<T: Copy + Default>(&self, table: &[T; 6]) -> T {
        if self.hyper_limit_level == 0 {
            T::default()
        } else {
            table[(self.hyper_limit_level as usize - 1).min(5)]
        }
    }

    /// スコープアイのクリティカルダメージ増加(wiki: カテゴリG)。Σ% の小数表現。
    /// **非クリティカルの一撃には乗らない**(G の供給源はすべてクリティカル時限定)。
    pub fn critical_damage_rate(&self) -> f64 {
        if !self.has(UltimateSkill::ScopeEye) {
            return 0.0;
        }
        let super_limit = if self.super_limit {
            SCOPE_EYE_SUPER
        } else {
            0.0
        };
        (SCOPE_EYE_BASE + super_limit + self.hyper(&SCOPE_EYE_HYPER)) / 100.0
    }

    /// フルスロットルの中ディレイ減少(段階5 で使う)。Σ% の小数表現。
    pub fn actual_delay_reduction(&self) -> f64 {
        if !self.has(UltimateSkill::FullThrottle) {
            return 0.0;
        }
        let super_limit = if self.super_limit {
            FULL_THROTTLE_DELAY_SUPER
        } else {
            0.0
        };
        (FULL_THROTTLE_DELAY_BASE + super_limit + self.hyper(&FULL_THROTTLE_DELAY_HYPER)) / 100.0
    }

    /// フルスロットルの段数増加。**単体チャネリングスキルにだけ**乗る
    /// (対象スキルかどうかの判定は `Skill::single_target_channeling`)。
    pub fn added_hit_count(&self) -> u32 {
        if !self.has(UltimateSkill::FullThrottle) {
            return 0;
        }
        self.hyper(&FULL_THROTTLE_HITS_HYPER)
    }

    /// ワイドフォーカスのスキル範囲増加。火力には効かないので表示専用。
    pub fn skill_range_bonus(&self) -> f64 {
        if !self.has(UltimateSkill::WideFocus) {
            return 0.0;
        }
        let super_limit = if self.super_limit {
            WIDE_FOCUS_SUPER
        } else {
            0.0
        };
        WIDE_FOCUS_BASE + super_limit + self.hyper(&WIDE_FOCUS_HYPER)
    }

    /// `augment_gated_level_max` はオーグメントで解放されている Lv 上限
    /// (`CommonSkills::augment_gated_level_max`)。ハイパーリミットも同じゲートに従う。
    pub fn validate(&self, augment_gated_level_max: u8) -> Result<(), UltimateSkillError> {
        if self.hyper_limit_level > HYPER_LIMIT_LEVEL_MAX {
            return Err(UltimateSkillError::HyperLimitOutOfRange {
                value: self.hyper_limit_level,
                max: HYPER_LIMIT_LEVEL_MAX,
            });
        }
        if self.hyper_limit_level > augment_gated_level_max {
            return Err(UltimateSkillError::AugmentRequired {
                value: self.hyper_limit_level,
                max: augment_gated_level_max,
            });
        }
        let [first, second] = self.slots;
        if first.is_some() && first == second {
            return Err(UltimateSkillError::Duplicated {
                name: first.unwrap().name(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum UltimateSkillError {
    #[error("ハイパーリミットの Lv は 0〜{max} です(指定値 {value})")]
    HyperLimitOutOfRange { value: u8, max: u8 },
    #[error("ハイパーリミット Lv{value} にはオーグメントの Lv が足りません(いまは Lv{max} まで)")]
    AugmentRequired { value: u8, max: u8 },
    #[error("極限スキル「{name}」が 2 枠とも同じです(別のスキルを選んでください)")]
    Duplicated { name: &'static str },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(skill: UltimateSkill, super_limit: bool, hyper: u8) -> UltimateSkills {
        UltimateSkills {
            slots: [Some(skill), None],
            super_limit,
            hyper_limit_level: hyper,
        }
    }

    // wiki Skill/極限: 基本 +20% / スーパーリミット +3 / ハイパーリミット Lv6 +17 → 最大値 +40%
    #[test]
    fn スコープアイは最大40パーセント() {
        let s = with(UltimateSkill::ScopeEye, true, 6);
        assert!((s.critical_damage_rate() - 0.40).abs() < 1e-12);

        // 基本だけなら +20%
        let base = with(UltimateSkill::ScopeEye, false, 0);
        assert!((base.critical_damage_rate() - 0.20).abs() < 1e-12);
    }

    #[test]
    fn 選んでいない極限スキルの効果は0() {
        let s = with(UltimateSkill::WideFocus, true, 6);
        assert_eq!(s.critical_damage_rate(), 0.0);
        assert_eq!(s.added_hit_count(), 0);
        assert_eq!(s.actual_delay_reduction(), 0.0);
    }

    // wiki Skill/極限: ×75% − 3 − 17 = ×55%(= 減少 45%)
    #[test]
    fn フルスロットルの中ディレイ減少は最大45パーセント() {
        let s = with(UltimateSkill::FullThrottle, true, 6);
        assert!((s.actual_delay_reduction() - 0.45).abs() < 1e-12);
    }

    // wiki Skill/極限: 段数はハイパーリミット Lv4 から +1/+2/+3
    #[test]
    fn フルスロットルの段数はハイパーリミットlv4から増える() {
        for (lv, expected) in [(0, 0), (3, 0), (4, 1), (5, 2), (6, 3)] {
            let s = with(UltimateSkill::FullThrottle, true, lv);
            assert_eq!(s.added_hit_count(), expected, "Lv{lv}");
        }
    }

    // wiki Skill/極限: +4 +2 +14 = +20
    #[test]
    fn ワイドフォーカスは最大プラス20() {
        let s = with(UltimateSkill::WideFocus, true, 6);
        assert!((s.skill_range_bonus() - 20.0).abs() < 1e-12);
    }

    #[test]
    fn 同じ極限スキルを2枠に入れられない() {
        let s = UltimateSkills {
            slots: [Some(UltimateSkill::ScopeEye), Some(UltimateSkill::ScopeEye)],
            ..Default::default()
        };
        assert!(matches!(
            s.validate(6),
            Err(UltimateSkillError::Duplicated { .. })
        ));

        let ok = UltimateSkills {
            slots: [
                Some(UltimateSkill::ScopeEye),
                Some(UltimateSkill::FullThrottle),
            ],
            ..Default::default()
        };
        assert!(ok.validate(6).is_ok());
    }

    // wiki Skill/極限: ハイパーリミット Lv2 以降はオーグメントで LvUp
    #[test]
    fn ハイパーリミットもオーグメントに縛られる() {
        let s = with(UltimateSkill::ScopeEye, false, 6);
        assert!(s.validate(6).is_ok());
        assert!(matches!(
            s.validate(1),
            Err(UltimateSkillError::AugmentRequired { .. })
        ));

        // オーグメント無しでも Lv1 は取れる
        let lv1 = with(UltimateSkill::ScopeEye, false, 1);
        assert!(lv1.validate(1).is_ok());
    }

    #[test]
    fn lv上限を超える値は拒否する() {
        let s = with(UltimateSkill::ScopeEye, false, 7);
        assert!(matches!(
            s.validate(7),
            Err(UltimateSkillError::HyperLimitOutOfRange { .. })
        ));
    }
}
