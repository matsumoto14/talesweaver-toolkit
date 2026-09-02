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
    CharacterSkillDef, DamageCategory, MasteryOverride, SkillAudience, SkillEffect, StatKind,
    StatLayer,
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
        source_url: WIKI,
        note: "[仮] 憑依モード時のボーナス",
    },
    // --- 攻撃ダメージ(wiki ステータス [X4]攻撃ダメージ(スキル)。上限 +65%)---
    // マスタリー名がそのままスキル名の行(ボリス【斬撃】等)は masteries.rs 側が持つ。
    // 「デバフ・デバフのデメリット効果」節(イスピンの<プシーキーの権能>、シベリンの挑発)は
    // **対象の**攻撃ダメージを上げるもので自分の火力に入らないので収録しない。
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
        source_url: WIKI,
        note: "マスタリー【良心】",
    },
    CharacterSkillDef {
        id: "yefnen_sharp_shard",
        game_character_id: "yefnen",
        name: "鋭い欠片<フラグ>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "yefnen_m3_2",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::SkillMultiplierRate,
                percent: 20.0,
            }],
        }],
        source_url: WIKI,
        note: "マスタリー【鋭い欠片】のフラグ。爆発の −20% は未収録",
    },
    CharacterSkillDef {
        id: "yefnen_sticky_shard",
        game_character_id: "yefnen",
        name: "べたつく欠片<フラグ>",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[],
        mastery_overrides: &[MasteryOverride {
            mastery_id: "yefnen_m3_3",
            effects: &[SkillEffect::Damage {
                category: DamageCategory::SkillMultiplierRate,
                percent: -10.0,
            }],
        }],
        source_url: WIKI,
        note: "マスタリー【べたつく欠片】。持続ダメージが減る代わりに攻撃ダメージ減少を与える",
    },
    CharacterSkillDef {
        id: "yefnen_swift_sword",
        game_character_id: "yefnen",
        name: "速剣",
        audience: SkillAudience::SelfOnly,
        max_level: 1,
        effects: &[SkillEffect::Damage {
            category: DamageCategory::SkillMultiplierRate,
            percent: -10.0,
        }],
        mastery_overrides: &[],
        source_url: WIKI,
        note: "ソードシェイプ系スキルのスキル倍率が下がる",
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
        source_url: WIKI,
        note: "[仮] マキシミン/クロエ同行時、味方にも",
    },
];

/// キャラスキルのカタログ。
pub fn character_skill_catalog() -> &'static [CharacterSkillDef] {
    CHARACTER_SKILLS
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{CharacterSkills, Masteries};

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
    fn 収録は72件() {
        assert_eq!(CHARACTER_SKILLS.len(), 72);
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
        assert!((x4(&sword.damage_contributions(catalog, &picked(&[]))) - 0.05).abs() < 1e-12);
        assert!(
            (x4(&sword.damage_contributions(catalog, &picked(&["maximin_m3_2"]))) - 0.05).abs()
                < 1e-12
        );
        assert!(
            (x4(&sword.damage_contributions(catalog, &picked(&["maximin_m3_3"]))) - 0.07).abs()
                < 1e-12
        );
    }
}
