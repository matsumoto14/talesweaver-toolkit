//! 部位別の装備アイテムカタログ(wiki: `Link/装備Item` から辿れる各 Item ページ)。
//! wiki 抽出ぶんは `generated.rs`、韓国コミュニティ資料ぶんは `sacred_kr.rs` が持ち、
//! ここで手書きぶんと突き合わせて 1 本のカタログに畳む。

use super::*;

#[path = "generated.rs"]
mod generated;
#[path = "sacred_kr.rs"]
mod sacred_kr;

/// 装備カタログの出典。
pub const EQUIPMENT_CATALOG_SOURCE: Source = Source {
    page: "Link/装備Item とリンク先の部位別 Item ページ",
    retrieved_on: "2026-08-27",
    note: "各ページで最後のインファーナルより後。数値未確定行は除外",
};

pub(super) fn wrist_type_from_page(page: &str) -> Option<WristType> {
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

/// カタログ品の補正式。鎧は出典文字列から推測せず、アイテムIDへ明示的に割り当てる。
pub fn equipment_enhance_type(item_id: &str) -> Option<EquipmentEnhanceType> {
    let item = find_equipment_item(item_id)?;
    if let Some(class) = item.weapon_class {
        return Some(class.system().enhance_type());
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
    /// 鎧のみ `Some`(`armor_class_for_type(enhance_type)`)。UI がキャラの装備可能クラスと
    /// 突き合わせるための鎧分類(wiki: 装備システム/防具区分)。
    pub armor_class: Option<ArmorClass>,
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
    /// レリックなら系列と段。育成順序(段上げの可否)の判定に使う
    pub relic: Option<RelicInfo>,
    pub source: Source,
}

impl EquipmentItem {
    /// 武器系統。武器種から決まり、武器種を持たない収録品は装備強化の補正式から引く。
    pub fn weapon_system(&self) -> Option<WeaponSystem> {
        self.weapon_class.map(WeaponClass::system).or_else(|| {
            self.enhance_type
                .and_then(WeaponSystem::from_enhance_type)
        })
    }

    /// 装備強化の補正式。武器は武器種から決まるので、それを優先する。
    pub fn resolved_enhance_type(&self) -> Option<EquipmentEnhanceType> {
        self.weapon_class
            .map(|class| class.system().enhance_type())
            .or(self.enhance_type)
    }
}

/// `Equipment::validate_against_catalog`(domain)がカタログを検証できるようにする実装。
/// domain は gamedata に依存できないので、domain 側にトレイトを置いてこちらで実装する
/// (`base_totals` が `&[EquipmentAbilityDef]` を受ける依存方向と同じ)。
impl domain::EquipmentCatalogEntry for EquipmentItem {
    fn id(&self) -> &str {
        self.id
    }
    fn slot(&self) -> PartSlot {
        self.slot
    }
    fn ability_slots(&self) -> usize {
        self.ability_slots
    }
    fn random_option_slots(&self) -> Option<usize> {
        self.random_option_slots
    }
    fn values_min(&self) -> EquipmentValues {
        self.values_min
    }
    fn values_max(&self) -> EquipmentValues {
        self.values_max
    }
    fn growth_caps(&self) -> Option<EquipmentValues> {
        self.growth_caps
    }
    fn enchant_caps(&self) -> EquipmentValues {
        self.enchant_caps
    }
    fn weapon_class(&self) -> Option<WeaponClass> {
        self.weapon_class
    }
    fn enhance_type(&self) -> Option<EquipmentEnhanceType> {
        self.resolved_enhance_type()
    }
    fn relic(&self) -> Option<RelicInfo> {
        self.relic
    }
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
    /// AFなどの耐久側固有効果。
    survival_effects: &'static [EquipmentSurvivalEffect],
    /// 候補を主軸スキルへ絞るための推奨依存。
    recommended_dependency: Option<SkillDependency>,
    /// `damage_effects` がこの依存のスキルにだけ効く場合の条件。
    damage_dependency: Option<SkillDependency>,
    /// 神鳥レリックはアビリティ・付加オプション枠を持たない(ルナリアと違う特例)。
    no_ability_or_random_option_slots: bool,
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
        let growth_caps = match self.slot {
            PartSlot::RelicPendant | PartSlot::RelicBracelet => Some(self.values_max),
            _ => self
                .growth_cap
                .map(|cap| v(cap, cap, cap, cap, cap, cap, cap, cap, cap)),
        };
        EquipmentItem {
            id: self.id,
            slot: self.slot,
            name: self.name,
            values_min: self.values_min,
            values_max: self.values_max,
            growth_cap: self.growth_cap,
            growth_caps,
            ability_slots: if self.no_ability_or_random_option_slots {
                0
            } else {
                self.slot.ability_slots()
            },
            random_option_slots: if self.no_ability_or_random_option_slots {
                None
            } else {
                self.slot.random_option_slots()
            },
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
            armor_class: self.enhance_type.and_then(armor_class_for_type),
            damage_effects: self.damage_effects,
            survival_effects: self.survival_effects,
            recommended_dependency: self.recommended_dependency,
            damage_dependency: self.damage_dependency,
            relic: None,
            source: self.source,
        }
    }
}

pub(super) const SURVIVAL_MITIGATION_10: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DamageMitigation { percent: 10.0 }];
pub(super) const SURVIVAL_MITIGATION_15: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DamageMitigation { percent: 15.0 }];
pub(super) const SURVIVAL_MITIGATION_40: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DamageMitigation { percent: 40.0 }];
const SURVIVAL_DEFENSE_FIXED_15: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DefenseFixed { value: 15 }];
const SURVIVAL_DEFENSE_RATE_20: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DefenseRate { percent: 20.0 }];
pub(super) const SURVIVAL_DEFENSE_RATE_30: &[EquipmentSurvivalEffect] =
    &[EquipmentSurvivalEffect::DefenseRate { percent: 30.0 }];

impl serde::Serialize for EquipmentItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("EquipmentItem", 21)?;
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
        s.serialize_field("weapon_system", &self.weapon_system())?;
        s.serialize_field("enhance_type", &self.resolved_enhance_type())?;
        s.serialize_field("relic", &self.relic)?;
        s.serialize_field("armor_class", &self.armor_class)?;
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
pub(super) fn v(
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
        no_ability_or_random_option_slots: false,
        survival_effects: &[],
        recommended_dependency: None,
        damage_dependency: None,
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
        no_ability_or_random_option_slots: false,
        survival_effects: &[],
        recommended_dependency: None,
        damage_dependency: None,
        source: ITEM_SOURCE_STALLION_EFFECT,
        ..effect_item(id, name, values, enchant_total_caps, &[])
    }
}

#[allow(clippy::too_many_arguments)]
fn defensio_artifact(
    id: &'static str,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_total_caps: EquipmentValues,
    damage_effects: &'static [SkillEffect],
    survival_effects: &'static [EquipmentSurvivalEffect],
    recommended_dependency: Option<SkillDependency>,
    damage_dependency: Option<SkillDependency>,
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
        no_ability_or_random_option_slots: false,
        survival_effects,
        recommended_dependency,
        damage_dependency,
        source: Source {
            page: "Item/アクセサリー用装備/アーティファクト",
            retrieved_on: "2026-08-27",
            note,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn artifact_item(
    id: &'static str,
    name: &'static str,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_total_caps: EquipmentValues,
    damage_effects: &'static [SkillEffect],
    survival_effects: &'static [EquipmentSurvivalEffect],
    recommended_dependency: Option<SkillDependency>,
    damage_dependency: Option<SkillDependency>,
    note: &'static str,
) -> WikiEquipmentItem {
    defensio_artifact(
        id,
        name,
        values_min,
        values_max,
        enchant_total_caps,
        damage_effects,
        survival_effects,
        recommended_dependency,
        damage_dependency,
        note,
    )
}

/// 神鳥・ルナリアレリック。各段階は直前段階の完成値から始まり、表の値まで成長する。
#[allow(clippy::too_many_arguments)]
fn relic_item(
    id: &'static str,
    name: &'static str,
    slot: PartSlot,
    min_main: i64,
    min_sub: i64,
    max_main: i64,
    max_sub: i64,
    kind: RelicKind,
    level: u8,
) -> (WikiEquipmentItem, RelicInfo) {
    let values = |main: i64, sub: i64| match slot {
        PartSlot::RelicPendant => v(main, main, 0, main, 0, sub, sub, 0, 0),
        PartSlot::RelicBracelet => v(0, 0, main, 0, main, 0, 0, sub, sub),
        _ => unreachable!("レリック以外の部位が指定されました"),
    };
    // 神鳥レリックはアビリティ・付加オプション枠を持たない(ルナリアレリックは持つ)。
    let no_ability_or_random_option_slots = kind == RelicKind::Godbird;
    let item = WikiEquipmentItem {
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
        no_ability_or_random_option_slots,
        survival_effects: &[],
        recommended_dependency: None,
        damage_dependency: None,
        source: Source {
            page: "Item/アクセサリ/レリック/神鳥のレリック・ルナリアレリック",
            retrieved_on: "2026-08-28",
            note: "直前段階の全補正MAXから開始し、表示段階のMAXまでランダム成長。エンチャント不可",
        },
    };
    (item, RelicInfo { kind, level })
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
        no_ability_or_random_option_slots: false,
        survival_effects: &[],
        recommended_dependency: None,
        damage_dependency: None,
        source: ITEM_SOURCE_DAMAGE_EFFECT,
    }
}

/// 装備カタログ。エンドゲーム帯 20 件 +「装着時に与ダメージが上がる」装備 19 件。
/// 後者は装備補正値だけでなく `damage_effects` を持ち、与ダメージ式のカテゴリに入る。
///
/// 静的データなので `build_equipment_catalog` の結果をプロセス内で 1 回だけ組み立て、
/// 以降は複製を返す(`find_equipment_item` など呼び出し頻度の高い箇所からの再構築を避ける)。
pub fn equipment_catalog() -> Vec<EquipmentItem> {
    cached_equipment_catalog().to_vec()
}

fn cached_equipment_catalog() -> &'static [EquipmentItem] {
    static CACHE: std::sync::OnceLock<Vec<EquipmentItem>> = std::sync::OnceLock::new();
    CACHE.get_or_init(build_equipment_catalog)
}

fn build_equipment_catalog() -> Vec<EquipmentItem> {
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
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
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_30,
            recommended_dependency: Some(SkillDependency::Stab),
            damage_dependency: Some(SkillDependency::Stab),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。突き依存+30%は同系列規則から補完" },
        },
        WikiEquipmentItem {
            id: "eclipse-hack-def", slot: PartSlot::Artifact,
            name: "†エクリプスの斬力 - ディフェンシオ",
            values_min: v(0, 170, 25, 0, 25, 25, 25, 25, 25),
            values_max: v(0, 190, 35, 0, 35, 35, 35, 35, 35), growth_cap: None,
            enchant_total_caps: v(0, 220, 55, 0, 55, 55, 55, 55, 55),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_30,
            recommended_dependency: Some(SkillDependency::Hack),
            damage_dependency: Some(SkillDependency::Hack),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。斬り攻撃ダメージ+30%" },
        },
        WikiEquipmentItem {
            id: "eclipse-int", slot: PartSlot::Artifact, name: "†エクリプスの魔力",
            values_min: v(0, 0, 20, 150, 20, 20, 20, 20, 20),
            values_max: v(0, 0, 30, 170, 30, 30, 30, 30, 30), growth_cap: None,
            enchant_total_caps: v(0, 0, 50, 200, 50, 50, 50, 50, 50),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_MITIGATION_10,
            recommended_dependency: Some(SkillDependency::Int),
            damage_dependency: Some(SkillDependency::Int),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。魔法攻撃ダメージ+30%" },
        },
        WikiEquipmentItem {
            id: "eclipse-mr-def", slot: PartSlot::Artifact,
            name: "†エクリプスの魔防力 - ディフェンシオ",
            values_min: v(0, 0, 25, 25, 170, 25, 25, 25, 25),
            values_max: v(0, 0, 35, 35, 190, 35, 35, 35, 35), growth_cap: None,
            enchant_total_caps: v(0, 0, 55, 55, 220, 55, 55, 55, 55),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_30,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_30,
            recommended_dependency: Some(SkillDependency::Mr),
            damage_dependency: Some(SkillDependency::Mr),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "喪失の島。MR系攻撃ダメージ+30%" },
        },
        WikiEquipmentItem {
            id: "dungeon-meshi-picking-tools", slot: PartSlot::Artifact, name: "†ピッキングツール",
            values_min: v(115, 0, 30, 0, 20, 18, 15, 15, 18),
            values_max: v(135, 0, 30, 0, 30, 25, 20, 20, 25), growth_cap: None,
            enchant_total_caps: v(170, 0, 30, 0, 30, 25, 25, 25, 25),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_20,
            recommended_dependency: Some(SkillDependency::Stab),
            damage_dependency: None,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ダンジョン飯タイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "dungeon-meshi-gourmet-guide", slot: PartSlot::Artifact, name: "†迷宮グルメガイド",
            values_min: v(0, 115, 30, 0, 20, 18, 15, 15, 18),
            values_max: v(0, 135, 30, 0, 30, 25, 20, 20, 25), growth_cap: None,
            enchant_total_caps: v(0, 170, 30, 0, 30, 25, 25, 25, 25),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_20,
            recommended_dependency: Some(SkillDependency::Hack),
            damage_dependency: None,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ダンジョン飯タイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "dungeon-meshi-thistle-book", slot: PartSlot::Artifact, name: "†シスルの魔術書",
            values_min: v(0, 0, 30, 115, 30, 15, 18, 18, 15),
            values_max: v(0, 0, 30, 135, 30, 20, 25, 25, 20), growth_cap: None,
            enchant_total_caps: v(0, 0, 30, 170, 30, 25, 25, 25, 25),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_20,
            recommended_dependency: Some(SkillDependency::Int),
            damage_dependency: None,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ダンジョン飯タイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "maid-dragon-magic-orb", slot: PartSlot::Artifact, name: "†魔力の玉",
            values_min: v(0, 0, 30, 90, 90, 39, 19, 44, 33),
            values_max: v(0, 0, 30, 103, 103, 39, 19, 44, 33), growth_cap: None,
            enchant_total_caps: v(0, 0, 30, 130, 130, 39, 19, 44, 33),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_20,
            recommended_dependency: Some(SkillDependency::Mr),
            damage_dependency: None,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "メイドラゴンタイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "log-horizon-akatsuki-doll", slot: PartSlot::Artifact, name: "†アカツキ人形",
            values_min: v(90, 90, 30, 0, 30, 23, 23, 23, 23),
            values_max: v(103, 103, 50, 0, 50, 25, 25, 25, 25), growth_cap: None,
            enchant_total_caps: v(130, 130, 70, 0, 70, 49, 49, 49, 49),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_SPECIAL_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_20,
            recommended_dependency: Some(SkillDependency::StabHack),
            damage_dependency: None,
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "ログ・ホライズンタイアップ。一定確率でダメージ+20%" },
        },
        WikiEquipmentItem {
            id: "arklon-hack-int", slot: PartSlot::Artifact, name: "†アークロンの魔斬力",
            values_min: v(0, 80, 18, 80, 18, 13, 13, 23, 13),
            values_max: v(0, 100, 21, 100, 21, 14, 15, 25, 14), growth_cap: None,
            enchant_total_caps: v(0, 130, 45, 130, 45, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_MITIGATION_10,
            recommended_dependency: Some(SkillDependency::HackInt),
            damage_dependency: Some(SkillDependency::HackInt),
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
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_30,
            recommended_dependency: Some(SkillDependency::StabHack),
            damage_dependency: Some(SkillDependency::StabHack),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。物理複合攻撃ダメージ+20%、ディフェンシオ。上限の欠落セルは同補正のリストア/スピーディーと一致" },
        },
        WikiEquipmentItem {
            id: "arklon-int-def", slot: PartSlot::Artifact,
            name: "†アークロンの魔力 - ディフェンシオ",
            values_min: v(0, 0, 22, 110, 24, 13, 13, 23, 13),
            values_max: v(0, 0, 25, 130, 27, 14, 15, 25, 14), growth_cap: None,
            enchant_total_caps: v(0, 0, 49, 160, 51, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_30,
            recommended_dependency: Some(SkillDependency::Int),
            damage_dependency: Some(SkillDependency::Int),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。魔法攻撃ダメージ+20%、ディフェンシオ" },
        },
        WikiEquipmentItem {
            id: "arklon-hack-int-def", slot: PartSlot::Artifact,
            name: "†アークロンの魔斬力 - ディフェンシオ",
            values_min: v(0, 90, 22, 90, 22, 13, 13, 23, 13),
            values_max: v(0, 110, 25, 110, 25, 14, 15, 25, 14), growth_cap: None,
            enchant_total_caps: v(0, 140, 49, 140, 49, 38, 21, 49, 38),
            weapon_class: None, enhance_type: None, damage_effects: ITEM_DAMAGE_DEPENDENCY_20,
            no_ability_or_random_option_slots: false,
            survival_effects: SURVIVAL_DEFENSE_RATE_30,
            recommended_dependency: Some(SkillDependency::HackInt),
            damage_dependency: Some(SkillDependency::HackInt),
            source: Source { page: "Item/アクセサリー用装備/アーティファクト", retrieved_on: "2026-08-27", note: "アークロン要塞。魔法斬り攻撃ダメージ+20%、ディフェンシオ" },
        },

        // ── AF: プシーキー / エクリプス / エーテリアルの6依存×通常・ディフェンシオ ──
        artifact_item("psyche-stab", "†プシーキーの突力",
            v(63, 0, 14, 0, 14, 13, 13, 23, 13), v(66, 0, 17, 0, 17, 14, 15, 25, 14),
            v(90, 0, 41, 0, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Stab), Some(SkillDependency::Stab),
            "リンゴの島。突き攻撃ダメージ+20%"),
        artifact_item("psyche-hack", "†プシーキーの斬力",
            v(0, 63, 14, 0, 14, 13, 13, 23, 13), v(0, 66, 17, 0, 17, 14, 15, 25, 14),
            v(0, 90, 41, 0, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Hack), Some(SkillDependency::Hack),
            "リンゴの島。斬り攻撃ダメージ+20%"),
        artifact_item("psyche-physical", "†プシーキーの物理力",
            v(41, 41, 14, 0, 14, 13, 13, 23, 13), v(44, 44, 17, 0, 17, 14, 15, 25, 14),
            v(68, 68, 41, 0, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::StabHack), Some(SkillDependency::StabHack),
            "リンゴの島。物理複合攻撃ダメージ+20%"),
        artifact_item("psyche-int", "†プシーキーの魔力",
            v(0, 0, 14, 63, 16, 13, 13, 23, 13), v(0, 0, 17, 66, 19, 14, 15, 25, 14),
            v(0, 0, 41, 90, 43, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Int), Some(SkillDependency::Int),
            "リンゴの島。魔法攻撃ダメージ+20%"),
        artifact_item("psyche-mr", "†プシーキーの魔防力",
            v(0, 0, 14, 19, 63, 13, 13, 23, 13), v(0, 0, 17, 22, 66, 14, 15, 25, 14),
            v(0, 0, 41, 46, 90, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Mr), Some(SkillDependency::Mr),
            "リンゴの島。MR系攻撃ダメージ+20%"),
        artifact_item("psyche-hack-int", "†プシーキーの魔斬力",
            v(0, 53, 14, 53, 14, 13, 13, 23, 13), v(0, 58, 17, 58, 17, 14, 15, 25, 14),
            v(0, 82, 41, 82, 41, 38, 21, 49, 38), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::HackInt), Some(SkillDependency::HackInt),
            "リンゴの島。魔法斬り攻撃ダメージ+20%"),

        artifact_item("eclipse-stab", "†エクリプスの突力",
            v(150, 0, 20, 0, 20, 20, 20, 20, 20), v(170, 0, 30, 0, 30, 30, 30, 30, 30),
            v(200, 0, 50, 0, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Stab), Some(SkillDependency::Stab),
            "喪失の島。突き依存+30%は同系列規則から補完"),
        artifact_item("eclipse-hack", "†エクリプスの斬力",
            v(0, 150, 20, 0, 20, 20, 20, 20, 20), v(0, 170, 30, 0, 30, 30, 30, 30, 30),
            v(0, 200, 50, 0, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Hack), Some(SkillDependency::Hack),
            "喪失の島。斬り攻撃ダメージ+30%"),
        artifact_item("eclipse-physical", "†エクリプスの物理力",
            v(120, 120, 20, 0, 20, 20, 20, 20, 20), v(140, 140, 30, 0, 30, 30, 30, 30, 30),
            v(170, 170, 50, 0, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::StabHack), Some(SkillDependency::StabHack),
            "喪失の島。上限と物理複合依存+30%は同系列規則から補完"),
        artifact_item("eclipse-mr", "†エクリプスの魔防力",
            v(0, 0, 20, 20, 150, 20, 20, 20, 20), v(0, 0, 30, 30, 170, 30, 30, 30, 30),
            v(0, 0, 50, 50, 200, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::Mr), Some(SkillDependency::Mr),
            "喪失の島。MR系攻撃ダメージ+30%"),
        artifact_item("eclipse-hack-int", "†エクリプスの魔斬力",
            v(0, 130, 20, 130, 20, 20, 20, 20, 20), v(0, 150, 30, 150, 30, 30, 30, 30, 30),
            v(0, 180, 50, 180, 50, 50, 50, 50, 50), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_MITIGATION_10, Some(SkillDependency::HackInt), Some(SkillDependency::HackInt),
            "喪失の島。上限と魔斬依存+30%は同系列規則から補完"),

        artifact_item("ethereal-stab", "†エーテリアルチューブ(突力)",
            v(210, 0, 30, 0, 30, 30, 30, 30, 30), v(230, 0, 40, 0, 40, 40, 40, 40, 40),
            v(260, 0, 60, 0, 60, 60, 60, 60, 60), &[],
            SURVIVAL_MITIGATION_15, Some(SkillDependency::Stab), None,
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-hack", "†エーテリアルチューブ(斬力)",
            v(0, 210, 30, 0, 30, 30, 30, 30, 30), v(0, 230, 40, 0, 40, 40, 40, 40, 40),
            v(0, 260, 60, 0, 60, 60, 60, 60, 60), &[],
            SURVIVAL_MITIGATION_15, Some(SkillDependency::Hack), None,
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-physical", "†エーテリアルチューブ(物理力)",
            v(190, 190, 30, 0, 30, 30, 30, 30, 30), v(210, 210, 40, 0, 40, 40, 40, 40, 40),
            v(240, 240, 60, 0, 60, 60, 60, 60, 60), &[],
            SURVIVAL_MITIGATION_15, Some(SkillDependency::StabHack), None,
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-int", "†エーテリアルチューブ(魔力)",
            v(0, 0, 30, 210, 30, 30, 30, 30, 30), v(0, 0, 40, 230, 40, 40, 40, 40, 40),
            v(0, 0, 60, 260, 60, 60, 60, 60, 60), &[],
            SURVIVAL_MITIGATION_15, Some(SkillDependency::Int), None,
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-mr", "†エーテリアルチューブ(魔防力)",
            v(0, 0, 30, 30, 210, 30, 30, 30, 30), v(0, 0, 40, 40, 230, 40, 40, 40, 40),
            v(0, 0, 60, 60, 260, 60, 60, 60, 60), &[],
            SURVIVAL_MITIGATION_15, Some(SkillDependency::Mr), None,
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),
        artifact_item("ethereal-hack-int", "†エーテリアルチューブ(魔斬力)",
            v(0, 190, 30, 190, 30, 30, 30, 30, 30), v(0, 210, 40, 210, 40, 40, 40, 40, 40),
            v(0, 240, 60, 240, 60, 60, 60, 60, 60), &[],
            SURVIVAL_MITIGATION_15, Some(SkillDependency::HackInt), None,
            "ゆがんだ村。上限は同系列規則。通常版の依存倍率はWikiが??のため未計算"),

        defensio_artifact("psyche-stab-def", "†プシーキーの突力 - ディフェンシオ",
            v(69, 0, 20, 0, 20, 16, 16, 26, 16), v(72, 0, 23, 0, 23, 17, 18, 28, 17),
            v(96, 0, 47, 0, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_DEFENSE_FIXED_15, Some(SkillDependency::Stab), Some(SkillDependency::Stab),
            "リンゴの島。突き攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-hack-def", "†プシーキーの斬力 - ディフェンシオ",
            v(0, 69, 20, 0, 20, 16, 16, 26, 16), v(0, 72, 23, 0, 23, 17, 18, 28, 17),
            v(0, 96, 47, 0, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_DEFENSE_FIXED_15, Some(SkillDependency::Hack), Some(SkillDependency::Hack),
            "リンゴの島。斬り攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-physical-def", "†プシーキーの物理力 - ディフェンシオ",
            v(47, 47, 20, 0, 20, 16, 16, 26, 16), v(50, 50, 23, 0, 23, 17, 18, 28, 17),
            v(74, 74, 47, 0, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_DEFENSE_FIXED_15, Some(SkillDependency::StabHack), Some(SkillDependency::StabHack),
            "リンゴの島。物理複合攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-int-def", "†プシーキーの魔力 - ディフェンシオ",
            v(0, 0, 20, 69, 22, 16, 16, 26, 16), v(0, 0, 23, 72, 25, 17, 18, 28, 17),
            v(0, 0, 47, 96, 49, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_DEFENSE_FIXED_15, Some(SkillDependency::Int), Some(SkillDependency::Int),
            "リンゴの島。魔法攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-mr-def", "†プシーキーの魔防力 - ディフェンシオ",
            v(0, 0, 20, 25, 69, 16, 16, 26, 16), v(0, 0, 23, 28, 72, 17, 18, 28, 17),
            v(0, 0, 47, 52, 96, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_DEFENSE_FIXED_15, Some(SkillDependency::Mr), Some(SkillDependency::Mr),
            "リンゴの島。MR系攻撃ダメージ+20%、ディフェンシオ"),
        defensio_artifact("psyche-hack-int-def", "†プシーキーの魔斬力 - ディフェンシオ",
            v(0, 61, 20, 61, 20, 16, 16, 26, 16), v(0, 64, 23, 64, 23, 17, 18, 28, 17),
            v(0, 88, 47, 88, 47, 41, 24, 52, 41), ITEM_DAMAGE_DEPENDENCY_20,
            SURVIVAL_DEFENSE_FIXED_15, Some(SkillDependency::HackInt), Some(SkillDependency::HackInt),
            "リンゴの島。魔法斬り攻撃ダメージ+20%、ディフェンシオ"),

        defensio_artifact("eclipse-physical-def", "†エクリプスの物理力 - ディフェンシオ",
            v(140, 140, 25, 0, 25, 25, 25, 25, 25), v(160, 160, 35, 0, 35, 35, 35, 35, 35),
            v(190, 190, 55, 0, 55, 55, 55, 55, 55), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_DEFENSE_RATE_30, Some(SkillDependency::StabHack), Some(SkillDependency::StabHack),
            "喪失の島。補正と物理複合依存+30%は同系列規則から補完"),
        defensio_artifact("eclipse-int-def", "†エクリプスの魔力 - ディフェンシオ",
            v(0, 0, 25, 170, 25, 25, 25, 25, 25), v(0, 0, 35, 190, 35, 35, 35, 35, 35),
            v(0, 0, 55, 220, 55, 55, 55, 55, 55), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_DEFENSE_RATE_30, Some(SkillDependency::Int), Some(SkillDependency::Int),
            "喪失の島。副補正上限と魔攻依存+30%は同系列規則から補完"),
        defensio_artifact("eclipse-hack-int-def", "†エクリプスの魔斬力 - ディフェンシオ",
            v(0, 150, 25, 150, 25, 25, 25, 25, 25), v(0, 170, 35, 170, 35, 35, 35, 35, 35),
            v(0, 200, 55, 200, 55, 55, 55, 55, 55), ITEM_DAMAGE_DEPENDENCY_30,
            SURVIVAL_DEFENSE_RATE_30, Some(SkillDependency::HackInt), Some(SkillDependency::HackInt),
            "喪失の島。補正と魔斬依存+30%は同系列規則から補完"),

        defensio_artifact("ethereal-stab-def", "†エーテリアルチューブ(突力) - ディフェンシオ",
            v(230, 0, 35, 0, 35, 35, 35, 35, 35), v(250, 0, 45, 0, 45, 45, 45, 45, 45),
            v(280, 0, 65, 0, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            SURVIVAL_MITIGATION_40, Some(SkillDependency::Stab), Some(SkillDependency::Stab),
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。突き依存+35%"),
        defensio_artifact("ethereal-hack-def", "†エーテリアルチューブ(斬力) - ディフェンシオ",
            v(0, 230, 35, 0, 35, 35, 35, 35, 35), v(0, 250, 45, 0, 45, 45, 45, 45, 45),
            v(0, 280, 65, 0, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            SURVIVAL_MITIGATION_40, Some(SkillDependency::Hack), Some(SkillDependency::Hack),
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。斬り依存+35%"),
        defensio_artifact("ethereal-physical-def", "†エーテリアルチューブ(物理力) - ディフェンシオ",
            v(210, 210, 35, 0, 35, 35, 35, 35, 35), v(230, 230, 45, 0, 45, 45, 45, 45, 45),
            v(260, 260, 65, 0, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            SURVIVAL_MITIGATION_40, Some(SkillDependency::StabHack), Some(SkillDependency::StabHack),
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。物理複合依存+35%"),
        defensio_artifact("ethereal-int-def", "†エーテリアルチューブ(魔力) - ディフェンシオ",
            v(0, 0, 35, 230, 35, 35, 35, 35, 35), v(0, 0, 45, 250, 45, 45, 45, 45, 45),
            v(0, 0, 65, 280, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            SURVIVAL_MITIGATION_40, Some(SkillDependency::Int), Some(SkillDependency::Int),
            "ゆがんだ村。Wiki確定補正。魔攻依存+35%"),
        defensio_artifact("ethereal-mr-def", "†エーテリアルチューブ(魔防力) - ディフェンシオ",
            v(0, 0, 35, 35, 230, 35, 35, 35, 35), v(0, 0, 45, 45, 250, 45, 45, 45, 45),
            v(0, 0, 65, 65, 280, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            SURVIVAL_MITIGATION_40, Some(SkillDependency::Mr), Some(SkillDependency::Mr),
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。魔防依存+35%"),
        defensio_artifact("ethereal-hack-int-def", "†エーテリアルチューブ(魔斬力) - ディフェンシオ",
            v(0, 210, 35, 210, 35, 35, 35, 35, 35), v(0, 230, 45, 230, 45, 45, 45, 45, 45),
            v(0, 260, 65, 260, 65, 65, 65, 65, 65), ITEM_DAMAGE_DEPENDENCY_35,
            SURVIVAL_MITIGATION_40, Some(SkillDependency::HackInt), Some(SkillDependency::HackInt),
            "ゆがんだ村。補正上限は魔力ディフェンシオと同系列規則。魔斬依存+35%"),

    ];

    // 盾+ は通常の候補を並べず、ユーザー指定の成長カフスだけを扱う。
    catalog.retain(|item| item.slot != PartSlot::ShieldPlus);
    catalog.push(WikiEquipmentItem {
        id: "rising-holic-cuffs",
        slot: PartSlot::ShieldPlus,
        name: "†ライジングホリックカフス",
        values_min: v(1, 1, 1, 1, 1, 1, 1, 1, 1),
        values_max: v(1, 1, 1, 1, 1, 1, 1, 1, 1),
        growth_cap: Some(200),
        enchant_total_caps: EquipmentValues::default(),
        weapon_class: None,
        enhance_type: None,
        damage_effects: &[],
        no_ability_or_random_option_slots: false,
        survival_effects: &[],
        recommended_dependency: None,
        damage_dependency: None,
        source: Source {
            page: "Item/防具/腕/盾＋",
            retrieved_on: "2026-08-27",
            note: "成長コンテンツ。初期入力は全補正1、成長上限は全補正200。表示名はユーザー指定",
        },
    });

    // 既存の手検証済みデータ(装着時効果を含む)を優先し、同名の自動抽出行は足さない。
    for item in generated::wiki_equipment_catalog() {
        if !catalog.iter().any(|existing| existing.name == item.name) {
            catalog.push(item);
        }
    }
    // 日本 Tale Wiki で数値が確定している同名行を優先し、未収録のセイクリッド装備だけを
    // 韓国コミュニティ資料から補完する。
    for item in sacred_kr::sacred_equipment_catalog() {
        if !catalog.iter().any(|existing| existing.name == item.name) {
            catalog.push(item);
        }
    }
    let mut items: Vec<EquipmentItem> = catalog
        .into_iter()
        .map(WikiEquipmentItem::into_item)
        .collect();
    items.extend(relic_items());
    items
}

/// レリックは段(`RelicInfo`)を属性で持つ。段の上げ下げ(`domain::relic_step`)は
/// id の文字列ではなくこの属性を見る。
fn relic_items() -> Vec<EquipmentItem> {
    vec![
        // ── レリック: 直前段階の完成値から、選択段階の上限までランダム成長 ──
        relic_item("godbird-pendant-plus1", "†神鳥のペンダント(+1)", PartSlot::RelicPendant, 0, 0, 30, 25, RelicKind::Godbird, 1),
        relic_item("godbird-pendant-plus2", "†神鳥のペンダント(+2)", PartSlot::RelicPendant, 30, 25, 50, 45, RelicKind::Godbird, 2),
        relic_item("godbird-pendant-plus3", "†神鳥のペンダント(+3)", PartSlot::RelicPendant, 50, 45, 55, 50, RelicKind::Godbird, 3),
        relic_item("godbird-pendant-plus4", "†神鳥のペンダント(+4)", PartSlot::RelicPendant, 55, 50, 60, 60, RelicKind::Godbird, 4),
        relic_item("godbird-pendant-plus5", "†神鳥のペンダント(+5)", PartSlot::RelicPendant, 60, 60, 65, 65, RelicKind::Godbird, 5),
        relic_item("godbird-pendant-plus6", "†神鳥のペンダント(+6)", PartSlot::RelicPendant, 65, 65, 70, 70, RelicKind::Godbird, 6),
        relic_item("godbird-pendant-plus7", "†神鳥のペンダント(+7)", PartSlot::RelicPendant, 70, 70, 75, 75, RelicKind::Godbird, 7),
        relic_item("godbird-pendant-plus8", "†神鳥のペンダント(+8)", PartSlot::RelicPendant, 75, 75, 80, 80, RelicKind::Godbird, 8),
        relic_item("godbird-pendant-plus9", "†神鳥のペンダント(+9)", PartSlot::RelicPendant, 80, 80, 90, 90, RelicKind::Godbird, 9),
        relic_item("godbird-pendant-plus10", "†神鳥のペンダント(+10)", PartSlot::RelicPendant, 90, 90, 100, 100, RelicKind::Godbird, 10),
        relic_item("lunaria-pendant-plus1", "†ルナリアペンダント(+1)", PartSlot::RelicPendant, 100, 100, 110, 110, RelicKind::Lunaria, 1),
        relic_item("lunaria-pendant-plus2", "†ルナリアペンダント(+2)", PartSlot::RelicPendant, 110, 110, 120, 120, RelicKind::Lunaria, 2),
        relic_item("lunaria-pendant-plus3", "†ルナリアペンダント(+3)", PartSlot::RelicPendant, 120, 120, 130, 130, RelicKind::Lunaria, 3),
        relic_item("lunaria-pendant-plus4", "†ルナリアペンダント(+4)", PartSlot::RelicPendant, 130, 130, 140, 140, RelicKind::Lunaria, 4),
        relic_item("lunaria-pendant-plus5", "†ルナリアペンダント(+5)", PartSlot::RelicPendant, 140, 140, 150, 150, RelicKind::Lunaria, 5),
        relic_item("lunaria-pendant-plus6", "†ルナリアペンダント(+6)", PartSlot::RelicPendant, 150, 150, 160, 160, RelicKind::Lunaria, 6),
        relic_item("lunaria-pendant-plus7", "†ルナリアペンダント(+7)", PartSlot::RelicPendant, 160, 160, 170, 170, RelicKind::Lunaria, 7),
        relic_item("lunaria-pendant-plus8", "†ルナリアペンダント(+8)", PartSlot::RelicPendant, 170, 170, 180, 180, RelicKind::Lunaria, 8),
        relic_item("lunaria-pendant-plus9", "†ルナリアペンダント(+9)", PartSlot::RelicPendant, 180, 180, 190, 190, RelicKind::Lunaria, 9),
        relic_item("lunaria-pendant-plus10", "†ルナリアペンダント(+10)", PartSlot::RelicPendant, 190, 190, 200, 200, RelicKind::Lunaria, 10),

        relic_item("godbird-bracelet-plus1", "†神鳥のブレスレット(+1)", PartSlot::RelicBracelet, 0, 0, 30, 25, RelicKind::Godbird, 1),
        relic_item("godbird-bracelet-plus2", "†神鳥のブレスレット(+2)", PartSlot::RelicBracelet, 30, 25, 50, 45, RelicKind::Godbird, 2),
        relic_item("godbird-bracelet-plus3", "†神鳥のブレスレット(+3)", PartSlot::RelicBracelet, 50, 45, 55, 50, RelicKind::Godbird, 3),
        relic_item("godbird-bracelet-plus4", "†神鳥のブレスレット(+4)", PartSlot::RelicBracelet, 55, 50, 60, 60, RelicKind::Godbird, 4),
        relic_item("godbird-bracelet-plus5", "†神鳥のブレスレット(+5)", PartSlot::RelicBracelet, 60, 60, 65, 65, RelicKind::Godbird, 5),
        relic_item("godbird-bracelet-plus6", "†神鳥のブレスレット(+6)", PartSlot::RelicBracelet, 65, 65, 70, 70, RelicKind::Godbird, 6),
        relic_item("godbird-bracelet-plus7", "†神鳥のブレスレット(+7)", PartSlot::RelicBracelet, 70, 70, 75, 75, RelicKind::Godbird, 7),
        relic_item("godbird-bracelet-plus8", "†神鳥のブレスレット(+8)", PartSlot::RelicBracelet, 75, 75, 80, 80, RelicKind::Godbird, 8),
        relic_item("godbird-bracelet-plus9", "†神鳥のブレスレット(+9)", PartSlot::RelicBracelet, 80, 80, 90, 90, RelicKind::Godbird, 9),
        relic_item("godbird-bracelet-plus10", "†神鳥のブレスレット(+10)", PartSlot::RelicBracelet, 90, 90, 100, 100, RelicKind::Godbird, 10),
        relic_item("lunaria-bracelet-plus1", "†ルナリアブレスレット(+1)", PartSlot::RelicBracelet, 100, 100, 110, 110, RelicKind::Lunaria, 1),
        relic_item("lunaria-bracelet-plus2", "†ルナリアブレスレット(+2)", PartSlot::RelicBracelet, 110, 110, 120, 120, RelicKind::Lunaria, 2),
        relic_item("lunaria-bracelet-plus3", "†ルナリアブレスレット(+3)", PartSlot::RelicBracelet, 120, 120, 130, 130, RelicKind::Lunaria, 3),
        relic_item("lunaria-bracelet-plus4", "†ルナリアブレスレット(+4)", PartSlot::RelicBracelet, 130, 130, 140, 140, RelicKind::Lunaria, 4),
        relic_item("lunaria-bracelet-plus5", "†ルナリアブレスレット(+5)", PartSlot::RelicBracelet, 140, 140, 150, 150, RelicKind::Lunaria, 5),
        relic_item("lunaria-bracelet-plus6", "†ルナリアブレスレット(+6)", PartSlot::RelicBracelet, 150, 150, 160, 160, RelicKind::Lunaria, 6),
        relic_item("lunaria-bracelet-plus7", "†ルナリアブレスレット(+7)", PartSlot::RelicBracelet, 160, 160, 170, 170, RelicKind::Lunaria, 7),
        relic_item("lunaria-bracelet-plus8", "†ルナリアブレスレット(+8)", PartSlot::RelicBracelet, 170, 170, 180, 180, RelicKind::Lunaria, 8),
        relic_item("lunaria-bracelet-plus9", "†ルナリアブレスレット(+9)", PartSlot::RelicBracelet, 180, 180, 190, 190, RelicKind::Lunaria, 9),
        relic_item("lunaria-bracelet-plus10", "†ルナリアブレスレット(+10)", PartSlot::RelicBracelet, 190, 190, 200, 200, RelicKind::Lunaria, 10),
    ]
    .into_iter()
    .map(|(item, relic)| {
        let mut item = item.into_item();
        item.relic = Some(relic);
        item
    })
    .collect()
}


pub fn find_equipment_item(id: &str) -> Option<EquipmentItem> {
    cached_equipment_catalog()
        .iter()
        .copied()
        .find(|item| item.id == id)
}

/// `character_wrist_base_bonus` の材料(キャラのルール・バンド判定・腕合計値)だけを解決する。
///
/// 依存種別ごとに何度も呼ばず 1 回だけ材料を作り、複数の依存種別ぶんの変換を domain 側で
/// まとめて計算できるようにする(`evaluate_contents` のように依存種別が複数あるとき用)。
pub fn character_wrist_bonus_material(
    game_character_id: &str,
    equipment: &Equipment,
    catalog: &[EquipmentItem],
) -> domain::WristBonusMaterial {
    let Some(wrist) = equipment.parts.shield.selected() else {
        return domain::WristBonusMaterial::default();
    };
    let rule = crate::characters::find_character(game_character_id).and_then(|c| c.wrist_bonus);
    let is_band = wrist
        .item_id
        .as_deref()
        .and_then(|id| catalog.iter().find(|item| item.id == id))
        .is_some_and(|item| item.wrist_type == Some(WristType::Band));
    let siena_thrust = equipment
        .siena
        .shield
        .selected()
        .map(|entry| entry.aura.values().thrust)
        .unwrap_or(0);
    domain::WristBonusMaterial {
        rule,
        is_band,
        wrist_totals: wrist.base.add(wrist.enchant),
        siena_thrust,
        // どの依存種別で振り先を選ぶかは呼び出し側(commands.rs)が決める(キャラの主軸
        // スキルを使うかどうかは文脈依存のため、ここでは解決しない)。
        style_dependency_override: None,
    }
}

/// キャラ固有パッシブにより、腕装備の補正から「基本能力値」へ派生する装備補正。
///
/// どのキャラがどのルールか(`WristBonusRule`)は `characters::find_character` が持つデータ。
/// ここでは腕装備の選択状態とカタログから `WristType`(バンドかどうか)を解決し、
/// 実際の変換計算は `domain::wrist_base_bonus` に委ねる(元の `base` / `enchant` は変更しない)。
pub fn character_wrist_base_bonus(
    game_character_id: &str,
    base_stats: &BaseStats,
    style_dependency: SkillDependency,
    equipment: &Equipment,
    catalog: &[EquipmentItem],
) -> EquipmentValues {
    let material = character_wrist_bonus_material(game_character_id, equipment, catalog);
    domain::wrist_base_bonus(
        material.rule,
        material.is_band,
        base_stats,
        style_dependency,
        material.wrist_totals,
        material.siena_thrust,
    )
}

/// 装備しているアイテムそのものの装着時効果を、与ダメージ式のカテゴリ寄与に変換する。
/// 装備補正値は `Equipment::base_totals` が別に見る。
///
/// `Equipment::ability_damage_contributions` と同じ役割だが、`EquipmentItem` は
/// `Source` / `WeaponClass` を持つので domain ではなくこちら側にある。
pub fn item_damage_contributions(
    equipment: &Equipment,
    dependency: SkillDependency,
) -> Vec<domain::DamageContribution> {
    let catalog = equipment_catalog();
    let effects: Vec<(String, &'static SkillEffect)> = equipment
        .parts
        .iter()
        .into_iter()
        .filter_map(|(_, part)| part.item_id.as_deref())
        .filter_map(|id| catalog.iter().find(|item| item.id == id))
        .filter(|item| {
            item.damage_dependency
                .is_none_or(|required| required == dependency)
        })
        .flat_map(|item| {
            item.damage_effects
                .iter()
                .map(move |e| (item.name.to_string(), e))
        })
        .collect();
    domain::damage_contributions(effects.into_iter())
}
