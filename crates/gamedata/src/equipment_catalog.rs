//! 装備カタログ(部位別アイテム・武器系統・装備強化倍率・武器アビリティ)。
//!
//! 出典: 装備システム / 装備システム/エンチャント / 装備システム/装備強化 / 装備システム/アビリティ /
//! Item/武器/刀 / Item/武器/太刀 / Item/防具/兜 / Item/防具/鎧/軽鎧 / Item/防具/腕/シールド /
//! Item/防具/腕/盾＋ / Item/アクセサリ/顔・体・手・足・エフェクト(取得 2026-08-24)。
//! docs/claude/goals/2026-08-24-equipment-parts.md「wiki 調査結果」「カタログ seed」節参照。

use domain::{
    DamageCategory, EnhanceGrade, EnhanceRates, Equipment, EquipmentAbilityAdditionalDef,
    EquipmentAbilityAdditionalKind, EquipmentAbilityDef, EquipmentAbilityFamily, EquipmentEnhanceType,
    EquipmentValues, PartSlot, SkillEffect,
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
    retrieved_on: "2026-08-27",
    note: "武器3スロット。カテゴリー1/3は旧装着アビリティ、カテゴリー4は新装着アビリティページ。追加効果は自動適用しない",
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

pub fn weapon_system(class: WeaponClass) -> WeaponSystem {
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

pub fn enhance_rates_for_type(kind: EquipmentEnhanceType) -> Option<EnhanceRates> {
    Some(match kind {
        EquipmentEnhanceType::WeaponStab => EnhanceRates { thrust: 6.67, slash: 1.00, magic_attack: 0.0, magic_defense: 0.0 },
        EquipmentEnhanceType::WeaponStabHack => EnhanceRates { thrust: 4.55, slash: 4.55, magic_attack: 0.0, magic_defense: 0.0 },
        EquipmentEnhanceType::WeaponHack => EnhanceRates { thrust: 1.00, slash: 6.67, magic_attack: 0.0, magic_defense: 0.0 },
        EquipmentEnhanceType::WeaponInt => EnhanceRates { thrust: 0.0, slash: 0.0, magic_attack: 6.95, magic_defense: 1.05 },
        EquipmentEnhanceType::WeaponIntHack => EnhanceRates { thrust: 0.0, slash: 3.85, magic_attack: 4.55, magic_defense: 0.0 },
        EquipmentEnhanceType::WeaponMr => EnhanceRates { thrust: 0.0, slash: 0.0, magic_attack: 0.70, magic_defense: 7.70 },
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
    Some((min + (max - min) * grade.percentile()).round())
}

pub fn armor_enhance_multiplier(level: u8, grade: Option<EnhanceGrade>) -> Option<f64> {
    enhance_multiplier(level).or_else(|| grade.and_then(|g| enhance_grade_multiplier(level, g))).map(|v| v / 2.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorClass { Light, Heavy, Magic, Suit, Robe }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmorEnhanceRates { pub physical_defense: f64, pub magic_defense: f64 }

pub fn armor_enhance_rates(class: ArmorClass) -> ArmorEnhanceRates {
    match class {
        ArmorClass::Light => ArmorEnhanceRates { physical_defense: 3.90, magic_defense: 4.00 },
        ArmorClass::Heavy => ArmorEnhanceRates { physical_defense: 3.10, magic_defense: 3.80 },
        ArmorClass::Magic => ArmorEnhanceRates { physical_defense: 3.80, magic_defense: 4.00 },
        ArmorClass::Suit => ArmorEnhanceRates { physical_defense: 7.80, magic_defense: 0.00 },
        ArmorClass::Robe => ArmorEnhanceRates { physical_defense: 4.00, magic_defense: 3.80 },
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
    if let Some(class) = find_equipment_item(item_id).and_then(|item| item.weapon_class) {
        return Some(match weapon_system(class) {
            WeaponSystem::Stab => EquipmentEnhanceType::WeaponStab,
            WeaponSystem::StabHack => EquipmentEnhanceType::WeaponStabHack,
            WeaponSystem::Hack => EquipmentEnhanceType::WeaponHack,
            WeaponSystem::Int => EquipmentEnhanceType::WeaponInt,
            WeaponSystem::IntHack => EquipmentEnhanceType::WeaponIntHack,
            WeaponSystem::Mr => EquipmentEnhanceType::WeaponMr,
        });
    }
    match item_id {
        "aquilus-armor" | "abyss-armor" => Some(EquipmentEnhanceType::ArmorLight),
        "lina-clothes" => Some(EquipmentEnhanceType::ArmorRobe),
        _ => None,
    }
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// **装着時効果**(wiki: Item ページ備考の「装着時 …」)。装備補正値ではなく
    /// 与ダメージ式のカテゴリ(X5 / X6 / Old / O)に入る。
    /// **「一定確率で」のものも発動前提で入れる**(ユーザー確定 2026-08-27: ほぼ発動する)
    pub damage_effects: &'static [SkillEffect],
    pub source: Source,
}

impl serde::Serialize for EquipmentItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("EquipmentItem", 11)?;
        s.serialize_field("id", self.id)?; s.serialize_field("slot", &self.slot)?;
        s.serialize_field("name", self.name)?; s.serialize_field("values_min", &self.values_min)?;
        s.serialize_field("values_max", &self.values_max)?; s.serialize_field("enchant_caps", &self.enchant_caps)?;
        s.serialize_field("weapon_class", &self.weapon_class)?;
        s.serialize_field("weapon_system", &self.weapon_class.map(weapon_system))?;
        s.serialize_field("enhance_type", &equipment_enhance_type(self.id))?;
        s.serialize_field("damage_effects", &self.damage_effects)?; s.serialize_field("source", &self.source)?;
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

/// 装着時「与ダメージ+3%」= カテゴリX6 攻撃ダメージ(日本独自)(上限 +30%)。
const ITEM_DAMAGE_JAPAN_3: &[SkillEffect] =
    &[SkillEffect::Damage { category: DamageCategory::AttackDamageJapan, percent: 3.0 }];
/// 装着時「与ダメージ+1%」= カテゴリX6。
const ITEM_DAMAGE_JAPAN_1: &[SkillEffect] =
    &[SkillEffect::Damage { category: DamageCategory::AttackDamageJapan, percent: 1.0 }];
/// 装着時「物理/魔法攻撃力 +5%」= カテゴリX6。wiki 注記どおり物理・魔法に関係なく上がる。
const ITEM_DAMAGE_JAPAN_5: &[SkillEffect] =
    &[SkillEffect::Damage { category: DamageCategory::AttackDamageJapan, percent: 5.0 }];
/// 装着時「攻撃力が3%増加」= カテゴリX5 攻撃ダメージ(特殊)(wiki は上限未記載)。
const ITEM_DAMAGE_SPECIAL_3: &[SkillEffect] =
    &[SkillEffect::Damage { category: DamageCategory::AttackDamageSpecial, percent: 3.0 }];
/// 要塞占領報酬の体装備「攻撃ダメージ増加」= カテゴリOld 攻撃ダメージII(初期 100%・上限 300%)。
const ITEM_DAMAGE_LEGACY_25: &[SkillEffect] =
    &[SkillEffect::Damage { category: DamageCategory::AttackDamageLegacy, percent: 25.0 }];
/// 「魔法での与ダメージ+3%」= カテゴリO 物理/魔法ダメージ増加。
/// wiki の注記どおり物理攻撃(熊)にも乗るので、依存で分けずカテゴリO にそのまま入れる。
const ITEM_DAMAGE_PHYSICAL_MAGIC_3: &[SkillEffect] =
    &[SkillEffect::Damage { category: DamageCategory::PhysicalMagicDamageRate, percent: 3.0 }];

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
    enchant_caps: EquipmentValues,
    damage_effects: &'static [SkillEffect],
) -> EquipmentItem {
    EquipmentItem {
        id,
        slot: PartSlot::Effect,
        name,
        values_min: values,
        values_max: values,
        enchant_caps,
        weapon_class: None,
        damage_effects,
        source: ITEM_SOURCE_DAMAGE_EFFECT,
    }
}

/// 「装着時攻撃力が3%増加」= カテゴリX5。5 種の違いは特化する 1 値(20)だけで、
/// 残りの装備補正は 5、命中/Cri/回避/敏捷は 18、エンチャント上限は全 255 で共通。
fn effect_attack_3(id: &'static str, name: &'static str, values: EquipmentValues) -> EquipmentItem {
    effect_item(id, name, values, v(255, 255, 255, 255, 255, 255, 255, 255, 255), ITEM_DAMAGE_SPECIAL_3)
}

/// 「スキル使用時、一定確率で攻撃ダメージ(攻撃力)が3%上昇」= カテゴリX6。
/// Lv310 帯のコラボエフェクト。補正値は全 25 で、特化する 1 値と上限だけが違う。
fn effect_trigger_3(
    id: &'static str,
    name: &'static str,
    values: EquipmentValues,
    enchant_caps: EquipmentValues,
) -> EquipmentItem {
    effect_item(id, name, values, enchant_caps, ITEM_DAMAGE_JAPAN_3)
}

/// 宝箱「凛々の明星」の 4 種。1 値だけ 30〜50 の MR レンジを持ち、ほかは全 25。
fn effect_trigger_3_ranged(
    id: &'static str,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_caps: EquipmentValues,
) -> EquipmentItem {
    EquipmentItem {
        id,
        slot: PartSlot::Effect,
        name,
        values_min,
        values_max,
        enchant_caps,
        weapon_class: None,
        damage_effects: ITEM_DAMAGE_JAPAN_3,
        source: ITEM_SOURCE_DAMAGE_EFFECT,
    }
}

/// 装備カタログ。エンドゲーム帯 20 件 +「装着時に与ダメージが上がる」装備 19 件。
/// 後者は装備補正値だけでなく `damage_effects` を持ち、与ダメージ式のカテゴリに入る。
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_TACHI,
        },
        EquipmentItem {
            id: "aquilus-great-sword",
            slot: PartSlot::Weapon,
            name: "†アクィルスブレイド",
            values_min: v(80, 184, 35, 161, 38, 34, 29, 28, 26),
            values_max: v(85, 194, 38, 171, 40, 38, 32, 30, 28),
            enchant_caps: v(280, 300, 280, 300, 280, 280, 37, 280, 280),
            weapon_class: Some(WeaponClass::GreatSword),
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_GREAT_SWORD,
        },
        EquipmentItem {
            id: "abyss-great-sword",
            slot: PartSlot::Weapon,
            name: "†アビスブレード",
            values_min: v(84, 230, 35, 230, 38, 34, 29, 28, 26),
            values_max: v(89, 250, 38, 250, 40, 38, 32, 30, 28),
            enchant_caps: v(400, 400, 100, 400, 100, 100, 100, 100, 100),
            weapon_class: Some(WeaponClass::GreatSword),
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_GREAT_SWORD,
        },
        EquipmentItem {
            id: "aquilus-helm",
            slot: PartSlot::Helm,
            name: "†アクィルスヘルム",
            values_min: v(73, 75, 71, 75, 81, 47, 41, 47, 47),
            values_max: v(83, 85, 81, 85, 91, 57, 51, 57, 57),
            enchant_caps: v(113, 115, 105, 115, 121, 81, 57, 81, 81),
            weapon_class: None,
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
            source: ITEM_SOURCE_NOTE_SHIELD_PLUS,
        },
        // ── 装着時効果つき(与ダメージ式のカテゴリに入る)──────────────────────
        // カテゴリX6 攻撃ダメージ(日本独自): コラボ武器「装備時、与ダメージ+3%上昇」
        EquipmentItem {
            id: "nibanboshi-katana",
            slot: PartSlot::Weapon,
            name: "†ニバンボシ(刀)",
            values_min: v(120, 320, 42, 22, 42, 36, 30, 30, 28),
            values_max: v(160, 360, 45, 27, 45, 36, 30, 31, 31),
            enchant_caps: v(460, 480, 100, 100, 100, 105, 105, 100, 100),
            weapon_class: Some(WeaponClass::Katana),
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_KATANA,
        },
        EquipmentItem {
            id: "nibanboshi-tachi",
            slot: PartSlot::Weapon,
            name: "†ニバンボシ(太刀)",
            values_min: v(240, 240, 39, 41, 33, 36, 32, 29, 29),
            values_max: v(260, 260, 41, 47, 36, 36, 32, 32, 34),
            enchant_caps: v(480, 480, 100, 100, 100, 105, 105, 100, 100),
            weapon_class: Some(WeaponClass::Tachi),
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_TACHI,
        },
        // カテゴリO 物理/魔法ダメージ増加
        EquipmentItem {
            id: "lina-clothes",
            slot: PartSlot::Armor,
            name: "†リナの服",
            values_min: v(0, 0, 260, 30, 280, 85, 0, 81, 0),
            values_max: v(0, 0, 280, 45, 300, 115, 0, 91, 0),
            enchant_caps: v(0, 0, 300, 150, 350, 120, 0, 105, 0),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_PHYSICAL_MAGIC_3,
            source: ITEM_SOURCE_DAMAGE_ROBE,
        },
        // カテゴリOld 攻撃ダメージII(要塞占領報酬。2 種は補正値まで同じ)
        EquipmentItem {
            id: "archangel-wing",
            slot: PartSlot::Body,
            name: "†主天使の羽",
            values_min: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            values_max: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            enchant_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_LEGACY_25,
            source: ITEM_SOURCE_DAMAGE_BODY,
        },
        EquipmentItem {
            id: "sigma-wing",
            slot: PartSlot::Body,
            name: "†シグマウィング",
            values_min: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            values_max: v(75, 75, 65, 75, 75, 65, 50, 65, 65),
            enchant_caps: v(0, 0, 0, 0, 0, 0, 0, 0, 0),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_LEGACY_25,
            source: ITEM_SOURCE_DAMAGE_BODY,
        },
        // カテゴリX6: 手装備。けものフレンズコラボは +5%、ダンジョン飯コラボは +3%
        EquipmentItem {
            id: "gorilla-armcover",
            slot: PartSlot::Hand,
            name: "†ゴリラのあーむかばー",
            values_min: v(44, 44, 44, 44, 44, 78, 38, 38, 38),
            values_max: v(54, 54, 54, 54, 54, 100, 48, 48, 48),
            enchant_caps: v(90, 90, 80, 90, 80, 118, 54, 66, 66),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_JAPAN_5,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        EquipmentItem {
            id: "tanuki-gloves",
            slot: PartSlot::Hand,
            name: "†タヌキの手袋",
            values_min: v(44, 44, 38, 44, 44, 78, 38, 38, 38),
            values_max: v(54, 54, 48, 54, 54, 100, 48, 48, 48),
            enchant_caps: v(112, 90, 112, 112, 112, 130, 60, 78, 78),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_JAPAN_5,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        EquipmentItem {
            id: "izutsumi-gauntlet",
            slot: PartSlot::Hand,
            name: "†イヅツミの手甲",
            values_min: v(80, 80, 82, 60, 60, 150, 82, 82, 82),
            values_max: v(90, 90, 92, 80, 80, 180, 92, 92, 92),
            enchant_caps: v(150, 150, 112, 105, 105, 210, 112, 112, 112),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        EquipmentItem {
            id: "rin-gloves",
            slot: PartSlot::Hand,
            name: "†リンの手袋",
            values_min: v(60, 60, 82, 100, 80, 150, 82, 82, 82),
            values_max: v(80, 80, 92, 120, 90, 180, 92, 92, 92),
            enchant_caps: v(105, 105, 112, 150, 150, 210, 112, 112, 112),
            weapon_class: None,
            damage_effects: ITEM_DAMAGE_JAPAN_3,
            source: ITEM_SOURCE_DAMAGE_HAND,
        },
        // カテゴリX5 攻撃ダメージ(特殊): エフェクト(装着時攻撃力 +3%)
        effect_attack_3("beast-cerberus", "【年占】†幻獣(ケルベロス)", v(20, 5, 5, 5, 5, 18, 18, 18, 18)),
        effect_attack_3("beast-phoenix", "【年占】†幻獣(フェニックス)", v(5, 20, 5, 5, 5, 18, 18, 18, 18)),
        effect_attack_3("beast-griffon", "【年占】†幻獣(グリフォン)", v(5, 5, 20, 5, 5, 18, 18, 18, 18)),
        effect_attack_3("beast-leviathan", "【年占】†幻獣(リヴァイアサン)", v(5, 5, 5, 20, 5, 18, 18, 18, 18)),
        effect_attack_3("beast-unicorn", "【年占】†幻獣(ユニコーン)", v(5, 5, 5, 5, 20, 18, 18, 18, 18)),
        effect_attack_3("memorial-crest-dark", "【18th】†記念の祝福紋様 − 闇", v(20, 5, 5, 5, 5, 18, 18, 18, 18)),
        effect_attack_3("memorial-crest-water", "【18th】†記念の祝福紋様 − 水", v(5, 20, 5, 5, 5, 18, 18, 18, 18)),
        effect_attack_3("memorial-crest-fire", "【18th】†記念の祝福紋様 − 炎", v(5, 5, 20, 5, 5, 18, 18, 18, 18)),
        effect_attack_3("memorial-crest-light", "【18th】†記念の祝福紋様 − 光", v(5, 5, 5, 20, 5, 18, 18, 18, 18)),
        effect_attack_3("memorial-crest-wind", "【18th】†記念の祝福紋様 − 風", v(5, 5, 5, 5, 20, 18, 18, 18, 18)),
        // カテゴリX6: エフェクトの「スキル使用時、一定確率で 3% 上昇」。**発動前提で入れる**
        // (ユーザー確定 2026-08-27)。wiki の文言は「攻撃ダメージ」「攻撃力」で揺れるが、
        // ステータス表 1205 行はどちらも同じ X6 +3% の行にまとめている
        effect_trigger_3("logh-full-control-battle", "†全力管制戦闘",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25), v(375, 375, 375, 375, 375, 255, 255, 255, 255)),
        effect_trigger_3("slayers-drag-slave", "†竜破斬＜ドラグ・スレイブ＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25), v(255, 255, 255, 400, 255, 255, 255, 255, 255)),
        effect_trigger_3("slayers-giga-slave", "†重破斬＜ギガ・スレイブ＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25), v(255, 400, 255, 255, 255, 255, 255, 255, 255)),
        effect_trigger_3("slayers-ragna-blade", "†神滅斬＜ラグナ・ブレード＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25), v(400, 255, 255, 255, 255, 255, 255, 255, 255)),
        effect_trigger_3("slayers-claire-bible", "†異界黙示録＜クレアバイブル＞",
            v(25, 25, 25, 25, 25, 25, 25, 25, 25), v(255, 255, 255, 255, 400, 255, 255, 255, 255)),
        // 宝箱「凛々の明星」の 4 種は 1 値だけ 30〜50 のレンジを持つ
        // (†ヴァイオレットペインの突き欄は wiki が 255 = 上限値の書き間違いなので 25 を採る)
        effect_trigger_3_ranged("rinrin-tidal-wave", "†タイダルウェイブ",
            v(30, 25, 25, 25, 25, 25, 25, 25, 25), v(50, 25, 25, 25, 25, 25, 25, 25, 25),
            v(500, 255, 255, 255, 255, 255, 255, 255, 255)),
        effect_trigger_3_ranged("rinrin-heavenly-wing-sword", "†天翔光翼剣",
            v(25, 30, 25, 25, 25, 25, 25, 25, 25), v(25, 50, 25, 25, 25, 25, 25, 25, 25),
            v(255, 500, 255, 255, 255, 255, 255, 255, 255)),
        effect_trigger_3_ranged("rinrin-violet-pain", "†ヴァイオレットペイン",
            v(25, 25, 25, 30, 25, 25, 25, 25, 25), v(25, 25, 25, 50, 25, 25, 25, 25, 25),
            v(255, 255, 255, 500, 255, 255, 255, 255, 255)),
        effect_trigger_3_ranged("rinrin-crimson-flare", "†クリムゾンフレア",
            v(25, 25, 25, 25, 30, 25, 25, 25, 25), v(25, 25, 25, 25, 50, 25, 25, 25, 25),
            v(255, 255, 255, 255, 500, 255, 255, 255, 255)),
        // 「装着時：与ダメージ+1%」(確率ではない)
        effect_item("logh-lost", "†ロスト", v(22, 22, 22, 22, 22, 22, 22, 22, 22),
            v(255, 255, 255, 255, 255, 255, 255, 255, 255), ITEM_DAMAGE_JAPAN_1),
    ]
}

pub fn find_equipment_item(id: &str) -> Option<EquipmentItem> {
    equipment_catalog().into_iter().find(|item| item.id == id)
}

/// 装備しているアイテムそのものの装着時効果を、与ダメージ式のカテゴリ寄与に変換する。
/// 装備補正値は `Equipment::base_totals` が別に見る。
///
/// `Equipment::ability_damage_contributions` と同じ役割だが、`EquipmentItem` は
/// `Source` / `WeaponClass` を持つので domain ではなくこちら側にある。
pub fn item_damage_contributions(equipment: &Equipment) -> Vec<(DamageCategory, f64)> {
    let catalog = equipment_catalog();
    let effects: Vec<&'static SkillEffect> = equipment
        .parts
        .iter()
        .into_iter()
        .filter_map(|(_, part)| part.item_id.as_deref())
        .filter_map(|id| catalog.iter().find(|item| item.id == id))
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
        EquipmentAbilityFamily::PointedBlade => "固定ダメージ/割合ダメージ/突き/自然回復/命中からランダム",
        EquipmentAbilityFamily::SharpBlade => "固定ダメージ/割合ダメージ/斬り/自然回復/命中からランダム",
        EquipmentAbilityFamily::Intelligence => "固定ダメージ/割合ダメージ/魔攻/自然回復/命中からランダム",
        EquipmentAbilityFamily::MagicResistance => "固定ダメージ/割合ダメージ/魔防/自然回復/命中からランダム",
        EquipmentAbilityFamily::WeaponDelay => "",
    };
    let stat_kind = match family {
        EquipmentAbilityFamily::PointedBlade => Thrust,
        EquipmentAbilityFamily::SharpBlade => Slash,
        EquipmentAbilityFamily::Intelligence => MagicAttack,
        EquipmentAbilityFamily::MagicResistance => MagicDefense,
        EquipmentAbilityFamily::WeaponDelay => unreachable!("新装着アビリティに武器ディレイ系は無い"),
    };
    let (fixed, rate, stat_min, stat_max, recovery_min, recovery_max, accuracy_min, accuracy_max) =
        match values.thrust.max(values.slash).max(values.magic_attack).max(values.magic_defense) {
            11 => (5000, 8, 5, 10, 4, 14, 6, 10),
            13 => (6000, 9, 7, 12, 6, 16, 8, 12),
            17 => (7000, 10, 7, 15, 6, 16, 8, 14),
            _ => (10000, 11, 9, 18, 8, 18, 10, 16),
        };
    let additional_options = vec![
        EquipmentAbilityAdditionalDef { kind: FixedDamage, min: fixed, max: fixed },
        EquipmentAbilityAdditionalDef { kind: DamageRate, min: rate, max: rate },
        EquipmentAbilityAdditionalDef { kind: stat_kind, min: stat_min, max: stat_max },
        EquipmentAbilityAdditionalDef { kind: HpRecovery, min: recovery_min, max: recovery_max },
        EquipmentAbilityAdditionalDef { kind: MpRecovery, min: recovery_min, max: recovery_max },
        EquipmentAbilityAdditionalDef { kind: Accuracy, min: accuracy_min, max: accuracy_max },
    ];
    EquipmentAbilityDef {
        id, name, family, category: 4, slot: PartSlot::Weapon,
        exclusive_group: "weapon-category-4",
        additional_slots: 2, additional_effects, additional_options, record_only: false,
        effect_summary, values, damage_effects: &[],
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
        id, name, family, category, slot: PartSlot::Weapon,
        exclusive_group: match category {
            1 => "weapon-category-1",
            3 => "weapon-category-3",
            _ => "weapon-category-other",
        },
        additional_slots: 0, additional_effects: "", additional_options: vec![], record_only,
        effect_summary, values, damage_effects: &[],
    }
}

/// 武器アビリティは装備攻撃力(突き/斬り/魔攻/魔防)にしか効かない。
fn a(thrust: i64, slash: i64, magic_attack: i64, magic_defense: i64) -> EquipmentValues {
    EquipmentValues { thrust, slash, magic_attack, magic_defense, ..Default::default() }
}

/// 武器アビリティ。武器は最大3スロットで、同じカテゴリーは1つまで。
/// カテゴリー1と4は同じ攻撃系統でも併用できる（例: 下級斬り + 夜星の鋭い刃）。
/// カテゴリー4の追加アビリティはランダムなので自動適用しない。
pub fn equipment_abilities() -> Vec<EquipmentAbilityDef> {
    let mut out = Vec::new();

    // カテゴリー1: 旧アビリティ。2026年追加の「下級〜」も同カテゴリー。
    for (family, entries) in [
        (EquipmentAbilityFamily::PointedBlade, [
            ("low-pointed-blade", "(下)尖った刃", 2, "突き +2"),
            ("middle-pointed-blade", "(中)尖った刃", 3, "突き +3"),
            ("upper-pointed-blade", "(上)尖った刃", 4, "突き +4"),
            ("lower-grade-stab", "下級突き", 12, "突き +12"),
        ]),
        (EquipmentAbilityFamily::SharpBlade, [
            ("low-sharp-blade", "(下)鋭い刃", 2, "斬り +2"),
            ("middle-sharp-blade", "(中)鋭い刃", 3, "斬り +3"),
            ("upper-sharp-blade", "(上)鋭い刃", 4, "斬り +4"),
            ("lower-grade-slash", "下級斬り", 12, "斬り +12"),
        ]),
        (EquipmentAbilityFamily::Intelligence, [
            ("low-intelligence", "(下)知力", 2, "魔攻 +2"),
            ("middle-intelligence", "(中)知力", 3, "魔攻 +3"),
            ("upper-intelligence", "(上)知力", 4, "魔攻 +4"),
            ("lower-grade-magic-attack", "下級魔法攻撃", 12, "魔攻 +12"),
        ]),
        (EquipmentAbilityFamily::MagicResistance, [
            ("low-magic-resistance", "(下)耐魔力", 2, "魔防 +2"),
            ("middle-magic-resistance", "(中)耐魔力", 3, "魔防 +3"),
            ("upper-magic-resistance", "(上)耐魔力", 4, "魔防 +4"),
            ("lower-grade-magic-defense", "下級魔法防御", 12, "魔防 +12"),
        ]),
    ] {
        for (id, name, value, summary) in entries {
            let values = match family {
                EquipmentAbilityFamily::PointedBlade => a(value, 0, 0, 0),
                EquipmentAbilityFamily::SharpBlade => a(0, value, 0, 0),
                EquipmentAbilityFamily::Intelligence => a(0, 0, value, 0),
                EquipmentAbilityFamily::MagicResistance => a(0, 0, 0, value),
                EquipmentAbilityFamily::WeaponDelay => EquipmentValues::default(),
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
            id, name, EquipmentAbilityFamily::WeaponDelay, 3,
            EquipmentValues::default(), summary, true,
        ));
    }

    for (id, name, value) in [
        ("ancient-pointed-blade", "古代精霊の尖った刃", 11), ("abyss-pointed-blade", "深淵の尖った刃", 13),
        ("loss-pointed-blade", "喪失の尖った刃", 17), ("night-star-pointed-blade", "夜星の尖った刃", 20),
    ] { out.push(new_ability(id, name, EquipmentAbilityFamily::PointedBlade, a(value, 0, 0, 0), match value { 11 => "突き +11", 13 => "突き +13", 17 => "突き +17", _ => "突き +20" })); }
    for (id, name, value) in [
        ("ancient-sharp-blade", "古代精霊の鋭い刃", 11), ("abyss-sharp-blade", "深淵の鋭い刃", 13),
        ("loss-sharp-blade", "喪失の鋭い刃", 17), ("night-star-sharp-blade", "夜星の鋭い刃", 20),
    ] { out.push(new_ability(id, name, EquipmentAbilityFamily::SharpBlade, a(0, value, 0, 0), match value { 11 => "斬り +11", 13 => "斬り +13", 17 => "斬り +17", _ => "斬り +20" })); }
    for (id, name, value) in [
        ("ancient-intelligence", "古代精霊の知力", 11), ("abyss-intelligence", "深淵の知力", 13),
        ("loss-intelligence", "喪失の知力", 17), ("night-star-intelligence", "夜星の知力", 20),
    ] { out.push(new_ability(id, name, EquipmentAbilityFamily::Intelligence, a(0, 0, value, 0), match value { 11 => "魔攻 +11", 13 => "魔攻 +13", 17 => "魔攻 +17", _ => "魔攻 +20" })); }
    for (id, name, value) in [
        ("ancient-magic-resistance", "古代精霊の耐魔力", 11), ("abyss-magic-resistance", "深淵の耐魔力", 13),
        ("loss-magic-resistance", "喪失の耐魔力", 17), ("night-star-magic-resistance", "夜星の耐魔力", 20),
    ] { out.push(new_ability(id, name, EquipmentAbilityFamily::MagicResistance, a(0, 0, 0, value), match value { 11 => "魔防 +11", 13 => "魔防 +13", 17 => "魔防 +17", _ => "魔防 +20" })); }
    out
}

#[cfg(test)]
mod tests {
    use domain::DamageCategory;

    /// 追加アビリティは抽選結果なので、登録した基本アビリティから自動適用しない。
    #[test]
    fn 追加アビリティは説明だけを持ち自動計算しない() {
        for def in equipment_abilities().into_iter().filter(|d| d.category == 4) {
            assert!(def.damage_effects.is_empty(), "{}", def.id);
            assert_eq!(def.additional_slots, 2, "{}", def.id);
            assert!(def.additional_effects.contains("ランダム"), "{}", def.id);
        }
    }

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn カタログは51件_idは重複しない() {
        let catalog = equipment_catalog();
        // エンドゲーム帯 22 件 + 装着時効果つき 29 件
        assert_eq!(catalog.len(), 51);
        let ids: HashSet<&str> = catalog.iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), catalog.len());
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
            ("memorial-crest-wind", DamageCategory::AttackDamageSpecial, 3.0),
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
        let with_effects =
            equipment_catalog().iter().filter(|i| !i.damage_effects.is_empty()).count();
        assert_eq!(with_effects, 29);
    }

    /// 装備中のアイテムだけが寄与する。カテゴリ側の上限は `CategoryTotals` が掛けるので、
    /// ここでは Σ% の小数表現がそのまま出る。
    #[test]
    fn item_damage_contributionsは装備中のアイテムだけを見る() {
        let mut equipment = Equipment::default();
        assert!(item_damage_contributions(&equipment).is_empty());

        equipment.parts.hand.item_id = Some("gorilla-armcover".to_string());
        equipment.parts.body.item_id = Some("archangel-wing".to_string());
        equipment.parts.effect.item_id = Some("beast-unicorn".to_string());
        // カタログに無い id は無視する(保存時に storage が弾いている)
        equipment.parts.helm.item_id = Some("unknown".to_string());

        let mut got = item_damage_contributions(&equipment);
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
    fn 武器アビリティはカテゴリー1_3_4の37件_idは重複しない() {
        let abilities = equipment_abilities();
        assert_eq!(abilities.len(), 37);
        let ids: HashSet<&str> = abilities.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), abilities.len());
    }

    /// カテゴリー4は新装着アビリティ4系統各4件。
    #[test]
    fn アビリティは4系統各4件で記録値と追加枠情報を持つ() {
        use domain::EquipmentAbilityFamily::*;
        let abilities = equipment_abilities();
        for family in [PointedBlade, SharpBlade, Intelligence, MagicResistance] {
            let members: Vec<_> = abilities.iter().filter(|a| a.category == 4 && a.family == family).collect();
            assert_eq!(members.len(), 4, "{family:?} は 4 件");
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
                assert!(def.damage_effects.is_empty(), "追加アビリティは自動適用しない");
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
        let def = abilities.iter().find(|a| a.id == "night-star-pointed-blade").unwrap();
        assert_eq!(def.name, "夜星の尖った刃");
        assert_eq!(def.values, a(20, 0, 0, 0));
        use domain::EquipmentAbilityAdditionalKind::*;
        assert_eq!(def.additional_options.iter().find(|o| o.kind == FixedDamage).map(|o| (o.min, o.max)), Some((10_000, 10_000)));
        assert_eq!(def.additional_options.iter().find(|o| o.kind == DamageRate).map(|o| (o.min, o.max)), Some((11, 11)));
        assert_eq!(def.additional_options.iter().find(|o| o.kind == Thrust).map(|o| (o.min, o.max)), Some((9, 18)));
        assert_eq!(def.additional_options.iter().find(|o| o.kind == Accuracy).map(|o| (o.min, o.max)), Some((10, 16)));
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
        let def = abilities.iter().find(|a| a.id == "ancient-magic-resistance").unwrap();
        assert_eq!(def.name, "古代精霊の耐魔力");
        assert_eq!(def.values, a(0, 0, 0, 11));
    }

    #[test]
    fn 強化等級はwiki確率区分の上端を四捨五入する() {
        use domain::EnhanceGrade::*;
        assert_eq!([Lowest, Low, Middle, High, Highest].map(|grade| enhance_grade_multiplier(15, grade).unwrap()), [700.0, 740.0, 820.0, 870.0, 880.0]);
        assert_eq!([Lowest, Low, Middle, High, Highest].map(|grade| armor_enhance_multiplier(15, Some(grade)).unwrap()), [350.0, 370.0, 410.0, 435.0, 440.0]);
    }
}
