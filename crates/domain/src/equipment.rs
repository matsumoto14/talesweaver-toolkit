//! 装備補正(wiki: カテゴリA の内訳「装備攻撃力」)。docs/damage-formula.md §4 A、§5(武器強化)。
//!
//! 装備は部位別(12 スロット)で持つ(docs/claude/goals/2026-08-24-equipment-parts.md)。
//! 「基本能力値」= 部位ごとの実測補正値 + 武器アビリティの加算。
//! 「強化能力値」= 部位ごとのエンチャント値 + シエナのオーラの能力値(武器/盾)+ テシスコア。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stats::StatKind;
use crate::thesis_core::{CoreRegion, ThesisCoreError, ThesisCores};

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
/// 未記載。カタログ外のカスタム入力に掛ける安全域として暫定採用)`[仮]`。
pub const EQUIPMENT_VALUE_MAX: i64 = 9999;
/// ストロングウェポンの Lv 上限(wiki Skill/共通: Lv1〜6)。
pub const STRONG_WEAPON_LEVEL_MAX: u8 = 6;
/// 装備強化の Lv 上限(wiki: 装備システム/装備強化。+1〜+15)。
pub const ENHANCE_LEVEL_MAX: u8 = 15;
/// +12 以上で追加固定ダメージがレンジ振り(MR)になる境界(wiki: +11 覚醒までは確定値)。
pub const ENHANCE_LEVEL_RANDOM_RANGE_MIN: u8 = 12;
/// +12 以上の追加固定ダメージ実測値の上限(wiki に明記なし。+15 最上位帯でも数百万に収まる
/// 実用上の安全域として暫定採用)`[仮]`。
pub const ENHANCE_ADDED_DAMAGE_MAX: i64 = 9_999_999;
/// シエナのオーラの増幅段階の上限(wiki: 装備システム/シエナのオーラ「発現・増幅」0→1〜9→10)。
pub const SIENA_STAGE_MAX: u8 = 10;
/// シエナのオーラの追加オプション「攻撃力増加」の 1 部位あたり上限 %(wiki: 追加オプション一覧の
/// 最大 8〜10%。同じ種類のオプションは同じ装備の別スロットには登場しないため 1 部位 1 個)。
pub const SIENA_ATTACK_RATE_PERCENT_MAX: f64 = 10.0;
/// シエナのオーラの能力値スロットによるステ加算の 1 部位・1 ステあたり上限
/// (wiki: 能力値一覧(その他の部位)の STAB〜AGI は 1〜10。段階 10 = 10 スロットが全部同じステに
/// 乗った場合の 100)。
pub const SIENA_STAT_BONUS_MAX: i64 = 100;
/// シエナのオーラの追加オプション「全ステータス増加」の 1 部位あたり上限
/// (wiki: 追加オプション一覧の最大帯 21〜30。同じ種類のオプションは同じ装備の別スロットには
/// 登場しないため 1 部位 1 個)。STAB〜AGI の全ステにこの値がそのまま加算される。
pub const SIENA_ALL_STATS_BONUS_MAX: i64 = 30;

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
    Relic,
}

impl PartSlot {
    /// この部位が装備強化(+1〜+15)を持てるか(wiki: 装備システム/装備強化。武器・鎧のみ)。
    pub fn allows_enhance(self) -> bool {
        matches!(self, PartSlot::Weapon | PartSlot::Armor)
    }

    /// この部位が武器アビリティを持てるか(wiki: 装備システム/アビリティ。武器のみが火力に効く)。
    pub fn allows_abilities(self) -> bool {
        matches!(self, PartSlot::Weapon)
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

    fn is_zero(&self) -> bool {
        StatKind::ALL.iter().all(|k| self.get(*k) == 0)
    }

    fn add(self, other: SienaStatBonus) -> SienaStatBonus {
        let mut total = self;
        for kind in StatKind::ALL {
            *total.get_mut(kind) += other.get(kind);
        }
        total
    }
}

/// シエナのオーラ(wiki: 装備システム/シエナのオーラ)。Lv310 装備の 8 部位に発現できる。
///
/// 能力値・追加オプションはどちらも再抽選のランダム値なので、wiki から静的データとして
/// 与えられる値は無い(段階ごとの解放スロット数だけが決まっている)。部位ごとの実測値を
/// ユーザーが入力する。
///
/// - `values`(武器/盾): 装備補正がエンチャント扱いで増加 → 強化能力値へ合流
/// - `stats`(その他の部位): 能力値スロットの STAB〜AGI。最終固定値増加
/// - `all_stats`(全部位共通の追加オプション「全ステータス増加」): STAB〜AGI の全ステに
///   この値がそのまま加算される(最終固定値増加)
/// - `attack_rate_percent`(全部位共通の追加オプション「攻撃力増加」): 実際は与ダメージ割合増加
///   = カテゴリ New1(`DamageCategory::SienaAuraAttackRate`)
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct SienaAura {
    /// 増幅段階 0..=10(0 = 未発現)。解放される能力値スロット数と等しい。
    /// 計算には使わず、入力値の妥当性の目安として持つ
    #[serde(default)]
    pub stage: u8,
    /// 能力値の合計(武器/盾のみ)。強化能力値へ合流する
    #[serde(default)]
    pub values: EquipmentValues,
    /// 能力値スロットのステ加算(武器/盾以外)。最終固定値層へ合流する
    #[serde(default)]
    pub stats: SienaStatBonus,
    /// 追加オプション「全ステータス増加」。全ステに同じ値が乗る(部位を問わない)
    #[serde(default)]
    pub all_stats: i64,
    /// 追加オプション「攻撃力増加」の % (New1)
    #[serde(default)]
    pub attack_rate_percent: f64,
}

impl SienaAura {
    /// この部位のステ加算の合計(能力値スロット + 追加オプション「全ステータス増加」)。
    pub fn stat_bonus(&self) -> SienaStatBonus {
        let mut total = self.stats;
        if self.all_stats != 0 {
            for kind in StatKind::ALL {
                *total.get_mut(kind) += self.all_stats;
            }
        }
        total
    }

    fn is_neutral(&self) -> bool {
        self.stage == 0
            && self.values == EquipmentValues::default()
            && self.stats.is_zero()
            && self.all_stats == 0
            && self.attack_rate_percent == 0.0
    }

    fn validate(&self, slot: PartSlot) -> Result<(), EquipmentError> {
        if self.is_neutral() {
            return Ok(());
        }
        if !slot.allows_siena() {
            return Err(EquipmentError::SienaNotAllowed { slot });
        }
        if self.stage > SIENA_STAGE_MAX {
            return Err(EquipmentError::SienaStageOutOfRange {
                slot,
                value: self.stage,
                max: SIENA_STAGE_MAX,
            });
        }
        self.values.validate()?;
        if !slot.siena_values_are_equipment() && self.values != EquipmentValues::default() {
            return Err(EquipmentError::SienaValuesNotAllowed { slot });
        }
        if slot.siena_values_are_equipment() && !self.stats.is_zero() {
            return Err(EquipmentError::SienaStatsNotAllowed { slot });
        }
        for kind in StatKind::ALL {
            let value = self.stats.get(kind);
            if !(0..=SIENA_STAT_BONUS_MAX).contains(&value) {
                return Err(EquipmentError::SienaStatOutOfRange {
                    slot,
                    value,
                    max: SIENA_STAT_BONUS_MAX,
                });
            }
        }
        if !(0..=SIENA_ALL_STATS_BONUS_MAX).contains(&self.all_stats) {
            return Err(EquipmentError::SienaAllStatsOutOfRange {
                slot,
                value: self.all_stats,
                max: SIENA_ALL_STATS_BONUS_MAX,
            });
        }
        if !(0.0..=SIENA_ATTACK_RATE_PERCENT_MAX).contains(&self.attack_rate_percent) {
            return Err(EquipmentError::SienaAttackRateOutOfRange {
                slot,
                value: self.attack_rate_percent,
                max: SIENA_ATTACK_RATE_PERCENT_MAX,
            });
        }
        Ok(())
    }
}

/// 装備部位 1 つ。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentPart {
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
    /// +12 以上の追加固定ダメージ実測値の上書き。+11 以下は式で確定するため `None` 固定
    #[serde(default)]
    pub enhance_added_damage: Option<i64>,
    /// 装備アビリティ id(武器のみ非空を許可)
    #[serde(default)]
    pub abilities: Vec<String>,
    /// シエナのオーラ(発現できるのは 8 部位。未発現は中立値)
    #[serde(default)]
    pub siena: SienaAura,
}

impl EquipmentPart {
    fn validate(&self, slot: PartSlot) -> Result<(), EquipmentError> {
        self.base.validate()?;
        self.enchant.validate()?;
        self.siena.validate(slot)?;
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
        if self.enhance_added_damage.is_some() && self.enhance_level < ENHANCE_LEVEL_RANDOM_RANGE_MIN {
            return Err(EquipmentError::EnhanceAddedDamageNotAllowed { slot, enhance_level: self.enhance_level });
        }
        if let Some(added) = self.enhance_added_damage {
            if !(0..=ENHANCE_ADDED_DAMAGE_MAX).contains(&added) {
                return Err(EquipmentError::EnhanceAddedDamageOutOfRange {
                    slot,
                    value: added,
                    max: ENHANCE_ADDED_DAMAGE_MAX,
                });
            }
        }
        if !self.abilities.is_empty() && !slot.allows_abilities() {
            return Err(EquipmentError::AbilitiesNotAllowed { slot });
        }
        Ok(())
    }
}

/// 装備補正の値域・部位制約違反。
/// (シエナのオーラの攻撃力増加が % の実数なので `Eq` は導出しない)
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum EquipmentError {
    #[error("装備補正の{field}は 0〜{max} の範囲で指定してください(指定値 {value})")]
    ValueOutOfRange { field: &'static str, value: i64, max: i64 },
    #[error("ストロングウェポンの Lv は 0〜{max} です(指定値 {value})")]
    StrongWeaponLevelOutOfRange { value: u8, max: u8 },
    #[error("{slot:?} の装備強化 Lv は 0〜{max} です(指定値 {value})")]
    EnhanceLevelOutOfRange { slot: PartSlot, value: u8, max: u8 },
    #[error("{slot:?} は装備強化の対象外です(武器・鎧のみ)")]
    EnhanceNotAllowed { slot: PartSlot },
    #[error("{slot:?} の追加固定ダメージ上書きは強化 Lv {enhance_level} では指定できません(+12 以上のみ)")]
    EnhanceAddedDamageNotAllowed { slot: PartSlot, enhance_level: u8 },
    #[error("{slot:?} の追加固定ダメージは 0〜{max} の範囲で指定してください(指定値 {value})")]
    EnhanceAddedDamageOutOfRange { slot: PartSlot, value: i64, max: i64 },
    #[error("{slot:?} は装備アビリティの対象外です(武器のみ)")]
    AbilitiesNotAllowed { slot: PartSlot },
    #[error("{slot:?} はシエナのオーラの対象外です(兜/鎧/武器/盾/頭/体/手/足のみ)")]
    SienaNotAllowed { slot: PartSlot },
    #[error("{slot:?} のシエナのオーラの増幅段階は 0〜{max} です(指定値 {value})")]
    SienaStageOutOfRange { slot: PartSlot, value: u8, max: u8 },
    #[error("{slot:?} のシエナのオーラは装備補正の能力値を持ちません(武器・盾のみ)")]
    SienaValuesNotAllowed { slot: PartSlot },
    #[error("{slot:?} のシエナのオーラはステータス加算を持ちません(武器・盾は装備補正)")]
    SienaStatsNotAllowed { slot: PartSlot },
    #[error("{slot:?} のシエナのオーラのステータス加算は 0〜{max} です(指定値 {value})")]
    SienaStatOutOfRange { slot: PartSlot, value: i64, max: i64 },
    #[error("{slot:?} のシエナのオーラの全ステータス増加は 0〜{max} です(指定値 {value})")]
    SienaAllStatsOutOfRange { slot: PartSlot, value: i64, max: i64 },
    #[error("{slot:?} のシエナのオーラの攻撃力増加は 0〜{max}% です(指定値 {value})")]
    SienaAttackRateOutOfRange { slot: PartSlot, value: f64, max: f64 },
    #[error(transparent)]
    ThesisCore(#[from] ThesisCoreError),
}

/// 武器アビリティの系統(wiki: 装備システム/アビリティ。尖った刃/鋭い刃/知力/耐魔力)。
/// 同じ系統は 1 部位に 1 つだけ付く(段が違っても併用できない)。
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
}

impl EquipmentAbilityFamily {
    pub const ALL: [EquipmentAbilityFamily; 4] = [
        EquipmentAbilityFamily::PointedBlade,
        EquipmentAbilityFamily::SharpBlade,
        EquipmentAbilityFamily::Intelligence,
        EquipmentAbilityFamily::MagicResistance,
    ];
}

/// 武器アビリティ定義(gamedata がカタログを持つ。domain の `BuffDefinition` と同じ依存方向)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EquipmentAbilityDef {
    pub id: &'static str,
    pub name: &'static str,
    /// 系統。1 部位につき同じ系統は 1 つまで
    pub family: EquipmentAbilityFamily,
    /// 装備攻撃力(基本能力値)への加算値
    pub values: EquipmentValues,
}

/// キャラの装備補正一式(部位別装備 12 スロット + パワーウェポン/ストロングウェポン)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Equipment {
    #[serde(default)]
    pub parts: EquipmentParts,
    /// パワーウェポン(wiki Skill/共通: 自身の装備補正を2%増加。Lv1 のみ、ストロングウェポンと重複可)
    #[serde(default)]
    pub power_weapon: bool,
    /// ストロングウェポンの Lv(0 = 未使用、1〜6 = 該当 Lv。wiki Skill/共通: 3/6/9/12/15/18%)
    #[serde(default)]
    pub strong_weapon_level: u8,
    /// テシスコア(地域ごとに 6 枠)。火力タイプの補正は強化能力値へ合流する
    #[serde(default)]
    pub thesis_cores: ThesisCores,
}

/// 12 部位。named field で持つ(`parts.weapon` 等)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentParts {
    #[serde(default)]
    pub weapon: EquipmentPart,
    #[serde(default)]
    pub armor: EquipmentPart,
    #[serde(default)]
    pub helm: EquipmentPart,
    #[serde(default)]
    pub shield: EquipmentPart,
    #[serde(default)]
    pub shield_plus: EquipmentPart,
    #[serde(default)]
    pub head: EquipmentPart,
    #[serde(default)]
    pub body: EquipmentPart,
    #[serde(default)]
    pub hand: EquipmentPart,
    #[serde(default)]
    pub leg: EquipmentPart,
    #[serde(default)]
    pub effect: EquipmentPart,
    #[serde(default)]
    pub artifact: EquipmentPart,
    #[serde(default)]
    pub relic: EquipmentPart,
}

impl EquipmentParts {
    /// 12 部位を `(PartSlot, &EquipmentPart)` で列挙する。
    pub fn iter(&self) -> [(PartSlot, &EquipmentPart); 12] {
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
            (PartSlot::Relic, &self.relic),
        ]
    }

    /// 部位を可変で引く。
    pub fn get_mut(&mut self, slot: PartSlot) -> &mut EquipmentPart {
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
            PartSlot::Relic => &mut self.relic,
        }
    }
}

impl Equipment {
    pub fn validate(&self) -> Result<(), EquipmentError> {
        for (slot, part) in self.parts.iter() {
            part.validate(slot)?;
        }
        if self.strong_weapon_level > STRONG_WEAPON_LEVEL_MAX {
            return Err(EquipmentError::StrongWeaponLevelOutOfRange {
                value: self.strong_weapon_level,
                max: STRONG_WEAPON_LEVEL_MAX,
            });
        }
        self.thesis_cores.validate()?;
        Ok(())
    }

    /// 基本能力値の合計(Σ part.base + Σ 武器アビリティの加算値)。
    pub fn base_totals(&self, abilities: &[EquipmentAbilityDef]) -> EquipmentValues {
        let mut total = EquipmentValues::default();
        for (_, part) in self.parts.iter() {
            total = total.add(part.base);
        }
        for ability_id in &self.parts.weapon.abilities {
            if let Some(def) = abilities.iter().find(|a| a.id == *ability_id) {
                total = total.add(def.values);
            }
        }
        total
    }

    /// 強化能力値の合計(Σ part.enchant + Σ シエナのオーラの能力値(武器/盾)+ テシスコア)。
    ///
    /// `region` はダメージ計算の対象コンテンツのテシスコア地域。テシスコアの能力値増加は
    /// 対象ダンジョン内でのみ有効なので、`None`(コアが効かないコンテンツ)なら加算しない。
    pub fn enhanced_totals(&self, region: Option<CoreRegion>) -> EquipmentValues {
        let mut total = EquipmentValues::default();
        for (slot, part) in self.parts.iter() {
            total = total.add(part.enchant);
            if slot.siena_values_are_equipment() {
                total = total.add(part.siena.values);
            }
        }
        total.add(self.thesis_cores.equipment_values(region))
    }

    /// シエナのオーラの追加オプション「攻撃力増加」の合計(wiki: New1)。Σ% の小数表現。
    pub fn siena_attack_rate(&self) -> f64 {
        self.parts.iter().into_iter().map(|(_, part)| part.siena.attack_rate_percent).sum::<f64>()
            / 100.0
    }

    /// シエナのオーラによるステ加算の合計(能力値スロット + 全ステータス増加。最終固定値層に乗る)。
    pub fn siena_stat_bonus(&self) -> SienaStatBonus {
        let mut total = SienaStatBonus::default();
        for (_, part) in self.parts.iter() {
            total = total.add(part.siena.stat_bonus());
        }
        total
    }

    /// 装備攻撃力強化倍率(wiki: カテゴリA の内訳)。
    /// パワーウェポン(+2%)+ ストロングウェポン Lv × 3%。
    /// その部位だけを未装備(中立値)にした複製。部位ごとの寄与(外したときの差分)を出すのに使う。
    pub fn without_part(&self, slot: PartSlot) -> Equipment {
        let mut copy = self.clone();
        *copy.parts.get_mut(slot) = EquipmentPart::default();
        copy
    }

    pub fn enhance_rate(&self) -> f64 {
        let power_weapon_rate = if self.power_weapon { 0.02 } else { 0.0 };
        power_weapon_rate + f64::from(self.strong_weapon_level) * 0.03
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

    fn coefficients() -> EquipmentCoefficients {
        EquipmentCoefficients {
            base: EquipmentRates { thrust: 14.5, slash: 14.5, magic_attack: 0.0, magic_defense: 0.0 },
            enhanced: EquipmentRates { thrust: 28.75, slash: 28.75, magic_attack: 0.0, magic_defense: 0.0 },
        }
    }

    fn equipment_with(weapon_base: EquipmentValues, weapon_enchant: EquipmentValues) -> Equipment {
        Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart { base: weapon_base, enchant: weapon_enchant, ..Default::default() },
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
        let base = eq.base_totals(&[]);
        let enhanced = eq.enhanced_totals(None);
        // 150*14.5*2 + 60*28.75*2 = 4350 + 3450 = 7800
        assert!((equipment_attack_power(&base, &enhanced, &coefficients()) - 7800.0).abs() < 1e-9);
    }

    #[test]
    fn 装備なしなら装備攻撃力は0() {
        let eq = Equipment::default();
        let base = eq.base_totals(&[]);
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
            values: EquipmentValues { slash: 9, ..Default::default() },
        }];
        let base = eq.base_totals(&abilities);
        assert_eq!(base, EquipmentValues { thrust: 100, slash: 209, magic_defense: 50, ..Default::default() });

        let enhanced = eq.enhanced_totals(None);
        assert_eq!(enhanced, EquipmentValues { thrust: 10, slash: 20, ..Default::default() });
    }

    #[test]
    fn 強化倍率はパワーウェポンとストロングウェポンの合計() {
        assert_eq!(Equipment::default().enhance_rate(), 0.0);
        let pw = Equipment { power_weapon: true, ..Default::default() };
        assert!((pw.enhance_rate() - 0.02).abs() < 1e-12);
        let sw6 = Equipment { strong_weapon_level: 6, ..Default::default() };
        assert!((sw6.enhance_rate() - 0.18).abs() < 1e-12);
        let both = Equipment { power_weapon: true, strong_weapon_level: 6, ..Default::default() };
        assert!((both.enhance_rate() - 0.20).abs() < 1e-12);
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
        eq.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::StrongWeaponLevelOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX;
        eq.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX;
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
    fn 武器以外の強化レベルは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.helm.enhance_level = 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceNotAllowed { slot: PartSlot::Helm })));

        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = ENHANCE_LEVEL_MAX;
        assert!(eq.validate().is_ok());
        let mut eq2 = Equipment::default();
        eq2.parts.armor.enhance_level = ENHANCE_LEVEL_MAX;
        assert!(eq2.validate().is_ok());

        let mut over = Equipment::default();
        over.parts.weapon.enhance_level = ENHANCE_LEVEL_MAX + 1;
        assert!(matches!(over.validate(), Err(EquipmentError::EnhanceLevelOutOfRange { .. })));
    }

    #[test]
    fn 強化11以下でのadded_damage上書きは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 11;
        eq.parts.weapon.enhance_added_damage = Some(100);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::EnhanceAddedDamageNotAllowed { slot: PartSlot::Weapon, .. })
        ));

        let mut ok = Equipment::default();
        ok.parts.weapon.enhance_level = 12;
        ok.parts.weapon.enhance_added_damage = Some(140);
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn 武器以外のアビリティは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.helm.abilities = vec!["sharp-blade-e".to_string()];
        assert!(matches!(eq.validate(), Err(EquipmentError::AbilitiesNotAllowed { slot: PartSlot::Helm })));

        let mut ok = Equipment::default();
        ok.parts.weapon.abilities = vec!["sharp-blade-e".to_string()];
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
    fn 追加固定ダメージ上書きの値域違反は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 12;
        eq.parts.weapon.enhance_added_damage = Some(-1);
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceAddedDamageOutOfRange { .. })));

        eq.parts.weapon.enhance_added_damage = Some(ENHANCE_ADDED_DAMAGE_MAX + 1);
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceAddedDamageOutOfRange { .. })));

        eq.parts.weapon.enhance_added_damage = Some(ENHANCE_ADDED_DAMAGE_MAX);
        assert!(eq.validate().is_ok());
    }

    fn siena_values(thrust: i64, slash: i64) -> SienaAura {
        SienaAura {
            stage: 10,
            values: EquipmentValues { thrust, slash, ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn シエナのオーラの能力値は武器と盾だけ強化能力値に入る() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enchant = EquipmentValues { thrust: 10, ..Default::default() };
        eq.parts.weapon.siena = siena_values(6, 4);
        eq.parts.shield.siena = siena_values(0, 5);
        // 武器・盾以外はステ加算(装備補正には入らない)
        eq.parts.helm.siena = SienaAura {
            stage: 5,
            stats: SienaStatBonus { stab: 20, hack: 10, ..Default::default() },
            ..Default::default()
        };
        assert!(eq.validate().is_ok());

        assert_eq!(
            eq.enhanced_totals(None),
            EquipmentValues { thrust: 16, slash: 9, ..Default::default() }
        );
        assert_eq!(eq.base_totals(&[]), EquipmentValues::default());
        assert_eq!(
            eq.siena_stat_bonus(),
            SienaStatBonus { stab: 20, hack: 10, ..Default::default() }
        );
    }

    #[test]
    fn シエナのオーラの攻撃力増加は全部位の合計() {
        let mut eq = Equipment::default();
        eq.parts.weapon.siena.attack_rate_percent = 10.0;
        eq.parts.armor.siena.attack_rate_percent = 3.0;
        eq.parts.leg.siena.attack_rate_percent = 2.5;
        assert!(eq.validate().is_ok());
        assert!((eq.siena_attack_rate() - 0.155).abs() < 1e-12);
        assert_eq!(Equipment::default().siena_attack_rate(), 0.0);
    }

    #[test]
    fn 全ステータス増加は全ステに同じ値が乗る() {
        let mut eq = Equipment::default();
        // 武器・盾も追加オプションは持てる(wiki: 追加オプションは全ての部位で共通)
        eq.parts.weapon.siena = SienaAura { stage: 10, all_stats: 30, ..Default::default() };
        eq.parts.helm.siena = SienaAura {
            stage: 5,
            stats: SienaStatBonus { stab: 12, ..Default::default() },
            all_stats: 21,
            ..Default::default()
        };
        assert!(eq.validate().is_ok());

        // 武器 30 + 兜 21 が全ステに乗り、STAB だけ能力値スロットの 12 が上乗せされる
        let total = eq.siena_stat_bonus();
        assert_eq!(total.stab, 30 + 21 + 12);
        for kind in [StatKind::Hack, StatKind::Int, StatKind::Def, StatKind::Mr, StatKind::Dex, StatKind::Agi] {
            assert_eq!(total.get(kind), 51);
        }

        // 上限超過は拒否する
        eq.parts.weapon.siena.all_stats = SIENA_ALL_STATS_BONUS_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::SienaAllStatsOutOfRange { .. })));
    }

    #[test]
    fn シエナのオーラの部位制約と値域() {
        // 盾+ / 効果 / AF / レリックは発現できない
        let mut eq = Equipment::default();
        eq.parts.shield_plus.siena.stage = 1;
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::SienaNotAllowed { slot: PartSlot::ShieldPlus })
        ));

        // 武器・盾以外は装備補正の能力値を持てない
        let mut eq = Equipment::default();
        eq.parts.helm.siena = siena_values(1, 0);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::SienaValuesNotAllowed { slot: PartSlot::Helm })
        ));

        // 武器・盾はステ加算を持てない
        let mut eq = Equipment::default();
        eq.parts.weapon.siena = SienaAura {
            stage: 1,
            stats: SienaStatBonus { stab: 1, ..Default::default() },
            ..Default::default()
        };
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::SienaStatsNotAllowed { slot: PartSlot::Weapon })
        ));

        let mut eq = Equipment::default();
        eq.parts.armor.siena.stage = SIENA_STAGE_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::SienaStageOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.armor.siena.attack_rate_percent = SIENA_ATTACK_RATE_PERCENT_MAX + 0.1;
        assert!(matches!(eq.validate(), Err(EquipmentError::SienaAttackRateOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.armor.siena.stats.agi = SIENA_STAT_BONUS_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::SienaStatOutOfRange { .. })));
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
}
