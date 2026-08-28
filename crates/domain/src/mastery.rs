//! キャラクタースキルのマスタリー(wiki: 各キャラの Skill ページ、スキル表の `P (M1)`〜`(M4)`)。
//!
//! マスタリーは**段(M1〜M4)ごとに 3 択で、1 つだけ選ぶ**。同じ段の選択肢は
//! 効き先がばらばらで、1 つの器には収まらない:
//!
//! | 例(ボリス M1) | 効き先 |
//! |---|---|
//! | 一閃 中ディレイ −5% | 中ディレイ倍率B |
//! | 斬撃 攻撃ダメージ +2% | カテゴリX(攻撃ダメージ) |
//! | 一撃 防御力 15% 無視確率 15% | 確率発動で未収録 |
//!
//! だから「バフ」「中ディレイ減少スキル」のようなカタログに散らすと、**段の排他が
//! カタログをまたいでしまい表せない**(実際、一閃だけが中ディレイのカタログに、
//! シルバースカル優勝者だけがバフカタログに入っていた)。ここで段ごと 1 つの型にする。
//!
//! 計算に入らない選択肢(防御側・条件付き)も**選べないと段の状態が表せない**ので、
//! `SkillEffect::RecordOnly` として持つ(ランダムOP のグレー枠と同じ扱い)。
//!
//! **段の数はキャラによって違う**(3〜5 段。リーチェだけ 15 段)ので定数では持たず、
//! カタログの `tier` から出す。
//!
//! **スキルの性能を書き換えるマスタリー**(ミラの【グッドフェイス】= 極・スパートの
//! 中ディレイ低下率を固定、マキシミンの M3 三択 = 極・呪われた魔剣の値を分岐)は
//! ここに効果を持たせない。効果はスキル側(`character_skill.rs` の `mastery_overrides`)が
//! 持ち、ここは `RecordOnly` にする — でないと同じ効果を二重に数えてしまう。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::character_skill::{damage_contributions, SkillEffect};
use crate::damage::DamageContribution;
use crate::stat_sources::StatLayer;
use crate::stats::StatKind;

/// マスタリー 1 つ(gamedata がカタログを持つ。`CharacterSkillDef` と同じ依存方向)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MasteryDef {
    pub id: &'static str,
    pub game_character_id: &'static str,
    /// 段(wiki のスキル表の No. 列)。キャラによって 3〜15 段
    pub tier: u8,
    pub name: &'static str,
    pub effect: SkillEffect,
    /// wiki の記述そのまま(記録のみの理由もここ)
    pub note: &'static str,
}

pub type MasteryCatalog = [MasteryDef];

/// 選んでいるマスタリー。**段ごとに 1 つ**なので、持つのは id の集合だけでよい。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Masteries {
    /// `MasteryDef::id`。段の重複はここでは防がず `validate` で弾く
    #[serde(default)]
    pub picked: Vec<String>,
}

impl Masteries {
    fn defs<'a>(&'a self, catalog: &'a MasteryCatalog) -> impl Iterator<Item = &'a MasteryDef> {
        self.picked
            .iter()
            .filter_map(|id| catalog.iter().find(|d| d.id == id.as_str()))
    }

    /// その段で選んでいるマスタリー。
    pub fn picked_in<'a>(&self, catalog: &'a MasteryCatalog, tier: u8) -> Option<&'a MasteryDef> {
        self.picked
            .iter()
            .filter_map(|id| catalog.iter().find(|d| d.id == id.as_str()))
            .find(|d| d.tier == tier)
    }

    /// 中ディレイ減少の合計(Σ% の小数表現)。
    pub fn actual_delay_reduction(&self, catalog: &MasteryCatalog) -> f64 {
        self.defs(catalog)
            .filter_map(|d| match d.effect {
                SkillEffect::ActualDelay { percent } => Some(percent / 100.0),
                _ => None,
            })
            .sum()
    }

    /// 与ダメージ式のカテゴリへの寄与。`source` は `マスタリー【名】`。
    pub fn damage_contributions(&self, catalog: &MasteryCatalog) -> Vec<DamageContribution> {
        damage_contributions(
            self.defs(catalog)
                .map(|d| (format!("マスタリー【{}】", d.name), &d.effect)),
        )
    }

    /// ステ増加への寄与(ステ, Σ% の小数表現, 層, マスタリー名)。
    pub fn stat_rates<'a>(
        &'a self,
        catalog: &'a MasteryCatalog,
    ) -> Vec<(StatKind, f64, StatLayer, &'a str)> {
        let mut out = Vec::new();
        for def in self.defs(catalog) {
            if let SkillEffect::StatRate {
                stats,
                percent,
                layer,
            } = def.effect
            {
                for kind in stats {
                    out.push((*kind, percent / 100.0, layer, def.name));
                }
            }
        }
        out
    }

    /// カタログ参照・キャラ一致・段の重複を検証する。
    pub fn validate(
        &self,
        catalog: &MasteryCatalog,
        game_character_id: &str,
    ) -> Result<(), MasteryError> {
        let mut seen_tiers: Vec<u8> = Vec::with_capacity(self.picked.len());
        for id in &self.picked {
            let def = catalog
                .iter()
                .find(|d| d.id == id.as_str())
                .ok_or_else(|| MasteryError::Unknown { id: id.clone() })?;
            if def.game_character_id != game_character_id {
                return Err(MasteryError::ForeignCharacter {
                    id: id.clone(),
                    game_character_id: game_character_id.to_string(),
                });
            }
            if seen_tiers.contains(&def.tier) {
                return Err(MasteryError::TierConflict { tier: def.tier });
            }
            seen_tiers.push(def.tier);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum MasteryError {
    #[error("未知のマスタリー '{id}' です")]
    Unknown { id: String },
    #[error(
        "マスタリー '{id}' はこのキャラ(game_character_id={game_character_id})のものではありません"
    )]
    ForeignCharacter {
        id: String,
        game_character_id: String,
    },
    #[error("マスタリー M{tier} は 1 つだけ選べます")]
    TierConflict { tier: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::DamageCategory;

    const CATALOG: &[MasteryDef] = &[
        MasteryDef {
            id: "boris_m1_issen",
            game_character_id: "boris",
            tier: 1,
            name: "一閃",
            effect: SkillEffect::ActualDelay { percent: 5.0 },
            note: "",
        },
        MasteryDef {
            id: "boris_m1_zangeki",
            game_character_id: "boris",
            tier: 1,
            name: "斬撃",
            effect: SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 2.0,
            },
            note: "",
        },
        MasteryDef {
            id: "boris_m2_silver_skull",
            game_character_id: "boris",
            tier: 2,
            name: "シルバースカル優勝者",
            effect: SkillEffect::StatRate {
                stats: &[StatKind::Hack, StatKind::Def],
                percent: 10.0,
                layer: StatLayer::MultiplierB,
            },
            note: "",
        },
        MasteryDef {
            id: "boris_m2_survivor",
            game_character_id: "boris",
            tier: 2,
            name: "抗争の生存者",
            effect: SkillEffect::RecordOnly,
            note: "被ダメージ -3%",
        },
    ];

    /// テスト用: カテゴリX4(攻撃ダメージ(スキル))の合計。
    fn x4(contributions: &[DamageContribution]) -> f64 {
        contributions
            .iter()
            .filter(|c| c.category == DamageCategory::AttackDamageSkill)
            .map(|c| c.value)
            .sum()
    }

    fn picked(ids: &[&str]) -> Masteries {
        Masteries {
            picked: ids.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn 段ごとに効き先が分かれて合流する() {
        let m = picked(&["boris_m1_issen", "boris_m2_silver_skull"]);
        assert!((m.actual_delay_reduction(CATALOG) - 0.05).abs() < 1e-12);
        assert_eq!(x4(&m.damage_contributions(CATALOG)), 0.0);
        assert_eq!(
            m.stat_rates(CATALOG),
            vec![
                (
                    StatKind::Hack,
                    0.10,
                    StatLayer::MultiplierB,
                    "シルバースカル優勝者"
                ),
                (
                    StatKind::Def,
                    0.10,
                    StatLayer::MultiplierB,
                    "シルバースカル優勝者"
                ),
            ]
        );

        let m = picked(&["boris_m1_zangeki"]);
        assert_eq!(m.actual_delay_reduction(CATALOG), 0.0);
        assert!((x4(&m.damage_contributions(CATALOG)) - 0.02).abs() < 1e-12);
    }

    #[test]
    fn 記録のみは合計に入らないが段は埋まる() {
        let m = picked(&["boris_m2_survivor"]);
        assert_eq!(x4(&m.damage_contributions(CATALOG)), 0.0);
        assert_eq!(m.stat_rates(CATALOG), vec![]);
        assert_eq!(
            m.picked_in(CATALOG, 2).map(|d| d.id),
            Some("boris_m2_survivor")
        );
        assert!(m.validate(CATALOG, "boris").is_ok());
    }

    #[test]
    fn 同じ段は1つしか選べない() {
        let m = picked(&["boris_m1_issen", "boris_m1_zangeki"]);
        assert!(matches!(
            m.validate(CATALOG, "boris"),
            Err(MasteryError::TierConflict { tier: 1 })
        ));
    }

    #[test]
    fn 他キャラのマスタリーと未知のidは弾く() {
        let m = picked(&["boris_m1_issen"]);
        assert!(matches!(
            m.validate(CATALOG, "mira"),
            Err(MasteryError::ForeignCharacter { .. })
        ));
        let m = picked(&["nope"]);
        assert!(matches!(
            m.validate(CATALOG, "boris"),
            Err(MasteryError::Unknown { .. })
        ));
    }
}
