//! キャラスキル(wiki: 各キャラの Skill ページ、ステータス「能力値増加/減少カテゴリー」/
//! 「与ダメージ計算式・ダメージ増加/減少カテゴリー」/「中ディレイ倍率B」)。
//!
//! キャラが持つ**パッシブ・自己バフ・味方バフ**を 1 つのカタログにまとめる。以前は
//! 効き先ごとに器が分かれていた(ステ上昇はバフカタログ、中ディレイ減少は専用カタログ)ため、
//! **同じスキルが効き先の数だけ散る**問題があった。極・スパートは「AGI +10% + 中ディレイ減少」、
//! 極・呪われた魔剣は「攻撃ダメージ +5% + 被ダメージ +5%」で、どちらも 1 つの器に収まらない。
//!
//! ## マスタリーはスキルの性能を書き換える
//!
//! wiki の各カテゴリ表は「スキルの行 + マスタリーで値が分岐する子行」という形で書かれている:
//!
//! ```text
//! [X4]攻撃ダメージ(スキル)
//! |マキシミン|呪われた魔剣|+5%|マスタリー【呪われた魔剣】|
//! |~        |~          |+5%|マスタリー【封印された魔剣】|
//! |~        |~          |+7%|マスタリー【自我を持つ魔剣】|
//! ```
//!
//! だから効果は**スキル側**が持ち、マスタリーは `mastery_overrides` の選択子として働く。
//! 逆(マスタリー側に効果を持たせる)にすると、ミラの極・スパートで実際に起きたように
//! 「スキルはこのカタログ、そのマスタリーは別カタログで `RecordOnly`」とコメントで
//! 繋ぐしかなくなり、依存が読めなくなる。
//!
//! **マスタリーは攻撃スキルの性能も書き換える**(ノクターンの【クイックチャージ】は
//! `<レーザーカノン>` のチャージ速度と攻撃ダメージを変える)。その場合も同じ形 —
//! 効果は `Skill` 側、マスタリーは選択子 — で足せる。いまは該当データがすべて未収録
//! (`RecordOnly`)なので `Skill` にはフィールドを足さない。
//!
//! **マスタリーだけが持つ効果**(ボリスの【斬撃】= 攻撃ダメージ +2% のようにスキルを
//! 名指ししないもの)は `mastery.rs` 側が直接 `SkillEffect` を持つ。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::actual_delay::ActualDelayContribution;
use crate::category::{CategoryKind, DamageCategory};
use crate::mastery::Masteries;
use crate::stat_sources::StatLayer;
use crate::stats::StatKind;

/// スキル・マスタリーの効き先。1 つのスキルが複数持つ(スパート = AGI + 中ディレイ)。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillEffect {
    /// 能力値の増加。層(割合増加 / 倍率B 等)は供給源ごとに違うので明示して持つ
    StatRate {
        stats: &'static [StatKind],
        /// % 表記(10.0 = +10%)
        percent: f64,
        layer: StatLayer,
    },
    /// 中ディレイ減少 %(wiki: ステータス「中ディレイ倍率B」)
    ActualDelay { percent: f64 },
    /// 与ダメージ式のカテゴリへの加算。**上限はカテゴリ側が持つ**(`DamageCategory::cap`)。
    ///
    /// 割合カテゴリは % 表記(5.0 = +5%)、固定値カテゴリはその値そのもの。
    /// 攻撃ダメージ(X)は X1〜X6 で上限が違うので、どの副カテゴリかまで指定する
    Damage { category: DamageCategory, percent: f64 },
    /// **記録するだけ**。wiki に効果はあるが、まだ配線していない
    /// (被ダメージ・移動速度・確率発動・条件付き・減衰する値)
    RecordOnly,
}

impl SkillEffect {
    /// 与ダメージ・能力値に入るか。`false` は記録するだけ。
    pub fn is_modeled(&self) -> bool {
        !matches!(self, SkillEffect::RecordOnly)
    }

    /// 画面に出す効果の要約。
    pub fn label(&self) -> String {
        match self {
            SkillEffect::StatRate { stats, percent, .. } => {
                let names: Vec<&str> = stats.iter().map(|k| k.label()).collect();
                format!("{} +{percent}%", names.join(" / "))
            }
            SkillEffect::ActualDelay { percent } => format!("中ディレイ −{percent}%"),
            SkillEffect::Damage { category, percent } => match category.kind() {
                CategoryKind::Fixed => format!("{} +{percent}", category.label()),
                _ => format!("{} +{percent}%", category.label()),
            },
            SkillEffect::RecordOnly => "記録のみ".to_string(),
        }
    }
}

/// 誰に効くか。味方スキルは**誰でも ON にできる**(同行者が使う前提)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillAudience {
    /// 自身のみ。所有キャラでしか ON にできない
    SelfOnly,
    /// 味方にも掛かる
    Ally,
}

/// マスタリーによる効果の差し替え(wiki のカテゴリ表の子行)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MasteryOverride {
    /// `MasteryDef::id`
    pub mastery_id: &'static str,
    /// このマスタリーを取っているときの効果(基本効果を**置き換える**)
    pub effects: &'static [SkillEffect],
}

/// キャラスキル 1 つ。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CharacterSkillDef {
    pub id: &'static str,
    /// このスキルを持つキャラ(`GameCharacter::id`)。味方スキルでも**使う側の**キャラ
    pub game_character_id: &'static str,
    pub name: &'static str,
    pub audience: SkillAudience,
    /// マスタリー未取得のときの効果。空 = マスタリーを取ってはじめて効果が出る
    pub effects: &'static [SkillEffect],
    /// マスタリーを取ると効果が差し替わる。上から順に見て最初に一致したものを使う
    pub mastery_overrides: &'static [MasteryOverride],
    pub source_url: &'static str,
    pub note: &'static str,
}

impl CharacterSkillDef {
    /// 選んでいるマスタリーを踏まえた実際の効果。
    ///
    /// 味方スキルは**相手のマスタリーが分からない**ので差し替えを見ない(基本効果のまま)。
    pub fn effects(&self, masteries: &Masteries) -> &'static [SkillEffect] {
        if self.audience == SkillAudience::Ally {
            return self.effects;
        }
        self.mastery_overrides
            .iter()
            .find(|o| masteries.picked.iter().any(|id| id == o.mastery_id))
            .map(|o| o.effects)
            .unwrap_or(self.effects)
    }
}

/// カタログ。呼び出しは `&CharacterSkillCatalog` = `&[CharacterSkillDef]`。
pub type CharacterSkillCatalog = [CharacterSkillDef];

/// ON にしているキャラスキル。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CharacterSkills {
    /// `CharacterSkillDef::id`。重複は `validate` で弾く
    #[serde(default)]
    pub skill_ids: Vec<String>,
}

impl CharacterSkills {
    /// ON にしているスキルの定義と、マスタリー適用後の効果。
    fn resolved<'a>(
        &self,
        catalog: &'a CharacterSkillCatalog,
        masteries: &Masteries,
    ) -> Vec<(&'a CharacterSkillDef, &'static [SkillEffect])> {
        self.skill_ids
            .iter()
            .filter_map(|id| catalog.iter().find(|d| d.id == id.as_str()))
            .map(|def| (def, def.effects(masteries)))
            .collect()
    }

    /// 中ディレイ減少の供給源(Σ% の小数表現)。
    pub fn actual_delay_contributions(
        &self,
        catalog: &CharacterSkillCatalog,
        masteries: &Masteries,
    ) -> Vec<ActualDelayContribution> {
        let mut out = Vec::new();
        for (def, effects) in self.resolved(catalog, masteries) {
            for effect in effects {
                if let SkillEffect::ActualDelay { percent } = effect {
                    out.push(ActualDelayContribution {
                        source: def.name.to_string(),
                        rate: percent / 100.0,
                    });
                }
            }
        }
        out
    }

    /// 与ダメージ式のカテゴリへの寄与(カテゴリ, 値)。割合は Σ% の小数表現、固定値はそのまま。
    pub fn damage_contributions(
        &self,
        catalog: &CharacterSkillCatalog,
        masteries: &Masteries,
    ) -> Vec<(DamageCategory, f64)> {
        damage_contributions(
            self.resolved(catalog, masteries).iter().flat_map(|(_, effects)| effects.iter()),
        )
    }

    /// ステ増加への寄与(ステ, Σ% の小数表現, 層, スキル名)。
    pub fn stat_rates<'a>(
        &self,
        catalog: &'a CharacterSkillCatalog,
        masteries: &Masteries,
    ) -> Vec<(StatKind, f64, StatLayer, &'a str)> {
        let mut out = Vec::new();
        for (def, effects) in self.resolved(catalog, masteries) {
            for effect in effects {
                if let SkillEffect::StatRate { stats, percent, layer } = effect {
                    for kind in *stats {
                        out.push((*kind, percent / 100.0, *layer, def.name));
                    }
                }
            }
        }
        out
    }

    /// カタログ参照・キャラ一致・重複を検証する。味方スキルは誰でも ON にできる。
    pub fn validate(
        &self,
        catalog: &CharacterSkillCatalog,
        game_character_id: &str,
    ) -> Result<(), CharacterSkillError> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.skill_ids.len());
        for id in &self.skill_ids {
            let def = catalog
                .iter()
                .find(|d| d.id == id.as_str())
                .ok_or_else(|| CharacterSkillError::Unknown { id: id.clone() })?;
            if def.audience == SkillAudience::SelfOnly
                && def.game_character_id != game_character_id
            {
                return Err(CharacterSkillError::ForeignCharacter {
                    id: id.clone(),
                    game_character_id: game_character_id.to_string(),
                });
            }
            if seen.contains(&def.id) {
                return Err(CharacterSkillError::Duplicated { id: id.clone() });
            }
            seen.push(def.id);
        }
        Ok(())
    }
}

/// 効き先の並びを与ダメージ式のカテゴリ寄与に畳む。バフ・キャラスキルで共有する。
pub fn damage_contributions<'a>(
    effects: impl Iterator<Item = &'a SkillEffect>,
) -> Vec<(DamageCategory, f64)> {
    effects
        .filter_map(|e| match e {
            // 割合カテゴリは % 表記なので小数に直す。固定値カテゴリはそのまま
            SkillEffect::Damage { category, percent } => Some(match category.kind() {
                CategoryKind::Rate => (*category, percent / 100.0),
                _ => (*category, *percent),
            }),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum CharacterSkillError {
    #[error("未知のキャラスキルです: {id}")]
    Unknown { id: String },
    #[error("キャラスキル '{id}' はこのキャラ(game_character_id={game_character_id})のスキルではありません")]
    ForeignCharacter { id: String, game_character_id: String },
    #[error("キャラスキル '{id}' が重複して選択されています")]
    Duplicated { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGI: &[StatKind] = &[StatKind::Agi];
    const STAB_DEF: &[StatKind] = &[StatKind::Stab, StatKind::Def];

    /// ミラの極・スパート。素は減衰する中ディレイ減少なので記録のみ、
    /// マスタリー【グッドフェイス】を取ると 5% 固定になる(wiki ステータス「中ディレイ倍率B」)。
    const SPURT_GOOD_FACE: &[SkillEffect] = &[SkillEffect::ActualDelay { percent: 5.0 }];
    /// マキシミンの極・呪われた魔剣。M3 の三択で +5% / +5% / +7% に分岐する。
    const CURSED_BASE: &[SkillEffect] = &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 5.0,
            }];
    const CURSED_EGO: &[SkillEffect] = &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 7.0,
            }];
    const AGI_UP: &[SkillEffect] =
        &[SkillEffect::StatRate { stats: AGI, percent: 10.0, layer: StatLayer::MultiplierB }];
    const ELITE_SWORDSMAN: &[SkillEffect] =
        &[SkillEffect::StatRate { stats: STAB_DEF, percent: 10.0, layer: StatLayer::MultiplierB }];

    const CATALOG: &[CharacterSkillDef] = &[
        CharacterSkillDef {
            id: "mira_spurt",
            game_character_id: "mira",
            name: "極・スパート",
            audience: SkillAudience::SelfOnly,
            effects: &[SkillEffect::RecordOnly],
            mastery_overrides: &[MasteryOverride {
                mastery_id: "mira_m4_2",
                effects: SPURT_GOOD_FACE,
            }],
            source_url: "",
            note: "",
        },
        CharacterSkillDef {
            id: "maximin_cursed_sword",
            game_character_id: "maximin",
            name: "極・呪われた魔剣",
            audience: SkillAudience::SelfOnly,
            effects: CURSED_BASE,
            mastery_overrides: &[MasteryOverride {
                mastery_id: "maximin_m3_3",
                effects: CURSED_EGO,
            }],
            source_url: "",
            note: "",
        },
        CharacterSkillDef {
            id: "ispin_encourage",
            game_character_id: "ispin",
            name: "極・エンカレッジ",
            audience: SkillAudience::Ally,
            effects: AGI_UP,
            mastery_overrides: &[],
            source_url: "",
            note: "",
        },
        // マスタリーを取ってはじめて効果が出るスキル(ジョシュアの憑依モード)
        CharacterSkillDef {
            id: "joshua_possession_swordsman",
            game_character_id: "joshua",
            name: "憑依【剣闘士】",
            audience: SkillAudience::SelfOnly,
            effects: &[],
            mastery_overrides: &[MasteryOverride {
                mastery_id: "joshua_m2_3",
                effects: ELITE_SWORDSMAN,
            }],
            source_url: "",
            note: "",
        },
    ];

    /// テスト用: カテゴリX4(攻撃ダメージ(スキル))の合計。
    fn x4(rates: &[(DamageCategory, f64)]) -> f64 {
        rates.iter().filter(|(c, _)| *c == DamageCategory::AttackDamageSkill).map(|(_, v)| v).sum()
    }

    fn on(ids: &[&str]) -> CharacterSkills {
        CharacterSkills { skill_ids: ids.iter().map(|s| s.to_string()).collect() }
    }

    fn picked(ids: &[&str]) -> Masteries {
        Masteries { picked: ids.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn マスタリーがスキルの効果を差し替える() {
        let skills = on(&["mira_spurt"]);
        // 素のスパートは減衰するので中ディレイに入らない
        assert!(skills.actual_delay_contributions(CATALOG, &picked(&[])).is_empty());
        // 【グッドフェイス】で 5% 固定になる
        let c = skills.actual_delay_contributions(CATALOG, &picked(&["mira_m4_2"]));
        assert_eq!(c.len(), 1);
        assert!((c[0].rate - 0.05).abs() < 1e-12);
        assert_eq!(c[0].source, "極・スパート");
        // 同じ段の別の選択肢では差し替わらない
        assert!(skills
            .actual_delay_contributions(CATALOG, &picked(&["mira_m4_1"]))
            .is_empty());
    }

    #[test]
    fn 呪われた魔剣はm3の三択で値が変わる() {
        let skills = on(&["maximin_cursed_sword"]);
        assert!((x4(&skills.damage_contributions(CATALOG, &picked(&[]))) - 0.05).abs() < 1e-12);
        assert!(
            (x4(&skills.damage_contributions(CATALOG, &picked(&["maximin_m3_3"]))) - 0.07).abs() < 1e-12
        );
        // スキルを ON にしていなければ、マスタリーを取っていても入らない
        assert_eq!(x4(&on(&[]).damage_contributions(CATALOG, &picked(&["maximin_m3_3"]))), 0.0);
    }

    #[test]
    fn マスタリーを取ってはじめて効果が出るスキルがある() {
        let skills = on(&["joshua_possession_swordsman"]);
        assert!(skills.stat_rates(CATALOG, &picked(&[])).is_empty());
        assert_eq!(
            skills.stat_rates(CATALOG, &picked(&["joshua_m2_3"])),
            vec![
                (StatKind::Stab, 0.10, StatLayer::MultiplierB, "憑依【剣闘士】"),
                (StatKind::Def, 0.10, StatLayer::MultiplierB, "憑依【剣闘士】"),
            ]
        );
    }

    #[test]
    fn 味方スキルは誰でもonにできて自身のスキルは所有キャラだけ() {
        assert!(on(&["ispin_encourage"]).validate(CATALOG, "maximin").is_ok());
        assert!(matches!(
            on(&["mira_spurt"]).validate(CATALOG, "maximin"),
            Err(CharacterSkillError::ForeignCharacter { .. })
        ));
        assert!(matches!(
            on(&["nope"]).validate(CATALOG, "mira"),
            Err(CharacterSkillError::Unknown { .. })
        ));
        assert!(matches!(
            on(&["mira_spurt", "mira_spurt"]).validate(CATALOG, "mira"),
            Err(CharacterSkillError::Duplicated { .. })
        ));
    }

    /// 味方スキルは相手のマスタリーが分からないので差し替えを見ない。
    #[test]
    fn 味方スキルはマスタリーで差し替わらない() {
        let skills = on(&["ispin_encourage"]);
        let rates = skills.stat_rates(CATALOG, &picked(&["mira_m4_2"]));
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].0, StatKind::Agi);
    }
}
