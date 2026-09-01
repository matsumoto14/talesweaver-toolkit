//! 装備カタログ・装備強化・装着アビリティの検証。

// 検証だけが使う私物(値の組み立てヘルパーと出典の生データ)。公開 API ではないので
// `pub use` には載せず、ここから直接引く。
use super::abilities::a;
use super::items::{
    v, wrist_type_from_page, SURVIVAL_DEFENSE_RATE_30, SURVIVAL_MITIGATION_10,
    SURVIVAL_MITIGATION_15, SURVIVAL_MITIGATION_40,
};

use domain::DamageCategory;

/// テスト用: `DamageContribution` を (カテゴリ, 値) に落として比較しやすくする。
fn pairs(contributions: &[domain::DamageContribution]) -> Vec<(DamageCategory, f64)> {
    contributions
        .iter()
        .map(|c| (c.category, c.value))
        .collect()
}

/// 追加アビリティは抽選結果なので、登録した基本アビリティから自動適用しない。
/// カテゴリー4でもランダム追加枠を持つのはアビリティアイテム方式(夜星系)だけで、
/// 装備システム UI の N-/R-/L-/E- 系は表で追加効果が決まっているため枠を持たない。
#[test]
fn 追加アビリティは説明だけを持ち自動計算しない() {
    for def in equipment_abilities()
        .into_iter()
        .filter(|d| d.slot == PartSlot::Weapon && d.category == 4 && d.additional_slots > 0)
    {
        assert!(def.damage_effects.is_empty(), "{}", def.id);
        assert_eq!(def.additional_slots, 2, "{}", def.id);
        assert!(def.additional_effects.contains("ランダム"), "{}", def.id);
    }
}

use super::*;
use std::collections::HashSet;

fn wrist(
    item_id: &str,
    thrust: i64,
    agility: i64,
    enchant_thrust: i64,
    enchant_agility: i64,
) -> Equipment {
    let mut equipment = Equipment::default();
    equipment.parts.shield.item_id = Some(item_id.to_string());
    equipment.parts.shield.base = EquipmentValues {
        thrust,
        agility,
        ..Default::default()
    };
    equipment.parts.shield.enchant = EquipmentValues {
        thrust: enchant_thrust,
        agility: enchant_agility,
        ..Default::default()
    };
    equipment
}

#[test]
fn 上位装備カタログは780件_idは重複しない() {
    let catalog = equipment_catalog();
    // 既存の手検証済み行を優先し、2026-08-27 の全 Item ページ抽出を名前で重複排除。
    assert_eq!(catalog.len(), 780);
    let ids: HashSet<&str> = catalog.iter().map(|i| i.id).collect();
    assert_eq!(ids.len(), catalog.len());
}

#[test]
fn スタリオンサインは主能力700_その他255との差分をエンチャント枠にする() {
    let blue = find_equipment_item("stallion-sign-blue").unwrap();
    assert_eq!(blue.values_max, v(30, 5, 5, 5, 5, 35, 35, 35, 35));
    assert_eq!(
        blue.enchant_caps,
        v(670, 250, 250, 250, 250, 220, 220, 220, 220)
    );

    let yellow = find_equipment_item("stallion-sign-yellow").unwrap();
    assert_eq!(
        yellow.enchant_caps,
        v(250, 250, 250, 250, 670, 220, 220, 220, 220)
    );
}

#[test]
fn 腕種別はカタログのwiki区分から判定する() {
    assert_eq!(
        find_equipment_item("wiki-21ae0bc1de72").unwrap().wrist_type,
        Some(WristType::Band)
    );
    assert_eq!(
        wrist_type_from_page("韓国コミュニティ装備整理シート/밴드"),
        Some(WristType::Band)
    );
    assert_eq!(
        find_equipment_item("abyss-shield").unwrap().wrist_type,
        Some(WristType::Shield)
    );
    assert_eq!(
        find_equipment_item("rising-holic-cuffs")
            .unwrap()
            .wrist_type,
        None
    );
}

#[test]
fn ボリスとマキシミンは腕の突き基本とエンチャントを魔攻基本へ変換する() {
    let mut equipment = wrist("abyss-shield", 100, 0, 30, 0);
    equipment
        .siena
        .shield
        .registered
        .push(domain::RegisteredSienaAura {
            id: 1,
            label: String::new(),
            aura: domain::SienaAura {
                slots: vec![domain::SienaSlot {
                    kind: domain::SienaValueKind::Thrust,
                    value: 5,
                }],
                extras: vec![],
            },
        });
    equipment.siena.shield.selected_id = Some(1);
    let stats = BaseStats::default();
    let catalog = equipment_catalog();
    for character in ["boris", "maximin"] {
        let bonus = character_wrist_base_bonus(
            character,
            &stats,
            SkillDependency::HackInt,
            &equipment,
            &catalog,
        );
        assert_eq!(bonus.magic_attack, 135, "{character}");
        assert_eq!(bonus.thrust, 0, "元の突き値を移動せず派生値だけ返す");
        let base = equipment.base_totals(&[], &[]).add(bonus);
        let enhanced = equipment.enhanced_totals(None);
        assert_eq!(base.magic_attack, 135, "変換結果は基本能力値へ入る");
        assert_eq!(
            enhanced.thrust, 35,
            "元のエンチャント枠は強化能力値にも残る"
        );
    }
}

#[test]
fn バンド敏捷の七割をキャラと型に応じた基本補正へ変換する() {
    // (101 + 10) * 0.7 = 77.7 → 77
    let equipment = wrist("wiki-21ae0bc1de72", 0, 101, 0, 10);
    let catalog = equipment_catalog();
    let normal = BaseStats {
        hack: 200,
        mr: 100,
        ..Default::default()
    };
    let magic = BaseStats {
        hack: 100,
        mr: 200,
        ..Default::default()
    };

    assert_eq!(
        character_wrist_base_bonus(
            "nayatorei",
            &normal,
            SkillDependency::StabHack,
            &equipment,
            &catalog
        )
        .thrust,
        77
    );
    assert_eq!(
        character_wrist_base_bonus(
            "nayatorei",
            &normal,
            SkillDependency::Hack,
            &equipment,
            &catalog
        )
        .slash,
        77
    );
    assert_eq!(
        character_wrist_base_bonus(
            "isaac",
            &normal,
            SkillDependency::Stab,
            &equipment,
            &catalog
        )
        .thrust,
        77
    );
    assert_eq!(
        character_wrist_base_bonus(
            "mira",
            &normal,
            SkillDependency::Hack,
            &equipment,
            &catalog
        )
        .slash,
        77
    );
    assert_eq!(
        character_wrist_base_bonus(
            "benya",
            &normal,
            SkillDependency::Hack,
            &equipment,
            &catalog
        )
        .slash,
        77
    );
    assert_eq!(
        character_wrist_base_bonus("benya", &magic, SkillDependency::Mr, &equipment, &catalog)
            .magic_defense,
        77
    );
    assert_eq!(
        character_wrist_base_bonus(
            "roamini",
            &magic,
            SkillDependency::Int,
            &equipment,
            &catalog
        )
        .magic_attack,
        77
    );
}

#[test]
fn バンド以外と対象外キャラとベンヤ同値は変換しない() {
    let shield = wrist("abyss-shield", 0, 100, 0, 0);
    let band = wrist("wiki-21ae0bc1de72", 0, 100, 0, 0);
    let catalog = equipment_catalog();
    let equal = BaseStats {
        hack: 100,
        mr: 100,
        ..Default::default()
    };
    assert_eq!(
        character_wrist_base_bonus("mira", &equal, SkillDependency::Hack, &shield, &catalog),
        EquipmentValues::default()
    );
    assert_eq!(
        character_wrist_base_bonus("lucian", &equal, SkillDependency::Stab, &band, &catalog),
        EquipmentValues::default()
    );
    assert_eq!(
        character_wrist_base_bonus("benya", &equal, SkillDependency::Hack, &band, &catalog),
        EquipmentValues::default()
    );
}

#[test]
fn セイクリッド通常と改を全部位104件収録し固定エンチャント枠へ変換する() {
    let sacred: Vec<_> = equipment_catalog()
        .into_iter()
        .filter(|item| item.name.contains("セイクリッド"))
        .collect();
    assert_eq!(sacred.len(), 104);

    for item in sacred {
        let caps = [
            item.enchant_caps.thrust,
            item.enchant_caps.slash,
            item.enchant_caps.physical_defense,
            item.enchant_caps.magic_attack,
            item.enchant_caps.magic_defense,
            item.enchant_caps.accuracy,
            item.enchant_caps.critical,
            item.enchant_caps.evasion,
            item.enchant_caps.agility,
        ];
        for cap in caps {
            assert!(cap >= 0, "{}: エンチャント枠 {cap}", item.name);
        }
    }
}

#[test]
fn 韓国コミュニティ資料で照合したセイクリッドブレードを収録する() {
    let sacred = find_equipment_item("wiki-1a51cc7cf165").unwrap();
    assert_eq!(sacred.name, "†セイクリッドブレード");
    assert_eq!(sacred.values_min.slash, 410);
    assert_eq!(sacred.values_max.slash, 450);
    assert_eq!(sacred.enchant_caps.slash, 300);

    let improved = find_equipment_item("wiki-6a2669c83e79").unwrap();
    assert_eq!(improved.name, "†改・セイクリッドブレード");
    assert_eq!(improved.values_min.slash, 480);
    assert_eq!(improved.values_max.slash, 530);
    assert_eq!(improved.enchant_caps.slash, 310);
    // 固定枠なので実物が最大値より10低い520でも、310を全部付与できて830になる。
    assert_eq!(520 + improved.enchant_caps.slash, 830);
}

/// 装着時効果は wiki ステータス のカテゴリ表どおりの効き先に入る。
/// **装備補正値(基本能力値)ではない**ので `base_totals` には出ない。
#[test]
fn 装着時効果は与ダメージ式のカテゴリに入る() {
    let expected = [
        ("nibanboshi-katana", DamageCategory::AttackDamageJapan, 3.0),
        ("nibanboshi-tachi", DamageCategory::AttackDamageJapan, 3.0),
        ("lina-clothes", DamageCategory::PhysicalMagicDamageRate, 3.0),
        ("archangel-wing", DamageCategory::AttackDamageLegacy, 25.0),
        ("sigma-wing", DamageCategory::AttackDamageLegacy, 25.0),
        ("gorilla-armcover", DamageCategory::AttackDamageJapan, 5.0),
        ("tanuki-gloves", DamageCategory::AttackDamageJapan, 5.0),
        ("izutsumi-gauntlet", DamageCategory::AttackDamageJapan, 3.0),
        ("rin-gloves", DamageCategory::AttackDamageJapan, 3.0),
        ("beast-cerberus", DamageCategory::AttackDamageSpecial, 3.0),
        (
            "memorial-crest-wind",
            DamageCategory::AttackDamageSpecial,
            3.0,
        ),
        // 「一定確率で」も発動前提で入れる(ユーザー確定 2026-08-27)
        ("slayers-drag-slave", DamageCategory::AttackDamageJapan, 3.0),
        ("rinrin-tidal-wave", DamageCategory::AttackDamageJapan, 3.0),
        ("logh-lost", DamageCategory::AttackDamageJapan, 1.0),
    ];
    for (id, category, percent) in expected {
        let item = find_equipment_item(id).unwrap_or_else(|| panic!("{id} がカタログに無い"));
        assert_eq!(
            item.damage_effects,
            &[SkillEffect::Damage { category, percent }],
            "{id}"
        );
    }
    let with_effects = equipment_catalog()
        .iter()
        .filter(|i| !i.damage_effects.is_empty())
        .count();
    assert_eq!(with_effects, 221);
}

/// 装備中のアイテムだけが寄与する。カテゴリ側の上限は `CategoryTotals` が掛けるので、
/// ここでは Σ% の小数表現がそのまま出る。
#[test]
fn item_damage_contributionsは装備中のアイテムだけを見る() {
    let mut equipment = Equipment::default();
    assert!(item_damage_contributions(&equipment, SkillDependency::Hack).is_empty());

    equipment.parts.hand.item_id = Some("gorilla-armcover".to_string());
    equipment.parts.body.item_id = Some("archangel-wing".to_string());
    equipment.parts.effect.item_id = Some("beast-unicorn".to_string());
    // カタログに無い id は無視する(保存時に storage が弾いている)
    equipment.parts.helm.item_id = Some("unknown".to_string());

    let mut got = pairs(&item_damage_contributions(
        &equipment,
        SkillDependency::Hack,
    ));
    got.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    assert_eq!(
        got,
        vec![
            (DamageCategory::AttackDamageSpecial, 0.03),
            (DamageCategory::AttackDamageJapan, 0.05),
            (DamageCategory::AttackDamageLegacy, 0.25),
        ]
    );
}

#[test]
fn afの依存別効果は一致するスキルだけに入る() {
    let mut equipment = Equipment::default();
    equipment.parts.artifact.item_id = Some("eclipse-hack-def".to_string());
    assert_eq!(
        pairs(&item_damage_contributions(
            &equipment,
            SkillDependency::Hack
        )),
        vec![(DamageCategory::DependencyDamageRate, 0.30)]
    );
    assert!(item_damage_contributions(&equipment, SkillDependency::HackInt).is_empty());
}

#[test]
fn afは6依存すべてにディフェンシオ候補がある() {
    use SkillDependency::*;
    for dependency in [Stab, Hack, StabHack, Int, Mr, HackInt] {
        assert!(
            equipment_catalog().iter().any(|item| {
                item.slot == PartSlot::Artifact
                    && item.name.contains("ディフェンシオ")
                    && item.recommended_dependency == Some(dependency)
            }),
            "{dependency:?}"
        );
    }
}

#[test]
fn afの主要3段は各6依存の通常版とディフェンシオを持つ() {
    for prefix in ["psyche", "eclipse", "ethereal"] {
        for suffix in ["stab", "hack", "physical", "int", "mr", "hack-int"] {
            for id in [
                format!("{prefix}-{suffix}"),
                format!("{prefix}-{suffix}-def"),
            ] {
                assert!(find_equipment_item(&id).is_some(), "{id}");
            }
        }
    }
}

#[test]
fn エクリプス魔斬ディフェンシオは魔斬スキルへ与ダメ30パーセント() {
    let mut equipment = Equipment::default();
    equipment.parts.artifact.item_id = Some("eclipse-hack-int-def".to_string());
    assert_eq!(
        pairs(&item_damage_contributions(
            &equipment,
            SkillDependency::HackInt
        )),
        vec![(DamageCategory::DependencyDamageRate, 0.30)]
    );
    assert!(item_damage_contributions(&equipment, SkillDependency::Int).is_empty());
}

#[test]
fn afの耐久効果は攻撃効果と分離して主要3段へ入る() {
    assert_eq!(
        find_equipment_item("eclipse-hack-int")
            .unwrap()
            .survival_effects,
        SURVIVAL_MITIGATION_10
    );
    assert_eq!(
        find_equipment_item("eclipse-hack-int-def")
            .unwrap()
            .survival_effects,
        SURVIVAL_DEFENSE_RATE_30
    );
    assert_eq!(
        find_equipment_item("ethereal-hack-int")
            .unwrap()
            .survival_effects,
        SURVIVAL_MITIGATION_15
    );
    assert_eq!(
        find_equipment_item("ethereal-hack-int-def")
            .unwrap()
            .survival_effects,
        SURVIVAL_MITIGATION_40
    );

    let mut equipment = Equipment::default();
    equipment.parts.artifact.item_id = Some("ethereal-hack-int-def".to_string());
    assert_eq!(
        pairs(&item_damage_contributions(
            &equipment,
            SkillDependency::HackInt
        )),
        vec![(DamageCategory::DependencyDamageRate, 0.35)],
        "緩和40%を自分の与ダメージ式へ混ぜない"
    );
}

#[test]
fn 神鳥とルナリアレリックは20段階あり直前段階の完成値から成長する() {
    let catalog = equipment_catalog();
    assert_eq!(
        catalog
            .iter()
            .filter(|item| item.id.starts_with("godbird-pendant-")
                || item.id.starts_with("lunaria-pendant-"))
            .count(),
        20
    );
    assert_eq!(
        catalog
            .iter()
            .filter(|item| item.id.starts_with("godbird-bracelet-")
                || item.id.starts_with("lunaria-bracelet-"))
            .count(),
        20
    );

    let pendant = find_equipment_item("godbird-pendant-plus2").unwrap();
    let bracelet = find_equipment_item("godbird-bracelet-plus2").unwrap();
    assert_eq!(pendant.values_min, v(30, 30, 0, 30, 0, 25, 25, 0, 0));
    assert_eq!(bracelet.values_min, v(0, 0, 30, 0, 30, 0, 0, 25, 25));
    assert_eq!(
        pendant.growth_caps.unwrap(),
        v(50, 50, 0, 50, 0, 45, 45, 0, 0)
    );
    assert_eq!(
        bracelet.growth_caps.unwrap(),
        v(0, 0, 50, 0, 50, 0, 0, 45, 45)
    );
    assert_eq!(pendant.enchant_caps, EquipmentValues::default());
    assert_eq!(bracelet.enchant_caps, EquipmentValues::default());
    assert_eq!(pendant.ability_slots, 0);
    assert_eq!(pendant.random_option_slots, None);

    let lunaria = find_equipment_item("lunaria-pendant-plus10").unwrap();
    assert_eq!(lunaria.values_min, v(190, 190, 0, 190, 0, 190, 190, 0, 0));
    assert_eq!(
        lunaria.growth_caps.unwrap(),
        v(200, 200, 0, 200, 0, 200, 200, 0, 0)
    );
    assert_eq!(lunaria.ability_slots, 1);
    assert_eq!(lunaria.random_option_slots, Some(2));
}

#[test]
fn 武器以外はweapon_classを持たない() {
    for item in equipment_catalog() {
        if item.slot == PartSlot::Weapon {
            assert!(
                item.weapon_class.is_some(),
                "{} は武器なのに weapon_class が無い",
                item.id
            );
        } else {
            assert!(
                item.weapon_class.is_none(),
                "{} は武器以外なのに weapon_class を持つ",
                item.id
            );
        }
    }
}

#[test]
fn 全装備のレンジと上限は値域内で鎧は強化種別を持つ() {
    for item in equipment_catalog() {
        for ((label, min), (_, max)) in item
            .values_min
            .fields()
            .into_iter()
            .zip(item.values_max.fields())
        {
            assert!(
                (0..=max).contains(&min),
                "{} の {} レンジが逆",
                item.name,
                label
            );
            assert!(
                max <= domain::EQUIPMENT_VALUE_MAX,
                "{} の {} が値域外",
                item.name,
                label
            );
        }
        for (label, cap) in item.enchant_caps.fields() {
            assert!(
                (0..=domain::EQUIPMENT_VALUE_MAX).contains(&cap),
                "{} の {} 上限が値域外",
                item.name,
                label
            );
        }
        if item.slot == PartSlot::Armor {
            assert!(
                item.enhance_type.is_some(),
                "{} は鎧なのに強化種別が無い",
                item.name
            );
        }
    }
}

#[test]
fn 全31武器種とコラボ装備を収録する() {
    let catalog = equipment_catalog();
    assert_eq!(
        catalog
            .iter()
            .filter_map(|item| item.weapon_class)
            .collect::<HashSet<_>>()
            .len(),
        31
    );
    assert!(catalog
        .iter()
        .any(|item| item.name == "†エクリプスシミター"));
    assert!(catalog.iter().any(|item| item.name == "†ニバンボシ(刀)"));
    assert!(catalog
        .iter()
        .any(|item| item.weapon_class == Some(WeaponClass::SwordShape)));
}

#[test]
fn 盾プラスは初期1で成長上限200かつエンチャント不可() {
    let items: Vec<_> = equipment_catalog()
        .into_iter()
        .filter(|i| i.slot == PartSlot::ShieldPlus)
        .collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "†ライジングホリックカフス");
    assert_eq!(items[0].values_max, v(1, 1, 1, 1, 1, 1, 1, 1, 1));
    assert_eq!(items[0].growth_cap, Some(200));
    assert_eq!(items[0].enchant_caps, EquipmentValues::default());
}

#[test]
fn find_equipment_itemはidで引ける() {
    assert!(find_equipment_item("abyss-scimitar").is_some());
    assert!(find_equipment_item("nope").is_none());
}

#[test]
fn 系統ごとの強化補正式() {
    let r = enhance_rates(WeaponClass::Katana);
    assert_eq!((r.thrust, r.slash), (1.00, 6.67));
    let r = enhance_rates(WeaponClass::Tachi);
    assert_eq!((r.thrust, r.slash), (4.55, 4.55));
    let r = enhance_rates(WeaponClass::Rapier);
    assert_eq!((r.thrust, r.slash), (6.67, 1.00));
    let r = enhance_rates(WeaponClass::MagicWand);
    assert_eq!((r.magic_attack, r.magic_defense), (6.95, 1.05));
    let r = enhance_rates(WeaponClass::GreatSword);
    assert_eq!((r.slash, r.magic_attack), (3.85, 4.55));
    let r = enhance_rates(WeaponClass::HolyStaff);
    assert_eq!((r.magic_attack, r.magic_defense), (0.70, 7.70));
}

#[test]
fn 強化倍率は1から11がsome_それ以外はnone() {
    assert_eq!(enhance_multiplier(1), Some(0.4));
    assert_eq!(enhance_multiplier(10), Some(28.8));
    assert_eq!(enhance_multiplier(11), Some(40.0));
    assert_eq!(enhance_multiplier(0), None);
    assert_eq!(enhance_multiplier(12), None);
}

#[test]
fn 強化倍率レンジは12から15がsome_それ以外はnone() {
    assert_eq!(enhance_multiplier_range(12), Some((140.0, 280.0)));
    assert_eq!(enhance_multiplier_range(15), Some((680.0, 880.0)));
    assert_eq!(enhance_multiplier_range(11), None);
    assert_eq!(enhance_multiplier_range(16), None);
}

#[test]
fn 武器アビリティはカテゴリー1_3_4の56件_idは重複しない() {
    let abilities = equipment_abilities();
    assert_eq!(
        abilities
            .iter()
            .filter(|a| a.slot == PartSlot::Weapon)
            .count(),
        56
    );
    let ids: HashSet<&str> = abilities.iter().map(|a| a.id).collect();
    assert_eq!(ids.len(), abilities.len());
}

#[test]
fn 装着アビリティはwikiに表がある全部位を持つ() {
    let slots: HashSet<PartSlot> = equipment_abilities().iter().map(|a| a.slot).collect();
    assert_eq!(
        slots,
        HashSet::from([
            PartSlot::Weapon,
            PartSlot::Armor,
            PartSlot::Helm,
            PartSlot::Shield,
            PartSlot::ShieldPlus,
            PartSlot::Head,
            PartSlot::Hand,
            PartSlot::Leg,
            PartSlot::RelicPendant,
            PartSlot::RelicBracelet,
        ])
    );
    let cuffs: Vec<_> = equipment_abilities()
        .into_iter()
        .filter(|a| a.slot == PartSlot::ShieldPlus)
        .collect();
    assert_eq!(cuffs.len(), 6);
    assert!(cuffs.iter().all(|a| a.value_option.is_some()));
    assert_eq!(PartSlot::ShieldPlus.ability_slots(), 2);
}

/// アビリティアイテム方式(古代精霊〜夜星)は4系統各4件。
#[test]
fn アビリティは4系統各4件で記録値と追加枠情報を持つ() {
    use domain::EquipmentAbilityFamily::*;
    let abilities = equipment_abilities();
    for family in [PointedBlade, SharpBlade, Intelligence, MagicResistance] {
        let members: Vec<_> = abilities
            .iter()
            .filter(|a| {
                a.slot == PartSlot::Weapon
                    && a.category == 4
                    && a.family == family
                    && a.additional_slots > 0
            })
            .collect();
        assert_eq!(members.len(), 4, "{family:?} は 4 件");
        for def in members {
            let nonzero = [
                (def.values.thrust, PointedBlade),
                (def.values.slash, SharpBlade),
                (def.values.magic_attack, Intelligence),
                (def.values.magic_defense, MagicResistance),
            ];
            for (value, owner) in nonzero {
                assert_eq!(
                    value != 0,
                    owner == family,
                    "{} の加算先が系統と食い違う",
                    def.id
                );
            }
            assert!(
                def.damage_effects.is_empty(),
                "追加アビリティは自動適用しない"
            );
            assert_eq!(def.additional_slots, 2);
            assert!(!def.additional_effects.is_empty());
            assert_eq!(def.additional_options.len(), 6);
            assert!(!def.record_only, "基本アビリティ値は計算へ反映する");
        }
    }
}

/// 装備システム/アビリティ カテゴリー4(N-/R-/L-/E- 系)の取り込み件数を wiki の表と
/// 突き合わせて固定する。ランダム追加枠を持たないことが、アビリティアイテム方式との区別。
#[test]
fn 装備システムuiのカテゴリー4は部位ごとにwikiの表どおり() {
    let abilities = equipment_abilities();
    let ui: Vec<_> = abilities
        .iter()
        .filter(|a| {
            a.category == 4
                && a.name
                    .split('-')
                    .next()
                    .is_some_and(|head| matches!(head, "N" | "R" | "L" | "E" | "G"))
        })
        .collect();
    let count = |slot: PartSlot| ui.iter().filter(|a| a.slot == slot).count();
    // 兜3(R/L/E) 鎧20(5系統×4) 武器19(4系統×4 + 失われた魂3) 盾8(2系統×4)
    // 頭28(7属性×4) 手8(2系統×4) 足3(R/L/E)
    assert_eq!(count(PartSlot::Helm), 3);
    assert_eq!(count(PartSlot::Armor), 20);
    assert_eq!(count(PartSlot::Weapon), 19);
    assert_eq!(count(PartSlot::Shield), 8);
    assert_eq!(count(PartSlot::Head), 28);
    assert_eq!(count(PartSlot::Hand), 8);
    assert_eq!(count(PartSlot::Leg), 3);
    assert_eq!(ui.len(), 89);
    // 表で追加効果が決まっているのでランダム追加枠を持たない(頭の G- 月石だけは
    // アビリティアイテム方式でも取れるため、先に収録した 1 枠付きの定義を使う)。
    for def in ui.iter().filter(|a| a.slot != PartSlot::Head) {
        assert_eq!(def.additional_slots, 0, "{}", def.id);
        assert!(!def.effect_summary.is_empty(), "{}", def.id);
    }
}

/// 1 列目の効果は装備補正へ、2 列目のダメージ増加は X3 へ入る。
#[test]
fn e_鎧研磨は物防30_r_尖った刃は突き7とx3の3パーセント() {
    let abilities = equipment_abilities();
    let armor = abilities.iter().find(|a| a.id == "e-armor-polish").unwrap();
    assert_eq!(armor.name, "E-鎧研磨");
    assert_eq!(armor.values.physical_defense, 30);
    assert!(!armor.record_only);

    let weapon = abilities
        .iter()
        .find(|a| a.id == "r-pointed-blade")
        .unwrap();
    assert_eq!(weapon.values, a(7, 0, 0, 0));
    assert_eq!(
        weapon.damage_effects,
        &[SkillEffect::Damage {
            category: DamageCategory::AttackDamageBasicTrigger,
            percent: 3.0,
        }]
    );

    // 最大HP・移動速度・属性は EquipmentValues に無いので記録のみ。
    for id in [
        "n-vitality-armor",
        "n-mana-armor",
        "r-agility-leg",
        "n-fire-moonstone",
    ] {
        let def = abilities.iter().find(|a| a.id == id).unwrap();
        assert!(def.record_only, "{id}");
        assert_eq!(def.values, EquipmentValues::default(), "{id}");
    }
}

/// 新装着アビリティは 古代精霊 / 深淵 / 喪失 / 夜星 の4系列。どの系列も同じ
/// 種類ぞろえで、部位ごとの内訳が揃っていないと「喪失だけ無い」が再発する。
#[test]
fn 新装着アビリティは4系列とも同じ種類ぞろえを持つ() {
    let abilities = equipment_abilities();
    for prefix in ["古代精霊の", "深淵の", "喪失の", "夜星の"] {
        let members: Vec<_> = abilities
            .iter()
            .filter(|a| a.name.starts_with(prefix))
            .collect();
        let count = |slot: PartSlot| members.iter().filter(|a| a.slot == slot).count();
        assert_eq!(count(PartSlot::Weapon), 4, "{prefix}武器");
        assert_eq!(count(PartSlot::Armor), 5, "{prefix}鎧");
        assert_eq!(count(PartSlot::Shield), 2, "{prefix}盾");
        assert_eq!(count(PartSlot::Hand), 2, "{prefix}手");
        assert_eq!(count(PartSlot::Leg), 1, "{prefix}足");
        assert_eq!(members.len(), 14, "{prefix}合計");
        // 追加アビリティ2枠は系列ごとに値域が違う。夜星の値域を使い回していないこと。
        for def in &members {
            assert_eq!(def.additional_slots, 2, "{}", def.id);
            assert!(!def.additional_options.is_empty(), "{}", def.id);
        }
    }
    // 喪失の鎧まわりは wiki の表どおり(ユーザーがゲーム内画面で確認した並び)。
    let loss = |id: &str| abilities.iter().find(|a| a.id == id).unwrap();
    assert_eq!(loss("loss-armor-polish").values.physical_defense, 45);
    assert_eq!(loss("loss-magic-resistance-armor").values.magic_defense, 45);
    assert_eq!(loss("loss-evasion-armor").values.evasion, 13);
    assert_eq!(loss("loss-shield-polish").values.physical_defense, 22);
    assert_eq!(loss("loss-critical-hand").values.critical, 12);
    // 「命中 +13」は単純な装備命中率補正(wiki 装備システム/アビリティ)。的中剣は
    // キャラスキル「極・的中剣」であって装着アビリティではない(2026-09-01 訂正)
    assert_eq!(loss("loss-accuracy-hand").values.accuracy, 13);
    // 最大HP は装備補正 9 値に無いので記録のみ。
    let vitality = loss("loss-vitality-armor");
    assert!(vitality.record_only);
    assert_eq!(vitality.effect_summary, "最大HP +12,000");
}

#[test]
fn 夜星の尖った刃は突き20() {
    let abilities = equipment_abilities();
    let def = abilities
        .iter()
        .find(|a| a.id == "night-star-pointed-blade")
        .unwrap();
    assert_eq!(def.name, "夜星の尖った刃");
    assert_eq!(def.values, a(20, 0, 0, 0));
    use domain::EquipmentAbilityAdditionalKind::*;
    assert_eq!(
        def.additional_options
            .iter()
            .find(|o| o.kind == FixedDamage)
            .map(|o| (o.min, o.max)),
        Some((10_000, 10_000))
    );
    assert_eq!(
        def.additional_options
            .iter()
            .find(|o| o.kind == DamageRate)
            .map(|o| (o.min, o.max)),
        Some((11, 11))
    );
    assert_eq!(
        def.additional_options
            .iter()
            .find(|o| o.kind == Thrust)
            .map(|o| (o.min, o.max)),
        Some((9, 18))
    );
    assert_eq!(
        def.additional_options
            .iter()
            .find(|o| o.kind == Accuracy)
            .map(|o| (o.min, o.max)),
        Some((10, 16))
    );
}

#[test]
fn ユーザー例の3枠はカテゴリーが異なる() {
    let abilities = equipment_abilities();
    let picked = ["night-star-sharp-blade", "lower-grade-slash", "storm-blade"]
        .map(|id| abilities.iter().find(|a| a.id == id).unwrap());
    assert_eq!(picked.map(|a| a.category), [4, 1, 3]);
    assert_eq!(picked[0].values.slash, 20);
    assert_eq!(picked[1].values.slash, 12);
    assert_eq!(picked[2].effect_summary, "武器ディレイ -7%");
}

#[test]
fn 古代精霊の耐魔力は魔防11() {
    let abilities = equipment_abilities();
    let def = abilities
        .iter()
        .find(|a| a.id == "ancient-magic-resistance")
        .unwrap();
    assert_eq!(def.name, "古代精霊の耐魔力");
    assert_eq!(def.values, a(0, 0, 0, 11));
}

#[test]
fn 強化等級はwiki確率区分の上端を四捨五入する() {
    use domain::EnhanceGrade::*;
    assert_eq!(
        [Lowest, Low, Middle, High, Highest]
            .map(|grade| enhance_grade_multiplier(15, grade).unwrap()),
        [700.0, 740.0, 820.0, 870.0, 880.0]
    );
    assert_eq!(
        [Lowest, Low, Middle, High, Highest].map(|grade| armor_enhance_multiplier(
            15,
            Some(grade)
        )
        .unwrap()),
        [350.0, 370.0, 410.0, 435.0, 440.0]
    );
}
