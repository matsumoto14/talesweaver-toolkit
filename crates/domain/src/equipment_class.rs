//! 装備の分類(武器種・武器系統・鎧種・サブアーム種)と、キャラ・主軸スキルから見た装備候補の適合。
//!
//! 分類そのものは wiki「装備システム/装備強化」の系統表・各防具カテゴリの装備可能種で、
//! ゲームのドメイン語彙。カタログ(gamedata)はこの分類を各アイテムに付けるだけで、
//! 「このキャラ・このスキルで使えるか」の判定はここが唯一の正。

use serde::{Deserialize, Serialize};

use crate::defense::AttackType;
use crate::equipment::{EquipmentAbilityFamily, EquipmentEnhanceType, PartSlot};
use crate::skill::{Skill, SkillDependency};

/// 武器種(wiki: 装備システム/装備強化「系統」表の該当武器)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponClass {
    // STAB系
    Rapier,
    Dagger,
    Spear,
    SmallSword,
    PhysicalGun,
    Claw,
    HandLauncher,
    // STAB+HACK系
    LongSword,
    Tachi,
    WarStaff,
    ShortSword,
    Rod,
    Nunchaku,
    // HACK系
    Katana,
    Axe,
    Whip,
    Kara,
    DualBladePhysical,
    Scythe,
    ArmingSword,
    SwordShape,
    // INT系
    MagicWand,
    Wand,
    MagicGun,
    Scepter,
    Totem,
    // INT+HACK系
    GreatSword,
    // MR系
    HolyStaff,
    Handbell,
    DualBladeMagic,
    Hammer,
}

impl WeaponClass {
    /// 武器種が属する系統(wiki: 装備システム/装備強化「系統」列)。
    pub fn system(self) -> WeaponSystem {
        use WeaponClass::*;
        use WeaponSystem::*;
        match self {
            Rapier | Dagger | Spear | SmallSword | PhysicalGun | Claw | HandLauncher => Stab,
            LongSword | Tachi | WarStaff | ShortSword | Rod | Nunchaku => StabHack,
            Katana | Axe | Whip | Kara | DualBladePhysical | Scythe | ArmingSword | SwordShape => {
                Hack
            }
            MagicWand | Wand | MagicGun | Scepter | Totem => Int,
            GreatSword => IntHack,
            HolyStaff | Handbell | DualBladeMagic | Hammer => Mr,
        }
    }
}

/// 武器系統(wiki: 装備システム/装備強化「系統」列)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponSystem {
    Stab,
    StabHack,
    Hack,
    Int,
    IntHack,
    Mr,
}

impl WeaponSystem {
    pub const ALL: [WeaponSystem; 6] = [
        WeaponSystem::Stab,
        WeaponSystem::StabHack,
        WeaponSystem::Hack,
        WeaponSystem::Int,
        WeaponSystem::IntHack,
        WeaponSystem::Mr,
    ];

    /// この依存種別のスキルで実用になる武器系統。複合系統の武器は片側の依存でも使える。
    pub fn for_dependency(dependency: SkillDependency) -> &'static [WeaponSystem] {
        use WeaponSystem::*;
        match dependency {
            SkillDependency::Stab => &[Stab, StabHack],
            SkillDependency::Hack => &[Hack, StabHack, IntHack],
            SkillDependency::Int => &[Int, IntHack],
            SkillDependency::Mr => &[Mr],
            SkillDependency::StabHack => &[StabHack],
            SkillDependency::HackInt => &[IntHack],
        }
    }

    /// 装備強化の補正式(`EquipmentEnhanceType`)から武器系統を引く。武器以外の種別は `None`。
    pub fn from_enhance_type(kind: EquipmentEnhanceType) -> Option<WeaponSystem> {
        Some(match kind {
            EquipmentEnhanceType::WeaponStab => WeaponSystem::Stab,
            EquipmentEnhanceType::WeaponStabHack => WeaponSystem::StabHack,
            EquipmentEnhanceType::WeaponHack => WeaponSystem::Hack,
            EquipmentEnhanceType::WeaponInt => WeaponSystem::Int,
            EquipmentEnhanceType::WeaponIntHack => WeaponSystem::IntHack,
            EquipmentEnhanceType::WeaponMr => WeaponSystem::Mr,
            _ => return None,
        })
    }

    /// 系統と同じ係数を持つスキル依存種別。武器の系統から「この武器で伸ばす補正」を引くのに使う。
    pub fn dependency(self) -> SkillDependency {
        match self {
            WeaponSystem::Stab => SkillDependency::Stab,
            WeaponSystem::StabHack => SkillDependency::StabHack,
            WeaponSystem::Hack => SkillDependency::Hack,
            WeaponSystem::Int => SkillDependency::Int,
            WeaponSystem::IntHack => SkillDependency::HackInt,
            WeaponSystem::Mr => SkillDependency::Mr,
        }
    }

    /// 系統に対応する装備強化の補正式。
    pub fn enhance_type(self) -> EquipmentEnhanceType {
        match self {
            WeaponSystem::Stab => EquipmentEnhanceType::WeaponStab,
            WeaponSystem::StabHack => EquipmentEnhanceType::WeaponStabHack,
            WeaponSystem::Hack => EquipmentEnhanceType::WeaponHack,
            WeaponSystem::Int => EquipmentEnhanceType::WeaponInt,
            WeaponSystem::IntHack => EquipmentEnhanceType::WeaponIntHack,
            WeaponSystem::Mr => EquipmentEnhanceType::WeaponMr,
        }
    }

    /// この系統の武器に装着できるアビリティ系統か。
    /// 武器ディレイと失われた魂(最大HP)は攻撃系統に紐づかないので、どの武器でも選べる。
    pub fn accepts_ability(self, family: EquipmentAbilityFamily) -> bool {
        use EquipmentAbilityFamily::*;
        match family {
            WeaponDelay | Vitality => true,
            _ => match self {
                WeaponSystem::Stab => family == PointedBlade,
                WeaponSystem::Hack => family == SharpBlade,
                WeaponSystem::StabHack => matches!(family, PointedBlade | SharpBlade),
                WeaponSystem::Int => family == Intelligence,
                WeaponSystem::IntHack => matches!(family, SharpBlade | Intelligence),
                WeaponSystem::Mr => family == MagicResistance,
            },
        }
    }
}

/// 鎧の区分(wiki: 装備システム/防具区分)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorClass {
    Light,
    Heavy,
    Magic,
    Suit,
    Robe,
}

/// 腕装備(サブアーム)の区分(wiki: `Item/防具/腕/*`)。
/// キャラパッシブは「盾部位全般」と「バンドだけ」を区別するため、表示名ではなく
/// カタログの分類として持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WristType {
    Shield,
    Spellbook,
    Knuckle,
    Band,
    Bracelet,
    Pendulum,
    CrystalBall,
    DualBladePhysical,
    PhysicalMagazine,
    MagicMagazine,
    DualBladeMagic,
}

impl WristType {
    /// 物理・魔法のどちらの攻撃向けに作られた専用サブアームか。
    /// 共用(盾・ナックル・バンド等)は `None`。
    pub fn attack_type(self) -> Option<AttackType> {
        match self {
            WristType::CrystalBall
            | WristType::DualBladePhysical
            | WristType::PhysicalMagazine => Some(AttackType::Physical),
            WristType::Spellbook | WristType::MagicMagazine | WristType::DualBladeMagic => {
                Some(AttackType::Magic)
            }
            _ => None,
        }
    }

    /// 同じキャラで物理・魔法の専用サブアームが分かれる場合だけ、攻撃タイプで狭める。
    /// 片側しか持たないキャラ(盾のみ等)はそのまま返す。
    pub fn narrow_by_attack_type(types: &[WristType], attack_type: AttackType) -> Vec<WristType> {
        let has = |target: AttackType| types.iter().any(|t| t.attack_type() == Some(target));
        if has(AttackType::Physical) && has(AttackType::Magic) {
            types
                .iter()
                .copied()
                .filter(|t| t.attack_type() == Some(attack_type))
                .collect()
        } else {
            types.to_vec()
        }
    }
}

/// 候補 1 件の適合度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemFit {
    /// 主軸スキル・キャラに合う(既定で見せる候補)
    Recommended,
    /// キャラは装備できるが、主軸スキルの絞り込みには合わない
    Usable,
    /// このキャラは装備できない
    Other,
}

/// 何で絞ったか。文言は画面が組む(UI のもの)ので、ここは構造だけを返す。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FitCriterion {
    /// 主軸スキルの実用武器種(依存能力だけでは絞れないスキル)
    WeaponClasses { classes: Vec<WeaponClass> },
    /// 主軸スキルの依存能力に合う武器系統
    WeaponSystems { systems: Vec<WeaponSystem> },
    /// 主軸スキルの攻撃タイプに合うサブアーム種
    WristTypes { types: Vec<WristType> },
    /// キャラが装備できるか(主軸スキルでは絞れない部位)
    CharacterUsable,
    /// AF の推奨依存
    Dependency { dependency: SkillDependency },
}

/// カタログ 1 件の分類。gamedata の `EquipmentItem` から呼び出し側が詰める
/// (domain は gamedata に依存できない)。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemClassification {
    pub weapon_class: Option<WeaponClass>,
    pub armor_class: Option<ArmorClass>,
    pub wrist_type: Option<WristType>,
    pub recommended_dependency: Option<SkillDependency>,
}

/// キャラが装備できる区分(gamedata の `GameCharacter` から詰める)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterEquipmentClasses<'a> {
    pub weapon_classes: &'a [WeaponClass],
    pub armor_classes: &'a [ArmorClass],
    pub wrist_types: &'a [WristType],
}

/// 部位 × キャラ × 主軸スキルで決まる、候補の絞り込み規則。
#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentFitRule {
    slot: PartSlot,
    weapon_classes: Option<Vec<WeaponClass>>,
    armor_classes: Option<Vec<ArmorClass>>,
    wrist_types: Option<Vec<WristType>>,
    criterion: Option<FitCriterion>,
}

impl EquipmentFitRule {
    pub fn new(
        slot: PartSlot,
        character: Option<CharacterEquipmentClasses<'_>>,
        main_skill: Option<&Skill>,
    ) -> Self {
        let criterion = match slot {
            PartSlot::Weapon => match main_skill {
                Some(skill) if !skill.weapon_classes.is_empty() => Some(FitCriterion::WeaponClasses {
                    classes: skill.weapon_classes.clone(),
                }),
                Some(skill) => Some(FitCriterion::WeaponSystems {
                    systems: WeaponSystem::for_dependency(skill.dependency).to_vec(),
                }),
                None => character.map(|_| FitCriterion::CharacterUsable),
            },
            PartSlot::Armor => character.map(|_| FitCriterion::CharacterUsable),
            PartSlot::Shield => character.map(|c| FitCriterion::WristTypes {
                types: match main_skill {
                    Some(skill) => WristType::narrow_by_attack_type(
                        c.wrist_types,
                        skill.dependency.attack_type(),
                    ),
                    None => c.wrist_types.to_vec(),
                },
            }),
            PartSlot::Artifact => main_skill.map(|skill| FitCriterion::Dependency {
                dependency: skill.dependency,
            }),
            _ => None,
        };
        Self {
            slot,
            weapon_classes: character.map(|c| c.weapon_classes.to_vec()),
            armor_classes: character.map(|c| c.armor_classes.to_vec()),
            wrist_types: character.map(|c| c.wrist_types.to_vec()),
            criterion,
        }
    }

    /// 何で絞ったか。絞っていなければ `None`(画面は絞り込みの帯を出さない)。
    pub fn criterion(&self) -> Option<&FitCriterion> {
        self.criterion.as_ref()
    }

    /// この部位の候補 1 件の適合度。
    pub fn fit(&self, item: &ItemClassification) -> ItemFit {
        let usable = match self.slot {
            PartSlot::Weapon => match &self.weapon_classes {
                Some(classes) => item.weapon_class.is_some_and(|c| classes.contains(&c)),
                None => true,
            },
            PartSlot::Armor => match &self.armor_classes {
                Some(classes) => item.armor_class.is_some_and(|c| classes.contains(&c)),
                None => true,
            },
            PartSlot::Shield => match &self.wrist_types {
                Some(types) => item.wrist_type.is_some_and(|t| types.contains(&t)),
                None => true,
            },
            _ => true,
        };
        if !usable {
            return ItemFit::Other;
        }
        match &self.criterion {
            None => ItemFit::Usable,
            Some(FitCriterion::CharacterUsable) => ItemFit::Recommended,
            Some(FitCriterion::WeaponClasses { classes }) => {
                Self::rank(item.weapon_class.is_some_and(|c| classes.contains(&c)))
            }
            Some(FitCriterion::WeaponSystems { systems }) => Self::rank(
                item.weapon_class
                    .is_some_and(|c| systems.contains(&c.system())),
            ),
            Some(FitCriterion::WristTypes { types }) => {
                Self::rank(item.wrist_type.is_some_and(|t| types.contains(&t)))
            }
            Some(FitCriterion::Dependency { dependency }) => {
                Self::rank(item.recommended_dependency == Some(*dependency))
            }
        }
    }

    fn rank(matched: bool) -> ItemFit {
        if matched {
            ItemFit::Recommended
        } else {
            ItemFit::Usable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(dependency: SkillDependency, weapon_classes: Vec<WeaponClass>) -> Skill {
        let mut skill = Skill::for_test("test", dependency);
        skill.weapon_classes = weapon_classes;
        skill
    }

    fn weapon(class: WeaponClass) -> ItemClassification {
        ItemClassification {
            weapon_class: Some(class),
            ..Default::default()
        }
    }

    const BORIS: CharacterEquipmentClasses<'static> = CharacterEquipmentClasses {
        weapon_classes: &[WeaponClass::Katana, WeaponClass::GreatSword, WeaponClass::Tachi],
        armor_classes: &[ArmorClass::Light, ArmorClass::Heavy, ArmorClass::Magic],
        wrist_types: &[WristType::Knuckle],
    };

    #[test]
    fn 武器はキャラの装備可能種で先に絞り主軸スキルで狭める() {
        let skill = skill(SkillDependency::Hack, vec![WeaponClass::Katana]);
        let rule = EquipmentFitRule::new(PartSlot::Weapon, Some(BORIS), Some(&skill));
        assert_eq!(rule.fit(&weapon(WeaponClass::Katana)), ItemFit::Recommended);
        // ボリスは装備できるが、このスキルの実用武器ではない
        assert_eq!(rule.fit(&weapon(WeaponClass::Tachi)), ItemFit::Usable);
        // ボリスが装備できない
        assert_eq!(rule.fit(&weapon(WeaponClass::Rapier)), ItemFit::Other);
    }

    #[test]
    fn 実用武器種を持たないスキルは依存能力の系統で絞る() {
        let skill = skill(SkillDependency::Hack, vec![]);
        let rule = EquipmentFitRule::new(PartSlot::Weapon, Some(BORIS), Some(&skill));
        assert_eq!(
            rule.criterion(),
            Some(&FitCriterion::WeaponSystems {
                systems: WeaponSystem::for_dependency(SkillDependency::Hack).to_vec(),
            })
        );
        // 刀(HACK)・大剣(INT+HACK)は斬り依存で実用、太刀(STAB+HACK)も系統に入る
        assert_eq!(rule.fit(&weapon(WeaponClass::Katana)), ItemFit::Recommended);
        assert_eq!(
            rule.fit(&weapon(WeaponClass::GreatSword)),
            ItemFit::Recommended
        );
        assert_eq!(rule.fit(&weapon(WeaponClass::Tachi)), ItemFit::Recommended);
    }

    #[test]
    fn 主軸スキルもキャラも無ければ絞らない() {
        let rule = EquipmentFitRule::new(PartSlot::Weapon, None, None);
        assert_eq!(rule.criterion(), None);
        assert_eq!(rule.fit(&weapon(WeaponClass::Rapier)), ItemFit::Usable);
    }

    #[test]
    fn サブアームは物理魔法が分かれるキャラだけ攻撃タイプで狭める() {
        let physical_and_magic = [WristType::PhysicalMagazine, WristType::MagicMagazine];
        assert_eq!(
            WristType::narrow_by_attack_type(&physical_and_magic, AttackType::Magic),
            vec![WristType::MagicMagazine]
        );
        let shield_only = [WristType::Shield];
        assert_eq!(
            WristType::narrow_by_attack_type(&shield_only, AttackType::Magic),
            vec![WristType::Shield]
        );
    }

    #[test]
    fn 武器アビリティは系統に合うものだけ装着できる() {
        use EquipmentAbilityFamily::*;
        assert!(WeaponSystem::Stab.accepts_ability(PointedBlade));
        assert!(!WeaponSystem::Stab.accepts_ability(SharpBlade));
        assert!(WeaponSystem::IntHack.accepts_ability(SharpBlade));
        assert!(WeaponSystem::IntHack.accepts_ability(Intelligence));
        assert!(WeaponSystem::Mr.accepts_ability(MagicResistance));
        // 武器ディレイと失われた魂はどの系統でも装着できる
        for system in WeaponSystem::ALL {
            assert!(system.accepts_ability(WeaponDelay));
            assert!(system.accepts_ability(Vitality));
        }
    }
}
