//! 装着アビリティ(wiki: 装備システム/アビリティ、Item/合成/装着アビリティシステム/
//! 新装着アビリティ)。部位ごとの候補と、その本体値・ランダム追加枠を組み立てる。

use super::*;

/// 武器アビリティ(装備システム/アビリティ)の出典。
pub const EQUIPMENT_ABILITY_SOURCE: Source = Source {
    page: "装備システム/アビリティ",
    retrieved_on: "2026-09-01",
    note: "武器3スロット。カテゴリー1/3は旧装着アビリティ。カテゴリー4は2経路あり、装備システムUIのN-/R-/L-/E-系(装備システム/アビリティ)と、アイテム方式の古代精霊/深淵/喪失/夜星の4系列(Item/合成/装着アビリティシステム/新装着アビリティ)。ランダム追加効果は自動適用しない",
};

fn new_ability(
    id: &'static str,
    name: &'static str,
    family: EquipmentAbilityFamily,
    values: EquipmentValues,
    effect_summary: &'static str,
) -> EquipmentAbilityDef {
    use EquipmentAbilityAdditionalKind::*;
    let additional_effects = match family {
        EquipmentAbilityFamily::PointedBlade => {
            "固定ダメージ/割合ダメージ/突き/自然回復/命中からランダム"
        }
        EquipmentAbilityFamily::SharpBlade => {
            "固定ダメージ/割合ダメージ/斬り/自然回復/命中からランダム"
        }
        EquipmentAbilityFamily::Intelligence => {
            "固定ダメージ/割合ダメージ/魔攻/自然回復/命中からランダム"
        }
        EquipmentAbilityFamily::MagicResistance => {
            "固定ダメージ/割合ダメージ/魔防/自然回復/命中からランダム"
        }
        EquipmentAbilityFamily::WeaponDelay => "",
        _ => unreachable!("武器用の新装着アビリティ以外が渡されました"),
    };
    let stat_kind = match family {
        EquipmentAbilityFamily::PointedBlade => Thrust,
        EquipmentAbilityFamily::SharpBlade => Slash,
        EquipmentAbilityFamily::Intelligence => MagicAttack,
        EquipmentAbilityFamily::MagicResistance => MagicDefense,
        EquipmentAbilityFamily::WeaponDelay => {
            unreachable!("新装着アビリティに武器ディレイ系は無い")
        }
        _ => unreachable!("武器用の新装着アビリティ以外が渡されました"),
    };
    let (fixed, rate, stat_min, stat_max, recovery_min, recovery_max, accuracy_min, accuracy_max) =
        match values
            .thrust
            .max(values.slash)
            .max(values.magic_attack)
            .max(values.magic_defense)
        {
            11 => (5000, 8, 5, 10, 4, 14, 6, 10),
            13 => (6000, 9, 7, 12, 6, 16, 8, 12),
            17 => (7000, 10, 7, 15, 6, 16, 8, 14),
            _ => (10000, 11, 9, 18, 8, 18, 10, 16),
        };
    let additional_options = vec![
        EquipmentAbilityAdditionalDef {
            kind: FixedDamage,
            min: fixed,
            max: fixed,
        },
        EquipmentAbilityAdditionalDef {
            kind: DamageRate,
            min: rate,
            max: rate,
        },
        EquipmentAbilityAdditionalDef {
            kind: stat_kind,
            min: stat_min,
            max: stat_max,
        },
        EquipmentAbilityAdditionalDef {
            kind: HpRecovery,
            min: recovery_min,
            max: recovery_max,
        },
        EquipmentAbilityAdditionalDef {
            kind: MpRecovery,
            min: recovery_min,
            max: recovery_max,
        },
        EquipmentAbilityAdditionalDef {
            kind: Accuracy,
            min: accuracy_min,
            max: accuracy_max,
        },
    ];
    EquipmentAbilityDef {
        id,
        name,
        family,
        category: 4,
        slot: PartSlot::Weapon,
        value_option: None,
        exclusive_group: "weapon-category-4",
        additional_slots: 2,
        additional_effects,
        additional_options,
        record_only: false,
        effect_summary,
        values,
        damage_effects: &[],
    }
}

fn fixed_ability(
    id: &'static str,
    name: &'static str,
    family: EquipmentAbilityFamily,
    category: u8,
    values: EquipmentValues,
    effect_summary: &'static str,
    record_only: bool,
) -> EquipmentAbilityDef {
    EquipmentAbilityDef {
        id,
        name,
        family,
        category,
        slot: PartSlot::Weapon,
        value_option: None,
        exclusive_group: match category {
            1 => "weapon-category-1",
            3 => "weapon-category-3",
            _ => "weapon-category-other",
        },
        additional_slots: 0,
        additional_effects: "",
        additional_options: vec![],
        record_only,
        effect_summary,
        values,
        damage_effects: &[],
    }
}

fn slot_ability(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    family: EquipmentAbilityFamily,
    group: &'static str,
    values: EquipmentValues,
    effect_summary: &'static str,
    record_only: bool,
    damage_effects: &'static [SkillEffect],
) -> EquipmentAbilityDef {
    use EquipmentAbilityAdditionalKind::*;
    let option = |kind, min, max| EquipmentAbilityAdditionalDef { kind, min, max };
    let value_option = match (slot, family) {
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::Accuracy) => Some((Accuracy, 7, 13)),
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::Evasion) => Some((Evasion, 7, 13)),
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::PointedBlade) => Some((Thrust, 7, 15)),
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::SharpBlade) => Some((Slash, 7, 15)),
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::Intelligence) => Some((MagicAttack, 7, 15)),
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::MagicResistance) => {
            Some((MagicDefense, 7, 15))
        }
        (PartSlot::RelicPendant, EquipmentAbilityFamily::PointedBlade) => Some((Thrust, 1, 15)),
        (PartSlot::RelicPendant, EquipmentAbilityFamily::SharpBlade) => Some((Slash, 1, 15)),
        (PartSlot::RelicPendant, EquipmentAbilityFamily::Intelligence) => {
            Some((MagicAttack, 1, 15))
        }
        (PartSlot::RelicPendant, EquipmentAbilityFamily::MagicResistance) => {
            Some((MagicDefense, 1, 15))
        }
        (PartSlot::RelicBracelet, EquipmentAbilityFamily::Accuracy) => Some((Accuracy, 1, 13)),
        (PartSlot::RelicBracelet, EquipmentAbilityFamily::Evasion) => Some((Evasion, 1, 13)),
        (PartSlot::RelicBracelet, EquipmentAbilityFamily::Critical) => Some((Critical, 1, 12)),
        _ => None,
    }
    .map(|(kind, min, max)| EquipmentAbilityAdditionalDef { kind, min, max });
    let (additional_slots, additional_effects, additional_options) = match slot {
        PartSlot::Armor => {
            // 機敏だけダメージ耐性を持たない(wiki 新装着アビリティの機敏の行に無い。
            // ランダムOP の耐性と取り違えていた。ユーザー確認 2026-09-01)
            let mut options = if family == EquipmentAbilityFamily::Evasion {
                vec![option(HpRecovery, 8, 18), option(MpRecovery, 8, 18)]
            } else {
                vec![
                    option(DamageResistance, 11, 11),
                    option(HpRecovery, 8, 18),
                    option(MpRecovery, 8, 18),
                ]
            };
            match family {
                EquipmentAbilityFamily::Vitality | EquipmentAbilityFamily::ArmorPolish => {
                    options.push(option(PhysicalDamageReduction, 150, 150));
                    options.push(option(PhysicalDefense, 10, 16));
                }
                EquipmentAbilityFamily::Mana | EquipmentAbilityFamily::MagicResistance => {
                    options.push(option(MagicDamageReduction, 150, 150));
                    options.push(option(MagicDefense, 8, 14));
                }
                EquipmentAbilityFamily::Evasion => {
                    options.push(option(EvasionRate, 9, 20));
                    options.push(option(PhysicalDamageReduction, 100, 100));
                    options.push(option(MagicDamageReduction, 100, 100));
                    options.push(option(SpRecovery, 8, 18));
                }
                _ => {}
            }
            (2, "ランダム追加2枠", options)
        }
        PartSlot::Shield => {
            let reduction = if family == EquipmentAbilityFamily::ShieldPolish {
                PhysicalDamageReduction
            } else {
                MagicDamageReduction
            };
            (
                2,
                "ランダム追加2枠",
                vec![
                    option(DamageResistance, 10, 10),
                    option(reduction, 100, 100),
                    option(HpRecovery, 8, 18),
                    option(MpRecovery, 8, 18),
                    option(SpRecovery, 8, 18),
                    option(EvasionRate, 10, 18),
                ],
            )
        }
        PartSlot::ShieldPlus => {
            let options = if matches!(
                family,
                EquipmentAbilityFamily::Accuracy | EquipmentAbilityFamily::Evasion
            ) {
                vec![
                    option(DamageResistance, 5, 6),
                    option(PhysicalDamageReduction, 90, 100),
                    option(MagicDamageReduction, 90, 100),
                    option(HpRecovery, 15, 20),
                    option(MpRecovery, 15, 20),
                    option(SpRecovery, 15, 20),
                    option(Critical, 5, 10),
                ]
            } else {
                vec![
                    option(FireElement, 10, 30),
                    option(WaterElement, 10, 30),
                    option(WindElement, 10, 30),
                    option(EarthElement, 10, 30),
                    option(LightningElement, 10, 30),
                    option(WhiteElement, 10, 30),
                    option(DarkElement, 10, 30),
                ]
            };
            (1, "ランダム追加1枠", options)
        }
        PartSlot::Hand => (
            2,
            "ランダム追加2枠",
            vec![
                option(FixedDamage, 10_000, 10_000),
                option(DamageRate, 9, 9),
                option(Thrust, 8, 14),
                option(Slash, 8, 14),
                option(MagicAttack, 8, 14),
                option(MagicDefense, 8, 14),
            ],
        ),
        PartSlot::Head => {
            let kinds: [EquipmentAbilityAdditionalKind; 3] = match id {
                "g-earth-moonstone" => [DarkElement, WaterElement, WindElement],
                "g-dark-moonstone" => [WaterElement, WindElement, LightningElement],
                "g-water-moonstone" => [WindElement, LightningElement, WhiteElement],
                "g-wind-moonstone" => [LightningElement, WhiteElement, FireElement],
                "g-lightning-moonstone" => [WhiteElement, FireElement, EarthElement],
                "g-white-moonstone" => [FireElement, EarthElement, DarkElement],
                _ => [EarthElement, DarkElement, WaterElement],
            };
            (
                1,
                "ランダム追加1枠",
                kinds.into_iter().map(|kind| option(kind, 20, 20)).collect(),
            )
        }
        PartSlot::Leg => (
            2,
            "ランダム追加2枠",
            vec![
                option(HpRecovery, 8, 18),
                option(MpRecovery, 8, 18),
                option(SpRecovery, 8, 18),
                option(EvasionRate, 7, 15),
            ],
        ),
        PartSlot::RelicPendant => (
            1,
            "ランダム追加1枠",
            vec![
                option(FireElement, 20, 30),
                option(WaterElement, 20, 30),
                option(WindElement, 20, 30),
                option(EarthElement, 20, 30),
                option(LightningElement, 20, 30),
                option(WhiteElement, 20, 30),
                option(DarkElement, 20, 30),
                option(DamageRate, 5, 10),
            ],
        ),
        PartSlot::RelicBracelet => (
            1,
            "ランダム追加1枠",
            vec![
                option(DamageResistance, 5, 10),
                option(PhysicalDamageReduction, 90, 100),
                option(MagicDamageReduction, 90, 100),
                option(HpRecovery, 15, 20),
                option(MpRecovery, 15, 20),
                option(SpRecovery, 15, 20),
            ],
        ),
        _ => (0, "", vec![]),
    };
    EquipmentAbilityDef {
        id,
        name,
        family,
        category: 4,
        slot,
        value_option,
        exclusive_group: group,
        additional_slots,
        additional_effects,
        additional_options,
        record_only,
        effect_summary,
        values,
        damage_effects,
    }
}

fn fixed_slot_ability(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    family: EquipmentAbilityFamily,
    category: u8,
    group: &'static str,
    values: EquipmentValues,
    effect_summary: &'static str,
) -> EquipmentAbilityDef {
    let mut def = slot_ability(
        id,
        name,
        slot,
        family,
        group,
        values,
        effect_summary,
        false,
        &[],
    );
    def.category = category;
    def.additional_slots = 0;
    def.additional_effects = "";
    def.additional_options.clear();
    def
}

/// 防具アビリティの単一補正。武器の `a` と同じで、要約と値がずれないよう 1 か所に寄せる。
fn pd(physical_defense: i64) -> EquipmentValues {
    EquipmentValues {
        physical_defense,
        ..EquipmentValues::default()
    }
}
fn md(magic_defense: i64) -> EquipmentValues {
    EquipmentValues {
        magic_defense,
        ..EquipmentValues::default()
    }
}
fn ev(evasion: i64) -> EquipmentValues {
    EquipmentValues {
        evasion,
        ..EquipmentValues::default()
    }
}
fn cr(critical: i64) -> EquipmentValues {
    EquipmentValues {
        critical,
        ..EquipmentValues::default()
    }
}
fn ac(accuracy: i64) -> EquipmentValues {
    EquipmentValues {
        accuracy,
        ..EquipmentValues::default()
    }
}

/// 装備システム UI から付けるカテゴリー4(N- / R- / L- / E- 系)。
/// アビリティアイテム方式(夜星など)と違い「追加効果」列は表で固定なので、
/// ランダム追加枠を持たない。追加効果のうち計算へ入るのはダメージ増加(X3)だけで、
/// 防御率・回避率は `EquipmentAbilityDef` に受け皿が無い(装備アイテム側の
/// `survival_effects` にしかない)ため、要約の文言としてだけ残す。
fn ui_category4(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    family: EquipmentAbilityFamily,
    group: &'static str,
    values: EquipmentValues,
    effect_summary: &'static str,
    record_only: bool,
    damage_effects: &'static [SkillEffect],
) -> EquipmentAbilityDef {
    let mut def = slot_ability(
        id,
        name,
        slot,
        family,
        group,
        values,
        effect_summary,
        record_only,
        damage_effects,
    );
    def.value_option = None;
    def.additional_slots = 0;
    def.additional_effects = "";
    def.additional_options.clear();
    def
}

/// 追加アビリティ1件。系列ごとに値域が違うので、書き下すときの手数を減らす。
fn add_opt(
    kind: EquipmentAbilityAdditionalKind,
    min: i32,
    max: i32,
) -> EquipmentAbilityAdditionalDef {
    EquipmentAbilityAdditionalDef { kind, min, max }
}

/// 新装着アビリティの系列もの。`slot_ability` は最上位(夜星)の値域を既定に持つので、
/// 下位系列は追加アビリティの候補だけを差し替える。
fn line_ability(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    family: EquipmentAbilityFamily,
    group: &'static str,
    values: EquipmentValues,
    effect_summary: &'static str,
    record_only: bool,
    additional_options: Vec<EquipmentAbilityAdditionalDef>,
) -> EquipmentAbilityDef {
    let mut def = slot_ability(
        id,
        name,
        slot,
        family,
        group,
        values,
        effect_summary,
        record_only,
        &[],
    );
    def.additional_options = additional_options;
    def
}

/// 追加効果を持たない段(N- など)。同じタプル配列に並べるための空スライス。
const NO_EFFECTS: &[SkillEffect] = &[];

const HELM_SKILL_1: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::SkillMultiplierFixed,
    percent: 1.0,
}];

const HELM_SKILL_5: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::SkillMultiplierFixed,
    percent: 5.0,
}];

/// 「ダメージ増加 +n%」はカテゴリX3(攻撃ダメージ(基本発動)・上限 +80%)に入る。
/// X3 の説明に「武器/手アビリティ」と名指しがあるので、装備攻撃力ではなくこちらへ足す。
const ATTACK_DAMAGE_X3_3: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageBasicTrigger,
    percent: 3.0,
}];
const ATTACK_DAMAGE_X3_4: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageBasicTrigger,
    percent: 4.0,
}];
const ATTACK_DAMAGE_X3_6: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageBasicTrigger,
    percent: 6.0,
}];

const HELM_SKILL_10: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::SkillMultiplierFixed,
    percent: 10.0,
}];

/// 武器アビリティは装備攻撃力(突き/斬り/魔攻/魔防)にしか効かない。
pub(super) fn a(thrust: i64, slash: i64, magic_attack: i64, magic_defense: i64) -> EquipmentValues {
    EquipmentValues {
        thrust,
        slash,
        magic_attack,
        magic_defense,
        ..Default::default()
    }
}

/// 武器アビリティ。武器は最大3スロットで、同じカテゴリーは1つまで。
/// カテゴリー1と4は同じ攻撃系統でも併用できる（例: 下級斬り + 夜星の鋭い刃）。
/// カテゴリー4の追加アビリティはランダムなので自動適用しない。
pub fn equipment_abilities() -> Vec<EquipmentAbilityDef> {
    use EquipmentAbilityAdditionalKind::*;
    let mut out = Vec::new();

    // カテゴリー1: 旧アビリティ。2026年追加の「下級〜」も同カテゴリー。
    for (family, entries) in [
        (
            EquipmentAbilityFamily::PointedBlade,
            [
                ("low-pointed-blade", "(下)尖った刃", 2, "突き +2"),
                ("middle-pointed-blade", "(中)尖った刃", 3, "突き +3"),
                ("upper-pointed-blade", "(上)尖った刃", 4, "突き +4"),
                ("lower-grade-stab", "下級突き", 12, "突き +12"),
            ],
        ),
        (
            EquipmentAbilityFamily::SharpBlade,
            [
                ("low-sharp-blade", "(下)鋭い刃", 2, "斬り +2"),
                ("middle-sharp-blade", "(中)鋭い刃", 3, "斬り +3"),
                ("upper-sharp-blade", "(上)鋭い刃", 4, "斬り +4"),
                ("lower-grade-slash", "下級斬り", 12, "斬り +12"),
            ],
        ),
        (
            EquipmentAbilityFamily::Intelligence,
            [
                ("low-intelligence", "(下)知力", 2, "魔攻 +2"),
                ("middle-intelligence", "(中)知力", 3, "魔攻 +3"),
                ("upper-intelligence", "(上)知力", 4, "魔攻 +4"),
                ("lower-grade-magic-attack", "下級魔法攻撃", 12, "魔攻 +12"),
            ],
        ),
        (
            EquipmentAbilityFamily::MagicResistance,
            [
                ("low-magic-resistance", "(下)耐魔力", 2, "魔防 +2"),
                ("middle-magic-resistance", "(中)耐魔力", 3, "魔防 +3"),
                ("upper-magic-resistance", "(上)耐魔力", 4, "魔防 +4"),
                ("lower-grade-magic-defense", "下級魔法防御", 12, "魔防 +12"),
            ],
        ),
    ] {
        for (id, name, value, summary) in entries {
            let values = match family {
                EquipmentAbilityFamily::PointedBlade => a(value, 0, 0, 0),
                EquipmentAbilityFamily::SharpBlade => a(0, value, 0, 0),
                EquipmentAbilityFamily::Intelligence => a(0, 0, value, 0),
                EquipmentAbilityFamily::MagicResistance => a(0, 0, 0, value),
                EquipmentAbilityFamily::WeaponDelay => EquipmentValues::default(),
                _ => unreachable!("武器の基本アビリティ以外が渡されました"),
            };
            out.push(fixed_ability(id, name, family, 1, values, summary, false));
        }
    }

    // カテゴリー3: 武器ディレイは前/後ディレイに作用し、中ディレイには作用しない。
    // 現環境では後ディレイがほぼ 0 になるため DPS 計算の対象外とし、記録表示のみ。
    for (id, name, summary) in [
        ("gale-blade", "疾風の刃", "武器ディレイ -5%"),
        ("storm-blade", "暴風の刃", "武器ディレイ -7%"),
        ("soft-wind-blade", "軟風の刃", "武器ディレイ +5%"),
        ("breeze-blade", "微風の刃", "武器ディレイ +10%"),
        ("silence-blade", "静寂の刃", "武器ディレイ +15%"),
    ] {
        out.push(fixed_ability(
            id,
            name,
            EquipmentAbilityFamily::WeaponDelay,
            3,
            EquipmentValues::default(),
            summary,
            true,
        ));
    }

    for (id, name, value) in [
        ("ancient-pointed-blade", "古代精霊の尖った刃", 11),
        ("abyss-pointed-blade", "深淵の尖った刃", 13),
        ("loss-pointed-blade", "喪失の尖った刃", 17),
        ("night-star-pointed-blade", "夜星の尖った刃", 20),
    ] {
        out.push(new_ability(
            id,
            name,
            EquipmentAbilityFamily::PointedBlade,
            a(value, 0, 0, 0),
            match value {
                11 => "突き +11",
                13 => "突き +13",
                17 => "突き +17",
                _ => "突き +20",
            },
        ));
    }
    for (id, name, value) in [
        ("ancient-sharp-blade", "古代精霊の鋭い刃", 11),
        ("abyss-sharp-blade", "深淵の鋭い刃", 13),
        ("loss-sharp-blade", "喪失の鋭い刃", 17),
        ("night-star-sharp-blade", "夜星の鋭い刃", 20),
    ] {
        out.push(new_ability(
            id,
            name,
            EquipmentAbilityFamily::SharpBlade,
            a(0, value, 0, 0),
            match value {
                11 => "斬り +11",
                13 => "斬り +13",
                17 => "斬り +17",
                _ => "斬り +20",
            },
        ));
    }
    for (id, name, value) in [
        ("ancient-intelligence", "古代精霊の知力", 11),
        ("abyss-intelligence", "深淵の知力", 13),
        ("loss-intelligence", "喪失の知力", 17),
        ("night-star-intelligence", "夜星の知力", 20),
    ] {
        out.push(new_ability(
            id,
            name,
            EquipmentAbilityFamily::Intelligence,
            a(0, 0, value, 0),
            match value {
                11 => "魔攻 +11",
                13 => "魔攻 +13",
                17 => "魔攻 +17",
                _ => "魔攻 +20",
            },
        ));
    }
    for (id, name, value) in [
        ("ancient-magic-resistance", "古代精霊の耐魔力", 11),
        ("abyss-magic-resistance", "深淵の耐魔力", 13),
        ("loss-magic-resistance", "喪失の耐魔力", 17),
        ("night-star-magic-resistance", "夜星の耐魔力", 20),
    ] {
        out.push(new_ability(
            id,
            name,
            EquipmentAbilityFamily::MagicResistance,
            a(0, 0, 0, value),
            match value {
                11 => "魔防 +11",
                13 => "魔防 +13",
                17 => "魔防 +17",
                _ => "魔防 +20",
            },
        ));
    }

    // 武器以外は現環境で使う最上位を収録する。ランダム実測値の部位は
    // 範囲を要約し、固定値として計算へ混ぜない。
    out.push(slot_ability(
        "helm-e-skill-attack",
        "E-スキル攻撃力増加",
        PartSlot::Helm,
        EquipmentAbilityFamily::SkillAttack,
        "helm-skill-attack",
        EquipmentValues::default(),
        "スキル攻撃力 +10",
        false,
        HELM_SKILL_10,
    ));

    for (id, name, family, values, summary) in [
        (
            "upper-armor-polish",
            "(上)鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            EquipmentValues {
                physical_defense: 40,
                ..EquipmentValues::default()
            },
            "物防 +40",
        ),
        (
            "upper-magic-resistance-armor",
            "(上)魔法耐性・鎧",
            EquipmentAbilityFamily::MagicResistance,
            EquipmentValues {
                magic_defense: 30,
                ..EquipmentValues::default()
            },
            "魔防 +30",
        ),
        (
            "upper-evasion-armor",
            "(上)機敏",
            EquipmentAbilityFamily::Evasion,
            EquipmentValues {
                evasion: 3,
                ..EquipmentValues::default()
            },
            "回避 +3",
        ),
    ] {
        out.push(fixed_slot_ability(
            id,
            name,
            PartSlot::Armor,
            family,
            2,
            "armor-category-2",
            values,
            summary,
        ));
    }

    for (id, name, family, values, summary, record_only) in [
        (
            "night-star-vitality-armor",
            "夜星の生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +30,000",
            true,
        ),
        (
            "night-star-mana-armor",
            "夜星のマナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +9,000",
            true,
        ),
        (
            "night-star-armor-polish",
            "夜星の鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            EquipmentValues {
                physical_defense: 60,
                ..EquipmentValues::default()
            },
            "物防 +60",
            false,
        ),
        (
            "night-star-magic-resistance-armor",
            "夜星の魔法耐性(鎧)",
            EquipmentAbilityFamily::MagicResistance,
            EquipmentValues {
                magic_defense: 60,
                ..EquipmentValues::default()
            },
            "魔防 +60",
            false,
        ),
        (
            "night-star-evasion-armor",
            "夜星の機敏",
            EquipmentAbilityFamily::Evasion,
            EquipmentValues {
                evasion: 16,
                ..EquipmentValues::default()
            },
            "回避 +16",
            false,
        ),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::Armor,
            family,
            "armor-ability",
            values,
            summary,
            record_only,
            &[],
        ));
    }

    for (id, name, family, values, summary) in [
        (
            "night-star-shield-polish",
            "夜星の盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            EquipmentValues {
                physical_defense: 30,
                ..EquipmentValues::default()
            },
            "物防 +30",
        ),
        (
            "night-star-magic-resistance-shield",
            "夜星の魔法耐性(盾)",
            EquipmentAbilityFamily::MagicResistance,
            EquipmentValues {
                magic_defense: 15,
                ..EquipmentValues::default()
            },
            "魔防 +15",
        ),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::Shield,
            family,
            "shield-ability",
            values,
            summary,
            false,
            &[],
        ));
    }

    for (id, name, family, summary) in [
        (
            "mystic-mine-accuracy",
            "神秘鉱の的中剣",
            EquipmentAbilityFamily::Accuracy,
            "命中 +7〜13",
        ),
        (
            "mystic-mine-evasion",
            "神秘鉱の機敏",
            EquipmentAbilityFamily::Evasion,
            "回避 +7〜13",
        ),
        (
            "mystic-mine-pointed-blade",
            "神秘鉱の尖った刃",
            EquipmentAbilityFamily::PointedBlade,
            "突き +7〜15",
        ),
        (
            "mystic-mine-sharp-blade",
            "神秘鉱の鋭い刃",
            EquipmentAbilityFamily::SharpBlade,
            "斬り +7〜15",
        ),
        (
            "mystic-mine-intelligence",
            "神秘鉱の知力",
            EquipmentAbilityFamily::Intelligence,
            "魔攻 +7〜15",
        ),
        (
            "mystic-mine-magic-resistance",
            "神秘鉱の耐魔力",
            EquipmentAbilityFamily::MagicResistance,
            "魔防 +7〜15",
        ),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::ShieldPlus,
            family,
            id,
            EquipmentValues::default(),
            summary,
            false,
            &[],
        ));
    }

    for (id, name, summary) in [
        ("g-fire-moonstone", "G-火の月石", "火属性 +20"),
        ("g-water-moonstone", "G-水の月石", "水属性 +20"),
        ("g-wind-moonstone", "G-風の月石", "風属性 +20"),
        ("g-earth-moonstone", "G-土の月石", "土属性 +20"),
        ("g-lightning-moonstone", "G-雷の月石", "雷属性 +20"),
        ("g-white-moonstone", "G-白の月石", "白属性 +20"),
        ("g-dark-moonstone", "G-黒の月石", "黒属性 +20"),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::Head,
            EquipmentAbilityFamily::Element,
            "head-element",
            EquipmentValues::default(),
            summary,
            true,
            &[],
        ));
    }

    for (id, name, family, values, summary) in [
        (
            "night-star-critical-hand",
            "夜星の致命打",
            EquipmentAbilityFamily::Critical,
            EquipmentValues {
                critical: 15,
                ..EquipmentValues::default()
            },
            "クリティカル +15",
        ),
        (
            "night-star-accuracy-hand",
            "夜星の的中剣",
            EquipmentAbilityFamily::Accuracy,
            EquipmentValues {
                accuracy: 16,
                ..EquipmentValues::default()
            },
            "命中 +16",
        ),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::Hand,
            family,
            "hand-ability",
            values,
            summary,
            false,
            &[],
        ));
    }
    for (id, name, family, values, summary) in [
        (
            "upper-critical-hand",
            "(上)致命打",
            EquipmentAbilityFamily::Critical,
            EquipmentValues {
                critical: 3,
                ..EquipmentValues::default()
            },
            "クリティカル +3",
        ),
        (
            "upper-accuracy-hand",
            "(上)的中剣",
            EquipmentAbilityFamily::Accuracy,
            EquipmentValues {
                accuracy: 3,
                ..EquipmentValues::default()
            },
            "命中 +3",
        ),
    ] {
        out.push(fixed_slot_ability(
            id,
            name,
            PartSlot::Hand,
            family,
            3,
            "hand-category-3",
            values,
            summary,
        ));
    }

    out.push(slot_ability(
        "night-star-agility-leg",
        "夜星の敏捷",
        PartSlot::Leg,
        EquipmentAbilityFamily::Agility,
        "leg-ability",
        EquipmentValues::default(),
        "移動速度 +12",
        true,
        &[],
    ));

    for (id, name, family, summary) in [
        (
            "rest-pointed-blade",
            "安息の尖った刃",
            EquipmentAbilityFamily::PointedBlade,
            "突き +1〜15",
        ),
        (
            "rest-sharp-blade",
            "安息の鋭い刃",
            EquipmentAbilityFamily::SharpBlade,
            "斬り +1〜15",
        ),
        (
            "rest-intelligence",
            "安息の知力",
            EquipmentAbilityFamily::Intelligence,
            "魔攻 +1〜15",
        ),
        (
            "rest-magic-resistance",
            "安息の耐魔力",
            EquipmentAbilityFamily::MagicResistance,
            "魔防 +1〜15",
        ),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::RelicPendant,
            family,
            "relic-pendant-ability",
            EquipmentValues::default(),
            summary,
            false,
            &[],
        ));
    }
    for (id, name, family, summary) in [
        (
            "immortal-accuracy",
            "不死の的中剣",
            EquipmentAbilityFamily::Accuracy,
            "命中 +1〜13",
        ),
        (
            "immortal-evasion",
            "不死の機敏",
            EquipmentAbilityFamily::Evasion,
            "回避 +1〜13",
        ),
        (
            "immortal-critical",
            "不死の致命打",
            EquipmentAbilityFamily::Critical,
            "クリティカル +1〜12",
        ),
        (
            "immortal-vitality",
            "不死の生命力",
            EquipmentAbilityFamily::Vitality,
            "最大HP +6,000〜10,000",
        ),
    ] {
        out.push(slot_ability(
            id,
            name,
            PartSlot::RelicBracelet,
            family,
            "relic-bracelet-ability",
            EquipmentValues::default(),
            summary,
            id == "immortal-vitality",
            &[],
        ));
    }

    // 装備システム/アビリティ カテゴリー4(N- / R- / L- / E- 系)。装備システム UI から
    // SEED で付ける現行の等級ラダーで、上の夜星系(アビリティアイテム方式)とは入手経路が別。
    // 表の 1 列目(効果)を装備補正へ入れ、2 列目(追加効果)はダメージ増加だけ X3 へ入れる。
    // 最大HP/最大MP・移動速度・属性は `EquipmentValues` に受け皿が無いので record_only。
    for (id, name, summary, effects) in [
        (
            "helm-r-skill-attack",
            "R-スキル攻撃力増加",
            "スキル攻撃力 +1",
            HELM_SKILL_1,
        ),
        (
            "helm-l-skill-attack",
            "L-スキル攻撃力増加",
            "スキル攻撃力 +5",
            HELM_SKILL_5,
        ),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Helm,
            EquipmentAbilityFamily::SkillAttack,
            "helm-skill-attack",
            EquipmentValues::default(),
            summary,
            false,
            effects,
        ));
    }

    for (id, name, family, values, summary, record_only) in [
        (
            "n-armor-polish",
            "N-鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(15),
            "物防 +15",
            false,
        ),
        (
            "r-armor-polish",
            "R-鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(17),
            "物防 +17・防御率 +5%",
            false,
        ),
        (
            "l-armor-polish",
            "L-鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(19),
            "物防 +19・防御率 +5%",
            false,
        ),
        (
            "e-armor-polish",
            "E-鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(30),
            "物防 +30・防御率 +6%",
            false,
        ),
        (
            "n-magic-resistance-armor",
            "N-魔法耐性・鎧",
            EquipmentAbilityFamily::MagicResistance,
            md(16),
            "魔防 +16",
            false,
        ),
        (
            "r-magic-resistance-armor",
            "R-魔法耐性・鎧",
            EquipmentAbilityFamily::MagicResistance,
            md(18),
            "魔防 +18・防御率 +5%",
            false,
        ),
        (
            "l-magic-resistance-armor",
            "L-魔法耐性・鎧",
            EquipmentAbilityFamily::MagicResistance,
            md(23),
            "魔防 +23・防御率 +5%",
            false,
        ),
        (
            "e-magic-resistance-armor",
            "E-魔法耐性・鎧",
            EquipmentAbilityFamily::MagicResistance,
            md(30),
            "魔防 +30・防御率 +6%",
            false,
        ),
        (
            "n-evasion-armor",
            "N-機敏",
            EquipmentAbilityFamily::Evasion,
            ev(4),
            "回避 +4",
            false,
        ),
        (
            "r-evasion-armor",
            "R-機敏",
            EquipmentAbilityFamily::Evasion,
            ev(5),
            "回避 +5・回避率 +3%",
            false,
        ),
        (
            "l-evasion-armor",
            "L-機敏",
            EquipmentAbilityFamily::Evasion,
            ev(6),
            "回避 +6・回避率 +4%",
            false,
        ),
        (
            "e-evasion-armor",
            "E-機敏",
            EquipmentAbilityFamily::Evasion,
            ev(7),
            "回避 +7・回避率 +5%",
            false,
        ),
        (
            "n-vitality-armor",
            "N-生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +300",
            true,
        ),
        (
            "r-vitality-armor",
            "R-生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +330",
            true,
        ),
        (
            "l-vitality-armor",
            "L-生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +350",
            true,
        ),
        (
            "e-vitality-armor",
            "E-生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +1,000",
            true,
        ),
        (
            "n-mana-armor",
            "N-マナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +200",
            true,
        ),
        (
            "r-mana-armor",
            "R-マナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +225",
            true,
        ),
        (
            "l-mana-armor",
            "L-マナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +250",
            true,
        ),
        (
            "e-mana-armor",
            "E-マナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +1,000",
            true,
        ),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Armor,
            family,
            "armor-ability",
            values,
            summary,
            record_only,
            &[],
        ));
    }

    for (id, name, family, values, summary, effects) in [
        (
            "n-pointed-blade",
            "N-尖った刃",
            EquipmentAbilityFamily::PointedBlade,
            a(6, 0, 0, 0),
            "突き +6",
            NO_EFFECTS,
        ),
        (
            "r-pointed-blade",
            "R-尖った刃",
            EquipmentAbilityFamily::PointedBlade,
            a(7, 0, 0, 0),
            "突き +7・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "l-pointed-blade",
            "L-尖った刃",
            EquipmentAbilityFamily::PointedBlade,
            a(8, 0, 0, 0),
            "突き +8・与ダメ +4%",
            ATTACK_DAMAGE_X3_4,
        ),
        (
            "e-pointed-blade",
            "E-尖った刃",
            EquipmentAbilityFamily::PointedBlade,
            a(9, 0, 0, 0),
            "突き +9・与ダメ +6%",
            ATTACK_DAMAGE_X3_6,
        ),
        (
            "n-sharp-blade",
            "N-鋭い刃",
            EquipmentAbilityFamily::SharpBlade,
            a(0, 6, 0, 0),
            "斬り +6",
            NO_EFFECTS,
        ),
        (
            "r-sharp-blade",
            "R-鋭い刃",
            EquipmentAbilityFamily::SharpBlade,
            a(0, 7, 0, 0),
            "斬り +7・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "l-sharp-blade",
            "L-鋭い刃",
            EquipmentAbilityFamily::SharpBlade,
            a(0, 8, 0, 0),
            "斬り +8・与ダメ +4%",
            ATTACK_DAMAGE_X3_4,
        ),
        (
            "e-sharp-blade",
            "E-鋭い刃",
            EquipmentAbilityFamily::SharpBlade,
            a(0, 9, 0, 0),
            "斬り +9・与ダメ +6%",
            ATTACK_DAMAGE_X3_6,
        ),
        (
            "n-intelligence",
            "N-知力",
            EquipmentAbilityFamily::Intelligence,
            a(0, 0, 6, 0),
            "魔攻 +6",
            NO_EFFECTS,
        ),
        (
            "r-intelligence",
            "R-知力",
            EquipmentAbilityFamily::Intelligence,
            a(0, 0, 7, 0),
            "魔攻 +7・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "l-intelligence",
            "L-知力",
            EquipmentAbilityFamily::Intelligence,
            a(0, 0, 8, 0),
            "魔攻 +8・与ダメ +4%",
            ATTACK_DAMAGE_X3_4,
        ),
        (
            "e-intelligence",
            "E-知力",
            EquipmentAbilityFamily::Intelligence,
            a(0, 0, 9, 0),
            "魔攻 +9・与ダメ +6%",
            ATTACK_DAMAGE_X3_6,
        ),
        (
            "n-magic-resistance",
            "N-耐魔力",
            EquipmentAbilityFamily::MagicResistance,
            a(0, 0, 0, 6),
            "魔防 +6",
            NO_EFFECTS,
        ),
        (
            "r-magic-resistance",
            "R-耐魔力",
            EquipmentAbilityFamily::MagicResistance,
            a(0, 0, 0, 7),
            "魔防 +7・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "l-magic-resistance",
            "L-耐魔力",
            EquipmentAbilityFamily::MagicResistance,
            a(0, 0, 0, 8),
            "魔防 +8・与ダメ +4%",
            ATTACK_DAMAGE_X3_4,
        ),
        (
            "e-magic-resistance",
            "E-耐魔力",
            EquipmentAbilityFamily::MagicResistance,
            a(0, 0, 0, 9),
            "魔防 +9・与ダメ +6%",
            ATTACK_DAMAGE_X3_6,
        ),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Weapon,
            family,
            "weapon-category-4",
            values,
            summary,
            false,
            effects,
        ));
    }

    // 失われた魂は本体効果が最大HPで計算へ入らない。破線(record_only)で出す以上、
    // 追加効果のダメージ増加だけをこっそり合計へ混ぜると表示と食い違うので入れない。
    for (id, name, summary) in [
        ("n-lost-soul", "N-失われた魂", "最大HP +1,000・与ダメ +4%"),
        ("r-lost-soul", "R-失われた魂", "最大HP +2,000・与ダメ +5%"),
        ("l-lost-soul", "L-失われた魂", "最大HP +4,000・与ダメ +6%"),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Weapon,
            EquipmentAbilityFamily::Vitality,
            "weapon-category-4",
            EquipmentValues::default(),
            summary,
            true,
            &[],
        ));
    }

    for (id, name, family, values, summary) in [
        (
            "n-shield-polish",
            "N-盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(13),
            "物防 +13",
        ),
        (
            "r-shield-polish",
            "R-盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(14),
            "物防 +14・防御率 +4%",
        ),
        (
            "l-shield-polish",
            "L-盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(15),
            "物防 +15・防御率 +4%",
        ),
        (
            "e-shield-polish",
            "E-盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(16),
            "物防 +16・防御率 +5%",
        ),
        (
            "n-magic-resistance-shield",
            "N-魔法耐性・盾",
            EquipmentAbilityFamily::MagicResistance,
            md(4),
            "魔防 +4",
        ),
        (
            "r-magic-resistance-shield",
            "R-魔法耐性・盾",
            EquipmentAbilityFamily::MagicResistance,
            md(5),
            "魔防 +5・防御率 +4%",
        ),
        (
            "l-magic-resistance-shield",
            "L-魔法耐性・盾",
            EquipmentAbilityFamily::MagicResistance,
            md(6),
            "魔防 +6・防御率 +4%",
        ),
        (
            "e-magic-resistance-shield",
            "E-魔法耐性・盾",
            EquipmentAbilityFamily::MagicResistance,
            md(7),
            "魔防 +7・防御率 +5%",
        ),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Shield,
            family,
            "shield-ability",
            values,
            summary,
            false,
            &[],
        ));
    }

    // 月石は最上位(G-)だけ既収録だった。属性値は装備補正 9 値に無いので record_only。
    for (id, name, summary) in [
        ("n-fire-moonstone", "N-火の月石", "火属性 +5"),
        ("r-fire-moonstone", "R-火の月石", "火属性 +10"),
        ("l-fire-moonstone", "L-火の月石", "火属性 +15"),
        ("n-water-moonstone", "N-水の月石", "水属性 +5"),
        ("r-water-moonstone", "R-水の月石", "水属性 +10"),
        ("l-water-moonstone", "L-水の月石", "水属性 +15"),
        ("n-wind-moonstone", "N-風の月石", "風属性 +5"),
        ("r-wind-moonstone", "R-風の月石", "風属性 +10"),
        ("l-wind-moonstone", "L-風の月石", "風属性 +15"),
        ("n-earth-moonstone", "N-土の月石", "土属性 +5"),
        ("r-earth-moonstone", "R-土の月石", "土属性 +10"),
        ("l-earth-moonstone", "L-土の月石", "土属性 +15"),
        ("n-lightning-moonstone", "N-雷の月石", "雷属性 +5"),
        ("r-lightning-moonstone", "R-雷の月石", "雷属性 +10"),
        ("l-lightning-moonstone", "L-雷の月石", "雷属性 +15"),
        ("n-white-moonstone", "N-白の月石", "白属性 +5"),
        ("r-white-moonstone", "R-白の月石", "白属性 +10"),
        ("l-white-moonstone", "L-白の月石", "白属性 +15"),
        ("n-dark-moonstone", "N-黒の月石", "黒属性 +5"),
        ("r-dark-moonstone", "R-黒の月石", "黒属性 +10"),
        ("l-dark-moonstone", "L-黒の月石", "黒属性 +15"),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Head,
            EquipmentAbilityFamily::Element,
            "head-element",
            EquipmentValues::default(),
            summary,
            true,
            &[],
        ));
    }

    for (id, name, family, values, summary, effects) in [
        (
            "n-critical-hand",
            "N-致命打",
            EquipmentAbilityFamily::Critical,
            cr(4),
            "クリティカル +4",
            NO_EFFECTS,
        ),
        (
            "r-critical-hand",
            "R-致命打",
            EquipmentAbilityFamily::Critical,
            cr(5),
            "クリティカル +5・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "l-critical-hand",
            "L-致命打",
            EquipmentAbilityFamily::Critical,
            cr(6),
            "クリティカル +6・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "e-critical-hand",
            "E-致命打",
            EquipmentAbilityFamily::Critical,
            cr(7),
            "クリティカル +7・与ダメ +4%",
            ATTACK_DAMAGE_X3_4,
        ),
        (
            "n-accuracy-hand",
            "N-的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(4),
            "命中 +4",
            NO_EFFECTS,
        ),
        (
            "r-accuracy-hand",
            "R-的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(5),
            "命中 +5・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "l-accuracy-hand",
            "L-的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(6),
            "命中 +6・与ダメ +3%",
            ATTACK_DAMAGE_X3_3,
        ),
        (
            "e-accuracy-hand",
            "E-的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(7),
            "命中 +7・与ダメ +4%",
            ATTACK_DAMAGE_X3_4,
        ),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Hand,
            family,
            "hand-ability",
            values,
            summary,
            false,
            effects,
        ));
    }

    // 足の R/L/E は wiki「装備システム/アビリティ」側の値(移動速度 +1〜+3)。
    // 新装着アビリティ側の敏捷(古代精霊 +5 〜 夜星 +12)とは別体系で、**実際に使われるのは
    // 新装着側**(R/L/E は性能が低く基本使わない。ユーザー確認 2026-09-01)。
    // 消さずに残すのは、付いている装備を持っている人が表現できなくなるため(既定では畳んだ側に出る)
    for (id, name, summary) in [
        ("r-agility-leg", "R-敏捷", "移動速度 +1・回避率 +1%"),
        ("l-agility-leg", "L-敏捷", "移動速度 +2・回避率 +2%"),
        ("e-agility-leg", "E-敏捷", "移動速度 +3・回避率 +3%"),
    ] {
        out.push(ui_category4(
            id,
            name,
            PartSlot::Leg,
            EquipmentAbilityFamily::Agility,
            "leg-ability",
            EquipmentValues::default(),
            summary,
            true,
            &[],
        ));
    }

    // 新装着アビリティの下位3系列(古代精霊 LV.300 / 深淵 LV.310 / 喪失 LV.310)。
    // これまで最上位の夜星しか収録しておらず、実際に多くのキャラが着けている喪失系が
    // 一覧に出なかった。系列は 古代精霊 < 深淵 < 喪失 < 夜星 の順で、入手地域は
    // 順に リンゴの島 / アークロン要塞 / エクリプス / ゆがんだ村。
    // 追加アビリティ2枠の値域も系列ごとに違うので、`slot_ability` の夜星既定を上書きする。
    // 出典: Item/合成/装着アビリティシステム/新装着アビリティ(取得 2026-09-01)。
    for (id, name, family, values, summary, record_only, options) in [
        (
            "ancient-vitality-armor",
            "古代精霊の生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +8,000",
            true,
            vec![
                add_opt(DamageResistance, 8, 8),
                add_opt(PhysicalDamageReduction, 80, 80),
                add_opt(PhysicalDefense, 6, 10),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
            ],
        ),
        (
            "abyss-vitality-armor",
            "深淵の生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +10,000",
            true,
            vec![
                add_opt(DamageResistance, 9, 9),
                add_opt(PhysicalDamageReduction, 100, 100),
                add_opt(PhysicalDefense, 8, 12),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "loss-vitality-armor",
            "喪失の生命力",
            EquipmentAbilityFamily::Vitality,
            EquipmentValues::default(),
            "最大HP +12,000",
            true,
            vec![
                add_opt(DamageResistance, 10, 10),
                add_opt(PhysicalDamageReduction, 100, 100),
                add_opt(PhysicalDefense, 8, 14),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "ancient-mana-armor",
            "古代精霊のマナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +3,000",
            true,
            vec![
                add_opt(DamageResistance, 8, 8),
                add_opt(MagicDamageReduction, 80, 80),
                add_opt(MagicDefense, 4, 8),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
            ],
        ),
        (
            "abyss-mana-armor",
            "深淵のマナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +5,000",
            true,
            vec![
                add_opt(DamageResistance, 9, 9),
                add_opt(MagicDamageReduction, 100, 100),
                add_opt(MagicDefense, 6, 10),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "loss-mana-armor",
            "喪失のマナ",
            EquipmentAbilityFamily::Mana,
            EquipmentValues::default(),
            "最大MP +7,000",
            true,
            vec![
                add_opt(DamageResistance, 10, 10),
                add_opt(MagicDamageReduction, 100, 100),
                add_opt(MagicDefense, 6, 12),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "ancient-armor-polish",
            "古代精霊の鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(35),
            "物防 +35",
            false,
            vec![
                add_opt(DamageResistance, 8, 8),
                add_opt(PhysicalDamageReduction, 80, 80),
                add_opt(PhysicalDefense, 6, 10),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
            ],
        ),
        (
            "abyss-armor-polish",
            "深淵の鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(40),
            "物防 +40",
            false,
            vec![
                add_opt(DamageResistance, 9, 9),
                add_opt(PhysicalDamageReduction, 100, 100),
                add_opt(PhysicalDefense, 8, 12),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "loss-armor-polish",
            "喪失の鎧研磨",
            EquipmentAbilityFamily::ArmorPolish,
            pd(45),
            "物防 +45",
            false,
            vec![
                add_opt(DamageResistance, 10, 10),
                add_opt(PhysicalDamageReduction, 100, 100),
                add_opt(PhysicalDefense, 8, 14),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "ancient-magic-resistance-armor",
            "古代精霊の魔法耐性(鎧)",
            EquipmentAbilityFamily::MagicResistance,
            md(35),
            "魔防 +35",
            false,
            vec![
                add_opt(DamageResistance, 8, 8),
                add_opt(MagicDamageReduction, 80, 80),
                add_opt(MagicDefense, 4, 8),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
            ],
        ),
        (
            "abyss-magic-resistance-armor",
            "深淵の魔法耐性(鎧)",
            EquipmentAbilityFamily::MagicResistance,
            md(40),
            "魔防 +40",
            false,
            vec![
                add_opt(DamageResistance, 9, 9),
                add_opt(MagicDamageReduction, 100, 100),
                add_opt(MagicDefense, 6, 10),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "loss-magic-resistance-armor",
            "喪失の魔法耐性(鎧)",
            EquipmentAbilityFamily::MagicResistance,
            md(45),
            "魔防 +45",
            false,
            vec![
                add_opt(DamageResistance, 10, 10),
                add_opt(MagicDamageReduction, 100, 100),
                add_opt(MagicDefense, 6, 12),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "ancient-evasion-armor",
            "古代精霊の機敏",
            EquipmentAbilityFamily::Evasion,
            ev(9),
            "回避 +9",
            false,
            vec![
                add_opt(EvasionRate, 6, 12),
                add_opt(PhysicalDamageReduction, 50, 50),
                add_opt(MagicDamageReduction, 50, 50),
                add_opt(SpRecovery, 4, 14),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
            ],
        ),
        (
            "abyss-evasion-armor",
            "深淵の機敏",
            EquipmentAbilityFamily::Evasion,
            ev(11),
            "回避 +11",
            false,
            vec![
                add_opt(EvasionRate, 8, 14),
                add_opt(PhysicalDamageReduction, 70, 70),
                add_opt(MagicDamageReduction, 70, 70),
                add_opt(SpRecovery, 6, 16),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
        (
            "loss-evasion-armor",
            "喪失の機敏",
            EquipmentAbilityFamily::Evasion,
            ev(13),
            "回避 +13",
            false,
            vec![
                add_opt(EvasionRate, 8, 16),
                add_opt(PhysicalDamageReduction, 70, 70),
                add_opt(MagicDamageReduction, 70, 70),
                add_opt(SpRecovery, 6, 16),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
            ],
        ),
    ] {
        out.push(line_ability(
            id,
            name,
            PartSlot::Armor,
            family,
            "armor-ability",
            values,
            summary,
            record_only,
            options,
        ));
    }

    for (id, name, family, values, summary, options) in [
        (
            "ancient-shield-polish",
            "古代精霊の盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(18),
            "物防 +18",
            vec![
                add_opt(DamageResistance, 7, 7),
                add_opt(PhysicalDamageReduction, 50, 50),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
                add_opt(SpRecovery, 4, 14),
                add_opt(EvasionRate, 6, 12),
            ],
        ),
        (
            "abyss-shield-polish",
            "深淵の盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(20),
            "物防 +20",
            vec![
                add_opt(DamageResistance, 8, 8),
                add_opt(PhysicalDamageReduction, 70, 70),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
                add_opt(SpRecovery, 6, 16),
                add_opt(EvasionRate, 8, 14),
            ],
        ),
        (
            "loss-shield-polish",
            "喪失の盾研磨",
            EquipmentAbilityFamily::ShieldPolish,
            pd(22),
            "物防 +22",
            vec![
                add_opt(DamageResistance, 9, 9),
                add_opt(PhysicalDamageReduction, 70, 70),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
                add_opt(SpRecovery, 6, 16),
                add_opt(EvasionRate, 8, 16),
            ],
        ),
        (
            "ancient-magic-resistance-shield",
            "古代精霊の魔法耐性(盾)",
            EquipmentAbilityFamily::MagicResistance,
            md(9),
            "魔防 +9",
            vec![
                add_opt(DamageResistance, 7, 7),
                add_opt(MagicDamageReduction, 50, 50),
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
                add_opt(SpRecovery, 4, 14),
                add_opt(EvasionRate, 6, 12),
            ],
        ),
        (
            "abyss-magic-resistance-shield",
            "深淵の魔法耐性(盾)",
            EquipmentAbilityFamily::MagicResistance,
            md(11),
            "魔防 +11",
            vec![
                add_opt(DamageResistance, 8, 8),
                add_opt(MagicDamageReduction, 70, 70),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
                add_opt(SpRecovery, 6, 16),
                add_opt(EvasionRate, 8, 14),
            ],
        ),
        (
            "loss-magic-resistance-shield",
            "喪失の魔法耐性(盾)",
            EquipmentAbilityFamily::MagicResistance,
            md(13),
            "魔防 +13",
            vec![
                add_opt(DamageResistance, 9, 9),
                add_opt(MagicDamageReduction, 70, 70),
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
                add_opt(SpRecovery, 6, 16),
                add_opt(EvasionRate, 8, 16),
            ],
        ),
    ] {
        out.push(line_ability(
            id,
            name,
            PartSlot::Shield,
            family,
            "shield-ability",
            values,
            summary,
            false,
            options,
        ));
    }

    for (id, name, family, values, summary, options) in [
        (
            "ancient-critical-hand",
            "古代精霊の致命打",
            EquipmentAbilityFamily::Critical,
            cr(8),
            "クリティカル +8",
            vec![
                add_opt(FixedDamage, 5000, 5000),
                add_opt(DamageRate, 6, 6),
                add_opt(Thrust, 4, 8),
                add_opt(Slash, 4, 8),
                add_opt(MagicAttack, 4, 8),
                add_opt(MagicDefense, 4, 8),
            ],
        ),
        (
            "abyss-critical-hand",
            "深淵の致命打",
            EquipmentAbilityFamily::Critical,
            cr(10),
            "クリティカル +10",
            vec![
                add_opt(FixedDamage, 6000, 6000),
                add_opt(DamageRate, 7, 7),
                add_opt(Thrust, 6, 10),
                add_opt(Slash, 6, 10),
                add_opt(MagicAttack, 6, 10),
                add_opt(MagicDefense, 6, 10),
            ],
        ),
        (
            "loss-critical-hand",
            "喪失の致命打",
            EquipmentAbilityFamily::Critical,
            cr(12),
            "クリティカル +12",
            vec![
                add_opt(FixedDamage, 7000, 7000),
                add_opt(DamageRate, 8, 8),
                add_opt(Thrust, 6, 12),
                add_opt(Slash, 6, 12),
                add_opt(MagicAttack, 6, 12),
                add_opt(MagicDefense, 6, 12),
            ],
        ),
        (
            "ancient-accuracy-hand",
            "古代精霊の的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(9),
            "命中 +9",
            vec![
                add_opt(FixedDamage, 5000, 5000),
                add_opt(DamageRate, 6, 6),
                add_opt(Thrust, 4, 8),
                add_opt(Slash, 4, 8),
                add_opt(MagicAttack, 4, 8),
                add_opt(MagicDefense, 4, 8),
            ],
        ),
        (
            "abyss-accuracy-hand",
            "深淵の的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(11),
            "命中 +11",
            vec![
                add_opt(FixedDamage, 6000, 6000),
                add_opt(DamageRate, 7, 7),
                add_opt(Thrust, 6, 10),
                add_opt(Slash, 6, 10),
                add_opt(MagicAttack, 6, 10),
                add_opt(MagicDefense, 6, 10),
            ],
        ),
        (
            "loss-accuracy-hand",
            "喪失の的中剣",
            EquipmentAbilityFamily::Accuracy,
            ac(13),
            "命中 +13",
            vec![
                add_opt(FixedDamage, 7000, 7000),
                add_opt(DamageRate, 8, 8),
                add_opt(Thrust, 6, 12),
                add_opt(Slash, 6, 12),
                add_opt(MagicAttack, 6, 12),
                add_opt(MagicDefense, 6, 12),
            ],
        ),
    ] {
        out.push(line_ability(
            id,
            name,
            PartSlot::Hand,
            family,
            "hand-ability",
            values,
            summary,
            false,
            options,
        ));
    }

    for (id, name, summary, options) in [
        (
            "ancient-agility-leg",
            "古代精霊の敏捷",
            "移動速度 +5",
            vec![
                add_opt(HpRecovery, 4, 14),
                add_opt(MpRecovery, 4, 14),
                add_opt(SpRecovery, 4, 14),
                add_opt(EvasionRate, 4, 8),
            ],
        ),
        (
            "abyss-agility-leg",
            "深淵の敏捷",
            "移動速度 +7",
            vec![
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
                add_opt(SpRecovery, 6, 16),
                add_opt(EvasionRate, 6, 10),
            ],
        ),
        (
            "loss-agility-leg",
            "喪失の敏捷",
            "移動速度 +9",
            vec![
                add_opt(HpRecovery, 6, 16),
                add_opt(MpRecovery, 6, 16),
                add_opt(SpRecovery, 6, 16),
                add_opt(EvasionRate, 6, 12),
            ],
        ),
    ] {
        out.push(line_ability(
            id,
            name,
            PartSlot::Leg,
            EquipmentAbilityFamily::Agility,
            "leg-ability",
            EquipmentValues::default(),
            summary,
            true,
            options,
        ));
    }

    out
}
