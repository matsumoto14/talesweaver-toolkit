//! 共通スキル(wiki: Skill/共通。取得 2026-08-25)。
//!
//! キャラ横断で習得するパッシブ。装備でもバフでもないので独立した補正源として持つ。
//!
//! 効き先は 3 系統:
//! - **装備攻撃力強化倍率**(カテゴリA の内訳): パワーウェポン + ストロングウェポン
//! - **装備防御力倍率**(§6 の防御力計算): コートアーマー + プロテクトアーマー + 改・プロテクトアーマー
//! - **追加ダメージ(新-割合)**(§5): シャープネスビジョン
//! - **能力値倍率B**(§2): アンリーシュ(能力解放)
//!
//! オーグメントは**前提スキル**で、ストロングウェポン / プロテクトアーマー / ハイパーリミットの
//! Lv2 以降に必要(wiki の該当行に赤字で明記)。倍率そのものには効かないので Lv 上限の制約として扱う。
//! レインフォースも同じ形の前提スキルで、アンリーシュの Lv6 以降に必要。

use serde::{Deserialize, Serialize};

use crate::stats::StatKind;
use crate::ultimate_skill::UltimateSkills;

/// ストロングウェポンの Lv 上限(wiki Skill/共通: Lv1〜6 = 3〜18%)。
pub const STRONG_WEAPON_LEVEL_MAX: u8 = 6;
/// プロテクトアーマーの Lv 上限(wiki Skill/共通: Lv1〜6)。
pub const PROTECT_ARMOR_LEVEL_MAX: u8 = 6;
/// 改・プロテクトアーマーの Lv 上限(wiki Skill/共通: Lv1〜5)。
pub const KAI_PROTECT_ARMOR_LEVEL_MAX: u8 = 5;
/// シャープネスビジョンの Lv 上限(wiki Skill/共通: Lv1〜10)。
pub const SHARPNESS_VISION_LEVEL_MAX: u8 = 10;
/// オーグメントの Lv 上限(wiki Skill/共通: Lv5)。
pub const AUGMENT_LEVEL_MAX: u8 = 5;
/// アンリーシュ(能力解放)の Lv 上限(wiki Skill/共通: Lv1〜10)。
pub const UNLEASH_LEVEL_MAX: u8 = 10;
/// レインフォースの Lv 上限(wiki Skill/共通: Lv5)。アンリーシュ Lv6 以降の前提。
pub const REINFORCE_LEVEL_MAX: u8 = 5;
/// レインフォース無しで取れるアンリーシュの Lv(wiki: Lv6 以降が LvUp 必要 = Lv5 までは不要)。
pub const UNLEASH_FREE_LEVEL_MAX: u8 = 5;
/// アンリーシュの枠数(wiki Skill/共通: 2 つまで使用可能)。
pub const UNLEASH_SLOTS: usize = 2;

/// 割合供給源 1 行(トレース表示用)。「なぜこの数字?」パネルの掘り下げに使う汎用の形。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateContribution {
    pub source: String,
    pub value: f64,
}

/// パワーウェポンの装備攻撃力強化倍率(wiki: 自身の装備補正を 2% 増加)。
/// UI(選択肢のラベル・数値表示)は `StatLimits::power_weapon_rate` で参照する(写経しない)。
pub const POWER_WEAPON_RATE: f64 = 0.02;
/// ストロングウェポンの Lv あたりの装備攻撃力強化倍率(3%/6%/9%/12%/15%/18%)。
pub const STRONG_WEAPON_RATE_PER_LEVEL: f64 = 0.03;
/// コートアーマーの装備防御力倍率(物理 18% / 魔法 12%)。
pub const COAT_ARMOR_PHYSICAL_RATE: f64 = 0.18;
pub const COAT_ARMOR_MAGIC_RATE: f64 = 0.12;
/// プロテクトアーマー Lv1〜6 の装備防御力倍率(物理 36/45/54/63/72/81%)。
pub const PROTECT_ARMOR_PHYSICAL: [f64; 6] = [0.36, 0.45, 0.54, 0.63, 0.72, 0.81];
/// 同(魔法 24/30/36/42/48/54%)。
pub const PROTECT_ARMOR_MAGIC: [f64; 6] = [0.24, 0.30, 0.36, 0.42, 0.48, 0.54];
/// 改・プロテクトアーマー Lv1〜5(物理 9/18/27/36/45%)。
pub const KAI_PROTECT_ARMOR_PHYSICAL: [f64; 5] = [0.09, 0.18, 0.27, 0.36, 0.45];
/// 同(魔法 6/12/18/24/30%)。
pub const KAI_PROTECT_ARMOR_MAGIC: [f64; 5] = [0.06, 0.12, 0.18, 0.24, 0.30];
/// シャープネスビジョン Lv1〜10 の割合追加ダメージ(5/10/15/20/25/28/31/34/37/40%)。
pub const SHARPNESS_VISION: [f64; 10] =
    [0.05, 0.10, 0.15, 0.20, 0.25, 0.28, 0.31, 0.34, 0.37, 0.40];
/// アンリーシュ Lv1〜10 の能力値倍率B(wiki Skill/共通「バフ等含むステータス値 × 強化倍率[%]」
/// +1/+2/+3/+4/+5/+8/+11/+14/+17/+20%)。ユーザーの実測(基本能力値 506 で
/// 990/995/1000/1005/1010/1025/1040/1055/1071/1086)と 1 の位まで一致する(2026-08-25)。
pub const UNLEASH: [f64; 10] = [0.01, 0.02, 0.03, 0.04, 0.05, 0.08, 0.11, 0.14, 0.17, 0.20];

/// アンリーシュ(能力解放)の 1 枠。ステを 1 つ選んで Lv を上げる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UnleashSlot {
    /// 解放するステ。`None` = この枠は未使用
    #[serde(default)]
    pub stat: Option<StatKind>,
    /// Lv(0〜10)。Lv6 以降はレインフォースの Lv が要る
    #[serde(default)]
    pub level: u8,
}

/// 装備防御力倍率(物理 / 魔法)。初期値はどちらも 1.0(wiki §6)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DefenseRates {
    pub physical: f64,
    pub magic: f64,
}

impl DefenseRates {
    /// 補正なし。リンゴの島・ベリネンルミではコンテンツ側の仕様で常にこれになる(wiki §6)。
    pub const NEUTRAL: DefenseRates = DefenseRates {
        physical: 1.0,
        magic: 1.0,
    };
}

impl Default for DefenseRates {
    fn default() -> Self {
        DefenseRates::NEUTRAL
    }
}

/// 共通スキル一式。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CommonSkills {
    /// パワーウェポン(Lv1 のみ)。装備攻撃力強化倍率 +2%。ストロングウェポンと重複可
    #[serde(default)]
    pub power_weapon: bool,
    /// ストロングウェポンの Lv(0 = 未使用、1〜6)。Lv2 以降はオーグメントの Lv が要る
    #[serde(default)]
    pub strong_weapon_level: u8,
    /// コートアーマー(Lv1 のみ)。装備防御力倍率 物+18% / 魔+12%。プロテクトアーマーと重複可
    #[serde(default)]
    pub coat_armor: bool,
    /// プロテクトアーマーの Lv(0〜6)。Lv2 以降はオーグメントの Lv が要る
    #[serde(default)]
    pub protect_armor_level: u8,
    /// 改・プロテクトアーマーの Lv(0〜5)。プロテクトアーマーとは別枠で加算される
    #[serde(default)]
    pub kai_protect_armor_level: u8,
    /// シャープネスビジョンの Lv(0〜10)。割合追加ダメージ(§5 新-割合)
    #[serde(default)]
    pub sharpness_vision_level: u8,
    /// オーグメントの Lv(0〜5)。前提スキルで、倍率そのものには効かない
    #[serde(default)]
    pub augment_level: u8,
    /// アンリーシュ(能力解放)の 2 枠。選んだステの能力値倍率B に乗る
    #[serde(default)]
    pub unleash: [UnleashSlot; UNLEASH_SLOTS],
    /// レインフォースの Lv(0〜5)。前提スキルで、アンリーシュの Lv6 以降に要る
    #[serde(default)]
    pub reinforce_level: u8,
    /// 極限スキル(wiki: Skill/極限)。2 枠 + スーパーリミット / ハイパーリミット。
    /// 効果を底上げする 2 スキルは共通スキル(ハイパーアタック / エクストリームアタック)の
    /// 極限形なのでここにぶら下げる
    #[serde(default)]
    pub ultimate: UltimateSkills,
}

impl CommonSkills {
    /// 新規登録キャラの実用既定。共通スキルは「ほぼ全員が取り切っている」前提で上限を入れる
    /// (2026-08-24 決定2: ここを 0 にすると全員が毎回同じ値を入れ直すことになる)。
    /// 人によって違うのはオーグメント・極限スキル 2 枠・シャープネスビジョンだけ。
    /// ストロングウェポン Lv6 にはオーグメント Lv5 が要る(wiki Skill/共通)ので合わせて入れる。
    /// **保存済みキャラの値は書き換えない**(新規登録だけが使う)。
    pub fn practical_default() -> Self {
        CommonSkills {
            power_weapon: true,
            strong_weapon_level: STRONG_WEAPON_LEVEL_MAX,
            coat_armor: true,
            protect_armor_level: PROTECT_ARMOR_LEVEL_MAX,
            kai_protect_armor_level: KAI_PROTECT_ARMOR_LEVEL_MAX,
            // Lv5 までは自然に上がる(ここで止まる人が多い)。Lv6 以降は習得スクロールが要るので人による
            sharpness_vision_level: 5,
            augment_level: AUGMENT_LEVEL_MAX,
            unleash: [UnleashSlot::default(); UNLEASH_SLOTS],
            // アンリーシュ Lv10 の前提。ステを選べば Lv は上限で入る
            reinforce_level: REINFORCE_LEVEL_MAX,
            ultimate: UltimateSkills {
                slots: [None; crate::ultimate_skill::ULTIMATE_SKILL_SLOTS],
                super_limit: true,
                hyper_limit_level: crate::ultimate_skill::HYPER_LIMIT_LEVEL_MAX,
            },
        }
    }
}

impl CommonSkills {
    /// 装備攻撃力強化倍率(wiki: カテゴリA の内訳)。パワーウェポン + ストロングウェポン Lv×3%。
    pub fn equipment_attack_rate(&self) -> f64 {
        self.equipment_attack_rate_sources()
            .iter()
            .map(|s| s.value)
            .sum()
    }

    /// 装備攻撃力強化倍率の供給源内訳。Σvalue = `equipment_attack_rate`(計算を二重に書かない)。
    /// 「なぜこの数字?」パネルの「装備攻撃力強化倍率」の材料掘り下げに使う
    pub fn equipment_attack_rate_sources(&self) -> Vec<RateContribution> {
        let mut out = Vec::new();
        if self.power_weapon {
            out.push(RateContribution {
                source: "パワーウェポン".to_string(),
                value: POWER_WEAPON_RATE,
            });
        }
        if self.strong_weapon_level > 0 {
            out.push(RateContribution {
                source: format!("ストロングウェポン Lv{}", self.strong_weapon_level),
                value: f64::from(self.strong_weapon_level) * STRONG_WEAPON_RATE_PER_LEVEL,
            });
        }
        out
    }

    /// 装備防御力倍率(wiki §6)。初期 100% にコートアーマー・プロテクトアーマー・
    /// 改・プロテクトアーマー・シエナのオーラの防御力増加を**加算**する。
    ///
    /// `siena_rate` はシエナのオーラの追加オプション「防御力増加」の合計(Σ% の小数表現)。
    /// wiki 装備システム/シエナのオーラ: 実際は装備防御力倍率増加でプロテクトアーマーと加算される。
    pub fn defense_rates(&self, siena_rate: f64) -> DefenseRates {
        let level_rate = |level: u8, table: &[f64]| -> f64 {
            if level == 0 {
                0.0
            } else {
                table[(level as usize - 1).min(table.len() - 1)]
            }
        };
        let (coat_physical, coat_magic) = if self.coat_armor {
            (COAT_ARMOR_PHYSICAL_RATE, COAT_ARMOR_MAGIC_RATE)
        } else {
            (0.0, 0.0)
        };
        DefenseRates {
            physical: 1.0
                + coat_physical
                + level_rate(self.protect_armor_level, &PROTECT_ARMOR_PHYSICAL)
                + level_rate(self.kai_protect_armor_level, &KAI_PROTECT_ARMOR_PHYSICAL)
                + siena_rate,
            magic: 1.0
                + coat_magic
                + level_rate(self.protect_armor_level, &PROTECT_ARMOR_MAGIC)
                + level_rate(self.kai_protect_armor_level, &KAI_PROTECT_ARMOR_MAGIC)
                + siena_rate,
        }
    }

    /// シャープネスビジョンの割合追加ダメージ(§5 新-割合)。Σ% の小数表現。
    pub fn sharpness_vision_rate(&self) -> f64 {
        if self.sharpness_vision_level == 0 {
            return 0.0;
        }
        SHARPNESS_VISION[(self.sharpness_vision_level as usize - 1).min(SHARPNESS_VISION.len() - 1)]
    }

    /// オーグメントで解放されている Lv 上限(wiki: Lv2 以降はオーグメントの LvUp が必要)。
    /// オーグメント Lv0 なら 1(= アイテムだけで上げられる Lv1)、Lv5 なら 6。
    pub fn augment_gated_level_max(&self) -> u8 {
        self.augment_level + 1
    }

    /// レインフォースで解放されているアンリーシュの Lv 上限
    /// (wiki: Lv6 以降はレインフォースの LvUp が必要)。Lv0 なら 5、Lv5 なら 10。
    pub fn reinforce_gated_level_max(&self) -> u8 {
        self.reinforce_level + UNLEASH_FREE_LEVEL_MAX
    }

    /// アンリーシュによるこのステの能力値倍率B(wiki: ステータス「能力値倍率B」)。
    /// 2 枠とも同じステには入れられないので、合致する枠は高々 1 つ。
    pub fn unleash_rate(&self, kind: StatKind) -> f64 {
        self.unleash
            .iter()
            .filter(|slot| slot.stat == Some(kind) && slot.level > 0)
            .map(|slot| UNLEASH[(slot.level as usize - 1).min(UNLEASH.len() - 1)])
            .sum()
    }

    pub fn validate(&self) -> Result<(), CommonSkillError> {
        let check = |name: &'static str, value: u8, max: u8| {
            if value > max {
                Err(CommonSkillError::LevelOutOfRange { name, value, max })
            } else {
                Ok(())
            }
        };
        check(
            "ストロングウェポン",
            self.strong_weapon_level,
            STRONG_WEAPON_LEVEL_MAX,
        )?;
        check(
            "プロテクトアーマー",
            self.protect_armor_level,
            PROTECT_ARMOR_LEVEL_MAX,
        )?;
        check(
            "改・プロテクトアーマー",
            self.kai_protect_armor_level,
            KAI_PROTECT_ARMOR_LEVEL_MAX,
        )?;
        check(
            "シャープネスビジョン",
            self.sharpness_vision_level,
            SHARPNESS_VISION_LEVEL_MAX,
        )?;
        check("オーグメント", self.augment_level, AUGMENT_LEVEL_MAX)?;
        check("レインフォース", self.reinforce_level, REINFORCE_LEVEL_MAX)?;
        // オーグメント制約(wiki Skill/共通の該当行に赤字で明記)
        let max = self.augment_gated_level_max();
        for (name, level) in [
            ("ストロングウェポン", self.strong_weapon_level),
            ("プロテクトアーマー", self.protect_armor_level),
        ] {
            if level > max {
                return Err(CommonSkillError::AugmentRequired {
                    name,
                    value: level,
                    augment_level: self.augment_level,
                    max,
                });
            }
        }
        self.ultimate.validate(max)?;
        // アンリーシュ(wiki Skill/共通: 2 つまで・Lv6 以降はレインフォースが前提)
        let unleash_max = self.reinforce_gated_level_max();
        for slot in &self.unleash {
            if slot.stat.is_none() {
                continue;
            }
            check("アンリーシュ", slot.level, UNLEASH_LEVEL_MAX)?;
            if slot.level > unleash_max {
                return Err(CommonSkillError::ReinforceRequired {
                    value: slot.level,
                    reinforce_level: self.reinforce_level,
                    max: unleash_max,
                });
            }
        }
        let [first, second] = self.unleash;
        if first.stat.is_some() && first.stat == second.stat {
            return Err(CommonSkillError::UnleashDuplicated {
                kind: first.stat.unwrap(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum CommonSkillError {
    #[error("{name}の Lv は 0〜{max} です(指定値 {value})")]
    LevelOutOfRange {
        name: &'static str,
        value: u8,
        max: u8,
    },
    #[error("{name} Lv{value} にはオーグメントの Lv が足りません(オーグメント Lv{augment_level} では Lv{max} まで)")]
    AugmentRequired {
        name: &'static str,
        value: u8,
        augment_level: u8,
        max: u8,
    },
    #[error("アンリーシュ Lv{value} にはレインフォースの Lv が足りません(レインフォース Lv{reinforce_level} では Lv{max} まで)")]
    ReinforceRequired {
        value: u8,
        reinforce_level: u8,
        max: u8,
    },
    #[error("アンリーシュの 2 枠が同じステ({kind:?})です(別のステを選んでください)")]
    UnleashDuplicated { kind: StatKind },
    #[error(transparent)]
    UltimateSkill(#[from] crate::ultimate_skill::UltimateSkillError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unleash(stat: StatKind, level: u8) -> UnleashSlot {
        UnleashSlot {
            stat: Some(stat),
            level,
        }
    }

    /// wiki Skill/共通「能力解放 - XXX(アンリーシュ)」: +1/+2/+3/+4/+5/+8/+11/+14/+17/+20%。
    /// ユーザーの実測(2026-08-25。基本能力値 506・最終固定値 479)と 1 の位まで一致する:
    /// Lv1〜10 で最終能力値 990/995/1000/1005/1010/1025/1040/1055/1071/1086
    #[test]
    fn アンリーシュは実測どおりの倍率bになる() {
        let basic = 506_i64;
        let final_fixed = 479_i64;
        let expected = [990, 995, 1000, 1005, 1010, 1025, 1040, 1055, 1071, 1086];
        for (i, want) in expected.iter().enumerate() {
            let level = i as u8 + 1;
            let c = CommonSkills {
                unleash: [unleash(StatKind::Stab, level), UnleashSlot::default()],
                reinforce_level: REINFORCE_LEVEL_MAX,
                ..Default::default()
            };
            let rate = c.unleash_rate(StatKind::Stab);
            let effective = basic + crate::rounding::floor_int(basic as f64 * rate) + final_fixed;
            assert_eq!(effective, *want, "Lv{level}(倍率 {rate})");
        }
    }

    #[test]
    fn アンリーシュは選んだステにだけ乗り2枠とも効く() {
        let c = CommonSkills {
            unleash: [unleash(StatKind::Stab, 10), unleash(StatKind::Dex, 5)],
            reinforce_level: REINFORCE_LEVEL_MAX,
            ..Default::default()
        };
        assert!((c.unleash_rate(StatKind::Stab) - 0.20).abs() < 1e-12);
        assert!((c.unleash_rate(StatKind::Dex) - 0.05).abs() < 1e-12);
        assert_eq!(c.unleash_rate(StatKind::Hack), 0.0);
        // 未選択の枠・Lv0 は効かない
        assert_eq!(CommonSkills::default().unleash_rate(StatKind::Stab), 0.0);
    }

    // wiki Skill/共通: Lv6 以降はレインフォース(Lv1〜5)の LvUp が必要
    #[test]
    fn アンリーシュのlv6以降はレインフォースに縛られる() {
        let with = |level: u8, reinforce: u8| CommonSkills {
            unleash: [unleash(StatKind::Stab, level), UnleashSlot::default()],
            reinforce_level: reinforce,
            ..Default::default()
        };
        // レインフォース無しでも Lv5 までは取れる
        assert!(with(5, 0).validate().is_ok());
        assert!(matches!(
            with(6, 0).validate(),
            Err(CommonSkillError::ReinforceRequired { max: 5, .. })
        ));
        assert!(with(6, 1).validate().is_ok());
        assert!(with(10, 5).validate().is_ok());
        assert!(matches!(
            with(10, 4).validate(),
            Err(CommonSkillError::ReinforceRequired { max: 9, .. })
        ));
        // Lv 上限そのもの
        assert!(matches!(
            with(11, 5).validate(),
            Err(CommonSkillError::LevelOutOfRange {
                name: "アンリーシュ",
                ..
            })
        ));
    }

    #[test]
    fn アンリーシュの2枠に同じステは入れられない() {
        let same = CommonSkills {
            unleash: [unleash(StatKind::Stab, 1), unleash(StatKind::Stab, 2)],
            ..Default::default()
        };
        assert!(matches!(
            same.validate(),
            Err(CommonSkillError::UnleashDuplicated {
                kind: StatKind::Stab
            })
        ));
        let ok = CommonSkills {
            unleash: [unleash(StatKind::Stab, 1), unleash(StatKind::Hack, 2)],
            ..Default::default()
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn 装備攻撃力強化倍率はパワーウェポンとストロングウェポンの和() {
        let s = CommonSkills {
            power_weapon: true,
            strong_weapon_level: 6,
            ..Default::default()
        };
        assert!((s.equipment_attack_rate() - 0.20).abs() < 1e-12);
        assert_eq!(CommonSkills::default().equipment_attack_rate(), 0.0);
    }

    // wiki Skill/共通: コートアーマー 物18%/魔12%、プロテクトアーマー Lv6 物81%/魔54%
    #[test]
    fn 装備防御力倍率はコートアーマーとプロテクトアーマーの加算() {
        let s = CommonSkills {
            coat_armor: true,
            protect_armor_level: 6,
            ..Default::default()
        };
        let r = s.defense_rates(0.0);
        assert!((r.physical - (1.0 + 0.18 + 0.81)).abs() < 1e-12);
        assert!((r.magic - (1.0 + 0.12 + 0.54)).abs() < 1e-12);
    }

    // 改・プロテクトアーマーは wiki 上も別行のスキルなので別枠で加算する
    #[test]
    fn 改プロテクトアーマーは別枠で加算される() {
        let s = CommonSkills {
            protect_armor_level: 6,
            kai_protect_armor_level: 5,
            ..Default::default()
        };
        let r = s.defense_rates(0.0);
        assert!((r.physical - (1.0 + 0.81 + 0.45)).abs() < 1e-12);
        assert!((r.magic - (1.0 + 0.54 + 0.30)).abs() < 1e-12);
    }

    // wiki 装備システム/シエナのオーラ: 防御力増加は装備防御力倍率増加でプロテクトアーマーと加算
    #[test]
    fn シエナの防御力増加は装備防御力倍率に加算される() {
        let s = CommonSkills {
            protect_armor_level: 1,
            ..Default::default()
        };
        let r = s.defense_rates(0.10);
        assert!((r.physical - (1.0 + 0.36 + 0.10)).abs() < 1e-12);
        assert!((r.magic - (1.0 + 0.24 + 0.10)).abs() < 1e-12);
    }

    #[test]
    fn 未習得の装備防御力倍率は中立値() {
        assert_eq!(
            CommonSkills::default().defense_rates(0.0),
            DefenseRates::NEUTRAL
        );
    }

    // wiki Skill/共通: Lv1〜5 は 5/10/15/20/25%、Lv6〜10 は 28/31/34/37/40%
    #[test]
    fn シャープネスビジョンは最大40パーセント() {
        let lv = |n| {
            CommonSkills {
                sharpness_vision_level: n,
                ..Default::default()
            }
            .sharpness_vision_rate()
        };
        assert_eq!(lv(0), 0.0);
        assert!((lv(5) - 0.25).abs() < 1e-12);
        assert!((lv(6) - 0.28).abs() < 1e-12);
        assert!((lv(10) - 0.40).abs() < 1e-12);
    }

    // wiki Skill/共通: ストロングウェポン・プロテクトアーマーは Lv2 以降にオーグメントの LvUp が要る
    #[test]
    fn オーグメントが足りないlvは拒否する() {
        let mut s = CommonSkills {
            strong_weapon_level: 6,
            augment_level: 5,
            ..Default::default()
        };
        assert!(s.validate().is_ok());

        s.augment_level = 4;
        assert!(matches!(
            s.validate(),
            Err(CommonSkillError::AugmentRequired { .. })
        ));

        // オーグメント無しでも Lv1 は取れる
        let lv1 = CommonSkills {
            strong_weapon_level: 1,
            ..Default::default()
        };
        assert!(lv1.validate().is_ok());
    }

    #[test]
    fn lv上限を超える値は拒否する() {
        let s = CommonSkills {
            sharpness_vision_level: 11,
            ..Default::default()
        };
        assert!(matches!(
            s.validate(),
            Err(CommonSkillError::LevelOutOfRange { .. })
        ));
    }
}
