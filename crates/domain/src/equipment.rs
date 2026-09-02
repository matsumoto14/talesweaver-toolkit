//! 装備補正(wiki: カテゴリA の内訳「装備攻撃力」)。docs/damage-formula.md §4 A、§5(武器強化)。
//!
//! 装備は部位別(12 スロット)で持つ(docs/claude/goals/2026-08-24-equipment-parts.md)。
//! 「基本能力値」= 部位ごとの実測補正値 + 武器アビリティの加算。
//! 「強化能力値」= 部位ごとのエンチャント値 + シエナのオーラの能力値(武器/盾)+ テシスコア。

use crate::category::DamageCategory;
use crate::character_skill::{damage_contributions, SkillEffect};
use crate::damage::DamageContribution;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::element::{Element, ElementValues};
use crate::equipment_class::{WeaponClass, WeaponSystem};
use crate::random_option::{
    RandomOptionDef, RandomOptionError, RandomOptionSlot, RandomOptionTotals,
    RANDOM_OPTION_VALUE_MAX,
};
use crate::siena::{SienaAuras, SienaError};
use crate::stats::StatKind;
use crate::thesis_core::{CoreRegion, ThesisCoreError, ThesisCores};
use crate::title::{title_values, TitleDef};
use crate::validation::{ValidationError, ValidationLocation};

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

/// 装備補正 9 値の種別。`EquipmentValues` のどのフィールドかを値で指す。
/// (装備攻撃力に効く 4 種だけを指す `EquipmentValueKind` とは別。こちらは 9 値すべて)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentStatKind {
    Thrust,
    Slash,
    PhysicalDefense,
    MagicAttack,
    MagicDefense,
    Accuracy,
    Critical,
    Evasion,
    Agility,
}

impl EquipmentStatKind {
    pub const ALL: [EquipmentStatKind; 9] = [
        EquipmentStatKind::Thrust,
        EquipmentStatKind::Slash,
        EquipmentStatKind::PhysicalDefense,
        EquipmentStatKind::MagicAttack,
        EquipmentStatKind::MagicDefense,
        EquipmentStatKind::Accuracy,
        EquipmentStatKind::Critical,
        EquipmentStatKind::Evasion,
        EquipmentStatKind::Agility,
    ];
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
/// 画面で選べる強化 Lv。0 = 強化なし。+10 未満は実用しないので出さず、+10〜+15 を並べる
/// (+12 以上は等級 `EnhanceGrade` つき)
pub const ENHANCE_LEVEL_CANDIDATES: [u8; 7] = [0, 10, 11, 12, 13, 14, 15];
/// +12〜+15 の追加固定ダメージ等級。各等級は wiki の確率区分
/// (10/30/70/95/100%)の上端を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnhanceGrade {
    Lowest,
    Low,
    Middle,
    High,
    Highest,
}

impl EnhanceGrade {
    pub fn percentile(self) -> f64 {
        match self {
            Self::Lowest => 0.10,
            Self::Low => 0.30,
            Self::Middle => 0.70,
            Self::High => 0.95,
            Self::Highest => 1.0,
        }
    }
}

impl EquipmentValues {
    /// 装備補正 9 値の表示名。唯一の正。`CoreType::label`(thesis_core.rs)は
    /// 対応する 8 種をここから引く(装備補正とテシスコアで敏捷度補正の表記が食い違わないようにする)。
    pub const THRUST_LABEL: &'static str = "突き攻撃力";
    pub const SLASH_LABEL: &'static str = "斬り攻撃力";
    pub const PHYSICAL_DEFENSE_LABEL: &'static str = "物理防御力";
    pub const MAGIC_ATTACK_LABEL: &'static str = "魔法攻撃力";
    pub const MAGIC_DEFENSE_LABEL: &'static str = "魔法防御力";
    pub const ACCURACY_LABEL: &'static str = "命中率補正";
    pub const CRITICAL_LABEL: &'static str = "クリティカル補正";
    pub const EVASION_LABEL: &'static str = "回避率補正";
    pub const AGILITY_LABEL: &'static str = "敏捷度補正";

    /// (表示名, 値)の 9 組。検証・UI ラベル・合計表示の唯一の並び順にする。
    pub fn fields(&self) -> [(&'static str, i64); 9] {
        [
            (Self::THRUST_LABEL, self.thrust),
            (Self::SLASH_LABEL, self.slash),
            (Self::PHYSICAL_DEFENSE_LABEL, self.physical_defense),
            (Self::MAGIC_ATTACK_LABEL, self.magic_attack),
            (Self::MAGIC_DEFENSE_LABEL, self.magic_defense),
            (Self::ACCURACY_LABEL, self.accuracy),
            (Self::CRITICAL_LABEL, self.critical),
            (Self::EVASION_LABEL, self.evasion),
            (Self::AGILITY_LABEL, self.agility),
        ]
    }

    /// (serde フィールド名, 表示名)の 9 組。`StatLimits::equipment_stat_labels` の元。
    pub const FIELD_LABELS: [(&'static str, &'static str); 9] = [
        ("thrust", Self::THRUST_LABEL),
        ("slash", Self::SLASH_LABEL),
        ("physical_defense", Self::PHYSICAL_DEFENSE_LABEL),
        ("magic_attack", Self::MAGIC_ATTACK_LABEL),
        ("magic_defense", Self::MAGIC_DEFENSE_LABEL),
        ("accuracy", Self::ACCURACY_LABEL),
        ("critical", Self::CRITICAL_LABEL),
        ("evasion", Self::EVASION_LABEL),
        ("agility", Self::AGILITY_LABEL),
    ];

    /// 種別で 1 値を取り出す。
    pub fn get(&self, kind: EquipmentStatKind) -> i64 {
        match kind {
            EquipmentStatKind::Thrust => self.thrust,
            EquipmentStatKind::Slash => self.slash,
            EquipmentStatKind::PhysicalDefense => self.physical_defense,
            EquipmentStatKind::MagicAttack => self.magic_attack,
            EquipmentStatKind::MagicDefense => self.magic_defense,
            EquipmentStatKind::Accuracy => self.accuracy,
            EquipmentStatKind::Critical => self.critical,
            EquipmentStatKind::Evasion => self.evasion,
            EquipmentStatKind::Agility => self.agility,
        }
    }

    fn validate(&self) -> Result<(), EquipmentError> {
        for (field, value) in self.fields() {
            if !(0..=EQUIPMENT_VALUE_MAX).contains(&value) {
                return Err(EquipmentError::ValueOutOfRange {
                    field,
                    value,
                    max: EQUIPMENT_VALUE_MAX,
                });
            }
        }
        Ok(())
    }

    /// 各フィールドを `caps` の対応値で頭打ちにする(装備更新でエンチャントが新しい枠を
    /// 超えないようにする。UI 側の `clampToCaps` と同じ)。
    pub fn clamp_to(self, caps: EquipmentValues) -> EquipmentValues {
        EquipmentValues {
            thrust: self.thrust.min(caps.thrust),
            slash: self.slash.min(caps.slash),
            physical_defense: self.physical_defense.min(caps.physical_defense),
            magic_attack: self.magic_attack.min(caps.magic_attack),
            magic_defense: self.magic_defense.min(caps.magic_defense),
            accuracy: self.accuracy.min(caps.accuracy),
            critical: self.critical.min(caps.critical),
            evasion: self.evasion.min(caps.evasion),
            agility: self.agility.min(caps.agility),
        }
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

/// 装備補正 1 塊の供給源(部位の基本値・部位アビリティ・称号・手首補正・エンチャント・
/// シエナのオーラ・テシスコア)。「なぜこの数字?」パネルの装備攻撃力掘り下げに使う。
/// `source` は人が読める名前(部位ラベル・称号名・地域名を組み込み済み)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentValueSource {
    pub source: String,
    pub values: EquipmentValues,
}

/// `EquipmentValueSource` の一覧を合算する(`base_totals`/`enhanced_totals` が使う。
/// 合計と内訳を二重に計算しない)。
pub fn sum_equipment_value_sources(sources: &[EquipmentValueSource]) -> EquipmentValues {
    sources
        .iter()
        .fold(EquipmentValues::default(), |acc, s| acc.add(s.values))
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
    /// 13 部位すべて(表示順 = wiki: 装備システム ページ冒頭の表の並び)。
    pub const ALL: [PartSlot; 13] = [
        PartSlot::Weapon,
        PartSlot::Armor,
        PartSlot::Helm,
        PartSlot::Shield,
        PartSlot::ShieldPlus,
        PartSlot::Head,
        PartSlot::Body,
        PartSlot::Hand,
        PartSlot::Leg,
        PartSlot::Effect,
        PartSlot::Artifact,
        PartSlot::RelicPendant,
        PartSlot::RelicBracelet,
    ];

    /// この部位が装備強化(+1〜+15)を持てるか(wiki: 装備システム/装備強化。武器・鎧のみ)。
    pub fn allows_enhance(self) -> bool {
        matches!(self, PartSlot::Weapon | PartSlot::Armor)
    }

    /// この部位が通常のエンチャント枠(装備システム/エンチャント)を持つか。
    /// 成長装備の盾＋とレリックは補正値を育てる別の入力モデルで、エンチャントを持たない。
    pub fn allows_enchant_plan(self) -> bool {
        !matches!(
            self,
            PartSlot::ShieldPlus | PartSlot::RelicPendant | PartSlot::RelicBracelet
        )
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
        Some(if matches!(self, PartSlot::Weapon) {
            3
        } else {
            2
        })
    }

    /// この部位が属性強化を持てるか
    /// (wiki: 装備システム冒頭の表「属性強化」行 = 兜/鎧/武/盾/頭/体/手/足/効果/AF。盾+・レリックは対象外)。
    pub fn allows_element(self) -> bool {
        !matches!(
            self,
            PartSlot::ShieldPlus | PartSlot::RelicPendant | PartSlot::RelicBracelet
        )
    }

    /// シエナのオーラの能力値スロットに「能力値一覧(武器/盾)」の 6 種
    /// (突き/斬り/魔攻/魔防/物理複合/魔法斬り、強化能力値扱い)が出る部位か
    /// (wiki: シエナのオーラ「能力値一覧(武器/盾)」)。`false` の部位は
    /// 「能力値一覧(その他の部位)」が出る ── 命中率/回避率は装備命中率・回避率補正、
    /// STAB〜AGI はステの最終固定値、残り 3 種(物理/魔法ダメージ耐性・被Cri減少)は
    /// 防御側の最終固定値(未モデル)に分かれる(`SienaValueKind::effect` を見よ)。
    pub fn siena_values_are_equipment(self) -> bool {
        matches!(self, PartSlot::Weapon | PartSlot::Shield)
    }

    /// 表示名(wiki: 装備システム ページ冒頭の表)。唯一の正 — UI 側はここを参照する。
    pub fn label(self) -> &'static str {
        match self {
            PartSlot::Weapon => "武器",
            PartSlot::Armor => "鎧",
            PartSlot::Helm => "兜",
            PartSlot::Shield => "盾",
            PartSlot::ShieldPlus => "盾+",
            PartSlot::Head => "頭",
            PartSlot::Body => "体",
            PartSlot::Hand => "手",
            PartSlot::Leg => "足",
            PartSlot::Effect => "効果",
            PartSlot::Artifact => "AF",
            PartSlot::RelicPendant => "レリック(ペンダント)",
            PartSlot::RelicBracelet => "レリック(ブレスレット)",
        }
    }
}

/// 部位ごとの枠数・可否ルール(ドラフト非依存の定数。UI がリテラルで持たず参照する。
/// `StatLimits::part_slot_rules` に載る。各フィールドは `PartSlot` の同名メソッドの写し)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartSlotRule {
    pub slot: PartSlot,
    pub label: String,
    /// この部位の装着アビリティの実スロット数
    pub ability_slots: usize,
    /// この部位が装着アビリティを持てるか
    pub allows_ability: bool,
    /// この部位が装備強化(+1〜+15)を持てるか
    pub allows_enhance: bool,
    /// この部位がシエナのオーラを発現できるか
    pub allows_siena: bool,
    /// シエナのオーラの能力値がこの部位では「装備補正」として付くか
    pub siena_counts_as_equipment: bool,
    /// この部位がランダムオプションを持てるか
    pub allows_random_option: bool,
    /// この部位に付けられるランダムオプションの数(持てない部位は `None`)
    pub random_option_slots: Option<usize>,
    /// この部位が属性強化を持てるか
    pub allows_element: bool,
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

    /// 7 ステの合計(表示用。部位ごと・全部位合計のどちらにも使う)。
    pub fn total(&self) -> i64 {
        StatKind::ALL.iter().map(|&k| self.get(k)).sum()
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
    /// 装備強化の追加効果(武器の固定ダメージ / 鎧の追加HP)の補正式。
    /// カタログ外装備でもユーザーが種別を選べば計算できる。
    #[serde(default)]
    pub enhance_type: Option<EquipmentEnhanceType>,
    /// +12 以上の追加効果等級。+11 以下は式で確定するため `None` 固定。
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
    /// (カテゴリー整合性は `Equipment::validate_against_catalog` がカタログを受けて検証する)
    #[serde(default)]
    pub random_options: Vec<RandomOptionSlot>,
    /// カタログ外装備のエンチャント上限(実測)。カタログ品が付いていれば
    /// `EquipmentCatalogEntry::enchant_caps` が正で、こちらは使わない
    /// (先例: `enhance_type` と同じ「カタログ外でもユーザーが入れれば計算できる」枠)。
    #[serde(default)]
    pub enchant_caps: Option<EquipmentValues>,
}

impl EquipmentPart {
    /// エンチャント上限を解決する: カタログ品が付いていればカタログの `enchant_caps` が正、
    /// 無ければパートの実測 `enchant_caps`、どちらも無ければ「未収録」(`None`)。
    /// この解決順をドメイン側の 1 箇所に持たせ、呼び出し側(commands.rs 等)で
    /// 分岐を書き直させない(ADR 001: 値域上限にフォールバックを持たない)。
    pub fn resolve_enchant_caps<C: EquipmentCatalogEntry>(&self, catalog: &[C]) -> Option<EquipmentValues> {
        match &self.item_id {
            Some(item_id) => catalog
                .iter()
                .find(|i| i.id() == item_id.as_str())
                .map(|i| i.enchant_caps()),
            None => self.enchant_caps,
        }
    }

    /// この部位の武器系統。カタログ品はカタログの武器種から、カタログ外は
    /// ユーザーが選んだ装備強化の補正式(`enhance_type`)から決まる。どちらも無ければ
    /// 系統不明(`None`)で、装着アビリティの系統絞り込みをしない。
    pub fn weapon_system<C: EquipmentCatalogEntry>(&self, catalog: &[C]) -> Option<WeaponSystem> {
        let entry = self
            .item_id
            .as_ref()
            .and_then(|id| catalog.iter().find(|i| i.id() == id.as_str()));
        if let Some(entry) = entry {
            if let Some(class) = entry.weapon_class() {
                return Some(class.system());
            }
            if let Some(system) = entry.enhance_type().and_then(WeaponSystem::from_enhance_type) {
                return Some(system);
            }
        }
        self.enhance_type.and_then(WeaponSystem::from_enhance_type)
    }

    /// ランダムオプションの部位制約と値域。カタログ整合性(未知 id・カテゴリー重複)は
    /// カタログを引数で受ける `Equipment::validate_against_catalog` で見る。
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
        if slot
            .random_option_slots()
            .is_some_and(|max| self.random_options.len() > max)
        {
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
        if let Some(caps) = self.enchant_caps {
            caps.validate()?;
        }
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
                return Err(EquipmentError::EnhanceTypeRequired {
                    slot,
                    enhance_level: self.enhance_level,
                });
            };
            let compatible = match slot {
                PartSlot::Weapon => matches!(
                    kind,
                    EquipmentEnhanceType::WeaponStab
                        | EquipmentEnhanceType::WeaponStabHack
                        | EquipmentEnhanceType::WeaponHack
                        | EquipmentEnhanceType::WeaponInt
                        | EquipmentEnhanceType::WeaponIntHack
                        | EquipmentEnhanceType::WeaponMr
                ),
                PartSlot::Armor => matches!(
                    kind,
                    EquipmentEnhanceType::ArmorLight
                        | EquipmentEnhanceType::ArmorHeavy
                        | EquipmentEnhanceType::ArmorMagic
                        | EquipmentEnhanceType::ArmorSuit
                        | EquipmentEnhanceType::ArmorRobe
                ),
                _ => false,
            };
            if !compatible {
                return Err(EquipmentError::EnhanceTypeNotAllowed { slot, kind });
            }
        }
        if self.enhance_grade.is_some() && self.enhance_level < ENHANCE_LEVEL_RANDOM_RANGE_MIN {
            return Err(EquipmentError::EnhanceAddedDamageNotAllowed {
                slot,
                enhance_level: self.enhance_level,
            });
        }
        if self.enhance_level >= ENHANCE_LEVEL_RANDOM_RANGE_MIN && self.enhance_grade.is_none() {
            return Err(EquipmentError::EnhanceGradeRequired {
                slot,
                enhance_level: self.enhance_level,
            });
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
    ValueOutOfRange {
        field: &'static str,
        value: i64,
        max: i64,
    },
    #[error("{slot:?} の装備強化 Lv は 0〜{max} です(指定値 {value})")]
    EnhanceLevelOutOfRange { slot: PartSlot, value: u8, max: u8 },
    #[error("{slot:?} は装備強化の対象外です(武器・鎧のみ)")]
    EnhanceNotAllowed { slot: PartSlot },
    #[error("{slot:?} の装備強化 Lv {enhance_level} では装備種別を選んでください")]
    EnhanceTypeRequired { slot: PartSlot, enhance_level: u8 },
    #[error("{slot:?} に装備強化種別 {kind:?} は指定できません")]
    EnhanceTypeNotAllowed {
        slot: PartSlot,
        kind: EquipmentEnhanceType,
    },
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

/// 装備強化の追加効果補正式。武器は固定ダメージ、鎧は追加HPを算出する。
/// カタログ品は自動設定し、カタログ外だけ選択する。
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
    fn default() -> Self {
        Self {
            registered: Vec::new(),
            selected_id: None,
        }
    }
}

impl From<EquipmentPart> for EquipmentPartList {
    fn from(mut part: EquipmentPart) -> Self {
        if part.id == 0 {
            part.id = 1;
        }
        let id = part.id;
        Self {
            registered: vec![part],
            selected_id: Some(id),
        }
    }
}

impl std::ops::Deref for EquipmentPartList {
    type Target = EquipmentPart;
    fn deref(&self) -> &Self::Target {
        self.selected()
            .expect("EquipmentPartList selected_id invariant")
    }
}
impl std::ops::DerefMut for EquipmentPartList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if self.selected().is_none() {
            let id = self.registered.iter().map(|p| p.id).max().unwrap_or(0) + 1;
            let mut part = EquipmentPart::default();
            part.id = id;
            self.registered.push(part);
            self.selected_id = Some(id);
        }
        self.selected_mut()
            .expect("EquipmentPartList selected_id invariant")
    }
}

impl EquipmentPartList {
    pub fn selected(&self) -> Option<&EquipmentPart> {
        self.selected_id
            .and_then(|id| self.registered.iter().find(|p| p.id == id))
    }
    pub fn selected_mut(&mut self) -> Option<&mut EquipmentPart> {
        let id = self.selected_id?;
        self.registered.iter_mut().find(|p| p.id == id)
    }
    pub fn validate(&self, slot: PartSlot) -> Result<(), EquipmentError> {
        if self.selected_id.is_some() && self.selected().is_none() {
            return Err(EquipmentError::UnknownSelectedId { slot });
        }
        let mut ids = std::collections::HashSet::new();
        for part in &self.registered {
            if part.id == 0 || !ids.insert(part.id) {
                return Err(EquipmentError::DuplicatePartId { slot, id: part.id });
            }
            part.validate(slot)?;
        }
        Ok(())
    }
}

impl EquipmentParts {
    /// 選択中の部位だけを列挙する。
    pub fn iter(&self) -> Vec<(PartSlot, &EquipmentPart)> {
        self.iter_lists()
            .into_iter()
            .filter_map(|(slot, parts)| parts.selected().map(|p| (slot, p)))
            .collect()
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

    /// 部位を引く(`get_mut` の不変版)。
    pub fn get(&self, slot: PartSlot) -> &EquipmentPartList {
        match slot {
            PartSlot::Weapon => &self.weapon,
            PartSlot::Armor => &self.armor,
            PartSlot::Helm => &self.helm,
            PartSlot::Shield => &self.shield,
            PartSlot::ShieldPlus => &self.shield_plus,
            PartSlot::Head => &self.head,
            PartSlot::Body => &self.body,
            PartSlot::Hand => &self.hand,
            PartSlot::Leg => &self.leg,
            PartSlot::Effect => &self.effect,
            PartSlot::Artifact => &self.artifact,
            PartSlot::RelicPendant => &self.relic_pendant,
            PartSlot::RelicBracelet => &self.relic_bracelet,
        }
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

/// カタログを引ける層(`gamedata` の `EquipmentItem` 相当)が渡す装備品 1 件のビュー。
/// domain は gamedata に依存できないので、`base_totals` が `&[EquipmentAbilityDef]` を
/// 受ける流儀と同じく、カタログの中身は呼び出し側(gamedata の `EquipmentItem`)がこのトレイトを
/// 実装して渡す。
pub trait EquipmentCatalogEntry {
    fn id(&self) -> &str;
    fn slot(&self) -> PartSlot;
    fn ability_slots(&self) -> usize;
    fn random_option_slots(&self) -> Option<usize>;
    fn values_min(&self) -> EquipmentValues;
    fn values_max(&self) -> EquipmentValues;
    fn growth_caps(&self) -> Option<EquipmentValues>;
    fn enchant_caps(&self) -> EquipmentValues;
    /// 武器なら武器種。装着アビリティの系統適合を見るのに使う
    fn weapon_class(&self) -> Option<WeaponClass>;
    /// 装備強化の補正式。武器は `weapon_class` から決まるので、それ以外(鎧など)だけ
    fn enhance_type(&self) -> Option<EquipmentEnhanceType>;
    /// レリックなら種別と段。育成順序(段上げの可否)を見るのに使う
    fn relic(&self) -> Option<RelicInfo>;
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

    /// 装備のカタログ整合性を検証する(未知の item_id/ability id・部位不一致・成長値/エンチャント枠超過・
    /// アビリティのカテゴリー重複・ランダムオプションのカテゴリー重複)。呼び出し側(保存前の `storage`、
    /// DB に書かないプレビュー系コマンド双方)がカタログを渡す(`base_totals` と同じ依存方向)。
    ///
    /// `custom`(`item_id` が `None`)のエンチャントは `Equipment::validate` の値域チェック(0〜共通上限)
    /// で既に検証済み。ここではカタログ item のときはカタログ固有の `enchant_caps` を、
    /// custom で実測上限(`EquipmentPart::enchant_caps`)が入っているときはそれを追加でチェックする。
    pub fn validate_against_catalog<C: EquipmentCatalogEntry>(
        &self,
        equipment_catalog: &[C],
        equipment_abilities: &[EquipmentAbilityDef],
        random_options: &[RandomOptionDef],
    ) -> Result<(), ValidationError> {
        for (slot, parts) in self.parts.iter_lists() {
            for part in &parts.registered {
                let here = || ValidationLocation::part(slot, part.id);
                let at_ability =
                    |ability_id: &str| ValidationLocation::ability(slot, part.id, ability_id);
                if let Some(item_id) = &part.item_id {
                    let item = equipment_catalog
                        .iter()
                        .find(|i| i.id() == item_id.as_str())
                        .ok_or_else(|| {
                            ValidationError::at(
                                format!("未知の装備アイテム '{item_id}' です"),
                                here(),
                            )
                        })?;
                    if item.slot() != slot {
                        return Err(ValidationError::at(
                            format!(
                                "装備アイテム '{item_id}' は {:?} 用ですが {:?} 部位に指定されています",
                                item.slot(),
                                slot
                            ),
                            here(),
                        ));
                    }
                    if part.abilities.len() > item.ability_slots() {
                        return Err(ValidationError::at(
                            format!(
                                "装備アイテム '{item_id}' のアビリティは {} 枠までです",
                                item.ability_slots()
                            ),
                            here(),
                        ));
                    }
                    if part.random_options.len() > item.random_option_slots().unwrap_or(0) {
                        return Err(ValidationError::at(
                            format!(
                                "装備アイテム '{item_id}' のランダムオプションは {} 枠までです",
                                item.random_option_slots().unwrap_or(0)
                            ),
                            here(),
                        ));
                    }
                    if let Some(caps) = item.growth_caps() {
                        if let Some((name, value, minimum)) = part
                            .base
                            .fields()
                            .into_iter()
                            .zip(item.values_min().fields())
                            .find_map(|((name, value), (_, minimum))| {
                                (value < minimum).then_some((name, value, minimum))
                            })
                        {
                            return Err(ValidationError::at(
                                format!(
                                    "装備アイテム '{item_id}' の{name}成長値 {value} が下限 {minimum} を下回っています"
                                ),
                                here(),
                            ));
                        }
                        if let Some((name, value, cap)) =
                            part.base.fields().into_iter().zip(caps.fields()).find_map(
                                |((name, value), (_, cap))| {
                                    (value > cap).then_some((name, value, cap))
                                },
                            )
                        {
                            return Err(ValidationError::at(
                                format!(
                                    "装備アイテム '{item_id}' の{name}成長値 {value} が上限 {cap} を超えています"
                                ),
                                here(),
                            ));
                        }
                    }
                    if let Some((name, enchant, cap)) = part
                        .enchant
                        .fields()
                        .into_iter()
                        .zip(item.enchant_caps().fields())
                        .find_map(|((name, enchant), (_, cap))| {
                            (enchant > cap).then_some((name, enchant, cap))
                        })
                    {
                        return Err(ValidationError::at(
                            format!(
                                "装備アイテム '{item_id}' の{name}エンチャント {enchant} が枠 {cap} を超えています"
                            ),
                            here(),
                        ));
                    }
                } else if let Some(caps) = part.enchant_caps {
                    // カタログ外(カスタム名)装備。実測の `enchant_caps` が入っていれば、
                    // カタログ品と同じ枠超過チェックをかける(カタログ品が付いているときの
                    // `enchant_caps` はここでは参照しない — カタログが正)。
                    if let Some((name, enchant, cap)) = part
                        .enchant
                        .fields()
                        .into_iter()
                        .zip(caps.fields())
                        .find_map(|((name, enchant), (_, cap))| {
                            (enchant > cap).then_some((name, enchant, cap))
                        })
                    {
                        return Err(ValidationError::at(
                            format!(
                                "カスタム装備の{name}エンチャント {enchant} が実測上限 {cap} を超えています"
                            ),
                            here(),
                        ));
                    }
                }
                // アビリティはカテゴリーごとに1つまで。同じ攻撃系統でもカテゴリー1と4は併用できる。
                // 武器は系統に合う効果系統しか装着できない(系統不明のカスタム武器は通す)。
                let weapon_system = (slot == PartSlot::Weapon)
                    .then(|| part.weapon_system(equipment_catalog))
                    .flatten();
                let mut groups = std::collections::HashSet::new();
                for ability_id in &part.abilities {
                    let def = equipment_abilities
                        .iter()
                        .find(|a| a.id == ability_id.as_str())
                        .ok_or_else(|| {
                            ValidationError::at(
                                format!("未知の装備アビリティ '{ability_id}' です"),
                                at_ability(ability_id),
                            )
                        })?;
                    if def.slot != slot {
                        return Err(ValidationError::at(
                            format!("装備アビリティ '{}' は {:?} 用です", def.name, def.slot),
                            at_ability(ability_id),
                        ));
                    }
                    if let Some(system) = weapon_system {
                        if !system.accepts_ability(def.family) {
                            return Err(ValidationError::at(
                                format!(
                                    "装備アビリティ '{}' はこの武器の系統({:?})には装着できません",
                                    def.name, system
                                ),
                                at_ability(ability_id),
                            ));
                        }
                    }
                    if !groups.insert(def.exclusive_group) {
                        return Err(ValidationError::at(
                            format!(
                                "装備アビリティ '{}' は同じ系統がすでに選ばれています(系統ごとに 1 つまで)",
                                def.name
                            ),
                            at_ability(ability_id),
                        ));
                    }
                }
                let mut value_abilities = std::collections::HashSet::new();
                for value in &part.ability_values {
                    if !value_abilities.insert(value.ability_id.as_str()) {
                        return Err(ValidationError::at(
                            format!(
                                "装備アビリティ本体値 '{}' が重複しています",
                                value.ability_id
                            ),
                            at_ability(&value.ability_id),
                        ));
                    }
                    if !part.abilities.iter().any(|id| id == &value.ability_id) {
                        return Err(ValidationError::at(
                            format!(
                                "装備アビリティ本体値の親 '{}' が選択されていません",
                                value.ability_id
                            ),
                            at_ability(&value.ability_id),
                        ));
                    }
                    let def = equipment_abilities
                        .iter()
                        .find(|a| a.id == value.ability_id)
                        .ok_or_else(|| {
                            ValidationError::at(
                                format!("未知の装備アビリティ本体値 '{}' です", value.ability_id),
                                at_ability(&value.ability_id),
                            )
                        })?;
                    let option = def.value_option.as_ref().ok_or_else(|| {
                        ValidationError::at(
                            format!("'{}' は本体の可変値を持ちません", def.name),
                            at_ability(&value.ability_id),
                        )
                    })?;
                    if value.kind != option.kind
                        || !(option.min..=option.max).contains(&value.value)
                    {
                        return Err(ValidationError::at(
                            format!(
                                "'{}' の本体値は {:?} {}〜{} です",
                                def.name, option.kind, option.min, option.max
                            ),
                            at_ability(&value.ability_id),
                        ));
                    }
                }
                for ability_id in &part.abilities {
                    if equipment_abilities
                        .iter()
                        .find(|a| a.id == ability_id.as_str())
                        .is_some_and(|def| def.value_option.is_some())
                        && !value_abilities.contains(ability_id.as_str())
                    {
                        return Err(ValidationError::at(
                            format!("装備アビリティ '{}' の本体値がありません", ability_id),
                            at_ability(ability_id),
                        ));
                    }
                }
                let mut addition_counts: std::collections::HashMap<&str, usize> =
                    std::collections::HashMap::new();
                for addition in &part.ability_additions {
                    if !part.abilities.iter().any(|id| id == &addition.ability_id) {
                        return Err(ValidationError::at(
                            format!(
                                "追加アビリティの親 '{}' が選択されていません",
                                addition.ability_id
                            ),
                            at_ability(&addition.ability_id),
                        ));
                    }
                    let def = equipment_abilities
                        .iter()
                        .find(|a| a.id == addition.ability_id)
                        .ok_or_else(|| {
                            ValidationError::at(
                                format!("未知の追加アビリティ親 '{}' です", addition.ability_id),
                                at_ability(&addition.ability_id),
                            )
                        })?;
                    let count = addition_counts.entry(def.id).or_default();
                    *count += 1;
                    if *count > usize::from(def.additional_slots) {
                        return Err(ValidationError::at(
                            format!(
                                "'{}' の追加アビリティは{}枠までです",
                                def.name, def.additional_slots
                            ),
                            at_ability(&addition.ability_id),
                        ));
                    }
                    let option = def
                        .additional_options
                        .iter()
                        .find(|o| o.kind == addition.kind)
                        .ok_or_else(|| {
                            ValidationError::at(
                                format!(
                                    "'{}' には {:?} の追加候補がありません",
                                    def.name, addition.kind
                                ),
                                at_ability(&addition.ability_id),
                            )
                        })?;
                    if !(option.min..=option.max).contains(&addition.value) {
                        return Err(ValidationError::at(
                            format!(
                                "'{}' の追加アビリティ値 {} は {}〜{} の範囲外です",
                                def.name, addition.value, option.min, option.max
                            ),
                            at_ability(&addition.ability_id),
                        ));
                    }
                }
                Self::validate_random_options_catalog(slot, part, random_options)?;
            }
        }
        Ok(())
    }

    /// ランダムオプションのカタログ整合性(未知 id・部位不一致・未収録ランク・カテゴリー重複)。
    ///
    /// wiki「ランダムオプション」転移の説明: 「同じカテゴリーのオプションを共存させることは出来ず、
    /// 転移させると優先的に上書きされる。ただし、カテゴリーなし(一覧表では 0 表記)はその限りではない」。
    /// 部位ごとの枠数は wiki に記載が無いので数では縛らない。
    fn validate_random_options_catalog(
        slot: PartSlot,
        part: &EquipmentPart,
        random_options: &[RandomOptionDef],
    ) -> Result<(), ValidationError> {
        let mut categories = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for option in &part.random_options {
            let at = || ValidationLocation::random_option(slot, part.id, &option.option_id);
            let def = random_options
                .iter()
                .find(|d| d.id == option.option_id.as_str())
                .ok_or_else(|| {
                    ValidationError::at(
                        format!("未知のランダムオプション '{}' です", option.option_id),
                        at(),
                    )
                })?;
            if def.slot != slot {
                return Err(ValidationError::at(
                    format!(
                        "ランダムオプション '{}' は {:?} 用ですが {:?} 部位に指定されています",
                        def.name, def.slot, slot
                    ),
                    at(),
                ));
            }
            if def.tier(option.rank).is_none() {
                return Err(ValidationError::at(
                    format!(
                        "ランダムオプション '{}' に {:?} ランクはありません",
                        def.name, option.rank
                    ),
                    at(),
                ));
            }
            if !ids.insert(def.id) {
                return Err(ValidationError::at(
                    format!(
                        "ランダムオプション '{}' が同じ部位に重複しています",
                        def.name
                    ),
                    at(),
                ));
            }
            // カテゴリー 0 は「カテゴリーなし」で共存できる
            if def.category != 0 && !categories.insert(def.category) {
                return Err(ValidationError::at(
                    format!(
                        "ランダムオプション '{}' はカテゴリー{} が同じ部位ですでに選ばれています(同じカテゴリーは 1 つまで)",
                        def.name, def.category
                    ),
                    at(),
                ));
            }
        }
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
        sum_equipment_value_sources(&self.base_sources(abilities, titles))
    }

    /// 基本能力値の供給源内訳(部位の実測値 → 部位アビリティ → 称号の順)。
    /// 全 0 の供給源は入れない。`base_totals` はこの Σ(計算を二重に書かない)。
    pub fn base_sources(
        &self,
        abilities: &[EquipmentAbilityDef],
        titles: &[TitleDef],
    ) -> Vec<EquipmentValueSource> {
        let mut sources = Vec::new();
        for (slot, part) in self.iter_selected() {
            if part.base != EquipmentValues::default() {
                sources.push(EquipmentValueSource {
                    source: format!("{}(基本値)", slot.label()),
                    values: part.base,
                });
            }
        }
        for (slot, part) in self.iter_selected() {
            let values = part_ability_values(slot, part, abilities);
            if values != EquipmentValues::default() {
                sources.push(EquipmentValueSource {
                    source: format!("{} アビリティ", slot.label()),
                    values,
                });
            }
        }
        let title_values = title_values(self.title.as_deref(), titles);
        if title_values != EquipmentValues::default() {
            let name = titles
                .iter()
                .find(|t| Some(t.id) == self.title.as_deref())
                .map_or("", |t| t.name);
            sources.push(EquipmentValueSource {
                source: format!("称号【{name}】"),
                values: title_values,
            });
        }
        sources
    }

    /// 部位別のアビリティ由来の装備補正(表示用の内訳。part.base・称号は含まない)。
    /// `base_totals` の二項目(装備アビリティの合計)を部位ごとに割ったもの。
    pub fn ability_values_by_part(
        &self,
        abilities: &[EquipmentAbilityDef],
    ) -> Vec<PartEquipmentValues> {
        self.iter_selected()
            .map(|(slot, part)| PartEquipmentValues {
                slot,
                values: part_ability_values(slot, part, abilities),
            })
            .collect()
    }

    /// 部位別のエンチャント値(表示用の内訳。`part.enchant` そのものを
    /// `ability_values_by_part` と同じ形で返す。part 1 つぶんの「装備値の合計」表示に使う)。
    pub fn enchant_values_by_part(&self) -> Vec<PartEquipmentValues> {
        self.iter_selected()
            .map(|(slot, part)| PartEquipmentValues {
                slot,
                values: part.enchant,
            })
            .collect()
    }

    /// アビリティの追加効果(wiki: アビリティ表の「追加効果」列)を
    /// 与ダメージ式のカテゴリ寄与に変換する。装備攻撃力への加算は `base_totals` が別に見る。
    pub fn ability_damage_contributions(
        &self,
        abilities: &[EquipmentAbilityDef],
    ) -> Vec<DamageContribution> {
        let effects: Vec<(String, &SkillEffect)> = self
            .iter_selected()
            .into_iter()
            .flat_map(|(slot, part)| {
                part.abilities.iter().filter_map(move |id| {
                    abilities
                        .iter()
                        .find(|a| a.id == id.as_str() && a.slot == slot)
                })
            })
            .flat_map(|def| {
                def.damage_effects
                    .iter()
                    .map(move |e| (def.name.to_string(), e))
            })
            .collect();
        let mut contributions = damage_contributions(effects.into_iter());
        for (slot, part) in self.iter_selected() {
            for addition in &part.ability_additions {
                match addition.kind {
                    EquipmentAbilityAdditionalKind::FixedDamage => {
                        contributions.push(DamageContribution {
                            source: format!("追加効果({})", slot.label()),
                            category: DamageCategory::BasicTriggerDamageFixed,
                            value: addition.value as f64,
                        });
                    }
                    EquipmentAbilityAdditionalKind::DamageRate => {
                        contributions.push(DamageContribution {
                            source: format!("追加効果({})", slot.label()),
                            category: DamageCategory::AttackDamageBasicTrigger,
                            value: addition.value as f64 / 100.0,
                        });
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
        sum_equipment_value_sources(&self.enhanced_sources(region))
    }

    /// 強化能力値の供給源内訳(部位のエンチャント → シエナのオーラ(武器/盾)→ テシスコア
    /// の順)。全 0 の供給源は入れない。`enhanced_totals` はこの Σ(計算を二重に書かない)。
    pub fn enhanced_sources(&self, region: Option<CoreRegion>) -> Vec<EquipmentValueSource> {
        let mut sources = Vec::new();
        for (slot, part) in self.iter_selected() {
            if part.enchant != EquipmentValues::default() {
                sources.push(EquipmentValueSource {
                    source: format!("{} エンチャント", slot.label()),
                    values: part.enchant,
                });
            }
        }
        // 武器/盾は強化能力値(突き〜魔法斬り)、その他の部位は命中率・回避率が
        // 装備命中率補正・装備回避率補正として同じ `EquipmentValues` に乗る
        // (`SienaValueKind::effect` が部位を問わず振り分ける。ここで部位を見て
        // 分岐する必要はない)。
        for (slot, aura) in self.siena.iter_selected() {
            let values = aura.values();
            if values != EquipmentValues::default() {
                sources.push(EquipmentValueSource {
                    source: format!("シエナのオーラ({})", slot.label()),
                    values,
                });
            }
        }
        if let Some(region) = region {
            let values = self.thesis_cores.equipment_values(Some(region));
            if values != EquipmentValues::default() {
                sources.push(EquipmentValueSource {
                    source: format!("テシスコア({})", region.label()),
                    values,
                });
            }
        }
        sources
    }

    /// 全部位のランダムオプションの集計。カタログは呼び出し側が渡す
    /// (`base_totals` の武器アビリティと同じ依存方向。domain は gamedata に依存できない)。
    /// カタログに無い id の枠は無視する(保存前に `Equipment::validate_against_catalog` が弾いている)。
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
        self.siena
            .iter_selected()
            .map(|(_, aura)| aura.attack_rate_percent())
            .sum::<f64>()
            / 100.0
    }

    /// シエナのオーラの追加オプション「防御力増加」の合計。Σ% の小数表現。
    /// 装備防御力倍率へ合流する(`CommonSkills::defense_rates` の引数)。
    pub fn siena_defense_rate(&self) -> f64 {
        self.siena
            .iter_selected()
            .map(|(_, aura)| aura.defense_rate_percent())
            .sum::<f64>()
            / 100.0
    }

    /// シエナのオーラの追加オプション「中ディレイ減少」の合計。Σ% の小数表現。
    /// 中ディレイ減少値(倍率B)へ合流する(wiki: ステータス「中ディレイ倍率B」)。
    pub fn siena_actual_delay_reduction(&self) -> f64 {
        self.siena
            .iter_selected()
            .map(|(_, aura)| aura.actual_delay_percent())
            .sum::<f64>()
            / 100.0
    }

    /// シエナのオーラの追加オプション「クリティカル確率」の合計。Σ% の小数表現。
    /// クリティカル率の AGI 由来の項に `× (1 + これ)` で効く(wiki: `#CriticalChance`)。
    ///
    /// wiki は「同一名称の効果同士で加算されるかどうかは要検証」としているが、
    /// 他の追加オプション(攻撃力増加・防御力増加・中ディレイ減少)と同じく部位ぶん加算する `[仮]`。
    pub fn siena_critical_rate(&self) -> f64 {
        self.siena
            .iter_selected()
            .map(|(_, aura)| aura.critical_rate_percent())
            .sum::<f64>()
            / 100.0
    }

    /// 装備に付与した属性値の合計(属性ごと)。
    pub fn element_values(&self, selected: Option<Element>) -> ElementValues {
        let mut total = ElementValues::default();
        if let Some(element) = selected.filter(|e| e.can_enchant_equipment()) {
            for (_, _) in self.iter_selected().filter(|(slot, part)| {
                slot.allows_element()
                    && (part.item_id.is_some()
                        || part.custom_name.as_deref().is_some_and(|n| !n.is_empty()))
            }) {
                *total.get_mut(element) += 9;
            }
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

    pub fn without_part(&self, slot: PartSlot) -> Equipment {
        self.without_selected_part(slot)
    }

    pub fn iter_selected(&self) -> impl Iterator<Item = (PartSlot, &EquipmentPart)> {
        self.parts
            .iter_lists()
            .into_iter()
            .filter_map(|(slot, parts)| parts.selected().map(|p| (slot, p)))
    }
}

/// 部位 1 つぶんの値(表示用。基本能力値の内訳やシエナのオーラ部位値など、部位キー付きで返す
/// プレビュー値に共通で使う)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PartEquipmentValues {
    pub slot: PartSlot,
    pub values: EquipmentValues,
}

/// 部位 1 つぶんの単純な合計値(表示用。シエナのオーラのステ加算合計など)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PartStatTotal {
    pub slot: PartSlot,
    pub value: i64,
}

/// 1 部位ぶんのアビリティ由来の装備補正(アビリティ定義値 + ロール値 + 追加効果)。
/// part.base・称号は含まない(`base_totals` / `ability_values_by_part` が共有する)。
fn part_ability_values(
    slot: PartSlot,
    part: &EquipmentPart,
    abilities: &[EquipmentAbilityDef],
) -> EquipmentValues {
    let mut total = EquipmentValues::default();
    for ability_id in &part.abilities {
        if let Some(def) = abilities
            .iter()
            .find(|a| a.id == *ability_id && a.slot == slot)
        {
            total = total.add(def.values);
        }
    }
    for value in &part.ability_values {
        add_ability_value(&mut total, value);
    }
    for addition in &part.ability_additions {
        add_ability_value(&mut total, addition);
    }
    total
}

fn add_ability_value(total: &mut EquipmentValues, value: &EquipmentAbilityAdditional) {
    match value.kind {
        EquipmentAbilityAdditionalKind::Thrust => total.thrust += i64::from(value.value),
        EquipmentAbilityAdditionalKind::Slash => total.slash += i64::from(value.value),
        EquipmentAbilityAdditionalKind::MagicAttack => total.magic_attack += i64::from(value.value),
        EquipmentAbilityAdditionalKind::MagicDefense => {
            total.magic_defense += i64::from(value.value)
        }
        EquipmentAbilityAdditionalKind::Accuracy => total.accuracy += i64::from(value.value),
        EquipmentAbilityAdditionalKind::PhysicalDefense => {
            total.physical_defense += i64::from(value.value)
        }
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
    equipment_value_terms(values, rates)
        .into_iter()
        .map(|(_, amount, coefficient)| amount as f64 * coefficient)
        .sum()
}

/// 装備値 4 種のうち、装備攻撃力に効く種別(wiki: カテゴリA の内訳 突き/斬り/魔攻/魔防)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentValueKind {
    Thrust,
    Slash,
    MagicAttack,
    MagicDefense,
}

/// 装備攻撃力への層(基本能力値 / 強化能力値。wiki: カテゴリA の内訳)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentAttackLayer {
    Base,
    Enhanced,
}

/// 装備攻撃力の内訳 1 行のうち、その値に効いた供給源 1 件(部位実測値・部位アビリティ・
/// 称号・手首補正・エンチャント・シエナのオーラ・テシスコア)。Σamount = part.amount、
/// Σcontribution = part.contribution。「なぜこの数字?」パネルをさらに掘り下げたときの
/// 「どの部位・供給源から来たか」一覧に使う。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentAttackSource {
    pub source: String,
    pub amount: i64,
    pub contribution: f64,
}

/// 装備攻撃力の内訳 1 行(層 × 装備値種別)。係数 0 の組は持たない。
/// 「なぜこの数字?」パネルの「装備攻撃力」の材料掘り下げに使う
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentAttackPart {
    pub layer: EquipmentAttackLayer,
    pub value: EquipmentValueKind,
    pub amount: i64,
    pub coefficient: f64,
    pub contribution: f64,
    /// この値(非 0 の source のみ)。
    pub sources: Vec<EquipmentAttackSource>,
}

/// `values`/`rates` の 4 種を(種別, 値, 係数)に並べる。`equipment_values_attack` と
/// `equipment_attack_parts` が同じ並びを共有する(計算を二重に書かない)。
fn equipment_value_terms(
    values: &EquipmentValues,
    rates: &EquipmentRates,
) -> [(EquipmentValueKind, i64, f64); 4] {
    [
        (EquipmentValueKind::Thrust, values.thrust, rates.thrust),
        (EquipmentValueKind::Slash, values.slash, rates.slash),
        (
            EquipmentValueKind::MagicAttack,
            values.magic_attack,
            rates.magic_attack,
        ),
        (
            EquipmentValueKind::MagicDefense,
            values.magic_defense,
            rates.magic_defense,
        ),
    ]
}

/// 装備値 4 種のうち 1 種別の値を取り出す(`EquipmentValueSource` の内訳を出すのに使う)。
fn equipment_value_kind_amount(values: &EquipmentValues, kind: EquipmentValueKind) -> i64 {
    match kind {
        EquipmentValueKind::Thrust => values.thrust,
        EquipmentValueKind::Slash => values.slash,
        EquipmentValueKind::MagicAttack => values.magic_attack,
        EquipmentValueKind::MagicDefense => values.magic_defense,
    }
}

/// 装備攻撃力の内訳(基本能力値層 + 強化能力値層)。Σcontribution = 装備攻撃力
/// (`equipment_values_attack(base, &c.base) + equipment_values_attack(enhanced, &c.enhanced)`)。
/// 各行はさらに供給源(`base_sources`/`enhanced_sources`)ごとの内訳を持つ。
/// 「なぜこの数字?」パネルの「装備攻撃力」の材料掘り下げに使う
pub fn equipment_attack_parts(
    base_sources: &[EquipmentValueSource],
    enhanced_sources: &[EquipmentValueSource],
    c: &EquipmentCoefficients,
) -> Vec<EquipmentAttackPart> {
    let base = sum_equipment_value_sources(base_sources);
    let enhanced = sum_equipment_value_sources(enhanced_sources);
    let layers = [
        (EquipmentAttackLayer::Base, &base, &c.base, base_sources),
        (
            EquipmentAttackLayer::Enhanced,
            &enhanced,
            &c.enhanced,
            enhanced_sources,
        ),
    ];
    let mut parts = Vec::new();
    for (layer, values, rates, sources) in layers {
        for (value, amount, coefficient) in equipment_value_terms(values, rates) {
            if coefficient == 0.0 {
                continue;
            }
            let source_rows = sources
                .iter()
                .filter_map(|s| {
                    let source_amount = equipment_value_kind_amount(&s.values, value);
                    (source_amount != 0).then(|| EquipmentAttackSource {
                        source: s.source.clone(),
                        amount: source_amount,
                        contribution: source_amount as f64 * coefficient,
                    })
                })
                .collect();
            parts.push(EquipmentAttackPart {
                layer,
                value,
                amount,
                coefficient,
                contribution: amount as f64 * coefficient,
                sources: source_rows,
            });
        }
    }
    parts
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
pub fn weapon_added_damage(
    weapon_base: &EquipmentValues,
    rates: &EnhanceRates,
    multiplier: f64,
) -> i64 {
    let correction = weapon_base.thrust as f64 * rates.thrust
        + weapon_base.slash as f64 * rates.slash
        + weapon_base.magic_attack as f64 * rates.magic_attack
        + weapon_base.magic_defense as f64 * rates.magic_defense;
    let inner = crate::rounding::trunc_int(correction) as f64;
    let added = crate::rounding::trunc_int(inner * multiplier);
    if added % 2 != 0 {
        added - 1
    } else {
        added
    }
}

/// 鎧系装備の装備強化による追加 HP。
///
/// `補正 = 物防×r.physical_defense + 魔防×r.magic_defense`
/// `追加効果 = INT(INT(補正) × 倍率)`(武器と異なり奇数切捨は適用しない)。
/// 係数(`physical_defense_rate`/`magic_defense_rate`)は鎧種別ごとに gamedata が持つので引数で受ける。
pub fn armor_added_hp(
    armor_base: &EquipmentValues,
    physical_defense_rate: f64,
    magic_defense_rate: f64,
    multiplier: f64,
) -> i64 {
    let correction = armor_base.physical_defense as f64 * physical_defense_rate
        + armor_base.magic_defense as f64 * magic_defense_rate;
    let inner = crate::rounding::trunc_int(correction) as f64;
    crate::rounding::trunc_int(inner * multiplier)
}

/// キャラ固有パッシブによる、腕装備の補正から「基本能力値」への派生ルール。
///
/// バンド系(`BandAgility*`)は「バンドの敏捷(基本+エンチャント)の 0.7 倍
/// (小数点以下切り捨て)」を対象キャラの主軸ステへ振る。振り先はキャラごとに
/// 固定/主軸スキル依存/ステ大小比較のいずれかで決まる(gamedata の
/// `GameCharacter::wrist_bonus` がキャラごとにどのルールかを持つ)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WristBonusRule {
    /// ボリス・マキシミン: バンド判定なし。腕(盾/ナックル)の突き(基本+エンチャント+
    /// シエナ盾のオーラ突き)を魔攻の基本補正に変換する。
    ThrustToMagicAttack,
    /// ナヤトレイ・イサック: バンドのみ。主軸スキル依存で突き(STAB/STAB+HACK)/
    /// 斬り(HACK)へ振る。それ以外の依存は変換しない。
    BandAgilityByDependency,
    /// ミラ: バンドのみ。常に斬りへ振る。
    BandAgilityToSlash,
    /// ベンヤ: バンドのみ。HACK と MR の大小比較で斬り/魔防へ振る(同値は変換しない)。
    BandAgilityByStatComparison,
    /// ロアミニ: バンドのみ。常に魔攻へ振る。
    BandAgilityToMagicAttack,
}

/// バンド敏捷の 0.7 倍(小数点以下切り捨て)。
fn band_agility_bonus(agility: i64) -> i64 {
    crate::rounding::trunc_int(agility as f64 * 0.7)
}

/// `WristBonusRule` を適用し、腕装備補正から派生する基本能力値を返す。
///
/// `is_band` は選択中の腕装備がバンド種別かどうか(装備カタログの `WristType` 解決は
/// gamedata が行う。domain はカタログを知らないため引数で受ける)。`wrist_totals` は
/// 選択中の腕装備の基本+エンチャント合計、`siena_thrust` はシエナ盾のオーラの突き
/// (`ThrustToMagicAttack` のみ使う)。元の腕装備の基本/エンチャント値そのものは
/// 変更しない(このルールは派生先へ「足す」値だけを返す)。
pub fn wrist_base_bonus(
    rule: Option<WristBonusRule>,
    is_band: bool,
    base_stats: &crate::stats::BaseStats,
    style_dependency: crate::skill::SkillDependency,
    wrist_totals: EquipmentValues,
    siena_thrust: i64,
) -> EquipmentValues {
    use crate::skill::SkillDependency;

    let Some(rule) = rule else {
        return EquipmentValues::default();
    };
    if rule == WristBonusRule::ThrustToMagicAttack {
        return EquipmentValues {
            magic_attack: wrist_totals.thrust + siena_thrust,
            ..Default::default()
        };
    }
    if !is_band {
        return EquipmentValues::default();
    }
    let bonus = band_agility_bonus(wrist_totals.agility);
    let mut values = EquipmentValues::default();
    match rule {
        WristBonusRule::BandAgilityByDependency => match style_dependency {
            SkillDependency::Stab | SkillDependency::StabHack => values.thrust = bonus,
            SkillDependency::Hack => values.slash = bonus,
            _ => {}
        },
        WristBonusRule::BandAgilityToSlash => values.slash = bonus,
        WristBonusRule::BandAgilityByStatComparison => {
            if base_stats.hack > base_stats.mr {
                values.slash = bonus;
            } else if base_stats.hack < base_stats.mr {
                values.magic_defense = bonus;
            }
        }
        WristBonusRule::BandAgilityToMagicAttack => values.magic_attack = bonus,
        WristBonusRule::ThrustToMagicAttack => unreachable!("above早期returnで処理済み"),
    }
    values
}

/// レリックの系列(wiki: Item/アクセサリ/レリック)。神鳥とルナリアは別系列で、
/// 段上げは同じ系列の中だけを進む。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelicKind {
    Godbird,
    Lunaria,
}

/// カタログ 1 件がどの系列の第何段か。id の文字列を解析させないため、カタログが属性で持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelicInfo {
    pub kind: RelicKind,
    pub level: u8,
}

/// レリックの育成状況。段上げは「いまの段の補正値が上限まで育ってから」というゲーム内の順序を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RelicState {
    pub kind: RelicKind,
    pub level: u8,
    /// 同じ系列でカタログに存在する最大段
    pub max_level: u8,
    /// いまの段の補正値が上限まで育っているか
    pub growth_done: bool,
    /// いまの段の上限までに残っている補正値の合計
    pub growth_remaining: i64,
    pub can_up: bool,
    pub can_down: bool,
}

/// 段の上げ下げ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelicDirection {
    Up,
    Down,
}

fn relic_entry<'a, C: EquipmentCatalogEntry>(
    part: &EquipmentPart,
    catalog: &'a [C],
) -> Option<(&'a C, RelicInfo)> {
    let item_id = part.item_id.as_ref()?;
    let entry = catalog.iter().find(|i| i.id() == item_id.as_str())?;
    let info = entry.relic()?;
    Some((entry, info))
}

/// いまの段で補正値を上げられる装備値(カタログの `growth_caps` がそのまま今の段の上限)。
fn relic_growth_kinds<C: EquipmentCatalogEntry>(entry: &C) -> Vec<(EquipmentStatKind, i64)> {
    let Some(caps) = entry.growth_caps() else {
        return Vec::new();
    };
    EquipmentStatKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let cap = caps.get(kind);
            (cap > 0).then_some((kind, cap))
        })
        .collect()
}

/// レリックの育成状況。レリック以外・未装備・カタログ外は `None`。
pub fn relic_state<C: EquipmentCatalogEntry>(
    part: &EquipmentPart,
    catalog: &[C],
) -> Option<RelicState> {
    let (entry, info) = relic_entry(part, catalog)?;
    let max_level = catalog
        .iter()
        .filter(|i| i.slot() == entry.slot() && i.relic().is_some_and(|r| r.kind == info.kind))
        .filter_map(|i| i.relic().map(|r| r.level))
        .max()
        .unwrap_or(info.level);
    let growth_remaining: i64 = relic_growth_kinds(entry)
        .into_iter()
        .map(|(kind, cap)| (cap - part.base.get(kind)).max(0))
        .sum();
    let growth_done = growth_remaining == 0;
    Some(RelicState {
        kind: info.kind,
        level: info.level,
        max_level,
        growth_done,
        growth_remaining,
        can_up: info.level < max_level && growth_done,
        can_down: info.level > 1,
        })
}

/// レリックの段を 1 つ動かした部位を返す。動かせないとき(系列の端・補正値が未完成・
/// 次の段がカタログに無い)は `None`。
///
/// 段を上げた直後の補正値は「まだ育っていない」= その段の下限(= 直前段階の完成値。wiki 注記)。
/// 段を下げる方向は「その段は育成済みだった」扱いのまま上限に置く。
pub fn relic_step<C: EquipmentCatalogEntry>(
    part: &EquipmentPart,
    catalog: &[C],
    direction: RelicDirection,
) -> Option<EquipmentPart> {
    let state = relic_state(part, catalog)?;
    let (entry, info) = relic_entry(part, catalog)?;
    let up = direction == RelicDirection::Up;
    if up && !state.can_up {
        return None;
    }
    if !up && !state.can_down {
        return None;
    }
    let next_level = if up { info.level + 1 } else { info.level - 1 };
    let next_entry = catalog.iter().find(|i| {
        i.slot() == entry.slot()
            && i.relic() == Some(RelicInfo {
                kind: info.kind,
                level: next_level,
            })
    })?;
    let mut next = part.clone();
    next.item_id = Some(next_entry.id().to_string());
    next.custom_name = None;
    next.base = if up {
        next_entry.values_min()
    } else {
        next_entry.values_max()
    };
    next.enchant = next.enchant.clamp_to(next_entry.enchant_caps());
    // カタログ品はカタログの enchant_caps が正(`resolve_enchant_caps`)。
    next.enchant_caps = None;
    next.enhance_type = next_entry.enhance_type();
    next.abilities.truncate(next_entry.ability_slots());
    next.ability_values
        .retain(|value| next.abilities.contains(&value.ability_id));
    next.ability_additions
        .retain(|addition| next.abilities.contains(&addition.ability_id));
    next.random_options
        .truncate(next_entry.random_option_slots().unwrap_or(0));
    Some(next)
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
            base: EquipmentRates {
                thrust: 14.5,
                slash: 14.5,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
            enhanced: EquipmentRates {
                thrust: 28.75,
                slash: 28.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
        }
    }

    fn equipment_with(weapon_base: EquipmentValues, weapon_enchant: EquipmentValues) -> Equipment {
        Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart {
                    base: weapon_base,
                    enchant: weapon_enchant,
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn 装備攻撃力は基本と強化の合計() {
        let eq = equipment_with(
            EquipmentValues {
                thrust: 150,
                slash: 150,
                ..Default::default()
            },
            EquipmentValues {
                thrust: 60,
                slash: 60,
                ..Default::default()
            },
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
        assert_eq!(
            equipment_attack_power(&base, &enhanced, &coefficients()),
            0.0
        );
    }

    #[test]
    fn base_totalsはアビリティ込み_enchantedはenchant側() {
        let mut eq = equipment_with(
            EquipmentValues {
                thrust: 100,
                slash: 200,
                ..Default::default()
            },
            EquipmentValues {
                thrust: 10,
                slash: 20,
                ..Default::default()
            },
        );
        eq.parts.weapon.abilities = vec!["sharp-blade-e".to_string()];
        eq.parts.armor.base = EquipmentValues {
            magic_defense: 50,
            ..Default::default()
        };

        let abilities = vec![EquipmentAbilityDef {
            id: "sharp-blade-e",
            name: "E-鋭い刃",
            family: EquipmentAbilityFamily::SharpBlade,
            category: 4,
            slot: PartSlot::Weapon,
            value_option: None,
            exclusive_group: "weapon-category-4",
            additional_slots: 2,
            additional_effects: "",
            additional_options: vec![],
            record_only: false,
            effect_summary: "斬り +9",
            values: EquipmentValues {
                slash: 9,
                ..Default::default()
            },
            damage_effects: &[],
        }];
        let base = eq.base_totals(&abilities, &[]);
        assert_eq!(
            base,
            EquipmentValues {
                thrust: 100,
                slash: 209,
                magic_defense: 50,
                ..Default::default()
            }
        );

        let enhanced = eq.enhanced_totals(None);
        assert_eq!(
            enhanced,
            EquipmentValues {
                thrust: 10,
                slash: 20,
                ..Default::default()
            }
        );
    }

    #[test]
    fn 部位別エンチャント内訳の合計は強化能力値の合計と一致する() {
        let eq = equipment_with(
            EquipmentValues {
                thrust: 100,
                slash: 200,
                ..Default::default()
            },
            EquipmentValues {
                thrust: 10,
                slash: 20,
                ..Default::default()
            },
        );
        let by_part = eq.enchant_values_by_part();
        let summed = by_part
            .iter()
            .fold(EquipmentValues::default(), |acc, p| acc.add(p.values));
        assert_eq!(summed, eq.enhanced_totals(None));
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
            EquipmentAbilityAdditional {
                ability_id: "night-star-sharp-blade".into(),
                kind: Slash,
                value: 18,
            },
            EquipmentAbilityAdditional {
                ability_id: "night-star-sharp-blade".into(),
                kind: Accuracy,
                value: 16,
            },
        ];
        let base = eq.base_totals(&[], &[]);
        assert_eq!(base.slash, 18);
        assert_eq!(base.accuracy, 16);

        eq.parts.weapon.ability_additions = vec![
            EquipmentAbilityAdditional {
                ability_id: "night-star-sharp-blade".into(),
                kind: FixedDamage,
                value: 10_000,
            },
            EquipmentAbilityAdditional {
                ability_id: "night-star-sharp-blade".into(),
                kind: DamageRate,
                value: 11,
            },
        ];
        let contributions = eq.ability_damage_contributions(&[]);
        assert!(contributions.iter().any(
            |c| c.category == DamageCategory::BasicTriggerDamageFixed && c.value == 10_000.0
        ));
        assert!(contributions
            .iter()
            .any(|c| c.category == DamageCategory::AttackDamageBasicTrigger && c.value == 0.11));
    }

    #[test]
    fn 武器アビリティは3枠まで() {
        let mut eq = Equipment::default();
        eq.parts.weapon.abilities = vec!["a".into(), "b".into(), "c".into()];
        assert!(eq.validate().is_ok());
        eq.parts.weapon.abilities.push("d".into());
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::TooManyAbilities {
                slot: PartSlot::Weapon,
                max: 3
            })
        ));
    }

    #[test]
    fn 値域違反は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX + 1;
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::ValueOutOfRange { .. })
        ));

        let mut eq = Equipment::default();
        eq.parts.weapon.enchant.magic_defense = -1;
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::ValueOutOfRange { .. })
        ));

        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX;
        assert!(eq.validate().is_ok());
    }

    #[test]
    fn 装備値の値域は9種すべてを検証する() {
        // wiki Item ページの列順そのまま。1 種でも欠けると検証をすり抜ける
        let names: Vec<&str> = EquipmentValues::default()
            .fields()
            .iter()
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(
            names,
            vec![
                "突き攻撃力",
                "斬り攻撃力",
                "物理防御力",
                "魔法攻撃力",
                "魔法防御力",
                "命中率補正",
                "クリティカル補正",
                "回避率補正",
                "敏捷度補正",
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
            assert!(matches!(
                eq.validate(),
                Err(EquipmentError::ValueOutOfRange { .. })
            ));
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
        assert_eq!(
            eq.element_values(Some(Element::Neutral)),
            ElementValues::default()
        );
        assert_eq!(
            eq.element_values(Some(Element::Fire)),
            ElementValues::default()
        );
    }

    #[test]
    fn 武器以外の強化レベルは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.helm.enhance_level = 1;
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::EnhanceNotAllowed {
                slot: PartSlot::Helm
            })
        ));

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
        assert!(matches!(
            over.validate(),
            Err(EquipmentError::EnhanceLevelOutOfRange { .. })
        ));
    }

    #[test]
    fn 強化等級は12以上だけ許可する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 11;
        eq.parts.weapon.enhance_type = Some(EquipmentEnhanceType::WeaponHack);
        eq.parts.weapon.enhance_grade = Some(EnhanceGrade::Highest);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::EnhanceAddedDamageNotAllowed {
                slot: PartSlot::Weapon,
                ..
            })
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
            Err(EquipmentError::EnhanceGradeRequired {
                slot: PartSlot::Weapon,
                enhance_level: 12
            })
        ));
    }

    #[test]
    fn 強化する装備は部位に合う種別が必須() {
        let mut missing = Equipment::default();
        missing.parts.weapon.enhance_level = 10;
        assert!(matches!(
            missing.validate(),
            Err(EquipmentError::EnhanceTypeRequired { .. })
        ));

        let mut mismatch = Equipment::default();
        mismatch.parts.weapon.enhance_level = 10;
        mismatch.parts.weapon.enhance_type = Some(EquipmentEnhanceType::ArmorMagic);
        assert!(matches!(
            mismatch.validate(),
            Err(EquipmentError::EnhanceTypeNotAllowed { .. })
        ));
    }

    #[test]
    fn 対象外部位のアビリティは拒否し兜は許可する() {
        let mut eq = Equipment::default();
        eq.parts.body.abilities = vec!["unknown".to_string()];
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::AbilitiesNotAllowed {
                slot: PartSlot::Body
            })
        ));

        let mut ok = Equipment::default();
        ok.parts.helm.abilities = vec!["helm-e-skill-attack".to_string()];
        assert!(ok.validate().is_ok());
    }

    // wiki 例(goal 文書): HACK系(斬り×6.67 + 突き×1.00)・突100/斬300
    // → INT(300×6.67+100×1.00) = INT(2001+100) = INT(2101) = 2101
    // +10 倍率 28.8 → INT(2101×28.8) = INT(60508.8) = 60508(偶数なのでそのまま)
    #[test]
    fn 武器追加固定ダメージ_hack系の式() {
        let rates = EnhanceRates {
            thrust: 1.00,
            slash: 6.67,
            magic_attack: 0.0,
            magic_defense: 0.0,
        };
        let weapon = EquipmentValues {
            thrust: 100,
            slash: 300,
            ..Default::default()
        };
        assert_eq!(weapon_added_damage(&weapon, &rates, 28.8), 60508);
    }

    #[test]
    fn 武器追加固定ダメージ_奇数なら1引く() {
        // 補正 = 101(突き×1.0)、倍率 1.0(+2 相当) → INT(101×1.0) = 101(奇数) → 100
        let rates = EnhanceRates {
            thrust: 1.0,
            slash: 0.0,
            magic_attack: 0.0,
            magic_defense: 0.0,
        };
        let weapon = EquipmentValues {
            thrust: 101,
            ..Default::default()
        };
        assert_eq!(weapon_added_damage(&weapon, &rates, 1.0), 100);
    }

    // wiki 例: 魔鎧+15最上(物防係数3.8・魔防係数4.0)・物防650/魔防510
    // → INT(650×3.8 + 510×4.0) × 440 = INT(4,510) × 440 = 1,984,400(武器と異なり奇数切捨は無い)
    #[test]
    fn 鎧追加hp_魔鎧15最上の式() {
        let armor = EquipmentValues {
            physical_defense: 650,
            magic_defense: 510,
            ..Default::default()
        };
        assert_eq!(armor_added_hp(&armor, 3.8, 4.0, 440.0), 1_984_400);
    }

    #[test]
    fn 登録idの0と重複は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.registered.push(EquipmentPart::default());
        eq.parts.weapon.selected_id = Some(0);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::DuplicatePartId { .. })
        ));
    }

    fn siena_values(thrust: i64, slash: i64) -> SienaAura {
        let mut slots = Vec::new();
        if thrust > 0 {
            slots.push(SienaSlot {
                kind: SienaValueKind::Thrust,
                value: thrust,
            });
        }
        if slash > 0 {
            slots.push(SienaSlot {
                kind: SienaValueKind::Slash,
                value: slash,
            });
        }
        SienaAura {
            slots,
            extras: Vec::new(),
        }
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
        list.registered = vec![RegisteredSienaAura {
            id: 1,
            label: String::new(),
            aura,
        }];
        list.selected_id = Some(1);
    }

    #[test]
    fn シエナのオーラの能力値は武器と盾だけ強化能力値に入る() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enchant = EquipmentValues {
            thrust: 10,
            ..Default::default()
        };
        set_siena(&mut eq, PartSlot::Weapon, siena_values(6, 4));
        set_siena(&mut eq, PartSlot::Shield, siena_values(0, 5));
        // 武器・盾以外はステ加算(装備補正には入らない)
        set_siena(
            &mut eq,
            PartSlot::Helm,
            SienaAura {
                slots: vec![
                    SienaSlot {
                        kind: SienaValueKind::Stab,
                        value: 10,
                    },
                    SienaSlot {
                        kind: SienaValueKind::Stab,
                        value: 10,
                    },
                    SienaSlot {
                        kind: SienaValueKind::Hack,
                        value: 10,
                    },
                ],
                extras: Vec::new(),
            },
        );
        assert!(eq.validate().is_ok());

        assert_eq!(
            eq.enhanced_totals(None),
            EquipmentValues {
                thrust: 16,
                slash: 9,
                ..Default::default()
            }
        );
        assert_eq!(eq.base_totals(&[], &[]), EquipmentValues::default());
        assert_eq!(
            eq.siena_stat_bonus(),
            SienaStatBonus {
                stab: 20,
                hack: 10,
                ..Default::default()
            }
        );
    }

    #[test]
    fn シエナのオーラの命中率と回避率は装備命中回避補正に入る() {
        // wiki: 能力値一覧(その他の部位)「命中率」「回避率」の注記どおり、
        // 武器/盾以外の部位に出た命中率・回避率は装備命中率補正・装備回避率補正へ
        // 数値ぶんの固定値として加算される(取得日 2026-09-01)。
        let mut eq = Equipment::default();
        set_siena(
            &mut eq,
            PartSlot::Helm,
            SienaAura {
                slots: vec![
                    SienaSlot {
                        kind: SienaValueKind::Accuracy,
                        value: 6,
                    },
                    SienaSlot {
                        kind: SienaValueKind::Evasion,
                        value: 3,
                    },
                ],
                extras: Vec::new(),
            },
        );
        assert!(eq.validate().is_ok());
        let totals = eq.enhanced_totals(None);
        assert_eq!(totals.accuracy, 6);
        assert_eq!(totals.evasion, 3);
        // 強化能力値(突き〜魔法斬り)には乗らない
        assert_eq!(
            EquipmentValues {
                accuracy: 0,
                evasion: 0,
                ..totals
            },
            EquipmentValues::default()
        );
    }

    #[test]
    fn シエナのオーラの攻撃力増加は全部位の合計() {
        let mut eq = Equipment::default();
        for (slot, rate) in [
            (PartSlot::Weapon, 10.0),
            (PartSlot::Armor, 3.0),
            (PartSlot::Leg, 2.0),
        ] {
            let kind = if slot.siena_values_are_equipment() {
                SienaValueKind::Thrust
            } else {
                SienaValueKind::Stab
            };
            let mut aura = siena_stage(3, kind);
            aura.extras.push(SienaExtraSlot {
                kind: SienaExtraKind::AttackRate,
                value: rate,
            });
            set_siena(&mut eq, slot, aura);
        }
        assert!(eq.validate().is_ok());
        assert!((eq.siena_attack_rate() - 0.15).abs() < 1e-12);
        assert_eq!(Equipment::default().siena_attack_rate(), 0.0);
    }

    #[test]
    fn シエナのオーラは登録一覧の装着中だけ反映する() {
        let mut low = siena_stage(3, SienaValueKind::Thrust);
        low.extras.push(SienaExtraSlot {
            kind: SienaExtraKind::AttackRate,
            value: 3.0,
        });
        let mut high = siena_stage(3, SienaValueKind::Thrust);
        high.extras.push(SienaExtraSlot {
            kind: SienaExtraKind::AttackRate,
            value: 9.0,
        });
        let mut eq = Equipment::default();
        eq.siena.weapon = SienaAuraList {
            registered: vec![
                RegisteredSienaAura {
                    id: 1,
                    label: "普段用".into(),
                    aura: low,
                },
                RegisteredSienaAura {
                    id: 2,
                    label: "火力用".into(),
                    aura: high,
                },
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
        weapon.extras.push(SienaExtraSlot {
            kind: SienaExtraKind::AllStats,
            value: 30.0,
        });
        set_siena(&mut eq, PartSlot::Weapon, weapon);
        set_siena(
            &mut eq,
            PartSlot::Helm,
            SienaAura {
                slots: vec![
                    SienaSlot {
                        kind: SienaValueKind::Stab,
                        value: 10,
                    },
                    SienaSlot {
                        kind: SienaValueKind::Stab,
                        value: 2,
                    },
                    SienaSlot {
                        kind: SienaValueKind::Def,
                        value: 1,
                    },
                ],
                extras: vec![SienaExtraSlot {
                    kind: SienaExtraKind::AllStats,
                    value: 21.0,
                }],
            },
        );
        assert!(eq.validate().is_ok());

        // 武器 30 + 兜 21 が全ステに乗り、STAB だけ能力値スロットの 12 が上乗せされる
        let total = eq.siena_stat_bonus();
        assert_eq!(total.stab, 30 + 21 + 12);
        assert_eq!(total.def, 51 + 1);
        for kind in [
            StatKind::Hack,
            StatKind::Int,
            StatKind::Mr,
            StatKind::Dex,
            StatKind::Agi,
        ] {
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
            Err(EquipmentError::Siena(SienaError::KindNotAllowed {
                slot: PartSlot::Helm,
                ..
            }))
        ));

        // 武器・盾はステ加算を持てない
        let mut eq = Equipment::default();
        set_siena(
            &mut eq,
            PartSlot::Weapon,
            siena_stage(1, SienaValueKind::Stab),
        );
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::KindNotAllowed {
                slot: PartSlot::Weapon,
                ..
            }))
        ));

        // 段階(= スロット数)の上限
        let mut eq = Equipment::default();
        set_siena(
            &mut eq,
            PartSlot::Armor,
            siena_stage(SIENA_STAGE_MAX + 1, SienaValueKind::Stab),
        );
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::Siena(SienaError::TooManySlots { .. }))
        ));
    }

    #[test]
    fn テシスコアは対象地域のときだけ強化能力値に入る() {
        use crate::thesis_core::{CoreSet, CoreType, ThesisCore, CORE_SLOT_COUNT};

        let mut eq = Equipment::default();
        eq.parts.weapon.enchant = EquipmentValues {
            slash: 100,
            ..Default::default()
        };
        *eq.thesis_cores.get_mut(CoreRegion::Abyss) = CoreSet {
            slots: [Some(ThesisCore {
                core_type: CoreType::Slash,
                evolution: 4,
                enhancement: 4,
            }); CORE_SLOT_COUNT],
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

        const TIERS: &[RandomOptionTier] = &[RandomOptionTier {
            rank: RandomOptionRank::Special,
            min: 10.0,
            max: 25.0,
        }];
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
        RandomOptionSlot {
            option_id: id.to_string(),
            rank: RandomOptionRank::Special,
            value: None,
        }
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
        assert_eq!(
            totals.dependency_damage_rate.get(SkillDependency::StabHack),
            0.25
        );
        assert_eq!(totals.accuracy_point, 25);
    }

    #[test]
    fn カタログに無いランダムオプションidは集計されない() {
        let mut eq = Equipment::default();
        eq.parts.shield.random_options = vec![ro("nope")];
        assert_eq!(
            eq.random_option_totals(&ro_defs()),
            RandomOptionTotals::default()
        );
    }

    // wiki: 装備システム冒頭の表「転移」行に 効果・AF は無い
    #[test]
    fn 効果とafはランダムオプションを持てない() {
        for slot in [PartSlot::Effect, PartSlot::Artifact] {
            let mut eq = Equipment::default();
            eq.parts.get_mut(slot).random_options = vec![ro("shield-dep")];
            assert!(matches!(
                eq.validate(),
                Err(EquipmentError::RandomOption(
                    RandomOptionError::NotAllowed { .. }
                ))
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
            Err(EquipmentError::RandomOption(
                RandomOptionError::ValueOutOfRange { .. }
            ))
        ));
    }

    // --- 称号 -----------------------------------------------------------

    fn title_defs() -> Vec<crate::title::TitleDef> {
        use crate::title::{TitleDef, TitleKind};
        vec![TitleDef {
            common: false,
            id: "eclipse",
            name: "エクリプス",
            kind: TitleKind::Special,
            group: "喪失の島",
            level: None,
            values: EquipmentValues {
                thrust: 40,
                slash: 40,
                ..Default::default()
            },
            attack_damage_percent: 0.0,
            conditional_added_damage: None,
            note: "",
        }]
    }

    // wiki: 称号システム。表示中の 1 件だけが基本能力値に乗る
    #[test]
    fn 称号は基本能力値に合流する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.base = EquipmentValues {
            thrust: 100,
            ..Default::default()
        };
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
        assert_eq!(
            eq.base_totals(&[], &title_defs()),
            EquipmentValues::default()
        );
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
            Err(EquipmentError::RandomOption(
                RandomOptionError::TooMany { .. }
            ))
        ));
        // 武器だけ 3 枠
        assert!(part.validate(PartSlot::Weapon).is_ok());
        part.random_options.push(op("d"));
        assert!(matches!(
            part.validate(PartSlot::Weapon),
            Err(EquipmentError::RandomOption(
                RandomOptionError::TooMany { .. }
            ))
        ));
    }

    #[test]
    fn バンド敏捷の0点7倍は整数演算の切り捨て割り算と同値() {
        // 旧実装は `agility * 7 / 10`(i64 の 0 方向切り捨て)。band_agility_bonus は
        // 浮動小数×0.7 を trunc_int で丸めるので、実在しうる範囲で結果が一致することを担保する。
        for agility in -50..=2000i64 {
            assert_eq!(
                band_agility_bonus(agility),
                agility * 7 / 10,
                "agility={agility}"
            );
        }
    }

    #[test]
    fn wrist_base_bonusはルールなしなら常に加算しない() {
        use crate::skill::SkillDependency;
        let stats = crate::stats::BaseStats::default();
        let totals = EquipmentValues {
            thrust: 100,
            agility: 100,
            ..Default::default()
        };
        assert_eq!(
            wrist_base_bonus(None, true, &stats, SkillDependency::Stab, totals, 0),
            EquipmentValues::default()
        );
    }

    #[test]
    fn thrust_to_magic_attackはバンド判定なしで突きを魔攻へ変換する() {
        use crate::skill::SkillDependency;
        let stats = crate::stats::BaseStats::default();
        let totals = EquipmentValues {
            thrust: 100,
            ..Default::default()
        };
        let bonus = wrist_base_bonus(
            Some(WristBonusRule::ThrustToMagicAttack),
            false,
            &stats,
            SkillDependency::HackInt,
            totals,
            35,
        );
        assert_eq!(bonus.magic_attack, 135);
        assert_eq!(bonus.thrust, 0, "元の突き値を移動せず派生値だけ返す");
    }

    #[test]
    fn band系ルールはバンド以外なら変換しない() {
        use crate::skill::SkillDependency;
        let stats = crate::stats::BaseStats {
            hack: 200,
            mr: 100,
            ..Default::default()
        };
        let totals = EquipmentValues {
            agility: 100,
            ..Default::default()
        };
        for rule in [
            WristBonusRule::BandAgilityByDependency,
            WristBonusRule::BandAgilityToSlash,
            WristBonusRule::BandAgilityByStatComparison,
            WristBonusRule::BandAgilityToMagicAttack,
        ] {
            assert_eq!(
                wrist_base_bonus(Some(rule), false, &stats, SkillDependency::Hack, totals, 0),
                EquipmentValues::default(),
                "{rule:?}"
            );
        }
    }

    #[test]
    fn band_agility_by_stat_comparisonは同値なら変換しない() {
        use crate::skill::SkillDependency;
        let stats = crate::stats::BaseStats {
            hack: 100,
            mr: 100,
            ..Default::default()
        };
        let totals = EquipmentValues {
            agility: 100,
            ..Default::default()
        };
        assert_eq!(
            wrist_base_bonus(
                Some(WristBonusRule::BandAgilityByStatComparison),
                true,
                &stats,
                SkillDependency::Hack,
                totals,
                0
            ),
            EquipmentValues::default()
        );
    }

    // --- レリックの段の遷移 ------------------------------------------------------------

    struct MockRelic {
        id: &'static str,
        level: u8,
        values_min: EquipmentValues,
        values_max: EquipmentValues,
    }
    impl EquipmentCatalogEntry for MockRelic {
        fn id(&self) -> &str {
            self.id
        }
        fn slot(&self) -> PartSlot {
            PartSlot::RelicPendant
        }
        fn ability_slots(&self) -> usize {
            0
        }
        fn random_option_slots(&self) -> Option<usize> {
            None
        }
        fn values_min(&self) -> EquipmentValues {
            self.values_min
        }
        fn values_max(&self) -> EquipmentValues {
            self.values_max
        }
        fn growth_caps(&self) -> Option<EquipmentValues> {
            Some(self.values_max)
        }
        fn enchant_caps(&self) -> EquipmentValues {
            EquipmentValues::default()
        }
        fn weapon_class(&self) -> Option<WeaponClass> {
            None
        }
        fn enhance_type(&self) -> Option<EquipmentEnhanceType> {
            None
        }
        fn relic(&self) -> Option<RelicInfo> {
            Some(RelicInfo {
                kind: RelicKind::Godbird,
                level: self.level,
            })
        }
    }

    fn relic_catalog() -> Vec<MockRelic> {
        let values = |thrust: i64| EquipmentValues {
            thrust,
            ..Default::default()
        };
        vec![
            MockRelic {
                id: "relic-1",
                level: 1,
                values_min: values(0),
                values_max: values(30),
            },
            MockRelic {
                id: "relic-2",
                level: 2,
                values_min: values(30),
                values_max: values(50),
            },
        ]
    }

    fn relic_part(item_id: &str, thrust: i64) -> EquipmentPart {
        EquipmentPart {
            item_id: Some(item_id.to_string()),
            base: EquipmentValues {
                thrust,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn 補正値が上限に届くまで段を上げられない() {
        let catalog = relic_catalog();
        let part = relic_part("relic-1", 20);
        let state = relic_state(&part, &catalog).unwrap();
        assert_eq!(state.level, 1);
        assert_eq!(state.max_level, 2);
        assert_eq!(state.growth_remaining, 10);
        assert!(!state.growth_done);
        assert!(!state.can_up);
        assert!(!state.can_down);
        assert!(relic_step(&part, &catalog, RelicDirection::Up).is_none());
    }

    #[test]
    fn 段を上げた直後の補正値はその段の下限に戻る() {
        let catalog = relic_catalog();
        let part = relic_part("relic-1", 30);
        let state = relic_state(&part, &catalog).unwrap();
        assert!(state.growth_done && state.can_up);
        let next = relic_step(&part, &catalog, RelicDirection::Up).unwrap();
        assert_eq!(next.item_id.as_deref(), Some("relic-2"));
        assert_eq!(next.base.thrust, 30);
    }

    #[test]
    fn 段を下げると育成済みの上限に戻る() {
        let catalog = relic_catalog();
        let part = relic_part("relic-2", 40);
        let next = relic_step(&part, &catalog, RelicDirection::Down).unwrap();
        assert_eq!(next.item_id.as_deref(), Some("relic-1"));
        assert_eq!(next.base.thrust, 30);
    }

    // --- resolve_enchant_caps: カタログ → パート実測 → 未収録(None)の解決順 --------------

    struct MockCatalogEntry {
        id: &'static str,
        enchant_caps: EquipmentValues,
    }
    impl EquipmentCatalogEntry for MockCatalogEntry {
        fn id(&self) -> &str {
            self.id
        }
        fn slot(&self) -> PartSlot {
            PartSlot::Weapon
        }
        fn ability_slots(&self) -> usize {
            0
        }
        fn random_option_slots(&self) -> Option<usize> {
            None
        }
        fn values_min(&self) -> EquipmentValues {
            EquipmentValues::default()
        }
        fn values_max(&self) -> EquipmentValues {
            EquipmentValues::default()
        }
        fn growth_caps(&self) -> Option<EquipmentValues> {
            None
        }
        fn enchant_caps(&self) -> EquipmentValues {
            self.enchant_caps
        }
        fn weapon_class(&self) -> Option<WeaponClass> {
            None
        }
        fn enhance_type(&self) -> Option<EquipmentEnhanceType> {
            None
        }
        fn relic(&self) -> Option<RelicInfo> {
            None
        }
    }

    #[test]
    fn resolve_enchant_capsはカタログ品ならカタログの上限を正とする() {
        let catalog = [MockCatalogEntry {
            id: "sword-1",
            enchant_caps: EquipmentValues {
                slash: 300,
                ..Default::default()
            },
        }];
        let part = EquipmentPart {
            item_id: Some("sword-1".to_string()),
            // カタログ品が付いているので、これは無視される。
            enchant_caps: Some(EquipmentValues {
                slash: 999,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            part.resolve_enchant_caps(&catalog),
            Some(EquipmentValues {
                slash: 300,
                ..Default::default()
            })
        );
    }

    #[test]
    fn resolve_enchant_capsはカタログ外ならパートの実測上限を使う() {
        let catalog: [MockCatalogEntry; 0] = [];
        let part = EquipmentPart {
            item_id: None,
            custom_name: Some("自作の剣".to_string()),
            enchant_caps: Some(EquipmentValues {
                slash: 250,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            part.resolve_enchant_caps(&catalog),
            Some(EquipmentValues {
                slash: 250,
                ..Default::default()
            })
        );
    }

    #[test]
    fn resolve_enchant_capsはどちらも無ければ未収録() {
        let catalog: [MockCatalogEntry; 0] = [];
        let part = EquipmentPart {
            item_id: None,
            custom_name: Some("自作の剣".to_string()),
            enchant_caps: None,
            ..Default::default()
        };
        assert_eq!(part.resolve_enchant_caps(&catalog), None);
    }

    #[test]
    fn カスタム装備のエンチャントが実測上限を超えると検証エラー() {
        let mut equipment = Equipment::default();
        equipment.parts.weapon = EquipmentPart {
            item_id: None,
            custom_name: Some("自作の剣".to_string()),
            enchant: EquipmentValues {
                slash: 260,
                ..Default::default()
            },
            enchant_caps: Some(EquipmentValues {
                slash: 250,
                ..Default::default()
            }),
            ..Default::default()
        }
        .into();
        let catalog: [MockCatalogEntry; 0] = [];
        let result = equipment.validate_against_catalog(&catalog, &[], &[]);
        assert!(result.is_err(), "実測上限超過を検出できていない: {result:?}");
    }
}
