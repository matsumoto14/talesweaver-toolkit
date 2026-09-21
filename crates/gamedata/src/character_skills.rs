//! キャラスキルのカタログ(wiki: 各キャラの Skill ページ / ステータスの各カテゴリ表。取得 2026-08-27)。
//!
//! キャラの**パッシブ・自己バフ・味方バフ**。効き先(ステ / 中ディレイ / 攻撃ダメージ)が
//! 違っても 1 つのカタログに入れる。以前は器が分かれていたため、極・スパートが
//! 中ディレイ専用カタログに、ロアミニの極・ア・プチがバフカタログに、というように
//! **同じ種類のものが 3 か所に散っていた**。
//!
//! **マスタリーで値が変わるスキルは `mastery_overrides` で持つ。**wiki のカテゴリ表が
//! 「スキルの行 + マスタリーで分岐する子行」という形なので、その形をそのまま写す。
//! 対応するマスタリー側(`masteries.rs`)は `RecordOnly` のままにする — 両方に効果を
//! 持たせると二重に数える。
//!
//! **数値は各キャラの「スキル性能一覧」= ステータスの各カテゴリ表を正とする。**
//! キャラページの「マスタリースキル」の節には更新から取り残された値が混ざっていて、
//! 11 件で食い違っていた(シベリン【バーサーク-攻撃型】は性能一覧 +5% に対し節は +30%)。
//!
//! **値が確認できているものだけ収録する。**ステータスの各カテゴリ表に載っている供給源が
//! 一次ソースで、スキルページの説明文しか無いもの(層が分からないもの)は `RecordOnly`。

use domain::{
    CharacterSkillDef, CharacterSkills, DamageCategory, MasteryOverride, SkillAudience, SkillEffect,
    SkillForm, SkillRequirement, StatKind, StatLayer,
};

use crate::Source;

pub const CHARACTER_SKILL_SOURCE: Source = Source {
    page: "各キャラの Skill ページ / ステータス「能力値増加/減少カテゴリー」「与ダメージ計算式・\
           ダメージ増加/減少カテゴリー」「中ディレイ倍率B」",
    retrieved_on: "2026-08-27",
    note: "キャラのパッシブ・自己バフ・味方バフ。マスタリーで値が変わるものは mastery_overrides で持つ",
};

const AGI: &[StatKind] = &[StatKind::Agi];
const INT: &[StatKind] = &[StatKind::Int];
const DEF_MR: &[StatKind] = &[StatKind::Def, StatKind::Mr];
const STAB_DEF: &[StatKind] = &[StatKind::Stab, StatKind::Def];
const INT_MR: &[StatKind] = &[StatKind::Int, StatKind::Mr];
const ALL_STATS: &[StatKind] = &StatKind::ALL;

/// wiki ステータス「中ディレイ倍率B」のキャラ固有ぶんはすべて −5%。
const DELAY_5: &[SkillEffect] = &[SkillEffect::ActualDelay { percent: 5.0 }];
/// 極・スパートの AGI +10%(倍率B)。中ディレイ減少は素だと減衰するので記録のみ。
const SPURT_AGI: &[SkillEffect] = &[
    SkillEffect::StatRate {
        stats: AGI,
        percent: 10.0,
        layer: StatLayer::MultiplierB,
    },
    SkillEffect::RecordOnly,
];
/// マスタリー【グッドフェイス】を取ると中ディレイ低下率が 5% 固定になる。
const SPURT_GOOD_FACE: &[SkillEffect] = &[
    SkillEffect::StatRate {
        stats: AGI,
        percent: 10.0,
        layer: StatLayer::MultiplierB,
    },
    SkillEffect::ActualDelay { percent: 5.0 },
];
/// 極・呪われた魔剣。攻撃ダメージは [X4]、被ダメージ増加は [S4] で未配線。
const CURSED_SWORD_5: &[SkillEffect] = &[
    SkillEffect::Damage {
        category: DamageCategory::AttackDamageSkill,
        percent: 5.0,
    },
    SkillEffect::RecordOnly,
];
const CURSED_SWORD_5_ATTACK_ONLY: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageSkill,
    percent: 5.0,
}];
const CURSED_SWORD_7: &[SkillEffect] = &[
    SkillEffect::Damage {
        category: DamageCategory::AttackDamageSkill,
        percent: 7.0,
    },
    SkillEffect::RecordOnly,
];

const WIKI: &str = "https://talewiki.com/?%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9";

/// 公式お知らせ「システム・キャラクターバランシング実施のお知らせ」§6「バフ、デバフスキル
/// 効果調整」の表(2024-02-21、取得 2026-09-18)。**敵にかけるデバフ**は wiki より優先する
/// (ユーザー決定)。
const NOTICE_153335: &str = "https://talesweaver.nexon.co.jp/notice/notice.aspx?no=153335";

/// 韓国公式スキル情報「귀환(帰還)」= イェフネンの極限スキル(2026-09-21 取得)。
/// <フラグ> のスタックごとの倍率・段数・Cri倍率・周期の出典(wiki と食い違ったらこちらが正)。
const KR_YEVGNEN_RETURN: &str = "https://tw.dn.nexoncdn.co.kr/ActionInfo/18_Yevgnen/3008482.htm";

const CHARACTER_SKILLS: &[CharacterSkillDef] = &[
    // --- 中ディレイ減少のパッシブ(wiki ステータス「中ディレイ倍率B」。全件 −5%)---
    CharacterSkillDef {
        id: "boris_sword_priest",
        game_character_id: "boris",
        name: "剣の司祭",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: DELAY_5,
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    CharacterSkillDef {
        id: "ispin_rivalry",
        game_character_id: "ispin",
        name: "ライバルリー",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: DELAY_5,
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    CharacterSkillDef {
        id: "maximin_clumsy_pair",
        game_character_id: "maximin",
        name: "ドタバタペア",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        // wiki は中ディレイ倍率B と [X4] の両方に載っている。効き先が 2 つある例
        effects: &[
            SkillEffect::ActualDelay { percent: 5.0 },
            SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 2.0,
            },
        ],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    CharacterSkillDef {
        id: "chloe_rivalry",
        game_character_id: "chloe",
        name: "ライバルリー",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: DELAY_5,
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    CharacterSkillDef {
        id: "anais_loki_specialization",
        game_character_id: "anais",
        name: "ロキ特化",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: DELAY_5,
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    CharacterSkillDef {
        id: "isolet_corona_gale",
        game_character_id: "isolet",
        name: "コロナゲイル",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: DELAY_5,
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    // --- マスタリーで性能が変わる自己バフ ---
    // wiki ステータス「中ディレイ倍率B」: |ミラ|スパート|-25%/-15%/-5%/-0%|| / |~|~|-5%|【グッドフェイス】|
    // 素の中ディレイ減少は 25% から 9.6 秒かけて 0% まで減衰するので定常値にできない(記録のみ)。
    // AGI +10% の層は**倍率B**。実測(2026-08-27、素ステ AGI 271)で
    // 375 → 412(+37)= floor(375 × 0.10)。割合増加なら floor(271 × 0.10) = 27 で合わない。
    CharacterSkillDef {
        id: "mira_spurt",
        game_character_id: "mira",
        name: "極・スパート",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: SPURT_AGI,
        mastery_overrides: &[MasteryOverride {
            mastery_id: "mira_m4_2",
            effects: SPURT_GOOD_FACE,
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "中ディレイ減少は素だと 25% → 0% に減衰。移動速度 +10 は未配線",
    },
    // wiki ステータス [X4]攻撃ダメージ(スキル): |マキシミン|呪われた魔剣|+5%|【呪われた魔剣】|
    // |~|~|+5%|【封印された魔剣】| |~|~|+7%|【自我を持つ魔剣】|
    // 被ダメージ増加(+5% / +7%)は [S4] で未配線なので `RecordOnly` を併記する。
    CharacterSkillDef {
        id: "maximin_cursed_sword",
        game_character_id: "maximin",
        name: "極・呪われた魔剣",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: CURSED_SWORD_5,
        mastery_overrides: &[
            MasteryOverride {
                mastery_id: "maximin_m3_1",
                effects: CURSED_SWORD_5,
            },
            MasteryOverride {
                mastery_id: "maximin_m3_2",
                effects: CURSED_SWORD_5_ATTACK_ONLY,
            },
            MasteryOverride {
                mastery_id: "maximin_m3_3",
                effects: CURSED_SWORD_7,
            },
        ],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "持続2分・CT10分。M3 の三択で値が変わる。被ダメージ +5% は未配線",
    },
    // マキシミン専用「極・的中剣」(wiki Skill/マキシミン #HitSword。取得 2026-09-01)。
    // 命中Pにかかる倍率が SLv に比例して増える(Lv*5%。QA・持続14〜20分・Master=Lv7)。
    // アビリティの的中剣(装備システム/アビリティ)とは別物 — 装備側は単純な装備命中率補正
    // (`EquipmentValues.accuracy`)であり、こちらはスキルの命中P割合増加(SLv 制)。
    // SLv は `CharacterSkills::skill_levels` に持つ(既存の on/off のみのキャラスキルと違う軸)。
    CharacterSkillDef {
        id: "maximin_hit_sword",
        game_character_id: "maximin",
        name: "極・的中剣",
        audience: SkillAudience::SelfOnly,
        // wiki: `|Master＝Lv7|`
        max_level: 7,
        // wiki: `Lv*5%`(Lv7 で ×1.35)。命中P変動は `#AccuracyPoint` の表(Lv1 の行は集中と共通)
        effects: &[SkillEffect::AccuracyRate {
            per_level: 0.05,
            shift: &[3, 2, 1, 1, 0, -1, -2],
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "命中Pにかかる倍率が SLv×5%増加(Master=Lv7で+35%)。ペット集中(Lv1相当)が優先",
    },
    // --- マスタリーを取ってはじめて効果が出るスキル ---
    // ロアミニの M3【ア・プチ】【パウアトゥン】は「スキルを選択」= そのスキルが使えるようになる。
    CharacterSkillDef {
        id: "roamini_ha_petit",
        game_character_id: "roamini",
        name: "極・ア・プチ",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "roamini_m3_3",
            effects: &[
                SkillEffect::StatRate {
                    stats: INT,
                    percent: 10.0,
                    layer: StatLayer::MultiplierB,
                },
                SkillEffect::Damage {
                    category: DamageCategory::AttackDamageSkill,
                    percent: 3.0,
                },
            ],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "射程 +4 は未配線",
    },
    CharacterSkillDef {
        id: "roamini_powatun",
        game_character_id: "roamini",
        name: "極・パウアトゥン",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "roamini_m3_2",
            effects: &[SkillEffect::StatRate {
                stats: DEF_MR,
                percent: 10.0,
                layer: StatLayer::MultiplierB,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "被ダメージ −10% と最大HP増加は未配線",
    },
    // ジョシュアの憑依モード。マスタリー【エリート】を取ったときだけボーナスが乗る。
    // モード(剣闘士 / 魔法師)は ON にするほうを選ぶ
    CharacterSkillDef {
        id: "joshua_possession_swordsman",
        game_character_id: "joshua",
        name: "憑依【剣闘士】",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "joshua_m2_3",
            effects: &[SkillEffect::StatRate {
                stats: STAB_DEF,
                percent: 10.0,
                layer: StatLayer::MultiplierB,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "[仮] 憑依モード時のボーナス",
    },
    CharacterSkillDef {
        id: "joshua_possession_mage",
        game_character_id: "joshua",
        name: "憑依【魔法師】",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "joshua_m2_3",
            effects: &[SkillEffect::StatRate {
                stats: INT_MR,
                percent: 10.0,
                layer: StatLayer::MultiplierB,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "[仮] 憑依モード時のボーナス",
    },
    // --- 攻撃ダメージ(wiki ステータス [X4]攻撃ダメージ(スキル)。上限 +65%)---
    // マスタリー名がそのままスキル名の行(ボリス【斬撃】等)は masteries.rs 側が持つ。
    // 「デバフ・デバフのデメリット効果」節は 2 種を分ける(公式お知らせ no=153335 §6 で裏取り、
    // 2026-09-18): **敵被ダメージ増加**(イスピン〈プシーキーの権能〉等)は自分の火力に入るので
    // `SkillAudience::Enemy` の敵デバフとして下の「敵にかけるデバフ」節に収録する。
    // **敵攻撃ダメージ増加**(シベリン挑発のデメリット等)は敵が自分に与えるダメージの計算経路が
    // 無いので収録しない。
    CharacterSkillDef {
        id: "lucian_lagrange_sword",
        game_character_id: "lucian",
        name: "ラグランジュ神速剣",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "lucian_m2_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 5.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【強剣】。前後ディレイも増える",
    },
    CharacterSkillDef {
        id: "lucian_powered_streak",
        game_character_id: "lucian",
        name: "極・連撃 / 極・無双乱舞 / 極・旋風斬",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "lucian_m3_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 5.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【力を込めた連撃】。この 3 スキルを主軸にするときだけ ON にする",
    },
    CharacterSkillDef {
        id: "lucian_iron_wall",
        game_character_id: "lucian",
        name: "鉄壁",
        audience: SkillAudience::Ally,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "味方にも",
    },
    CharacterSkillDef {
        id: "boris_winter_survivor",
        game_character_id: "boris",
        name: "冬を乗り越える者",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 6.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "boris_zinneman_survivor",
        game_character_id: "boris",
        name: "ジンネマン家の生き残り",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "boris_m2_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 3.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【必滅者】",
    },
    CharacterSkillDef {
        id: "boris_snow_guard",
        game_character_id: "boris",
        name: "スノーガード<騎士道>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "boris_m3_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 3.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【騎士道】。<騎士道>が 30 スタック時の値(持続30s)",
    },
    CharacterSkillDef {
        id: "boris_guard_warrior",
        game_character_id: "boris",
        name: "護衛武士",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "mira_captain",
        game_character_id: "mira",
        name: "キャプテン",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "mira_high_jump",
        game_character_id: "mira",
        name: "ハイジャンプ",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "mira_m2_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 3.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【ヴァイパーズアイ】(持続 5 分)",
    },
    CharacterSkillDef {
        id: "ispin_forge_promotion",
        game_character_id: "ispin",
        name: "鍛造<プロモーション>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 3.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "ispin_forge",
        game_character_id: "ispin",
        name: "鍛造",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "ispin_m3_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 2.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【ハードトレーニング】で<鍛造>バフに攻撃ダメージが付く(持続 2 分)",
    },
    CharacterSkillDef {
        id: "ispin_non_retour",
        game_character_id: "ispin",
        name: "ノン・ルトゥール",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 4.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "maximin_boy_patriarch",
        game_character_id: "maximin",
        name: "少年家長",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "パッシブ",
    },
    CharacterSkillDef {
        id: "tichiel_study_tincture",
        game_character_id: "tichiel",
        name: "勉強用チンキ剤",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "tichiel_red_eye_medicine",
        game_character_id: "tichiel",
        name: "赤い目の名薬",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 20.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "tichiel_red_eye_penalty",
        game_character_id: "tichiel",
        name: "赤い目の名薬(ペナルティ)",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: -20.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "名薬の反動。攻撃ダメージが下がる",
    },
    CharacterSkillDef {
        id: "nayatorei_miao",
        game_character_id: "nayatorei",
        name: "苗族",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "siberin_berserk",
        game_character_id: "siberin",
        name: "バーサーク",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[
            MasteryOverride {
                mastery_id: "siberin_m5_1",
                effects: &[SkillEffect::Damage {
                    category: DamageCategory::AttackDamageSkill,
                    percent: 5.0,
                }],
            },
            MasteryOverride {
                mastery_id: "siberin_m5_2",
                effects: &[SkillEffect::Damage {
                    category: DamageCategory::AttackDamageSkill,
                    percent: 3.0,
                }],
            },
            MasteryOverride {
                mastery_id: "siberin_m5_3",
                effects: &[],
            },
        ],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "M5 の型で変わる。防御型は攻撃ダメージが上がらない",
    },
    CharacterSkillDef {
        id: "joshua_medium_ghost",
        game_character_id: "joshua",
        name: "霊媒<幽霊>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "joshua_m2_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 3.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【アノーイングネイバー】",
    },
    CharacterSkillDef {
        id: "joshua_needle_thread",
        game_character_id: "joshua",
        name: "糸と針",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "joshua_soul_burst",
        game_character_id: "joshua",
        name: "ソウルバースト",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "joshua_m4_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 5.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【エクソダス】",
    },
    CharacterSkillDef {
        id: "chloe_mana_wall",
        game_character_id: "chloe",
        name: "マナウォール",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[
            MasteryOverride {
                mastery_id: "chloe_m5_2",
                effects: &[SkillEffect::Damage {
                    category: DamageCategory::AttackDamageSkill,
                    percent: 5.0,
                }],
            },
            MasteryOverride {
                mastery_id: "chloe_m5_3",
                effects: &[SkillEffect::Damage {
                    category: DamageCategory::AttackDamageSkill,
                    percent: 7.0,
                }],
            },
        ],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "M5 の型で変わる。防御型は攻撃ダメージが上がらない",
    },
    CharacterSkillDef {
        id: "chloe_magic_researcher",
        game_character_id: "chloe",
        name: "魔法研究者<渡空>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "ranjie_uprising",
        game_character_id: "ranjie",
        name: "アップライジング",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "敵味方に効果有",
    },
    CharacterSkillDef {
        id: "isaac_darkreuz_martial",
        game_character_id: "isaac",
        name: "ダルクロイツの武術家<招式>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "isaac_father_of_daughter",
        game_character_id: "isaac",
        name: "娘持ちの父親",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "anais_lost_brother",
        game_character_id: "anais",
        name: "生き別れの弟",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "anais_joy_together",
        game_character_id: "anais",
        name: "共にいられる喜び",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 3.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "anais_bear_angry",
        game_character_id: "anais",
        name: "くまさんアングリー",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "wiki:「効果なし(2024/2/21〜)」",
    },
    CharacterSkillDef {
        id: "anais_lucibear_barrier",
        game_character_id: "anais",
        name: "ルシベアバリア",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 3.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "wiki は +1〜3%。最大値で入れている",
    },
    CharacterSkillDef {
        id: "anais_fire_aura",
        game_character_id: "anais",
        name: "ファイヤーオーラ",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "anais_hard_weapon",
        game_character_id: "anais",
        name: "ハードウエポン",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 20.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "wiki は +5,13〜20%。最大値で入れている",
    },
    CharacterSkillDef {
        id: "isolet_zone_burst",
        game_character_id: "isolet",
        name: "ゾーンバースト<武威>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "isolet_song_of_light",
        game_character_id: "isolet",
        name: "光の歌",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 5.0,
        }],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "isolet_m3_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 10.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【光の歌（攻撃）】で +10% になる",
    },
    CharacterSkillDef {
        id: "isolet_noble_solitude",
        game_character_id: "isolet",
        name: "高貴な孤独",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 3.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "isolet_boris",
        game_character_id: "isolet",
        name: "ボリス",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "benya_trans_spirit",
        game_character_id: "benya",
        name: "極・トランススピリット",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "benya_m4_1",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 10.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【トランススピリット】で選択",
    },
    CharacterSkillDef {
        id: "benya_altruistic_spirit",
        game_character_id: "benya",
        name: "極・アルトリスティックスピリット",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "benya_m4_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 7.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【アルトリスティックスピリット】で選択。自身のぶん",
    },
    CharacterSkillDef {
        id: "benya_altruistic_spirit_party",
        game_character_id: "benya",
        name: "極・アルトリスティックスピリット(味方)",
        audience: SkillAudience::Ally,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "使い手のマスタリー【アルトリスティックスピリット】が前提",
    },
    CharacterSkillDef {
        id: "benya_harmonic_spirit",
        game_character_id: "benya",
        name: "極・ハーモニックスピリット",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "benya_m4_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 7.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【ハーモニックスピリット】で選択",
    },
    CharacterSkillDef {
        id: "benya_gracestar_brooch",
        game_character_id: "benya",
        name: "グレイスターブローチ",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "roamini_curse_pendulum",
        game_character_id: "roamini",
        name: "カース・ペンジュラム",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 15.0,
        }],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "roamini_m1_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 20.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【シンボルオブスピリット】で +20%(CT 1.5 倍)",
    },
    CharacterSkillDef {
        id: "roamini_miao",
        game_character_id: "roamini",
        name: "苗族",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "nocturne_cosmos",
        game_character_id: "nocturne",
        name: "コスモス<調和>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "nocturne_tw_guardian",
        game_character_id: "nocturne",
        name: "テイルズウィーバー守護者",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "nocturne_electric_burst",
        game_character_id: "nocturne",
        name: "エレクトリックバースト",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 3.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "leeche_friend",
        game_character_id: "leeche",
        name: "友達",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 2.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "",
    },
    CharacterSkillDef {
        id: "leeche_attack_fever",
        game_character_id: "leeche",
        name: "極・攻撃の熱気",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "leeche_m4_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::AttackDamageSkill,
                percent: 3.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【極・攻撃の熱気】で選択",
    },
    CharacterSkillDef {
        id: "leeche_attack_fever_party",
        game_character_id: "leeche",
        name: "極・攻撃の熱気(味方)",
        audience: SkillAudience::Ally,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageSkill,
            percent: 5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "使い手のマスタリー【極・攻撃の熱気】が前提",
    },
    // --- スキル倍率増加(割合)(wiki ステータス [E1]スキル倍率増加I(割合))---
    CharacterSkillDef {
        id: "isaac_energy_field",
        game_character_id: "isaac",
        name: "エネルギーフィールド",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "isaac_m2_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::SkillMultiplierRate,
                percent: 50.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【プリチャージ】",
    },
    CharacterSkillDef {
        id: "roamini_curse_end",
        game_character_id: "roamini",
        name: "カース・エンド",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "roamini_m2_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::SkillMultiplierRate,
                percent: 50.0,
            }],
        }],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "マスタリー【良心】",
    },
    // --- イェフネンの形態ごとのパッシブ(wiki「Skill/イェフネン」スキル性能一覧 /
    // ステータス各カテゴリ表、2026-09-21 取得)。**効果を技データ側に持つもの**は
    // ここでは効果を持たず、「習得している印」として `requires` で形態だけを宣言する。
    // 値は `Skill::swift_sword` / `Skill::full_charge`(gamedata/skills.rs)が持つ。
    //
    // 鋭い欠片 / べたつく欠片(マスタリー M3)は**<フラグ>にしか効かない** E1 で、技そのものには
    // 効かない。以前はキャラスキルとして収録していたが誤りなので削除した(2026-09-21。
    // 保存済みの id は storage の v18 移行と IndexedDB の v10 移行が落とす)。
    CharacterSkillDef {
        id: "yefnen_swift_sword",
        game_character_id: "yefnen",
        name: "速剣",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        // 倍率 ×0.9・段数 +1 は wiki スキル性能一覧の「(速剣適用時)」行そのものなので、
        // 技データ(`Skill::swift_sword`)が持つ。ここに E1 −10% を書くと二重に効く
        effects: &[],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: Some(SkillRequirement::Form(SkillForm::Sword)),
        source_url: WIKI,
        note: "ソードシェイプ系 4 技の倍率 ×0.9・段数 +1(習得していると常に適用)",
    },
    CharacterSkillDef {
        id: "yefnen_full_charge",
        game_character_id: "yefnen",
        name: "最大までチャージ",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        // 段数とチャージ時間は技データ(`Skill::full_charge`)が持つ
        effects: &[],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: Some(SkillRequirement::FullCharge),
        source_url: WIKI,
        note: "スレイ・アックス 8→17 段 / クラッシュ・アックス 6→10 段。チャージ 1 秒(マスタリー【アックス特化】で 0.5 秒)が 1 回の所要時間に乗る",
    },
    CharacterSkillDef {
        id: "yefnen_back_attack",
        game_character_id: "yefnen",
        name: "後方から攻撃",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        // マスタリー【パイク特化】を取ってはじめて効果が出る。マスタリー側は `RecordOnly`
        // のままにする(両方に効果を持たせると二重に数える)
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "yefnen_m1_1",
            effects: &[SkillEffect::AddedDamageRate { percent: 10.0 }],
        }],
        exclusive_with: &[],
        requires: Some(SkillRequirement::Form(SkillForm::Pike)),
        source_url: WIKI,
        note: "マスタリー【パイク特化】: パイクシェイプ系で後方攻撃時 追加ダメージ +10%",
    },
    CharacterSkillDef {
        id: "yefnen_boris_mark",
        game_character_id: "yefnen",
        name: "なんともいえない切なさ",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::AddedDamageRate { percent: 10.0 }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "<ボリス!> が付いた敵への攻撃に追加ダメージ +10%(形態を問わない)",
    },
    // --- 値・層が確かめられていないので記録だけ(確率発動は docs/adr/005 で収録見送り)---
    CharacterSkillDef {
        id: "yefnen_boris_company",
        game_character_id: "yefnen",
        name: "ボリス同行",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::RecordOnly],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "攻撃ダメージ +3%。wiki のカテゴリ表に無く、X のどの副カテゴリか未確認",
    },
    CharacterSkillDef {
        id: "yefnen_revenge",
        game_character_id: "yefnen",
        name: "<復讐>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::RecordOnly],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "攻撃ダメージ +30%。wiki のカテゴリ表に無く、X のどの副カテゴリか未確認",
    },
    CharacterSkillDef {
        id: "yefnen_reinforce",
        game_character_id: "yefnen",
        name: "<強化>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::RecordOnly],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "攻撃ダメージ +30%。wiki のカテゴリ表に無く、X のどの副カテゴリか未確認",
    },
    CharacterSkillDef {
        id: "yefnen_chisel_smash",
        game_character_id: "yefnen",
        name: "チゼルの防御貫通",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::RecordOnly],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: Some(SkillRequirement::Form(SkillForm::Chisel)),
        source_url: WIKI,
        note: "スレイ・チゼル / クラッシュ・チゼルの強打系: 確率 15% で防御 −15%。確率発動は収録しない方針(docs/adr/005)なので計算には入れない",
    },
    // <フラグ> は技とは別枠のダメージ(持続 1 秒ごと + スレイ / クラッシュで爆発)。
    // ON = 敵に <フラグ> が付いている、SLv = スタック数(最大 10)。
    // 倍率表・持続 / 爆発の作り方・マスタリー3 の ±% は gamedata/flag.rs が持つ
    // (ここに効果を書くと技の与ダメージ式に合流してしまう)。
    CharacterSkillDef {
        id: "yefnen_flag",
        game_character_id: "yefnen",
        name: "<フラグ>",
        audience: SkillAudience::SelfOnly,
        max_level: 10,
        effects: &[SkillEffect::SeparateDamage],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: KR_YEVGNEN_RETURN,
        note: "連 / 爆 が積み、スレイ / クラッシュ が爆発させる。スタック数ぶん倍率が上がる(1:320% → 10:400%)",
    },
    CharacterSkillDef {
        id: "yefnen_shard_wire",
        game_character_id: "yefnen",
        name: "シャードワイヤー",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::RecordOnly],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "効果の層・値が wiki のカテゴリ表で確認できていないため記録のみ",
    },
    // --- 最終ダメージ(wiki ステータス [L]最終ダメージ。上限 +45%)---
    CharacterSkillDef {
        id: "benya_dark_blessing",
        game_character_id: "benya",
        name: "ダークブレッシング",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::FinalDamageRate,
            percent: 100.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "極・ミラクルスピリットの追加効果。上限 +45% で頭打ちになる",
    },
    // --- ステ上昇の自己バフ・味方バフ(wiki ステータス「能力値増加/減少カテゴリー」)---
    CharacterSkillDef {
        id: "benya_soul_gate",
        game_character_id: "benya",
        name: "極・ソウルゲート",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::StatRate {
            stats: AGI,
            percent: 5.0,
            layer: StatLayer::PercentOfBase,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "自身のみ",
    },
    CharacterSkillDef {
        id: "ispin_encourage",
        game_character_id: "ispin",
        name: "極・エンカレッジ",
        audience: SkillAudience::Ally,
        max_level: 1,
        effects: &[SkillEffect::StatRate {
            stats: ALL_STATS,
            percent: 10.0,
            layer: StatLayer::MultiplierB,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "味方にも(30分)",
    },
    CharacterSkillDef {
        id: "siberin_charm",
        game_character_id: "siberin",
        name: "魅力発散",
        audience: SkillAudience::Ally,
        max_level: 1,
        effects: &[SkillEffect::StatRate {
            stats: ALL_STATS,
            percent: 1.0,
            layer: StatLayer::MultiplierB,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "[仮] 女性キャラ同行時、味方にも",
    },
    CharacterSkillDef {
        id: "tichiel_magic_teacher",
        game_character_id: "tichiel",
        name: "魔法の先生",
        audience: SkillAudience::Ally,
        max_level: 1,
        effects: &[SkillEffect::StatRate {
            stats: INT,
            percent: 10.0,
            layer: StatLayer::MultiplierB,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "[仮] マキシミン/クロエ同行時、味方にも",
    },
    // --- 敵にかけるデバフ(公式お知らせ no=153335 §6。同行者がかける前提なので誰でも ON にできる。
    // 効果は [S4]敵被ダメージ増加(`DamageCategory::TakenDamageReduction` に負値)---
    CharacterSkillDef {
        id: "lucian_breeze",
        game_character_id: "lucian",
        name: "そよ風",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "siberin_fear",
        game_character_id: "siberin",
        name: "フィアー",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "maximin_invective_abuse",
        game_character_id: "maximin",
        name: "毒舌【暴言】",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "<毒舌>にマスタリー【暴言】を乗せた形。敵被ダメージ増加(+10%)",
    },
    CharacterSkillDef {
        id: "joshua_weaken",
        game_character_id: "joshua",
        name: "弱化",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+10%)",
    },
    CharacterSkillDef {
        id: "ranjie_disturbance",
        game_character_id: "ranjie",
        name: "撹乱",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+10%)。ボリス同行時の追加 +3% は条件付きなので含めない",
    },
    CharacterSkillDef {
        id: "isaac_fake",
        game_character_id: "isaac",
        name: "フェイク",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "mira_card_spray_a_debuff",
        game_character_id: "mira",
        name: "カードスプレーA",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "tichiel_sparkling_kite_debuff",
        game_character_id: "tichiel",
        name: "スパークリングカイト",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+10%)",
    },
    CharacterSkillDef {
        id: "ispin_mark_de_victoire",
        game_character_id: "ispin",
        name: "マーク・デ・ヴィクトワール",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)。自分の被ダメージ減少(-2%)は含めない",
    },
    CharacterSkillDef {
        id: "ispin_psyche_authority",
        game_character_id: "ispin",
        name: "プシーキーの権能",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "ispin_sabotage",
        game_character_id: "ispin",
        name: "サボタージュ",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "nayatorei_plunder",
        game_character_id: "nayatorei",
        name: "強奪",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "chloe_wildfire",
        game_character_id: "chloe",
        name: "ワイルドファイヤー",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "anais_fairy_light_debuff",
        game_character_id: "anais",
        name: "フェアリーライト",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -3.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+3%)",
    },
    CharacterSkillDef {
        id: "anais_ice_snare",
        game_character_id: "anais",
        name: "アイススネア",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+10%)。敵攻撃ダメージ減少(-10%)は含めない",
    },
    CharacterSkillDef {
        id: "benya_force_of_rubert",
        game_character_id: "benya",
        name: "フォース・オブ・ルベルト",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+10%)",
    },
    CharacterSkillDef {
        id: "roamini_curse_pendulum_debuff",
        game_character_id: "roamini",
        name: "カース・ペンジュラム",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -15.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &["roamini_curse_pendulum_symbol_of_spirit_debuff"],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+15%)。マスタリー【シンボルオブスピリット】なら +20%(別行)",
    },
    CharacterSkillDef {
        id: "roamini_curse_pendulum_symbol_of_spirit_debuff",
        game_character_id: "roamini",
        name: "カース・ペンジュラム【シンボルオブスピリット】",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -20.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &["roamini_curse_pendulum_debuff"],
        requires: None,
        source_url: NOTICE_153335,
        note: "マスタリー【シンボルオブスピリット】を取っているときの敵被ダメージ増加(+20%)。\
               roamini_curse_pendulum_debuff(基本 +15%)と排他 — 両方 ON にすると二重計上になる",
    },
    CharacterSkillDef {
        id: "nocturne_endure_charge",
        game_character_id: "nocturne",
        name: "インデュアチャージ",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "leeche_thin_defense",
        game_character_id: "leeche",
        name: "手薄",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "leeche_frailty",
        game_character_id: "leeche",
        name: "薄弱",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -5.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+5%)",
    },
    CharacterSkillDef {
        id: "leeche_ganapoli_puppetry",
        game_character_id: "leeche",
        name: "ガナポリー人形術",
        audience: SkillAudience::Enemy,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::TakenDamageReduction,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: NOTICE_153335,
        note: "敵被ダメージ増加(+10%)",
    },
    // ブレンドはスタックする(最大 10)。スタック 1 つにつき敵の被ダメージ +1%。
    // 公式お知らせ(153335)はイェフネン実装前の表なので、ここだけは wiki が正。
    CharacterSkillDef {
        id: "yefnen_blend",
        game_character_id: "yefnen",
        name: "ブレンド",
        audience: SkillAudience::Enemy,
        max_level: 10,
        effects: &[SkillEffect::DamagePerLevel {
            category: DamageCategory::TakenDamageReduction,
            percent: -1.0,
        }],
        mastery_overrides: &[],
        exclusive_with: &[],
        requires: None,
        source_url: WIKI,
        note: "敵被ダメージ増加 +1% × スタック(最大 10)",
    },
];

/// キャラスキルのカタログ。
pub fn character_skill_catalog() -> &'static [CharacterSkillDef] {
    CHARACTER_SKILLS
}

/// 保存済みのキャラスキル選択から、**カタログに無くなった id** を落とす。
///
/// 残っていると `CharacterSkills::validate` が `Unknown` を返し、そのキャラの計算・
/// プレビューがまるごと止まる(実機で「未知のキャラスキルです」)。id の一覧は持たず
/// カタログそのものを引くので、カタログから消すたびにここへ 1 行足す必要がない。
///
/// SQLite の v18 移行(`storage::migrate_removed_character_skills`)・IndexedDB の v10 移行・
/// 書き出し JSON の読み込み(`transfer.ts`)がこの 1 関数を通る。冪等
/// (カタログにある id しか残らないので、2 回通しても結果は同じ)。
/// 戻り値は落としたものがあったか。
pub fn normalize_character_skill_selection(skills: &mut CharacterSkills) -> bool {
    let known = |id: &str| CHARACTER_SKILLS.iter().any(|d| d.id == id);
    let before = (skills.skill_ids.len(), skills.skill_levels.len());
    skills.skill_ids.retain(|id| known(id));
    skills.skill_levels.retain(|id, _| known(id));
    (skills.skill_ids.len(), skills.skill_levels.len()) != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{
        find_skill, resolve_skill_variants, skills_for, AXE_SPECIALIZATION_MASTERY_ID,
        FULL_CHARGE_SKILL_ID, SWIFT_SWORD_SKILL_ID,
    };
    use domain::Masteries;

    /// テスト用: カテゴリX4(攻撃ダメージ(スキル))の合計。
    fn x4(contributions: &[domain::DamageContribution]) -> f64 {
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

    /// 収録件数。カタログを入れ替えたら数を更新する(黙って増減させない)。
    #[test]
    fn 収録は102件() {
        assert_eq!(CHARACTER_SKILLS.len(), 102);
    }

    /// 上限はカテゴリ側(`DamageCategory::cap`)が見るので、ここでは
    /// **効き先ごとに値が正気の範囲か**だけ確認する(桁を間違えたら気づけるように)。
    #[test]
    fn ダメージへの効き先は効き先ごとの範囲に収まる() {
        for skill in CHARACTER_SKILLS {
            let all = skill.effects.iter().chain(
                skill
                    .mastery_overrides
                    .iter()
                    .flat_map(|o| o.effects.iter()),
            );
            for effect in all {
                let SkillEffect::Damage { category, percent } = effect else {
                    continue;
                };
                let range = match category {
                    // [X4] 攻撃ダメージ(スキル)。ティチエルの名薬ペナルティが −20%
                    DamageCategory::AttackDamageSkill => -20.0..=20.0,
                    // [E1] スキル倍率増加(割合)。イサック/ロアミニの +50% が最大
                    DamageCategory::SkillMultiplierRate => -20.0..=50.0,
                    // [L] 最終ダメージ。ダークブレッシングの +100%(上限 +45% で頭打ち)
                    DamageCategory::FinalDamageRate => 0.0..=100.0,
                    // [S4] 敵被ダメージ増加(敵にかけるデバフ)。公式お知らせ no=153335 §6 の
                    // 最大値はロアミニ〈カース・ペンジュラム【シンボルオブスピリット】〉の -20%
                    DamageCategory::TakenDamageReduction => -20.0..=0.0,
                    other => panic!("{} の効き先 {other:?} に範囲が決まっていない", skill.id),
                };
                assert!(
                    range.contains(percent),
                    "{} {} {percent}",
                    skill.id,
                    category.label()
                );
            }
        }
    }

    #[test]
    fn id_は一意で収録キャラはすべてプレイアブル一覧にある() {
        let mut ids: Vec<&str> = CHARACTER_SKILLS.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CHARACTER_SKILLS.len());
        for d in CHARACTER_SKILLS {
            assert!(
                crate::find_character(d.game_character_id).is_some(),
                "{} が一覧に無い",
                d.id
            );
        }
    }

    /// `mastery_overrides` の参照先がマスタリーカタログに実在し、
    /// **そのマスタリー自身は `RecordOnly`**(効果はスキル側が持つので二重に数えない)。
    #[test]
    fn 差し替え元のマスタリーは実在して記録のみ() {
        let masteries = crate::mastery_catalog();
        for skill in CHARACTER_SKILLS {
            for over in skill.mastery_overrides {
                let def = masteries
                    .iter()
                    .find(|m| m.id == over.mastery_id)
                    .unwrap_or_else(|| {
                        panic!("{} の差し替え元 {} が無い", skill.id, over.mastery_id)
                    });
                assert_eq!(
                    def.game_character_id, skill.game_character_id,
                    "{} と {} でキャラが違う",
                    skill.id, over.mastery_id
                );
                assert!(
                    !def.effect.is_modeled(),
                    "{} はスキル側と二重に数えている",
                    over.mastery_id
                );
            }
        }
    }

    /// 中ディレイ減少はキャラ固有ぶんが全件 −5%(wiki ステータス「中ディレイ倍率B」)。
    #[test]
    fn 中ディレイ減少は全件5パーセント() {
        for skill in CHARACTER_SKILLS {
            let all: Vec<&SkillEffect> = skill
                .effects
                .iter()
                .chain(
                    skill
                        .mastery_overrides
                        .iter()
                        .flat_map(|o| o.effects.iter()),
                )
                .collect();
            for effect in all {
                if let SkillEffect::ActualDelay { percent } = effect {
                    assert_eq!(*percent, 5.0, "{}", skill.id);
                }
            }
        }
    }

    /// AGI +10% は素でも乗る。中ディレイ減少は【グッドフェイス】を取ったときだけ
    /// (素は 25% から 9.6 秒で 0% まで減衰するので定常値にできない)。
    #[test]
    fn スパートはagiが常に乗り中ディレイはグッドフェイスのときだけ入る() {
        let catalog = character_skill_catalog();
        let spurt = on(&["mira_spurt"]);
        for masteries in [picked(&[]), picked(&["mira_m4_2"])] {
            assert_eq!(
                spurt.stat_rates(catalog, &masteries),
                vec![(
                    domain::StatKind::Agi,
                    0.10,
                    StatLayer::MultiplierB,
                    "極・スパート"
                )]
            );
        }
        assert!(spurt
            .actual_delay_contributions(catalog, &picked(&[]))
            .is_empty());
        let with = spurt.actual_delay_contributions(catalog, &picked(&["mira_m4_2"]));
        assert_eq!(with.len(), 1);
        assert!((with[0].rate - 0.05).abs() < 1e-12);
    }

    /// 層の根拠。ユーザー実測(2026-08-27、ミラ 素ステ AGI 271):
    /// スパート前 375 → 後 412(+37)。倍率B は `floor(basic × 10%)` なので
    /// `floor(375 × 0.10) = 37` で一致する。割合増加なら `floor(271 × 0.10) = 27` で合わない。
    #[test]
    fn スパートのagiは倍率bで実測の375から412に一致する() {
        use domain::{effective_stat, StatKind, StatModifiers};

        const BASE: u32 = 271;
        // 素ステ以外の補正で basic を 375 にする(実測時の状態)
        let mut m = StatModifiers {
            fixed: 375 - i64::from(BASE),
            ..Default::default()
        };
        let (before, _) = effective_stat(StatKind::Agi, BASE, &m, i64::MAX);
        assert_eq!(before, 375);

        let mut contributions = Vec::new();
        let mut set = domain::StatModifierSet::default();
        *set.get_mut(StatKind::Agi) = m.clone();
        domain::apply_character_skills(
            &mut set,
            &mut contributions,
            &on(&["mira_spurt"]),
            &picked(&[]),
            character_skill_catalog(),
        );
        m = set.get(StatKind::Agi).clone();
        let (after, trace) = effective_stat(StatKind::Agi, BASE, &m, i64::MAX);
        assert_eq!(after, 412, "実測 375 → 412 と一致しない");
        assert_eq!(trace.multiplier_b_bonus, 37);
        // 割合増加ではないこと(それなら floor(271 × 0.10) = 27 になる)
        assert_eq!(trace.percent_of_base_total, 0);
    }

    #[test]
    fn 呪われた魔剣はm3の三択で5と7に分かれる() {
        let catalog = character_skill_catalog();
        let sword = on(&["maximin_cursed_sword"]);
        assert!((x4(&sword.damage_contributions(catalog, &picked(&[]), None)) - 0.05).abs() < 1e-12);
        assert!(
            (x4(&sword.damage_contributions(catalog, &picked(&["maximin_m3_2"]), None)) - 0.05).abs()
                < 1e-12
        );
        assert!(
            (x4(&sword.damage_contributions(catalog, &picked(&["maximin_m3_3"]), None)) - 0.07).abs()
                < 1e-12
        );
    }


    // --- イェフネンの形態(wiki「Skill/イェフネン」スキル性能一覧、2026-09-21 取得)---

    /// 形態は 5 つ × 4 技 = 20 件。イェフネン以外の技は形態を持たない。
    #[test]
    fn イェフネンの20技に形態が付く() {
        use domain::SkillForm::*;
        let expected: &[(&str, domain::SkillForm)] = &[
            ("yefnen_continuous", Sword),
            ("yefnen_explosion", Sword),
            ("yefnen_slay", Sword),
            ("yefnen_crash", Sword),
            ("yefnen_continuous_pike", Pike),
            ("yefnen_explosion_pike", Pike),
            ("yefnen_slay_pike", Pike),
            ("yefnen_crash_pike", Pike),
            ("yefnen_continuous_axe", Axe),
            ("yefnen_explosion_axe", Axe),
            ("yefnen_slay_axe", Axe),
            ("yefnen_crash_axe", Axe),
            ("yefnen_continuous_urumi", Urumi),
            ("yefnen_explosion_urumi", Urumi),
            ("yefnen_slay_urumi", Urumi),
            ("yefnen_crash_urumi", Urumi),
            ("yefnen_continuous_chisel", Chisel),
            ("yefnen_explosion_chisel", Chisel),
            ("yefnen_slay_chisel", Chisel),
            ("yefnen_crash_chisel", Chisel),
        ];
        for (id, form) in expected {
            assert_eq!(find_skill(id).unwrap().form, Some(*form), "{id}");
        }
        // 形態を持つのはこの 20 件だけ(他キャラの技に紛れ込んでいない)
        let with_form = skills_for("yefnen")
            .iter()
            .filter(|s| s.form.is_some())
            .count();
        assert_eq!(with_form, expected.len());
        assert_eq!(find_skill("lucian_continuous").unwrap().form, None);
    }

    /// 速剣(wiki 一覧の「(速剣適用時)」行)。倍率は素の ×0.9、段数は表の実値。
    /// ソードシェイプ系 4 技**だけ**に載り、他の形態には一切効かない。
    #[test]
    fn 速剣はソード系4技だけの倍率と段数に差し替わる() {
        let cases: &[(&str, f64, u32)] = &[
            ("yefnen_continuous", 3.429, 12),
            ("yefnen_explosion", 4.086, 6),
            ("yefnen_slay", 6.3, 13),
            ("yefnen_crash", 6.075, 7),
        ];
        let learned = CharacterSkills {
            skill_ids: vec![SWIFT_SWORD_SKILL_ID.to_string()],
            ..Default::default()
        };
        let none = Masteries::default();
        for (id, multiplier, hit_count) in cases {
            let base = find_skill(id).unwrap();
            // 素の倍率の 0.9 倍であること(wiki のカテゴリ表「スキル倍率増加(割合)−10%」)
            assert!((base.multiplier * 0.9 - multiplier).abs() < 1e-9, "{id}");
            let resolved = resolve_skill_variants(base.clone(), &learned, &none);
            assert!((resolved.multiplier - multiplier).abs() < 1e-12, "{id}");
            assert_eq!(resolved.hit_count, *hit_count, "{id}");
            assert!((resolved.power - multiplier * f64::from(*hit_count)).abs() < 1e-9, "{id}");
            // 習得していなければ素のまま
            let plain = resolve_skill_variants(base.clone(), &CharacterSkills::default(), &none);
            assert_eq!(plain.multiplier, base.multiplier, "{id}");
            assert_eq!(plain.hit_count, base.hit_count, "{id}");
        }
        // ソード以外(パイク・アックス・ウルミ・チゼル)には効かない
        for id in [
            "yefnen_continuous_pike",
            "yefnen_slay_axe",
            "yefnen_crash_urumi",
            "yefnen_explosion_chisel",
        ] {
            let base = find_skill(id).unwrap();
            let resolved = resolve_skill_variants(base.clone(), &learned, &none);
            assert_eq!(resolved.multiplier, base.multiplier, "{id}");
            assert_eq!(resolved.hit_count, base.hit_count, "{id}");
            assert_eq!(resolved.charge_seconds, 0.0, "{id}");
        }
    }

    /// チャージ(wiki 一覧の段数幅)。アックスの スレイ 8〜17 / クラッシュ 6〜10 だけ。
    /// マスタリー【アックス特化】でチャージタイムが半減する。
    #[test]
    fn 最大チャージはアックスのスレイとクラッシュの段数を上げる() {
        let charging = CharacterSkills {
            skill_ids: vec![FULL_CHARGE_SKILL_ID.to_string()],
            ..Default::default()
        };
        let none = Masteries::default();
        let axe_spec = Masteries {
            picked: vec![AXE_SPECIALIZATION_MASTERY_ID.to_string()],
        };
        for (id, base_hits, max_hits) in [("yefnen_slay_axe", 8, 17), ("yefnen_crash_axe", 6, 10)] {
            let base = find_skill(id).unwrap();
            assert_eq!(base.hit_count, base_hits, "{id}");
            let full = resolve_skill_variants(base.clone(), &charging, &none);
            assert_eq!(full.hit_count, max_hits, "{id}");
            assert_eq!(full.charge_seconds, 1.0, "{id}");
            // 【アックス特化】でチャージタイム半減
            let fast = resolve_skill_variants(base.clone(), &charging, &axe_spec);
            assert_eq!(fast.charge_seconds, 0.5, "{id}");
            assert_eq!(fast.hit_count, max_hits, "{id}");
        }
        // チャージできない技には効かない(アックスの 連・爆 も含む)
        for id in ["yefnen_continuous_axe", "yefnen_slay", "yefnen_slay_pike"] {
            let base = find_skill(id).unwrap();
            let resolved = resolve_skill_variants(base.clone(), &charging, &axe_spec);
            assert_eq!(resolved.hit_count, base.hit_count, "{id}");
            assert_eq!(resolved.charge_seconds, 0.0, "{id}");
        }
    }

    /// 後方攻撃の追加ダメージ +10% は**パイクの技 + マスタリー【パイク特化】**のときだけ。
    #[test]
    fn 後方攻撃の追加ダメージはパイクとm1_1がそろったときだけ入る() {
        let catalog = character_skill_catalog();
        let on_back = on(&["yefnen_back_attack"]);
        let pike = find_skill("yefnen_slay_pike").unwrap();
        let sword = find_skill("yefnen_slay").unwrap();
        let pike_spec = picked(&["yefnen_m1_1"]);

        let rate = |skills: &CharacterSkills, m: &Masteries, s: &domain::Skill| {
            skills.added_damage_rate(catalog, m, Some(s))
        };
        assert!((rate(&on_back, &pike_spec, &pike) - 0.10).abs() < 1e-12);
        // マスタリー未取得は 0(スキルを ON にしただけでは効かない)
        assert_eq!(rate(&on_back, &picked(&[]), &pike), 0.0);
        // 形態が違えば 0(ソードシェイプには効かない)
        assert_eq!(rate(&on_back, &pike_spec, &sword), 0.0);
        // スキルを ON にしていなければ 0
        assert_eq!(rate(&on(&[]), &pike_spec, &pike), 0.0);
    }

    /// <ボリス!> は形態を問わず追加ダメージ +10%。後方攻撃と重なれば足し合わさる。
    #[test]
    fn ボリスの追加ダメージは形態を問わない() {
        let catalog = character_skill_catalog();
        let chisel = find_skill("yefnen_slay_chisel").unwrap();
        let pike = find_skill("yefnen_slay_pike").unwrap();
        assert!(
            (on(&["yefnen_boris_mark"]).added_damage_rate(catalog, &picked(&[]), Some(&chisel))
                - 0.10)
                .abs()
                < 1e-12
        );
        let both = on(&["yefnen_boris_mark", "yefnen_back_attack"]);
        assert!(
            (both.added_damage_rate(catalog, &picked(&["yefnen_m1_1"]), Some(&pike)) - 0.20).abs()
                < 1e-12
        );
    }

    /// ブレンドはスタック 1 つにつき敵の被ダメージ +1%(最大 10)。
    #[test]
    fn ブレンドはスタック数に比例する() {
        let catalog = character_skill_catalog();
        let def = catalog.iter().find(|d| d.id == "yefnen_blend").unwrap();
        assert_eq!(def.max_level, 10);
        let stacked = |n: u8| {
            let mut skills = on(&["yefnen_blend"]);
            skills.skill_levels.insert("yefnen_blend".to_string(), n);
            skills
                .damage_contributions(catalog, &picked(&[]), None)
                .iter()
                .filter(|c| c.category == DamageCategory::TakenDamageReduction)
                .map(|c| c.value)
                .sum::<f64>()
        };
        assert!((stacked(5) - -0.05).abs() < 1e-12);
        assert!((stacked(1) - -0.01).abs() < 1e-12);
        assert!((stacked(10) - -0.10).abs() < 1e-12);
        // SLv 未指定は上限(10 スタック)= 以前の固定 −10% と同値
        let sum: f64 = on(&["yefnen_blend"])
            .damage_contributions(catalog, &picked(&[]), None)
            .iter()
            .filter(|c| c.category == DamageCategory::TakenDamageReduction)
            .map(|c| c.value)
            .sum();
        assert!((sum - -0.10).abs() < 1e-12);
    }

    /// 誤って収録していた <フラグ> 専用のマスタリー効果は消えている(2026-09-21)。
    /// 保存済みの id はカタログを引く正規化で落ちる。
    #[test]
    fn 欠片系は収録から消えて保存済みの選択からも落ちる() {
        let catalog = character_skill_catalog();
        for id in ["yefnen_sharp_shard", "yefnen_sticky_shard"] {
            assert!(catalog.iter().all(|d| d.id != id), "{id}");
        }
        let mut saved = CharacterSkills {
            skill_ids: vec![
                "yefnen_sharp_shard".into(),
                "yefnen_swift_sword".into(),
                "yefnen_sticky_shard".into(),
            ],
            skill_levels: [("yefnen_sticky_shard".to_string(), 1u8), ("yefnen_blend".to_string(), 3)]
                .into_iter()
                .collect(),
        };
        assert!(normalize_character_skill_selection(&mut saved));
        assert_eq!(saved.skill_ids, vec!["yefnen_swift_sword".to_string()]);
        assert_eq!(saved.skill_levels.get("yefnen_blend"), Some(&3));
        assert!(saved.validate(catalog, "yefnen").is_ok());
        // 2 回目は何も落ちない(冪等)
        assert!(!normalize_character_skill_selection(&mut saved));
    }

    /// 敵にかけるデバフが与ダメージ式まで効いているか、実カタログから通しで確かめる。
    /// カタログ → 寄与 → カテゴリ集計 → 式で使う倍率、の全段をつなぐ。
    #[test]
    fn 敵デバフはカタログからダメージ式の倍率まで届く() {
        use domain::{CategoryTotals, DamageCategory};

        let catalog = character_skill_catalog();
        let totals_of = |ids: &[&str]| {
            let mut t = CategoryTotals::neutral();
            for c in on(ids).damage_contributions(catalog, &picked(&[]), None) {
                t.add(c.category, c.value);
            }
            t
        };

        // 何も ON にしていなければ S は中立(×1.00)
        assert_eq!(
            totals_of(&[]).get(DamageCategory::TakenDamageReduction),
            1.0
        );

        // 毒舌【暴言】= 敵被ダメージ増加 +10% → (1−S) が 1.10 倍
        let one = totals_of(&["maximin_invective_abuse"]);
        assert!(
            (one.get(DamageCategory::TakenDamageReduction) - 1.10).abs() < 1e-12,
            "{}",
            one.get(DamageCategory::TakenDamageReduction)
        );

        // 積める。毒舌【暴言】+10% と弱化 +10% で 1.20 倍
        let two = totals_of(&["maximin_invective_abuse", "joshua_weaken"]);
        assert!(
            (two.get(DamageCategory::TakenDamageReduction) - 1.20).abs() < 1e-12,
            "{}",
            two.get(DamageCategory::TakenDamageReduction)
        );

        // 下限 −30% で頭打ち。+10 +10 +15 +10 = +45% を積んでも 1.30 倍まで
        let capped = totals_of(&[
            "maximin_invective_abuse",
            "joshua_weaken",
            "roamini_curse_pendulum_debuff",
            "yefnen_blend",
        ]);
        assert!(
            (capped.get(DamageCategory::TakenDamageReduction) - 1.30).abs() < 1e-12,
            "{}",
            capped.get(DamageCategory::TakenDamageReduction)
        );
    }
}
