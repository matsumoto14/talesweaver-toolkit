//! 装備カタログ(部位別アイテム・武器系統・装備強化倍率・武器アビリティ)。
//!
//! 出典: 装備システム / 装備システム/エンチャント / 装備システム/装備強化 / 装備システム/アビリティ /
//! `Link/装備Item` から辿れる武器・防具・アクセサリ各ページ(取得 2026-08-27)。
//! docs/claude/goals/2026-08-24-equipment-parts.md「インファーナルより上位の全装備カタログ」節参照。

use domain::{
    BaseStats, DamageCategory, EnhanceGrade, EnhanceRates, Equipment,
    EquipmentAbilityAdditionalDef, EquipmentAbilityAdditionalKind, EquipmentAbilityDef,
    EquipmentAbilityFamily, EquipmentEnhanceType, EquipmentValues, PartSlot, SkillDependency,
    SkillEffect,
};

use crate::Source;

#[path = "equipment_catalog_generated.rs"]
mod equipment_catalog_generated;
#[path = "equipment_catalog_sacred_kr.rs"]
mod equipment_catalog_sacred_kr;

/// 装備カタログの出典。
pub const EQUIPMENT_CATALOG_SOURCE: Source = Source {
    page: "Link/装備Item とリンク先の部位別 Item ページ",
    retrieved_on: "2026-08-27",
    note: "各ページで最後のインファーナルより後。数値未確定行は除外",
};

/// 装備強化(装備システム/装備強化)の出典。
pub const ENHANCE_SOURCE: Source = Source {
    page: "装備システム/装備強化",
    retrieved_on: "2026-08-24",
    note: "武器系統ごとの補正式・倍率表。+12以上はレンジ内ランダム(MR)",
};

/// 武器アビリティ(装備システム/アビリティ)の出典。
pub const EQUIPMENT_ABILITY_SOURCE: Source = Source {
    page: "装備システム/アビリティ",
    retrieved_on: "2026-08-27",
    note: "武器3スロット。カテゴリー1/3は旧装着アビリティ、カテゴリー4は新装着アビリティページ。追加効果は自動適用しない",
};

/// 武器種(wiki: 装備システム/装備強化「系統」表の該当武器)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
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

/// 武器系統(wiki: 装備システム/装備強化「系統」列)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponSystem {
    Stab,
    StabHack,
    Hack,
    Int,
    IntHack,
    Mr,
}

/// 腕装備の区分(wiki: `Item/防具/腕/*`)。
/// キャラパッシブは「盾部位全般」と「バンドだけ」を区別するため、表示名ではなく
/// カタログの分類として持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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

fn wrist_type_from_page(page: &str) -> Option<WristType> {
    let category = page
        .strip_prefix("Item/防具/腕/")
        .or_else(|| page.strip_prefix("韓国コミュニティ装備整理シート/"))?;
    Some(match category {
        "シールド" | "방패" => WristType::Shield,
        "スペルブック" | "스펠북" => WristType::Spellbook,
        "ナックル" | "리스트" => WristType::Knuckle,
        "バンド" | "밴드" => WristType::Band,
        "ブレスレット（護符）" | "암릿" => WristType::Bracelet,
        "ペンデュラム" | "펜듈럼" => WristType::Pendulum,
        "水晶玉" | "수정구" => WristType::CrystalBall,
        "物理双剣" | "물리검(sub)" => WristType::DualBladePhysical,
        "物理弾倉" | "물리탄창" => WristType::PhysicalMagazine,
        "魔力弾倉" | "마법탄창" => WristType::MagicMagazine,
        "魔法双剣" | "마법검(sub)" => WristType::DualBladeMagic,
        _ => return None,
    })
}

pub fn weapon_system(class: WeaponClass) -> WeaponSystem {
    use WeaponClass::*;
    use WeaponSystem::*;
    match class {
        Rapier | Dagger | Spear | SmallSword | PhysicalGun | Claw | HandLauncher => Stab,
        LongSword | Tachi | WarStaff | ShortSword | Rod | Nunchaku => StabHack,
        Katana | Axe | Whip | Kara | DualBladePhysical | Scythe | ArmingSword | SwordShape => Hack,
        MagicWand | Wand | MagicGun | Scepter | Totem => Int,
        GreatSword => IntHack,
        HolyStaff | Handbell | DualBladeMagic | Hammer => Mr,
    }
}

/// 武器系統ごとの強化補正式の係数(wiki: 装備システム/装備強化)。
/// 装着アビリティによる補正は含めない(補正は Item の実測 base 値のみで算出する)。
pub fn enhance_rates(class: WeaponClass) -> EnhanceRates {
    match weapon_system(class) {
        // 突き攻撃力 x 6.67 + 斬り攻撃力 x 1.00
        WeaponSystem::Stab => EnhanceRates {
            thrust: 6.67,
            slash: 1.00,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        // 突き攻撃力 x 4.55 + 斬り攻撃力 x 4.55
        WeaponSystem::StabHack => EnhanceRates {
            thrust: 4.55,
            slash: 4.55,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        // 斬り攻撃力 x 6.67 + 突き攻撃力 x 1.00
        WeaponSystem::Hack => EnhanceRates {
            thrust: 1.00,
            slash: 6.67,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        // 魔法攻撃力 x 6.95 + 魔法防御力 x 1.05
        WeaponSystem::Int => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 6.95,
            magic_defense: 1.05,
        },
        // 魔法攻撃力 x 4.55 + 斬り攻撃力 x 3.85
        WeaponSystem::IntHack => EnhanceRates {
            thrust: 0.0,
            slash: 3.85,
            magic_attack: 4.55,
            magic_defense: 0.0,
        },
        // 魔法防御力 x 7.70 + 魔法攻撃力 x 0.70
        WeaponSystem::Mr => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 0.70,
            magic_defense: 7.70,
        },
    }
}

pub fn enhance_rates_for_type(kind: EquipmentEnhanceType) -> Option<EnhanceRates> {
    Some(match kind {
        EquipmentEnhanceType::WeaponStab => EnhanceRates {
            thrust: 6.67,
            slash: 1.00,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponStabHack => EnhanceRates {
            thrust: 4.55,
            slash: 4.55,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponHack => EnhanceRates {
            thrust: 1.00,
            slash: 6.67,
            magic_attack: 0.0,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponInt => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 6.95,
            magic_defense: 1.05,
        },
        EquipmentEnhanceType::WeaponIntHack => EnhanceRates {
            thrust: 0.0,
            slash: 3.85,
            magic_attack: 4.55,
            magic_defense: 0.0,
        },
        EquipmentEnhanceType::WeaponMr => EnhanceRates {
            thrust: 0.0,
            slash: 0.0,
            magic_attack: 0.70,
            magic_defense: 7.70,
        },
        _ => return None,
    })
}

/// 装備強化 +1〜+11 の確定倍率(wiki: 装備システム/装備強化)。範囲外は `None`。
pub fn enhance_multiplier(level: u8) -> Option<f64> {
    match level {
        1 => Some(0.4),
        2 => Some(1.0),
        3 => Some(1.8),
        4 => Some(3.0),
        5 => Some(4.6),
        6 => Some(6.8),
        7 => Some(9.6),
        8 => Some(14.2),
        9 => Some(20.6),
        10 => Some(28.8),
        11 => Some(40.0),
        _ => None,
    }
}

/// 装備強化 +12〜+15 のレンジ倍率(wiki: 装備システム/装備強化。MR で振り直し可)。範囲外は `None`。
pub fn enhance_multiplier_range(level: u8) -> Option<(f64, f64)> {
    match level {
        12 => Some((140.0, 280.0)),
        13 => Some((300.0, 460.0)),
        14 => Some((480.0, 660.0)),
        15 => Some((680.0, 880.0)),
        _ => None,
    }
}

/// 確率区分の上端に対応する倍率。倍率は整数へ四捨五入する。
pub fn enhance_grade_multiplier(level: u8, grade: EnhanceGrade) -> Option<f64> {
    let (min, max) = enhance_multiplier_range(level)?;
    Some(domain::round_int(min + (max - min) * grade.percentile()) as f64)
}

pub fn armor_enhance_multiplier(level: u8, grade: Option<EnhanceGrade>) -> Option<f64> {
    enhance_multiplier(level)
        .or_else(|| grade.and_then(|g| enhance_grade_multiplier(level, g)))
        .map(|v| v / 2.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorClass {
    Light,
    Heavy,
    Magic,
    Suit,
    Robe,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmorEnhanceRates {
    pub physical_defense: f64,
    pub magic_defense: f64,
}

pub fn armor_enhance_rates(class: ArmorClass) -> ArmorEnhanceRates {
    match class {
        ArmorClass::Light => ArmorEnhanceRates {
            physical_defense: 3.90,
            magic_defense: 4.00,
        },
        ArmorClass::Heavy => ArmorEnhanceRates {
            physical_defense: 3.10,
            magic_defense: 3.80,
        },
        ArmorClass::Magic => ArmorEnhanceRates {
            physical_defense: 3.80,
            magic_defense: 4.00,
        },
        ArmorClass::Suit => ArmorEnhanceRates {
            physical_defense: 7.80,
            magic_defense: 0.00,
        },
        ArmorClass::Robe => ArmorEnhanceRates {
            physical_defense: 4.00,
            magic_defense: 3.80,
        },
    }
}

pub fn armor_class_for_type(kind: EquipmentEnhanceType) -> Option<ArmorClass> {
    match kind {
        EquipmentEnhanceType::ArmorLight => Some(ArmorClass::Light),
        EquipmentEnhanceType::ArmorHeavy => Some(ArmorClass::Heavy),
        EquipmentEnhanceType::ArmorMagic => Some(ArmorClass::Magic),
        EquipmentEnhanceType::ArmorSuit => Some(ArmorClass::Suit),
        EquipmentEnhanceType::ArmorRobe => Some(ArmorClass::Robe),
        _ => None,
    }
}

/// カタログ品の補正式。鎧は出典文字列から推測せず、アイテムIDへ明示的に割り当てる。
pub fn equipment_enhance_type(item_id: &str) -> Option<EquipmentEnhanceType> {
    let item = find_equipment_item(item_id)?;
    if let Some(class) = item.weapon_class {
        return Some(match weapon_system(class) {
            WeaponSystem::Stab => EquipmentEnhanceType::WeaponStab,
            WeaponSystem::StabHack => EquipmentEnhanceType::WeaponStabHack,
            WeaponSystem::Hack => EquipmentEnhanceType::WeaponHack,
            WeaponSystem::Int => EquipmentEnhanceType::WeaponInt,
            WeaponSystem::IntHack => EquipmentEnhanceType::WeaponIntHack,
            WeaponSystem::Mr => EquipmentEnhanceType::WeaponMr,
        });
    }
    item.enhance_type
}

/// 現在収録済み鎧の分類。`equipment_enhance_type` の明示メタデータだけから解決する。
pub fn armor_class(item_id: &str) -> Option<ArmorClass> {
    match equipment_enhance_type(item_id)? {
        EquipmentEnhanceType::ArmorLight => Some(ArmorClass::Light),
        EquipmentEnhanceType::ArmorHeavy => Some(ArmorClass::Heavy),
        EquipmentEnhanceType::ArmorMagic => Some(ArmorClass::Magic),
        EquipmentEnhanceType::ArmorSuit => Some(ArmorClass::Suit),
        EquipmentEnhanceType::ArmorRobe => Some(ArmorClass::Robe),
        _ => None,
    }
}

/// 装備カタログの 1 アイテム。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentSurvivalEffect {
    /// 2026-03-04以降のAF「ダメージ緩和」。被ダメージ計算の New2 に相当する。
    DamageMitigation { percent: f64 },
    /// 「盾研磨/防御力 +N%」。ダメージ緩和とは別効果なので混ぜない。
    DefenseRate { percent: f64 },
    /// 「盾研磨/防御力 +N」。割合表記のない固定値。
    DefenseFixed { value: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquipmentItem {
    pub id: &'static str,
    pub slot: PartSlot,
    pub name: &'static str,
    /// 基本能力値のレンジ下限(wiki: Item ページの MR レンジ)
    pub values_min: EquipmentValues,
    /// 基本能力値のレンジ上限
    pub values_max: EquipmentValues,
    /// 成長装備の各基本能力値の入力上限。通常装備は `None`。
    pub growth_cap: Option<i64>,
    /// 補正ごとに上限が違う成長装備。カフスのような一律上限もここへ展開して公開する。
    pub growth_caps: Option<EquipmentValues>,
    /// この装備品が持つアビリティ枠。神鳥レリックは0、ルナリアは1。
    pub ability_slots: usize,
    /// この装備品が持つ付加オプション枠。神鳥レリックは無し、ルナリアは2。
    pub random_option_slots: Option<usize>,
    /// 装備固有のエンチャント枠。実物の基本能力値によらず固定。
    /// wiki の「上限」行から `上限 - 基本能力値レンジ上限` で取り込む。エンチャント不可は 0。
    pub enchant_caps: EquipmentValues,
    /// 腕装備だけ `Some`。バンド装着時パッシブの判定に使う。
    pub wrist_type: Option<WristType>,
    /// 武器のみ `Some`(強化補正式の系統決定に使う)
    pub weapon_class: Option<WeaponClass>,
    /// 鎧のみ `Some`。防具種ごとの装備強化補正式を、出典文字列ではなく明示メタデータで持つ。
    pub enhance_type: Option<EquipmentEnhanceType>,
    /// **装着時効果**(wiki: Item ページ備考の「装着時 …」)。装備補正値ではなく
    /// 与ダメージ式のカテゴリ(X5 / X6 / Old / O)に入る。
    /// **「一定確率で」のものも発動前提で入れる**(ユーザー確定 2026-08-27: ほぼ発動する)
    pub damage_effects: &'static [SkillEffect],
    /// AFなどの耐久側固有効果。攻撃者の与ダメージ式へは混ぜず、耐久計算の供給源として分離する。
    pub survival_effects: &'static [EquipmentSurvivalEffect],
    /// 候補を主軸スキルへ絞るための推奨依存。効果の発動条件とは別物。
    pub recommended_dependency: Option<SkillDependency>,
    /// `damage_effects` がこの依存のスキルにだけ効く場合の条件。
    pub damage_dependency: Option<SkillDependency>,
    pub source: Source,
}

/// wiki の生データ。公開モデルへ変換するときに総上限を固定のエンチャント枠へ変える。
#[derive(Debug, Clone, Copy)]
struct WikiEquipmentItem {
    id: &'static str,
    slot: PartSlot,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    growth_cap: Option<i64>,
    enchant_total_caps: EquipmentValues,
    weapon_class: Option<WeaponClass>,
    enhance_type: Option<EquipmentEnhanceType>,
    damage_effects: &'static [SkillEffect],
    source: Source,
}

impl WikiEquipmentItem {
    fn into_item(self) -> EquipmentItem {
        let cap = |total: i64, maximum: i64| {
            if total == 0 {
                0
            } else {
                (total - maximum).max(0)
            }
        };
        let (recommended_dependency, damage_dependency) = item_dependencies(self.id);
        let godbird_relic = self.id.starts_with("godbird-");
        let growth_caps = match self.slot {
            PartSlot::RelicPendant | PartSlot::RelicBracelet => Some(self.values_max),
            _ => self.growth_cap.map(|cap| v(cap, cap, cap, cap, cap, cap, cap, cap, cap)),
        };
        EquipmentItem {
            id: self.id,
            slot: self.slot,
            name: self.name,
            values_min: self.values_min,
            values_max: self.values_max,
            growth_cap: self.growth_cap,
            growth_caps,
            ability_slots: if godbird_relic { 0 } else { self.slot.ability_slots() },
            random_option_slots: if godbird_relic { None } else { self.slot.random_option_slots() },
            enchant_caps: EquipmentValues {
                thrust: cap(self.enchant_total_caps.thrust, self.values_max.thrust),
                slash: cap(self.enchant_total_caps.slash, self.values_max.slash),
                physical_defense: cap(
                    self.enchant_total_caps.physical_defense,
                    self.values_max.physical_defense,
                ),
                magic_attack: cap(
                    self.enchant_total_caps.magic_attack,
                    self.values_max.magic_attack,
                ),
                magic_defense: cap(
                    self.enchant_total_caps.magic_defense,
                    self.values_max.magic_defense,
                ),
                accuracy: cap(self.enchant_total_caps.accuracy, self.values_max.accuracy),
                critical: cap(self.enchant_total_caps.critical, self.values_max.critical),
                evasion: cap(self.enchant_total_caps.evasion, self.values_max.evasion),
                agility: cap(self.enchant_total_caps.agility, self.values_max.agility),
            },
            wrist_type: wrist_type_from_page(self.source.page),
            weapon_class: self.weapon_class,
            enhance_type: self.enhance_type,
            damage_effects: self.damage_effects,
            survival_effects: item_survival_effects(self.id),
            recommended_dependency,
            damage_dependency,
            source: self.source,
        }
    }
}

const SURVIVAL_MITIGATION_10: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DamageMitigation { percent: 10.0 }];
const SURVIVAL_MITIGATION_15: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DamageMitigation { percent: 15.0 }];
const SURVIVAL_MITIGATION_40: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DamageMitigation { percent: 40.0 }];
const SURVIVAL_DEFENSE_FIXED_15: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DefenseFixed { value: 15 }];
const SURVIVAL_DEFENSE_RATE_20: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DefenseRate { percent: 20.0 }];
const SURVIVAL_DEFENSE_RATE_30: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DefenseRate { percent: 30.0 }];

fn item_survival_effects(id: &str) -> &'static [EquipmentSurvivalEffect] {
    match id {
        // 2024-02-28追加。2026-03-04に「ダメージ耐性」から「ダメージ緩和」へ置換。
        "psyche-stab" | "psyche-hack" | "psyche-physical" | "psyche-int" | "psyche-mr"
        | "psyche-hack-int" | "arklon-hack-int" | "eclipse-stab" | "eclipse-hack"
        | "eclipse-physical" | "eclipse-int" | "eclipse-mr" | "eclipse-hack-int" => {
            SURVIVAL_MITIGATION_10
        }
        // リンゴの島ディフェンシオはWikiの「盾研磨/防御力+15」どおり固定値。
        "psyche-stab-def" | "psyche-hack-def" | "psyche-physical-def" | "psyche-int-def"
        | "psyche-mr-def" | "psyche-hack-int-def" => SURVIVAL_DEFENSE_FIXED_15,
        // アークロン・エクリプスのディフェンシオは「盾研磨/防御力+30%」。
        "arklon-physical-def" | "arklon-int-def" | "arklon-hack-int-def"
        | "eclipse-stab-def" | "eclipse-hack-def" | "eclipse-physical-def"
        | "eclipse-int-def" | "eclipse-mr-def" | "eclipse-hack-int-def" => {
            SURVIVAL_DEFENSE_RATE_30
        }
        // ゆがんだ村の地域表に明記された現行値。
        "ethereal-stab" | "ethereal-hack" | "ethereal-physical" | "ethereal-int"
        | "ethereal-mr" | "ethereal-hack-int" => SURVIVAL_MITIGATION_15,
        "ethereal-stab-def" | "ethereal-hack-def" | "ethereal-physical-def"
        | "ethereal-int-def" | "ethereal-mr-def" | "ethereal-hack-int-def" => {
            SURVIVAL_MITIGATION_40
        }
        // 現在収録しているコラボAFの「ダメージ20%上昇・防御力20%上昇」。
        "dungeon-meshi-picking-tools" | "dungeon-meshi-gourmet-guide"
        | "dungeon-meshi-thistle-book" | "maid-dragon-magic-orb"
        | "log-horizon-akatsuki-doll" => SURVIVAL_DEFENSE_RATE_20,
        _ => &[],
    }
}

fn item_dependencies(id: &str) -> (Option<SkillDependency>, Option<SkillDependency>) {
    use SkillDependency::*;
    match id {
        "eclipse-stab" => (Some(Stab), Some(Stab)),
        "eclipse-stab-def" => (Some(Stab), Some(Stab)),
        "eclipse-hack" => (Some(Hack), Some(Hack)),
        "eclipse-hack-def" => (Some(Hack), Some(Hack)),
        "eclipse-physical" => (Some(StabHack), Some(StabHack)),
        "eclipse-physical-def" => (Some(StabHack), Some(StabHack)),
        "eclipse-int" => (Some(Int), Some(Int)),
        "eclipse-int-def" => (Some(Int), Some(Int)),
        "eclipse-mr" => (Some(Mr), Some(Mr)),
        "eclipse-mr-def" => (Some(Mr), Some(Mr)),
        "eclipse-hack-int" => (Some(HackInt), Some(HackInt)),
        "eclipse-hack-int-def" => (Some(HackInt), Some(HackInt)),
        "arklon-physical-def" => (Some(StabHack), Some(StabHack)),
        "arklon-int-def" => (Some(Int), Some(Int)),
        "arklon-hack-int-def" => (Some(HackInt), Some(HackInt)),
        "psyche-stab-def" => (Some(Stab), Some(Stab)),
        "psyche-hack-def" => (Some(Hack), Some(Hack)),
        "psyche-physical-def" => (Some(StabHack), Some(StabHack)),
        "psyche-int-def" => (Some(Int), Some(Int)),
        "psyche-mr-def" => (Some(Mr), Some(Mr)),
        "psyche-hack-int-def" => (Some(HackInt), Some(HackInt)),
        "psyche-stab" => (Some(Stab), Some(Stab)),
        "psyche-hack" => (Some(Hack), Some(Hack)),
        "psyche-physical" => (Some(StabHack), Some(StabHack)),
        "psyche-int" => (Some(Int), Some(Int)),
        "psyche-mr" => (Some(Mr), Some(Mr)),
        "psyche-hack-int" => (Some(HackInt), Some(HackInt)),
        "ethereal-stab-def" => (Some(Stab), Some(Stab)),
        "ethereal-hack-def" => (Some(Hack), Some(Hack)),
        "ethereal-physical-def" => (Some(StabHack), Some(StabHack)),
        "ethereal-int-def" => (Some(Int), Some(Int)),
        "ethereal-mr-def" => (Some(Mr), Some(Mr)),
        "ethereal-hack-int-def" => (Some(HackInt), Some(HackInt)),
        "ethereal-stab" => (Some(Stab), None),
        "ethereal-hack" => (Some(Hack), None),
        "ethereal-physical" => (Some(StabHack), None),
        "ethereal-int" => (Some(Int), None),
        "ethereal-mr" => (Some(Mr), None),
        "ethereal-hack-int" => (Some(HackInt), None),
        "dungeon-meshi-picking-tools" => (Some(Stab), None),
        "dungeon-meshi-gourmet-guide" => (Some(Hack), None),
        "dungeon-meshi-thistle-book" => (Some(Int), None),
        "maid-dragon-magic-orb" => (Some(Mr), None),
        "log-horizon-akatsuki-doll" => (Some(StabHack), None),
        "arklon-hack-int" => (Some(HackInt), Some(HackInt)),
        _ => (None, None),
    }
}

impl serde::Serialize for EquipmentItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("EquipmentItem", 19)?;
        s.serialize_field("id", self.id)?;
        s.serialize_field("slot", &self.slot)?;
        s.serialize_field("name", self.name)?;
        s.serialize_field("values_min", &self.values_min)?;
        s.serialize_field("values_max", &self.values_max)?;
        s.serialize_field("growth_cap", &self.growth_cap)?;
        s.serialize_field("growth_caps", &self.growth_caps)?;
        s.serialize_field("ability_slots", &self.ability_slots)?;
        s.serialize_field("random_option_slots", &self.random_option_slots)?;
        s.serialize_field("enchant_caps", &self.enchant_caps)?;
        s.serialize_field("wrist_type", &self.wrist_type)?;
        s.serialize_field("weapon_class", &self.weapon_class)?;
        s.serialize_field("weapon_system", &self.weapon_class.map(weapon_system))?;
        s.serialize_field(
            "enhance_type",
            &self
                .weapon_class
                .map(|class| match weapon_system(class) {
                    WeaponSystem::Stab => EquipmentEnhanceType::WeaponStab,
                    WeaponSystem::StabHack => EquipmentEnhanceType::WeaponStabHack,
                    WeaponSystem::Hack => EquipmentEnhanceType::WeaponHack,
                    WeaponSystem::Int => EquipmentEnhanceType::WeaponInt,
                    WeaponSystem::IntHack => EquipmentEnhanceType::WeaponIntHack,
                    WeaponSystem::Mr => EquipmentEnhanceType::WeaponMr,
                })
                .or(self.enhance_type),
        )?;
        s.serialize_field("damage_effects", &self.damage_effects)?;
        s.serialize_field("survival_effects", &self.survival_effects)?;
        s.serialize_field("recommended_dependency", &self.recommended_dependency)?;
        s.serialize_field("damage_dependency", &self.damage_dependency)?;
        s.serialize_field("source", &self.source)?;
        s.end()
    }
}

const ITEM_SOURCE_NOTE_KATANA: Source = Source {
    page: "Item/武器/刀",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)",
};
const ITEM_SOURCE_NOTE_TACHI: Source = Source {
    page: "Item/武器/太刀",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)",
};
const ITEM_SOURCE_NOTE_GREAT_SWORD: Source = Source {
    page: "Item/武器/大剣",
    retrieved_on: "2026-08-27",
    note: "エンドゲーム帯(Lv300/310)。氷撃斬向け",
};
const ITEM_SOURCE_NOTE_HELM: Source = Source {
    page: "Item/防具/兜",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)",
};
const ITEM_SOURCE_NOTE_ARMOR: Source = Source {
    page: "Item/防具/鎧/軽鎧",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)。魔防のみ(突/斬/魔攻は0)",
};
const ITEM_SOURCE_NOTE_SHIELD: Source = Source {
    page: "Item/防具/腕/シールド",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)。魔防のみ",
};
const ITEM_SOURCE_NOTE_ACCESSORY: Source = Source {
    page: "Item/アクセサリ/顔・体・手・足",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)",
};
const ITEM_SOURCE_NOTE_SHIELD_PLUS: Source = Source {
    page: "Item/防具/腕/盾＋",
    retrieved_on: "2026-08-24",
    note: "エンチャント不可。[EP]チャプターアーティファクトは強化上限10、アルカディア・メメントモリは塔クリアで全補正50",
};

/// 装着時効果つきの装備の出典(2026-08-27 取得)。どの装備がどのカテゴリに入るかは
/// ステータス ページ `#z4747f51` のカテゴリ表が正で、数値と装備補正はここの Item ページから取る。
const ITEM_SOURCE_DAMAGE_KATANA: Source = Source {
    page: "Item/武器/刀",
    retrieved_on: "2026-08-27",
    note: "装着時「与ダメージ+3%」のコラボ武器(Lv310)",
};
const ITEM_SOURCE_DAMAGE_TACHI: Source = Source {
    page: "Item/武器/太刀",
    retrieved_on: "2026-08-27",
    note: "装着時「与ダメージ+3%」のコラボ武器(Lv310)",
};
const ITEM_SOURCE_DAMAGE_ROBE: Source = Source {
    page: "Item/防具/鎧/ローブ",
    retrieved_on: "2026-08-27",
    note: "装着時「魔法での与ダメージ+3%」。突/斬は列が「-」なので 0",
};
const ITEM_SOURCE_DAMAGE_HAND: Source = Source {
    page: "Item/アクセサリ/手",
    retrieved_on: "2026-08-27",
    note: "装着時に与ダメージが上がるコラボ手装備",
};
const ITEM_SOURCE_DAMAGE_BODY: Source = Source {
    page: "Item/アクセサリ/体",
    retrieved_on: "2026-08-27",
    note: "要塞占領報酬。エンチャント・インクリ不可(属性強化のみ)なので上限は全 0",
};
const ITEM_SOURCE_DAMAGE_EFFECT: Source = Source {
    page: "Item/アクセサリ/エフェクト",
    retrieved_on: "2026-08-27",
    note: "エフェクトの攻撃系。「スキル使用時、一定確率で」も発動前提で入れる(ユーザー確定 2026-08-27)。           Lv15 帯の旧コラボ(同じ +3% で補正値が弱い)は wiki に上限行が無いので未収録",
};
const ITEM_SOURCE_STALLION_EFFECT: Source = Source {
    page: "公式お知らせ no=154958 / Item/アクセサリ/エフェクト",
    retrieved_on: "2026-08-27",
    note: "主能力の総上限700は公式。その他8補正の総上限255はユーザー確定 2026-08-27",
};

/// 装着時「与ダメージ+3%」= カテゴリX6 攻撃ダメージ(日本独自)(上限 +30%)。
const ITEM_DAMAGE_JAPAN_3: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageJapan,
    percent: 3.0,
}];
/// 装着時「与ダメージ+1%」= カテゴリX6。
const ITEM_DAMAGE_JAPAN_1: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageJapan,
    percent: 1.0,
}];
/// 装着時「物理/魔法攻撃力 +5%」= カテゴリX6。wiki 注記どおり物理・魔法に関係なく上がる。
const ITEM_DAMAGE_JAPAN_5: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageJapan,
    percent: 5.0,
}];
/// 装着時「攻撃力が3%増加」= カテゴリX5 攻撃ダメージ(特殊)(wiki は上限未記載)。
const ITEM_DAMAGE_SPECIAL_3: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageSpecial,
    percent: 3.0,
}];
/// 要塞占領報酬の体装備「攻撃ダメージ増加」= カテゴリOld 攻撃ダメージII(初期 100%・上限 300%)。
const ITEM_DAMAGE_LEGACY_25: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageLegacy,
    percent: 25.0,
}];
/// 「魔法での与ダメージ+3%」= カテゴリO 物理/魔法ダメージ増加。
/// wiki の注記どおり物理攻撃(熊)にも乗るので、依存で分けずカテゴリO にそのまま入れる。
const ITEM_DAMAGE_PHYSICAL_MAGIC_3: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::PhysicalMagicDamageRate,
    percent: 3.0,
}];
/// コラボ AF の「一定確率でダメージ20%上昇」。発動前提で X5 に入れる。
const ITEM_DAMAGE_SPECIAL_20: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageSpecial,
    percent: 20.0,
}];
/// 依存別 AF の攻撃ダメージ。`damage_dependency` が一致するスキルにだけ適用する。
const ITEM_DAMAGE_DEPENDENCY_20: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::DependencyDamageRate,
    percent: 20.0,
}];
const ITEM_DAMAGE_DEPENDENCY_30: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::DependencyDamageRate,
    percent: 30.0,
}];
const ITEM_DAMAGE_DEPENDENCY_35: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::DependencyDamageRate,
    percent: 35.0,
}];

/// wiki Item ページの列順そのまま: 突き / 斬り / 物防 / 魔攻 / 魔防 / 命中 / Cri補正 / 回避 / 敏捷。
#[rustfmt::skip]
#[allow(clippy::too_many_arguments)]
fn v(
    thrust: i64, slash: i64, physical_defense: i64, magic_attack: i64, magic_defense: i64,
    accuracy: i64, critical: i64, evasion: i64, agility: i64,
) -> EquipmentValues {
    EquipmentValues {
        thrust, slash, physical_defense, magic_attack, magic_defense,
        accuracy, critical, evasion, agility,
    }
}

/// エフェクト 1 件。装備補正はレンジを持たない(MR 個体差の記載が無い)ものがほとんどなので、
/// レンジがある 1 件だけ `values_max` を別に渡す。
fn effect_item(
    id: &'static str,
    name: &'static str,
    values: EquipmentValues,
    enchant_total_caps: EquipmentValues,
    damage_effects: &'static [SkillEffect],
) -> WikiEquipmentItem {
    WikiEquipmentItem {
        id,
        slot: PartSlot::Effect,
        name,
        values_min: values,
        values_max: values,
        growth_cap: None,
        enchant_total_caps,
        weapon_class: None,
        enhance_type: None,
        damage_effects,
        source: ITEM_SOURCE_DAMAGE_EFFECT,
    }
}

fn stallion_effect(
    id: &'static str,
    name: &'static str,
    values: EquipmentValues,
    enchant_total_caps: EquipmentValues,
) -> WikiEquipmentItem {
    WikiEquipmentItem {
        source: ITEM_SOURCE_STALLION_EFFECT,
        ..effect_item(id, name, values, enchant_total_caps, &[])
    }
}

fn defensio_artifact(
    id: &'static str,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_total_caps: EquipmentValues,
    damage_effects: &'static [SkillEffect],
    note: &'static str,
) -> WikiEquipmentItem {
    WikiEquipmentItem {
        id,
        slot: PartSlot::Artifact,
        name,
        values_min,
        values_max,
        growth_cap: None,
        enchant_total_caps,
        weapon_class: None,
        enhance_type: None,
        damage_effects,
        source: Source {
            page: "Item/アクセサリー用装備/アーティファクト",
            retrieved_on: "2026-08-27",
            note,
        },
    }
}

fn artifact_item(
    id: &'static str,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_total_caps: EquipmentValues,
    damage_effects: &'static [SkillEffect],
    note: &'static str,
) -> WikiEquipmentItem {
    defensio_artifact(id, name, values_min, values_max, enchant_total_caps, damage_effects, note)
}

/// 神鳥・ルナリアレリック。各段階は直前段階の完成値から始まり、表の値まで成長する。
fn relic_item(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    min_main: i64,
    min_sub: i64,
    max_main: i64,
    max_sub: i64,
) -> WikiEquipmentItem {
    let values = |main: i64, sub: i64| match slot {
        PartSlot::RelicPendant => v(main, main, 0, main, 0, sub, sub, 0, 0),
        PartSlot::RelicBracelet => v(0, 0, main, 0, main, 0, 0, sub, sub),
        _ => unreachable!("レリック以外の部位が指定されました"),
    };
    WikiEquipmentItem {
        id,
        slot,
        name,
        values_min: values(min_main, min_sub),
        values_max: values(max_main, max_sub),
        growth_cap: None,
        enchant_total_caps: EquipmentValues::default(),
        weapon_class: None,
        enhance_type: None,
        damage_effects: &[],
        source: Source {
            page: "Item/アクセサリ/レリック/神鳥のレリック・ルナリアレリック",
            retrieved_on: "2026-08-28",
            note: "直前段階の全補正MAXから開始し、表示段階のMAXまでランダム成長。エンチャント不可",
        },
    }
}

/// 「装着時攻撃力が3%増加」= カテゴリX5。5 種の違いは特化する 1 値(20)だけで、
/// 残りの装備補正は 5、命中/Cri/回避/敏捷は 18、装備本体との総上限は全 255 で共通。
fn effect_attack_3(
    id: &'static str,
    name: &'static str,
    values: EquipmentValues,
) -> WikiEquipmentItem {
    effect_item(
        id,
        name,
        values,
        v(255, 255, 255, 255, 255, 255, 255, 255, 255),
        ITEM_DAMAGE_SPECIAL_3,
    )
}

/// 「スキル使用時、一定確率で攻撃ダメージ(攻撃力)が3%上昇」= カテゴリX6。
/// Lv310 帯のコラボエフェクト。補正値は全 25 で、特化する 1 値と上限だけが違う。
fn effect_trigger_3(
    id: &'static str,
    name: &'static str,
    values: EquipmentValues,
    enchant_total_caps: EquipmentValues,
) -> WikiEquipmentItem {
    effect_item(id, name, values, enchant_total_caps, ITEM_DAMAGE_JAPAN_3)
}

/// 宝箱「凛々の明星」の 4 種。1 値だけ 30〜50 の MR レンジを持ち、ほかは全 25。
fn effect_trigger_3_ranged(
    id: &'static str,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_total_caps: EquipmentValues,
) -> WikiEquipmentItem {
    WikiEquipmentItem {
        id,
        slot: PartSlot::Effect,
        name,
        values_min,
        values_max,
        growth_cap: None,
        enchant_total_caps,
        weapon_class: None,
        enhance_type: None,
        damage_effects: ITEM_DAMAGE_JAPAN_3,
        source: ITEM_SOURCE_DAMAGE_EFFECT,
    }
}

/// 装備カタログ。エンドゲーム帯 20 件 +「装着時に与ダメージが上がる」装備 19 件。
/// 後者は装備補正値だけでなく `damage_effects` を持ち、与ダメージ式のカテゴリに入る。
pub fn equipment_catalog() -> Vec<EquipmentItem> {
    let mut catalog = vec![
        WikiEquipmentItem {
            id: "aquilus-scimitar",
            slot: PartSlot::Weapon,
            name: "†アクィルスシミター",
            values_min: v(95, 233, 36, 39, 33, 34, 27, 30, 28),
            values_max: v(105, 243, 39, 45, 35, 36, 30, 31, 31),
            growth_cap: None,
            enchant_total_caps: v(280, 300, 280, 280, 280, 280, 37, 280, 280),
            weapon_class: Some(WeaponClass::Katana),
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_KATANA,
        },
        WikiEquipmentItem {
            id: "abyss-scimitar",
            slot: PartSlot::Weapon,
            name: "†アビスシミター",
            values_min: v(115, 300, 36, 39, 33, 34, 30, 27, 28),
            values_max: v(130, 330, 39, 45, 35, 36, 31, 30, 31),
            growth_cap: None,
            enchant_total_caps: v(400, 400, 100, 100, 100, 100, 100, 100, 100),
            weapon_class: Some(WeaponClass::Katana),
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_KATANA,
        },
        WikiEquipmentItem {
            id: "aquilus-fake-sword",
            slot: PartSlot::Weapon,
            name: "†アクィルスフェイクソード",
            values_min: v(167, 170, 39, 41, 33, 34, 29, 29, 29),
            values_max: v(177, 180, 41, 47, 36, 36, 32, 32, 34),
            growth_cap: None,
            enchant_total_caps: v(300, 300, 280, 280, 280, 280, 37, 280, 280),
            weapon_class: Some(WeaponClass::Tachi),
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_TACHI,
        },
        WikiEquipmentItem {
            id: "abyss-fake-sword",
            slot: PartSlot::Weapon,
            name: "†アビスフェイクソード",
            values_min: v(215, 215, 39, 41, 33, 34, 29, 29, 29),
            values_max: v(235, 235, 41, 47, 36, 36, 32, 32, 34),
            growth_cap: None,
            enchant_total_caps: v(400, 400, 100, 100, 100, 100, 100, 100, 100),
            weapon_class: Some(WeaponClass::Tachi),
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_TACHI,
        },
        WikiEquipmentItem {
            id: "aquilus-great-sword",
            slot: PartSlot::Weapon,
            name: "†アクィルスブレイド",
            values_min: v(80, 184, 35, 161, 38, 34, 29, 28, 26),
            values_max: v(85, 194, 38, 171, 40, 38, 32, 30, 28),
            growth_cap: None,
            enchant_total_caps: v(280, 300, 280, 300, 280, 280, 37, 280, 280),
            weapon_class: Some(WeaponClass::GreatSword),
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_GREAT_SWORD,
        },
        WikiEquipmentItem {
            id: "abyss-great-sword",
            slot: PartSlot::Weapon,
            name: "†アビスブレード",
            values_min: v(84, 230, 35, 230, 38, 34, 29, 28, 26),
            values_max: v(89, 250, 38, 250, 40, 38, 32, 30, 28),
            growth_cap: None,
            enchant_total_caps: v(400, 400, 100, 400, 100, 100, 100, 100, 100),
            weapon_class: Some(WeaponClass::GreatSword),
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_GREAT_SWORD,
        },
        WikiEquipmentItem {
            id: "aquilus-helm",
            slot: PartSlot::Helm,
            name: "†アクィルスヘルム",
            values_min: v(73, 75, 71, 75, 81, 47, 41, 47, 47),
            values_max: v(83, 85, 81, 85, 91, 57, 51, 57, 57),
            growth_cap: None,
            enchant_total_caps: v(113, 115, 105, 115, 121, 81, 57, 81, 81),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_HELM,
        },
        WikiEquipmentItem {
            id: "abyss-helm",
            slot: PartSlot::Helm,
            name: "†アビスヘルム",
            values_min: v(92, 92, 94, 92, 104, 82, 82, 82, 82),
            values_max: v(102, 102, 124, 102, 134, 92, 92, 92, 92),
            growth_cap: None,
            enchant_total_caps: v(122, 122, 154, 122, 164, 112, 112, 112, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_HELM,
        },
        WikiEquipmentItem {
            id: "aquilus-armor",
            slot: PartSlot::Armor,
            name: "†アクィルスアーマー",
            values_min: v(0, 0, 197, 0, 181, 0, 0, 102, 0),
            values_max: v(0, 0, 207, 0, 191, 0, 0, 112, 0),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 237, 0, 221, 0, 0, 136, 0),
            weapon_class: None,
            enhance_type: Some(EquipmentEnhanceType::ArmorLight),
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ARMOR,
        },
        WikiEquipmentItem {
            id: "abyss-armor",
            slot: PartSlot::Armor,
            name: "†アビスアーマー",
            values_min: v(0, 0, 260, 0, 230, 0, 0, 100, 0),
            values_max: v(0, 0, 280, 0, 260, 0, 0, 120, 0),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 310, 0, 290, 0, 0, 150, 0),
            weapon_class: None,
            enhance_type: Some(EquipmentEnhanceType::ArmorLight),
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ARMOR,
        },
        WikiEquipmentItem {
            id: "aquilus-shield",
            slot: PartSlot::Shield,
            name: "†アクィルスシールド",
            values_min: v(0, 0, 177, 0, 172, 0, 0, 0, 0),
            values_max: v(0, 0, 187, 0, 182, 0, 0, 0, 0),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 217, 0, 212, 0, 0, 0, 0),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_SHIELD,
        },
        WikiEquipmentItem {
            id: "abyss-shield",
            slot: PartSlot::Shield,
            name: "†アビスシールド",
            values_min: v(0, 0, 200, 0, 200, 0, 0, 0, 0),
            values_max: v(0, 0, 220, 0, 220, 0, 0, 0, 0),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 260, 0, 260, 0, 0, 0, 0),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_SHIELD,
        },
        WikiEquipmentItem {
            id: "aquilus-amulet",
            slot: PartSlot::Head,
            name: "†アクィルスアミュレット",
            values_min: v(73, 75, 68, 73, 84, 45, 39, 45, 45),
            values_max: v(83, 85, 78, 83, 94, 55, 49, 55, 55),
            growth_cap: None,
            enchant_total_caps: v(113, 115, 92, 113, 124, 79, 55, 79, 79),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "abyss-amulet",
            slot: PartSlot::Head,
            name: "†アビスアミュレット",
            values_min: v(92, 92, 82, 92, 92, 82, 94, 82, 82),
            values_max: v(102, 102, 92, 102, 102, 92, 124, 92, 92),
            growth_cap: None,
            enchant_total_caps: v(122, 122, 112, 122, 122, 112, 154, 112, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "aquilus-wing",
            slot: PartSlot::Body,
            name: "†アクィルスウィング",
            values_min: v(76, 76, 62, 76, 78, 48, 42, 48, 48),
            values_max: v(86, 86, 72, 86, 88, 58, 52, 58, 58),
            growth_cap: None,
            enchant_total_caps: v(116, 116, 96, 116, 118, 78, 58, 82, 82),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "abyss-wing",
            slot: PartSlot::Body,
            name: "†アビスウィング",
            values_min: v(94, 94, 82, 94, 82, 82, 82, 82, 82),
            values_max: v(124, 124, 92, 124, 92, 92, 92, 92, 92),
            growth_cap: None,
            enchant_total_caps: v(154, 154, 112, 154, 112, 112, 112, 112, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "aquilus-gauntlet",
            slot: PartSlot::Hand,
            name: "†アクィルスガントレット",
            values_min: v(72, 72, 56, 72, 72, 90, 44, 44, 44),
            values_max: v(82, 82, 66, 82, 82, 110, 54, 54, 54),
            growth_cap: None,
            enchant_total_caps: v(112, 112, 90, 112, 112, 130, 60, 78, 78),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "abyss-gauntlet",
            slot: PartSlot::Hand,
            name: "†アビスガントレット",
            values_min: v(92, 92, 82, 92, 92, 150, 82, 82, 82),
            values_max: v(102, 102, 92, 102, 102, 180, 92, 92, 92),
            growth_cap: None,
            enchant_total_caps: v(122, 122, 112, 122, 122, 210, 112, 112, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "aquilus-boots",
            slot: PartSlot::Leg,
            name: "†アクィルスブーツ",
            values_min: v(72, 72, 56, 72, 72, 44, 44, 90, 44),
            values_max: v(82, 82, 66, 82, 82, 54, 54, 110, 54),
            growth_cap: None,
            enchant_total_caps: v(112, 112, 90, 112, 112, 78, 60, 130, 78),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "abyss-boots",
            slot: PartSlot::Leg,
            name: "†アビスブーツ",
            values_min: v(92, 92, 82, 92, 92, 82, 82, 150, 82),
            values_max: v(102, 102, 92, 102, 102, 92, 92, 180, 92),
            growth_cap: None,
            enchant_total_caps: v(122, 122, 112, 122, 122, 112, 112, 210, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        WikiEquipmentItem {
            id: "chapter-artifact",
            slot: PartSlot::ShieldPlus,
            name: "[EP]†チャプターアーティファクト",
            values_min: v(1, 1, 1, 1, 1, 1, 1, 1, 1),
            values_max: v(1, 1, 1, 1, 1, 1, 1, 1, 1),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_SHIELD_PLUS,
        },
        WikiEquipmentItem {
            id: "arcadia-mementomori",
            slot: PartSlot::ShieldPlus,
            name: "†アルカディア・メメントモリ",
            values_min: v(50, 50, 50, 50, 50, 50, 50, 50, 50),
            values_max: v(50, 50, 50, 50, 50, 50, 50, 50, 50),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            enhance_type: None,
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_SHIELD_PLUS,
        },
        // ── 装着時効果つき(与ダメージ式のカテゴリに入る)──────────────────────
        // カテゴリX6 攻撃ダメージ(日本独自): コラボ武器「装備時、与ダメージ+3%上昇」
        WikiEquipmentItem {
            id: "nibanboshi-katana",
            slot: PartSlot::Weapon,
            name: "†ニバンボシ(刀)",
            values_min: v(120, 320, 42, 22, 42, 36, 30, 30, 28),
            values_max: v(160, 360, 45, 27, 45, 36, 30, 31, 31),
            growth_cap: None,
            enchant_total_caps: v(460, 480, 100, 100, 100, 105, 105, 100, 100),
            weapon_class: Some(WeaponClass::Katana),
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_KATANA,
        },
        WikiEquipmentItem {
            id: "nibanboshi-tachi",
            slot: PartSlot::Weapon,
            name: "†ニバンボシ(太刀)",
            values_min: v(240, 240, 39, 41, 33, 36, 32, 29, 29),
            values_max: v(260, 260, 41, 47, 36, 36, 32, 32, 34),
            growth_cap: None,
            enchant_total_caps: v(480, 480, 100, 100, 100, 105, 105, 100, 100),
            weapon_class: Some(WeaponClass::Tachi),
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_TACHI,
        },
        // カテゴリO 物理/魔法ダメージ増加
        WikiEquipmentItem {
            id: "lina-clothes",
            slot: PartSlot::Armor,
            name: "†リナの服",
            values_min: v(0, 0, 260, 30, 280, 85, 0, 81, 0),
            values_max: v(0, 0, 280, 45, 300, 115, 0, 91, 0),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 300, 150, 350, 120, 0, 105, 0),
            weapon_class: None,
            enhance_type: Some(EquipmentEnhanceType::ArmorRobe),
            damage_effects: ITEM_DAMAGE_PHYSICAL_MAGIC_3,
            source: ITEM_SOURCE_DAMAGE_ROBE,
        },
        // カテゴリOld 攻撃ダメージII(要塞占領報酬。2 種は補正値まで同じ)
        WikiEquipmentItem {
            id: "archangel-wing",
            slot: PartSlot::Body,
            name: "†主天使の羽",
            values_min: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            values_max: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_LEGACY_25,
            source: ITEM_SOURCE_DAMAGE_BODY,
        },
        WikiEquipmentItem {
            id: "sigma-wing",
            slot: PartSlot::Body,
            name: "†シグマウィング",
            values_min: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            values_max: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            growth_cap: None,
            enchant_total_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_LEGACY_25,
            source: ITEM_SOURCE_DAMAGE_BODY,
        },
        // カテゴリX6: 手装備。けものフレンズコラボは +5%、ダンジョン飯コラボは +3%
        WikiEquipmentItem {
            id: "gorilla-armcover",
            slot: PartSlot::Hand,
            name: "†ゴリラのあーむかばー",
            values_min: v(44, 44, 44, 44, 44, 78, 38, 38, 38),
            values_max: v(54, 54, 54, 54, 54, 100, 48, 48, 48),
            growth_cap: None,
            enchant_total_caps: v(90, 90, 80, 90, 80, 118, 54, 66, 66),
            weapon_class: None,
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_JAPAN_5,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        WikiEquipmentItem {
            id: "tanuki-gloves",
            slot: PartSlot::Hand,
            name: "†タヌキの手袋",
            values_min: v(44, 44, 38, 44, 44, 78, 38, 38, 38),
            values_max: v(54, 54, 48, 54, 54, 100, 48, 48, 48),
            growth_cap: None,
            enchant_total_caps: v(112, 90, 112, 112, 112, 130, 60, 78, 78),
            weapon_class: None,
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_JAPAN_5,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        WikiEquipmentItem {
            id: "izutsumi-gauntlet",
            slot: PartSlot::Hand,
            name: "†イヅツミの手甲",
            values_min: v(80, 80, 82, 60, 60, 150, 82, 82, 82),
            values_max: v(90, 90, 92, 80, 80, 180, 92, 92, 92),
            growth_cap: None,
            enchant_total_caps: v(150, 150, 112, 105, 105, 210, 112, 112, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        WikiEquipmentItem {
            id: "rin-gloves",
            slot: PartSlot::Hand,
            name: "†リンの手袋",
            values_min: v(60, 60, 82, 100, 80, 150, 82, 82, 82),
            values_max: v(80, 80, 92, 120, 90, 180, 92, 92, 92),
            growth_cap: None,
            enchant_total_caps: v(105, 105, 112, 150, 150, 210, 112, 112, 112),
            weapon_class: None,
            enhance_type: None,
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        // カテゴリX5 攻撃ダメージ(特殊): エフェクト(装着時攻撃力 +3%)
        effect_attack_3(
            "beast-cerberus",
            "【年占】†幻獣(ケルベロス)",
            v(20, 5, 5, 5, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "beast-phoenix",
            "【年占】†幻獣(フェニックス)",
            v(5, 20, 5, 5, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "beast-griffon",
            "【年占】†幻獣(グリフォン)",
            v(5, 5, 20, 5, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "beast-leviathan",
            "【年占】†幻獣(リヴァイアサン)",
            v(5, 5, 5, 20, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "beast-unicorn",
            "【年占】†幻獣(ユニコーン)",
            v(5, 5, 5, 5, 20, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "memorial-crest-dark",
            "【18th】†記念の祝福紋様 − 闇",
            v(20, 5, 5, 5, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "memorial-crest-water",
            "【18th】†記念の祝福紋様 − 水",
            v(5, 20, 5, 5, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "memorial-crest-fire",
            "【18th】†記念の祝福紋様 − 炎",
            v(5, 5, 20, 5, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "memorial-crest-light",
            "【18th】†記念の祝福紋様 − 光",
            v(5, 5, 5, 20, 5, 18, 18, 18, 18),
        ),
        effect_attack_3(
            "memorial-crest-wind",
            "【18th】†記念の祝福紋様 − 風",
            v(5, 5, 5, 5, 20, 18, 18, 18, 18),
        ),
        // カテゴリX6: エフェクトの「スキル使用時、一定確率で 3% 上昇」。**発動前提で入れる**
        // (ユーザー確定 2026-08-27)。wiki の文言は「攻撃ダメージ」「攻撃力」で揺れるが、
        // ステータス表 1205 行はどちらも同じ X6 +3% の行にまとめている
        effect_trigger_3(
            "logh-full-control-battle",
            "†全力管制戦闘",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25),
            v(375, 375, 375, 375, 375, 255, 255, 255, 255),
        ),
        effect_trigger_3(
            "slayers-drag-slave",
            "†竜破斬＜ドラグ・スレイブ＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25),
            v(255, 255, 255, 400, 255, 255, 255, 255, 255),
        ),
        effect_trigger_3(
            "slayers-giga-slave",
            "†重破斬＜ギガ・スレイブ＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25),
            v(255, 400, 255, 255, 255, 255, 255, 255, 255),
        ),
        effect_trigger_3(
            "slayers-ragna-blade",
            "†神滅斬＜ラグナ・ブレード＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25),
            v(400, 255, 255, 255, 255, 255, 255, 255, 255),
        ),
        effect_trigger_3(
            "slayers-claire-bible",
            "†異界黙示録＜クレアバイブル＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25),
            v(255, 255, 255, 255, 400, 255, 255, 255, 255),
        ),
        // 宝箱「凛々の明星」の 4 種は 1 値だけ 30〜50 のレンジを持つ
        // (†ヴァイオレットペインの突き欄は wiki が 255 = 上限値の書き間違いなので 25 を採る)
        effect_trigger_3_ranged(
            "rinrin-tidal-wave",
            "†タイダルウェイブ",
            v(30, 25, 25, 25, 25, 25, 25, 25, 25),
            v(50, 25, 25, 25, 25, 25, 25, 25, 25),
            v(500, 255, 255, 255, 255, 255, 255, 255, 255),
        ),
        effect_trigger_3_ranged(
            "rinrin-heavenly-wing-sword",
            "†天翔光翼剣",
            v(25, 30, 25, 25, 25, 25, 25, 25, 25),
            v(25, 50, 25, 25, 25, 25, 25, 25, 25),
            v(255, 500, 255, 255, 255, 255, 255, 255, 255),
        ),
        effect_trigger_3_ranged(
            "rinrin-violet-pain",
            "†ヴァイオレットペイン",
            v(25, 25, 25, 30, 25, 25, 25, 25, 25),
            v(25, 25, 25, 50, 25, 25, 25, 25, 25),
            v(255, 255, 255, 500, 255, 255, 255, 255, 255),
        ),
        effect_trigger_3_ranged(
            "rinrin-crimson-flare",
            "†クリムゾンフレア",
            v(25, 25, 25, 25, 30, 25, 25, 25, 25),
            v(25, 25, 25, 25, 50, 25, 25, 25, 25),
            v(255, 255, 255, 255, 500, 255, 255, 255, 255),
        ),
        // 「装着時：与ダメージ+1%」(確率ではない)
        effect_item(
            "logh-lost",
            "†ロスト",
            v(22, 22, 22, 22, 22, 22, 22, 22, 22),
            v(255, 255, 255, 255, 255, 255, 255, 255, 255),
            ITEM_DAMAGE_JAPAN_1,
        ),
        // ── 効果: 21st メモリアル。9補正と総上限が全て確定している現行上位品 ──
        effect_item("star-sharp-circle", "†スターシャープサークル",
            v(30, 5, 25, 25, 25, 25, 25, 25, 25),
            v(600, 400, 255, 255, 255, 255, 255, 255, 255), &[]),
        effect_item("star-slash-circle", "†スタースラッシュサークル",
            v(5, 30, 25, 25, 25, 25, 25, 25, 25),
            v(400, 600, 255, 255, 255, 255, 255, 255, 255), &[]),
        effect_item("star-magic-circle", "†スターマジックサークル",
            v(25, 5, 25, 30, 25, 25, 25, 25, 25),
            v(255, 400, 255, 600, 255, 255, 255, 255, 255), &[]),
        effect_item("star-holy-circle", "†スターホーリーサークル",
            v(25, 25, 25, 5, 30, 25, 25, 25, 25),
            v(255, 255, 255, 400, 600, 255, 255, 255, 255), &[]),

        // ── 効果: 22nd メモリアル。主能力700は公式、その他はユーザー確定の255 ──
        stallion_effect("stallion-sign-blue", "†スタリオンサイン-ブルー",
            v(30, 5, 5, 5, 5, 35, 35, 35, 35),
            v(700, 255, 255, 255, 255, 255, 255, 255, 255)),
        stallion_effect("stallion-sign-green", "†スタリオンサイン-グリーン",
            v(5, 30, 5, 5, 5, 35, 35, 35, 35),
            v(255, 700, 255, 255, 255, 255, 255, 255, 255)),
        stallion_effect("stallion-sign-purple", "†スタリオンサイン-パープル",
            v(5, 5, 5, 30, 5, 35, 35, 35, 35),
            v(255, 255, 255, 700, 255, 255, 255, 255, 255)),
        stallion_effect("stallion-sign-yellow", "†スタリオンサイン-イエロー",
            v(5, 5, 5, 5, 30, 35, 35, 35, 35),
            v(255, 255, 255, 255, 700, 255, 255, 255, 255)),

        // ── AF: 依存別に実用候補を揃える。確率効果は従来方針どおり発動前提 ──
        WikiEquipmentItem {
            id: "eclipse-stab-def", slot: PartSlot::Artifact,
            name: "†エクリプスの突力 - ディフェンシオ",
            values_min: v(170, 0, 20, 0, 25, 25, 25, 25, 25),
            values_max: v(190, 0, 30, 0, 35, 35, 35, 35, 35), growth_cap: None,
            enchant_total_caps: v(220, 0, 50, 0, 55, 55, 55, 55, 55),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。突き依存+30%は同系列規則から補完" },
        },
        WikiEquipmentItem {
            id: "eclipse-hack-def", slot: PartSlot::Artifact,
            name: "†エクリプスの斬力 - ディフェンシオ",
            values_min: v(0, 170, 25, 0, 25, 25, 25, 25, 25),
            values_max: v(0, 190, 35, 0, 35, 35, 35, 35, 35), growth_cap: None,
            enchant_total_caps: v(0, 220, 55, 0, 55, 55, 55, 55, 55),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。斬り攻撃ダメージ+30%" },
        },
        WikiEquipmentItem {
            id: "eclipse-int", slot: PartSlot::Artifact, name: "†エクリプスの魔力",
            values_min: v(0, 0, 20, 150, 20, 20, 20, 20, 20),
            values_max: v(0, 0, 30, 170, 30, 30, 30, 30, 30), growth_cap: None,
            enchant_total_caps: v(0, 0, 50, 200, 50, 50, 50, 50, 50),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。魔法攻撃ダメージ+30%" },
        },
        WikiEquipmentItem {
            id: "eclipse-mr-def", slot: PartSlot::Artifact,
            name: "†エクリプスの魔防力 - ディフェンシオ",
            values_min: v(0, 0, 25, 25, 170, 25, 25, 25, 25),
            values_max: v(0, 0, 35, 35, 190, 35, 35, 35, 35), growth_cap: None,
            enchant_total_caps: v(0, 0, 55, 55, 220, 55, 55, 55, 55),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。MR系攻撃ダメージ+30%" },
        },
        WikiEquipmentItem {
            id: "dungeon-meshi-picking-tools", slot: PartSlot::Artifact, name: "†ピッキングツール",
            values_min: v(115, 0, 30, 0, 20, 18, 15, 15, 18),
            values_max: v(135, 0, 30, 0, 30, 25, 20, 20, 25), growth_cap: None,
            enchant_total_caps: v(170, 0, 30, 0, 30, 25, 25, 25, 25),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ダンジョン飯タイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "dungeon-meshi-gourmet-guide", slot: PartSlot::Artifact, name: "†迷宮グルメガイド",
            values_min: v(0, 115, 30, 0, 20, 18, 15, 15, 18),
            values_max: v(0, 135, 30, 0, 30, 25, 20, 20, 25), growth_cap: None,
            enchant_total_caps: v(0, 170, 30, 0, 30, 25, 25, 25, 25),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ダンジョン飯タイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "dungeon-meshi-thistle-book", slot: PartSlot::Artifact, name: "†シスルの魔術書",
            values_min: v(0, 0, 30, 115, 30, 15, 18, 18, 15),
            values_max: v(0, 0, 30, 135, 30, 20, 25, 25, 20), growth_cap: None,
            enchant_total_caps: v(0, 0, 30, 170, 30, 25, 25, 25, 25),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ダンジョン飯タイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "maid-dragon-magic-orb", slot: PartSlot::Artifact, name: "†魔力の玉",
            values_min: v(0, 0, 30, 90, 90, 39, 19, 44, 33),
            values_max: v(0, 0, 30, 103, 103, 39, 19, 44, 33), growth_cap: None,
            enchant_total_caps: v(0, 0, 30, 130, 130, 39, 19, 44, 33),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "メイドラゴンタイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "log-horizon-akatsuki-doll", slot: PartSlot::Artifact, name: "†アカツキ人形",
            values_min: v(90, 90, 30, 0, 30, 23, 23, 23, 23),
            values_max: v(103, 103, 50, 0, 50, 25, 25, 25, 25), growth_cap: None,
            enchant_total_caps: v(130, 130, 70, 0, 70, 49, 49, 49, 49),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ログ・ホライズンタイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "arklon-hack-int", slot: PartSlot::Artifact, name: "†アークロンの魔斬力",
            values_min: v(0, 80, 18, 80, 18, 13, 13, 23, 13),
            values_max: v(0, 100, 21, 100, 21, 14, 15, 25, 14), growth_cap: None,
            enchant_total_caps: v(0, 130, 45, 130, 45, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。魔法斬り攻撃ダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "arklon-physical-def", slot: PartSlot::Artifact,
            name: "†アークロンの物理力 - ディフェンシオ",
            values_min: v(80, 80, 22, 0, 22, 13, 13, 23, 13),
            values_max: v(100, 100, 25, 0, 25, 14, 15, 25, 14), growth_cap: None,
            // Wikiの同一補正行の欠落セルは、数値が同じリストア/スピーディーの上限を採用。
            enchant_total_caps: v(130, 130, 49, 0, 49, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。物理複合攻撃ダメージ+20%、ディフェンシオ。上限の欠落セルは同補正のリストア/スピーディーと一致" },
        },
        WikiEquipmentItem {
            id: "arklon-int-def", slot: PartSlot::Artifact,
            name: "†アークロンの魔力 - ディフェンシオ",
            values_min: v(0, 0, 22, 110, 24, 13, 13, 23, 13),
            values_max: v(0, 0, 25, 130, 27, 14, 15, 25, 14), growth_cap: None,
            enchant_total_caps: v(0, 0, 49, 160, 51, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。魔法攻撃ダメージ+20%、ディフェンシオ" },
        },
        WikiEquipmentItem {
            id: "arklon-hack-int-def", slot: PartSlot::Artifact,
            name: "†アークロンの魔斬力 - ディフェンシオ",
            values_min: v(0, 90, 22, 90, 22, 13, 13, 23, 13),
            values_max: v(0, 110, 25, 110, 25, 14, 15, 25, 14), growth_cap: None,
            enchant_total_caps: v(0, 140, 49, 140, 49, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。魔法斬り攻撃ダメージ+20%、ディフェンシオ" },
        },

        // ── AF: プシーキー / エクリプス / エーテリアルの6依存×通常・ディフェンシオ ──
        artifact_item("psyche-stab", "†プシーキーの突力",
            v(63, 0, 14, 0, 14, 13, 13, 23, 13), v(66, 0, 17, 0, 17, 14, 15, 25, 14),
            v(90, 0, 41, 0, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。突き攻撃ダメージ+20%"),
        artifact_item("psyche-hack", "†プシーキーの斬力",
            v(0, 63, 14, 0, 14, 13, 13, 23, 13), v(0, 66, 17, 0, 17, 14, 15, 25, 14),
            v(0, 90, 41, 0, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。斬り攻撃ダメージ+20%"),
        artifact_item("psyche-physical", "†プシーキーの物理力",
            v(41, 41, 14, 0, 14, 13, 13, 23, 13), v(44, 44, 17, 0, 17, 14, 15, 25, 14),
            v(68, 68, 41, 0, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。物理複合攻撃ダメージ+20%"),
        artifact_item("psyche-int", "†プシーキーの魔力",
            v(0, 0, 14, 63, 16, 13, 13, 23, 13), v(0, 0, 17, 66, 19, 14, 15, 25, 14),
            v(0, 0, 41, 90, 43, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。魔法攻撃ダメージ+20%"),
        artifact_item("psyche-mr", "†プシーキーの魔防力",
            v(0, 0, 14, 19, 63, 13, 13, 23, 13), v(0, 0, 17, 22, 66, 14, 15, 25, 14),
            v(0, 0, 41, 46, 90, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。MR系攻撃ダメージ+20%"),
        artifact_item("psyche-hack-int", "†プシーキーの魔斬力",
            v(0, 53, 14, 53, 14, 13, 13, 23, 13), v(0, 58, 17, 58, 17, 14, 15, 25, 14),
            v(0, 82, 41, 82, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。魔法斬り攻撃ダメージ+20%"),

        artifact_item("eclipse-stab", "†エクリプスの突力",
            v(150, 0, 20, 0, 20, 20, 20, 20, 20), v(170, 0, 30, 0, 30, 30, 30, 30, 30),
            v(200, 0, 50, 0, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。突き依存+30%は同系列規則から補完"),
        artifact_item("eclipse-hack", "†エクリプスの斬力",
            v(0, 150, 20, 0, 20, 20, 20, 20, 20), v(0, 170, 30, 0, 30, 30, 30, 30, 30),
            v(0, 200, 50, 0, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。斬り攻撃ダメージ+30%"),
        artifact_item("eclipse-physical", "†エクリプスの物理力",
            v(120, 120, 20, 0, 20, 20, 20, 20, 20), v(140, 140, 30, 0, 30, 30, 30, 30, 30),
            v(170, 170, 50, 0, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。上限と物理複合依存+30%は同系列規則から補完"),
        artifact_item("eclipse-mr", "†エクリプスの魔防力",
            v(0, 0, 20, 20, 150, 20, 20, 20, 20), v(0, 0, 30, 30, 170, 30, 30, 30, 30),
            v(0, 0, 50, 50, 200, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。MR系攻撃ダメージ+30%"),
        artifact_item("eclipse-hack-int", "†エクリプスの魔斬力",
            v(0, 130, 20, 130, 20, 20, 20, 20, 20), v(0, 150, 30, 150, 30, 30, 30, 30, 30),
            v(0, 180, 50, 180, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。上限と魔斬依存+30%は同系列規則から補完"),

        artifact_item("ethereal-stab", "†エーテリアルチューブ(突力)",
            v(210, 0, 30, 0, 30, 30, 30, 30, 30), v(230, 0, 40, 0, 40, 40, 40, 40, 40),
            v(260, 0, 60, 0, 60, 60, 60, 60, 60), &[],
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-hack", "†エーテリアルチューブ(斬力)",
            v(0, 210, 30, 0, 30, 30, 30, 30, 30), v(0, 230, 40, 0, 40, 40, 40, 40, 40),
            v(0, 260, 60, 0, 60, 60, 60, 60, 60), &[],
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-physical", "†エーテリアルチューブ(物理力)",
            v(190, 190, 30, 0, 30, 30, 30, 30, 30), v(210, 210, 40, 0, 40, 40, 40, 40, 40),
            v(240, 240, 60, 0, 60, 60, 60, 60, 60), &[],
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-int", "†エーテリアルチューブ(魔力)",
            v(0, 0, 30, 210, 30, 30, 30, 30, 30), v(0, 0, 40, 230, 40, 40, 40, 40, 40),
            v(0, 0, 60, 260, 60, 60, 60, 60, 60), &[],
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-mr", "†エーテリアルチューブ(魔防力)",
            v(0, 0, 30, 30, 210, 30, 30, 30, 30), v(0, 0, 40, 40, 230, 40, 40, 40, 40),
            v(0, 0, 60, 60, 260, 60, 60, 60, 60), &[],
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-hack-int", "†エーテリアルチューブ(魔斬力)",
            v(0, 190, 30, 190, 30, 30, 30, 30, 30), v(0, 210, 40, 210, 40, 40, 40, 40, 40),
            v(0, 240, 60, 240, 60, 60, 60, 60, 60), &[],
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),

        defensio_artifact("psyche-stab-def", "†プシーキーの突力 - ディフェンシオ",
            v(69, 0, 20, 0, 20, 16, 16, 26, 16), v(72, 0, 23, 0, 23, 17, 18, 28, 17),
            v(96, 0, 47, 0, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。突き攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-hack-def", "†プシーキーの斬力 - ディフェンシオ",
            v(0, 69, 20, 0, 20, 16, 16, 26, 16), v(0, 72, 23, 0, 23, 17, 18, 28, 17),
            v(0, 96, 47, 0, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。斬り攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-physical-def", "†プシーキーの物理力 - ディフェンシオ",
            v(47, 47, 20, 0, 20, 16, 16, 26, 16), v(50, 50, 23, 0, 23, 17, 18, 28, 17),
            v(74, 74, 47, 0, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。物理複合攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-int-def", "†プシーキーの魔力 - ディフェンシオ",
            v(0, 0, 20, 69, 22, 16, 16, 26, 16), v(0, 0, 23, 72, 25, 17, 18, 28, 17),
            v(0, 0, 47, 96, 49, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。魔法攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-mr-def", "†プシーキーの魔防力 - ディフェンシオ",
            v(0, 0, 20, 25, 69, 16, 16, 26, 16), v(0, 0, 23, 28, 72, 17, 18, 28, 17),
            v(0, 0, 47, 52, 96, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。MR系攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-hack-int-def", "†プシーキーの魔斬力 - ディフェンシオ",
            v(0, 61, 20, 61, 20, 16, 16, 26, 16), v(0, 64, 23, 64, 23, 17, 18, 28, 17),
            v(0, 88, 47, 88, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            "リンゴの島。魔法斬り攻撃ダメージ+20%、ディフェンシオ"),

        defensio_artifact("eclipse-physical-def", "†エクリプスの物理力 - ディフェンシオ",
            v(140, 140, 25, 0, 25, 25, 25, 25, 25), v(160, 160, 35, 0, 35, 35, 35, 35, 35),
            v(190, 190, 55, 0, 55, 55, 55, 55, 55), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。補正と物理複合依存+30%は同系列規則から補完"),
        defensio_artifact("eclipse-int-def", "†エクリプスの魔力 - ディフェンシオ",
            v(0, 0, 25, 170, 25, 25, 25, 25, 25), v(0, 0, 35, 190, 35, 35, 35, 35, 35),
            v(0, 0, 55, 220, 55, 55, 55, 55, 55), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。副補正上限と魔攻依存+30%は同系列規則から補完"),
        defensio_artifact("eclipse-hack-int-def", "†エクリプスの魔斬力 - ディフェンシオ",
            v(0, 150, 25, 150, 25, 25, 25, 25, 25), v(0, 170, 35, 170, 35, 35, 35, 35, 35),
            v(0, 200, 55, 200, 55, 55, 55, 55, 55), ITEM_DAMAGE_DEPENDENCY_30,
            "喪失の島。補正と魔斬依存+30%は同系列規則から補完"),

        defensio_artifact("ethereal-stab-def", "†エーテリアルチューブ(突力) - ディフェンシオ",
            v(230, 0, 35, 0, 35, 35, 35, 35, 35), v(250, 0, 45, 0, 45, 45, 45, 45, 45),
            v(280, 0, 65, 0, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。突き依存+35%"),
        defensio_artifact("ethereal-hack-def", "†エーテリアルチューブ(斬力) - ディフェンシオ",
            v(0, 230, 35, 0, 35, 35, 35, 35, 35), v(0, 250, 45, 0, 45, 45, 45, 45, 45),
            v(0, 280, 65, 0, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。斬り依存+35%"),
        defensio_artifact("ethereal-physical-def", "†エーテリアルチューブ(物理力) - ディフェンシオ",
            v(210, 210, 35, 0, 35, 35, 35, 35, 35), v(230, 230, 45, 0, 45, 45, 45, 45, 45),
            v(260, 260, 65, 0, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。物理複合依存+35%"),
        defensio_artifact("ethereal-int-def", "†エーテリアルチューブ(魔力) - ディフェンシオ",
            v(0, 0, 35, 230, 35, 35, 35, 35, 35), v(0, 0, 45, 250, 45, 45, 45, 45, 45),
            v(0, 0, 65, 280, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            "ゆがんだ村。Wiki確定補正。魔攻依存+35%"),
        defensio_artifact("ethereal-mr-def", "†エーテリアルチューブ(魔防力) - ディフェンシオ",
            v(0, 0, 35, 35, 230, 35, 35, 35, 35), v(0, 0, 45, 45, 250, 45, 45, 45, 45),
            v(0, 0, 65, 65, 280, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。魔防依存+35%"),
        defensio_artifact("ethereal-hack-int-def", "†エーテリアルチューブ(魔斬力) - ディフェンシオ",
            v(0, 210, 35, 210, 35, 35, 35, 35, 35), v(0, 230, 45, 230, 45, 45, 45, 45, 45),
            v(0, 260, 65, 260, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。魔斬依存+35%"),

        // ── レリック: 直前段階の完成値から、選択段階の上限までランダム成長 ──
        relic_item("godbird-pendant-plus1", "†神鳥のペンダント(+1)", PartSlot::RelicPendant, 0, 0, 30, 25),
        relic_item("godbird-pendant-plus2", "†神鳥のペンダント(+2)", PartSlot::RelicPendant, 30, 25, 50, 45),
        relic_item("godbird-pendant-plus3", "†神鳥のペンダント(+3)", PartSlot::RelicPendant, 50, 45, 55, 50),
        relic_item("godbird-pendant-plus4", "†神鳥のペンダント(+4)", PartSlot::RelicPendant, 55, 50, 60, 60),
        relic_item("godbird-pendant-plus5", "†神鳥のペンダント(+5)", PartSlot::RelicPendant, 60, 60, 65, 65),
        relic_item("godbird-pendant-plus6", "†神鳥のペンダント(+6)", PartSlot::RelicPendant, 65, 65, 70, 70),
        relic_item("godbird-pendant-plus7", "†神鳥のペンダント(+7)", PartSlot::RelicPendant, 70, 70, 75, 75),
        relic_item("godbird-pendant-plus8", "†神鳥のペンダント(+8)", PartSlot::RelicPendant, 75, 75, 80, 80),
        relic_item("godbird-pendant-plus9", "†神鳥のペンダント(+9)", PartSlot::RelicPendant, 80, 80, 90, 90),
        relic_item("godbird-pendant-plus10", "†神鳥のペンダント(+10)", PartSlot::RelicPendant, 90, 90, 100, 100),
        relic_item("lunaria-pendant-plus1", "†ルナリアペンダント(+1)", PartSlot::RelicPendant, 100, 100, 110, 110),
        relic_item("lunaria-pendant-plus2", "†ルナリアペンダント(+2)", PartSlot::RelicPendant, 110, 110, 120, 120),
        relic_item("lunaria-pendant-plus3", "†ルナリアペンダント(+3)", PartSlot::RelicPendant, 120, 120, 130, 130),
        relic_item("lunaria-pendant-plus4", "†ルナリアペンダント(+4)", PartSlot::RelicPendant, 130, 130, 140, 140),
        relic_item("lunaria-pendant-plus5", "†ルナリアペンダント(+5)", PartSlot::RelicPendant, 140, 140, 150, 150),
        relic_item("lunaria-pendant-plus6", "†ルナリアペンダント(+6)", PartSlot::RelicPendant, 150, 150, 160, 160),
        relic_item("lunaria-pendant-plus7", "†ルナリアペンダント(+7)", PartSlot::RelicPendant, 160, 160, 170, 170),
        relic_item("lunaria-pendant-plus8", "†ルナリアペンダント(+8)", PartSlot::RelicPendant, 170, 170, 180, 180),
        relic_item("lunaria-pendant-plus9", "†ルナリアペンダント(+9)", PartSlot::RelicPendant, 180, 180, 190, 190),
        relic_item("lunaria-pendant-plus10", "†ルナリアペンダント(+10)", PartSlot::RelicPendant, 190, 190, 200, 200),

        relic_item("godbird-bracelet-plus1", "†神鳥のブレスレット(+1)", PartSlot::RelicBracelet, 0, 0, 30, 25),
        relic_item("godbird-bracelet-plus2", "†神鳥のブレスレット(+2)", PartSlot::RelicBracelet, 30, 25, 50, 45),
        relic_item("godbird-bracelet-plus3", "†神鳥のブレスレット(+3)", PartSlot::RelicBracelet, 50, 45, 55, 50),
        relic_item("godbird-bracelet-plus4", "†神鳥のブレスレット(+4)", PartSlot::RelicBracelet, 55, 50, 60, 60),
        relic_item("godbird-bracelet-plus5", "†神鳥のブレスレット(+5)", PartSlot::RelicBracelet, 60, 60, 65, 65),
        relic_item("godbird-bracelet-plus6", "†神鳥のブレスレット(+6)", PartSlot::RelicBracelet, 65, 65, 70, 70),
        relic_item("godbird-bracelet-plus7", "†神鳥のブレスレット(+7)", PartSlot::RelicBracelet, 70, 70, 75, 75),
        relic_item("godbird-bracelet-plus8", "†神鳥のブレスレット(+8)", PartSlot::RelicBracelet, 75, 75, 80, 80),
        relic_item("godbird-bracelet-plus9", "†神鳥のブレスレット(+9)", PartSlot::RelicBracelet, 80, 80, 90, 90),
        relic_item("godbird-bracelet-plus10", "†神鳥のブレスレット(+10)", PartSlot::RelicBracelet, 90, 90, 100, 100),
        relic_item("lunaria-bracelet-plus1", "†ルナリアブレスレット(+1)", PartSlot::RelicBracelet, 100, 100, 110, 110),
        relic_item("lunaria-bracelet-plus2", "†ルナリアブレスレット(+2)", PartSlot::RelicBracelet, 110, 110, 120, 120),
        relic_item("lunaria-bracelet-plus3", "†ルナリアブレスレット(+3)", PartSlot::RelicBracelet, 120, 120, 130, 130),
        relic_item("lunaria-bracelet-plus4", "†ルナリアブレスレット(+4)", PartSlot::RelicBracelet, 130, 130, 140, 140),
        relic_item("lunaria-bracelet-plus5", "†ルナリアブレスレット(+5)", PartSlot::RelicBracelet, 140, 140, 150, 150),
        relic_item("lunaria-bracelet-plus6", "†ルナリアブレスレット(+6)", PartSlot::RelicBracelet, 150, 150, 160, 160),
        relic_item("lunaria-bracelet-plus7", "†ルナリアブレスレット(+7)", PartSlot::RelicBracelet, 160, 160, 170, 170),
        relic_item("lunaria-bracelet-plus8", "†ルナリアブレスレット(+8)", PartSlot::RelicBracelet, 170, 170, 180, 180),
        relic_item("lunaria-bracelet-plus9", "†ルナリアブレスレット(+9)", PartSlot::RelicBracelet, 180, 180, 190, 190),
        relic_item("lunaria-bracelet-plus10", "†ルナリアブレスレット(+10)", PartSlot::RelicBracelet, 190, 190, 200, 200),
    ];

    // 盾+ は通常の候補を並べず、ユーザー指定の成長カフスだけを扱う。
    catalog.retain(|item| item.slot != PartSlot::ShieldPlus);
    catalog.push(WikiEquipmentItem {
        id: "rising-holic-cuffs",
        slot: PartSlot::ShieldPlus,
        name: "†ライジングホリックカフス",
        values_min: v(140, 140, 140, 140, 140, 140, 140, 140, 140),
        values_max: v(140, 140, 140, 140, 140, 140, 140, 140, 140),
        growth_cap: Some(200),
        enchant_total_caps: EquipmentValues::default(),
        weapon_class: None,
        enhance_type: None,
        damage_effects: &[],
        source: Source {
            page: "Item/防具/腕/盾＋",
            retrieved_on: "2026-08-27",
            note: "成長コンテンツ。初期入力は全補正140、成長上限は全補正200。表示名はユーザー指定",
        },
    });

    // 既存の手検証済みデータ(装着時効果を含む)を優先し、同名の自動抽出行は足さない。
    for item in equipment_catalog_generated::wiki_equipment_catalog() {
        if !catalog.iter().any(|existing| existing.name == item.name) {
            catalog.push(item);
        }
    }
    // 日本 Tale Wiki で数値が確定している同名行を優先し、未収録のセイクリッド装備だけを
    // 韓国コミュニティ資料から補完する。
    for item in equipment_catalog_sacred_kr::sacred_equipment_catalog() {
        if !catalog.iter().any(|existing| existing.name == item.name) {
            catalog.push(item);
        }
    }
    catalog
        .into_iter()
        .map(WikiEquipmentItem::into_item)
        .collect()
}

pub fn find_equipment_item(id: &str) -> Option<EquipmentItem> {
    equipment_catalog().into_iter().find(|item| item.id == id)
}

/// キャラ固有パッシブにより、腕装備の補正から「基本能力値」へ派生する装備補正。
///
/// 元の `base` / `enchant` は変更しない。ボリス・マキシミンはエンチャント分も
/// 派生先では基本補正として扱う。バンド系も同様に、バンドの表示補正合計を参照して
/// 0.7 倍(小数点以下切り捨て)した値を基本補正へ足す。
pub fn character_wrist_base_bonus(
    game_character_id: &str,
    base_stats: &BaseStats,
    style_dependency: SkillDependency,
    equipment: &Equipment,
    catalog: &[EquipmentItem],
) -> EquipmentValues {
    let Some(wrist) = equipment.parts.shield.selected() else {
        return EquipmentValues::default();
    };

    if matches!(game_character_id, "boris" | "maximin") {
        let siena_thrust = equipment
            .siena
            .shield
            .selected()
            .map(|entry| entry.aura.values().thrust)
            .unwrap_or(0);
        return EquipmentValues {
            magic_attack: wrist.base.thrust + wrist.enchant.thrust + siena_thrust,
            ..Default::default()
        };
    }

    let is_band = wrist
        .item_id
        .as_deref()
        .and_then(|id| catalog.iter().find(|item| item.id == id))
        .is_some_and(|item| item.wrist_type == Some(WristType::Band));
    if !is_band {
        return EquipmentValues::default();
    }
    let bonus = (wrist.base.agility + wrist.enchant.agility) * 7 / 10;
    let mut values = EquipmentValues::default();
    match game_character_id {
        "nayatorei" | "isaac" => match style_dependency {
            SkillDependency::Stab | SkillDependency::StabHack => values.thrust = bonus,
            SkillDependency::Hack => values.slash = bonus,
            _ => {}
        },
        "mira" => values.slash = bonus,
        "benya" if base_stats.hack > base_stats.mr => values.slash = bonus,
        "benya" if base_stats.hack < base_stats.mr => values.magic_defense = bonus,
        "roamini" => values.magic_attack = bonus,
        _ => {}
    }
    values
}

/// 装備しているアイテムそのものの装着時効果を、与ダメージ式のカテゴリ寄与に変換する。
/// 装備補正値は `Equipment::base_totals` が別に見る。
///
/// `Equipment::ability_damage_contributions` と同じ役割だが、`EquipmentItem` は
/// `Source` / `WeaponClass` を持つので domain ではなくこちら側にある。
pub fn item_damage_contributions(
    equipment: &Equipment,
    dependency: SkillDependency,
) -> Vec<(DamageCategory, f64)> {
    let catalog = equipment_catalog();
    let effects: Vec<&'static SkillEffect> = equipment
        .parts
        .iter()
        .into_iter()
        .filter_map(|(_, part)| part.item_id.as_deref())
        .filter_map(|id| catalog.iter().find(|item| item.id == id))
        .filter(|item| item.damage_dependency.is_none_or(|required| required == dependency))
        .flat_map(|item| item.damage_effects.iter())
        .collect();
    domain::damage_contributions(effects.into_iter())
}

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
        (PartSlot::ShieldPlus, EquipmentAbilityFamily::MagicResistance) => Some((MagicDefense, 7, 15)),
        (PartSlot::RelicPendant, EquipmentAbilityFamily::PointedBlade) => Some((Thrust, 1, 15)),
        (PartSlot::RelicPendant, EquipmentAbilityFamily::SharpBlade) => Some((Slash, 1, 15)),
        (PartSlot::RelicPendant, EquipmentAbilityFamily::Intelligence) => Some((MagicAttack, 1, 15)),
        (PartSlot::RelicPendant, EquipmentAbilityFamily::MagicResistance) => Some((MagicDefense, 1, 15)),
        (PartSlot::RelicBracelet, EquipmentAbilityFamily::Accuracy) => Some((Accuracy, 1, 13)),
        (PartSlot::RelicBracelet, EquipmentAbilityFamily::Evasion) => Some((Evasion, 1, 13)),
        (PartSlot::RelicBracelet, EquipmentAbilityFamily::Critical) => Some((Critical, 1, 12)),
        _ => None,
    }
    .map(|(kind, min, max)| EquipmentAbilityAdditionalDef { kind, min, max });
    let (additional_slots, additional_effects, additional_options) = match slot {
        PartSlot::Armor => {
            let mut options = vec![
                option(DamageResistance, 11, 11),
                option(HpRecovery, 8, 18),
                option(MpRecovery, 8, 18),
            ];
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
            (2, "ランダム追加2枠", vec![
                option(DamageResistance, 10, 10), option(reduction, 100, 100),
                option(HpRecovery, 8, 18), option(MpRecovery, 8, 18),
                option(SpRecovery, 8, 18), option(EvasionRate, 10, 18),
            ])
        }
        PartSlot::ShieldPlus => {
            let options = if matches!(family, EquipmentAbilityFamily::Accuracy | EquipmentAbilityFamily::Evasion) {
                vec![
                    option(DamageResistance, 5, 6), option(PhysicalDamageReduction, 90, 100),
                    option(MagicDamageReduction, 90, 100), option(HpRecovery, 15, 20),
                    option(MpRecovery, 15, 20), option(SpRecovery, 15, 20), option(Critical, 5, 10),
                ]
            } else {
                vec![option(FireElement, 10, 30), option(WaterElement, 10, 30),
                    option(WindElement, 10, 30), option(EarthElement, 10, 30),
                    option(LightningElement, 10, 30), option(WhiteElement, 10, 30), option(DarkElement, 10, 30)]
            };
            (1, "ランダム追加1枠", options)
        }
        PartSlot::Hand => (2, "ランダム追加2枠", vec![
            option(FixedDamage, 10_000, 10_000), option(DamageRate, 9, 9),
            option(Thrust, 8, 14), option(Slash, 8, 14),
            option(MagicAttack, 8, 14), option(MagicDefense, 8, 14),
        ]),
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
            (1, "ランダム追加1枠", kinds.into_iter().map(|kind| option(kind, 20, 20)).collect())
        }
        PartSlot::Leg => (2, "ランダム追加2枠", vec![
            option(HpRecovery, 8, 18), option(MpRecovery, 8, 18),
            option(SpRecovery, 8, 18), option(EvasionRate, 7, 15),
        ]),
        PartSlot::RelicPendant => (1, "ランダム追加1枠", vec![
            option(FireElement, 20, 30), option(WaterElement, 20, 30),
            option(WindElement, 20, 30), option(EarthElement, 20, 30),
            option(LightningElement, 20, 30), option(WhiteElement, 20, 30),
            option(DarkElement, 20, 30), option(DamageRate, 5, 10),
        ]),
        PartSlot::RelicBracelet => (1, "ランダム追加1枠", vec![
            option(DamageResistance, 5, 10), option(PhysicalDamageReduction, 90, 100),
            option(MagicDamageReduction, 90, 100), option(HpRecovery, 15, 20),
            option(MpRecovery, 15, 20), option(SpRecovery, 15, 20),
        ]),
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
    let mut def = slot_ability(id, name, slot, family, group, values, effect_summary, false, &[]);
    def.category = category;
    def.additional_slots = 0;
    def.additional_effects = "";
    def.additional_options.clear();
    def
}

const HELM_SKILL_10: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::SkillMultiplierFixed,
    percent: 10.0,
}];

/// 武器アビリティは装備攻撃力(突き/斬り/魔攻/魔防)にしか効かない。
fn a(thrust: i64, slash: i64, magic_attack: i64, magic_defense: i64) -> EquipmentValues {
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
        "helm-e-skill-attack", "E-スキル攻撃力増加", PartSlot::Helm,
        EquipmentAbilityFamily::SkillAttack, "helm-skill-attack", EquipmentValues::default(),
        "スキル攻撃力 +10", false, HELM_SKILL_10,
    ));

    for (id, name, family, values, summary) in [
        ("upper-armor-polish", "(上)鎧研磨", EquipmentAbilityFamily::ArmorPolish, EquipmentValues { physical_defense: 40, ..EquipmentValues::default() }, "物防 +40"),
        ("upper-magic-resistance-armor", "(上)魔法耐性・鎧", EquipmentAbilityFamily::MagicResistance, EquipmentValues { magic_defense: 30, ..EquipmentValues::default() }, "魔防 +30"),
        ("upper-evasion-armor", "(上)機敏", EquipmentAbilityFamily::Evasion, EquipmentValues { evasion: 3, ..EquipmentValues::default() }, "回避 +3"),
    ] {
        out.push(fixed_slot_ability(id, name, PartSlot::Armor, family, 2, "armor-category-2", values, summary));
    }

    for (id, name, family, values, summary, record_only) in [
        ("night-star-vitality-armor", "夜星の生命力", EquipmentAbilityFamily::Vitality, EquipmentValues::default(), "最大HP +30,000", true),
        ("night-star-mana-armor", "夜星のマナ", EquipmentAbilityFamily::Mana, EquipmentValues::default(), "最大MP +9,000", true),
        ("night-star-armor-polish", "夜星の鎧研磨", EquipmentAbilityFamily::ArmorPolish, EquipmentValues { physical_defense: 60, ..EquipmentValues::default() }, "物防 +60", false),
        ("night-star-magic-resistance-armor", "夜星の魔法耐性(鎧)", EquipmentAbilityFamily::MagicResistance, EquipmentValues { magic_defense: 60, ..EquipmentValues::default() }, "魔防 +60", false),
        ("night-star-evasion-armor", "夜星の機敏", EquipmentAbilityFamily::Evasion, EquipmentValues { evasion: 16, ..EquipmentValues::default() }, "回避 +16", false),
    ] {
        out.push(slot_ability(id, name, PartSlot::Armor, family, "armor-ability", values, summary, record_only, &[]));
    }

    for (id, name, family, values, summary) in [
        ("night-star-shield-polish", "夜星の盾研磨", EquipmentAbilityFamily::ShieldPolish, EquipmentValues { physical_defense: 30, ..EquipmentValues::default() }, "物防 +30"),
        ("night-star-magic-resistance-shield", "夜星の魔法耐性(盾)", EquipmentAbilityFamily::MagicResistance, EquipmentValues { magic_defense: 15, ..EquipmentValues::default() }, "魔防 +15"),
    ] {
        out.push(slot_ability(id, name, PartSlot::Shield, family, "shield-ability", values, summary, false, &[]));
    }

    for (id, name, family, summary) in [
        ("mystic-mine-accuracy", "神秘鉱の的中剣", EquipmentAbilityFamily::Accuracy, "命中 +7〜13"),
        ("mystic-mine-evasion", "神秘鉱の機敏", EquipmentAbilityFamily::Evasion, "回避 +7〜13"),
        ("mystic-mine-pointed-blade", "神秘鉱の尖った刃", EquipmentAbilityFamily::PointedBlade, "突き +7〜15"),
        ("mystic-mine-sharp-blade", "神秘鉱の鋭い刃", EquipmentAbilityFamily::SharpBlade, "斬り +7〜15"),
        ("mystic-mine-intelligence", "神秘鉱の知力", EquipmentAbilityFamily::Intelligence, "魔攻 +7〜15"),
        ("mystic-mine-magic-resistance", "神秘鉱の耐魔力", EquipmentAbilityFamily::MagicResistance, "魔防 +7〜15"),
    ] {
        out.push(slot_ability(id, name, PartSlot::ShieldPlus, family, id, EquipmentValues::default(), summary, false, &[]));
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
        out.push(slot_ability(id, name, PartSlot::Head, EquipmentAbilityFamily::Element, "head-element", EquipmentValues::default(), summary, true, &[]));
    }

    for (id, name, family, values, summary) in [
        ("night-star-critical-hand", "夜星の致命打", EquipmentAbilityFamily::Critical, EquipmentValues { critical: 15, ..EquipmentValues::default() }, "クリティカル +15"),
        ("night-star-accuracy-hand", "夜星の的中剣", EquipmentAbilityFamily::Accuracy, EquipmentValues { accuracy: 16, ..EquipmentValues::default() }, "命中 +16"),
    ] {
        out.push(slot_ability(id, name, PartSlot::Hand, family, "hand-ability", values, summary, false, &[]));
    }
    for (id, name, family, values, summary) in [
        ("upper-critical-hand", "(上)致命打", EquipmentAbilityFamily::Critical, EquipmentValues { critical: 3, ..EquipmentValues::default() }, "クリティカル +3"),
        ("upper-accuracy-hand", "(上)的中剣", EquipmentAbilityFamily::Accuracy, EquipmentValues { accuracy: 3, ..EquipmentValues::default() }, "命中 +3"),
    ] {
        out.push(fixed_slot_ability(id, name, PartSlot::Hand, family, 3, "hand-category-3", values, summary));
    }

    out.push(slot_ability(
        "night-star-agility-leg", "夜星の敏捷", PartSlot::Leg, EquipmentAbilityFamily::Agility,
        "leg-ability", EquipmentValues::default(), "移動速度 +12", true, &[],
    ));

    for (id, name, family, summary) in [
        ("rest-pointed-blade", "安息の尖った刃", EquipmentAbilityFamily::PointedBlade, "突き +1〜15"),
        ("rest-sharp-blade", "安息の鋭い刃", EquipmentAbilityFamily::SharpBlade, "斬り +1〜15"),
        ("rest-intelligence", "安息の知力", EquipmentAbilityFamily::Intelligence, "魔攻 +1〜15"),
        ("rest-magic-resistance", "安息の耐魔力", EquipmentAbilityFamily::MagicResistance, "魔防 +1〜15"),
    ] {
        out.push(slot_ability(id, name, PartSlot::RelicPendant, family, "relic-pendant-ability", EquipmentValues::default(), summary, false, &[]));
    }
    for (id, name, family, summary) in [
        ("immortal-accuracy", "不死の的中剣", EquipmentAbilityFamily::Accuracy, "命中 +1〜13"),
        ("immortal-evasion", "不死の機敏", EquipmentAbilityFamily::Evasion, "回避 +1〜13"),
        ("immortal-critical", "不死の致命打", EquipmentAbilityFamily::Critical, "クリティカル +1〜12"),
        ("immortal-vitality", "不死の生命力", EquipmentAbilityFamily::Vitality, "最大HP +6,000〜10,000"),
    ] {
        out.push(slot_ability(id, name, PartSlot::RelicBracelet, family, "relic-bracelet-ability", EquipmentValues::default(), summary, id == "immortal-vitality", &[]));
    }
    out
}

#[cfg(test)]
mod tests {
    use domain::DamageCategory;

    /// 追加アビリティは抽選結果なので、登録した基本アビリティから自動適用しない。
    #[test]
    fn 追加アビリティは説明だけを持ち自動計算しない() {
        for def in equipment_abilities()
            .into_iter()
            .filter(|d| d.slot == PartSlot::Weapon && d.category == 4)
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
        assert_eq!(blue.enchant_caps, v(670, 250, 250, 250, 250, 220, 220, 220, 220));

        let yellow = find_equipment_item("stallion-sign-yellow").unwrap();
        assert_eq!(yellow.enchant_caps, v(250, 250, 250, 250, 670, 220, 220, 220, 220));
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

        let mut got = item_damage_contributions(&equipment, SkillDependency::Hack);
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
            item_damage_contributions(&equipment, SkillDependency::Hack),
            vec![(DamageCategory::DependencyDamageRate, 0.30)]
        );
        assert!(item_damage_contributions(&equipment, SkillDependency::HackInt).is_empty());
    }

    #[test]
    fn afは6依存すべてにディフェンシオ候補がある() {
        use SkillDependency::*;
        for dependency in [Stab, Hack, StabHack, Int, Mr, HackInt] {
            assert!(equipment_catalog().iter().any(|item| {
                item.slot == PartSlot::Artifact
                    && item.name.contains("ディフェンシオ")
                    && item.recommended_dependency == Some(dependency)
            }), "{dependency:?}");
        }
    }

    #[test]
    fn afの主要3段は各6依存の通常版とディフェンシオを持つ() {
        for prefix in ["psyche", "eclipse", "ethereal"] {
            for suffix in ["stab", "hack", "physical", "int", "mr", "hack-int"] {
                for id in [format!("{prefix}-{suffix}"), format!("{prefix}-{suffix}-def")] {
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
            item_damage_contributions(&equipment, SkillDependency::HackInt),
            vec![(DamageCategory::DependencyDamageRate, 0.30)]
        );
        assert!(item_damage_contributions(&equipment, SkillDependency::Int).is_empty());
    }

    #[test]
    fn afの耐久効果は攻撃効果と分離して主要3段へ入る() {
        assert_eq!(
            find_equipment_item("eclipse-hack-int").unwrap().survival_effects,
            SURVIVAL_MITIGATION_10
        );
        assert_eq!(
            find_equipment_item("eclipse-hack-int-def").unwrap().survival_effects,
            SURVIVAL_DEFENSE_RATE_30
        );
        assert_eq!(
            find_equipment_item("ethereal-hack-int").unwrap().survival_effects,
            SURVIVAL_MITIGATION_15
        );
        assert_eq!(
            find_equipment_item("ethereal-hack-int-def").unwrap().survival_effects,
            SURVIVAL_MITIGATION_40
        );

        let mut equipment = Equipment::default();
        equipment.parts.artifact.item_id = Some("ethereal-hack-int-def".to_string());
        assert_eq!(
            item_damage_contributions(&equipment, SkillDependency::HackInt),
            vec![(DamageCategory::DependencyDamageRate, 0.35)],
            "緩和40%を自分の与ダメージ式へ混ぜない"
        );
    }

    #[test]
    fn 神鳥とルナリアレリックは20段階あり直前段階の完成値から成長する() {
        let catalog = equipment_catalog();
        assert_eq!(catalog.iter().filter(|item| item.id.starts_with("godbird-pendant-") || item.id.starts_with("lunaria-pendant-")).count(), 20);
        assert_eq!(catalog.iter().filter(|item| item.id.starts_with("godbird-bracelet-") || item.id.starts_with("lunaria-bracelet-")).count(), 20);

        let pendant = find_equipment_item("godbird-pendant-plus2").unwrap();
        let bracelet = find_equipment_item("godbird-bracelet-plus2").unwrap();
        assert_eq!(pendant.values_min, v(30, 30, 0, 30, 0, 25, 25, 0, 0));
        assert_eq!(bracelet.values_min, v(0, 0, 30, 0, 30, 0, 0, 25, 25));
        assert_eq!(pendant.growth_caps.unwrap(), v(50, 50, 0, 50, 0, 45, 45, 0, 0));
        assert_eq!(bracelet.growth_caps.unwrap(), v(0, 0, 50, 0, 50, 0, 0, 45, 45));
        assert_eq!(pendant.enchant_caps, EquipmentValues::default());
        assert_eq!(bracelet.enchant_caps, EquipmentValues::default());
        assert_eq!(pendant.ability_slots, 0);
        assert_eq!(pendant.random_option_slots, None);

        let lunaria = find_equipment_item("lunaria-pendant-plus10").unwrap();
        assert_eq!(lunaria.values_min, v(190, 190, 0, 190, 0, 190, 190, 0, 0));
        assert_eq!(lunaria.growth_caps.unwrap(), v(200, 200, 0, 200, 0, 200, 200, 0, 0));
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
    fn 盾プラスは初期140で成長上限200かつエンチャント不可() {
        let items: Vec<_> = equipment_catalog()
            .into_iter()
            .filter(|i| i.slot == PartSlot::ShieldPlus)
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "†ライジングホリックカフス");
        assert_eq!(
            items[0].values_max,
            v(140, 140, 140, 140, 140, 140, 140, 140, 140)
        );
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
    fn 武器アビリティはカテゴリー1_3_4の37件_idは重複しない() {
        let abilities = equipment_abilities();
        assert_eq!(abilities.iter().filter(|a| a.slot == PartSlot::Weapon).count(), 37);
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

    /// カテゴリー4は新装着アビリティ4系統各4件。
    #[test]
    fn アビリティは4系統各4件で記録値と追加枠情報を持つ() {
        use domain::EquipmentAbilityFamily::*;
        let abilities = equipment_abilities();
        for family in [PointedBlade, SharpBlade, Intelligence, MagicResistance] {
            let members: Vec<_> = abilities
                .iter()
                .filter(|a| a.slot == PartSlot::Weapon && a.category == 4 && a.family == family)
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
}
