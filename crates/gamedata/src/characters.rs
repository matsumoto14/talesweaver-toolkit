//! ゲーム内キャラクター(操作キャラ)と、スキル依存種別ごとのステ由来攻撃力係数。

use domain::{
    AccuracyCorrection, AttackCoefficients, EquipmentCoefficients, EquipmentRates, SkillDependency,
    StatKind, WristBonusRule,
};
use serde::Serialize;

use crate::{
    equipment_catalog::{ArmorClass, WeaponClass, WristType},
    Source,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameCharacter {
    pub id: &'static str,
    pub name: &'static str,
    /// Tale Wiki「Item/武器」武器一覧表の装備可能キャラ列(取得 2026-09-01)。
    pub weapon_classes: &'static [WeaponClass],
    /// Tale Wiki の各防具カテゴリに記載された装備可能種。
    pub armor_classes: &'static [ArmorClass],
    /// Tale Wiki の各サブアームカテゴリに記載された装備可能種。
    pub wrist_types: &'static [WristType],
    /// キャラ固有パッシブによる、腕装備補正の基本能力値への派生ルール。無いキャラは `None`
    /// (出典: Item/防具/腕 各キャラページの「特殊効果」欄)。
    pub wrist_bonus: Option<WristBonusRule>,
}

const CHARACTERS: &[GameCharacter] = &[
    GameCharacter {
        id: "lucian",
        name: "ルシアン",
        weapon_classes: &[WeaponClass::Rapier, WeaponClass::LongSword, WeaponClass::Katana],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy],
        wrist_types: &[WristType::Shield],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "boris",
        name: "ボリス",
        weapon_classes: &[WeaponClass::Katana, WeaponClass::GreatSword, WeaponClass::Tachi],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Magic],
        wrist_types: &[WristType::Knuckle],
        wrist_bonus: Some(WristBonusRule::ThrustToMagicAttack),
    },
    GameCharacter {
        id: "ispin",
        name: "イスピン",
        weapon_classes: &[WeaponClass::Rapier, WeaponClass::LongSword, WeaponClass::Katana],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy],
        wrist_types: &[WristType::Shield],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "maximin",
        name: "マキシミン",
        weapon_classes: &[WeaponClass::Katana, WeaponClass::GreatSword, WeaponClass::Tachi],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Magic],
        wrist_types: &[WristType::Shield, WristType::Knuckle],
        wrist_bonus: Some(WristBonusRule::ThrustToMagicAttack),
    },
    GameCharacter {
        id: "tichiel",
        name: "ティチエル",
        weapon_classes: &[WeaponClass::MagicWand, WeaponClass::HolyStaff, WeaponClass::WarStaff],
        armor_classes: &[ArmorClass::Light, ArmorClass::Robe],
        wrist_types: &[WristType::Bracelet],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "nayatorei",
        name: "ナヤトレイ",
        weapon_classes: &[WeaponClass::Dagger, WeaponClass::ShortSword, WeaponClass::Axe],
        armor_classes: &[ArmorClass::Light, ArmorClass::Suit],
        wrist_types: &[WristType::Band],
        wrist_bonus: Some(WristBonusRule::BandAgilityByDependency),
    },
    GameCharacter {
        id: "siberin",
        name: "シベリン",
        weapon_classes: &[WeaponClass::Spear, WeaponClass::Rod],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy],
        wrist_types: &[WristType::Knuckle],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "mira",
        name: "ミラ",
        weapon_classes: &[WeaponClass::Whip, WeaponClass::Nunchaku],
        armor_classes: &[ArmorClass::Light, ArmorClass::Suit],
        wrist_types: &[WristType::Band],
        wrist_bonus: Some(WristBonusRule::BandAgilityToSlash),
    },
    GameCharacter {
        id: "joshua",
        name: "ジョシュア",
        weapon_classes: &[WeaponClass::SmallSword, WeaponClass::Wand],
        armor_classes: &[ArmorClass::Light, ArmorClass::Magic],
        wrist_types: &[WristType::Spellbook, WristType::CrystalBall],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "chloe",
        name: "クロエ",
        weapon_classes: &[WeaponClass::MagicWand],
        armor_classes: &[ArmorClass::Light, ArmorClass::Robe],
        wrist_types: &[WristType::Bracelet],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "ranjie",
        name: "ランジエ",
        weapon_classes: &[WeaponClass::PhysicalGun, WeaponClass::MagicGun],
        armor_classes: &[ArmorClass::Light, ArmorClass::Magic],
        wrist_types: &[WristType::PhysicalMagazine, WristType::MagicMagazine],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "isaac",
        name: "イサック",
        weapon_classes: &[WeaponClass::Claw, WeaponClass::Kara],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Suit],
        wrist_types: &[WristType::Knuckle, WristType::Band],
        wrist_bonus: Some(WristBonusRule::BandAgilityByDependency),
    },
    GameCharacter {
        id: "anais",
        name: "アナイス",
        weapon_classes: &[WeaponClass::Scepter, WeaponClass::Handbell],
        armor_classes: &[ArmorClass::Light, ArmorClass::Robe],
        wrist_types: &[WristType::Bracelet],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "isolet",
        name: "イソレット",
        weapon_classes: &[WeaponClass::DualBladePhysical, WeaponClass::DualBladeMagic],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Magic],
        wrist_types: &[WristType::DualBladePhysical, WristType::DualBladeMagic],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "benya",
        name: "ベンヤ",
        weapon_classes: &[WeaponClass::Scythe, WeaponClass::Hammer],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Suit],
        wrist_types: &[WristType::Knuckle, WristType::Band, WristType::CrystalBall],
        wrist_bonus: Some(WristBonusRule::BandAgilityByStatComparison),
    },
    GameCharacter {
        id: "roamini",
        name: "ロアミニ",
        weapon_classes: &[WeaponClass::Totem],
        armor_classes: &[ArmorClass::Light, ArmorClass::Suit, ArmorClass::Robe],
        wrist_types: &[WristType::Band, WristType::Bracelet],
        wrist_bonus: Some(WristBonusRule::BandAgilityToMagicAttack),
    },
    GameCharacter {
        id: "nocturne",
        name: "ノクターン",
        weapon_classes: &[WeaponClass::HandLauncher],
        armor_classes: &[ArmorClass::Light, ArmorClass::Magic],
        wrist_types: &[WristType::PhysicalMagazine],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "leeche",
        name: "リーチェ",
        weapon_classes: &[WeaponClass::ArmingSword],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Magic],
        wrist_types: &[WristType::Pendulum],
        wrist_bonus: None,
    },
    GameCharacter {
        id: "yefnen",
        name: "イェフネン",
        weapon_classes: &[WeaponClass::SwordShape],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Magic],
        wrist_types: &[WristType::Knuckle],
        wrist_bonus: None,
    },
];

pub fn characters() -> &'static [GameCharacter] {
    CHARACTERS
}

pub fn find_character(id: &str) -> Option<&'static GameCharacter> {
    CHARACTERS.iter().find(|c| c.id == id)
}

/// ステ由来攻撃力係数・装備攻撃力係数の出典。
pub const ATTACK_COEFFICIENTS_SOURCE: Source = Source {
    page: "wiki 計算式まとめ#BaseAttackPower",
    retrieved_on: "2026-08-22",
    note: "旧リポ twtoolkit rawStatCoefficients.json(Excel v4.00 由来)と完全一致を確認済み",
};

/// スキル依存種別ごとのステ由来攻撃力係数(wiki: カテゴリA の内訳)。
///
/// 全キャラ共通(旧リポのデータ構造に同じ)。出典: `ATTACK_COEFFICIENTS_SOURCE`。
pub fn attack_coefficients(dependency: SkillDependency) -> AttackCoefficients {
    use StatKind::*;
    let (primary, secondary) = match dependency {
        SkillDependency::Stab => ((Stab, 2.1), (Hack, 1.08)),
        SkillDependency::Hack => ((Hack, 2.1), (Stab, 1.08)),
        SkillDependency::Int => ((Int, 2.4), (Mr, 0.6)),
        SkillDependency::Mr => ((Mr, 2.55), (Int, 0.45)),
        SkillDependency::StabHack => ((Stab, 1.8), (Hack, 1.8)),
        SkillDependency::HackInt => ((Hack, 1.8), (Int, 1.8)),
    };
    AttackCoefficients { primary, secondary }
}

/// スキル依存種別ごとの命中P補正(wiki 計算式まとめ の依存表「命中P補正(小数点以下切り捨て)」)。
pub fn accuracy_correction(dependency: SkillDependency) -> AccuracyCorrection {
    use SkillDependency::*;
    match dependency {
        // STAB: ボーナス STAB×0.1 / ペナルティ STAB/100
        Stab => AccuracyCorrection {
            bonus: Some((StatKind::Stab, 0.1)),
            penalty_primary: StatKind::Stab,
            penalty_secondary: None,
            penalty_divisor: 100.0,
        },
        // HACK: ボーナス HACK×0.06 / ペナルティ HACK/100
        Hack => AccuracyCorrection {
            bonus: Some((StatKind::Hack, 0.06)),
            penalty_primary: StatKind::Hack,
            penalty_secondary: None,
            penalty_divisor: 100.0,
        },
        // STAB+HACK: ボーナスなし / (STAB+HACK)/200
        StabHack => AccuracyCorrection {
            bonus: None,
            penalty_primary: StatKind::Stab,
            penalty_secondary: Some(StatKind::Hack),
            penalty_divisor: 200.0,
        },
        // INT+HACK: ボーナスなし / (INT+HACK)/250
        HackInt => AccuracyCorrection {
            bonus: None,
            penalty_primary: StatKind::Int,
            penalty_secondary: Some(StatKind::Hack),
            penalty_divisor: 250.0,
        },
        // INT: ボーナスなし / INT/100
        Int => AccuracyCorrection {
            bonus: None,
            penalty_primary: StatKind::Int,
            penalty_secondary: None,
            penalty_divisor: 100.0,
        },
        // MR: ボーナスなし / MR/100
        Mr => AccuracyCorrection {
            bonus: None,
            penalty_primary: StatKind::Mr,
            penalty_secondary: None,
            penalty_divisor: 100.0,
        },
    }
}

/// スキル依存種別ごとの装備攻撃力係数(wiki: カテゴリA の内訳「装備攻撃力」)。
/// 基本能力値/強化能力値で係数が異なる。出典: `ATTACK_COEFFICIENTS_SOURCE`。
pub fn equipment_coefficients(dependency: SkillDependency) -> EquipmentCoefficients {
    use SkillDependency::*;
    let (base, enhanced) = match dependency {
        Stab => (
            EquipmentRates {
                thrust: 23.75,
                slash: 3.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
            EquipmentRates {
                thrust: 32.5,
                slash: 18.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
        ),
        Hack => (
            EquipmentRates {
                thrust: 3.75,
                slash: 23.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
            EquipmentRates {
                thrust: 18.75,
                slash: 32.5,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
        ),
        StabHack => (
            EquipmentRates {
                thrust: 14.5,
                slash: 14.5,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
            EquipmentRates {
                thrust: 28.75,
                slash: 28.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
        ),
        HackInt => (
            EquipmentRates {
                thrust: 0.0,
                slash: 14.5,
                magic_attack: 14.5,
                magic_defense: 0.0,
            },
            EquipmentRates {
                thrust: 0.0,
                slash: 28.75,
                magic_attack: 28.75,
                magic_defense: 0.0,
            },
        ),
        Int => (
            EquipmentRates {
                thrust: 0.0,
                slash: 0.0,
                magic_attack: 23.75,
                magic_defense: 2.5,
            },
            EquipmentRates {
                thrust: 0.0,
                slash: 0.0,
                magic_attack: 32.5,
                magic_defense: 18.25,
            },
        ),
        Mr => (
            EquipmentRates {
                thrust: 0.0,
                slash: 0.0,
                magic_attack: 2.5,
                magic_defense: 20.5,
            },
            // wiki 注記: 韓国情報の 16.75 と異なるが、この数値(19.25)で適用と明記されている。
            EquipmentRates {
                thrust: 0.0,
                slash: 0.0,
                magic_attack: 19.25,
                magic_defense: 32.5,
            },
        ),
    };
    EquipmentCoefficients { base, enhanced }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{stat_attack_power, EffectiveStats};

    #[test]
    fn プレイアブルキャラは19名登録されている() {
        assert_eq!(characters().len(), 19);
        assert_eq!(find_character("boris").unwrap().name, "ボリス");
        assert_eq!(find_character("benya").unwrap().name, "ベンヤ");
        assert_eq!(find_character("roamini").unwrap().name, "ロアミニ");
        assert!(find_character("nope").is_none());
    }

    #[test]
    fn キャラidは重複しない() {
        let mut ids: Vec<_> = characters().iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), characters().len());
    }

    #[test]
    fn 鎧と腕の装備可能種はwikiのカテゴリ表どおり() {
        let lucian = find_character("lucian").unwrap();
        assert_eq!(
            lucian.armor_classes,
            &[ArmorClass::Light, ArmorClass::Heavy]
        );
        assert_eq!(lucian.wrist_types, &[WristType::Shield]);

        let roamini = find_character("roamini").unwrap();
        assert_eq!(
            roamini.armor_classes,
            &[ArmorClass::Light, ArmorClass::Suit, ArmorClass::Robe]
        );
        assert_eq!(roamini.wrist_types, &[WristType::Band, WristType::Bracelet]);

        let isolet = find_character("isolet").unwrap();
        assert_eq!(
            isolet.wrist_types,
            &[WristType::DualBladePhysical, WristType::DualBladeMagic]
        );
    }

    #[test]
    fn 武器の装備可能種はwikiの武器一覧表どおり() {
        assert!(characters().iter().all(|c| !c.weapon_classes.is_empty()));
        assert_eq!(
            find_character("isolet").unwrap().weapon_classes,
            &[WeaponClass::DualBladePhysical, WeaponClass::DualBladeMagic]
        );
        assert_eq!(
            find_character("anais").unwrap().weapon_classes,
            &[WeaponClass::Scepter, WeaponClass::Handbell]
        );
        assert_eq!(
            find_character("lucian").unwrap().weapon_classes,
            &[WeaponClass::Rapier, WeaponClass::LongSword, WeaponClass::Katana]
        );
        // 戦杖はティチエル専用(クロエは魔杖のみ)。
        assert_eq!(
            find_character("chloe").unwrap().weapon_classes,
            &[WeaponClass::MagicWand]
        );
    }

    #[test]
    fn 依存種別ごとの係数() {
        let stats = EffectiveStats {
            stab: 100,
            hack: 200,
            int: 300,
            mr: 400,
            ..Default::default()
        };
        let power = |d| stat_attack_power(&stats, &attack_coefficients(d));
        // 1.08×HACK + 2.1×STAB = 216 + 210
        assert!((power(SkillDependency::Stab) - 426.0).abs() < 1e-9);
        // 1.08×STAB + 2.1×HACK = 108 + 420
        assert!((power(SkillDependency::Hack) - 528.0).abs() < 1e-9);
        // 2.4×INT + 0.6×MR = 720 + 240
        assert!((power(SkillDependency::Int) - 960.0).abs() < 1e-9);
        // 0.45×INT + 2.55×MR = 135 + 1020
        assert!((power(SkillDependency::Mr) - 1155.0).abs() < 1e-9);
        // 1.8×(STAB+HACK) = 540
        assert!((power(SkillDependency::StabHack) - 540.0).abs() < 1e-9);
        // 1.8×(HACK+INT) = 900
        assert!((power(SkillDependency::HackInt) - 900.0).abs() < 1e-9);
    }

    #[test]
    fn 依存種別ごとの装備係数() {
        use SkillDependency::*;
        let c = equipment_coefficients(Stab);
        assert_eq!((c.base.thrust, c.base.slash), (23.75, 3.75));
        assert_eq!((c.enhanced.thrust, c.enhanced.slash), (32.5, 18.75));

        let c = equipment_coefficients(Hack);
        assert_eq!((c.base.thrust, c.base.slash), (3.75, 23.75));
        assert_eq!((c.enhanced.thrust, c.enhanced.slash), (18.75, 32.5));

        let c = equipment_coefficients(StabHack);
        assert_eq!((c.base.thrust, c.base.slash), (14.5, 14.5));
        assert_eq!((c.enhanced.thrust, c.enhanced.slash), (28.75, 28.75));

        let c = equipment_coefficients(HackInt);
        assert_eq!((c.base.slash, c.base.magic_attack), (14.5, 14.5));
        assert_eq!((c.enhanced.slash, c.enhanced.magic_attack), (28.75, 28.75));

        let c = equipment_coefficients(Int);
        assert_eq!((c.base.magic_attack, c.base.magic_defense), (23.75, 2.5));
        assert_eq!(
            (c.enhanced.magic_attack, c.enhanced.magic_defense),
            (32.5, 18.25)
        );

        let c = equipment_coefficients(Mr);
        assert_eq!((c.base.magic_attack, c.base.magic_defense), (2.5, 20.5));
        assert_eq!(
            (c.enhanced.magic_attack, c.enhanced.magic_defense),
            (19.25, 32.5)
        );
    }
}
