//! ランダムオプション(wiki: ランダムオプション。取得 2026-08-25)。
//!
//! 装備の部位ごとに付く追加効果。装備補正 9 値(`EquipmentValues`)には乗らず、
//! **与ダメージ式のカテゴリ**か**命中P/回避P**に直接効く。
//!
//! - `突き/斬り/物理複合/魔法/神聖/魔法斬り 攻撃力が X% 増加` は wiki の補足どおり
//!   「特定のスキルの与ダメージが X% 増加」= カテゴリP(`DependencyDamageRate`)。
//!   スキルの依存種別が一致したときだけ乗る
//! - `攻撃ダメージが X% 増加` はカテゴリX(`AttackDamageRate`)
//! - `命中率/回避率が X 増加` は命中P・回避P への加算(wiki `#AccuracyPoint` 末項の「ランダムOP」)
//!
//! 枠数は wiki に部位ごとの記載が無い。代わりに**同じカテゴリー番号は 1 部位に 1 つまで**という
//! 制約が転移の説明にある(「同じカテゴリーのオプションを共存させることは出来ず、転移させると
//! 優先的に上書きされる。ただし、カテゴリーなし(一覧表では 0 表記)はその限りではない」)。
//! この制約はカタログを引数で受ける `Equipment::validate_against_catalog` で検証する。

use serde::{Deserialize, Serialize};

use crate::skill::SkillDependency;

/// ランダムオプションのランク(wiki 一覧表の列)。上ほど効果値が大きい。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RandomOptionRank {
    Normal,
    Valuable,
    Rare,
    Special,
    /// S・真(転移不可)
    STrue,
}

impl RandomOptionRank {
    pub const ALL: [RandomOptionRank; 5] = [
        RandomOptionRank::Normal,
        RandomOptionRank::Valuable,
        RandomOptionRank::Rare,
        RandomOptionRank::Special,
        RandomOptionRank::STrue,
    ];
}

/// ランダムオプションの効き先。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RandomOptionEffect {
    /// 特定依存ダメージ増加(wiki: カテゴリP)。スキルの依存種別が一致したときだけ乗る
    DependencyDamageRate(SkillDependency),
    /// 攻撃ダメージ(wiki: カテゴリX)
    AttackDamageRate,
    /// 割合追加ダメージ(wiki: §5「新-割合」)。合計ダメージに乗る。
    /// **発動条件(ボス限定・確率・石の消費)は満たしている前提で常に効くものとして入れる**
    /// (ユーザー確認 2026-08-26)。条件は `note` に残す
    AddedDamageRate,
    /// 命中P への加算(wiki `#AccuracyPoint`: 命中P割合増加の計算後に加算)
    AccuracyPoint,
    /// 回避P への加算
    EvasionPoint,
    /// 命中P と 回避P の両方へ加算
    AccuracyAndEvasionPoint,
    /// 中ディレイ減少値(wiki: ステータス「中ディレイ倍率B」)への加算
    ActualDelayReduction,
    /// 計算には反映しない(発動条件付き・被ダメージ側・未実装の概念)。理由はカタログの `note`
    RecordOnly,
}

impl RandomOptionEffect {
    /// この効き先が計算に反映されるか。`false` = 記録するだけ。
    pub fn is_applied(self) -> bool {
        !matches!(self, RandomOptionEffect::RecordOnly)
    }
}

/// ランクごとの効果値レンジ(wiki 一覧表のセル `X=1〜2` 等)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RandomOptionTier {
    pub rank: RandomOptionRank,
    pub min: f64,
    pub max: f64,
}

/// ランダムオプション定義(gamedata がカタログを持つ。`EquipmentAbilityDef` と同じ依存方向)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RandomOptionDef {
    pub id: &'static str,
    /// wiki 一覧表の「オプション」列(X などのプレースホルダは実値に置き換えた表現)
    pub name: &'static str,
    /// 一覧のバッジに出す短い名前(「一般ボス」「魔攻」など)。名前をそのまま並べると読めない
    pub short: &'static str,
    /// この OP が付く部位
    pub slot: crate::equipment::PartSlot,
    /// wiki 一覧表の「カテゴリー」列。同じ番号は 1 部位に 1 つまで(0 は制約なし)
    pub category: u8,
    pub effect: RandomOptionEffect,
    /// ランクごとの効果値レンジ。wiki に載っているランクだけを持つ
    pub tiers: &'static [RandomOptionTier],
    /// 補足(発動条件・記録のみの理由・部位の別名など)
    pub note: &'static str,
    /// **実際によく付ける OP**。画面はこれをチップで先に出し、残りは奥に置く
    /// (ユーザー確認 2026-08-26)
    pub common: bool,
}

impl RandomOptionDef {
    pub fn tier(&self, rank: RandomOptionRank) -> Option<RandomOptionTier> {
        self.tiers.iter().copied().find(|t| t.rank == rank)
    }
}

/// キャラが実際に付けている 1 枠。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomOptionSlot {
    /// `RandomOptionDef::id`
    pub option_id: String,
    pub rank: RandomOptionRank,
    /// 実測値の上書き。`None` = レンジ上限(オプション変化石で振り直せるので想定値は最上値。
    /// 装備強化 +12 以上と同じ扱い。決定記録「2026-08-25 装備強化のレンジ倍率」)
    #[serde(default)]
    pub value: Option<f64>,
}

impl RandomOptionSlot {
    /// この枠の効果値。上書きが無ければレンジ上限。
    pub fn value(&self, def: &RandomOptionDef) -> f64 {
        self.value.unwrap_or_else(|| def.tier(self.rank).map_or(0.0, |t| t.max))
    }
}

/// 特定依存ダメージ増加(カテゴリP)の依存種別ごとの Σ%。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct DependencyRates {
    pub stab: f64,
    pub hack: f64,
    pub int: f64,
    pub mr: f64,
    pub stab_hack: f64,
    pub hack_int: f64,
}

impl DependencyRates {
    pub fn get(&self, dependency: SkillDependency) -> f64 {
        match dependency {
            SkillDependency::Stab => self.stab,
            SkillDependency::Hack => self.hack,
            SkillDependency::Int => self.int,
            SkillDependency::Mr => self.mr,
            SkillDependency::StabHack => self.stab_hack,
            SkillDependency::HackInt => self.hack_int,
        }
    }

    fn get_mut(&mut self, dependency: SkillDependency) -> &mut f64 {
        match dependency {
            SkillDependency::Stab => &mut self.stab,
            SkillDependency::Hack => &mut self.hack,
            SkillDependency::Int => &mut self.int,
            SkillDependency::Mr => &mut self.mr,
            SkillDependency::StabHack => &mut self.stab_hack,
            SkillDependency::HackInt => &mut self.hack_int,
        }
    }
}

/// 全部位のランダムオプションの集計。割合は Σ% の小数表現(+8% → 0.08)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RandomOptionTotals {
    /// カテゴリP(特定依存ダメージ増加)
    pub dependency_damage_rate: DependencyRates,
    /// カテゴリX(攻撃ダメージ)
    pub attack_damage_rate: f64,
    /// §5「新-割合」の割合追加ダメージ。Σ% の小数表現
    pub added_damage_rate: f64,
    /// 命中P への加算
    pub accuracy_point: i64,
    /// 回避P への加算
    pub evasion_point: i64,
    /// 中ディレイ減少値への加算。Σ% の小数表現
    pub actual_delay_reduction: f64,
    /// 計算に反映しなかった枠の数(UI が「記録するだけ」と出すのに使う)
    pub record_only_count: usize,
}

impl RandomOptionTotals {
    /// 1 枠を足しこむ。
    pub fn add(&mut self, def: &RandomOptionDef, slot: &RandomOptionSlot) {
        let value = slot.value(def);
        match def.effect {
            RandomOptionEffect::DependencyDamageRate(dependency) => {
                *self.dependency_damage_rate.get_mut(dependency) += value / 100.0;
            }
            RandomOptionEffect::AttackDamageRate => self.attack_damage_rate += value / 100.0,
            RandomOptionEffect::AddedDamageRate => self.added_damage_rate += value / 100.0,
            RandomOptionEffect::AccuracyPoint => self.accuracy_point += value as i64,
            RandomOptionEffect::EvasionPoint => self.evasion_point += value as i64,
            RandomOptionEffect::AccuracyAndEvasionPoint => {
                self.accuracy_point += value as i64;
                self.evasion_point += value as i64;
            }
            RandomOptionEffect::ActualDelayReduction => {
                self.actual_delay_reduction += value / 100.0;
            }
            RandomOptionEffect::RecordOnly => self.record_only_count += 1,
        }
    }
}

/// ランダムオプションの値域・部位制約違反。
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum RandomOptionError {
    #[error("{slot:?} はランダムオプションの対象外です(効果・AF 以外)")]
    NotAllowed { slot: crate::equipment::PartSlot },
    #[error("ランダムオプション '{option_id}' の効果値は 0〜{max} です(指定値 {value})")]
    ValueOutOfRange { option_id: String, value: f64, max: f64 },
    #[error("{slot:?} のランダムオプションは {max} 枠までです")]
    TooMany { slot: crate::equipment::PartSlot, max: usize },
}

/// ランダムオプションの効果値の上限(wiki に全 OP 共通の上限は無い。
/// 一覧表の最大が S・真の 48% なので、カスタム入力の安全域として暫定採用)`[仮]`。
pub const RANDOM_OPTION_VALUE_MAX: f64 = 100.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::PartSlot;

    const TIERS: &[RandomOptionTier] = &[
        RandomOptionTier { rank: RandomOptionRank::Rare, min: 6.0, max: 8.0 },
        RandomOptionTier { rank: RandomOptionRank::Special, min: 10.0, max: 25.0 },
    ];

    fn def(effect: RandomOptionEffect) -> RandomOptionDef {
        RandomOptionDef {
            id: "test",
            name: "テスト",
            slot: PartSlot::Shield,
            category: 15,
            effect,
            tiers: TIERS,
            note: "",
            common: false,
            short: "テスト",
        }
    }

    #[test]
    fn value_defaults_to_tier_max() {
        let d = def(RandomOptionEffect::AttackDamageRate);
        let slot = RandomOptionSlot {
            option_id: "test".into(),
            rank: RandomOptionRank::Special,
            value: None,
        };
        assert_eq!(slot.value(&d), 25.0);
    }

    #[test]
    fn value_override_wins() {
        let d = def(RandomOptionEffect::AttackDamageRate);
        let slot = RandomOptionSlot {
            option_id: "test".into(),
            rank: RandomOptionRank::Special,
            value: Some(12.0),
        };
        assert_eq!(slot.value(&d), 12.0);
    }

    #[test]
    fn dependency_rate_lands_on_matching_dependency_only() {
        let d = def(RandomOptionEffect::DependencyDamageRate(SkillDependency::Stab));
        let slot =
            RandomOptionSlot { option_id: "test".into(), rank: RandomOptionRank::Rare, value: None };
        let mut totals = RandomOptionTotals::default();
        totals.add(&d, &slot);
        assert_eq!(totals.dependency_damage_rate.get(SkillDependency::Stab), 0.08);
        assert_eq!(totals.dependency_damage_rate.get(SkillDependency::Hack), 0.0);
    }

    #[test]
    fn accuracy_and_evasion_lands_on_both() {
        let d = def(RandomOptionEffect::AccuracyAndEvasionPoint);
        let slot =
            RandomOptionSlot { option_id: "test".into(), rank: RandomOptionRank::Rare, value: None };
        let mut totals = RandomOptionTotals::default();
        totals.add(&d, &slot);
        assert_eq!(totals.accuracy_point, 8);
        assert_eq!(totals.evasion_point, 8);
    }

    #[test]
    fn record_only_is_counted_not_applied() {
        let d = def(RandomOptionEffect::RecordOnly);
        let slot = RandomOptionSlot {
            option_id: "test".into(),
            rank: RandomOptionRank::Special,
            value: None,
        };
        let mut totals = RandomOptionTotals::default();
        totals.add(&d, &slot);
        assert_eq!(totals.record_only_count, 1);
        assert_eq!(totals.attack_damage_rate, 0.0);
    }
}
