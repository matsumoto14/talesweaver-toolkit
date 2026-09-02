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
use crate::category::DamageCategory;
use crate::damage::DamageContribution;
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
    Damage {
        category: DamageCategory,
        percent: f64,
    },
    /// 命中P増加(wiki 計算式まとめ `#AccuracyPoint`: 射手のルーン・ハードウエポン(エアル)・
    /// 遊び用チンキ剤・テイルズウィーバーのエネルギー等)。値はそのまま命中Pへ加算する固定値
    AccuracyPoint {
        value: i64,
        /// この効果を無効にするキャラスキルの id(wiki 注記: テイルズウィーバーのエネルギーは
        /// 「極・的中剣」の効果中は無効)。`exclusive_slots` はバフ同士の排他しか表せないので、
        /// 効き先自体に持たせる
        exclusive_with: &'static [&'static str],
    },
    /// 最小回避率補正への加算(wiki `#HitRateCap`: 対人の命中率下限を上げる。
    /// テイルズウィーバーのエネルギーの「最小回避率 +10%」)。値は % 表記の整数
    MinEvasionRate { value: i64 },
    /// 命中P割合増加(wiki 計算式まとめ `#AccuracyPoint`「命中P割合増加」の的中剣枠)。
    /// SLv に比例して倍率が増える(`1 + per_level × Lv`)。割合とは別に SLv ごとの固定の
    /// 命中P変動 `shift[Lv-1]` を持つ(wiki の表)。SLv は `CharacterSkills::skill_levels` から
    /// 引き、上限は `CharacterSkillDef::max_level`(既存の `AccuracyPoint`(固定値の加算)とは別物)
    AccuracyRate {
        per_level: f64,
        shift: &'static [i64],
    },
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
            SkillEffect::Damage { category, percent } => {
                if category.is_percent_source() {
                    format!("{} +{percent}%", category.label())
                } else {
                    format!("{} +{percent}", category.label())
                }
            }
            SkillEffect::AccuracyPoint { value, .. } => format!("命中P +{value}"),
            SkillEffect::MinEvasionRate { value } => format!("最小回避率補正 +{value}%"),
            SkillEffect::AccuracyRate { per_level, .. } => {
                format!("命中P割合増加(SLv×{}%)", per_level * 100.0)
            }
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
    /// SLv の上限。on/off だけのスキルは 1。SLv を持つスキル(極・的中剣 = 7)は
    /// `CharacterSkills::skill_levels` で段階を選べる
    pub max_level: u8,
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

impl CharacterSkillDef {
    /// そのキャラが ON にできるスキルか(自分のスキル、または味方から受けるスキル)。
    pub fn applies_to(&self, game_character_id: &str) -> bool {
        self.audience == SkillAudience::Ally || self.game_character_id == game_character_id
    }
}

/// ON にしているキャラスキル。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CharacterSkills {
    /// `CharacterSkillDef::id`。重複は `validate` で弾く
    #[serde(default)]
    pub skill_ids: Vec<String>,
    /// SLv を持つスキル(`CharacterSkillDef::max_level > 1`)の `CharacterSkillDef::id` → SLv。
    /// ここに無い id は既定 Lv = 上限(`level_of`)。
    /// 追加フィールド(既存 JSON は無くても `#[serde(default)]` で読める。storage migration 不要)
    #[serde(default)]
    pub skill_levels: std::collections::BTreeMap<String, u8>,
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

    /// スキルの SLv。明示が無ければ上限(「ON = 満額の効果」。他のキャラスキルと同じ)。
    pub fn level_of(&self, def: &CharacterSkillDef) -> u8 {
        self.skill_levels
            .get(def.id)
            .copied()
            .unwrap_or(def.max_level)
            .clamp(1, def.max_level.max(1))
    }

    /// ON 中のスキルのうち命中P割合増加(`SkillEffect::AccuracyRate`)を持つものを、SLv まで
    /// 解決した `AccuracyBoost` にする。複数あれば倍率の高い方(通常は 1 つだけ)。無ければ None
    pub fn accuracy_boost(
        &self,
        catalog: &CharacterSkillCatalog,
        masteries: &Masteries,
    ) -> Option<crate::defense::AccuracyBoost> {
        self.resolved(catalog, masteries)
            .into_iter()
            .filter_map(|(def, _)| crate::defense::AccuracyBoost::from_skill(def, self.level_of(def)))
            .max_by(|a, b| a.rate.total_cmp(&b.rate))
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

    /// 与ダメージ式のカテゴリへの寄与。割合は Σ% の小数表現、固定値はそのまま。
    /// `source` はこのスキル名(マスタリーで差し替わっていても表示はスキル名のまま)
    pub fn damage_contributions(
        &self,
        catalog: &CharacterSkillCatalog,
        masteries: &Masteries,
    ) -> Vec<DamageContribution> {
        damage_contributions(
            self.resolved(catalog, masteries)
                .into_iter()
                .flat_map(|(def, effects)| effects.iter().map(move |e| (def.name.to_string(), e))),
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
                if let SkillEffect::StatRate {
                    stats,
                    percent,
                    layer,
                } = effect
                {
                    for kind in *stats {
                        out.push((*kind, percent / 100.0, *layer, def.name));
                    }
                }
            }
        }
        out
    }

    /// キャラ種を変えたときに残ってはいけない id(旧キャラ専用のスキル・カタログに無い id)を落とす。
    pub fn retain_applicable(&mut self, catalog: &CharacterSkillCatalog, game_character_id: &str) {
        self.skill_ids.retain(|id| {
            catalog
                .iter()
                .any(|d| d.id == id.as_str() && d.applies_to(game_character_id))
        });
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
            if !def.applies_to(game_character_id) {
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

/// 効き先の並びを与ダメージ式のカテゴリ寄与に畳む。バフ・キャラスキル・マスタリー・
/// 装備アビリティ・装備アイテムで共有する。`source` は人が読める供給源名
/// (スキル名・マスタリー名・バフ名・アビリティ名・アイテム名)
pub fn damage_contributions<'a>(
    effects: impl Iterator<Item = (String, &'a SkillEffect)>,
) -> Vec<DamageContribution> {
    effects
        .filter_map(|(source, e)| match e {
            // % 表記のカテゴリ(割合と E2)は小数に直す。実数の固定値カテゴリはそのまま
            SkillEffect::Damage { category, percent } => Some(DamageContribution {
                source,
                category: *category,
                value: if category.is_percent_source() {
                    percent / 100.0
                } else {
                    *percent
                },
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
    ForeignCharacter {
        id: String,
        game_character_id: String,
    },
    #[error("キャラスキル '{id}' が重複して選択されています")]
    Duplicated { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// E2「スキル倍率増加(固定値)」の供給源は wiki 上すべて %(兜アビリティ +1〜10% 等)。
    /// スキル倍率(D)と同じ目盛りなので 1/100 して入れる(旧リポ Excel v4.00 と一致)。
    #[test]
    fn e2は割合と同じくパーセントを小数へ直す() {
        let effect = SkillEffect::Damage {
            category: DamageCategory::SkillMultiplierFixed,
            percent: 10.0,
        };
        let contributions = damage_contributions([("E-スキル攻撃力増加".to_string(), &effect)].into_iter());
        assert_eq!(contributions.len(), 1);
        assert!((contributions[0].value - 0.10).abs() < 1e-12);
        assert_eq!(effect.label(), "スキル倍率増加(固定値) +10%");
    }

    /// 実数で持つ固定値カテゴリ(K・W)はそのまま。
    #[test]
    fn 実数の固定値カテゴリはそのまま入る() {
        let effect = SkillEffect::Damage {
            category: DamageCategory::FinalDamageFixed,
            percent: 500.0,
        };
        let contributions = damage_contributions([("テシスコア".to_string(), &effect)].into_iter());
        assert_eq!(contributions[0].value, 500.0);
    }

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
    const AGI_UP: &[SkillEffect] = &[SkillEffect::StatRate {
        stats: AGI,
        percent: 10.0,
        layer: StatLayer::MultiplierB,
    }];
    const ELITE_SWORDSMAN: &[SkillEffect] = &[SkillEffect::StatRate {
        stats: STAB_DEF,
        percent: 10.0,
        layer: StatLayer::MultiplierB,
    }];

    const CATALOG: &[CharacterSkillDef] = &[
        CharacterSkillDef {
            id: "mira_spurt",
            game_character_id: "mira",
            name: "極・スパート",
            audience: SkillAudience::SelfOnly,
            max_level: 1,
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
            max_level: 1,
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
            max_level: 1,
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
            max_level: 1,
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
    fn x4(contributions: &[DamageContribution]) -> f64 {
        contributions
            .iter()
            .filter(|c| c.category == DamageCategory::AttackDamageSkill)
            .map(|c| c.value)
            .sum()
    }

    fn on(ids: &[&str]) -> CharacterSkills {
        CharacterSkills {
            skill_ids: ids.iter().map(|s| s.to_string()).collect(),
            skill_levels: Default::default(),
        }
    }

    fn picked(ids: &[&str]) -> Masteries {
        Masteries {
            picked: ids.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn マスタリーがスキルの効果を差し替える() {
        let skills = on(&["mira_spurt"]);
        // 素のスパートは減衰するので中ディレイに入らない
        assert!(skills
            .actual_delay_contributions(CATALOG, &picked(&[]))
            .is_empty());
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
            (x4(&skills.damage_contributions(CATALOG, &picked(&["maximin_m3_3"]))) - 0.07).abs()
                < 1e-12
        );
        // スキルを ON にしていなければ、マスタリーを取っていても入らない
        assert_eq!(
            x4(&on(&[]).damage_contributions(CATALOG, &picked(&["maximin_m3_3"]))),
            0.0
        );
    }

    #[test]
    fn マスタリーを取ってはじめて効果が出るスキルがある() {
        let skills = on(&["joshua_possession_swordsman"]);
        assert!(skills.stat_rates(CATALOG, &picked(&[])).is_empty());
        assert_eq!(
            skills.stat_rates(CATALOG, &picked(&["joshua_m2_3"])),
            vec![
                (
                    StatKind::Stab,
                    0.10,
                    StatLayer::MultiplierB,
                    "憑依【剣闘士】"
                ),
                (
                    StatKind::Def,
                    0.10,
                    StatLayer::MultiplierB,
                    "憑依【剣闘士】"
                ),
            ]
        );
    }

    #[test]
    fn 味方スキルは誰でもonにできて自身のスキルは所有キャラだけ() {
        assert!(on(&["ispin_encourage"])
            .validate(CATALOG, "maximin")
            .is_ok());
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
