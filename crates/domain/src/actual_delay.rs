//! 中ディレイ(スキルの発動間隔)。出典: wiki 計算式まとめ `#ActualDelay` /
//! ステータス「中ディレイ倍率A / 倍率B」/ 装備システム/シエナのオーラ(取得 2026-08-25)。
//!
//! ```text
//! 中ディレイ = 基本中ディレイ × (1 − 中ディレイ減少値) × (2 コンボ以上なら 0.5)   (下限 0.3s)
//! ```
//!
//! - **基本中ディレイ**は wiki スキル性能一覧の「動作」列(`Skill::base_actual_delay`)
//! - **中ディレイ減少値**(= 倍率B の減少分)は上限 70%。供給源は極限スキル「フルスロットル」/
//!   カフス(盾+)のランダムオプション / シエナのオーラの追加オプション / キャラのパッシブ・マスタリー
//! - **コンボボーナス**(2 コンボ以上で ×0.5)は倍率A で、減少値の上限 70% の**対象外**
//! - wiki が「(固定)」と書いている中ディレイ(極・ギガブレイズ 等)は減少が効かない

use serde::{Deserialize, Serialize};

/// 中ディレイ減少値の上限(wiki `#ActualDelay`「中ディレイ減少値の上限は70%」。
/// = ステータス「中ディレイ倍率B (初期値:100%、下限30%)」)。
pub const ACTUAL_DELAY_REDUCTION_MAX: f64 = 0.70;
/// 中ディレイの下限(wiki `#ActualDelay`「中ディレイの下限は0.3s」)。
pub const ACTUAL_DELAY_MIN: f64 = 0.3;
/// コンボボーナスの倍率A(wiki ステータス「中ディレイ倍率A / コンボ / 2コンボ以上のコンボボーナス −50%」)。
const COMBO_DELAY_RATE: f64 = 0.5;
/// コンボボーナスが付くコンボ数。
const COMBO_DELAY_THRESHOLD: u32 = 2;

/// 中ディレイ減少の供給源 1 件(トレース表示用)。値は Σ% の小数表現(−5% → 0.05)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualDelayContribution {
    pub source: String,
    pub rate: f64,
}

/// 中ディレイの内訳。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualDelay {
    /// 基本中ディレイ(秒)。wiki スキル性能一覧の「動作」列
    pub base: f64,
    /// 上限を掛ける前の中ディレイ減少値
    pub reduction_raw: f64,
    /// 上限(70%)適用後の中ディレイ減少値
    pub reduction: f64,
    /// 倍率A(コンボボーナス)。2 コンボ以上で 0.5
    pub combo_rate: f64,
    /// 中ディレイ(秒)。下限 0.3s 適用後
    pub value: f64,
    /// 下限 0.3s で頭打ちになったか
    pub floored: bool,
    /// wiki が「(固定)」と書いている中ディレイ(減少が効かない)
    pub fixed: bool,
    /// 中ディレイ減少の供給源の内訳
    pub contributions: Vec<ActualDelayContribution>,
}

/// 中ディレイを出す。`contributions` は減少値の供給源(Σ% の小数表現)。
pub fn actual_delay(
    base: f64,
    fixed: bool,
    contributions: Vec<ActualDelayContribution>,
    combo_count: u32,
) -> ActualDelay {
    let reduction_raw: f64 = contributions.iter().map(|c| c.rate).sum();
    // 「(固定)」の中ディレイには減少が乗らない(コンボボーナスは倍率A なので別枠)
    let reduction = if fixed { 0.0 } else { reduction_raw.min(ACTUAL_DELAY_REDUCTION_MAX) };
    let combo_rate =
        if combo_count >= COMBO_DELAY_THRESHOLD { COMBO_DELAY_RATE } else { 1.0 };
    let raw = base * (1.0 - reduction) * combo_rate;
    let value = raw.max(ACTUAL_DELAY_MIN);
    ActualDelay {
        base,
        reduction_raw,
        reduction,
        combo_rate,
        value,
        floored: value > raw,
        fixed,
        contributions,
    }
}

/// 中ディレイ減少をもたらすキャラのパッシブ・マスタリー(wiki: ステータス「中ディレイ倍率B」)。
/// 共通の供給源(フルスロットル / ランダムオプション / シエナのオーラ)は別経路で入るので、
/// このカタログはキャラ固有のものだけを持つ。実データは gamedata。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ActualDelaySkillDef {
    pub id: &'static str,
    pub name: &'static str,
    /// このスキルを持つキャラ(`GameCharacter::id`)
    pub game_character_id: &'static str,
    /// 選べる減少 %(1 段だけのパッシブは 1 要素。ミラのスパートは −25/−15/−5/−0 の 4 段)
    pub percents: &'static [f64],
    pub note: &'static str,
}

/// カタログ。呼び出しは `&ActualDelaySkillCatalog` = `&[ActualDelaySkillDef]`。
pub type ActualDelaySkillCatalog = [ActualDelaySkillDef];

/// キャラのパッシブ 1 件の選択。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualDelaySkillChoice {
    pub skill_id: String,
    /// `ActualDelaySkillDef::percents` のインデックス。1 段だけのパッシブは 0
    #[serde(default)]
    pub choice_index: usize,
}

/// キャラのパッシブによる中ディレイ減少の選択一式。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ActualDelaySkills {
    #[serde(default)]
    pub choices: Vec<ActualDelaySkillChoice>,
}

impl ActualDelaySkills {
    /// 選択を供給源の内訳に変換する。カタログに無い id・範囲外のインデックスは
    /// `validate` で弾いている前提で無視する。
    pub fn contributions(
        &self,
        catalog: &ActualDelaySkillCatalog,
    ) -> Vec<ActualDelayContribution> {
        self.choices
            .iter()
            .filter_map(|choice| {
                let def = catalog.iter().find(|d| d.id == choice.skill_id.as_str())?;
                let percent = *def.percents.get(choice.choice_index)?;
                (percent != 0.0).then(|| ActualDelayContribution {
                    source: def.name.to_string(),
                    rate: percent / 100.0,
                })
            })
            .collect()
    }

    /// カタログ参照・キャラ一致・インデックス範囲・重複を検証する。
    pub fn validate(
        &self,
        catalog: &ActualDelaySkillCatalog,
        game_character_id: &str,
    ) -> Result<(), ActualDelayError> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.choices.len());
        for choice in &self.choices {
            let def = catalog
                .iter()
                .find(|d| d.id == choice.skill_id.as_str())
                .ok_or_else(|| ActualDelayError::UnknownSkill { id: choice.skill_id.clone() })?;
            if def.game_character_id != game_character_id {
                return Err(ActualDelayError::ForeignCharacterSkill {
                    id: choice.skill_id.clone(),
                    game_character_id: game_character_id.to_string(),
                });
            }
            if choice.choice_index >= def.percents.len() {
                return Err(ActualDelayError::ChoiceOutOfRange { id: choice.skill_id.clone() });
            }
            if seen.contains(&def.id) {
                return Err(ActualDelayError::Duplicated { id: choice.skill_id.clone() });
            }
            seen.push(def.id);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum ActualDelayError {
    #[error("未知の中ディレイ減少スキルです: {id}")]
    UnknownSkill { id: String },
    #[error("中ディレイ減少スキル '{id}' はこのキャラ(game_character_id={game_character_id})のスキルではありません")]
    ForeignCharacterSkill { id: String, game_character_id: String },
    #[error("中ディレイ減少スキル '{id}' の選択肢が範囲外です")]
    ChoiceOutOfRange { id: String },
    #[error("中ディレイ減少スキル '{id}' が重複して選択されています")]
    Duplicated { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(source: &str, rate: f64) -> ActualDelayContribution {
        ActualDelayContribution { source: source.to_string(), rate }
    }

    // wiki `#ActualDelay`: 中ディレイ = 基本 × (1 − 減少値) × (2 コンボ以上なら 0.5)
    #[test]
    fn 減少値とコンボボーナスが掛かる() {
        let d = actual_delay(1.4, false, vec![c("フルスロットル", 0.45)], 0);
        assert!((d.value - 1.4 * 0.55).abs() < 1e-12);
        assert_eq!(d.combo_rate, 1.0);

        // 2 コンボ以上でさらに半分
        let d = actual_delay(1.4, false, vec![c("フルスロットル", 0.45)], 2);
        assert!((d.value - 1.4 * 0.55 * 0.5).abs() < 1e-12);
    }

    // wiki `#ActualDelay`: 減少値の上限 70%。コンボボーナスは対象外
    #[test]
    fn 減少値は70パーセントで頭打ち() {
        let d = actual_delay(1.4, false, vec![c("A", 0.45), c("B", 0.30), c("C", 0.05)], 0);
        assert!((d.reduction_raw - 0.80).abs() < 1e-12);
        assert_eq!(d.reduction, 0.70);
        assert!((d.value - 1.4 * 0.30).abs() < 1e-12);
    }

    // wiki `#ActualDelay`: 中ディレイの下限は 0.3s
    #[test]
    fn 下限は0_3秒() {
        let d = actual_delay(0.8, false, vec![c("A", 0.70)], 2);
        // 0.8 × 0.30 × 0.5 = 0.12 → 下限 0.3
        assert_eq!(d.value, ACTUAL_DELAY_MIN);
        assert!(d.floored);

        let d = actual_delay(1.4, false, Vec::new(), 0);
        assert!(!d.floored);
    }

    // wiki スキル性能一覧の「(固定)」は減少が効かない(極・ギガブレイズ 等)
    #[test]
    fn 固定の中ディレイには減少が乗らない() {
        let d = actual_delay(0.8, true, vec![c("フルスロットル", 0.45)], 0);
        assert_eq!(d.reduction, 0.0);
        assert!((d.value - 0.8).abs() < 1e-12);
        // コンボボーナス(倍率A)は固定でも掛かる
        let d = actual_delay(0.8, true, vec![c("フルスロットル", 0.45)], 3);
        assert!((d.value - 0.4).abs() < 1e-12);
    }

    const CATALOG: &[ActualDelaySkillDef] = &[
        ActualDelaySkillDef {
            id: "mira_spurt",
            name: "スパート",
            game_character_id: "mira",
            percents: &[25.0, 15.0, 5.0, 0.0],
            note: "",
        },
        ActualDelaySkillDef {
            id: "boris_sword_priest",
            name: "剣の司祭",
            game_character_id: "boris",
            percents: &[5.0],
            note: "",
        },
    ];

    #[test]
    fn 選択は内訳になり0パーセントは出さない() {
        let s = ActualDelaySkills {
            choices: vec![ActualDelaySkillChoice {
                skill_id: "mira_spurt".into(),
                choice_index: 0,
            }],
        };
        let contributions = s.contributions(CATALOG);
        assert_eq!(contributions.len(), 1);
        assert!((contributions[0].rate - 0.25).abs() < 1e-12);

        // −0% の段は内訳に出さない
        let s = ActualDelaySkills {
            choices: vec![ActualDelaySkillChoice {
                skill_id: "mira_spurt".into(),
                choice_index: 3,
            }],
        };
        assert!(s.contributions(CATALOG).is_empty());
    }

    #[test]
    fn 他キャラのスキルと未知idと範囲外と重複を弾く() {
        let foreign = ActualDelaySkills {
            choices: vec![ActualDelaySkillChoice { skill_id: "mira_spurt".into(), choice_index: 0 }],
        };
        assert!(foreign.validate(CATALOG, "mira").is_ok());
        assert!(matches!(
            foreign.validate(CATALOG, "boris"),
            Err(ActualDelayError::ForeignCharacterSkill { .. })
        ));

        let unknown = ActualDelaySkills {
            choices: vec![ActualDelaySkillChoice { skill_id: "nope".into(), choice_index: 0 }],
        };
        assert!(matches!(
            unknown.validate(CATALOG, "mira"),
            Err(ActualDelayError::UnknownSkill { .. })
        ));

        let out_of_range = ActualDelaySkills {
            choices: vec![ActualDelaySkillChoice { skill_id: "mira_spurt".into(), choice_index: 4 }],
        };
        assert!(matches!(
            out_of_range.validate(CATALOG, "mira"),
            Err(ActualDelayError::ChoiceOutOfRange { .. })
        ));

        let duplicated = ActualDelaySkills {
            choices: vec![
                ActualDelaySkillChoice { skill_id: "mira_spurt".into(), choice_index: 0 },
                ActualDelaySkillChoice { skill_id: "mira_spurt".into(), choice_index: 1 },
            ],
        };
        assert!(matches!(
            duplicated.validate(CATALOG, "mira"),
            Err(ActualDelayError::Duplicated { .. })
        ));
    }
}
