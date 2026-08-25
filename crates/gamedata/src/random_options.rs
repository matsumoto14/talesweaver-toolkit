//! ランダムオプションのカタログ。
//!
//! 出典: wiki「ランダムオプション」(取得 2026-08-25)。部位ごとの節 + 転移の説明。
//!
//! **収録範囲は火力・命中・回避に関係する OP だけ**。wiki の一覧は HP/MP/移動速度/経験値/変身/
//! 効果音まで含む数百件だが、それらは計算にも記録にも使いようが無いので入れない。
//! 発動条件付き(後方から・ボス限定など)や、まだ実装していない概念(中ディレイ・最小回避率補正・
//! 被ダメージ側)に効く OP は `RandomOptionEffect::RecordOnly` で入れて「記録するだけ」と出す。
//!
//! 部位名の対応(wiki の節名 → `PartSlot`):
//! - 「サブアーム」= 盾(`Shield`)
//! - 「サブアーム(SHOW)」= 盾+ / カフス(`ShieldPlus`。Show 装備だが補正が設定されている装備)
//! - 「レリック(右)」「レリック(左)」= どちらも `Relic`。カテゴリー番号が重ならないので
//!   1 つの部位に両方を持たせても「同じカテゴリーは 1 つまで」の制約は正しく効く

use domain::{
    PartSlot, RandomOptionDef, RandomOptionEffect, RandomOptionRank, RandomOptionTier,
    SkillDependency,
};

use crate::Source;

/// ランダムオプションカタログの出典。
pub const RANDOM_OPTION_SOURCE: Source = Source {
    page: "ランダムオプション",
    retrieved_on: "2026-08-25",
    note: "火力・命中・回避に関係する OP のみ収録。枠数は wiki に記載が無く、\
           制約は「同じカテゴリーは 1 部位に 1 つまで(カテゴリー 0 は除く)」(転移の説明)",
};

use RandomOptionEffect::{
    AccuracyAndEvasionPoint, AccuracyPoint, ActualDelayReduction, AttackDamageRate,
    DependencyDamageRate, EvasionPoint, RecordOnly,
};
use RandomOptionRank::{Normal, Rare, STrue, Special, Valuable};

const fn tier(rank: RandomOptionRank, min: f64, max: f64) -> RandomOptionTier {
    RandomOptionTier { rank, min, max }
}

/// 盾(サブアーム)カテゴリー15 の依存別攻撃力増加。6 種すべて同じレンジ。
const SHIELD_DEPENDENCY_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 1.0, 2.0),
    tier(Valuable, 3.0, 5.0),
    tier(Rare, 6.0, 8.0),
    tier(Special, 10.0, 25.0),
    tier(STrue, 15.0, 28.0),
];

/// レリック(右)カテゴリー15。依存別攻撃力増加と攻撃ダメージ増加が同じレンジ。
const RELIC_CATEGORY15_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 3.0, 4.0), tier(Rare, 5.0, 7.0), tier(Special, 8.0, 10.0)];

/// レリック(左)カテゴリー3 / カテゴリー10。
const RELIC_RESISTANCE_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 3.0, 4.0), tier(Rare, 5.0, 7.0), tier(Special, 8.0, 10.0)];
const RELIC_ACCURACY_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 3.0, 5.0), tier(Rare, 6.0, 10.0), tier(Special, 11.0, 15.0)];

const SHIELD_ATTACK_DAMAGE_TIERS: &[RandomOptionTier] = &[
    tier(Valuable, 5.0, 10.0),
    tier(Rare, 15.0, 20.0),
    tier(Special, 25.0, 30.0),
    tier(STrue, 25.0, 33.0),
];

const ARMOR_RESISTANCE_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 1.0, 2.0),
    tier(Valuable, 3.0, 4.0),
    tier(Rare, 5.0, 7.0),
    tier(Special, 10.0, 15.0),
];

const HAND_ACCURACY_TIERS: &[RandomOptionTier] =
    &[tier(Special, 10.0, 15.0), tier(STrue, 15.0, 20.0)];
const HAND_EVASION_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 1.0, 2.0),
    tier(Valuable, 3.0, 4.0),
    tier(Rare, 5.0, 7.0),
    tier(Special, 10.0, 15.0),
];
const HAND_BOTH_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 1.0, 1.0),
    tier(Valuable, 2.0, 2.0),
    tier(Rare, 3.0, 4.0),
    tier(Special, 5.0, 7.0),
    tier(STrue, 7.0, 10.0),
];

/// 単一ランクだけの OP。
const SHIELD_FIXED_EVASION_TIERS: &[RandomOptionTier] = &[tier(Special, 3.0, 5.0)];
const CUFFS_ACTUAL_DELAY_TIERS: &[RandomOptionTier] = &[tier(Special, 1.0, 3.0)];
const ARMOR_FIXED_EVASION_TIERS: &[RandomOptionTier] = &[tier(Special, 5.0, 10.0)];
const HAND_MAX_EVASION_RATE_TIERS: &[RandomOptionTier] = &[tier(Special, 1.0, 3.0)];

const WEAPON_BOSS_TIERS: &[RandomOptionTier] = &[
    tier(Valuable, 2.0, 3.0),
    tier(Rare, 5.0, 10.0),
    tier(Special, 15.0, 18.0),
    tier(STrue, 15.0, 21.0),
];
const WEAPON_RAID_BOSS_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 2.0, 3.0), tier(Rare, 5.0, 10.0), tier(Special, 15.0, 18.0)];
const WEAPON_BACK_ATTACK_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 1.0, 3.0),
    tier(Valuable, 4.0, 5.0),
    tier(Rare, 5.0, 10.0),
    tier(Special, 9.0, 10.0),
];
const WEAPON_MELEE_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 5.0, 6.0),
    tier(Valuable, 7.0, 8.0),
    tier(Rare, 9.0, 10.0),
    tier(Special, 11.0, 12.0),
];

/// 依存別攻撃力増加の補足(wiki の補足列そのまま)。
const DEPENDENCY_NOTE: &str =
    "wiki 補足「特定のスキルの与ダメージが X% 増加」= カテゴリP。依存種別が一致するスキルにだけ乗る";

const fn def(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    category: u8,
    effect: RandomOptionEffect,
    tiers: &'static [RandomOptionTier],
    note: &'static str,
) -> RandomOptionDef {
    RandomOptionDef { id, name, slot, category, effect, tiers, note }
}

pub fn random_option_catalog() -> Vec<RandomOptionDef> {
    vec![
        // --- 盾(サブアーム)------------------------------------------------
        def(
            "shield-attack-damage",
            "攻撃ダメージが増加(被ダメージも増加)",
            PartSlot::Shield,
            15,
            AttackDamageRate,
            SHIELD_ATTACK_DAMAGE_TIERS,
            "被ダメージ増加(Valuable 5〜10% / Rare 10〜15% / Special・S真 20〜25%)は被ダメージ計算が無いので反映しない",
        ),
        def(
            "shield-thrust-rate",
            "突き攻撃力が増加",
            PartSlot::Shield,
            15,
            DependencyDamageRate(SkillDependency::Stab),
            SHIELD_DEPENDENCY_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "shield-slash-rate",
            "斬り攻撃力が増加",
            PartSlot::Shield,
            15,
            DependencyDamageRate(SkillDependency::Hack),
            SHIELD_DEPENDENCY_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "shield-physical-composite-rate",
            "物理複合攻撃力が増加",
            PartSlot::Shield,
            15,
            DependencyDamageRate(SkillDependency::StabHack),
            SHIELD_DEPENDENCY_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "shield-magic-rate",
            "魔法攻撃力が増加",
            PartSlot::Shield,
            15,
            DependencyDamageRate(SkillDependency::Int),
            SHIELD_DEPENDENCY_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "shield-holy-rate",
            "神聖攻撃力が増加",
            PartSlot::Shield,
            15,
            DependencyDamageRate(SkillDependency::Mr),
            SHIELD_DEPENDENCY_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "shield-magic-slash-rate",
            "魔法斬り攻撃力が増加",
            PartSlot::Shield,
            15,
            DependencyDamageRate(SkillDependency::HackInt),
            SHIELD_DEPENDENCY_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "shield-fixed-evasion",
            "固定回避が増加",
            PartSlot::Shield,
            8,
            RecordOnly,
            SHIELD_FIXED_EVASION_TIERS,
            "バンド Lv275〜。最小回避率補正だが通常回避「率」を出していないので記録のみ",
        ),
        // --- 盾+(カフス。wiki の節名は「サブアーム(SHOW)」)-----------------
        def(
            "cuffs-actual-delay",
            "スキルの中ディレイが減少",
            PartSlot::ShieldPlus,
            0,
            ActualDelayReduction,
            CUFFS_ACTUAL_DELAY_TIERS,
            "Lv310〜。中ディレイ倍率B の減少(wiki: ステータス「中ディレイ倍率B」)",
        ),
        // --- 鎧 ---------------------------------------------------------
        def(
            "armor-damage-resistance",
            "ダメージ耐性が増加",
            PartSlot::Armor,
            3,
            RecordOnly,
            ARMOR_RESISTANCE_TIERS,
            "カテゴリU。被ダメージ計算が無いので記録のみ",
        ),
        def(
            "armor-fixed-evasion",
            "固定回避が増加",
            PartSlot::Armor,
            3,
            RecordOnly,
            ARMOR_FIXED_EVASION_TIERS,
            "スーツ Lv275〜。最小回避率補正だが通常回避「率」を出していないので記録のみ",
        ),
        // --- 手 ---------------------------------------------------------
        def(
            "hand-accuracy",
            "命中率が増加",
            PartSlot::Hand,
            10,
            AccuracyPoint,
            HAND_ACCURACY_TIERS,
            "wiki 注記「命中P割合増加計算後に加算」",
        ),
        def(
            "hand-evasion",
            "回避率が増加",
            PartSlot::Hand,
            10,
            EvasionPoint,
            HAND_EVASION_TIERS,
            "",
        ),
        def(
            "hand-accuracy-evasion",
            "回避率と命中率が増加",
            PartSlot::Hand,
            10,
            AccuracyAndEvasionPoint,
            HAND_BOTH_TIERS,
            "wiki 注記「命中P割合増加計算後に加算」",
        ),
        def(
            "hand-max-evasion-rate",
            "最大回避率が増加",
            PartSlot::Hand,
            10,
            RecordOnly,
            HAND_MAX_EVASION_RATE_TIERS,
            "Lv275〜。通常回避「率」を出していないので記録のみ",
        ),
        // --- レリック(右)------------------------------------------------
        def(
            "relic-attack-damage",
            "攻撃ダメージが増加(レリック右)",
            PartSlot::Relic,
            15,
            AttackDamageRate,
            RELIC_CATEGORY15_TIERS,
            "",
        ),
        def(
            "relic-thrust-rate",
            "突き攻撃力が増加(レリック右)",
            PartSlot::Relic,
            15,
            DependencyDamageRate(SkillDependency::Stab),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-slash-rate",
            "斬り攻撃力が増加(レリック右)",
            PartSlot::Relic,
            15,
            DependencyDamageRate(SkillDependency::Hack),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-physical-composite-rate",
            "物理複合攻撃力が増加(レリック右)",
            PartSlot::Relic,
            15,
            DependencyDamageRate(SkillDependency::StabHack),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-magic-rate",
            "魔法攻撃力が増加(レリック右)",
            PartSlot::Relic,
            15,
            DependencyDamageRate(SkillDependency::Int),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-holy-rate",
            "神聖攻撃力が増加(レリック右)",
            PartSlot::Relic,
            15,
            DependencyDamageRate(SkillDependency::Mr),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-magic-slash-rate",
            "魔法斬り攻撃力が増加(レリック右)",
            PartSlot::Relic,
            15,
            DependencyDamageRate(SkillDependency::HackInt),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        // --- レリック(左)------------------------------------------------
        def(
            "relic-damage-resistance",
            "ダメージ耐性が増加(レリック左)",
            PartSlot::Relic,
            3,
            RecordOnly,
            RELIC_RESISTANCE_TIERS,
            "カテゴリU。被ダメージ計算が無いので記録のみ",
        ),
        def(
            "relic-accuracy",
            "命中率が増加(レリック左)",
            PartSlot::Relic,
            10,
            AccuracyPoint,
            RELIC_ACCURACY_TIERS,
            "",
        ),
        def(
            "relic-evasion",
            "回避率が増加(レリック左)",
            PartSlot::Relic,
            10,
            EvasionPoint,
            RELIC_ACCURACY_TIERS,
            "",
        ),
        // --- 武器(すべて発動条件付き = 記録のみ)---------------------------
        def(
            "weapon-boss-damage",
            "一般ボスモンスター攻撃時、追加ダメージ",
            PartSlot::Weapon,
            1,
            RecordOnly,
            WEAPON_BOSS_TIERS,
            "追加ダメージ(新-割合)。敵がボスかどうかを持っていないので記録のみ",
        ),
        def(
            "weapon-raid-boss-damage",
            "レイドボスモンスター攻撃時、追加ダメージ",
            PartSlot::Weapon,
            1,
            RecordOnly,
            WEAPON_RAID_BOSS_TIERS,
            "追加ダメージ(新-割合)。敵がレイドボスかどうかを持っていないので記録のみ",
        ),
        def(
            "weapon-back-attack-damage",
            "対象の後方から攻撃した場合、追加ダメージ",
            PartSlot::Weapon,
            1,
            RecordOnly,
            WEAPON_BACK_ATTACK_TIERS,
            "追加ダメージ(新-割合)。位置関係は計算対象外なので記録のみ",
        ),
        def(
            "weapon-melee-damage",
            "近接する対象攻撃時、追加ダメージ",
            PartSlot::Weapon,
            1,
            RecordOnly,
            WEAPON_MELEE_TIERS,
            "追加ダメージ(新-割合)。発動クールタイム 1 秒・射程条件があるので記録のみ",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let mut seen = HashSet::new();
        for d in random_option_catalog() {
            assert!(seen.insert(d.id), "id 重複: {}", d.id);
        }
    }

    #[test]
    fn every_option_is_on_a_random_option_slot() {
        for d in random_option_catalog() {
            assert!(d.slot.allows_random_option(), "{} は RO を持てない部位", d.id);
        }
    }

    #[test]
    fn tiers_are_ordered_and_non_negative() {
        for d in random_option_catalog() {
            assert!(!d.tiers.is_empty(), "{} にランクが無い", d.id);
            for t in d.tiers {
                assert!(t.min >= 0.0 && t.min <= t.max, "{} のレンジが不正", d.id);
            }
        }
    }

    /// 同じ部位・同じカテゴリーの OP は排他になる。カテゴリー 0 だけは例外(wiki: 転移)。
    #[test]
    fn relic_right_and_left_categories_do_not_collide() {
        let relic: Vec<_> =
            random_option_catalog().into_iter().filter(|d| d.slot == PartSlot::Relic).collect();
        // 右は 15、左は 3 と 10。1 部位に統合しても排他制約が壊れない
        let categories: HashSet<u8> = relic.iter().map(|d| d.category).collect();
        assert_eq!(categories, HashSet::from([3, 10, 15]));
    }
}
