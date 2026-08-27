//! 装備補正(wiki: カテゴリA の内訳「装備攻撃力」)。docs/damage-formula.md §4 A、§5(武器強化)。
//!
//! 装備は部位別(12 スロット)で持つ(docs/claude/goals/2026-08-24-equipment-parts.md)。
//! 「基本能力値」= 部位ごとの実測補正値 + 武器アビリティの加算。
//! 「強化能力値」= 部位ごとのエンチャント値 + シエナのオーラの能力値(武器/盾)+ テシスコア。

use crate::category::DamageCategory;
use crate::character_skill::{damage_contributions, SkillEffect};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::element::{Element, ElementValues};
use crate::random_option::{
    RandomOptionDef, RandomOptionError, RandomOptionSlot, RandomOptionTotals,
    RANDOM_OPTION_VALUE_MAX,
};
use crate::siena::{SienaAuras, SienaError};
use crate::stats::StatKind;
use crate::thesis_core::{CoreRegion, ThesisCoreError, ThesisCores};
use crate::title::{title_values, TitleDef};

/// 装備補正 9 種(wiki Item 各ページの列: 突き / 斬り / 物防 / 魔攻 / 魔防 / 命中 / Cri補正 / 回避 / 敏捷)。
/// 基本能力値・エンチャント値のどちらも同じ形。与ダメージ式に入るのは前半 4 種
/// (突き/斬り/魔攻/魔防)で、残りは防御側(§6)と命中・回避(§7)の入力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EquipmentValues {
    #[serde(default)]
    pub thrust: i64,
    #[serde(default)]
    pub slash: i64,
    #[serde(default)]
    pub physical_defense: i64,
    #[serde(default)]
    pub magic_attack: i64,
    #[serde(default)]
    pub magic_defense: i64,
    #[serde(default)]
    pub accuracy: i64,
    #[serde(default)]
    pub critical: i64,
    #[serde(default)]
    pub evasion: i64,
    #[serde(default)]
    pub agility: i64,
}

/// 装備補正 9 値の値域上限(wiki は装備ごとの「上限」行しか持たず、全装備共通の上限は
/// 未記載。カタログ外のカスタム入力に掛ける安全域)`[仮]`。
/// 実在の装備値は 3 桁に収まるので、9999 だと入力欄の桁が実態から離れる(ユーザー確認)。
pub const EQUIPMENT_VALUE_MAX: i64 = 1000;
/// 装備強化の Lv 上限(wiki: 装備システム/装備強化。+1〜+15)。
pub const ENHANCE_LEVEL_MAX: u8 = 15;
/// 武器に追加できる装着アビリティのスロット数。
/// 出典: Item/合成/装着アビリティシステム「種別 / スロット数」(武器 = 3)。
pub const WEAPON_ABILITY_SLOTS: usize = 3;
/// +12 以上で追加固定ダメージがレンジ振り(MR)になる境界(wiki: +11 覚醒までは確定値)。
pub const ENHANCE_LEVEL_RANDOM_RANGE_MIN: u8 = 12;
/// +12〜+15 の追加固定ダメージ等級。各等級は wiki の確率区分
/// (10/30/70/95/100%)の上端を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnhanceGrade { Lowest, Low, Middle, High, Highest }

impl EnhanceGrade {
    pub fn percentile(self) -> f64 {
        match self { Self::Lowest => 0.10, Self::Low => 0.30, Self::Middle => 0.70, Self::High => 0.95, Self::Highest => 1.0 }
    }
}

impl EquipmentValues {
    /// (表示名, 値)の 9 組。検証・UI ラベル・合計表示の唯一の並び順にする。
    pub fn fields(&self) -> [(&'static str, i64); 9] {
        [
            ("突き攻撃力", self.thrust),
            ("斬り攻撃力", self.slash),
            ("物理防御力", self.physical_defense),
            ("魔法攻撃力", self.magic_attack),
            ("魔法防御力", self.magic_defense),
            ("命中率補正", self.accuracy),
            ("クリティカル補正", self.critical),
            ("回避率補正", self.evasion),
            ("敏捷度補正", self.agility),
        ]
    }

    fn validate(&self) -> Result<(), EquipmentError> {
        for (field, value) in self.fields() {
            if !(0..=EQUIPMENT_VALUE_MAX).contains(&value) {
                return Err(EquipmentError::ValueOutOfRange { field, value, max: EQUIPMENT_VALUE_MAX });
            }
        }
        Ok(())
    }

    pub fn add(self, other: EquipmentValues) -> EquipmentValues {
        EquipmentValues {
            thrust: self.thrust + other.thrust,
            slash: self.slash + other.slash,
            physical_defense: self.physical_defense + other.physical_defense,
            magic_attack: self.magic_attack + other.magic_attack,
            magic_defense: self.magic_defense + other.magic_defense,
            accuracy: self.accuracy + other.accuracy,
            critical: self.critical + other.critical,
            evasion: self.evasion + other.evasion,
            agility: self.agility + other.agility,
        }
    }
}

/// 装備部位(wiki: 装備システム ページ冒頭の表。9 部位 + 効果/AF/レリック)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartSlot {
    Weapon,
    Armor,
    Helm,
    Shield,
    ShieldPlus,
    Head,
    Body,
    Hand,
    Leg,
    Effect,
    Artifact,
    /// レリック(ペンダント)。突き / 斬り / 魔攻 / 命中率 / Cri の側
    /// (wiki: Item/アクセサリ/レリック 神鳥のペンダント・ルナリアペンダント)
    RelicPendant,
    /// レリック(ブレスレット)。物防 / 魔防 / 回避率 / 敏捷度の側
    RelicBracelet,
}

impl PartSlot {
    /// この部位が装備強化(+1〜+15)を持てるか(wiki: 装備システム/装備強化。武器・鎧のみ)。
    pub fn allows_enhance(self) -> bool {
        matches!(self, PartSlot::Weapon | PartSlot::Armor)
    }

    /// この部位が装着アビリティを持てるか(wiki: 装備システム/アビリティ)。
    pub fn allows_abilities(self) -> bool {
        matches!(
            self,
            PartSlot::Weapon
                | PartSlot::Armor
                | PartSlot::Helm
                | PartSlot::Shield
                | PartSlot::ShieldPlus
                | PartSlot::Head
                | PartSlot::Hand
                | PartSlot::Leg
                | PartSlot::RelicPendant
                | PartSlot::RelicBracelet
        )
    }

    /// 装着アビリティの実スロット数。武器は3、盾＋は2、その他の対応部位は1。
    pub fn ability_slots(self) -> usize {
        match self {
            PartSlot::Weapon => WEAPON_ABILITY_SLOTS,
            PartSlot::Armor | PartSlot::ShieldPlus | PartSlot::Hand => 2,
            slot if slot.allows_abilities() => 1,
            _ => 0,
        }
    }

    /// この部位がシエナのオーラを発現できるか
    /// (wiki: 装備システム冒頭の表「オーラ」行 = 兜/鎧/武/盾/頭/体/手/足の 8 部位)。
    pub fn allows_siena(self) -> bool {
        matches!(
            self,
            PartSlot::Weapon
                | PartSlot::Armor
                | PartSlot::Helm
                | PartSlot::Shield
                | PartSlot::Head
                | PartSlot::Body
                | PartSlot::Hand
                | PartSlot::Leg
        )
    }

    /// この部位がランダムオプションを持てるか
    /// (wiki: 装備システム冒頭の表「転移」行 = 兜/鎧/武/盾/盾+/頭/体/手/足/レリック。効果・AF は対象外)。
    pub fn allows_random_option(self) -> bool {
        !matches!(self, PartSlot::Effect | PartSlot::Artifact)
    }

    /// この部位に付けられるランダムオプションの数(ユーザー確認 2026-08-26)。
    /// **武器だけ 3 枠**、ほかは 2 枠。レリックは wiki にも「付加オプション 2 枠」とある。
    /// 同じカテゴリーは重複できないので、実際に選べる組み合わせはさらに狭い
    pub fn random_option_slots(self) -> Option<usize> {
        if !self.allows_random_option() {
            return None;
        }
        Some(if matches!(self, PartSlot::Weapon) { 3 } else { 2 })
    }

    /// この部位が属性強化を持てるか
    /// (wiki: 装備システム冒頭の表「属性強化」行 = 兜/鎧/武/盾/頭/体/手/足/効果/AF。盾+・レリックは対象外)。
    pub fn allows_element(self) -> bool {
        !matches!(self, PartSlot::ShieldPlus | PartSlot::RelicPendant | PartSlot::RelicBracelet)
    }

    /// シエナのオーラの能力値が「装備補正(エンチャント扱い)」として付く部位
    /// (wiki: シエナのオーラ「能力値一覧(武器/盾)」)。その他の部位はステの最終固定値増加になる。
    pub fn siena_values_are_equipment(self) -> bool {
        matches!(self, PartSlot::Weapon | PartSlot::Shield)
    }
}

/// シエナのオーラによるステ加算(wiki: 能力値一覧(その他の部位)の STAB〜AGI と、
/// 追加オプション「全ステータス増加」。どちらも最終固定値増加)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SienaStatBonus {
    #[serde(default)]
    pub stab: i64,
    #[serde(default)]
    pub hack: i64,
    #[serde(default)]
    pub int: i64,
    #[serde(default)]
    pub def: i64,
    #[serde(default)]
    pub mr: i64,
    #[serde(default)]
    pub dex: i64,
    #[serde(default)]
    pub agi: i64,
}

impl SienaStatBonus {
    pub fn get(&self, kind: StatKind) -> i64 {
        match kind {
            StatKind::Stab => self.stab,
            StatKind::Hack => self.hack,
            StatKind::Int => self.int,
            StatKind::Def => self.def,
            StatKind::Mr => self.mr,
            StatKind::Dex => self.dex,
            StatKind::Agi => self.agi,
        }
    }

    pub fn get_mut(&mut self, kind: StatKind) -> &mut i64 {
        match kind {
            StatKind::Stab => &mut self.stab,
            StatKind::Hack => &mut self.hack,
            StatKind::Int => &mut self.int,
            StatKind::Def => &mut self.def,
            StatKind::Mr => &mut self.mr,
            StatKind::Dex => &mut self.dex,
            StatKind::Agi => &mut self.agi,
        }
    }

    fn add(self, other: SienaStatBonus) -> SienaStatBonus {
        let mut total = self;
        for kind in StatKind::ALL {
            *total.get_mut(kind) += other.get(kind);
        }
        total
    }
}

/// 装備部位 1 つ。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentPart {
    /// キャラ内で不変の登録ID。
    #[serde(default)]
    pub id: u64,
    /// 同名装備を見分ける任意ラベル。
    #[serde(default)]
    pub label: String,
    /// gamedata カタログ参照(`EquipmentItem::id`)。`None` = 未装備またはカスタム
    #[serde(default)]
    pub item_id: Option<String>,
    /// カタログ外アイテムの表示名 `[仮]`
    #[serde(default)]
    pub custom_name: Option<String>,
    /// 実測の基本能力値(カタログ選択時は UI がレンジ中央を既定セットし、MR 個人差は上書きする)
    #[serde(default)]
    pub base: EquipmentValues,
    /// エンチャント値(強化能力値)
    #[serde(default)]
    pub enchant: EquipmentValues,
    /// 装備強化 Lv(0..=15)。武器・鎧以外は 0 のみ許可
    #[serde(default)]
    pub enhance_level: u8,
    /// 固定ダメージの補正式。カタログ外装備でもユーザーが種別を選べば計算できる。
    #[serde(default)]
    pub enhance_type: Option<EquipmentEnhanceType>,
    /// +12 以上の固定ダメージ等級。+11 以下は式で確定するため `None` 固定。
    #[serde(default)]
    pub enhance_grade: Option<EnhanceGrade>,
    /// 装備アビリティ id
    #[serde(default)]
    pub abilities: Vec<String>,
    /// 装着アビリティ本体の可変補正(カフス・レリック等の実測値)。
    #[serde(default)]
    pub ability_values: Vec<EquipmentAbilityAdditional>,
    /// 新装着アビリティにランダム付与された追加アビリティ。
    #[serde(default)]
    pub ability_additions: Vec<EquipmentAbilityAdditional>,
    /// ランダムオプション(wiki: ランダムオプション)。同じカテゴリーは 1 部位に 1 つまで
    /// (カテゴリー整合性はカタログを引ける `storage` 側で検証する)
    #[serde(default)]
    pub random_options: Vec<RandomOptionSlot>,
}

impl EquipmentPart {
    /// ランダムオプションの部位制約と値域。カタログ整合性(未知 id・カテゴリー重複)は
    /// カタログを引ける `storage` 側で見る。
    fn validate_random_options(&self, slot: PartSlot) -> Result<(), RandomOptionError> {
        if self.random_options.is_empty() {
            return Ok(());
        }
        if !slot.allows_random_option() {
            return Err(RandomOptionError::NotAllowed { slot });
        }
        // レリックの付加オプションは 1 部位 2 枠(wiki: Item/アクセサリ/レリック
        // 「ルナリアレリックは 1 レベルから…付加オプション 2 枠が付与される」。
        // ユーザー確認 2026-08-26「2 個ずつ付けられる。カテゴリー重複は不可」)
        if slot.random_option_slots().is_some_and(|max| self.random_options.len() > max) {
            return Err(RandomOptionError::TooMany {
                slot,
                max: slot.random_option_slots().unwrap_or(0),
            });
        }
        for option in &self.random_options {
            if let Some(value) = option.value {
                if !(0.0..=RANDOM_OPTION_VALUE_MAX).contains(&value) {
                    return Err(RandomOptionError::ValueOutOfRange {
                        option_id: option.option_id.clone(),
                        value,
                        max: RANDOM_OPTION_VALUE_MAX,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate(&self, slot: PartSlot) -> Result<(), EquipmentError> {
        self.base.validate()?;
        self.enchant.validate()?;
        if self.enhance_level > ENHANCE_LEVEL_MAX {
            return Err(EquipmentError::EnhanceLevelOutOfRange {
                slot,
                value: self.enhance_level,
                max: ENHANCE_LEVEL_MAX,
            });
        }
        if self.enhance_level > 0 && !slot.allows_enhance() {
            return Err(EquipmentError::EnhanceNotAllowed { slot });
        }
        if self.enhance_level > 0 {
            let Some(kind) = self.enhance_type else {
                return Err(EquipmentError::EnhanceTypeRequired { slot, enhance_level: self.enhance_level });
            };
            let compatible = match slot {
                PartSlot::Weapon => matches!(kind,
                    EquipmentEnhanceType::WeaponStab | EquipmentEnhanceType::WeaponStabHack
                    | EquipmentEnhanceType::WeaponHack | EquipmentEnhanceType::WeaponInt
                    | EquipmentEnhanceType::WeaponIntHack | EquipmentEnhanceType::WeaponMr),
                PartSlot::Armor => matches!(kind,
                    EquipmentEnhanceType::ArmorLight | EquipmentEnhanceType::ArmorHeavy
                    | EquipmentEnhanceType::ArmorMagic | EquipmentEnhanceType::ArmorSuit
                    | EquipmentEnhanceType::ArmorRobe),
                _ => false,
            };
            if !compatible { return Err(EquipmentError::EnhanceTypeNotAllowed { slot, kind }); }
        }
        if self.enhance_grade.is_some() && self.enhance_level < ENHANCE_LEVEL_RANDOM_RANGE_MIN {
            return Err(EquipmentError::EnhanceAddedDamageNotAllowed { slot, enhance_level: self.enhance_level });
        }
        if self.enhance_level >= ENHANCE_LEVEL_RANDOM_RANGE_MIN && self.enhance_grade.is_none() {
            return Err(EquipmentError::EnhanceGradeRequired { slot, enhance_level: self.enhance_level });
        }
        if !self.abilities.is_empty() && !slot.allows_abilities() {
            return Err(EquipmentError::AbilitiesNotAllowed { slot });
        }
        if self.abilities.len() > slot.ability_slots() {
            return Err(EquipmentError::TooManyAbilities {
                slot,
                max: slot.ability_slots(),
            });
        }
        self.validate_random_options(slot)?;
        Ok(())
    }
}

/// 装備補正の値域・部位制約違反。
/// (シエナのオーラの攻撃力増加が % の実数なので `Eq` は導出しない)
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum EquipmentError {
    #[error("{slot:?} の登録装備ID {id} が0または重複しています")]
    DuplicatePartId { slot: PartSlot, id: u64 },
    #[error("{slot:?} の選択中装備IDが登録一覧にありません")]
    UnknownSelectedId { slot: PartSlot },
    #[error("装備補正の{field}は 0〜{max} の範囲で指定してください(指定値 {value})")]
    ValueOutOfRange { field: &'static str, value: i64, max: i64 },
    #[error("{slot:?} の装備強化 Lv は 0〜{max} です(指定値 {value})")]
    EnhanceLevelOutOfRange { slot: PartSlot, value: u8, max: u8 },
    #[error("{slot:?} は装備強化の対象外です(武器・鎧のみ)")]
    EnhanceNotAllowed { slot: PartSlot },
    #[error("{slot:?} の装備強化 Lv {enhance_level} では装備種別を選んでください")]
    EnhanceTypeRequired { slot: PartSlot, enhance_level: u8 },
    #[error("{slot:?} に装備強化種別 {kind:?} は指定できません")]
    EnhanceTypeNotAllowed { slot: PartSlot, kind: EquipmentEnhanceType },
    #[error("{slot:?} の追加固定ダメージ上書きは強化 Lv {enhance_level} では指定できません(+12 以上のみ)")]
    EnhanceAddedDamageNotAllowed { slot: PartSlot, enhance_level: u8 },
    #[error("{slot:?} の装備強化 Lv {enhance_level} では等級を選んでください")]
    EnhanceGradeRequired { slot: PartSlot, enhance_level: u8 },
    #[error("{slot:?} は装備アビリティの対象外です")]
    AbilitiesNotAllowed { slot: PartSlot },
    #[error("{slot:?} の装備アビリティは最大 {max} 個です")]
    TooManyAbilities { slot: PartSlot, max: usize },
    #[error(transparent)]
    ThesisCore(#[from] ThesisCoreError),
    #[error(transparent)]
    RandomOption(#[from] RandomOptionError),
    #[error(transparent)]
    Siena(#[from] SienaError),
}

/// 装備強化の固定ダメージ補正式。カタログ品は自動設定し、カタログ外だけ選択する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentEnhanceType {
    WeaponStab,
    WeaponStabHack,
    WeaponHack,
    WeaponInt,
    WeaponIntHack,
    WeaponMr,
    ArmorLight,
    ArmorHeavy,
    ArmorMagic,
    ArmorSuit,
    ArmorRobe,
}

/// 武器アビリティの効果系統。候補を武器の攻撃系統へ絞るために使う。
/// 排他単位はこの系統ではなく `category`（同じ斬り系でもカテゴリー1と4は併用できる）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentAbilityFamily {
    /// 尖った刃(突き攻撃力)
    PointedBlade,
    /// 鋭い刃(斬り攻撃力)
    SharpBlade,
    /// 知力(魔法攻撃力)
    Intelligence,
    /// 耐魔力(魔法防御力)
    MagicResistance,
    /// 武器ディレイ増減
    WeaponDelay,
    ArmorPolish,
    Vitality,
    Mana,
    Evasion,
    ShieldPolish,
    Critical,
    Accuracy,
    Element,
    Agility,
    SkillAttack,
}

/// 新装着アビリティの追加候補。実物で抽選された種類と値を登録する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentAbilityAdditionalKind {
    FixedDamage,
    DamageRate,
    Thrust,
    Slash,
    MagicAttack,
    MagicDefense,
    HpRecovery,
    MpRecovery,
    Accuracy,
    PhysicalDefense,
    Critical,
    Evasion,
    DamageResistance,
    PhysicalDamageReduction,
    MagicDamageReduction,
    SpRecovery,
    EvasionRate,
    FireElement,
    WaterElement,
    WindElement,
    EarthElement,
    LightningElement,
    WhiteElement,
    DarkElement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentAbilityAdditional {
    pub ability_id: String,
    pub kind: EquipmentAbilityAdditionalKind,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquipmentAbilityAdditionalDef {
    pub kind: EquipmentAbilityAdditionalKind,
    pub min: i32,
    pub max: i32,
}

impl EquipmentAbilityFamily {
    pub const ALL: [EquipmentAbilityFamily; 15] = [
        EquipmentAbilityFamily::PointedBlade,
        EquipmentAbilityFamily::SharpBlade,
        EquipmentAbilityFamily::Intelligence,
        EquipmentAbilityFamily::MagicResistance,
        EquipmentAbilityFamily::WeaponDelay,
        EquipmentAbilityFamily::ArmorPolish,
        EquipmentAbilityFamily::Vitality,
        EquipmentAbilityFamily::Mana,
        EquipmentAbilityFamily::Evasion,
        EquipmentAbilityFamily::ShieldPolish,
        EquipmentAbilityFamily::Critical,
        EquipmentAbilityFamily::Accuracy,
        EquipmentAbilityFamily::Element,
        EquipmentAbilityFamily::Agility,
        EquipmentAbilityFamily::SkillAttack,
    ];
}

/// 武器アビリティ定義(gamedata がカタログを持つ。domain の `BuffDefinition` と同じ依存方向)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EquipmentAbilityDef {
    pub id: &'static str,
    pub name: &'static str,
    /// 効果系統。UI の武器別候補絞り込みに使う。
    pub family: EquipmentAbilityFamily,
    /// 装着カテゴリー。同じカテゴリーは同一装備に1つまで。
    pub category: u8,
    /// 適用できる装備部位。
    pub slot: PartSlot,
    /// アビリティ本体が範囲値を持つ場合の入力定義。
    pub value_option: Option<EquipmentAbilityAdditionalDef>,
    /// 同じ値は同一装備に共存できない。
    pub exclusive_group: &'static str,
    /// ランダムに付く追加アビリティ枠数。
    pub additional_slots: u8,
    /// 追加候補の要約。自動適用せず記録のみ。
    pub additional_effects: &'static str,
    /// ランダム追加枠に出現し得る種類と値域。
    pub additional_options: Vec<EquipmentAbilityAdditionalDef>,
    /// 現在の計算式に未収録の効果か。
    pub record_only: bool,
    /// 画面に出す効果要約。計算未収録の効果も空欄にしない。
    pub effect_summary: &'static str,
    /// 装備攻撃力(基本能力値)への加算値
    pub values: EquipmentValues,
    /// **追加効果**(wiki: アビリティ表の「追加効果」列)。R- 以上の段に付く
    /// 「ダメージ増加 +n%」は装備攻撃力ではなく与ダメージ式のカテゴリX3 に入る
    pub damage_effects: &'static [SkillEffect],
}

/// キャラの装備補正一式(部位別装備 12 スロット + 称号 + テシスコア)。
///
/// 装備攻撃力強化倍率(パワーウェポン / ストロングウェポン)は**共通スキル**なので
/// `CommonSkills` が持つ(wiki: Skill/共通)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Equipment {
    #[serde(default)]
    pub parts: EquipmentParts,
    /// 抽出・注入で装備と独立して付け替える、部位別シエナのオーラ登録一覧。
    #[serde(default)]
    pub siena: SienaAuras,
    /// テシスコア(地域ごとに 6 枠)。火力タイプの補正は強化能力値へ合流する
    #[serde(default)]
    pub thesis_cores: ThesisCores,
    /// 表示中の称号(`TitleDef::id`)。**1 枠だけ**で、補正は基本能力値へ合流する
    /// (wiki: 称号システム。所持ぶんの累積ではない)。`None` = 未装備
    #[serde(default)]
    pub title: Option<String>,
}

/// 12 部位。named field で持つ(`parts.weapon` 等)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentParts {
    #[serde(default)]
    pub weapon: EquipmentPartList,
    #[serde(default)]
    pub armor: EquipmentPartList,
    #[serde(default)]
    pub helm: EquipmentPartList,
    #[serde(default)]
    pub shield: EquipmentPartList,
    #[serde(default)]
    pub shield_plus: EquipmentPartList,
    #[serde(default)]
    pub head: EquipmentPartList,
    #[serde(default)]
    pub body: EquipmentPartList,
    #[serde(default)]
    pub hand: EquipmentPartList,
    #[serde(default)]
    pub leg: EquipmentPartList,
    #[serde(default)]
    pub effect: EquipmentPartList,
    #[serde(default)]
    pub artifact: EquipmentPartList,
    #[serde(default)]
    pub relic_pendant: EquipmentPartList,
    #[serde(default)]
    pub relic_bracelet: EquipmentPartList,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentPartList {
    #[serde(default)]
    pub registered: Vec<EquipmentPart>,
    #[serde(default)]
    pub selected_id: Option<u64>,
}

impl Default for EquipmentPartList {
    fn default() -> Self { Self { registered: Vec::new(), selected_id: None } }
}

impl From<EquipmentPart> for EquipmentPartList {
    fn from(mut part: EquipmentPart) -> Self {
        if part.id == 0 { part.id = 1; }
        let id = part.id;
        Self { registered: vec![part], selected_id: Some(id) }
    }
}

impl std::ops::Deref for EquipmentPartList {
    type Target = EquipmentPart;
    fn deref(&self) -> &Self::Target { self.selected().expect("EquipmentPartList selected_id invariant") }
}
impl std::ops::DerefMut for EquipmentPartList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if self.selected().is_none() {
            let id = self.registered.iter().map(|p| p.id).max().unwrap_or(0) + 1;
            let mut part = EquipmentPart::default(); part.id = id;
            self.registered.push(part); self.selected_id = Some(id);
        }
        self.selected_mut().expect("EquipmentPartList selected_id invariant")
    }
}

impl EquipmentPartList {
    pub fn selected(&self) -> Option<&EquipmentPart> { self.selected_id.and_then(|id| self.registered.iter().find(|p| p.id == id)) }
    pub fn selected_mut(&mut self) -> Option<&mut EquipmentPart> { let id = self.selected_id?; self.registered.iter_mut().find(|p| p.id == id) }
    pub fn validate(&self, slot: PartSlot) -> Result<(), EquipmentError> {
        if self.selected_id.is_some() && self.selected().is_none() { return Err(EquipmentError::UnknownSelectedId { slot }); }
        let mut ids = std::collections::HashSet::new();
        for part in &self.registered {
            if part.id == 0 || !ids.insert(part.id) { return Err(EquipmentError::DuplicatePartId { slot, id: part.id }); }
            part.validate(slot)?;
        }
        Ok(())
    }
}

impl EquipmentParts {
    /// 選択中の部位だけを列挙する。
    pub fn iter(&self) -> Vec<(PartSlot, &EquipmentPart)> {
        self.iter_lists().into_iter().filter_map(|(slot, parts)| parts.selected().map(|p| (slot, p))).collect()
    }
    /// 13 部位を `(PartSlot, &EquipmentPart)` で列挙する。
    pub fn iter_lists(&self) -> [(PartSlot, &EquipmentPartList); 13] {
        [
            (PartSlot::Weapon, &self.weapon),
            (PartSlot::Armor, &self.armor),
            (PartSlot::Helm, &self.helm),
            (PartSlot::Shield, &self.shield),
            (PartSlot::ShieldPlus, &self.shield_plus),
            (PartSlot::Head, &self.head),
            (PartSlot::Body, &self.body),
            (PartSlot::Hand, &self.hand),
            (PartSlot::Leg, &self.leg),
            (PartSlot::Effect, &self.effect),
            (PartSlot::Artifact, &self.artifact),
            (PartSlot::RelicPendant, &self.relic_pendant),
            (PartSlot::RelicBracelet, &self.relic_bracelet),
        ]
    }

    /// 部位を可変で引く。
    pub fn get_mut(&mut self, slot: PartSlot) -> &mut EquipmentPartList {
        match slot {
            PartSlot::Weapon => &mut self.weapon,
            PartSlot::Armor => &mut self.armor,
            PartSlot::Helm => &mut self.helm,
            PartSlot::Shield => &mut self.shield,
            PartSlot::ShieldPlus => &mut self.shield_plus,
            PartSlot::Head => &mut self.head,
            PartSlot::Body => &mut self.body,
            PartSlot::Hand => &mut self.hand,
            PartSlot::Leg => &mut self.leg,
            PartSlot::Effect => &mut self.effect,
            PartSlot::Artifact => &mut self.artifact,
            PartSlot::RelicPendant => &mut self.relic_pendant,
            PartSlot::RelicBracelet => &mut self.relic_bracelet,
        }
    }
}

impl Equipment {
    pub fn validate(&self) -> Result<(), EquipmentError> {
        for (slot, parts) in self.parts.iter_lists() {
            parts.validate(slot)?;
        }
        self.siena.validate()?;
        self.thesis_cores.validate()?;
        Ok(())
    }

    /// 基本能力値の合計(Σ part.base + Σ 装備アビリティの加算値 + 表示中の称号)。
    ///
    /// 称号は装備部位ではないが、効き先が基本能力値なのでここで合流させる
    /// (wiki: 称号システム。ユーザー確定 2026-08-25)。
    pub fn base_totals(
        &self,
        abilities: &[EquipmentAbilityDef],
        titles: &[TitleDef],
    ) -> EquipmentValues {
        let mut total = EquipmentValues::default();
        for (_, part) in self.iter_selected() {
            total = total.add(part.base);
        }
        for (slot, part) in self.iter_selected() {
            for ability_id in &part.abilities {
                if let Some(def) = abilities.iter().find(|a| a.id == *ability_id && a.slot == slot) {
                    total = total.add(def.values);
                }
            }
            for value in &part.ability_values {
                add_ability_value(&mut total, value);
            }
            for addition in &part.ability_additions {
                add_ability_value(&mut total, addition);
            }
        }
        total.add(title_values(self.title.as_deref(), titles))
    }

    /// アビリティの追加効果(wiki: アビリティ表の「追加効果」列)を
    /// 与ダメージ式のカテゴリ寄与に変換する。装備攻撃力への加算は `base_totals` が別に見る。
    pub fn ability_damage_contributions(
        &self,
        abilities: &[EquipmentAbilityDef],
    ) -> Vec<(DamageCategory, f64)> {
        let effects: Vec<&SkillEffect> = self
            .iter_selected().into_iter()
            .flat_map(|(slot, part)| part.abilities.iter().filter_map(move |id| abilities.iter().find(|a| a.id == id.as_str() && a.slot == slot)))
            .flat_map(|def| def.damage_effects.iter())
            .collect();
        let mut contributions = damage_contributions(effects.into_iter());
        for (_, part) in self.iter_selected() {
            for addition in &part.ability_additions {
                match addition.kind {
                    EquipmentAbilityAdditionalKind::FixedDamage => {
                        contributions.push((DamageCategory::BasicTriggerDamageFixed, addition.value as f64));
                    }
                    EquipmentAbilityAdditionalKind::DamageRate => {
                        contributions.push((DamageCategory::AttackDamageBasicTrigger, addition.value as f64 / 100.0));
                    }
                    _ => {}
                }
            }
        }
        contributions
    }

    /// 強化能力値の合計(Σ part.enchant + Σ シエナのオーラの能力値(武器/盾)+ テシスコア)。
    ///
    /// `region` はダメージ計算の対象コンテンツのテシスコア地域。テシスコアの能力値増加は
    /// 対象ダンジョン内でのみ有効なので、`None`(コアが効かないコンテンツ)なら加算しない。
    pub fn enhanced_totals(&self, region: Option<CoreRegion>) -> EquipmentValues {
        let mut total = EquipmentValues::default();
        for (_, part) in self.iter_selected() {
            total = total.add(part.enchant);
        }
        for (slot, aura) in self.siena.iter_selected() {
            if slot.siena_values_are_equipment() {
                total = total.add(aura.values());
            }
        }
        total.add(self.thesis_cores.equipment_values(region))
    }

    /// 全部位のランダムオプションの集計。カタログは呼び出し側が渡す
    /// (`base_totals` の武器アビリティと同じ依存方向。domain は gamedata に依存できない)。
    /// カタログに無い id の枠は無視する(保存時に `storage` が弾いている)。
    pub fn random_option_totals(&self, defs: &[RandomOptionDef]) -> RandomOptionTotals {
        let mut totals = RandomOptionTotals::default();
        for (_, part) in self.iter_selected() {
            for option in &part.random_options {
                if let Some(def) = defs.iter().find(|d| d.id == option.option_id.as_str()) {
                    totals.add(def, option);
                }
            }
        }
        totals
    }

    /// シエナのオーラの追加オプション「攻撃力増加」の合計(wiki: New1)。Σ% の小数表現。
    pub fn siena_attack_rate(&self) -> f64 {
        self.siena.iter_selected().map(|(_, aura)| aura.attack_rate_percent()).sum::<f64>()
            / 100.0
    }

    /// シエナのオーラの追加オプション「防御力増加」の合計。Σ% の小数表現。
    /// 装備防御力倍率へ合流する(`CommonSkills::defense_rates` の引数)。
    pub fn siena_defense_rate(&self) -> f64 {
        self.siena.iter_selected().map(|(_, aura)| aura.defense_rate_percent()).sum::<f64>()
            / 100.0
    }

    /// シエナのオーラの追加オプション「中ディレイ減少」の合計。Σ% の小数表現。
    /// 中ディレイ減少値(倍率B)へ合流する(wiki: ステータス「中ディレイ倍率B」)。
    pub fn siena_actual_delay_reduction(&self) -> f64 {
        self.siena.iter_selected().map(|(_, aura)| aura.actual_delay_percent()).sum::<f64>()
            / 100.0
    }

    /// シエナのオーラの追加オプション「クリティカル確率」の合計。Σ% の小数表現。
    /// クリティカル率の AGI 由来の項に `× (1 + これ)` で効く(wiki: `#CriticalChance`)。
    ///
    /// wiki は「同一名称の効果同士で加算されるかどうかは要検証」としているが、
    /// 他の追加オプション(攻撃力増加・防御力増加・中ディレイ減少)と同じく部位ぶん加算する `[仮]`。
    pub fn siena_critical_rate(&self) -> f64 {
        self.siena.iter_selected().map(|(_, aura)| aura.critical_rate_percent()).sum::<f64>()
            / 100.0
    }

    /// 装備に付与した属性値の合計(属性ごと)。
    pub fn element_values(&self, selected: Option<Element>) -> ElementValues {
        let mut total = ElementValues::default();
        if let Some(element) = selected.filter(|e| e.can_enchant_equipment()) {
            for (_, _) in self.iter_selected().filter(|(slot, part)| {
                slot.allows_element() && (part.item_id.is_some() || part.custom_name.as_deref().is_some_and(|n| !n.is_empty()))
            }) { *total.get_mut(element) += 9; }
        }
        total
    }

    /// シエナのオーラによるステ加算の合計(能力値スロット + 全ステータス増加。最終固定値層に乗る)。
    pub fn siena_stat_bonus(&self) -> SienaStatBonus {
        let mut total = SienaStatBonus::default();
        for (_, aura) in self.siena.iter_selected() {
            total = total.add(aura.stat_bonus());
        }
        total
    }

    /// その部位だけを未装備(中立値)にした複製。部位ごとの寄与(外したときの差分)を出すのに使う。
    pub fn without_selected_part(&self, slot: PartSlot) -> Equipment {
        let mut copy = self.clone();
        copy.parts.get_mut(slot).selected_id = None;
        copy
    }

    pub fn without_part(&self, slot: PartSlot) -> Equipment { self.without_selected_part(slot) }

    pub fn iter_selected(&self) -> impl Iterator<Item = (PartSlot, &EquipmentPart)> {
        self.parts.iter_lists().into_iter().filter_map(|(slot, parts)| parts.selected().map(|p| (slot, p)))
    }
}

fn add_ability_value(total: &mut EquipmentValues, value: &EquipmentAbilityAdditional) {
    match value.kind {
        EquipmentAbilityAdditionalKind::Thrust => total.thrust += i64::from(value.value),
        EquipmentAbilityAdditionalKind::Slash => total.slash += i64::from(value.value),
        EquipmentAbilityAdditionalKind::MagicAttack => total.magic_attack += i64::from(value.value),
        EquipmentAbilityAdditionalKind::MagicDefense => total.magic_defense += i64::from(value.value),
        EquipmentAbilityAdditionalKind::Accuracy => total.accuracy += i64::from(value.value),
        EquipmentAbilityAdditionalKind::PhysicalDefense => total.physical_defense += i64::from(value.value),
        EquipmentAbilityAdditionalKind::Critical => total.critical += i64::from(value.value),
        EquipmentAbilityAdditionalKind::Evasion => total.evasion += i64::from(value.value),
        _ => {}
    }
}

/// 装備補正 4 種それぞれに掛かる係数(基本/強化のどちらか片方)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentRates {
    pub thrust: f64,
    pub slash: f64,
    pub magic_attack: f64,
    pub magic_defense: f64,
}

/// 装備攻撃力の係数(基本能力値用/強化能力値用)。スキル依存種別ごとに gamedata が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentCoefficients {
    pub base: EquipmentRates,
    pub enhanced: EquipmentRates,
}

/// 装備攻撃力(wiki: カテゴリA の内訳)。`Σ(基本値 × 基本係数) + Σ(強化値 × 強化係数)`。
/// `base`/`enhanced` は呼び出し側が `Equipment::base_totals`/`enhanced_totals` で集計して渡す。
pub fn equipment_attack_power(
    base: &EquipmentValues,
    enhanced: &EquipmentValues,
    c: &EquipmentCoefficients,
) -> f64 {
    equipment_values_attack(base, &c.base) + equipment_values_attack(enhanced, &c.enhanced)
}

/// 装備値 4 値と係数の内積。基本能力値・強化能力値それぞれの装備攻撃力を単独で出すのに使う
/// (`equipment_attack_power` は両者の和)。
pub fn equipment_values_attack(values: &EquipmentValues, rates: &EquipmentRates) -> f64 {
    values.thrust as f64 * rates.thrust
        + values.slash as f64 * rates.slash
        + values.magic_attack as f64 * rates.magic_attack
        + values.magic_defense as f64 * rates.magic_defense
}

/// 武器系統ごとの強化補正一次式の係数(wiki: 装備システム/装備強化)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EnhanceRates {
    pub thrust: f64,
    pub slash: f64,
    pub magic_attack: f64,
    pub magic_defense: f64,
}

/// 武器の追加固定ダメージ(wiki: 装備システム/装備強化、docs/damage-formula.md §5。与ダメージ式の外)。
///
/// `補正 = 突き×r.thrust + 斬り×r.slash + 魔攻×r.magic_attack + 魔防×r.magic_defense`
/// (アビリティによる補正は含めない = `weapon_base` は part.base の実測値そのもの)。
/// `追加効果 = INT(INT(補正) × 倍率)`。結果が奇数なら −1。
pub fn weapon_added_damage(weapon_base: &EquipmentValues, rates: &EnhanceRates, multiplier: f64) -> i64 {
    let correction = weapon_base.thrust as f64 * rates.thrust
        + weapon_base.slash as f64 * rates.slash
        + weapon_base.magic_attack as f64 * rates.magic_attack
        + weapon_base.magic_defense as f64 * rates.magic_defense;
    let inner = correction.trunc();
    let added = (inner * multiplier).trunc() as i64;
    if added % 2 != 0 {
        added - 1
    } else {
        added
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::siena::{
        RegisteredSienaAura, SienaAura, SienaAuraList, SienaExtraKind, SienaExtraSlot, SienaSlot,
        SienaValueKind, SIENA_STAGE_MAX,
    };

    fn coefficients() -> EquipmentCoefficients {
        EquipmentCoefficients {
            base: EquipmentRates { thrust: 14.5, slash: 14.5, magic_attack: 0.0, magic_defense: 0.0 },
            enhanced: EquipmentRates { thrust: 28.75, slash: 28.75, magic_attack: 0.0, magic_defense: 0.0 },
        }
    }

    fn equipment_with(weapon_base: EquipmentValues, weapon_enchant: EquipmentValues) -> Equipment {
        Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart { base: weapon_base, enchant: weapon_enchant, ..Default::default() }.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn 装備攻撃力は基本と強化の合計() {
        let eq = equipment_with(
            EquipmentValues { thrust: 150, slash: 150, ..Default::default() },
            EquipmentValues { thrust: 60, slash: 60, ..Default::default() },
        );
        let base = eq.base_totals(&[], &[]);
        let enhanced = eq.enhanced_totals(None);
        // 150*14.5*2 + 60*28.75*2 = 4350 + 3450 = 7800
        assert!((equipment_attack_power(&base, &enhanced, &coefficients()) - 7800.0).abs() < 1e-9);
    }

    #[test]
    fn 装備なしなら装備攻撃力は0() {
        let eq = Equipment::default();
        let base = eq.base_totals(&[], &[]);
        let enhanced = eq.enhanced_totals(None);
        assert_eq!(equipment_attack_power(&base, &enhanced, &coefficients()), 0.0);
    }

    #[test]
    fn base_totalsはアビリティ込み_enchantedはenchant側() {
        let mut eq = equipment_with(
            EquipmentValues { thrust: 100, slash: 200, ..Default::default() },
            EquipmentValues { thrust: 10, slash: 20, ..Default::default() },
        );
        eq.parts.weapon.abilities = vec!["sharp-blade-e".to_string()];
        eq.parts.armor.base = EquipmentValues { magic_defense: 50, ..Default::default() };

        let abilities = vec![EquipmentAbilityDef {
            id: "sharp-blade-e",
            name: "E-鋭い刃",
            family: EquipmentAbilityFamily::SharpBlade,
            category: 4, slot: PartSlot::Weapon, value_option: None, exclusive_group: "weapon-category-4", additional_slots: 2,
            additional_effects: "", additional_options: vec![], record_only: false, effect_summary: "斬り +9",
            values: EquipmentValues { slash: 9, ..Default::default() },
        damage_effects: &[],
        }];
        let base = eq.base_totals(&abilities, &[]);
        assert_eq!(base, EquipmentValues { thrust: 100, slash: 209, magic_defense: 50, ..Default::default() });

        let enhanced = eq.enhanced_totals(None);
        assert_eq!(enhanced, EquipmentValues { thrust: 10, slash: 20, ..Default::default() });
    }

    #[test]
    fn カフスのアビリティ実測値は装備基本値へ入る() {
        let mut eq = Equipment::default();
        eq.parts.shield_plus.abilities = vec!["mystic-mine-sharp-blade".into()];
        eq.parts.shield_plus.ability_values = vec![EquipmentAbilityAdditional {
            ability_id: "mystic-mine-sharp-blade".into(),
            kind: EquipmentAbilityAdditionalKind::Slash,
            value: 13,
        }];
        assert_eq!(eq.base_totals(&[], &[]).slash, 13);
    }

    #[test]
    fn 新装着アビリティのランダム追加は補正値_w_x3へ入る() {
        use EquipmentAbilityAdditionalKind::*;
        let mut eq = equipment_with(EquipmentValues::default(), EquipmentValues::default());
        eq.parts.weapon.ability_additions = vec![
            EquipmentAbilityAdditional { ability_id: "night-star-sharp-blade".into(), kind: Slash, value: 18 },
            EquipmentAbilityAdditional { ability_id: "night-star-sharp-blade".into(), kind: Accuracy, value: 16 },
        ];
        let base = eq.base_totals(&[], &[]);
        assert_eq!(base.slash, 18);
        assert_eq!(base.accuracy, 16);

        eq.parts.weapon.ability_additions = vec![
            EquipmentAbilityAdditional { ability_id: "night-star-sharp-blade".into(), kind: FixedDamage, value: 10_000 },
            EquipmentAbilityAdditional { ability_id: "night-star-sharp-blade".into(), kind: DamageRate, value: 11 },
        ];
        let contributions = eq.ability_damage_contributions(&[]);
        assert!(contributions.contains(&(DamageCategory::BasicTriggerDamageFixed, 10_000.0)));
        assert!(contributions.contains(&(DamageCategory::AttackDamageBasicTrigger, 0.11)));
    }

    #[test]
    fn 武器アビリティは3枠まで() {
        let mut eq = Equipment::default();
        eq.parts.weapon.abilities = vec!["a".into(), "b".into(), "c".into()];
        assert!(eq.validate().is_ok());
        eq.parts.weapon.abilities.push("d".into());
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::TooManyAbilities { slot: PartSlot::Weapon, max: 3 })
        ));
    }


    #[test]
    fn 値域違反は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.weapon.enchant.magic_defense = -1;
        assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX;
        assert!(eq.validate().is_ok());
    }

    #[test]
    fn 装備値の値域は9種すべてを検証する() {
        // wiki Item ページの列順そのまま。1 種でも欠けると検証をすり抜ける
        let names: Vec<&str> = EquipmentValues::default().fields().iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                "突き攻撃力", "斬り攻撃力", "物理防御力", "魔法攻撃力", "魔法防御力",
                "命中率補正", "クリティカル補正", "回避率補正", "敏捷度補正",
            ]
        );
        let over = EQUIPMENT_VALUE_MAX + 1;
        for setter in [
            |v: &mut EquipmentValues, x| v.thrust = x,
            |v: &mut EquipmentValues, x| v.slash = x,
            |v: &mut EquipmentValues, x| v.physical_defense = x,
            |v: &mut EquipmentValues, x| v.magic_attack = x,
            |v: &mut EquipmentValues, x| v.magic_defense = x,
            |v: &mut EquipmentValues, x| v.accuracy = x,
            |v: &mut EquipmentValues, x| v.critical = x,
            |v: &mut EquipmentValues, x| v.evasion = x,
            |v: &mut EquipmentValues, x| v.agility = x,
        ] {
            let mut eq = Equipment::default();
            setter(&mut eq.parts.weapon.base, over);
            assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));
        }
    }

    #[test]
    fn 選択属性は実装備の対象部位へ9ずつ自動反映される() {
        let mut eq = Equipment::default();
        eq.parts.weapon.item_id = Some("weapon".into());
        eq.parts.armor.custom_name = Some("custom armor".into());
        assert!(eq.validate().is_ok());
        let values = eq.element_values(Some(Element::Water));
        assert_eq!(values.get(Element::Water), 18);
        assert_eq!(values.get(Element::Fire), 0);
        assert_eq!(values.get(Element::Neutral), 0);
    }

    #[test]
    fn 無属性と未装備には属性強化を反映しない() {
        let mut eq = Equipment::default();
        eq.parts.shield_plus.item_id = Some("cuffs".into());
        assert_eq!(eq.element_values(Some(Element::Neutral)), ElementValues::default());
        assert_eq!(eq.element_values(Some(Element::Fire)), ElementValues::default());
    }

    #[test]
    fn 武器以外の強化レベルは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.helm.enhance_level = 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceNotAllowed { slot: PartSlot::Helm })));

        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = ENHANCE_LEVEL_MAX;
        eq.parts.weapon.enhance_type = Some(EquipmentEnhanceType::WeaponHack);
        eq.parts.weapon.enhance_grade = Some(EnhanceGrade::Highest);
        assert!(eq.validate().is_ok());
        let mut eq2 = Equipment::default();
        eq2.parts.armor.enhance_level = ENHANCE_LEVEL_MAX;
        eq2.parts.armor.enhance_type = Some(EquipmentEnhanceType::ArmorLight);
        eq2.parts.armor.enhance_grade = Some(EnhanceGrade::Highest);
        assert!(eq2.validate().is_ok());

        let mut over = Equipment::default();
        over.parts.weapon.enhance_level = ENHANCE_LEVEL_MAX + 1;
        assert!(matches!(over.validate(), Err(EquipmentError::EnhanceLevelOutOfRange { .. })));
    }

    #[test]
    fn 強化等級は12以上だけ許可する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 11;
        eq.parts.weapon.enhance_type = Some(EquipmentEnhanceType::WeaponHack);
        eq.parts.weapon.enhance_grade = Some(EnhanceGrade::Highest);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::EnhanceAddedDamageNotAllowed { slot: PartSlot::Weapon, .. })
        ));

        let mut ok = Equipment::default();
        ok.parts.weapon.enhance_level = 12;
        ok.parts.weapon.enhance_type = Some(EquipmentEnhanceType::WeaponHack);
        ok.parts.weapon.enhance_grade = Some(EnhanceGrade::Highest);
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn 強化12以上は等級必須() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 12;
        eq.parts.weapon.enhance_type = Some(EquipmentEnhanceType::WeaponHack);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::EnhanceGradeRequired { slot: PartSlot::Weapon, enhance_level: 12 })
        ));
    }

    #[test]
    fn 強化する装備は部位に合う種別が必須() {
        let mut missing = Equipment::default();
        missing.parts.weapon.enhance_level = 10;
        assert!(matches!(missing.validate(), Err(EquipmentError::EnhanceTypeRequired { .. })));

        let mut mismatch = Equipment::default();
        mismatch.parts.weapon.enhance_level = 10;
        mismatch.parts.weapon.enhance_type = Some(EquipmentEnhanceType::ArmorMagic);
        assert!(matches!(mismatch.validate(), Err(EquipmentError::EnhanceTypeNotAllowed { .. })));
    }

    #[test]
    fn 対象外部位のアビリティは拒否し兜は許可する() {
        let mut eq = Equipment::default();
        eq.parts.body.abilities = vec!["unknown".to_string()];
        assert!(matches!(eq.validate(), Err(EquipmentError::AbilitiesNotAllowed { slot: PartSlot::Body })));

        let mut ok = Equipment::default();
        ok.parts.helm.abilities = vec!["helm-e-skill-attack".to_string()];
        assert!(ok.validate().is_ok());
    }

    // wiki 例(goal 文書): HACK系(斬り×6.67 + 突き×1.00)・突100/斬300
    // → INT(300×6.67+100×1.00) = INT(2001+100) = INT(2101) = 2101
    // +10 倍率 28.8 → INT(2101×28.8) = INT(60508.8) = 60508(偶数なのでそのまま)
    #[test]
    fn 武器追加固定ダメージ_hack系の式() {
        let rates = EnhanceRates { thrust: 1.00, slash: 6.67, magic_attack: 0.0, magic_defense: 0.0 };
        let weapon = EquipmentValues { thrust: 100, slash: 300, ..Default::default() };
        assert_eq!(weapon_added_damage(&weapon, &rates, 28.8), 60508);
    }

    #[test]
    fn 武器追加固定ダメージ_奇数なら1引く() {
        // 補正 = 101(突き×1.0)、倍率 1.0(+2 相当) → INT(101×1.0) = 101(奇数) → 100
        let rates = EnhanceRates { thrust: 1.0, slash: 0.0, magic_attack: 0.0, magic_defense: 0.0 };
        let weapon = EquipmentValues { thrust: 101, ..Default::default() };
        assert_eq!(weapon_added_damage(&weapon, &rates, 1.0), 100);
    }

    #[test]
    fn 登録idの0と重複は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.registered.push(EquipmentPart::default());
        eq.parts.weapon.selected_id = Some(0);
        assert!(matches!(eq.validate(), Err(EquipmentError::DuplicatePartId { .. })));
    }

    fn siena_values(thrust: i64, slash: i64) -> SienaAura {
        let mut slots = Vec::new();
        if thrust > 0 {
            slots.push(SienaSlot { kind: SienaValueKind::Thrust, value: thrust });
        }
        if slash > 0 {
            slots.push(SienaSlot { kind: SienaValueKind::Slash, value: slash });
        }
        SienaAura { slots, extras: Vec::new() }
    }

    /// 追加オプションの枠を開けるための埋めスロット(段階 = スロット数)。
    fn siena_stage(stage: usize, kind: SienaValueKind) -> SienaAura {
        SienaAura {
            slots: vec![SienaSlot { kind, value: 1 }; stage],
            extras: Vec::new(),
        }
    }

    fn set_siena(eq: &mut Equipment, slot: PartSlot, aura: SienaAura) {
        let list = eq.siena.get_mut(slot).expect("シエナ対象部位");
        list.registered = vec![RegisteredSienaAura { id: 1, label: String::new(), aura }];
        list.selected_id = Some(1);
    }

    #[test]
    fn シエナのオーラの能力値は武器と盾だけ強化能力値に入る() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enchant = EquipmentValues { thrust: 10, ..Default::default() };
        set_siena(&mut eq, PartSlot::Weapon, siena_values(6, 4));
        set_siena(&mut eq, PartSlot::Shield, siena_values(0, 5));
        // 武器・盾以外はステ加算(装備補正には入らない)
        set_siena(&mut eq, PartSlot::Helm, SienaAura {
            slots: vec![
                SienaSlot { kind: SienaValueKind::Stab, value: 10 },
                SienaSlot { kind: SienaValueKind::Stab, value: 10 },
                SienaSlot { kind: SienaValueKind::Hack, value: 10 },
            ],
            extras: Vec::new(),
        });
        assert!(eq.validate().is_ok());

        assert_eq!(
            eq.enhanced_totals(None),
            EquipmentValues { thrust: 16, slash: 9, ..Default::default() }
        );
        assert_eq!(eq.base_totals(&[], &[]), EquipmentValues::default());
        assert_eq!(
            eq.siena_stat_bonus(),
            SienaStatBonus { stab: 20, hack: 10, ..Default::default() }
        );
    }

    #[test]
    fn シエナのオーラの攻撃力増加は全部位の合計() {
        let mut eq = Equipment::default();
        for (slot, rate) in [(PartSlot::Weapon, 10.0), (PartSlot::Armor, 3.0), (PartSlot::Leg, 2.0)] {
            let kind = if slot.siena_values_are_equipment() {
                SienaValueKind::Thrust
            } else {
                SienaValueKind::Stab
            };
            let mut aura = siena_stage(3, kind);
            aura.extras.push(SienaExtraSlot { kind: SienaExtraKind::AttackRate, value: rate });
            set_siena(&mut eq, slot, aura);
        }
        assert!(eq.validate().is_ok());
        assert!((eq.siena_attack_rate() - 0.15).abs() < 1e-12);
        assert_eq!(Equipment::default().siena_attack_rate(), 0.0);
    }

    #[test]
    fn シエナのオーラは登録一覧の装着中だけ反映する() {
        let mut low = siena_stage(3, SienaValueKind::Thrust);
        low.extras.push(SienaExtraSlot { kind: SienaExtraKind::AttackRate, value: 3.0 });
        let mut high = siena_stage(3, SienaValueKind::Thrust);
        high.extras.push(SienaExtraSlot { kind: SienaExtraKind::AttackRate, value: 9.0 });
        let mut eq = Equipment::default();
        eq.siena.weapon = SienaAuraList {
            registered: vec![
                RegisteredSienaAura { id: 1, label: "普段用".into(), aura: low },
                RegisteredSienaAura { id: 2, label: "火力用".into(), aura: high },
            ],
            selected_id: Some(2),
        };

        assert!(eq.validate().is_ok());
        assert!((eq.siena_attack_rate() - 0.09).abs() < 1e-12);
        eq.siena.weapon.selected_id = Some(1);
        assert!((eq.siena_attack_rate() - 0.03).abs() < 1e-12);
        eq.siena.weapon.selected_id = None;
        assert_eq!(eq.siena_attack_rate(), 0.0);
        assert_eq!(eq.siena.weapon.registered.len(), 2);
    }

    #[test]
    fn シエナ登録の未知選択idと重複idは拒否する() {
        let mut eq = Equipment::default();
        set_siena(&mut eq, PartSlot::Weapon, siena_values(1, 0));
        eq.siena.weapon.selected_id = Some(2);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::UnknownSelectedId {
                slot: PartSlot::Weapon
            }))
        ));

        eq.siena.weapon.selected_id = Some(1);
        eq.siena.weapon.registered.push(RegisteredSienaAura {
            id: 1,
            label: "重複".into(),
            aura: siena_values(1, 0),
        });
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::DuplicateRegistrationId {
                slot: PartSlot::Weapon,
                id: 1
            }))
        ));
    }

    #[test]
    fn 全ステータス増加は全ステに同じ値が乗る() {
        let mut eq = Equipment::default();
        // 武器・盾も追加オプションは持てる(wiki: 追加オプションは全ての部位で共通)
        let mut weapon = siena_stage(3, SienaValueKind::Thrust);
        weapon.extras.push(SienaExtraSlot { kind: SienaExtraKind::AllStats, value: 30.0 });
        set_siena(&mut eq, PartSlot::Weapon, weapon);
        set_siena(&mut eq, PartSlot::Helm, SienaAura {
            slots: vec![
                SienaSlot { kind: SienaValueKind::Stab, value: 10 },
                SienaSlot { kind: SienaValueKind::Stab, value: 2 },
                SienaSlot { kind: SienaValueKind::Def, value: 1 },
            ],
            extras: vec![SienaExtraSlot { kind: SienaExtraKind::AllStats, value: 21.0 }],
        });
        assert!(eq.validate().is_ok());

        // 武器 30 + 兜 21 が全ステに乗り、STAB だけ能力値スロットの 12 が上乗せされる
        let total = eq.siena_stat_bonus();
        assert_eq!(total.stab, 30 + 21 + 12);
        assert_eq!(total.def, 51 + 1);
        for kind in [StatKind::Hack, StatKind::Int, StatKind::Mr, StatKind::Dex, StatKind::Agi] {
            assert_eq!(total.get(kind), 51);
        }
    }

    #[test]
    fn シエナのオーラの部位制約() {
        // 武器・盾以外は装備補正の能力値を持てない
        let mut eq = Equipment::default();
        set_siena(&mut eq, PartSlot::Helm, siena_values(1, 0));
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::KindNotAllowed { slot: PartSlot::Helm, .. }))
        ));

        // 武器・盾はステ加算を持てない
        let mut eq = Equipment::default();
        set_siena(&mut eq, PartSlot::Weapon, siena_stage(1, SienaValueKind::Stab));
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::KindNotAllowed { slot: PartSlot::Weapon, .. }))
        ));

        // 段階(= スロット数)の上限
        let mut eq = Equipment::default();
        set_siena(&mut eq, PartSlot::Armor, siena_stage(SIENA_STAGE_MAX + 1, SienaValueKind::Stab));
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::TooManySlots { .. }))
        ));
    }

    #[test]
    fn テシスコアは対象地域のときだけ強化能力値に入る() {
        use crate::thesis_core::{CoreSet, CoreType, ThesisCore, CORE_SLOT_COUNT};

        let mut eq = Equipment::default();
        eq.parts.weapon.enchant = EquipmentValues { slash: 100, ..Default::default() };
        *eq.thesis_cores.get_mut(CoreRegion::Abyss) = CoreSet {
            slots: [Some(ThesisCore { core_type: CoreType::Slash, evolution: 4, enhancement: 4 });
                CORE_SLOT_COUNT],
        };
        assert!(eq.validate().is_ok());

        assert_eq!(eq.enhanced_totals(Some(CoreRegion::Abyss)).slash, 100 + 480);
        assert_eq!(eq.enhanced_totals(Some(CoreRegion::Eclipse)).slash, 100);
        assert_eq!(eq.enhanced_totals(None).slash, 100);
    }

    // --- ランダムオプション ---------------------------------------------

    fn ro_defs() -> Vec<RandomOptionDef> {
        use crate::random_option::{RandomOptionEffect, RandomOptionRank, RandomOptionTier};
        use crate::skill::SkillDependency;

        const TIERS: &[RandomOptionTier] =
            &[RandomOptionTier { rank: RandomOptionRank::Special, min: 10.0, max: 25.0 }];
        vec![
            RandomOptionDef {
                id: "shield-dep",
                name: "物理複合攻撃力が増加",
                slot: PartSlot::Shield,
                category: 15,
                effect: RandomOptionEffect::DependencyDamageRate(SkillDependency::StabHack),
                tiers: TIERS,
                note: "",
                common: false,
                short: "テスト",
            },
            RandomOptionDef {
                id: "hand-acc",
                name: "命中率が増加",
                slot: PartSlot::Hand,
                category: 10,
                effect: RandomOptionEffect::AccuracyPoint,
                tiers: TIERS,
                note: "",
                common: false,
                short: "テスト",
            },
        ]
    }

    fn ro(id: &str) -> RandomOptionSlot {
        use crate::random_option::RandomOptionRank;
        RandomOptionSlot { option_id: id.to_string(), rank: RandomOptionRank::Special, value: None }
    }

    #[test]
    fn ランダムオプションは全部位から集計される() {
        use crate::skill::SkillDependency;

        let mut eq = Equipment::default();
        eq.parts.shield.random_options = vec![ro("shield-dep")];
        eq.parts.hand.random_options = vec![ro("hand-acc")];
        assert!(eq.validate().is_ok());

        let totals = eq.random_option_totals(&ro_defs());
        // 上書きが無いのでレンジ上限 25% → 0.25
        assert_eq!(totals.dependency_damage_rate.get(SkillDependency::StabHack), 0.25);
        assert_eq!(totals.accuracy_point, 25);
    }

    #[test]
    fn カタログに無いランダムオプションidは集計されない() {
        let mut eq = Equipment::default();
        eq.parts.shield.random_options = vec![ro("nope")];
        assert_eq!(eq.random_option_totals(&ro_defs()), RandomOptionTotals::default());
    }

    // wiki: 装備システム冒頭の表「転移」行に 効果・AF は無い
    #[test]
    fn 効果とafはランダムオプションを持てない() {
        for slot in [PartSlot::Effect, PartSlot::Artifact] {
            let mut eq = Equipment::default();
            eq.parts.get_mut(slot).random_options = vec![ro("shield-dep")];
            assert!(matches!(
                eq.validate(),
                Err(EquipmentError::RandomOption(RandomOptionError::NotAllowed { .. }))
            ));
        }
    }

    #[test]
    fn ランダムオプションの効果値の上書きは値域を検証する() {
        let mut eq = Equipment::default();
        let mut option = ro("shield-dep");
        option.value = Some(RANDOM_OPTION_VALUE_MAX + 1.0);
        eq.parts.shield.random_options = vec![option];
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::RandomOption(RandomOptionError::ValueOutOfRange { .. }))
        ));
    }

    // --- 称号 -----------------------------------------------------------

    fn title_defs() -> Vec<crate::title::TitleDef> {
        use crate::title::{TitleDef, TitleKind};
        vec![TitleDef {
            id: "eclipse",
            name: "エクリプス",
            kind: TitleKind::Special,
            group: "喪失の島",
            level: None,
            values: EquipmentValues { thrust: 40, slash: 40, ..Default::default() },
            attack_damage_percent: 0.0,
            conditional_added_damage: None,
            note: "",
        }]
    }

    // wiki: 称号システム。表示中の 1 件だけが基本能力値に乗る
    #[test]
    fn 称号は基本能力値に合流する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.base = EquipmentValues { thrust: 100, ..Default::default() };
        assert_eq!(eq.base_totals(&[], &title_defs()).thrust, 100);

        eq.title = Some("eclipse".to_string());
        assert_eq!(eq.base_totals(&[], &title_defs()).thrust, 140);
        assert_eq!(eq.base_totals(&[], &title_defs()).slash, 40);
        // 強化能力値には入らない(称号にエンチャントは無い)
        assert_eq!(eq.enhanced_totals(None), EquipmentValues::default());
    }

    #[test]
    fn カタログに無い称号idは加算されない() {
        let mut eq = Equipment::default();
        eq.title = Some("nope".to_string());
        assert_eq!(eq.base_totals(&[], &title_defs()), EquipmentValues::default());
    }
    #[test]
    fn レリックの付加オプションは2枠まで() {
        let mut part = EquipmentPart::default();
        let op = |id: &str| RandomOptionSlot {
            option_id: id.to_string(),
            rank: crate::random_option::RandomOptionRank::Special,
            value: None,
        };
        part.random_options = vec![op("a"), op("b")];
        assert!(part.validate(PartSlot::RelicPendant).is_ok());
        part.random_options.push(op("c"));
        assert!(matches!(
            part.validate(PartSlot::RelicPendant),
            Err(EquipmentError::RandomOption(RandomOptionError::TooMany { .. }))
        ));
        // 武器だけ 3 枠
        assert!(part.validate(PartSlot::Weapon).is_ok());
        part.random_options.push(op("d"));
        assert!(matches!(
            part.validate(PartSlot::Weapon),
            Err(EquipmentError::RandomOption(RandomOptionError::TooMany { .. }))
        ));
    }

}
