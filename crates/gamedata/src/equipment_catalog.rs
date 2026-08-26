//! 装備カタログ(部位別アイテム・武器系統・装備強化倍率・武器アビリティ)。
//!
//! 出典: 装備システム / 装備システム/エンチャント / 装備システム/装備強化 / 装備システム/アビリティ /
//! Item/武器/刀 / Item/武器/太刀 / Item/防具/兜 / Item/防具/鎧/軽鎧 / Item/防具/腕/シールド /
//! Item/防具/腕/盾＋ / Item/アクセサリ/顔・体・手・足・エフェクト(取得 2026-08-24)。
//! docs/claude/goals/2026-08-24-equipment-parts.md「wiki 調査結果」「カタログ seed」節参照。

use domain::{
    DamageCategory, EnhanceRates, EquipmentAbilityDef, EquipmentAbilityFamily, EquipmentValues,
    PartSlot, SkillEffect,
};

use crate::Source;

/// 装備カタログの出典。
pub const EQUIPMENT_CATALOG_SOURCE: Source = Source {
    page: "Item/武器/刀, Item/武器/太刀, Item/防具/兜, Item/防具/鎧/軽鎧, Item/防具/腕/シールド, \
           Item/防具/腕/盾＋, Item/アクセサリ/顔・体・手・足・エフェクト",
    retrieved_on: "2026-08-24",
    note: "エンドゲーム帯(Lv300/310)のみ収録。他武器種・他 Lv 帯はカスタム入力で運用 `[仮]`",
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
    retrieved_on: "2026-08-24",
    note: "装備攻撃力(基本能力値)に効く武器の4系統×7段のみ収録。R以上のダメージ増加%等は§5スコープ外",
};

/// 武器種(wiki: 装備システム/装備強化「系統」表の該当武器)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponSystem {
    Stab,
    StabHack,
    Hack,
    Int,
    IntHack,
    Mr,
}

fn weapon_system(class: WeaponClass) -> WeaponSystem {
    use WeaponClass::*;
    use WeaponSystem::*;
    match class {
        Rapier | Dagger | Spear | SmallSword | PhysicalGun | Claw | HandLauncher => Stab,
        LongSword | Tachi | WarStaff | ShortSword | Rod | Nunchaku => StabHack,
        Katana | Axe | Whip | Kara | DualBladePhysical | Scythe | ArmingSword => Hack,
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
        WeaponSystem::Stab => {
            EnhanceRates { thrust: 6.67, slash: 1.00, magic_attack: 0.0, magic_defense: 0.0 }
        }
        // 突き攻撃力 x 4.55 + 斬り攻撃力 x 4.55
        WeaponSystem::StabHack => {
            EnhanceRates { thrust: 4.55, slash: 4.55, magic_attack: 0.0, magic_defense: 0.0 }
        }
        // 斬り攻撃力 x 6.67 + 突き攻撃力 x 1.00
        WeaponSystem::Hack => {
            EnhanceRates { thrust: 1.00, slash: 6.67, magic_attack: 0.0, magic_defense: 0.0 }
        }
        // 魔法攻撃力 x 6.95 + 魔法防御力 x 1.05
        WeaponSystem::Int => {
            EnhanceRates { thrust: 0.0, slash: 0.0, magic_attack: 6.95, magic_defense: 1.05 }
        }
        // 魔法攻撃力 x 4.55 + 斬り攻撃力 x 3.85
        WeaponSystem::IntHack => {
            EnhanceRates { thrust: 0.0, slash: 3.85, magic_attack: 4.55, magic_defense: 0.0 }
        }
        // 魔法防御力 x 7.70 + 魔法攻撃力 x 0.70
        WeaponSystem::Mr => {
            EnhanceRates { thrust: 0.0, slash: 0.0, magic_attack: 0.70, magic_defense: 7.70 }
        }
    }
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

/// 装備カタログの 1 アイテム。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct EquipmentItem {
    pub id: &'static str,
    pub slot: PartSlot,
    pub name: &'static str,
    /// 基本能力値のレンジ下限(wiki: Item ページの MR レンジ)
    pub values_min: EquipmentValues,
    /// 基本能力値のレンジ上限
    pub values_max: EquipmentValues,
    /// エンチャント上限(wiki: Item ページの「上限」行。エンチャント不可は全 0)
    pub enchant_caps: EquipmentValues,
    /// 武器のみ `Some`(強化補正式の系統決定に使う)
    pub weapon_class: Option<WeaponClass>,
    pub source: Source,
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

/// 装備カタログ(エンドゲーム帯 20 件)。
pub fn equipment_catalog() -> Vec<EquipmentItem> {
    vec![
        EquipmentItem {
            id: "aquilus-scimitar",
            slot: PartSlot::Weapon,
            name: "†アクィルスシミター",
            values_min: v(95, 233, 36, 39, 33, 34, 27, 30, 28),
            values_max: v(105, 243, 39, 45, 35, 36, 30, 31, 31),
            enchant_caps: v(280, 300, 280, 280, 280, 280, 37, 280, 280),
            weapon_class: Some(WeaponClass::Katana),
            source: ITEM_SOURCE_NOTE_KATANA,
        },
        EquipmentItem {
            id: "abyss-scimitar",
            slot: PartSlot::Weapon,
            name: "†アビスシミター",
            values_min: v(115, 300, 36, 39, 33, 34, 30, 27, 28),
            values_max: v(130, 330, 39, 45, 35, 36, 31, 30, 31),
            enchant_caps: v(400, 400, 100, 100, 100, 100, 100, 100, 100),
            weapon_class: Some(WeaponClass::Katana),
            source: ITEM_SOURCE_NOTE_KATANA,
        },
        EquipmentItem {
            id: "aquilus-fake-sword",
            slot: PartSlot::Weapon,
            name: "†アクィルスフェイクソード",
            values_min: v(167, 170, 39, 41, 33, 34, 29, 29, 29),
            values_max: v(177, 180, 41, 47, 36, 36, 32, 32, 34),
            enchant_caps: v(300, 300, 280, 280, 280, 280, 37, 280, 280),
            weapon_class: Some(WeaponClass::Tachi),
            source: ITEM_SOURCE_NOTE_TACHI,
        },
        EquipmentItem {
            id: "abyss-fake-sword",
            slot: PartSlot::Weapon,
            name: "†アビスフェイクソード",
            values_min: v(215, 215, 39, 41, 33, 34, 29, 29, 29),
            values_max: v(235, 235, 41, 47, 36, 36, 32, 32, 34),
            enchant_caps: v(400, 400, 100, 100, 100, 100, 100, 100, 100),
            weapon_class: Some(WeaponClass::Tachi),
            source: ITEM_SOURCE_NOTE_TACHI,
        },
        EquipmentItem {
            id: "aquilus-helm",
            slot: PartSlot::Helm,
            name: "†アクィルスヘルム",
            values_min: v(73, 75, 71, 75, 81, 47, 41, 47, 47),
            values_max: v(83, 85, 81, 85, 91, 57, 51, 57, 57),
            enchant_caps: v(113, 115, 105, 115, 121, 81, 57, 81, 81),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_HELM,
        },
        EquipmentItem {
            id: "abyss-helm",
            slot: PartSlot::Helm,
            name: "†アビスヘルム",
            values_min: v(92, 92, 94, 92, 104, 82, 82, 82, 82),
            values_max: v(102, 102, 124, 102, 134, 92, 92, 92, 92),
            enchant_caps: v(122, 122, 154, 122, 164, 112, 112, 112, 112),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_HELM,
        },
        EquipmentItem {
            id: "aquilus-armor",
            slot: PartSlot::Armor,
            name: "†アクィルスアーマー",
            values_min: v(0, 0, 197, 0, 181, 0, 0, 102, 0),
            values_max: v(0, 0, 207, 0, 191, 0, 0, 112, 0),
            enchant_caps: v(0, 0, 237, 0, 221, 0, 0, 136, 0),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ARMOR,
        },
        EquipmentItem {
            id: "abyss-armor",
            slot: PartSlot::Armor,
            name: "†アビスアーマー",
            values_min: v(0, 0, 260, 0, 230, 0, 0, 100, 0),
            values_max: v(0, 0, 280, 0, 260, 0, 0, 120, 0),
            enchant_caps: v(0, 0, 310, 0, 290, 0, 0, 150, 0),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ARMOR,
        },
        EquipmentItem {
            id: "aquilus-shield",
            slot: PartSlot::Shield,
            name: "†アクィルスシールド",
            values_min: v(0, 0, 177, 0, 172, 0, 0, 0, 0),
            values_max: v(0, 0, 187, 0, 182, 0, 0, 0, 0),
            enchant_caps: v(0, 0, 217, 0, 212, 0, 0, 0, 0),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_SHIELD,
        },
        EquipmentItem {
            id: "abyss-shield",
            slot: PartSlot::Shield,
            name: "†アビスシールド",
            values_min: v(0, 0, 200, 0, 200, 0, 0, 0, 0),
            values_max: v(0, 0, 220, 0, 220, 0, 0, 0, 0),
            enchant_caps: v(0, 0, 260, 0, 260, 0, 0, 0, 0),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_SHIELD,
        },
        EquipmentItem {
            id: "aquilus-amulet",
            slot: PartSlot::Head,
            name: "†アクィルスアミュレット",
            values_min: v(73, 75, 68, 73, 84, 45, 39, 45, 45),
            values_max: v(83, 85, 78, 83, 94, 55, 49, 55, 55),
            enchant_caps: v(113, 115, 92, 113, 124, 79, 55, 79, 79),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "abyss-amulet",
            slot: PartSlot::Head,
            name: "†アビスアミュレット",
            values_min: v(92, 92, 82, 92, 92, 82, 94, 82, 82),
            values_max: v(102, 102, 92, 102, 102, 92, 124, 92, 92),
            enchant_caps: v(122, 122, 112, 122, 122, 112, 154, 112, 112),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "aquilus-wing",
            slot: PartSlot::Body,
            name: "†アクィルスウィング",
            values_min: v(76, 76, 62, 76, 78, 48, 42, 48, 48),
            values_max: v(86, 86, 72, 86, 88, 58, 52, 58, 58),
            enchant_caps: v(116, 116, 96, 116, 118, 78, 58, 82, 82),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "abyss-wing",
            slot: PartSlot::Body,
            name: "†アビスウィング",
            values_min: v(94, 94, 82, 94, 82, 82, 82, 82, 82),
            values_max: v(124, 124, 92, 124, 92, 92, 92, 92, 92),
            enchant_caps: v(154, 154, 112, 154, 112, 112, 112, 112, 112),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "aquilus-gauntlet",
            slot: PartSlot::Hand,
            name: "†アクィルスガントレット",
            values_min: v(72, 72, 56, 72, 72, 90, 44, 44, 44),
            values_max: v(82, 82, 66, 82, 82, 110, 54, 54, 54),
            enchant_caps: v(112, 112, 90, 112, 112, 130, 60, 78, 78),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "abyss-gauntlet",
            slot: PartSlot::Hand,
            name: "†アビスガントレット",
            values_min: v(92, 92, 82, 92, 92, 150, 82, 82, 82),
            values_max: v(102, 102, 92, 102, 102, 180, 92, 92, 92),
            enchant_caps: v(122, 122, 112, 122, 122, 210, 112, 112, 112),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "aquilus-boots",
            slot: PartSlot::Leg,
            name: "†アクィルスブーツ",
            values_min: v(72, 72, 56, 72, 72, 44, 44, 90, 44),
            values_max: v(82, 82, 66, 82, 82, 54, 54, 110, 54),
            enchant_caps: v(112, 112, 90, 112, 112, 78, 60, 130, 78),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "abyss-boots",
            slot: PartSlot::Leg,
            name: "†アビスブーツ",
            values_min: v(92, 92, 82, 92, 92, 82, 82, 150, 82),
            values_max: v(102, 102, 92, 102, 102, 92, 92, 180, 92),
            enchant_caps: v(122, 122, 112, 122, 122, 112, 112, 210, 112),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_ACCESSORY,
        },
        EquipmentItem {
            id: "chapter-artifact",
            slot: PartSlot::ShieldPlus,
            name: "[EP]†チャプターアーティファクト",
            values_min: v(1, 1, 1, 1, 1, 1, 1, 1, 1),
            values_max: v(1, 1, 1, 1, 1, 1, 1, 1, 1),
            enchant_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_SHIELD_PLUS,
        },
        EquipmentItem {
            id: "arcadia-mementomori",
            slot: PartSlot::ShieldPlus,
            name: "†アルカディア・メメントモリ",
            values_min: v(50, 50, 50, 50, 50, 50, 50, 50, 50),
            values_max: v(50, 50, 50, 50, 50, 50, 50, 50, 50),
            enchant_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            source: ITEM_SOURCE_NOTE_SHIELD_PLUS,
        },
    ]
}

pub fn find_equipment_item(id: &str) -> Option<EquipmentItem> {
    equipment_catalog().into_iter().find(|item| item.id == id)
}

fn ability(
    id: &'static str,
    name: &'static str,
    family: EquipmentAbilityFamily,
    values: EquipmentValues,
) -> EquipmentAbilityDef {
    EquipmentAbilityDef { id, name, family, values, damage_effects: &[] }
}

/// 追加効果「ダメージ増加 +n%」付き(wiki: アビリティ表の「追加効果」列。R- 以上の段)。
/// **装備攻撃力ではなくカテゴリX3(攻撃ダメージ(基本発動)、上限 +80%)に入る。**
fn ability_with_damage(
    id: &'static str,
    name: &'static str,
    family: EquipmentAbilityFamily,
    values: EquipmentValues,
    damage_effects: &'static [SkillEffect],
) -> EquipmentAbilityDef {
    EquipmentAbilityDef { id, name, family, values, damage_effects }
}

/// R- 段の追加効果。全 4 系統で共通の +3%。
const DAMAGE_3: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageBasicTrigger,
    percent: 3.0,
}];
/// L- 段の追加効果。
const DAMAGE_4: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageBasicTrigger,
    percent: 4.0,
}];
/// E- 段の追加効果。
const DAMAGE_6: &[SkillEffect] = &[SkillEffect::Damage {
    category: DamageCategory::AttackDamageBasicTrigger,
    percent: 6.0,
}];

/// 武器アビリティは装備攻撃力(突き/斬り/魔攻/魔防)にしか効かない。
fn a(thrust: i64, slash: i64, magic_attack: i64, magic_defense: i64) -> EquipmentValues {
    EquipmentValues { thrust, slash, magic_attack, magic_defense, ..Default::default() }
}

/// 武器アビリティカタログ(4 系統 × 7 段 = 28 件。wiki: 装備システム/アビリティ)。
/// 段は (下)/(中)/(上)/N-/R-/L-/E- で装備攻撃力 +2/+3/+4/+6/+7/+8/+9。
///
/// **R- 以上には追加効果「ダメージ増加 +3/+4/+6%」が付く。**これは装備攻撃力ではなく
/// 与ダメージ式のカテゴリX3(攻撃ダメージ(基本発動)、上限 +80%)に入る。
pub fn equipment_abilities() -> Vec<EquipmentAbilityDef> {
    vec![
        // 尖った刃(突き攻撃力)
        ability("pointed-blade-low", "(下)尖った刃", EquipmentAbilityFamily::PointedBlade, a(2, 0, 0, 0)),
        ability("pointed-blade-mid", "(中)尖った刃", EquipmentAbilityFamily::PointedBlade, a(3, 0, 0, 0)),
        ability("pointed-blade-high", "(上)尖った刃", EquipmentAbilityFamily::PointedBlade, a(4, 0, 0, 0)),
        ability("pointed-blade-n", "N-尖った刃", EquipmentAbilityFamily::PointedBlade, a(6, 0, 0, 0)),
        ability_with_damage("pointed-blade-r", "R-尖った刃", EquipmentAbilityFamily::PointedBlade, a(7, 0, 0, 0), DAMAGE_3),
        ability_with_damage("pointed-blade-l", "L-尖った刃", EquipmentAbilityFamily::PointedBlade, a(8, 0, 0, 0), DAMAGE_4),
        ability_with_damage("pointed-blade-e", "E-尖った刃", EquipmentAbilityFamily::PointedBlade, a(9, 0, 0, 0), DAMAGE_6),
        // 鋭い刃(斬り攻撃力)
        ability("sharp-blade-low", "(下)鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 2, 0, 0)),
        ability("sharp-blade-mid", "(中)鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 3, 0, 0)),
        ability("sharp-blade-high", "(上)鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 4, 0, 0)),
        ability("sharp-blade-n", "N-鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 6, 0, 0)),
        ability_with_damage("sharp-blade-r", "R-鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 7, 0, 0), DAMAGE_3),
        ability_with_damage("sharp-blade-l", "L-鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 8, 0, 0), DAMAGE_4),
        ability_with_damage("sharp-blade-e", "E-鋭い刃", EquipmentAbilityFamily::SharpBlade, a(0, 9, 0, 0), DAMAGE_6),
        // 知力(魔法攻撃力)
        ability("intelligence-low", "(下)知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 2, 0)),
        ability("intelligence-mid", "(中)知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 3, 0)),
        ability("intelligence-high", "(上)知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 4, 0)),
        ability("intelligence-n", "N-知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 6, 0)),
        ability_with_damage("intelligence-r", "R-知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 7, 0), DAMAGE_3),
        ability_with_damage("intelligence-l", "L-知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 8, 0), DAMAGE_4),
        ability_with_damage("intelligence-e", "E-知力", EquipmentAbilityFamily::Intelligence, a(0, 0, 9, 0), DAMAGE_6),
        // 耐魔力(魔法防御力)
        ability("magic-resistance-low", "(下)耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 2)),
        ability("magic-resistance-mid", "(中)耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 3)),
        ability("magic-resistance-high", "(上)耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 4)),
        ability("magic-resistance-n", "N-耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 6)),
        ability_with_damage("magic-resistance-r", "R-耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 7), DAMAGE_3),
        ability_with_damage("magic-resistance-l", "L-耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 8), DAMAGE_4),
        ability_with_damage("magic-resistance-e", "E-耐魔力", EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, 9), DAMAGE_6),
    ]
}

#[cfg(test)]
mod tests {
    use domain::{DamageCategory, SkillEffect};

    /// R- 以上のアビリティには追加効果「ダメージ増加 +3/+4/+6%」が付く
    /// (wiki: 装備システム/アビリティ の「追加効果」列)。**装備攻撃力ではなくカテゴリX3**。
    #[test]
    fn r_以上のアビリティはx3のダメージ増加を持つ() {
        let expected = [("-r", 3.0), ("-l", 4.0), ("-e", 6.0)];
        for def in equipment_abilities() {
            let want = expected.iter().find(|(suffix, _)| def.id.ends_with(suffix));
            match want {
                Some((_, percent)) => assert_eq!(
                    def.damage_effects,
                    &[SkillEffect::Damage {
                        category: DamageCategory::AttackDamageBasicTrigger,
                        percent: *percent,
                    }],
                    "{}",
                    def.id
                ),
                None => assert!(def.damage_effects.is_empty(), "{}", def.id),
            }
        }
        // 4 系統 × R/L/E = 12 件
        let with_damage =
            equipment_abilities().iter().filter(|d| !d.damage_effects.is_empty()).count();
        assert_eq!(with_damage, 12);
    }

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn カタログは20件_idは重複しない() {
        let catalog = equipment_catalog();
        assert_eq!(catalog.len(), 20);
        let ids: HashSet<&str> = catalog.iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), catalog.len());
    }

    #[test]
    fn 武器以外はweapon_classを持たない() {
        for item in equipment_catalog() {
            if item.slot == PartSlot::Weapon {
                assert!(item.weapon_class.is_some(), "{} は武器なのに weapon_class が無い", item.id);
            } else {
                assert!(item.weapon_class.is_none(), "{} は武器以外なのに weapon_class を持つ", item.id);
            }
        }
    }

    #[test]
    fn 盾プラスはエンチャント不可() {
        for item in equipment_catalog().into_iter().filter(|i| i.slot == PartSlot::ShieldPlus) {
            assert_eq!(item.enchant_caps, EquipmentValues::default());
        }
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
    fn 武器アビリティは28件_idは重複しない() {
        let abilities = equipment_abilities();
        assert_eq!(abilities.len(), 28);
        let ids: HashSet<&str> = abilities.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), abilities.len());
    }

    /// 系統は UI の「系統ごとに 1 つ選ぶ」4 行と storage の重複検証の両方が使う。
    #[test]
    fn アビリティは4系統各7段で系統と加算先が対応する() {
        use domain::EquipmentAbilityFamily::*;
        let abilities = equipment_abilities();
        for family in [PointedBlade, SharpBlade, Intelligence, MagicResistance] {
            let members: Vec<_> = abilities.iter().filter(|a| a.family == family).collect();
            assert_eq!(members.len(), 7, "{family:?} は 7 段");
            for def in members {
                let nonzero = [
                    (def.values.thrust, PointedBlade),
                    (def.values.slash, SharpBlade),
                    (def.values.magic_attack, Intelligence),
                    (def.values.magic_defense, MagicResistance),
                ];
                for (value, owner) in nonzero {
                    assert_eq!(value != 0, owner == family, "{} の加算先が系統と食い違う", def.id);
                }
            }
        }
    }

    #[test]
    fn 尖った刃eは突き9() {
        let abilities = equipment_abilities();
        let def = abilities.iter().find(|a| a.id == "pointed-blade-e").unwrap();
        assert_eq!(def.name, "E-尖った刃");
        assert_eq!(def.values, a(9, 0, 0, 0));
    }

    #[test]
    fn 耐魔力下は魔防2() {
        let abilities = equipment_abilities();
        let def = abilities.iter().find(|a| a.id == "magic-resistance-low").unwrap();
        assert_eq!(def.name, "(下)耐魔力");
        assert_eq!(def.values, a(0, 0, 0, 2));
    }
}
