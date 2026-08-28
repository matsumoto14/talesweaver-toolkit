//! ランダムオプションのカタログ。
//!
//! 出典: wiki「ランダムオプション」(取得 2026-08-28)。部位ごとの節 + 転移の説明。
//!
//! **収録範囲は火力・命中・回避に関係する OP だけ**。wiki の一覧は HP/MP/移動速度/経験値/変身/
//! 効果音まで含む数百件だが、それらは計算にも記録にも使いようが無いので入れない。
//! 発動条件付き(後方から・ボス限定など)は条件を満たす前提で計算する。依存種別が明記された
//! 命中時 OP は選択スキルとの一致も判定する。まだ実装していない概念(最小回避率補正・
//! 被ダメージ側)に効く OP は `RandomOptionEffect::RecordOnly` で入れて「記録するだけ」と出す。
//!
//! 部位名の対応(wiki の節名 → `PartSlot`):
//! - 「サブアーム」= 盾(`Shield`)
//! - 「サブアーム(SHOW)」= 盾+ / カフス(`ShieldPlus`。Show 装備だが補正が設定されている装備)
//! - 「レリック(右)」「レリック(左)」= **付加オプションの 2 枠**(隠月の結晶(右)/(左))で、
//!   部位ではない(wiki: Item/アクセサリ/レリック「ルナリアレリックは 1 レベルから
//!   アビリティスロット 1 枠、付加オプション 2 枠が付与される」)。
//!   装備としての部位は**ペンダント**と**ブレスレット**の 2 つで、**付く OP が違う**
//!   (ユーザー確認 2026-08-26)。wiki の「レリック(右)」= ペンダント側(カテゴリー15 の
//!   攻撃系)、「レリック(左)」= ブレスレット側(カテゴリー1 の追加ダメージ・3 の耐性・
//!   10 の命中/回避)。**1 部位に 2 枠**で、同じカテゴリーは重複できない

use domain::{
    PartSlot, RandomOptionDef, RandomOptionEffect, RandomOptionRank, RandomOptionTier,
    SkillDependency,
};

use crate::Source;

/// ランダムオプションカタログの出典。
pub const RANDOM_OPTION_SOURCE: Source = Source {
    page: "ランダムオプション",
    retrieved_on: "2026-08-28",
    note: "火力・命中・回避に関係する OP のみ収録。枠数は wiki に記載が無く、\
           制約は「同じカテゴリーは 1 部位に 1 つまで(カテゴリー 0 は除く)」(転移の説明)。\
           **武器・脚の追加ダメージ系はすべて 追加ダメージ(新-割合)**(wiki「ステータス」\
           #追加ダメージ(割合)関連 の一覧に武器/脚のランダムオプションが列挙されている)",
};

use RandomOptionEffect::{
    AccuracyAndEvasionPoint, AccuracyPoint, ActualDelayReduction, AddedDamageRate,
    AttackDamageRate, DependencyDamageRate, EvasionPoint, MagicAddedDamageRate,
    PhysicalAddedDamageRate, RecordOnly,
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
/// レリック(左)カテゴリー1「レリックダンジョンモンスター攻撃時、X% の追加ダメージ」。
const RELIC_DUNGEON_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 5.0, 6.0), tier(Rare, 7.0, 8.0), tier(Special, 9.0, 10.0)];

const RELIC_RESISTANCE_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 3.0, 4.0), tier(Rare, 5.0, 7.0), tier(Special, 8.0, 10.0)];
const RELIC_ACCURACY_TIERS: &[RandomOptionTier] =
    &[tier(Valuable, 3.0, 5.0), tier(Rare, 6.0, 10.0), tier(Special, 11.0, 15.0)];

/// 武器カテゴリー1「物理 / 魔法攻撃が的中した場合、X% の確率で Y% の追加ダメージ」の **Y**。
/// 確率 X は満たしている前提で入れる(ユーザー確認 2026-08-26)。
const WEAPON_ON_HIT_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 10.0, 11.0),
    tier(Valuable, 12.0, 13.0),
    tier(Rare, 13.0, 14.0),
    tier(Special, 14.0, 15.0),
];

/// 武器カテゴリー16「攻撃時、強化の石を 1 個消耗する代わりに X% の追加ダメージ」。
const WEAPON_STONE_TIERS: &[RandomOptionTier] =
    &[tier(Rare, 18.0, 25.0), tier(Special, 30.0, 45.0), tier(STrue, 35.0, 48.0)];

/// 武器カテゴリー16「攻撃時、X SEED を消耗する代わりに Y% の追加ダメージ」の **Y**。
const WEAPON_SEED_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 2.0, 4.0),
    tier(Valuable, 5.0, 8.0),
    tier(Rare, 10.0, 15.0),
    tier(Special, 18.0, 25.0),
];

/// 脚カテゴリー17「攻撃時、移動速度が X 以下の場合、Y% の追加ダメージ」の **Y**。
/// 脚カテゴリー4「移動速度が X 減少し、ダメージ耐性が Y% 増加」の **Y**。
/// 被ダメージ計算が無いので記録のみ。
const LEG_RESISTANCE_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 1.0, 1.0),
    tier(Valuable, 1.0, 2.0),
    tier(Rare, 3.0, 4.0),
    tier(Special, 5.0, 7.0),
];

const LEG_SLOW_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 2.0, 5.0),
    tier(Valuable, 3.0, 7.0),
    tier(Rare, 5.0, 9.0),
    tier(Special, 7.0, 12.0),
];

/// 脚カテゴリー17「攻撃時、X% の確率で Y% の追加ダメージ(自分は移動速度減少)」の **Y**。
const LEG_CHANCE_TIERS: &[RandomOptionTier] = &[
    tier(Normal, 10.0, 10.0),
    tier(Valuable, 11.0, 12.0),
    tier(Rare, 13.0, 14.0),
    tier(Special, 15.0, 18.0),
];

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
    RandomOptionDef { id, name, short: short_name(name), slot, category, effect, tiers, note, common: false }
}

/// 一覧のバッジに出す短い名前。長い名前をそのまま並べると 1 行に収まらない。
const SHORT_NAMES: &[(&str, &str)] = &[
    ("攻撃ダメージが増加(被ダメージも増加)", "攻撃ダメージ"),
    ("攻撃ダメージが増加", "攻撃ダメージ"),
    ("突き攻撃力が増加", "突き"),
    ("斬り攻撃力が増加", "斬り"),
    ("物理複合攻撃力が増加", "物理複合"),
    ("魔法攻撃力が増加", "魔攻"),
    ("神聖攻撃力が増加", "神聖"),
    ("魔法斬り攻撃力が増加", "魔法斬り"),
    ("固定回避が増加", "固定回避"),
    ("スキルの中ディレイが減少", "中ディレイ"),
    ("ダメージ耐性が増加", "耐性"),
    ("命中率が増加", "命中"),
    ("回避率が増加", "回避"),
    ("回避率と命中率が増加", "命中・回避"),
    ("最大回避率が増加", "最大回避率"),
    ("レリックダンジョンのモンスターに追加ダメージ", "レリックD"),
    ("移動速度が減少し、ダメージ耐性が増加", "耐性(速度減)"),
    ("移動速度が遅いとき、追加ダメージ", "低速時"),
    ("攻撃時、確率で追加ダメージ(自分は移動速度減少)", "確率(速度減)"),
    ("一般ボスモンスター攻撃時、追加ダメージ", "一般ボス"),
    ("レイドボスモンスター攻撃時、追加ダメージ", "レイドボス"),
    ("対象の後方から攻撃した場合、追加ダメージ", "後方"),
    ("近接する対象攻撃時、追加ダメージ", "近接"),
    ("物理攻撃が的中した場合、確率で追加ダメージ", "物理命中"),
    ("魔法攻撃が的中した場合、確率で追加ダメージ", "魔法命中"),
    ("強化の石を 1 個消耗する代わりに追加ダメージ", "強化の石"),
    ("SEED を消耗する代わりに追加ダメージ", "SEED"),
];

const fn short_name(name: &'static str) -> &'static str {
    // const fn では文字列比較ができないので、実体は `random_option_catalog` で解決する
    name
}

/// **実際によく付ける OP**(ユーザー確認 2026-08-26)。画面はこれをチップで先に出す。
/// ここに無いものは「ほかの OP」の奥に置くだけで、使えなくなるわけではない。
const COMMON_IDS: &[&str] = &[
    // 武器: 火力に直接効く 3 つ + レイド
    "weapon-boss-damage",
    "weapon-raid-boss-damage",
    "weapon-on-hit-physical",
    "weapon-on-hit-magic",
    "weapon-stone-damage",
    // 盾: 攻撃ダメージ増加と依存別の与ダメージ増加(主軸の依存に合わせて 1 つ)
    "shield-attack-damage",
    "shield-thrust-rate",
    "shield-slash-rate",
    "shield-physical-composite-rate",
    "shield-magic-rate",
    "shield-holy-rate",
    "shield-magic-slash-rate",
    // レリック: ペンダントは攻撃系(主軸の依存に合わせて 1 つ)、ブレスレットは耐性・回避
    "relic-attack-damage",
    "relic-thrust-rate",
    "relic-slash-rate",
    "relic-physical-composite-rate",
    "relic-magic-rate",
    "relic-holy-rate",
    "relic-magic-slash-rate",
    "relic-damage-resistance",
    "relic-evasion",
    // 鎧: 耐久系(計算には入らないが、付ける人が多いので記録する)
    "armor-damage-resistance",
    "armor-fixed-evasion",
    // 脚: 耐久系
    "leg-resistance",
    // 手: 命中率
    "hand-accuracy",
    // カフス: 中ディレイ減少
    "cuffs-actual-delay",
];


pub fn random_option_catalog() -> Vec<RandomOptionDef> {
    let mut defs = random_option_defs();
    for d in &mut defs {
        d.common = COMMON_IDS.contains(&d.id);
        if let Some((_, short)) = SHORT_NAMES.iter().find(|(name, _)| *name == d.name) {
            d.short = short;
        }
    }
    defs
}

fn random_option_defs() -> Vec<RandomOptionDef> {
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
            "攻撃ダメージが増加",
            PartSlot::RelicPendant,
            15,
            AttackDamageRate,
            RELIC_CATEGORY15_TIERS,
            "",
        ),
        def(
            "relic-thrust-rate",
            "突き攻撃力が増加",
            PartSlot::RelicPendant,
            15,
            DependencyDamageRate(SkillDependency::Stab),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-slash-rate",
            "斬り攻撃力が増加",
            PartSlot::RelicPendant,
            15,
            DependencyDamageRate(SkillDependency::Hack),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-physical-composite-rate",
            "物理複合攻撃力が増加",
            PartSlot::RelicPendant,
            15,
            DependencyDamageRate(SkillDependency::StabHack),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-magic-rate",
            "魔法攻撃力が増加",
            PartSlot::RelicPendant,
            15,
            DependencyDamageRate(SkillDependency::Int),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-holy-rate",
            "神聖攻撃力が増加",
            PartSlot::RelicPendant,
            15,
            DependencyDamageRate(SkillDependency::Mr),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        def(
            "relic-magic-slash-rate",
            "魔法斬り攻撃力が増加",
            PartSlot::RelicPendant,
            15,
            DependencyDamageRate(SkillDependency::HackInt),
            RELIC_CATEGORY15_TIERS,
            DEPENDENCY_NOTE,
        ),
        // --- レリック(左)------------------------------------------------
        def(
            "relic-dungeon-damage",
            "レリックダンジョンのモンスターに追加ダメージ",
            PartSlot::RelicBracelet,
            1,
            AddedDamageRate,
            RELIC_DUNGEON_TIERS,
            "追加ダメージ(新-割合)。古代レリックの聖域で効く",
        ),
        def(
            "relic-damage-resistance",
            "ダメージ耐性が増加",
            PartSlot::RelicBracelet,
            3,
            RecordOnly,
            RELIC_RESISTANCE_TIERS,
            "カテゴリU。被ダメージ計算が無いので記録のみ",
        ),
        def(
            "relic-accuracy",
            "命中率が増加",
            PartSlot::RelicBracelet,
            10,
            AccuracyPoint,
            RELIC_ACCURACY_TIERS,
            "",
        ),
        def(
            "relic-evasion",
            "回避率が増加",
            PartSlot::RelicBracelet,
            10,
            EvasionPoint,
            RELIC_ACCURACY_TIERS,
            "",
        ),
        // --- 脚(耐久系。移動速度を捨ててダメージ耐性を取る形)-----------------
        def(
            "leg-resistance",
            "移動速度が減少し、ダメージ耐性が増加",
            PartSlot::Leg,
            4,
            RecordOnly,
            LEG_RESISTANCE_TIERS,
            "カテゴリー4。被ダメージ計算が無いので記録のみ(移動速度も計算対象外)",
        ),
        // --- 脚(追加ダメージ。wiki「ステータス」の一覧で 新-割合)-------------
        def(
            "leg-slow-damage",
            "移動速度が遅いとき、追加ダメージ",
            PartSlot::Leg,
            17,
            AddedDamageRate,
            LEG_SLOW_TIERS,
            "追加ダメージ(新-割合)。移動速度が X 以下のときに効く(歩きでも走る速度で判定)",
        ),
        def(
            "leg-chance-damage",
            "攻撃時、確率で追加ダメージ(自分は移動速度減少)",
            PartSlot::Leg,
            17,
            AddedDamageRate,
            LEG_CHANCE_TIERS,
            "追加ダメージ(新-割合)。1〜3% の確率。発動すると自分の移動速度が数段階下がる",
        ),
        // --- 武器 -----------------------------------------------------------
        //
        // **発動条件は満たしている前提で計算に入れる**(ユーザー確認 2026-08-26)。
        // 実際の運用ではボス戦で殴り、石は常時 ON、確率つきも期待値ではなく効いた側で
        // 見積もる。条件は `note` に残す。
        def(
            "weapon-boss-damage",
            "一般ボスモンスター攻撃時、追加ダメージ",
            PartSlot::Weapon,
            1,
            AddedDamageRate,
            WEAPON_BOSS_TIERS,
            "追加ダメージ(新-割合)。ボス戦で効く",
        ),
        def(
            "weapon-raid-boss-damage",
            "レイドボスモンスター攻撃時、追加ダメージ",
            PartSlot::Weapon,
            1,
            AddedDamageRate,
            WEAPON_RAID_BOSS_TIERS,
            "追加ダメージ(新-割合)。レイドボス戦で効く",
        ),
        def(
            "weapon-back-attack-damage",
            "対象の後方から攻撃した場合、追加ダメージ",
            PartSlot::Weapon,
            1,
            AddedDamageRate,
            WEAPON_BACK_ATTACK_TIERS,
            "追加ダメージ(新-割合)。後ろから殴っている前提",
        ),
        def(
            "weapon-melee-damage",
            "近接する対象攻撃時、追加ダメージ",
            PartSlot::Weapon,
            1,
            AddedDamageRate,
            WEAPON_MELEE_TIERS,
            "追加ダメージ(新-割合)。発動クールタイム 1 秒・射程 24 で確認",
        ),
        def(
            "weapon-on-hit-physical",
            "物理攻撃が的中した場合、確率で追加ダメージ",
            PartSlot::Weapon,
            1,
            PhysicalAddedDamageRate,
            WEAPON_ON_HIT_TIERS,
            "追加ダメージ(新-割合)。物理依存(STAB / HACK / STAB+HACK / 熊)のスキルで、\
             的中時に 5〜6% の確率。値は効いたときの Y%",
        ),
        def(
            "weapon-on-hit-magic",
            "魔法攻撃が的中した場合、確率で追加ダメージ",
            PartSlot::Weapon,
            1,
            MagicAddedDamageRate,
            WEAPON_ON_HIT_TIERS,
            "追加ダメージ(新-割合)。魔法依存(INT / MR / HACK+INT)のスキルで、\
             的中時に 5〜6% の確率。値は効いたときの Y%",
        ),
        def(
            "weapon-stone-damage",
            "強化の石を 1 個消耗する代わりに追加ダメージ",
            PartSlot::Weapon,
            16,
            AddedDamageRate,
            WEAPON_STONE_TIERS,
            "追加ダメージ(新-割合)。強化の石を 20 個以上持っていること。\
             複数体に当てるとその数だけ消費する。説明文から ON / OFF を切り替えられる",
        ),
        def(
            "weapon-seed-damage",
            "SEED を消耗する代わりに追加ダメージ",
            PartSlot::Weapon,
            16,
            AddedDamageRate,
            WEAPON_SEED_TIERS,
            "追加ダメージ(新-割合)。1,000,000 SEED 以上持っていること。\
             複数体に当てるとその数だけ消費する。説明文から ON / OFF を切り替えられる",
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
    fn 武器の命中時追加ダメージはカテゴリー1() {
        for id in ["weapon-on-hit-physical", "weapon-on-hit-magic"] {
            let def = random_option_catalog()
                .into_iter()
                .find(|d| d.id == id)
                .unwrap();
            assert_eq!(def.category, 1);
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

    /// レリックの OP は部位で分かれる。ペンダント = 攻撃系(カテゴリー15)、
    /// ブレスレット = 追加ダメージ / 耐性 / 命中・回避(カテゴリー1・3・10)。
    #[test]
    fn レリックのOPは部位で分かれる() {
        let categories = |slot: PartSlot| -> HashSet<u8> {
            random_option_catalog()
                .into_iter()
                .filter(|d| d.slot == slot)
                .map(|d| d.category)
                .collect()
        };
        assert_eq!(categories(PartSlot::RelicPendant), HashSet::from([15]));
        assert_eq!(categories(PartSlot::RelicBracelet), HashSet::from([1, 3, 10]));
    }
}
