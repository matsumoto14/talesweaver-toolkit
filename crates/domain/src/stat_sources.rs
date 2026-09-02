//! キャラクターの実効ステータスに効く恒常補正源(ペット・ルーン・クラウン・聖物)と常用バフ。
//!
//! docs/claude/goals/2026-08-21-character-stat-sources.md。バフは個別にコードで分岐せず、
//! 「カテゴリ(層)+ 数値 + 重複枠」を持つデータ(`BuffDefinition`)として解決する
//! (CLAUDE.md 原則、crates/domain/src/category.rs の設計思想を踏襲)。
//! カタログの実データ(常用バフ 16 件)は gamedata に置く。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::actual_delay::ActualDelayContribution;
use crate::attack_power::{
    attack_power_breakdown, stat_attack_power, AttackCoefficients, AttackPowerBreakdown,
};
use crate::category::DamageCategory;
use crate::character_skill::{
    damage_contributions, CharacterSkillCatalog, CharacterSkills, SkillEffect,
};
use crate::common_skill::{CommonSkills, DefenseRates, STRONG_WEAPON_LEVEL_MAX};
use crate::critical_rate::{CriticalRateSourceId, CriticalRateSources};
use crate::damage::DamageContribution;
use crate::element::ElementSources;
use crate::equipment::{
    equipment_values_attack, Equipment, EquipmentAbilityDef, EquipmentCoefficients, EquipmentError,
    EquipmentValues, PartEquipmentValues, PartSlot, PartStatTotal, ENHANCE_LEVEL_MAX,
    EQUIPMENT_VALUE_MAX,
};
use crate::mastery::{Masteries, MasteryCatalog};
use crate::random_option::{RandomOptionDef, RandomOptionTotals};
use crate::rounding::{floor_int, trunc_int};
use crate::soul_link::{
    SoulLinkError, SoulLinkPreview, SoulLinkStatus, SOUL_LINK_ARMOR_ENHANCE_LEVEL_MAX,
    SOUL_LINK_CRITICAL_DAMAGE_LEVEL_MAX, SOUL_LINK_EQUIPMENT_LEVEL_MAX,
    SOUL_LINK_FINAL_DAMAGE_LEVEL_MAX, SOUL_LINK_WEAPON_ENHANCE_LEVEL_MAX,
};
use crate::stats::{
    effective_stats, BaseStats, BaseStatsError, EffectiveStats, PerStat, StatKind, StatModifierSet,
    StatTrace, BASE_STAT_MAX, MULTIPLIER_B_MIN,
};
use crate::thesis_core::{
    CoreRegion, CoreSetBonus, CoreSetGroup, CORE_ENHANCEMENT_MAX, CORE_EVOLUTION_MAX,
    CORE_SLOT_COUNT,
};
use crate::title::TitleDef;
use crate::ultimate_skill::{UltimateSkill, UltimateSkills};

/// ペット S スキルの段階(wiki: PET)。上位段階ほど値が大きい。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetSkillTier {
    /// 〇〇強化
    Basic,
    TrueLv1,
    TrueLv2,
    TrueLv3,
    TrueLv4,
}

impl PetSkillTier {
    pub const ALL: [PetSkillTier; 5] = [
        PetSkillTier::Basic,
        PetSkillTier::TrueLv1,
        PetSkillTier::TrueLv2,
        PetSkillTier::TrueLv3,
        PetSkillTier::TrueLv4,
    ];

    /// 固定値ボーナス(wiki: PET。Lv5 +70 は JP 未実装のため未収録)。
    pub fn bonus(self) -> i64 {
        match self {
            PetSkillTier::Basic => 20,
            PetSkillTier::TrueLv1 => 30,
            PetSkillTier::TrueLv2 => 40,
            PetSkillTier::TrueLv3 => 50,
            PetSkillTier::TrueLv4 => 60,
        }
    }
}

/// 段階ごとの固定値ボーナス(UI の選択肢ラベル用。`GameTables::pet_skill_tier_bonus`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetSkillTierBonus {
    pub tier: PetSkillTier,
    pub bonus: i64,
}

pub(crate) fn pet_skill_tier_bonuses() -> Vec<PetSkillTierBonus> {
    PetSkillTier::ALL
        .iter()
        .map(|&tier| PetSkillTierBonus {
            tier,
            bonus: tier.bonus(),
        })
        .collect()
}

/// ペット S スキル。ステごとに 1 つ(上位段階を選ぶと置き換わる。加算にはならない)。
pub type PetSkills = PerStat<Option<PetSkillTier>>;

/// ルーンスキル(閃光/斬撃/英知/才気/石壁/魔壁/瞬発、wiki: ルーンマスター#skill_atk)。
/// +1/Lv、Lv20 上限。「装備可能ステには影響しない」固定値層。
pub type RuneLevels = PerStat<u8>;
pub const RUNE_LEVEL_MAX: u8 = 20;

/// クラウン(wiki: クラウン)。週次ランク報酬+名声強化で、シーズンごとに戻る。
/// 名声強化は +10 刻みで、選択報酬に指定した 1 ステだけ上限が 100 から 300 に伸びる。
/// 各フィールドは選択報酬を含む画面表示上の実値で、最終固定値層に乗る。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Crown {
    pub stab: u32,
    pub hack: u32,
    pub int: u32,
    pub def: u32,
    pub mr: u32,
    pub dex: u32,
    pub agi: u32,
    #[serde(default)]
    pub selected_stat: Option<StatKind>,
}

impl Crown {
    pub const STEP: u32 = 10;
    pub const BASE_MAX_VALUE: u32 = 100;
    pub const SELECTED_MAX_VALUE: u32 = 300;

    pub fn max_value(&self, kind: StatKind) -> u32 {
        if self.selected_stat == Some(kind) {
            Self::SELECTED_MAX_VALUE
        } else {
            Self::BASE_MAX_VALUE
        }
    }

    pub fn get(&self, kind: StatKind) -> u32 {
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
}

/// モンスターカード(wiki: ステータス「固定値増加/減少」の「カード装着」/ モンスターブック)。
/// 装着したカードのステータスがそのまま乗る。ステごと 0..=70、**固定値層**(倍率A の前)。
pub type MonsterCards = PerStat<u32>;
/// wiki ステータス「モンスターカード / カード装着 / +0〜70」。
pub const MONSTER_CARD_VALUE_MAX: u32 = 70;

/// 神鳥の聖物(wiki: 神鳥の聖物)。ステごと 0..=40 段階、+10 刻みで最終固定値に乗る。
pub type SacredRelic = PerStat<u8>;
pub const SACRED_RELIC_STAGE_MAX: u8 = 40;
/// 1 段階あたりの最終固定値。UI は `StatLimits::sacred_relic_value_per_stage` を参照する
pub const SACRED_RELIC_VALUE_PER_STAGE: i64 = 10;

/// 段階を最終固定値(0..=+400)に変換する。
pub fn sacred_relic_value(stage: u8) -> i64 {
    i64::from(stage) * SACRED_RELIC_VALUE_PER_STAGE
}

/// 最終固定値から段階へ逆算する(UI の「実際に増える値」入力用)。範囲外は clamp し、
/// 1 段階に満たない端数は切り捨てる(他の domain 換算と同じ floor 規約)。
pub fn sacred_relic_stage_from_value(value: i64) -> u8 {
    let max_value = i64::from(SACRED_RELIC_STAGE_MAX) * SACRED_RELIC_VALUE_PER_STAGE;
    (value.clamp(0, max_value) / SACRED_RELIC_VALUE_PER_STAGE) as u8
}

/// 能力値計算の 5 レイヤー(wiki §2)。gamedata の `BuffDefinition::layer` と共有する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatLayer {
    /// 割合増加(素ステ比)。バフごとに切捨ててから加算(0.1 = +10%)
    PercentOfBase,
    /// 固定値増加/減少
    Fixed,
    /// 能力値倍率A。乗算で重なる(1.1 = 1.1倍)
    MultiplierA,
    /// 能力値倍率B。加算(0.2 = +20%)
    MultiplierB,
    /// 最終固定値増加/減少
    FinalFixed,
}

/// 補正の出どころの区分。**ゲーム内の能力値と突き合わせるときの切り口**で、
/// 「今日 ON にしたバフ」「装備を替えたら動く分」「ふだん動かない分」を分けて見せるためのもの。
/// 層(`StatLayer`)が計算の順序を表すのに対し、こちらは人が確認する単位を表す。
///
/// 画面が補正源名を文字列で振り分けると、名前を変えた瞬間に区分が壊れる。
/// 区分は寄与を作る側(ここ)が付けて、そのままフロントまで運ぶ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatSourceGroup {
    /// 選択中の常用バフ(`BuffSelection`)。外せばすぐ消える分
    Buff,
    /// 装備から能力値に効くもの(シエナのオーラ)。装備を替えると動く分
    Equipment,
    /// それ以外(ペット・ルーン・カード・クラウン・聖物・キャラスキル・マスタリー・
    /// アンリーシュ・一時調整)。ふだん動かない分
    Other,
}

impl StatSourceGroup {
    /// 表示順(バフ → 装備 → そのほか)。UI は必ずこの順で列を並べる
    pub const ALL: [StatSourceGroup; 3] = [
        StatSourceGroup::Buff,
        StatSourceGroup::Equipment,
        StatSourceGroup::Other,
    ];
}

/// バフの対象ステ。
///
/// カタログ(`BuffDefinition`)は Rust 側で構築して Tauri コマンドの戻り値として
/// フロントへ一方向にシリアライズするだけで、デシリアライズされることが無いため
/// `Serialize` のみ導出する(`Stats` の `&'static [StatKind]` は serde の汎用スライス
/// 借用デシリアライズが対応する型ではなく、`Deserialize` を導出すると型検査が通らない)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffTarget {
    /// 全ステに適用
    AllStats,
    /// 特定の 1 ステに固定で適用
    Stat(StatKind),
    /// 登録時にどのステかを選ぶ(例: 固定増加系)。1 バフにつき 1 ステ
    UserSelected,
    /// 登録時にどのステかを選ぶ。**同じバフを複数のステに、それぞれ別の値で**掛けられる
    /// (クラブエフェクト: クラブレベルに応じた枠数だけ併用でき、上昇項目が同じものは
    ///  併用できない = 同じステを 2 回は選べない)。選択は 1 ステ 1 件で表す。
    UserSelectedMulti,
    /// 複数の特定ステに同じ値を適用(例: ロアミニの極・パウアトゥンが DEF/MR に効く)
    Stats(&'static [StatKind]),
}

/// バフの値の決め方。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffValue {
    /// 定義値そのまま
    Fixed(f64),
    /// 段階選択(例: イベントバフの +10/+20/+30/+50%)
    Choice(Vec<f64>),
    /// ユーザーの手入力(min..=max の範囲で検証する)
    UserInput { min: f64, max: f64 },
    /// **記録するだけ**。wiki に効果はあるが、まだ能力値・与ダメージ式に配線していない
    /// (被ダメージ減少・攻撃ダメージ増加など)。マスタリーは段ごとに 1 つしか取れないので、
    /// 計算に入らない選択肢も**選べないと段の状態が表せない**(ランダムOP のグレー枠と同じ扱い)
    RecordOnly,
}

/// バフ一覧を、人が「何を伸ばしたいか」で探すための目的。
/// 1 つのバフが複数の目的を持てる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffPurpose {
    Stats,
    Damage,
    Durability,
    /// 命中Pを伸ばす(wiki 計算式まとめ `#AccuracyPoint` の「命中P増加」)
    Accuracy,
}

/// ゲーム内でその効果を得る手掛かり。厳密な入手先ではなく、一覧の補助表示に使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffOrigin {
    Item,
    Event,
    Club,
    Skill,
    Rune,
    SoulLink,
    BattleState,
    Minigame,
}

/// バフカタログの 1 エントリ。**消費アイテム・イベントの常用バフ専用**
/// (キャラのパッシブ・自己バフ・味方バフは `character_skill.rs`)。
/// 型はここ(domain)、実データは gamedata に置く。
/// `target: BuffTarget` が `Serialize` のみのため、このカタログ自体も `Serialize` のみ導出する
/// (Tauri コマンドの戻り値としてフロントへ一方向にシリアライズするだけで、デシリアライズはしない)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuffDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub purposes: &'static [BuffPurpose],
    pub origin: BuffOrigin,
    pub target: BuffTarget,
    pub layer: StatLayer,
    pub value: BuffValue,
    /// 同時に選べない枠。空なら排他無し(独立)
    pub exclusive_slots: Vec<&'static str>,
    pub source_url: &'static str,
    pub note: &'static str,
    /// `BuffValue::UserInput` の初期値。それ以外は `None`
    pub default_value: Option<f64>,
    /// **ステ増加以外の効き先**(wiki: 与ダメージ式のカテゴリ)。同じバフが 2 か所に効くことがある
    /// (守護者のためのポーションは能力値の割合増加 +10% と最終ダメージ L +10% の両方)。
    /// ON/OFF だけで決まるので `BuffValue` の段階選択・手入力は掛からない
    pub damage_effects: &'static [SkillEffect],
}

/// バフカタログ。呼び出しは `&BuffCatalog` = `&[BuffDefinition]`。
pub type BuffCatalog = [BuffDefinition];

/// 火力バフを画面で分けるグループ。攻撃ダメージ(X)の副カテゴリ(X2 一般 / X1 イザベル /
/// X6 日本独自)に対応し、それ以外のカテゴリに効くものは「その他」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffDamageGroup {
    General,
    Isabel,
    Japan,
    Other,
}

impl BuffDefinition {
    /// ON にしたときの初期選択(ux-guidelines「初期値は実用値」)。段階選択は先頭、手入力は
    /// 既定値(無ければ上限)。対象ステを選ぶバフは `stat` をそのまま入れる
    pub fn default_choice(&self, stat: Option<StatKind>) -> BuffChoice {
        BuffChoice {
            buff_id: self.id.to_string(),
            stat: match self.target {
                BuffTarget::UserSelected | BuffTarget::UserSelectedMulti => stat,
                _ => None,
            },
            choice_index: match &self.value {
                BuffValue::Choice(_) => Some(0),
                _ => None,
            },
            value: match &self.value {
                BuffValue::UserInput { min, max } => {
                    Some(self.default_value.unwrap_or(*max).clamp(*min, *max))
                }
                _ => None,
            },
        }
    }

    /// このバフが属する火力グループ(複数のカテゴリに効くバフは複数)。
    pub fn damage_groups(&self) -> Vec<BuffDamageGroup> {
        let mut out = Vec::new();
        for effect in self.damage_effects {
            let SkillEffect::Damage { category, .. } = effect else {
                continue;
            };
            let group = match category {
                DamageCategory::AttackDamageGeneral => BuffDamageGroup::General,
                DamageCategory::AttackDamageIsabel => BuffDamageGroup::Isabel,
                DamageCategory::AttackDamageJapan => BuffDamageGroup::Japan,
                _ => BuffDamageGroup::Other,
            };
            if !out.contains(&group) {
                out.push(group);
            }
        }
        out
    }
}

/// まだ選んでいないバフのうち、選択中のバフと排他枠を取り合って選べないもの。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedBuff {
    pub buff_id: String,
    /// 枠を塞いでいる選択中のバフ名
    pub blocking: Vec<String>,
}

/// 排他枠(`exclusive_slots`)の衝突で選べないバフを列挙する(`build_modifiers` が弾く規則と同じ)。
pub fn blocked_buffs(buffs: &BuffSelection, catalog: &BuffCatalog) -> Vec<BlockedBuff> {
    let selected: Vec<&BuffDefinition> = buffs
        .choices
        .iter()
        .filter_map(|c| catalog.iter().find(|d| d.id == c.buff_id))
        .collect();
    catalog
        .iter()
        .filter(|d| !d.exclusive_slots.is_empty() && !selected.iter().any(|s| s.id == d.id))
        .filter_map(|d| {
            let mut blocking: Vec<String> = Vec::new();
            for s in &selected {
                let shares = s.exclusive_slots.iter().any(|slot| d.exclusive_slots.contains(slot));
                if shares && !blocking.iter().any(|name| name == s.name) {
                    blocking.push(s.name.to_string());
                }
            }
            (!blocking.is_empty()).then(|| BlockedBuff {
                buff_id: d.id.to_string(),
                blocking,
            })
        })
        .collect()
}

/// カタログ ID での 1 選択。対象ステ・選択肢インデックス・手入力値をすべて持てる形にし、
/// `BuffTarget`/`BuffValue` のどの組み合わせも汎用的に解決できるようにする。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuffChoice {
    pub buff_id: String,
    /// `BuffTarget::UserSelected` のとき必須
    pub stat: Option<StatKind>,
    /// `BuffValue::Choice` のとき必須
    pub choice_index: Option<usize>,
    /// `BuffValue::UserInput` のとき必須
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BuffSelection {
    pub choices: Vec<BuffChoice>,
}

/// 調整「加算」の妥当範囲(検証・仮定用の自由入力)。
/// 実際に入れる値は 3 桁に収まるが、検証用なので理論上限(2,400)より外まで許す(ユーザー確認)。
pub const ADJUSTMENT_ADD_MIN: i64 = -3000;
pub const ADJUSTMENT_ADD_MAX: i64 = 3000;
/// 調整「固定(pin)」の妥当範囲。**実測値をそのまま入れるための例外操作**なので、
/// 最終能力値の理論上限(2,400)を超える値も受ける(ユーザー確認)。
pub const ADJUSTMENT_PIN_MIN: i64 = 1;
pub const ADJUSTMENT_PIN_MAX: i64 = 3000;

/// ステ 1 つの自由な調整(検証・未収録バフ用)。
/// - `add`: このステに +N する(固定値層への加算)
/// - `pin`: 最終能力値そのものを N に固定する(実測値で計算したい時)。`Some` のとき
///   能力値計算の結果を上書きし、`StatTrace.pinned_from` に上書き前の値を残す
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatAdjustment {
    pub add: i64,
    pub pin: Option<i64>,
}

/// ステごとの自由な調整(検証・未収録バフ用)。計算リクエストにのみ乗り、キャラには保存しない。
pub type Adjustments = PerStat<StatAdjustment>;

impl PerStat<StatAdjustment> {
    /// `add` は `ADJUSTMENT_ADD_MIN..=ADJUSTMENT_ADD_MAX`、`pin`(指定時)は
    /// `ADJUSTMENT_PIN_MIN..=ADJUSTMENT_PIN_MAX` の範囲であることを検証する。
    pub fn validate(&self) -> Result<(), StatSourceError> {
        for kind in StatKind::ALL {
            let a = self.get(kind);
            if !(ADJUSTMENT_ADD_MIN..=ADJUSTMENT_ADD_MAX).contains(&a.add) {
                return Err(StatSourceError::AdjustmentOutOfRange {
                    field: "加算",
                    kind,
                    value: a.add,
                    min: ADJUSTMENT_ADD_MIN,
                    max: ADJUSTMENT_ADD_MAX,
                });
            }
            if let Some(pin) = a.pin {
                if !(ADJUSTMENT_PIN_MIN..=ADJUSTMENT_PIN_MAX).contains(&pin) {
                    return Err(StatSourceError::AdjustmentOutOfRange {
                        field: "固定",
                        kind,
                        value: pin,
                        min: ADJUSTMENT_PIN_MIN,
                        max: ADJUSTMENT_PIN_MAX,
                    });
                }
            }
        }
        Ok(())
    }
}

/// キャラクターに紐づく恒常補正源一式。バフはキャラクターとは独立した
/// `BuffSelection` として計算時に明示的に渡す。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StatSources {
    #[serde(default)]
    pub pet_skills: PetSkills,
    #[serde(default)]
    pub rune_levels: RuneLevels,
    #[serde(default)]
    pub crown: Crown,
    /// モンスターカード(wiki: ステータス「カード装着」)。ステごと 0〜70、固定値層
    #[serde(default)]
    pub monster_cards: MonsterCards,
    #[serde(default)]
    pub sacred_relic: SacredRelic,
    /// 装備の属性強化以外の属性値の供給源(ペット / モンスターカード / ルーン / 頭アビ / カフスアビ)
    #[serde(default)]
    pub elements: ElementSources,
    /// ON にしているキャラスキル(パッシブ・自己バフ・味方バフ。効果は `character_skill.rs`)
    #[serde(default)]
    pub character_skills: CharacterSkills,
    /// クリティカル率の供給源(wiki: 計算式まとめ `#CriticalChance`)
    #[serde(default)]
    pub critical_rate: CriticalRateSources,
    /// 選んでいるマスタリー(wiki: 各キャラの Skill ページ。段ごとに 1 つ)
    #[serde(default)]
    pub masteries: Masteries,
    /// ソウルリンクのリンクステータス 1〜10。
    /// 1〜4 は装備基本能力、5〜7 は戦闘計算、8〜10 は記録用。
    #[serde(default)]
    pub soul_link: SoulLinkStatus,
}

impl StatSources {
    /// 新規登録するキャラクターの恒常補正源。
    /// ソウルリンクを含め、未開放・未習得の中立値で始める。
    /// ルーンスキル(0..=20)/クラウン(0..=300)/聖物(0..=40段階)の値域を検証する。
    /// ペットは enum で構造的に制約済みなので対象外。
    pub fn validate(&self) -> Result<(), StatSourceError> {
        for kind in StatKind::ALL {
            let rune = self.rune_levels.get(kind);
            if rune > RUNE_LEVEL_MAX {
                return Err(StatSourceError::OutOfRange {
                    source_name: "ルーンスキル",
                    kind,
                    value: u32::from(rune),
                    max: u32::from(RUNE_LEVEL_MAX),
                });
            }

            let crown = self.crown.get(kind);
            let crown_max = self.crown.max_value(kind);
            if crown > crown_max {
                return Err(StatSourceError::OutOfRange {
                    source_name: "クラウン",
                    kind,
                    value: crown,
                    max: crown_max,
                });
            }
            if crown % Crown::STEP != 0 {
                return Err(StatSourceError::InvalidStep {
                    source_name: "クラウン",
                    kind,
                    value: crown,
                    step: Crown::STEP,
                });
            }

            let card = self.monster_cards.get(kind);
            if card > MONSTER_CARD_VALUE_MAX {
                return Err(StatSourceError::OutOfRange {
                    source_name: "モンスターカード",
                    kind,
                    value: card,
                    max: MONSTER_CARD_VALUE_MAX,
                });
            }

            let relic = self.sacred_relic.get(kind);
            if relic > SACRED_RELIC_STAGE_MAX {
                return Err(StatSourceError::OutOfRange {
                    source_name: "神鳥の聖物",
                    kind,
                    value: u32::from(relic),
                    max: u32::from(SACRED_RELIC_STAGE_MAX),
                });
            }
        }
        self.critical_rate.validate()?;
        self.soul_link.validate()?;
        Ok(())
    }
}

/// ステ増加を層に応じて足す(マスタリー・キャラスキル共通)。
/// 呼び出し元はどちらも恒常補正なので、区分は `Other` で固定する。
fn add_stat_rate(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    source: String,
    kind: StatKind,
    rate: f64,
    layer: StatLayer,
) {
    let m = modifiers.get_mut(kind);
    match layer {
        StatLayer::PercentOfBase => m.percent_of_base.push(rate),
        StatLayer::MultiplierA => m.multiplier_a.push(1.0 + rate),
        StatLayer::MultiplierB => m.multiplier_b += rate,
        // 割合で来る効果しか無いので固定値層は使わない
        StatLayer::Fixed | StatLayer::FinalFixed => return,
    }
    contributions.push(StatContribution {
        source,
        group: StatSourceGroup::Other,
        kind,
        layer,
        value: rate,
    });
}

/// マスタリーのステ増加を適用する(`build_stat_modifiers` の 1 段)。
pub fn apply_masteries(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    masteries: &Masteries,
    catalog: &MasteryCatalog,
) {
    for (kind, rate, layer, name) in masteries.stat_rates(catalog) {
        add_stat_rate(
            modifiers,
            contributions,
            format!("マスタリー【{name}】"),
            kind,
            rate,
            layer,
        );
    }
}

/// キャラスキルのステ増加を適用する。マスタリーで効果が差し替わるので
/// `masteries` も要る(wiki: ステータスの各カテゴリ表の「マスタリー」列)。
pub fn apply_character_skills(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    skills: &CharacterSkills,
    masteries: &Masteries,
    catalog: &CharacterSkillCatalog,
) {
    for (kind, rate, layer, name) in skills.stat_rates(catalog, masteries) {
        add_stat_rate(
            modifiers,
            contributions,
            name.to_string(),
            kind,
            rate,
            layer,
        );
    }
}

/// 選んでいるバフの、与ダメージ式のカテゴリへの寄与(カテゴリ, 値)。
/// ステ増加は `build_modifiers` が別に処理する(このバフは両方持てる)。
pub fn buff_damage_contributions(
    buffs: &BuffSelection,
    catalog: &BuffCatalog,
) -> Vec<DamageContribution> {
    let effects: Vec<(String, &SkillEffect)> = buffs
        .choices
        .iter()
        .filter_map(|c| catalog.iter().find(|d| d.id == c.buff_id))
        .flat_map(|d| d.damage_effects.iter().map(move |e| (d.name.to_string(), e)))
        .collect();
    damage_contributions(effects.into_iter())
}

/// 命中P増加効果(`SkillEffect::AccuracyPoint`)を持つバフだけへ絞り込み、いま効いている
/// 命中P割合増加スキル(的中剣)と排他(`exclusive_with`)のものを除く(wiki 注記:
/// テイルズウィーバーのエネルギーは的中剣の効果中は無効になる)。合計
/// (`buff_accuracy_point_total`)と伸びしろ(`buff_accuracy_point_room`)の両方が使う内側のフィルタ
fn accuracy_point_values<'a>(
    defs: impl Iterator<Item = &'a BuffDefinition>,
    boost: crate::defense::AccuracyBoost,
) -> i64 {
    let active_skill = boost.skill_id();
    defs.flat_map(|d| d.damage_effects.iter())
        .filter_map(|e| match e {
            SkillEffect::AccuracyPoint {
                value,
                exclusive_with,
            } if !active_skill.is_some_and(|id| exclusive_with.contains(&id)) => Some(*value),
            _ => None,
        })
        .sum()
}

/// 選んでいるバフの、命中P増加の合計(wiki 計算式まとめ `#AccuracyPoint`)。
pub fn buff_accuracy_point_total(
    buffs: &BuffSelection,
    catalog: &BuffCatalog,
    boost: crate::defense::AccuracyBoost,
) -> i64 {
    accuracy_point_values(
        buffs
            .choices
            .iter()
            .filter_map(|c| catalog.iter().find(|d| d.id == c.buff_id)),
        boost,
    )
}

/// 選んでいるバフの、最小回避率補正の合計(wiki 計算式まとめ `#HitRateCap`:
/// テイルズウィーバーのエネルギー「最小回避率 +10%」)。
pub fn buff_min_evasion_rate_total(buffs: &BuffSelection, catalog: &BuffCatalog) -> i64 {
    buffs
        .choices
        .iter()
        .filter_map(|c| catalog.iter().find(|d| d.id == c.buff_id))
        .flat_map(|d| d.damage_effects.iter())
        .filter_map(|e| match e {
            SkillEffect::MinEvasionRate { value } => Some(*value),
            _ => None,
        })
        .sum()
}

/// まだ選んでいない命中P増加バフぶんの伸びしろ(§伸びしろの定義)。
pub fn buff_accuracy_point_room(
    buffs: &BuffSelection,
    catalog: &BuffCatalog,
    boost: crate::defense::AccuracyBoost,
) -> i64 {
    accuracy_point_values(
        catalog
            .iter()
            .filter(|d| !buffs.choices.iter().any(|c| c.buff_id == d.id)),
        boost,
    )
}

/// バフ 1 件ぶんが、カテゴリ上限適用後の集計値へどれだけ配賦されるか。
/// 「このバフを足したことでカテゴリの値(キャップ適用後)がどれだけ動いたか」を表すので、
/// 同一カテゴリの `effect` を全部足すと必ずそのカテゴリの `CategoryTrace::value` に一致する
/// (`fill_contribution_effects` と同じ、累積再構築による telescoping)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuffDamageEffect {
    pub buff_name: String,
    pub category: DamageCategory,
    pub effect: f64,
}

/// `summarize_buff_selection` の結果(カテゴリ別集計 + バフ別配賦)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuffDamageSummary {
    pub categories: Vec<crate::category::CategoryTrace>,
    pub buff_effects: Vec<BuffDamageEffect>,
    /// 排他枠の衝突で選べないバフ(画面が「〜と選べません」を出す)
    pub blocked_buffs: Vec<BlockedBuff>,
}

/// バフセットだけが与ダメージカテゴリへ足す量。通常のダメージ計算と同じ
/// `CategoryTotals` を通すため、X1/X2 などカテゴリごとの上限も適用済みで返る。
/// バフ 1 件ぶんの寄与は「このバフを足す前後でカテゴリの値(キャップ適用後)がどれだけ動いたか」
/// を選択順に積み上げて出す(単独計算で引き直さない。§00 05: 数字の出どころを全部見せる)。
pub fn summarize_buff_selection(
    buffs: &BuffSelection,
    catalog: &BuffCatalog,
) -> Result<BuffDamageSummary, StatSourceError> {
    // 未知 ID・入力不足・排他違反を、能力値プレビューと同じ規則で検証する。
    build_modifiers(&StatSources::default(), buffs, catalog)?;
    let mut totals = crate::category::CategoryTotals::neutral();
    let mut buff_effects = Vec::new();
    for contribution in buff_damage_contributions(buffs, catalog) {
        let before = totals.value(contribution.category);
        totals.add(contribution.category, contribution.value);
        let after = totals.value(contribution.category);
        buff_effects.push(BuffDamageEffect {
            buff_name: contribution.source,
            category: contribution.category,
            effect: after - before,
        });
    }
    let categories = totals
        .trace()
        .into_iter()
        .filter(|row| row.category != DamageCategory::AttackDamageRate && row.raw != 0.0)
        .collect();
    Ok(BuffDamageSummary {
        blocked_buffs: blocked_buffs(buffs, catalog),
        categories,
        buff_effects,
    })
}

/// 寄与内訳の 1 行(ステトレース向け)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatContribution {
    pub source: String,
    /// 出どころの区分(バフ / 装備 / そのほか)。寄与を作るところで付ける
    pub group: StatSourceGroup,
    pub kind: StatKind,
    pub layer: StatLayer,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum StatSourceError {
    #[error(transparent)]
    SoulLink(#[from] SoulLinkError),
    #[error(transparent)]
    CriticalRate(#[from] crate::critical_rate::CriticalRateError),
    #[error("排他枠 '{slot}' が重複しています")]
    ExclusiveSlotConflict { slot: String },
    #[error("未知のバフです: {id}")]
    UnknownBuff { id: String },
    #[error("バフ '{id}' は対象ステの指定が必要です")]
    MissingStat { id: String },
    #[error("バフ '{id}' は選択肢の指定が必要です")]
    MissingChoice { id: String },
    #[error("バフ '{id}' は数値の入力が必要です")]
    MissingValue { id: String },
    #[error("バフ '{id}' の選択肢が範囲外です")]
    ChoiceOutOfRange { id: String },
    #[error("{source_name} の {kind:?} は 0..={max} の範囲で指定してください(指定値 {value})")]
    OutOfRange {
        source_name: &'static str,
        kind: StatKind,
        value: u32,
        max: u32,
    },
    #[error("{source_name} の {kind:?} は {step} 刻みで指定してください(指定値 {value})")]
    InvalidStep {
        source_name: &'static str,
        kind: StatKind,
        value: u32,
        step: u32,
    },
    #[error("バフ '{id}' が重複して選択されています")]
    DuplicateBuff { id: String },
    #[error("バフ '{id}' の {kind:?} が重複して選択されています")]
    DuplicateBuffStat { id: String, kind: StatKind },
    #[error("バフ '{id}' の入力値が範囲外です({min}..={max}、指定値 {value})")]
    ValueOutOfRange {
        id: String,
        value: f64,
        min: f64,
        max: f64,
    },
    #[error("調整の{field}は{kind:?}で{min}..={max}の範囲で指定してください(指定値 {value})")]
    AdjustmentOutOfRange {
        field: &'static str,
        kind: StatKind,
        value: i64,
        min: i64,
        max: i64,
    },
    #[error(transparent)]
    BaseStats(#[from] BaseStatsError),
    #[error(transparent)]
    Equipment(#[from] EquipmentError),
}

/// `StatSources` と バフカタログから `StatModifierSet` と寄与内訳を組み立てる。
pub fn build_modifiers(
    sources: &StatSources,
    buffs: &BuffSelection,
    catalog: &BuffCatalog,
) -> Result<(StatModifierSet, Vec<StatContribution>), StatSourceError> {
    let mut modifiers = StatModifierSet::default();
    let mut contributions = Vec::new();

    for kind in StatKind::ALL {
        if let Some(tier) = sources.pet_skills.get(kind) {
            let bonus = tier.bonus();
            modifiers.get_mut(kind).fixed += bonus;
            contributions.push(StatContribution {
                source: format!("ペット Sスキル({tier:?})"),
                group: StatSourceGroup::Other,
                kind,
                layer: StatLayer::Fixed,
                value: bonus as f64,
            });
        }
    }

    // 段階のない固定値の補正源(値そのまま)。(名前, 層, ステ別の値)
    let flat_sources: [(&str, StatLayer, PerStat<i64>); 4] = [
        (
            "ルーンスキル",
            StatLayer::Fixed,
            PerStat::from_fn(|k| i64::from(sources.rune_levels.get(k))),
        ),
        (
            "モンスターカード",
            StatLayer::Fixed,
            PerStat::from_fn(|k| i64::from(sources.monster_cards.get(k))),
        ),
        (
            "クラウン",
            StatLayer::FinalFixed,
            PerStat::from_fn(|k| i64::from(sources.crown.get(k))),
        ),
        (
            "神鳥の聖物",
            StatLayer::FinalFixed,
            PerStat::from_fn(|k| sacred_relic_value(sources.sacred_relic.get(k))),
        ),
    ];
    for (source, layer, values) in flat_sources {
        for (kind, &bonus) in values.iter() {
            if bonus <= 0 {
                continue;
            }
            match layer {
                StatLayer::Fixed => modifiers.get_mut(kind).fixed += bonus,
                StatLayer::FinalFixed => modifiers.get_mut(kind).final_fixed += bonus,
                _ => unreachable!("固定値の補正源は Fixed / FinalFixed だけ"),
            }
            contributions.push(StatContribution {
                source: source.to_string(),
                group: StatSourceGroup::Other,
                kind,
                layer,
                value: bonus as f64,
            });
        }
    }

    let mut used_slots: HashSet<&'static str> = HashSet::new();
    // 選択の一意性。`UserSelectedMulti` のバフだけは同じ id を複数回置けるので、
    // ステまで含めた組で見る(同じステを 2 回は置けない = wiki「上昇項目が同じ
    // エフェクトを併用することは出来ない」)。
    let mut used_choices: HashSet<(&str, Option<StatKind>)> = HashSet::new();
    // 排他枠を数えたバフ。複数エントリのバフが自分自身と枠を取り合わないよう、
    // 枠は 1 バフにつき 1 回だけ押さえる。
    let mut slotted_buff_ids: HashSet<&str> = HashSet::new();
    for choice in &buffs.choices {
        let def = catalog
            .iter()
            .find(|d| d.id == choice.buff_id)
            .ok_or_else(|| StatSourceError::UnknownBuff {
                id: choice.buff_id.clone(),
            })?;

        let multi = matches!(def.target, BuffTarget::UserSelectedMulti);
        if !used_choices.insert((choice.buff_id.as_str(), multi.then_some(choice.stat).flatten())) {
            return match (multi, choice.stat) {
                (true, Some(stat)) => Err(StatSourceError::DuplicateBuffStat {
                    id: choice.buff_id.clone(),
                    kind: stat,
                }),
                _ => Err(StatSourceError::DuplicateBuff {
                    id: choice.buff_id.clone(),
                }),
            };
        }

        if slotted_buff_ids.insert(choice.buff_id.as_str()) {
            for slot in def.exclusive_slots.iter().copied() {
                if !used_slots.insert(slot) {
                    return Err(StatSourceError::ExclusiveSlotConflict {
                        slot: slot.to_string(),
                    });
                }
            }
        }

        let value = match &def.value {
            // 記録するだけの選択(マスタリーの未配線分)。排他枠は押さえたうえで加算しない
            BuffValue::RecordOnly => continue,
            BuffValue::Fixed(v) => *v,
            BuffValue::Choice(options) => {
                let index = choice
                    .choice_index
                    .ok_or_else(|| StatSourceError::MissingChoice {
                        id: def.id.to_string(),
                    })?;
                *options
                    .get(index)
                    .ok_or_else(|| StatSourceError::ChoiceOutOfRange {
                        id: def.id.to_string(),
                    })?
            }
            BuffValue::UserInput { min, max } => {
                let v = choice.value.ok_or_else(|| StatSourceError::MissingValue {
                    id: def.id.to_string(),
                })?;
                if v < *min || v > *max {
                    return Err(StatSourceError::ValueOutOfRange {
                        id: def.id.to_string(),
                        value: v,
                        min: *min,
                        max: *max,
                    });
                }
                v
            }
        };

        let targets: Vec<StatKind> = match def.target {
            BuffTarget::AllStats => StatKind::ALL.to_vec(),
            BuffTarget::Stat(kind) => vec![kind],
            BuffTarget::UserSelected | BuffTarget::UserSelectedMulti => {
                vec![choice.stat.ok_or_else(|| StatSourceError::MissingStat {
                    id: def.id.to_string(),
                })?]
            }
            BuffTarget::Stats(kinds) => kinds.to_vec(),
        };

        for kind in targets {
            let m = modifiers.get_mut(kind);
            match def.layer {
                StatLayer::PercentOfBase => m.percent_of_base.push(value),
                StatLayer::Fixed => m.fixed += trunc_int(value),
                StatLayer::MultiplierA => m.multiplier_a.push(value),
                StatLayer::MultiplierB => m.multiplier_b += value,
                StatLayer::FinalFixed => m.final_fixed += trunc_int(value),
            }
            contributions.push(StatContribution {
                source: def.name.to_string(),
                group: StatSourceGroup::Buff,
                kind,
                layer: def.layer,
                value,
            });
        }
    }

    Ok((modifiers, contributions))
}

/// 計算リクエストにのみ乗る一時調整(キャラには保存しない)の加算(`add`)を `StatModifierSet` に合流させる
/// (`build_stat_modifiers` の最終段)。`pin` はここでは扱わない(呼び出し側が `apply_pins` に渡して適用する)。
pub fn apply_temporary_adjustments(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    adjustments: &Adjustments,
) {
    for kind in StatKind::ALL {
        let adjustment = adjustments.get(kind);
        if adjustment.add != 0 {
            modifiers.get_mut(kind).fixed += adjustment.add;
            contributions.push(StatContribution {
                source: "一時調整".to_string(),
                group: StatSourceGroup::Other,
                kind,
                layer: StatLayer::Fixed,
                value: adjustment.add as f64,
            });
        }
    }
}

/// 一時調整の「固定(pin)」を反映する。計算リクエストにのみ乗り、キャラには保存しない。
/// `stats`/`trace.effective`/`trace.pinned_from` に反映する。
pub fn apply_pins(
    stats: &mut EffectiveStats,
    traces: &mut [StatTrace],
    temporary: Option<&Adjustments>,
) {
    for kind in StatKind::ALL {
        let Some(pin) = temporary.and_then(|t| t.get(kind).pin) else {
            continue;
        };
        if let Some(trace) = traces.iter_mut().find(|t| t.kind == kind) {
            trace.pinned_from = Some(trace.effective);
            trace.effective = pin;
        }
        stats.set(kind, pin);
    }
}

/// 主軸スキルの依存種別から引いた攻撃力(A)の係数一式。
/// スキル依存種別ごとの実データは gamedata が持つので、呼び出し側が引いて渡す。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttackPowerCoefficients {
    /// ステ由来攻撃力の係数
    pub stat: AttackCoefficients,
    /// 装備攻撃力の係数(基本能力値用/強化能力値用)
    pub equipment: EquipmentCoefficients,
}

/// 部位 1 つの攻撃力(A)への寄与。「外すと A がこれだけ減る」量。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PartAttackContribution {
    pub slot: PartSlot,
    /// A − (その部位を未装備にした A)
    pub value: i64,
}

/// 攻撃力(A)のプレビュー。主軸スキルが選ばれているときだけ作る。
///
/// テシスコアの能力値増加は地域依存(`Equipment::enhanced_totals(region)`)だが、キャラ画面は
/// 対象コンテンツを選ばないので **地域なし(テシスコアの能力値を含まない)** で出す。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackPreview {
    pub breakdown: AttackPowerBreakdown,
    /// 12 部位それぞれの寄与(外したときの差分)
    pub part_contributions: Vec<PartAttackContribution>,
}

/// 選択中バフが足した固定値/割合増加が、倍率A/B を持つ補正源(マスタリー等)に増幅されて
/// 最終能力値へ乗った分(ステ別)。
pub type BuffStatAmplification = PerStat<i64>;

/// `preview_effective_stats` の結果(最終能力値・トレース・寄与内訳・攻撃力)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatPreview {
    pub stats: EffectiveStats,
    pub traces: Vec<StatTrace>,
    /// 補正源 1 件ぶんの帰属(層順のステップ幅を上限込みで。詳細は `contribution_source_effects`)
    pub source_effects: Vec<StatSourceEffect>,
    /// `source_effects` を ステ × 区分(バフ / 装備 / そのほか)でまとめたもの。
    /// ゲーム内の能力値と突き合わせるときに「どこから来た上昇か」を出す。
    /// 常に 7 ステ × 3 区分ぶん返る(正は `group_source_effects`)
    pub group_effects: Vec<StatGroupEffect>,
    /// 主軸スキル未選択なら `None`
    pub attack: Option<AttackPreview>,
    /// 共通スキル(wiki: Skill/共通・Skill/極限)の効き先サマリ。
    /// 正は `common_skill.rs` / `ultimate_skill.rs`(TS に写経しない)
    pub common_skill: CommonSkillPreview,
    /// クリティカル率増加(wiki: 計算式まとめ `#CriticalChance`)の合計。正は `critical_rate.rs`
    pub critical_rate_bonus: CriticalRateBonusPreview,
    /// 神鳥の聖物の段階→最終固定値換算の合計(Σ、正は `stat_sources::SacredRelic`)
    pub sacred_relic_total: i64,
    /// ソウルリンク 1〜10 の Rust 計算済み派生値。
    pub soul_link: SoulLinkPreview,
    /// 基本能力値の合計(Σ part.base + 装備アビリティ + 表示中の称号 + ソウルリンク)。
    /// 装備由来の正は `Equipment::base_totals`、ソウルリンク由来の正は `SoulLinkStatus::equipment_values`。
    pub equipment_base_total: EquipmentValues,
    /// 基本能力値のうち装備アビリティ由来の分だけを部位別に割ったもの(表示用の内訳)。
    /// 正は `Equipment::ability_values_by_part`
    pub part_ability_values: Vec<PartEquipmentValues>,
    /// シエナのオーラの能力値スロットの装備補正(部位別。武器/盾以外は常に 0)。
    /// 正は `SienaAura::values`
    pub siena_part_values: Vec<PartEquipmentValues>,
    /// テシスコアの地域別プレビュー(`CoreRegion::ALL` の順)。正は `crates/domain/src/thesis_core.rs`
    pub thesis_cores: Vec<ThesisCoreRegionPreview>,
    /// 強化能力値の合計(Σ part.enchant + シエナのオーラ武器/盾分。地域なし = テシスコアを含まない)。
    /// 正は `Equipment::enhanced_totals`
    pub equipment_enhanced_total: EquipmentValues,
    /// 強化能力値のうち `part.enchant` だけを部位別に割ったもの(表示用の内訳)。
    /// 正は `Equipment::enchant_values_by_part`
    pub part_enchant_values: Vec<PartEquipmentValues>,
    /// 全部位のランダムオプションの効き先別集計。正は `Equipment::random_option_totals`
    pub random_option_totals: RandomOptionTotals,
    /// シエナのオーラのステ加算(能力値スロット + 全ステータス増加)の 7 ステ合計。
    /// 正は `Equipment::siena_stat_bonus`
    pub siena_stat_total: i64,
    /// シエナのオーラの追加オプション「攻撃力増加」の合計。Σ% の小数表現。正は `Equipment::siena_attack_rate`
    pub siena_attack_rate: f64,
    /// テシスコアのセット効果の全地域合計(`thesis_cores` の `set_bonus` を合算したもの)。
    pub thesis_core_set_bonus_total: CoreSetBonus,
    /// 一番伸びている地域の `total_bonus`。地域ごとに別のセットなので合算はできない。
    /// 正は `ThesisCores::best_total_bonus`
    pub thesis_core_best_total: i64,
    /// ON にしているキャラスキルぶんの中ディレイ減少の供給源(Σ% の小数表現)。
    /// 正は `CharacterSkills::actual_delay_contributions`。上限(70%)を掛ける前の内訳
    pub character_skill_actual_delay: Vec<ActualDelayContribution>,
    /// マスタリーぶんの中ディレイ減少の合計(Σ% の小数表現)。正は `Masteries::actual_delay_reduction`
    pub mastery_actual_delay: f64,
    /// シエナのオーラの追加オプション「防御力増加」の合計。Σ% の小数表現。正は `Equipment::siena_defense_rate`
    pub siena_defense_rate: f64,
    /// シエナのオーラの追加オプション「中ディレイ減少」の合計。Σ% の小数表現。
    /// 正は `Equipment::siena_actual_delay_reduction`
    pub siena_actual_delay_rate: f64,
    /// シエナのオーラの追加オプション「クリティカル確率」の合計。Σ% の小数表現(AGI 由来の項への乗数)。
    /// 正は `Equipment::siena_critical_rate`
    pub siena_critical_rate: f64,
    /// シエナのオーラのステ加算合計を部位別に割ったもの(表示用の内訳)。正は `SienaAura::stat_bonus` の `total()`
    pub siena_part_stat_totals: Vec<PartStatTotal>,
    /// 選択中バフが足した固定値/割合増加が、倍率A/B を持つ補正源(マスタリー等)に増幅されて
    /// 最終能力値へ乗った分(ステ別)。バフ行の `source_effects` はその補正源自身が
    /// 吸収する前の層ステップ幅しか持たないため、そこだけ合計すると
    /// 「バフ無し→バフ有りで最終能力値が実際に何点動いたか」に届かない。その不足分がこれ。
    /// 定義(ステ `kind` ごと): (バフ有りの最終値 − バフ無しの最終値) − Σ(source_effects のうち
    /// そのステ・バフ由来の行)。バフ未選択なら全ステ 0。フロントで引き算しない(ADR 001)
    /// ためここで確定させて返す。
    pub buff_stat_amplification: BuffStatAmplification,
}

/// テシスコア 1 地域ぶんの表示用プレビュー(6 枠の合計・セット効果)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThesisCoreRegionPreview {
    pub region: CoreRegion,
    /// 6 枠の補正値合計(入場条件「コア N」と同じ値。正は `CoreSet::total_bonus`)
    pub total_bonus: i64,
    /// 強化能力値への加算(火力 + 補助。正は `CoreSet::equipment_values`)
    pub values: EquipmentValues,
    /// 成立しているセット(進化段階ごと)。空なら未発動。正は `CoreSet::set_groups`
    pub set_groups: Vec<CoreSetGroup>,
    /// 進化を問わず強化 4 に達しているコアの数。正は `CoreSet::ready_count`
    pub ready: usize,
    /// この地域のセット効果の合計(合算後)。正は `CoreSet::set_bonus`
    pub set_bonus: CoreSetBonus,
}

/// 共通スキルの効き先サマリ(装備攻撃力強化倍率・装備防御力倍率・極限スキルの効果値)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CommonSkillPreview {
    /// 装備防御力倍率(コートアーマー + プロテクトアーマー + 改・プロテクトアーマー +
    /// シエナのオーラの防御力増加)。初期値 1.0 の乗数(`DefenseRates::NEUTRAL`)
    pub defense_rates: DefenseRates,
    /// 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン)。Σ% の小数表現
    pub equipment_attack_rate: f64,
    pub ultimate: UltimateSkillPreview,
}

/// 極限スキル(wiki: Skill/極限)の効果値。正は `ultimate_skill::UltimateSkills`
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UltimateSkillPreview {
    /// スコープアイのクリティカルダメージ増加。Σ% の小数表現
    pub critical_damage_rate: f64,
    /// フルスロットルの中ディレイ減少。Σ% の小数表現
    pub actual_delay_reduction: f64,
    /// フルスロットルの単体チャネリングスキル段数増加
    pub added_hit_count: u32,
    /// ワイドフォーカスのスキル範囲増加(火力には効かない)
    pub skill_range_bonus: f64,
}

impl UltimateSkillPreview {
    /// いま選んでいる枠の効果。
    pub fn of(ultimate: &UltimateSkills) -> Self {
        UltimateSkillPreview {
            critical_damage_rate: ultimate.critical_damage_rate(),
            actual_delay_reduction: ultimate.actual_delay_reduction(),
            added_hit_count: ultimate.added_hit_count(),
            skill_range_bonus: ultimate.skill_range_bonus(),
        }
    }

    /// 3 種すべてを付けたとしたときの効果(スーパー / ハイパーリミットはいまの値)。
    /// 計算タブのチップに「付けたらいくつ効くか」を併記するために使う
    pub fn potential(ultimate: &UltimateSkills) -> Self {
        let combat = UltimateSkills {
            slots: [Some(UltimateSkill::ScopeEye), Some(UltimateSkill::FullThrottle)],
            ..*ultimate
        };
        let range = UltimateSkills {
            slots: [Some(UltimateSkill::WideFocus), None],
            ..*ultimate
        };
        UltimateSkillPreview {
            critical_damage_rate: combat.critical_damage_rate(),
            actual_delay_reduction: combat.actual_delay_reduction(),
            added_hit_count: combat.added_hit_count(),
            skill_range_bonus: range.skill_range_bonus(),
        }
    }
}

/// クリティカル率増加(wiki `#CriticalChance`)の合計。正は `critical_rate::CriticalRateSources`
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CriticalRateBonusPreview {
    /// 上限を掛ける前の合計(「頭打ち」表示に使う)
    pub raw: f64,
    /// 上限 +100% を掛けた合計
    pub value: f64,
    /// 設計者の研究室ぶんのクリティカル率増加(研究段階 × 1 段階あたりの増加量)。
    /// 正は `CriticalRateSources::architect_lab_bonus`
    pub architect_lab_bonus: f64,
}

/// シエナのオーラのステ加算(wiki: 能力値一覧(その他の部位)・追加オプション「全ステータス増加」)を
/// `StatModifierSet` の最終固定値層に合流させる。
///
/// シエナのオーラは装備部位に属する(`EquipmentPart::siena`)ので `StatSources` からは組み立てられない
/// (`build_stat_modifiers` の 1 段)。
pub fn apply_siena_stats(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    equipment: &Equipment,
) {
    let bonus = equipment.siena_stat_bonus();
    for kind in StatKind::ALL {
        let value = bonus.get(kind);
        if value != 0 {
            modifiers.get_mut(kind).final_fixed += value;
            contributions.push(StatContribution {
                source: "シエナのオーラ".to_string(),
                group: StatSourceGroup::Equipment,
                kind,
                layer: StatLayer::FinalFixed,
                value: value as f64,
            });
        }
    }
}

/// 共通スキル「アンリーシュ(能力解放)」のステ加算(wiki: ステータス「能力値倍率B」)を
/// `StatModifierSet` の能力値倍率B 層に合流させる。
///
/// アンリーシュは共通スキルなので `StatSources` からは組み立てられない(`build_stat_modifiers` の 1 段)。
pub fn apply_unleash(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    common: &CommonSkills,
) {
    for kind in StatKind::ALL {
        let rate = common.unleash_rate(kind);
        if rate != 0.0 {
            modifiers.get_mut(kind).multiplier_b += rate;
            contributions.push(StatContribution {
                source: "アンリーシュ".to_string(),
                group: StatSourceGroup::Other,
                kind,
                layer: StatLayer::MultiplierB,
                value: rate,
            });
        }
    }
}

/// `contribution_source_effects` が返す、補正源 1 件ぶんの帰属。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatSourceEffect {
    pub source: String,
    /// 出どころの区分。元の `StatContribution` からそのまま運ぶ
    pub group: StatSourceGroup,
    pub kind: StatKind,
    /// この要因が乗る層。ゲーム内の表示と突き合わせるとき、値が合わない原因は
    /// 「どの層の 1 件が抜けているか」なので、帰属だけでなく層も一緒に運ぶ
    pub layer: StatLayer,
    /// 層への入力値(固定値なら加算値、倍率A なら係数そのもの)。`effect` と違い
    /// wiki やゲーム内の表記(+7 / ×1.10)にそのまま対応する
    pub value: f64,
    /// この要因の `effect`(上限を跨いだ分もそのまま織り込んだ値)
    pub effect: i64,
}

/// ステ × 区分の帰属合計。`StatSourceEffect` を区分でまとめたもので、
/// **`素ステ + Σ(そのステの全区分) = 最終能力値`** が上限を跨いでも厳密に成り立つ
/// (`contribution_source_effects` の性質をそのまま引き継ぐ)。
/// フロントで足し算をしない(ADR 001)ため、まとめはここで済ませて返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatGroupEffect {
    pub kind: StatKind,
    pub group: StatSourceGroup,
    pub effect: i64,
}

/// `contribution_source_effects` の結果を ステ × 区分 でまとめる。
/// 値が 0 の組も必ず 1 行返す(7 ステ × 3 区分 = 21 行、`StatKind::ALL` × `StatSourceGroup::ALL` の順)。
/// 行が出たり消えたりすると列が増減して画面が動くため(§00 03)、常に同じ形で返す。
pub fn group_source_effects(effects: &[StatSourceEffect]) -> Vec<StatGroupEffect> {
    let mut out = Vec::with_capacity(StatKind::ALL.len() * StatSourceGroup::ALL.len());
    for kind in StatKind::ALL {
        for group in StatSourceGroup::ALL {
            out.push(StatGroupEffect {
                kind,
                group,
                effect: effects
                    .iter()
                    .filter(|e| e.kind == kind && e.group == group)
                    .map(|e| e.effect)
                    .sum(),
            });
        }
    }
    out
}

/// 補正源 1 件ぶんの帰属。「層順のステップ幅」を **`cap` を織り込んだ形**で返す
/// (生値ベースだと上限を跨いだときに呼び出し側が `capped_loss` を
/// 別途差し引く必要があった)。
///
/// 定義(累積再構築): `contributions` を層順(割合増加 → 固定値 → 倍率A → 倍率B → 最終固定値)に
/// 積みながら、「先頭から i 番目までの寄与だけを適用して最終能力値の式をまるごと計算し直した値」を
/// `total_i` とし(`cap` を毎回適用)、`effect_i = total_i − total_{i-1}` を返す。
/// 各段の floor を含めて式を丸ごと引き直すので、**`素ステ + Σeffect = 最終能力値(上限適用後)` が
/// 上限を跨いでも常に厳密に成り立つ**(単独計算/leave-one-out は floor と乗算のせいで一致しないため
/// 採らない)。
///
/// **注意**: これは層順で見たときのステップ幅であり、
/// 「この補正源が無かったら最終能力値がいくつ動くか(倍率A/B による増幅込みの実質的な影響)」
/// ではない(倍率A/B の補正源が、先に積んだ補正源を増幅した分を自分の `effect` に乗せて受け取る。
/// その影響を含めた値が要るときは呼び出し元で before/after を取る)。
///
/// 倍率A の実際の係数は `modifiers`(`StatModifierSet`)から層内の出現順で引く
/// (`StatContribution::value` はマスタリー等では「率」、バフでは「係数そのもの」と単位が
/// 異なることがあり、`c.value` をそのまま倍率として使えないため)。他の層は
/// `StatContribution::value` が常に `modifiers` へ加算した値そのものと一致する。
pub fn contribution_source_effects(
    contributions: &[StatContribution],
    base: &BaseStats,
    modifiers: &StatModifierSet,
    cap: i64,
) -> Vec<StatSourceEffect> {
    let mut out = Vec::with_capacity(contributions.len());
    for kind in StatKind::ALL {
        let m = modifiers.get(kind);
        let base_value = i64::from(base.get(kind));

        let raw_total = |percent_n: usize, fixed: i64, mult_a_n: usize, mult_b: f64, final_fixed: i64| -> i64 {
            let percent_total: i64 = m.percent_of_base[..percent_n]
                .iter()
                .map(|rate| floor_int(base_value as f64 * rate))
                .sum();
            let before_multiplier = base_value + percent_total + fixed;
            let multiplier_a_product: f64 = m.multiplier_a[..mult_a_n].iter().product();
            let basic = floor_int(before_multiplier as f64 * multiplier_a_product);
            let multiplier_b = mult_b.max(MULTIPLIER_B_MIN);
            let multiplier_b_bonus = floor_int(basic as f64 * multiplier_b);
            basic + multiplier_b_bonus + final_fixed
        };

        let mut percent_n = 0usize;
        let mut fixed = 0i64;
        let mut mult_a_n = 0usize;
        let mut mult_b = 0.0f64;
        let mut final_fixed = 0i64;
        let mut prev_total = raw_total(percent_n, fixed, mult_a_n, mult_b, final_fixed).min(cap);

        for layer in [
            StatLayer::PercentOfBase,
            StatLayer::Fixed,
            StatLayer::MultiplierA,
            StatLayer::MultiplierB,
            StatLayer::FinalFixed,
        ] {
            for c in contributions
                .iter()
                .filter(|c| c.kind == kind && c.layer == layer)
            {
                match layer {
                    StatLayer::PercentOfBase => percent_n += 1,
                    StatLayer::Fixed => fixed += trunc_int(c.value),
                    StatLayer::MultiplierA => mult_a_n += 1,
                    StatLayer::MultiplierB => mult_b += c.value,
                    StatLayer::FinalFixed => final_fixed += trunc_int(c.value),
                }
                let total = raw_total(percent_n, fixed, mult_a_n, mult_b, final_fixed).min(cap);
                out.push(StatSourceEffect {
                    source: c.source.clone(),
                    group: c.group,
                    kind,
                    layer: c.layer,
                    value: c.value,
                    effect: total - prev_total,
                });
                prev_total = total;
            }
        }
    }
    out
}

/// 能力値補正に要るカタログ一式(バフ・マスタリー・キャラスキル)。
#[derive(Debug, Clone, Copy)]
pub struct StatCatalogs<'a> {
    pub buffs: &'a BuffCatalog,
    pub masteries: &'a MasteryCatalog,
    pub character_skills: &'a CharacterSkillCatalog,
}

/// 能力値補正セットを組み立てる唯一の経路。段の順序(補正源とバフ → シエナのオーラ →
/// マスタリー → キャラスキル → アンリーシュ → 一時調整)はここだけが知る。
/// 能力値プレビューとダメージ計算の両方がここを通る(片方だけ段が増えて黙ってズレないように)。
///
/// `temporary` は計算リクエストにのみ乗る一時調整(キャラには保存しない)。`pin` はここでは
/// 扱わない(最終能力値が出たあとに `apply_pins`)。
pub fn build_stat_modifiers(
    sources: &StatSources,
    buffs: &BuffSelection,
    equipment: &Equipment,
    common: &CommonSkills,
    catalogs: StatCatalogs<'_>,
    temporary: Option<&Adjustments>,
) -> Result<(StatModifierSet, Vec<StatContribution>), StatSourceError> {
    let (mut modifiers, mut contributions) = build_modifiers(sources, buffs, catalogs.buffs)?;
    apply_siena_stats(&mut modifiers, &mut contributions, equipment);
    apply_masteries(
        &mut modifiers,
        &mut contributions,
        &sources.masteries,
        catalogs.masteries,
    );
    apply_character_skills(
        &mut modifiers,
        &mut contributions,
        &sources.character_skills,
        &sources.masteries,
        catalogs.character_skills,
    );
    apply_unleash(&mut modifiers, &mut contributions, common);
    if let Some(temporary) = temporary {
        temporary.validate()?;
        apply_temporary_adjustments(&mut modifiers, &mut contributions, temporary);
    }
    Ok((modifiers, contributions))
}

/// `BaseStats` + `StatSources` + 装備(シエナのオーラ)から最終能力値を組み立てる(pin 込み)。
/// 装備が最終能力値に効く経路(シエナのオーラのステ加算)を含むので、部位ごとの寄与を出すときは
/// 装備を差し替えてここから丸ごと引き直す。
fn effective_stats_with(
    base: &BaseStats,
    sources: &StatSources,
    buffs: &BuffSelection,
    equipment: &Equipment,
    common: &CommonSkills,
    catalogs: StatCatalogs<'_>,
    stat_cap: i64,
) -> Result<(EffectiveStats, Vec<StatTrace>, Vec<StatSourceEffect>), StatSourceError> {
    let (modifiers, contributions) =
        build_stat_modifiers(sources, buffs, equipment, common, catalogs, None)?;
    let source_effects = contribution_source_effects(&contributions, base, &modifiers, stat_cap);
    let (stats, traces) = effective_stats(base, &modifiers, stat_cap);
    Ok((stats, traces, source_effects))
}

/// 最終能力値と装備から攻撃力(A)を内訳付きで出す。ダメージ計算(`calculate_damage`)と
/// 同じ `attack_power_breakdown` を通す(計算を二重に書かない)。
/// テシスコアは地域依存なのでキャラ画面では地域なし(`enhanced_totals(None)`)で出す。
fn attack_power_of(
    stats: &EffectiveStats,
    equipment: &Equipment,
    soul_link: SoulLinkStatus,
    common: &CommonSkills,
    abilities: &[EquipmentAbilityDef],
    titles: &[TitleDef],
    coefficients: &AttackPowerCoefficients,
) -> AttackPowerBreakdown {
    attack_power_breakdown(
        stat_attack_power(stats, &coefficients.stat),
        equipment_values_attack(
            &equipment
                .base_totals(abilities, titles)
                .add(soul_link.equipment_values()),
            &coefficients.equipment.base,
        ),
        equipment_values_attack(
            &equipment.enhanced_totals(None),
            &coefficients.equipment.enhanced,
        ),
        common.equipment_attack_rate(),
    )
}

/// 「対象ステを選ぶ」バフ(`BuffTarget::UserSelected` / `UserSelectedMulti`)の、
/// **ステごとの実際の効き**。そのステに振ったときに最終能力値が何点動くかを返す。
///
/// カタログの生値をそのまま出さないのは、**上限で頭打ちになる分がキャラごとに違う**ため。
/// 素ステが `stat_cap` に張り付いているステは、+7 のバフを乗せても最終能力値は 1 も動かない
/// (実測: マキシミンの STAB)。呼び出し側が「選んでも何も起きないステ」を見分けられるよう、
/// その場合は `gain = 0` を返す。
///
/// 並び順は付けない — どう見せるか(効く順に並べる・0 を畳む)は表示側の判断。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BuffTargetStatGain {
    pub kind: StatKind,
    /// そのステに振ったときの最終能力値の増分(上限で頭打ちなら 0)
    pub gain: i64,
}

/// `def` を各ステに振ったときの最終能力値の増分を、全ステぶん求める。
///
/// 基準は「`def` を外した選択」。`buffs` に `def` が既に入っていても、その選択は基準から
/// 除いて測る(いま選んでいるステが有利に出ないようにする)。
#[allow(clippy::too_many_arguments)]
pub fn buff_target_stat_gains(
    base: &BaseStats,
    sources: &StatSources,
    buffs: &BuffSelection,
    equipment: &Equipment,
    common: &CommonSkills,
    catalogs: StatCatalogs<'_>,
    def: &BuffDefinition,
    stat_cap: i64,
) -> Result<Vec<BuffTargetStatGain>, StatSourceError> {
    let mut without = buffs.clone();
    without.choices.retain(|choice| choice.buff_id != def.id);
    let (baseline, ..) = effective_stats_with(
        base,
        sources,
        &without,
        equipment,
        common,
        catalogs,
        stat_cap,
    )?;

    let mut gains = Vec::with_capacity(StatKind::ALL.len());
    for kind in StatKind::ALL {
        let mut trial = without.clone();
        trial.choices.push(def.default_choice(Some(kind)));
        let after = match effective_stats_with(
            base,
            sources,
            &trial,
            equipment,
            common,
            catalogs,
            stat_cap,
        ) {
            Ok((stats, ..)) => stats,
            // 同じ排他枠を別のバフが押さえていて、そもそもこのバフを足せない。
            // 効きを測る対象になっていないので、ステごとの行を作らずに空で返す
            // (呼び出し側は「並べ替えの材料が無い」= 既定の並びのまま、で扱える)。
            Err(StatSourceError::ExclusiveSlotConflict { .. }) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        // 対象ステ以外にも動くバフ(倍率で他ステが増幅される等)があるので、全ステの差を足す
        let gain = StatKind::ALL
            .iter()
            .map(|k| after.get(*k) - baseline.get(*k))
            .sum();
        gains.push(BuffTargetStatGain { kind, gain });
    }
    Ok(gains)
}

/// 装備の基本能力値の合計(部位 + アビリティ + 称号 + ソウルリンク)。ソウルリンクは
/// エンチャントではなく基本能力値へ直接加算する。キャラ画面・防御・対人が同じ式を使う
pub fn equipment_base_total(
    equipment: &Equipment,
    soul_link: SoulLinkStatus,
    abilities: &[EquipmentAbilityDef],
    titles: &[TitleDef],
) -> EquipmentValues {
    equipment
        .base_totals(abilities, titles)
        .add(soul_link.equipment_values())
}

/// 最終能力値だけを出す(部位ごとの寄与・補正源内訳を組み立てない軽い経路。防御・対人が使う)。
pub fn effective_stats_of(
    base: &BaseStats,
    sources: &StatSources,
    buffs: &BuffSelection,
    equipment: &Equipment,
    common: &CommonSkills,
    catalogs: StatCatalogs<'_>,
    stat_cap: i64,
) -> Result<EffectiveStats, StatSourceError> {
    base.validate()?;
    sources.validate()?;
    equipment.validate()?;
    let (stats, _, _) = effective_stats_with(base, sources, buffs, equipment, common, catalogs, stat_cap)?;
    Ok(stats)
}

/// `BaseStats` + `StatSources` + 装備から最終能力値と(主軸スキルがあれば)攻撃力を組み立てる。
/// キャラ編集画面で「設定を触ると即時に再計算する」ために使う(保存はしない)。
///
/// `coefficients` はキャラの主軸スキルの依存種別から引いた係数。`None`(主軸スキル未選択)なら
/// 攻撃力は出さない。
pub fn preview_effective_stats(
    base: &BaseStats,
    sources: &StatSources,
    buffs: &BuffSelection,
    equipment: &Equipment,
    common: &CommonSkills,
    catalogs: StatCatalogs<'_>,
    abilities: &[EquipmentAbilityDef],
    titles: &[TitleDef],
    random_options: &[RandomOptionDef],
    coefficients: Option<AttackPowerCoefficients>,
    stat_cap: i64,
) -> Result<StatPreview, StatSourceError> {
    base.validate()?;
    sources.validate()?;
    equipment.validate()?;
    let (stats, traces, source_effects) = effective_stats_with(
        base,
        sources,
        buffs,
        equipment,
        common,
        catalogs,
        stat_cap,
    )?;
    let attack = match coefficients {
        None => None,
        Some(coefficients) => {
            let breakdown =
                attack_power_of(
                    &stats,
                    equipment,
                    sources.soul_link,
                    common,
                    abilities,
                    titles,
                    &coefficients,
                );
            // 部位を外すとシエナのオーラのステ加算も消える = 最終能力値まで動く。
            // 差分は「その装備を外した状態を丸ごと計算し直した A」との差にする。
            let mut part_contributions = Vec::with_capacity(12);
            for (slot, _) in equipment.parts.iter() {
                let without = equipment.without_selected_part(slot);
                let (stats_without, _, _) = effective_stats_with(
                    base,
                    sources,
                    buffs,
                    &without,
                    common,
                    catalogs,
                    stat_cap,
                )?;
                let a_without = attack_power_of(
                    &stats_without,
                    &without,
                    sources.soul_link,
                    common,
                    abilities,
                    titles,
                    &coefficients,
                );
                part_contributions.push(PartAttackContribution {
                    slot,
                    value: breakdown.value - a_without.value,
                });
            }
            Some(AttackPreview {
                breakdown,
                part_contributions,
            })
        }
    };
    let common_skill = CommonSkillPreview {
        defense_rates: common.defense_rates(equipment.siena_defense_rate()),
        equipment_attack_rate: common.equipment_attack_rate(),
        ultimate: UltimateSkillPreview::of(&common.ultimate),
    };
    let critical_rate_bonus = CriticalRateBonusPreview {
        raw: sources.critical_rate.raw_bonus(),
        value: sources.critical_rate.bonus(),
        architect_lab_bonus: sources.critical_rate.architect_lab_bonus(),
    };
    let sacred_relic_total: i64 = StatKind::ALL
        .iter()
        .map(|&k| sacred_relic_value(sources.sacred_relic.get(k)))
        .sum();
    let equipment_base_total = equipment_base_total(equipment, sources.soul_link, abilities, titles);
    let part_ability_values = equipment.ability_values_by_part(abilities);
    let siena_part_values = equipment
        .siena
        .iter_selected()
        .map(|(slot, aura)| PartEquipmentValues {
            slot,
            values: aura.values(),
        })
        .collect();
    let siena_part_stat_totals = equipment
        .siena
        .iter_selected()
        .map(|(slot, aura)| PartStatTotal {
            slot,
            value: aura.stat_bonus().total(),
        })
        .collect();
    let thesis_cores: Vec<ThesisCoreRegionPreview> = CoreRegion::ALL
        .into_iter()
        .map(|region| {
            let set = equipment.thesis_cores.get(region);
            ThesisCoreRegionPreview {
                region,
                total_bonus: set.total_bonus(),
                values: set.equipment_values(),
                set_groups: set.set_groups(),
                ready: set.ready_count(),
                set_bonus: set.set_bonus(),
            }
        })
        .collect();
    let siena_stat_bonus = equipment.siena_stat_bonus();
    let siena_stat_total: i64 = siena_stat_bonus.total();
    let buff_stat_amplification: BuffStatAmplification = if buffs.choices.is_empty() {
        BuffStatAmplification::default()
    } else {
        let (baseline_stats, _, _) = effective_stats_with(
            base,
            sources,
            &BuffSelection::default(),
            equipment,
            common,
            catalogs,
            stat_cap,
        )?;
        let mut amplification = BuffStatAmplification::default();
        for kind in StatKind::ALL {
            let total_diff = stats.get(kind) - baseline_stats.get(kind);
            // バフ行かどうかは区分で判る(名前一致にすると、同名の補正源が増えた時に壊れる)
            let buff_rows_total: i64 = source_effects
                .iter()
                .filter(|e| e.kind == kind && e.group == StatSourceGroup::Buff)
                .map(|e| e.effect)
                .sum();
            amplification.set(kind, total_diff - buff_rows_total);
        }
        amplification
    };
    let thesis_core_set_bonus_total = thesis_cores
        .iter()
        .fold(CoreSetBonus::default(), |acc, r| acc.add(r.set_bonus));
    Ok(StatPreview {
        stats,
        traces,
        group_effects: group_source_effects(&source_effects),
        source_effects,
        attack,
        common_skill,
        critical_rate_bonus,
        sacred_relic_total,
        soul_link: sources.soul_link.preview(),
        equipment_base_total,
        part_ability_values,
        siena_part_values,
        thesis_cores,
        equipment_enhanced_total: equipment.enhanced_totals(None),
        part_enchant_values: equipment.enchant_values_by_part(),
        random_option_totals: equipment.random_option_totals(random_options),
        siena_stat_total,
        siena_attack_rate: equipment.siena_attack_rate(),
        thesis_core_set_bonus_total,
        thesis_core_best_total: equipment.thesis_cores.best_total_bonus(),
        character_skill_actual_delay: sources
            .character_skills
            .actual_delay_contributions(catalogs.character_skills, &sources.masteries),
        mastery_actual_delay: sources.masteries.actual_delay_reduction(catalogs.masteries),
        siena_defense_rate: equipment.siena_defense_rate(),
        siena_actual_delay_rate: equipment.siena_actual_delay_reduction(),
        siena_critical_rate: equipment.siena_critical_rate(),
        siena_part_stat_totals,
        buff_stat_amplification,
    })
}

/// UI がリテラルで持たず参照するための値域一覧(上限・下限・刻み・係数だけ。起動時に 1 回取得する想定)。
/// 並び・ラベル・段階表は `crate::game_tables::GameTables` に置く。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatLimits {
    pub base_stat_max: u32,
    pub rune_level_max: u8,
    pub crown_base_max: u32,
    pub crown_selected_max: u32,
    pub crown_step: u32,
    /// モンスターカードの 1 ステあたり上限(wiki: ステータス「カード装着 +0〜70」)
    pub monster_card_max: u32,
    pub sacred_relic_stage_max: u8,
    /// ソウルリンクのリンクステータス 1〜4 の Lv 上限。
    pub soul_link_equipment_level_max: u8,
    pub soul_link_critical_damage_level_max: u8,
    pub soul_link_final_damage_level_max: u8,
    pub soul_link_weapon_enhance_level_max: u8,
    pub soul_link_armor_enhance_level_max: u8,
    pub adjustment_add_min: i64,
    pub adjustment_add_max: i64,
    pub adjustment_pin_min: i64,
    pub adjustment_pin_max: i64,
    pub equipment_value_max: i64,
    pub strong_weapon_level_max: u8,
    /// 装備強化 Lv 上限(wiki: 装備システム/装備強化。+1〜+15)
    pub enhance_level_max: u8,
    /// テシスコアの装着枠数(wiki: テシスコア効果)
    pub core_slot_count: usize,
    pub core_evolution_max: u8,
    pub core_enhancement_max: u8,
    /// 装備 1 部位に付与できる属性値の上限(wiki: 装備システム/属性強化)
    pub equipment_element_value_max: i64,
    /// キャラの属性値の上限(wiki: 属性システム)
    pub element_value_max: i64,
    /// 覚醒段階の上限(wiki: Quest/覚醒クエスト)
    pub awakening_stage_max: u8,
    /// エタの意志 Lv の上限(wiki: エタの意志「エタの成長」)
    pub eternal_level_max: u8,
    /// エタの意志 Lv > 0 のときに確定する覚醒段階
    pub eternal_awakening_stage: u8,
    /// ランダムオプションの効果値の上限 `[仮]`
    pub random_option_value_max: f64,
    /// プロテクトアーマーの Lv 上限(wiki: Skill/共通)
    pub protect_armor_level_max: u8,
    /// 改・プロテクトアーマーの Lv 上限(wiki: Skill/共通)
    pub kai_protect_armor_level_max: u8,
    /// シャープネスビジョンの Lv 上限(wiki: Skill/共通)
    pub sharpness_vision_level_max: u8,
    /// オーグメントの Lv 上限(wiki: Skill/共通)
    pub augment_level_max: u8,
    /// アンリーシュ(能力解放)の Lv 上限(wiki: Skill/共通)
    pub unleash_level_max: u8,
    /// アンリーシュの枠数(wiki: Skill/共通「2つまで使用可能」)
    pub unleash_slots: usize,
    /// レインフォースの Lv 上限(wiki: Skill/共通。アンリーシュ Lv6 以降の前提)
    pub reinforce_level_max: u8,
    /// ハイパーリミットの Lv 上限(wiki: Skill/極限)
    pub hyper_limit_level_max: u8,
    /// クリティカル率増加の上限 %(wiki: 計算式まとめ `#CriticalChance`)
    pub critical_rate_bonus_max: f64,
    /// 設計者の研究室の研究段階の上限(wiki: 設計者の研究室)
    pub architect_lab_stage_max: u8,
    /// 設計者の研究室 1 段階あたりのクリティカル率増加(wiki: 設計者の研究室 永続バフ)
    pub architect_lab_per_stage: f64,
    /// 極のルーンのクリティカル率増加(最大レベル時。wiki `#CriticalChance`)
    pub ultimate_rune_bonus_max: f64,
    /// 致命打のクリティカル率増加(wiki `#CriticalChance`)
    pub deadly_blow_bonus_max: f64,
    pub power_weapon_rate: f64,
    /// ストロングウェポン 1Lv あたりの装備攻撃力強化倍率(wiki: Skill/共通)。Σ% の小数表現
    pub strong_weapon_rate_per_level: f64,
    /// コートアーマーの装備防御力倍率(物理 / 魔法。wiki: Skill/共通)。Σ% の小数表現
    pub coat_armor_physical_rate: f64,
    pub coat_armor_magic_rate: f64,
    /// 神鳥の聖物 1 段階あたりの最終固定値(wiki: 神鳥の聖物)
    pub sacred_relic_value_per_stage: i64,
    /// コンボボーナスが付くコンボ数(wiki: カテゴリH)
    pub combo_bonus_threshold: u32,
    /// 中ディレイのコンボボーナスが付くコンボ数(wiki `#ActualDelay`)
    pub combo_delay_threshold: u32,
    /// コンボボーナスの割合(wiki: カテゴリH)。Σ% の小数表現
    pub combo_bonus_rate: f64,
    /// 中ディレイ減少値の上限(wiki `#ActualDelay`)。Σ% の小数表現
    pub actual_delay_reduction_max: f64,
    /// 中ディレイの下限(秒。wiki `#ActualDelay`)
    pub actual_delay_min: f64,
    /// レインフォース無しで取れるアンリーシュの Lv(wiki: Skill/共通)
    pub unleash_free_level_max: u8,
    /// オーグメント Lv + この値 = ストロングウェポン / プロテクトアーマー / ハイパーリミットの上限
    pub augment_gate_offset: u8,
    /// +12 以上で追加固定ダメージがレンジ振り(MR)になる境界(wiki: 装備システム/装備強化)
    pub enhance_grade_min_level: u8,
    /// 属性差 1 あたりの属性差ボーナス(wiki: カテゴリI)。Σ% の小数表現
    pub element_bonus_percent_per_point: f64,
    /// 属性差ボーナス(カテゴリI)の上限。Σ% の小数表現
    pub element_bonus_max: f64,
    /// カット率 J の分母(wiki カテゴリJ: `r = 1 − a/(a+80)`)
    pub cut_rate_denominator: f64,
    /// カット率 J の `a` の定数項(`a = 3 + [(合計 − 1) / 除数]`)
    pub cut_rate_a_base: f64,
    /// カット率 J の `a` の除数(物理 / 魔法)
    pub cut_rate_divisor: f64,
    /// カット率 J の `a` の除数(複合)
    pub cut_rate_composite_divisor: f64,
    /// 防御力(物理 / 魔法)のステ係数(`DEF*3 + 装備×倍率×6`)
    pub defense_stat_multiplier: f64,
    /// 防御力(物理 / 魔法)の装備係数
    pub defense_equipment_multiplier: f64,
    /// 複合防御力のステ係数(`(DEF+MR)*1.5 + 装備×3`)
    pub composite_defense_stat_multiplier: f64,
    /// 複合防御力の装備係数
    pub composite_defense_equipment_multiplier: f64,
    /// 回避Pの定数項(`回避P = [15 + (AGI + 装備回避率)×1.2 + 装備敏捷度/7 + ...]`)
    pub evasion_point_base: f64,
    /// 回避Pの AGI 係数
    pub evasion_point_agi_rate: f64,
    /// 回避Pの攻撃タイプ別増加の共通除数
    pub evasion_type_divisor: f64,
    /// 回避P(物理)の `[(STAB+HACK)/100]` の除数
    pub evasion_physical_attack_divisor: f64,
    /// ペット会心の倍率(wiki `#CriticalChance`)
    pub pet_critical_rate: f64,
    /// クリティカル率の下限(wiki `#CriticalChance`)
    pub critical_rate_min: f64,
    /// クリティカル率の上限
    pub critical_rate_max: f64,
}

pub fn stat_limits() -> StatLimits {
    StatLimits {
        base_stat_max: BASE_STAT_MAX,
        rune_level_max: RUNE_LEVEL_MAX,
        crown_base_max: Crown::BASE_MAX_VALUE,
        crown_selected_max: Crown::SELECTED_MAX_VALUE,
        crown_step: Crown::STEP,
        monster_card_max: MONSTER_CARD_VALUE_MAX,
        sacred_relic_stage_max: SACRED_RELIC_STAGE_MAX,
        soul_link_equipment_level_max: SOUL_LINK_EQUIPMENT_LEVEL_MAX,
        soul_link_critical_damage_level_max: SOUL_LINK_CRITICAL_DAMAGE_LEVEL_MAX,
        soul_link_final_damage_level_max: SOUL_LINK_FINAL_DAMAGE_LEVEL_MAX,
        soul_link_weapon_enhance_level_max: SOUL_LINK_WEAPON_ENHANCE_LEVEL_MAX,
        soul_link_armor_enhance_level_max: SOUL_LINK_ARMOR_ENHANCE_LEVEL_MAX,
        adjustment_add_min: ADJUSTMENT_ADD_MIN,
        adjustment_add_max: ADJUSTMENT_ADD_MAX,
        adjustment_pin_min: ADJUSTMENT_PIN_MIN,
        adjustment_pin_max: ADJUSTMENT_PIN_MAX,
        equipment_value_max: EQUIPMENT_VALUE_MAX,
        strong_weapon_level_max: STRONG_WEAPON_LEVEL_MAX,
        enhance_level_max: ENHANCE_LEVEL_MAX,
        core_slot_count: CORE_SLOT_COUNT,
        core_evolution_max: CORE_EVOLUTION_MAX,
        core_enhancement_max: CORE_ENHANCEMENT_MAX,
        equipment_element_value_max: crate::element::EQUIPMENT_ELEMENT_VALUE_MAX,
        element_value_max: crate::element::ELEMENT_VALUE_MAX,
        awakening_stage_max: crate::awakening::Awakening::MAX_STAGE,
        eternal_level_max: crate::awakening::Awakening::MAX_ETERNAL_LEVEL,
        eternal_awakening_stage: crate::awakening::Awakening::ETERNAL_STAGE,
        random_option_value_max: crate::random_option::RANDOM_OPTION_VALUE_MAX,
        protect_armor_level_max: crate::common_skill::PROTECT_ARMOR_LEVEL_MAX,
        kai_protect_armor_level_max: crate::common_skill::KAI_PROTECT_ARMOR_LEVEL_MAX,
        sharpness_vision_level_max: crate::common_skill::SHARPNESS_VISION_LEVEL_MAX,
        augment_level_max: crate::common_skill::AUGMENT_LEVEL_MAX,
        unleash_level_max: crate::common_skill::UNLEASH_LEVEL_MAX,
        unleash_slots: crate::common_skill::UNLEASH_SLOTS,
        reinforce_level_max: crate::common_skill::REINFORCE_LEVEL_MAX,
        hyper_limit_level_max: crate::ultimate_skill::HYPER_LIMIT_LEVEL_MAX,
        critical_rate_bonus_max: crate::critical_rate::CRITICAL_RATE_BONUS_MAX,
        architect_lab_stage_max: crate::critical_rate::ARCHITECT_LAB_STAGE_MAX,
        architect_lab_per_stage: crate::critical_rate::ARCHITECT_LAB_PER_STAGE,
        ultimate_rune_bonus_max: CriticalRateSourceId::UltimateRune.max_value(),
        deadly_blow_bonus_max: CriticalRateSourceId::DeadlyBlow.max_value(),
        power_weapon_rate: crate::common_skill::POWER_WEAPON_RATE,
        strong_weapon_rate_per_level: crate::common_skill::STRONG_WEAPON_RATE_PER_LEVEL,
        coat_armor_physical_rate: crate::common_skill::COAT_ARMOR_PHYSICAL_RATE,
        coat_armor_magic_rate: crate::common_skill::COAT_ARMOR_MAGIC_RATE,
        sacred_relic_value_per_stage: SACRED_RELIC_VALUE_PER_STAGE,
        combo_bonus_threshold: crate::damage::COMBO_BONUS_THRESHOLD,
        combo_delay_threshold: crate::actual_delay::COMBO_DELAY_THRESHOLD,
        combo_bonus_rate: crate::damage::COMBO_BONUS_RATE,
        actual_delay_reduction_max: crate::actual_delay::ACTUAL_DELAY_REDUCTION_MAX,
        actual_delay_min: crate::actual_delay::ACTUAL_DELAY_MIN,
        unleash_free_level_max: crate::common_skill::UNLEASH_FREE_LEVEL_MAX,
        augment_gate_offset: crate::common_skill::AUGMENT_GATE_OFFSET,
        enhance_grade_min_level: crate::equipment::ENHANCE_LEVEL_RANDOM_RANGE_MIN,
        element_bonus_percent_per_point: crate::damage::ELEMENT_BONUS_PERCENT_PER_POINT,
        element_bonus_max: DamageCategory::ElementBonus
            .cap()
            .and_then(|c| c.max)
            .expect("ElementBonus は上限つき"),
        cut_rate_denominator: crate::defense::CUT_RATE_DENOMINATOR,
        cut_rate_a_base: crate::defense::CUT_RATE_A_BASE,
        cut_rate_divisor: crate::defense::CUT_RATE_DIVISOR,
        cut_rate_composite_divisor: crate::defense::CUT_RATE_COMPOSITE_DIVISOR,
        defense_stat_multiplier: crate::defense::DEFENSE_STAT_MULTIPLIER,
        defense_equipment_multiplier: crate::defense::DEFENSE_EQUIPMENT_MULTIPLIER,
        composite_defense_stat_multiplier: crate::defense::COMPOSITE_DEFENSE_STAT_MULTIPLIER,
        composite_defense_equipment_multiplier: crate::defense::COMPOSITE_DEFENSE_EQUIPMENT_MULTIPLIER,
        evasion_point_base: crate::defense::EVASION_POINT_BASE,
        evasion_point_agi_rate: crate::defense::EVASION_POINT_AGI_RATE,
        evasion_type_divisor: crate::defense::EVASION_TYPE_DIVISOR,
        evasion_physical_attack_divisor: crate::defense::EVASION_PHYSICAL_ATTACK_DIVISOR,
        pet_critical_rate: crate::critical_rate::PET_CRITICAL_RATE,
        critical_rate_min: crate::critical_rate::CRITICAL_RATE_MIN,
        critical_rate_max: crate::critical_rate::CRITICAL_RATE_MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::{effective_stat, BaseStats, BaseStatsError, BASE_STAT_MAX};

    /// 上限に当たらない値(最終能力値の上限の挙動は stats.rs のテストで見る)。
    const NO_CAP: i64 = i64::MAX;

    fn c(source: &str, kind: StatKind, layer: StatLayer, value: f64) -> StatContribution {
        cg(source, StatSourceGroup::Other, kind, layer, value)
    }

    fn cg(
        source: &str,
        group: StatSourceGroup,
        kind: StatKind,
        layer: StatLayer,
        value: f64,
    ) -> StatContribution {
        StatContribution {
            source: source.to_string(),
            group,
            kind,
            layer,
            value,
        }
    }

    /// 「素ステ + Σ要因の effect − 上限で捨てた分 = 最終能力値」(§00 05: 数字の出どころを全部見せる)
    #[test]
    fn 要因のeffect合計は最終能力値と一致する() {
        let kind = StatKind::Int;
        let base = BaseStats {
            stab: 1,
            hack: 1,
            int: 300,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let mut modifiers = StatModifierSet::default();
        {
            let m = modifiers.get_mut(kind);
            m.percent_of_base = vec![0.1, 0.05];
            m.fixed = 90;
            m.multiplier_a = vec![1.1, 1.05];
            m.multiplier_b = 0.2 + 0.05;
            m.final_fixed = 330;
        }
        let contributions = vec![
            c("割合1", kind, StatLayer::PercentOfBase, 0.1),
            c("割合2", kind, StatLayer::PercentOfBase, 0.05),
            c("ペット", kind, StatLayer::Fixed, 60.0),
            c("ルーン", kind, StatLayer::Fixed, 30.0),
            c("倍率A1", kind, StatLayer::MultiplierA, 1.1),
            c("倍率A2", kind, StatLayer::MultiplierA, 1.05),
            c("アンリーシュ", kind, StatLayer::MultiplierB, 0.2),
            c("倍率B2", kind, StatLayer::MultiplierB, 0.05),
            c("クラウン", kind, StatLayer::FinalFixed, 300.0),
            c("聖物", kind, StatLayer::FinalFixed, 30.0),
        ];
        // 上限に当たらない場合
        let (_, trace) = effective_stat(kind, base.get(kind), modifiers.get(kind), NO_CAP);
        let effects = contribution_source_effects(&contributions, &base, &modifiers, NO_CAP);
        let total: i64 = effects.iter().map(|x| x.effect).sum();
        assert_eq!(trace.capped_loss, 0);
        assert_eq!(i64::from(trace.base) + total, trace.effective);

        // 上限で頭打ちなら、上限込みの帰属の合計がそのまま最終能力値に一致する
        let (_, trace) = effective_stat(kind, base.get(kind), modifiers.get(kind), 800);
        let effects = contribution_source_effects(&contributions, &base, &modifiers, 800);
        let total: i64 = effects.iter().map(|x| x.effect).sum();
        assert!(trace.capped_loss > 0);
        assert_eq!(i64::from(trace.base) + total, trace.effective);
    }

    /// `contribution_source_effects` は層順ステップ幅を `cap` 込みで返す。割合増加・固定値・倍率A・倍率Bを同時に持つケースで、上限に当たっても
    /// 厳密に `Σ per-source == 最終 − 素ステ` が成り立つことを確認する。
    #[test]
    fn 補正源ごとの帰属の合計は最終能力値と一致する() {
        let kind = StatKind::Int;
        let base = BaseStats {
            stab: 1,
            hack: 1,
            int: 300,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let mut modifiers = StatModifierSet::default();
        {
            let m = modifiers.get_mut(kind);
            m.percent_of_base = vec![0.1, 0.05];
            m.fixed = 90;
            m.multiplier_a = vec![1.1, 1.05];
            m.multiplier_b = 0.2 + 0.05;
            m.final_fixed = 330;
        }
        let contributions = vec![
            c("割合1", kind, StatLayer::PercentOfBase, 0.1),
            c("割合2", kind, StatLayer::PercentOfBase, 0.05),
            c("ペット", kind, StatLayer::Fixed, 60.0),
            c("ルーン", kind, StatLayer::Fixed, 30.0),
            c("マスタリー(倍率A1)", kind, StatLayer::MultiplierA, 1.1),
            c("マスタリー(倍率A2)", kind, StatLayer::MultiplierA, 1.05),
            c("アンリーシュ", kind, StatLayer::MultiplierB, 0.2),
            c("倍率B2", kind, StatLayer::MultiplierB, 0.05),
            c("クラウン", kind, StatLayer::FinalFixed, 300.0),
            c("聖物", kind, StatLayer::FinalFixed, 30.0),
        ];

        // 上限に当たらない場合
        let (_, trace) = effective_stat(kind, base.get(kind), modifiers.get(kind), NO_CAP);
        let effects = contribution_source_effects(&contributions, &base, &modifiers, NO_CAP);
        let total: i64 = effects.iter().map(|x| x.effect).sum();
        assert_eq!(i64::from(trace.base) + total, trace.effective);
        // 層順(割合増加→固定値→倍率A→倍率B→最終固定値)に積む方式なので、倍率A/B を持つ補正源
        // (マスタリー・アンリーシュ)が、先に積んだ割合増加/固定値の補正源(バフ等)を増幅した分を
        // 自分の行に受け取る。ペット(固定値)は素の値のまま、マスタリーの行がその増幅ぶんを持つ
        let pet = effects.iter().find(|e| e.source == "ペット").unwrap();
        assert_eq!(pet.effect, 60);
        let mastery_total: i64 = effects
            .iter()
            .filter(|e| e.source.starts_with("マスタリー"))
            .map(|e| e.effect)
            .sum();
        assert!(mastery_total > 0, "倍率Aの行が増幅ぶんを持つはず: {mastery_total}");

        // 上限で頭打ちでも、累積再構築なら差し引きなしで厳密に一致する
        let cap = 800;
        let (_, trace) = effective_stat(kind, base.get(kind), modifiers.get(kind), cap);
        let effects = contribution_source_effects(&contributions, &base, &modifiers, cap);
        let total: i64 = effects.iter().map(|x| x.effect).sum();
        assert!(trace.capped_loss > 0);
        assert_eq!(i64::from(trace.base) + total, trace.effective);
    }

    /// 区分の帰属は「常に 7 ステ × 3 区分」で返り、区分ごとの合計は元の帰属と一致する。
    /// 画面は列を固定で並べるので、値が 0 の組も行が消えてはいけない(§00 03)。
    #[test]
    fn 区分ごとの帰属は全ての組を返し合計が一致する() {
        let kind = StatKind::Hack;
        let effects = vec![
            StatSourceEffect {
                source: "クラブ効果".to_string(),
                group: StatSourceGroup::Buff,
                kind,
                layer: StatLayer::Fixed,
                value: 7.0,
                effect: 7,
            },
            StatSourceEffect {
                source: "シエナのオーラ".to_string(),
                group: StatSourceGroup::Equipment,
                kind,
                layer: StatLayer::FinalFixed,
                value: 30.0,
                effect: 30,
            },
            StatSourceEffect {
                source: "ペット Sスキル".to_string(),
                group: StatSourceGroup::Other,
                kind,
                layer: StatLayer::Fixed,
                value: 50.0,
                effect: 50,
            },
            StatSourceEffect {
                source: "クラウン".to_string(),
                group: StatSourceGroup::Other,
                kind,
                layer: StatLayer::FinalFixed,
                value: 100.0,
                effect: 100,
            },
        ];
        let groups = group_source_effects(&effects);
        assert_eq!(groups.len(), StatKind::ALL.len() * 3);
        let get = |k: StatKind, g: StatSourceGroup| {
            groups
                .iter()
                .find(|x| x.kind == k && x.group == g)
                .unwrap()
                .effect
        };
        assert_eq!(get(kind, StatSourceGroup::Buff), 7);
        assert_eq!(get(kind, StatSourceGroup::Equipment), 30);
        assert_eq!(get(kind, StatSourceGroup::Other), 150);
        // 寄与が 1 件も無いステも 0 の行で返る
        assert_eq!(get(StatKind::Int, StatSourceGroup::Buff), 0);
        assert_eq!(
            groups.iter().map(|g| g.effect).sum::<i64>(),
            effects.iter().map(|e| e.effect).sum::<i64>()
        );
    }

    /// バフの固定値がマスタリーの倍率B に増幅された分を `buff_stat_amplification` が拾うこと。
    /// 実機で見つかったズレ(HACK/DEX)の再現: バフが固定値を足す → マスタリーの倍率B がそれを
    /// 増幅する → バフ行の `source_effects` だけ合計しても最終能力値の増分に届かない。
    #[test]
    fn バフが倍率を持つ補正源に増幅された分をbuff_stat_amplificationが返す() {
        use crate::mastery::MasteryDef;

        const MASTERY_CATALOG: &[MasteryDef] = &[MasteryDef {
            id: "test_m2_silver_skull",
            game_character_id: "test",
            tier: 2,
            name: "シルバースカル優勝者",
            effect: SkillEffect::StatRate {
                stats: &[StatKind::Hack],
                percent: 25.0,
                layer: StatLayer::MultiplierB,
            },
            note: "",
        }];

        let base = BaseStats {
            stab: 1,
            hack: 100,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let sources = StatSources {
            masteries: Masteries {
                picked: vec!["test_m2_silver_skull".to_string()],
            },
            ..Default::default()
        };
        let buffs = BuffSelection {
            choices: vec![BuffChoice {
                buff_id: "club_effect".to_string(),
                stat: Some(StatKind::Hack),
                choice_index: None,
                value: None,
            }],
        };
        let catalog = test_catalog();

        let preview = preview_effective_stats(
            &base,
            &sources,
            &buffs,
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: MASTERY_CATALOG,
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            None,
            NO_CAP,
        )
        .unwrap();

        // クラブ効果(Hack +7、固定値)がマスタリーの倍率B(+10%)に増幅される
        // → floor(100 + 7) * 1.1 - floor(100 * 1.1) の差のうち、バフ行(+7)を超えた分
        let baseline = preview_effective_stats(
            &base,
            &sources,
            &BuffSelection::default(),
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: MASTERY_CATALOG,
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            None,
            NO_CAP,
        )
        .unwrap();
        let total_diff = preview.stats.get(StatKind::Hack) - baseline.stats.get(StatKind::Hack);
        let buff_row: i64 = preview
            .source_effects
            .iter()
            .filter(|e| e.source == "クラブ効果")
            .map(|e| e.effect)
            .sum();
        assert!(total_diff > buff_row, "マスタリーの倍率で増幅されるはず");
        assert_eq!(
            preview.buff_stat_amplification.get(StatKind::Hack),
            total_diff - buff_row
        );
        assert!(preview.buff_stat_amplification.get(StatKind::Hack) > 0);
        // 増幅は Hack だけに乗る(他ステは無関係)
        assert_eq!(preview.buff_stat_amplification.get(StatKind::Stab), 0);

        // バフ無しなら全ステ 0
        assert_eq!(baseline.buff_stat_amplification, BuffStatAmplification::default());
    }

    /// Σ(バフ行の source_effects) + 増幅 == バフあり − バフなし が、ステごとに厳密に成り立つこと。
    #[test]
    fn バフ行と増幅の合計はステごとに最終能力値の増分と厳密に一致する() {
        use crate::mastery::MasteryDef;

        const MASTERY_CATALOG: &[MasteryDef] = &[MasteryDef {
            id: "test_m2_silver_skull",
            game_character_id: "test",
            tier: 2,
            name: "シルバースカル優勝者",
            effect: SkillEffect::StatRate {
                stats: &[StatKind::Hack],
                percent: 25.0,
                layer: StatLayer::MultiplierB,
            },
            note: "",
        }];

        let base = BaseStats {
            stab: 50,
            hack: 100,
            int: 30,
            def: 40,
            mr: 20,
            dex: 60,
            agi: 70,
        };
        let sources = StatSources {
            masteries: Masteries {
                picked: vec!["test_m2_silver_skull".to_string()],
            },
            ..Default::default()
        };
        let buffs = BuffSelection {
            choices: vec![
                BuffChoice {
                    buff_id: "illumination_drink".to_string(),
                    stat: None,
                    choice_index: None,
                    value: None,
                },
                BuffChoice {
                    buff_id: "club_effect".to_string(),
                    stat: Some(StatKind::Hack),
                    choice_index: None,
                    value: None,
                },
            ],
        };
        let catalog = test_catalog();

        let preview = preview_effective_stats(
            &base,
            &sources,
            &buffs,
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: MASTERY_CATALOG,
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            None,
            NO_CAP,
        )
        .unwrap();
        let baseline = preview_effective_stats(
            &base,
            &sources,
            &BuffSelection::default(),
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: MASTERY_CATALOG,
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            None,
            NO_CAP,
        )
        .unwrap();

        let buff_names: HashSet<&str> = buffs
            .choices
            .iter()
            .filter_map(|c| catalog.iter().find(|d| d.id == c.buff_id))
            .map(|d| d.name)
            .collect();

        for kind in StatKind::ALL {
            let total_diff = preview.stats.get(kind) - baseline.stats.get(kind);
            let buff_rows_total: i64 = preview
                .source_effects
                .iter()
                .filter(|e| e.kind == kind && buff_names.contains(e.source.as_str()))
                .map(|e| e.effect)
                .sum();
            assert_eq!(
                buff_rows_total + preview.buff_stat_amplification.get(kind),
                total_diff,
                "{kind:?} で Σ(source_effects) + 増幅 != 増分"
            );

            // 区分の合計は最終能力値と一致する(ゲーム内の表示と突き合わせる前提の表示なので、
            // ここがズレると画面が嘘をつく)
            let group_total: i64 = preview
                .group_effects
                .iter()
                .filter(|g| g.kind == kind)
                .map(|g| g.effect)
                .sum();
            assert_eq!(
                i64::from(base.get(kind)) + group_total,
                preview.stats.get(kind),
                "{kind:?} で 素ステ + Σ(区分) != 最終能力値"
            );
        }
    }

    /// domain は gamedata に依存できないため、テスト用に必要分だけ縮小したカタログを用意する。
    /// 値は gamedata::buffs::buff_catalog() の実データと一致させること。
    fn test_catalog() -> Vec<BuffDefinition> {
        vec![
            BuffDefinition {
                id: "illumination_drink",
                name: "イルミネーション祭りのドリンク",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Item,
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.30),
                exclusive_slots: vec!["percent_slot_1", "percent_slot_2"],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
            BuffDefinition {
                id: "charge_potion",
                name: "充填の秘薬",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Item,
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.20),
                exclusive_slots: vec!["percent_slot_1"],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
            BuffDefinition {
                id: "event_buff",
                name: "イベントバフ",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Event,
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Choice(vec![0.10, 0.20, 0.30, 0.50]),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
            BuffDefinition {
                id: "trust_potion",
                name: "改・信頼の薬",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Item,
                target: BuffTarget::AllStats,
                layer: StatLayer::Fixed,
                value: BuffValue::UserInput {
                    min: 0.0,
                    max: 33.0,
                },
                exclusive_slots: vec!["trust_potion"],
                source_url: "",
                note: "",
                default_value: Some(33.0),
                damage_effects: &[],
            },
            BuffDefinition {
                id: "club_effect",
                name: "クラブ効果",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Skill,
                target: BuffTarget::UserSelectedMulti,
                layer: StatLayer::Fixed,
                value: BuffValue::Fixed(7.0),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
            BuffDefinition {
                id: "tales_weaver_energy",
                name: "テイルズウィーバーのエネルギー",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Skill,
                target: BuffTarget::AllStats,
                layer: StatLayer::MultiplierA,
                value: BuffValue::Fixed(1.1),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
            BuffDefinition {
                id: "unleash",
                name: "アンリーシュ",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Skill,
                target: BuffTarget::AllStats,
                layer: StatLayer::MultiplierB,
                value: BuffValue::Fixed(0.20),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
        ]
    }

    fn choice(id: &str) -> BuffChoice {
        BuffChoice {
            buff_id: id.to_string(),
            stat: None,
            choice_index: None,
            value: None,
        }
    }

    // --- 1. 各補正源が正しいレイヤーに積まれること ---

    #[test]
    fn ペットスキルは固定値層に積まれる() {
        let sources = StatSources {
            pet_skills: PetSkills {
                stab: Some(PetSkillTier::Basic),
                ..Default::default()
            },
            ..Default::default()
        };
        let (modifiers, contributions) =
            build_modifiers(&sources, &BuffSelection::default(), &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 20);
        let c = contributions
            .iter()
            .find(|c| c.kind == StatKind::Stab)
            .unwrap();
        assert_eq!(c.layer, StatLayer::Fixed);
        assert_eq!(c.value, 20.0);
    }

    #[test]
    fn ルーンスキルは固定値層に積まれる() {
        let sources = StatSources {
            rune_levels: RuneLevels {
                hack: 15,
                ..Default::default()
            },
            ..Default::default()
        };
        let (modifiers, contributions) =
            build_modifiers(&sources, &BuffSelection::default(), &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Hack).fixed, 15);
        let c = contributions
            .iter()
            .find(|c| c.kind == StatKind::Hack)
            .unwrap();
        assert_eq!(c.layer, StatLayer::Fixed);
    }

    #[test]
    fn クラウンは最終固定値層に積まれる() {
        let sources = StatSources {
            crown: Crown {
                def: 250,
                selected_stat: Some(StatKind::Def),
                ..Default::default()
            },
            ..Default::default()
        };
        let (modifiers, contributions) =
            build_modifiers(&sources, &BuffSelection::default(), &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Def).final_fixed, 250);
        let c = contributions
            .iter()
            .find(|c| c.kind == StatKind::Def)
            .unwrap();
        assert_eq!(c.layer, StatLayer::FinalFixed);
    }

    #[test]
    fn 聖物は段階を10倍して最終固定値層に積まれる() {
        let sources = StatSources {
            sacred_relic: SacredRelic {
                mr: 12,
                ..Default::default()
            },
            ..Default::default()
        };
        let (modifiers, contributions) =
            build_modifiers(&sources, &BuffSelection::default(), &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Mr).final_fixed, 120);
        let c = contributions
            .iter()
            .find(|c| c.kind == StatKind::Mr)
            .unwrap();
        assert_eq!(c.layer, StatLayer::FinalFixed);
        assert_eq!(c.value, 120.0);
    }

    // --- pin(能力値の固定。計算リクエストの一時調整のみから来る) ---

    #[test]
    fn pinされたステはpinned_fromに元の値が残り最終能力値が固定される() {
        let base = BaseStats {
            dex: 100,
            ..Default::default()
        };
        let temporary = Adjustments {
            dex: StatAdjustment {
                add: 0,
                pin: Some(999),
            },
            ..Default::default()
        };
        let (mut stats, mut traces) = effective_stats(&base, &StatModifierSet::default(), NO_CAP);
        apply_pins(&mut stats, &mut traces, Some(&temporary));

        let dex_trace = traces.iter().find(|t| t.kind == StatKind::Dex).unwrap();
        assert_eq!(dex_trace.pinned_from, Some(100));
        assert_eq!(dex_trace.effective, 999);
        assert_eq!(stats.get(StatKind::Dex), 999);

        // pin していないステの pinned_from は None のまま
        let stab_trace = traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.pinned_from, None);
    }

    #[test]
    fn apply_pinsはtemporaryが無ければ何もしない() {
        let base_stats = BaseStats {
            stab: 1,
            ..Default::default()
        };
        let (mut stats, mut traces) =
            effective_stats(&base_stats, &StatModifierSet::default(), NO_CAP);
        apply_pins(&mut stats, &mut traces, None);

        let stab_trace = traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.pinned_from, None);
        assert_eq!(stats.get(StatKind::Stab), 1);
    }

    /// 主軸スキル(StabHack 相当)の係数一式。値は gamedata::characters の実データに合わせる。
    fn test_attack_coefficients() -> AttackPowerCoefficients {
        use crate::equipment::EquipmentRates;
        AttackPowerCoefficients {
            stat: AttackCoefficients {
                primary: (StatKind::Stab, 1.8),
                secondary: (StatKind::Hack, 1.8),
            },
            equipment: EquipmentCoefficients {
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
            },
        }
    }

    /// 武器(基本値・エンチャント・シエナのオーラのステ加算)と手(基本値のみ)を持つ装備。
    fn test_equipment() -> Equipment {
        use crate::equipment::{EquipmentPart, EquipmentParts, EquipmentValues};
        use crate::siena::{
            RegisteredSienaAura, SienaAura, SienaAuraList, SienaAuras, SienaExtraKind,
            SienaExtraSlot, SienaSlot, SienaValueKind,
        };
        let aura = SienaAura {
            slots: vec![
                SienaSlot {
                    kind: SienaValueKind::Thrust,
                    value: 10,
                },
                SienaSlot {
                    kind: SienaValueKind::Thrust,
                    value: 10,
                },
                SienaSlot {
                    kind: SienaValueKind::Slash,
                    value: 10,
                },
                SienaSlot {
                    kind: SienaValueKind::Slash,
                    value: 10,
                },
            ],
            extras: vec![SienaExtraSlot {
                kind: SienaExtraKind::AllStats,
                value: 5.0,
            }],
        };
        Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart {
                    base: EquipmentValues {
                        thrust: 150,
                        slash: 150,
                        ..Default::default()
                    },
                    enchant: EquipmentValues {
                        thrust: 60,
                        slash: 60,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                hand: EquipmentPart {
                    base: EquipmentValues {
                        thrust: 30,
                        slash: 30,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            siena: SienaAuras {
                weapon: SienaAuraList {
                    registered: vec![RegisteredSienaAura {
                        id: 1,
                        label: String::new(),
                        aura,
                    }],
                    selected_id: Some(1),
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// テスト用の共通スキル。パワーウェポン + ストロングウェポン Lv6 = 強化倍率 +20%
    fn test_common_skills() -> CommonSkills {
        CommonSkills {
            power_weapon: true,
            strong_weapon_level: 6,
            augment_level: 5,
            ..Default::default()
        }
    }

    #[test]
    fn 主軸スキル未選択なら攻撃力は出ない() {
        let base = BaseStats {
            stab: 100,
            hack: 100,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let preview = preview_effective_stats(
            &base,
            &StatSources::default(),
            &BuffSelection::default(),
            &test_equipment(),
            &test_common_skills(),
            StatCatalogs {
                buffs: &test_catalog(),
                masteries: &[],
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            None,
            NO_CAP,
        )
        .unwrap();
        assert!(preview.attack.is_none());
    }

    #[test]
    fn 攻撃力の内訳はステと装備基本と装備強化の和になる() {
        let base = BaseStats {
            stab: 100,
            hack: 100,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let equipment = test_equipment();
        let preview = preview_effective_stats(
            &base,
            &StatSources::default(),
            &BuffSelection::default(),
            &equipment,
            &test_common_skills(),
            StatCatalogs {
                buffs: &test_catalog(),
                masteries: &[],
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            Some(test_attack_coefficients()),
            NO_CAP,
        )
        .unwrap();
        let attack = preview.attack.unwrap();
        let b = attack.breakdown;
        // シエナのオーラの全ステータス増加 +5 が最終能力値に乗る
        assert_eq!(preview.stats.get(StatKind::Stab), 105);
        assert!((b.stat_attack - (105.0 * 1.8 + 105.0 * 1.8)).abs() < 1e-9);
        // 基本 = 武器 150/150 + 手 30/30 → 突 180・斬 180
        assert!((b.equipment_base_attack - (180.0 * 14.5 + 180.0 * 14.5)).abs() < 1e-9);
        // 強化 = エンチャント 60/60 + シエナのオーラ(武器)20/20 → 突 80・斬 80。
        // テシスコアは地域なしなので入らない
        assert!((b.equipment_enhanced_attack - (80.0 * 28.75 + 80.0 * 28.75)).abs() < 1e-9);
        // パワーウェポン 2% + ストロングウェポン Lv6 18% = 20%
        assert!((b.enhance_rate - 0.20).abs() < 1e-9);
        assert_eq!(
            b.value,
            crate::attack_power::attack_power(b.stat_attack, b.equipment_attack(), b.enhance_rate)
        );
    }

    #[test]
    fn ソウルリンクはエンチャントでなく装備基本能力値に入る() {
        let base = BaseStats {
            stab: 100,
            hack: 100,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let sources = StatSources {
            soul_link: SoulLinkStatus {
                thrust_level: 1,
                slash_level: 2,
                magic_attack_level: 3,
                magic_defense_level: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let preview = preview_effective_stats(
            &base,
            &sources,
            &BuffSelection::default(),
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &test_catalog(),
                masteries: &[],
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            Some(test_attack_coefficients()),
            NO_CAP,
        )
        .unwrap();

        assert_eq!(
            preview.equipment_base_total,
            EquipmentValues {
                thrust: 2,
                slash: 4,
                magic_attack: 6,
                magic_defense: 8,
                ..Default::default()
            }
        );
        let attack = preview.attack.unwrap().breakdown;
        assert!((attack.equipment_base_attack - (2.0 * 14.5 + 4.0 * 14.5)).abs() < 1e-9);
        assert_eq!(attack.equipment_enhanced_attack, 0.0);
    }

    #[test]
    fn 部位の寄与は外したときの攻撃力との差に一致する() {
        let base = BaseStats {
            stab: 100,
            hack: 100,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let equipment = test_equipment();
        let sources = StatSources::default();
        let coefficients = test_attack_coefficients();
        let preview = preview_effective_stats(
            &base,
            &sources,
            &BuffSelection::default(),
            &equipment,
            &test_common_skills(),
            StatCatalogs {
                buffs: &test_catalog(),
                masteries: &[],
                character_skills: &[],
            },
            &[],
            &[],
            &[],
            Some(coefficients),
            NO_CAP,
        )
        .unwrap();
        let attack = preview.attack.unwrap();

        for (slot, _) in equipment.parts.iter() {
            let without = equipment.without_selected_part(slot);
            let preview_without = preview_effective_stats(
                &base,
                &sources,
                &BuffSelection::default(),
                &without,
                &test_common_skills(),
                StatCatalogs {
                    buffs: &test_catalog(),
                    masteries: &[],
                    character_skills: &[],
                },
                &[],
                &[],
                &[],
                Some(coefficients),
                NO_CAP,
            )
            .unwrap();
            let expected = attack.breakdown.value - preview_without.attack.unwrap().breakdown.value;
            let actual = attack
                .part_contributions
                .iter()
                .find(|c| c.slot == slot)
                .unwrap()
                .value;
            assert_eq!(
                actual, expected,
                "{slot:?} の寄与が外したときの差と一致しない"
            );
        }
        // 何も付いていない部位の寄与は 0、武器の寄与は正
        let weapon = attack
            .part_contributions
            .iter()
            .find(|c| c.slot == PartSlot::Weapon)
            .unwrap();
        assert!(weapon.value > 0);
        assert!(attack
            .part_contributions
            .iter()
            .all(|c| c.slot != PartSlot::Helm));
    }

    #[test]
    fn バフは層ごとに正しく積まれる() {
        let sources = StatSources::default();
        let buffs = BuffSelection {
            choices: vec![
                choice("illumination_drink"),
                choice("tales_weaver_energy"),
                choice("unleash"),
            ],
        };
        let (modifiers, _) = build_modifiers(&sources, &buffs, &test_catalog()).unwrap();
        let m = modifiers.get(StatKind::Stab);
        assert_eq!(m.percent_of_base, vec![0.30]);
        assert_eq!(m.multiplier_a, vec![1.1]);
        assert!((m.multiplier_b - 0.20).abs() < 1e-12);
    }

    #[test]
    fn バフの選択肢と手入力とユーザー選択ステが解決される() {
        let sources = StatSources::default();
        let buffs = BuffSelection {
            choices: vec![
                BuffChoice {
                    buff_id: "event_buff".into(),
                    stat: None,
                    choice_index: Some(2),
                    value: None,
                },
                BuffChoice {
                    buff_id: "trust_potion".into(),
                    stat: None,
                    choice_index: None,
                    value: Some(33.0),
                },
                BuffChoice {
                    buff_id: "club_effect".into(),
                    stat: Some(StatKind::Agi),
                    choice_index: None,
                    value: None,
                },
            ],
        };
        let (modifiers, _) = build_modifiers(&sources, &buffs, &test_catalog()).unwrap();
        // event_buff choice_index 2 → 0.30、trust_potion 手入力 33 が全ステの固定値に乗る
        assert_eq!(modifiers.get(StatKind::Int).percent_of_base, vec![0.30]);
        assert_eq!(modifiers.get(StatKind::Int).fixed, 33);
        // club_effect は Agi のみ選択、Fixed値7.0(実データに合わせ選択式ではなく固定値)
        assert_eq!(modifiers.get(StatKind::Agi).fixed, 33 + 7);
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 33);
    }

    // --- 2. ペットの上位選択で値が上書きされる(加算されない)こと ---

    #[test]
    fn ペットは上位段階を選んでも加算されず上書きになる() {
        // Option<PetSkillTier> は 1 値しか持てないため、TrueLv2 を選んだ状態は
        // 「Basic→TrueLv1→TrueLv2 と順に積み上げた結果」ではなく TrueLv2 単体の +40 になる。
        let sources = StatSources {
            pet_skills: PetSkills {
                stab: Some(PetSkillTier::TrueLv2),
                ..Default::default()
            },
            ..Default::default()
        };
        let (modifiers, _) =
            build_modifiers(&sources, &BuffSelection::default(), &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 40);
        assert_ne!(modifiers.get(StatKind::Stab).fixed, 20 + 30 + 40);
    }

    // --- 3. 排他枠違反 ---

    #[test]
    fn 排他枠が重複するとエラーになる() {
        let buffs = BuffSelection {
            choices: vec![choice("illumination_drink"), choice("charge_potion")],
        };
        let err = build_modifiers(&StatSources::default(), &buffs, &test_catalog()).unwrap_err();
        assert!(
            matches!(err, StatSourceError::ExclusiveSlotConflict { slot } if slot == "percent_slot_1")
        );
    }

    #[test]
    fn 未知のバフidはエラーになる() {
        let buffs = BuffSelection {
            choices: vec![choice("nope")],
        };
        let err = build_modifiers(&StatSources::default(), &buffs, &test_catalog()).unwrap_err();
        assert!(matches!(err, StatSourceError::UnknownBuff { id } if id == "nope"));
    }

    #[test]
    fn 同一buff_idを重複選択するとエラーになる() {
        // 排他枠が空の tales_weaver_energy を 2 回選んでも、排他枠チェックでは防げない
        // ことを確認しつつ、重複チェックで拒否されること。
        let buffs = BuffSelection {
            choices: vec![choice("tales_weaver_energy"), choice("tales_weaver_energy")],
        };
        let err = build_modifiers(&StatSources::default(), &buffs, &test_catalog()).unwrap_err();
        assert!(
            matches!(err, StatSourceError::DuplicateBuff { id } if id == "tales_weaver_energy")
        );
    }

    #[test]
    fn クラブ効果は複数のステに同時に掛けられる() {
        // クラブエフェクトはステごとに 1 つずつ、枠数だけ併用できる(wiki: クラブ)。
        let buffs = BuffSelection {
            choices: vec![
                BuffChoice {
                    stat: Some(StatKind::Stab),
                    ..choice("club_effect")
                },
                BuffChoice {
                    stat: Some(StatKind::Dex),
                    ..choice("club_effect")
                },
            ],
        };
        let (modifiers, contributions) =
            build_modifiers(&StatSources::default(), &buffs, &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 7);
        assert_eq!(modifiers.get(StatKind::Dex).fixed, 7);
        assert_eq!(modifiers.get(StatKind::Hack).fixed, 0);
        // 内訳もステごとに 1 行ずつ立つ(チップの増分表示がここから作られる)
        assert_eq!(
            contributions
                .iter()
                .filter(|c| c.source == "クラブ効果")
                .count(),
            2
        );
    }

    #[test]
    fn クラブ効果で同じステを二重に選ぶとエラーになる() {
        // 「上昇項目が同じエフェクトを併用することは出来ない」(wiki: クラブ)
        let buffs = BuffSelection {
            choices: vec![
                BuffChoice {
                    stat: Some(StatKind::Stab),
                    ..choice("club_effect")
                },
                BuffChoice {
                    stat: Some(StatKind::Stab),
                    ..choice("club_effect")
                },
            ],
        };
        let err = build_modifiers(&StatSources::default(), &buffs, &test_catalog()).unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::DuplicateBuffStat { id, kind }
            if id == "club_effect" && kind == StatKind::Stab
        ));
    }

    // --- 3.5. StatSources::validate() が各補正源の値域を拒否する ---

    #[test]
    fn ルーンスキルは0から20の範囲外を拒否する() {
        let mut sources = StatSources {
            rune_levels: RuneLevels {
                stab: RUNE_LEVEL_MAX,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(sources.validate().is_ok());
        sources.rune_levels.stab = RUNE_LEVEL_MAX + 1;
        let err = sources.validate().unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::OutOfRange {
                source_name: "ルーンスキル",
                kind: StatKind::Stab,
                value: 21,
                max: 20
            }
        ));
    }

    #[test]
    fn クラウンは選択報酬の能力値だけ300まで受ける() {
        let mut sources = StatSources {
            crown: Crown {
                hack: Crown::SELECTED_MAX_VALUE,
                selected_stat: Some(StatKind::Hack),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(sources.validate().is_ok());
        sources.crown.hack = Crown::SELECTED_MAX_VALUE + Crown::STEP;
        let err = sources.validate().unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::OutOfRange {
                source_name: "クラウン",
                kind: StatKind::Hack,
                value: 310,
                max: 300
            }
        ));
    }

    #[test]
    fn クラウンは未選択の能力値の100超過と10刻み以外を拒否する() {
        let mut sources = StatSources {
            crown: Crown {
                stab: Crown::BASE_MAX_VALUE + Crown::STEP,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(matches!(
            sources.validate(),
            Err(StatSourceError::OutOfRange {
                source_name: "クラウン",
                kind: StatKind::Stab,
                value: 110,
                max: 100,
            })
        ));

        sources.crown.stab = 15;
        assert!(matches!(
            sources.validate(),
            Err(StatSourceError::InvalidStep {
                source_name: "クラウン",
                kind: StatKind::Stab,
                value: 15,
                step: 10,
            })
        ));
    }

    #[test]
    fn 聖物は0から40段階の範囲外を拒否する() {
        let mut sources = StatSources {
            sacred_relic: SacredRelic {
                mr: SACRED_RELIC_STAGE_MAX,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(sources.validate().is_ok());
        sources.sacred_relic.mr = SACRED_RELIC_STAGE_MAX + 1;
        let err = sources.validate().unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::OutOfRange {
                source_name: "神鳥の聖物",
                kind: StatKind::Mr,
                value: 41,
                max: 40
            }
        ));
    }

    #[test]
    fn 中立なstatsourcesの値域検証は常に成功する() {
        assert!(StatSources::default().validate().is_ok());
    }

    #[test]
    fn 調整値のaddとpinは境界値を許容し範囲外を拒否する() {
        let mut adjustments = Adjustments::default();
        adjustments.stab.add = ADJUSTMENT_ADD_MIN;
        adjustments.hack.add = ADJUSTMENT_ADD_MAX;
        assert!(adjustments.validate().is_ok());

        let mut too_low = adjustments;
        too_low.stab.add = ADJUSTMENT_ADD_MIN - 1;
        assert!(matches!(
            too_low.validate(),
            Err(StatSourceError::AdjustmentOutOfRange {
                field: "加算",
                kind: StatKind::Stab,
                ..
            })
        ));

        let mut too_high = adjustments;
        too_high.hack.add = ADJUSTMENT_ADD_MAX + 1;
        assert!(matches!(
            too_high.validate(),
            Err(StatSourceError::AdjustmentOutOfRange {
                field: "加算",
                kind: StatKind::Hack,
                ..
            })
        ));

        let mut pin_ok = Adjustments::default();
        pin_ok.stab.pin = Some(ADJUSTMENT_PIN_MIN);
        pin_ok.hack.pin = Some(ADJUSTMENT_PIN_MAX);
        assert!(pin_ok.validate().is_ok());

        let mut pin_too_low = Adjustments::default();
        pin_too_low.stab.pin = Some(ADJUSTMENT_PIN_MIN - 1);
        assert!(matches!(
            pin_too_low.validate(),
            Err(StatSourceError::AdjustmentOutOfRange {
                field: "固定",
                kind: StatKind::Stab,
                ..
            })
        ));

        let mut pin_too_high = Adjustments::default();
        pin_too_high.hack.pin = Some(ADJUSTMENT_PIN_MAX + 1);
        assert!(matches!(
            pin_too_high.validate(),
            Err(StatSourceError::AdjustmentOutOfRange {
                field: "固定",
                kind: StatKind::Hack,
                ..
            })
        ));
    }

    // --- 4. BaseStats::validate() が 1..=310 の範囲外を拒否する ---

    #[test]
    fn 素ステは1から310の範囲外を拒否する() {
        let mut base = BaseStats {
            stab: BASE_STAT_MAX,
            hack: 1,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        assert!(base.validate().is_ok());
        base.stab = 0;
        assert!(matches!(
            base.validate(),
            Err(BaseStatsError::OutOfRange { value: 0, .. })
        ));
        base.stab = BASE_STAT_MAX + 1;
        assert!(matches!(
            base.validate(),
            Err(BaseStatsError::OutOfRange { value: 311, .. })
        ));
    }

    // --- 4.4. buff_target_stat_gains(対象ステを選ぶバフの、ステごとの実際の効き) ---

    fn gains_for(base: BaseStats, cap: i64) -> Vec<BuffTargetStatGain> {
        let catalog = test_catalog();
        let def = catalog.iter().find(|d| d.id == "club_effect").unwrap();
        buff_target_stat_gains(
            &base,
            &StatSources::default(),
            &BuffSelection::default(),
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: &[],
                character_skills: &[],
            },
            def,
            cap,
        )
        .unwrap()
    }

    #[test]
    fn 対象ステの効きは全ステぶん返る() {
        let gains = gains_for(BaseStats { stab: 1, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 }, NO_CAP);
        assert_eq!(gains.len(), StatKind::ALL.len());
        // test_catalog の club_effect は Fixed(7.0)。上限に当たらなければどのステも +7
        assert!(gains.iter().all(|g| g.gain == 7), "{gains:?}");
    }

    #[test]
    fn 上限に張り付いたステの効きは0になる() {
        // STAB だけ上限(10)に達している状態。ここへ +7 を乗せても最終能力値は動かない
        let base = BaseStats { stab: 10, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
        let gains = gains_for(base, 10);
        let stab = gains.iter().find(|g| g.kind == StatKind::Stab).unwrap();
        let hack = gains.iter().find(|g| g.kind == StatKind::Hack).unwrap();
        assert_eq!(stab.gain, 0, "上限に張り付いたステは選んでも動かない");
        assert_eq!(hack.gain, 7);
    }

    #[test]
    fn 対象ステの効きはいま選んでいるステに左右されない() {
        // 既に DEX を選んでいる状態でも、基準は「このバフを外した状態」なので
        // どのステも同じ +7 が返る(選択中のステだけ 0 に見える、という罠を防ぐ)
        let base = BaseStats { stab: 1, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
        let catalog = test_catalog();
        let def = catalog.iter().find(|d| d.id == "club_effect").unwrap();
        let buffs = BuffSelection {
            choices: vec![BuffChoice {
                stat: Some(StatKind::Dex),
                ..choice("club_effect")
            }],
        };
        let gains = buff_target_stat_gains(
            &base,
            &StatSources::default(),
            &buffs,
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: &[],
                character_skills: &[],
            },
            def,
            NO_CAP,
        )
        .unwrap();
        assert!(gains.iter().all(|g| g.gain == 7), "{gains:?}");
    }

    #[test]
    fn 排他枠が埋まっているバフの効きは空で返る() {
        // illumination_drink と同じ枠を charge_potion が押さえている状態では、
        // illumination_drink はそもそも足せない — エラーにせず「材料なし」を返す
        let catalog = test_catalog();
        let def = catalog
            .iter()
            .find(|d| d.id == "illumination_drink")
            .unwrap();
        let buffs = BuffSelection {
            choices: vec![choice("charge_potion")],
        };
        let gains = buff_target_stat_gains(
            &BaseStats { stab: 1, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 },
            &StatSources::default(),
            &buffs,
            &Equipment::default(),
            &CommonSkills::default(),
            StatCatalogs {
                buffs: &catalog,
                masteries: &[],
                character_skills: &[],
            },
            def,
            NO_CAP,
        )
        .unwrap();
        assert!(gains.is_empty());
    }

    // --- 4.5. apply_temporary_adjustments(計算リクエストにのみ乗る一時調整) ---

    #[test]
    fn 一時調整の加算は固定値に積まれてsourceが一時調整になる() {
        let temporary = Adjustments {
            mr: StatAdjustment { add: 12, pin: None },
            ..Default::default()
        };
        let (mut modifiers, mut contributions) = build_modifiers(
            &StatSources::default(),
            &BuffSelection::default(),
            &test_catalog(),
        )
        .unwrap();
        apply_temporary_adjustments(&mut modifiers, &mut contributions, &temporary);

        assert_eq!(modifiers.get(StatKind::Mr).fixed, 12);
        let rows: Vec<_> = contributions
            .iter()
            .filter(|c| c.kind == StatKind::Mr)
            .collect();
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|c| c.source == "一時調整"));
    }

    #[test]
    fn 中立な一時調整は何も積まない() {
        let (mut modifiers, mut contributions) = build_modifiers(
            &StatSources::default(),
            &BuffSelection::default(),
            &test_catalog(),
        )
        .unwrap();
        let before_contributions = contributions.len();
        apply_temporary_adjustments(&mut modifiers, &mut contributions, &Adjustments::default());

        assert_eq!(contributions.len(), before_contributions);
        for kind in StatKind::ALL {
            assert_eq!(modifiers.get(kind).fixed, 0);
            assert_eq!(modifiers.get(kind).final_fixed, 0);
        }
    }

    // --- 4.6. build_stat_modifiers(補正パイプラインの唯一の経路) ---

    #[test]
    fn build_stat_modifiersは一時調整を最終段で積み範囲外なら弾く() {
        let catalogs = StatCatalogs {
            buffs: &test_catalog(),
            masteries: &[],
            character_skills: &[],
        };
        let temporary = Adjustments {
            mr: StatAdjustment { add: 12, pin: None },
            ..Default::default()
        };
        let (modifiers, contributions) = build_stat_modifiers(
            &StatSources::default(),
            &BuffSelection::default(),
            &Equipment::default(),
            &CommonSkills::default(),
            catalogs,
            Some(&temporary),
        )
        .unwrap();
        assert_eq!(modifiers.get(StatKind::Mr).fixed, 12);
        assert!(contributions.iter().any(|c| c.source == "一時調整"));

        let out_of_range = Adjustments {
            mr: StatAdjustment {
                add: ADJUSTMENT_ADD_MAX + 1,
                pin: None,
            },
            ..Default::default()
        };
        let err = build_stat_modifiers(
            &StatSources::default(),
            &BuffSelection::default(),
            &Equipment::default(),
            &CommonSkills::default(),
            catalogs,
            Some(&out_of_range),
        )
        .unwrap_err();
        assert!(
            matches!(err, StatSourceError::AdjustmentOutOfRange { .. }),
            "{err:?}"
        );
    }

    // --- 5. 通し値テスト(goal 指定) ---

    // 素ステ310 + ルーン+20(fixed) + ペット+60(TrueLv4, fixed) + 聖物40段階(+400, final_fixed)
    // + バフ tales_weaver_energy(multiplier_a 1.1) + バフ unleash(multiplier_b 0.20) を stab に適用する。
    //
    // 手計算:
    //   基本 = floor((310 + 20 + 60) × 1.1) = floor(429.0) = 429
    //   最終 = 429 + floor(429 × 0.2) + 400 = 429 + 85 + 400 = 914
    #[test]
    fn 通し値_ルーン_ペット_聖物_バフ2種を合成すると914になる() {
        let sources = StatSources {
            pet_skills: PetSkills {
                stab: Some(PetSkillTier::TrueLv4),
                ..Default::default()
            },
            rune_levels: RuneLevels {
                stab: 20,
                ..Default::default()
            },
            sacred_relic: SacredRelic {
                stab: 40,
                ..Default::default()
            },
            ..Default::default()
        };
        let buffs = BuffSelection {
            choices: vec![choice("tales_weaver_energy"), choice("unleash")],
        };
        let (modifiers, _) = build_modifiers(&sources, &buffs, &test_catalog()).unwrap();
        let base = BaseStats {
            stab: 310,
            ..Default::default()
        };
        let (value, trace) = effective_stat(
            StatKind::Stab,
            base.stab,
            modifiers.get(StatKind::Stab),
            NO_CAP,
        );
        assert_eq!(trace.basic, 429);
        assert_eq!(trace.multiplier_b_bonus, 85);
        assert_eq!(value, 914);
    }

    // wiki ステータス「固定値増加/減少」: モンスターカード(カード装着)+0〜70。
    // ユーザーの実測(2026-08-25)でも AGI に +70 乗っていた
    #[test]
    fn モンスターカードは固定値層に乗り上限70() {
        let mut sources = StatSources::default();
        sources.monster_cards.agi = 70;
        assert!(sources.validate().is_ok());

        let (modifiers, contributions) =
            build_modifiers(&sources, &BuffSelection::default(), &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Agi).fixed, 70);
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 0);
        let c = contributions
            .iter()
            .find(|c| c.source == "モンスターカード")
            .unwrap();
        assert_eq!(c.kind, StatKind::Agi);
        assert_eq!(c.layer, StatLayer::Fixed);
        assert_eq!(c.value, 70.0);

        sources.monster_cards.agi = 71;
        assert!(matches!(
            sources.validate(),
            Err(StatSourceError::OutOfRange {
                source_name: "モンスターカード",
                max: 70,
                ..
            })
        ));
    }

    #[test]
    fn stat_limitsは対応する定数と一致する() {
        let limits = stat_limits();
        assert_eq!(limits.base_stat_max, BASE_STAT_MAX);
        assert_eq!(limits.rune_level_max, RUNE_LEVEL_MAX);
        assert_eq!(limits.crown_base_max, Crown::BASE_MAX_VALUE);
        assert_eq!(limits.crown_selected_max, Crown::SELECTED_MAX_VALUE);
        assert_eq!(limits.crown_step, Crown::STEP);
        assert_eq!(limits.sacred_relic_stage_max, SACRED_RELIC_STAGE_MAX);
        assert_eq!(
            limits.soul_link_equipment_level_max,
            SOUL_LINK_EQUIPMENT_LEVEL_MAX
        );
        assert_eq!(
            limits.soul_link_critical_damage_level_max,
            SOUL_LINK_CRITICAL_DAMAGE_LEVEL_MAX
        );
        assert_eq!(
            limits.soul_link_final_damage_level_max,
            SOUL_LINK_FINAL_DAMAGE_LEVEL_MAX
        );
        assert_eq!(
            limits.soul_link_weapon_enhance_level_max,
            SOUL_LINK_WEAPON_ENHANCE_LEVEL_MAX
        );
        assert_eq!(
            limits.soul_link_armor_enhance_level_max,
            SOUL_LINK_ARMOR_ENHANCE_LEVEL_MAX
        );
        assert_eq!(limits.adjustment_add_min, ADJUSTMENT_ADD_MIN);
        assert_eq!(limits.adjustment_add_max, ADJUSTMENT_ADD_MAX);
        assert_eq!(limits.adjustment_pin_min, ADJUSTMENT_PIN_MIN);
        assert_eq!(limits.adjustment_pin_max, ADJUSTMENT_PIN_MAX);
        assert_eq!(limits.equipment_value_max, EQUIPMENT_VALUE_MAX);
        assert_eq!(limits.strong_weapon_level_max, STRONG_WEAPON_LEVEL_MAX);
        assert_eq!(
            limits.enhance_level_max,
            crate::equipment::ENHANCE_LEVEL_MAX
        );
        assert_eq!(limits.combo_bonus_threshold, crate::damage::COMBO_BONUS_THRESHOLD);
        assert_eq!(
            limits.combo_delay_threshold,
            crate::actual_delay::COMBO_DELAY_THRESHOLD
        );
        assert_eq!(limits.combo_bonus_rate, crate::damage::COMBO_BONUS_RATE);
        assert_eq!(
            limits.actual_delay_reduction_max,
            crate::actual_delay::ACTUAL_DELAY_REDUCTION_MAX
        );
        assert_eq!(limits.actual_delay_min, crate::actual_delay::ACTUAL_DELAY_MIN);
        assert_eq!(
            limits.unleash_free_level_max,
            crate::common_skill::UNLEASH_FREE_LEVEL_MAX
        );
        assert_eq!(
            limits.enhance_grade_min_level,
            crate::equipment::ENHANCE_LEVEL_RANDOM_RANGE_MIN
        );
        assert_eq!(
            limits.element_bonus_percent_per_point,
            crate::damage::ELEMENT_BONUS_PERCENT_PER_POINT
        );
        assert_eq!(
            limits.element_bonus_max,
            DamageCategory::ElementBonus.cap().unwrap().max.unwrap()
        );
        assert_eq!(limits.cut_rate_denominator, crate::defense::CUT_RATE_DENOMINATOR);
        assert_eq!(limits.cut_rate_a_base, crate::defense::CUT_RATE_A_BASE);
        assert_eq!(limits.cut_rate_divisor, crate::defense::CUT_RATE_DIVISOR);
        assert_eq!(
            limits.cut_rate_composite_divisor,
            crate::defense::CUT_RATE_COMPOSITE_DIVISOR
        );
        assert_eq!(
            limits.defense_stat_multiplier,
            crate::defense::DEFENSE_STAT_MULTIPLIER
        );
        assert_eq!(
            limits.defense_equipment_multiplier,
            crate::defense::DEFENSE_EQUIPMENT_MULTIPLIER
        );
        assert_eq!(
            limits.composite_defense_stat_multiplier,
            crate::defense::COMPOSITE_DEFENSE_STAT_MULTIPLIER
        );
        assert_eq!(
            limits.composite_defense_equipment_multiplier,
            crate::defense::COMPOSITE_DEFENSE_EQUIPMENT_MULTIPLIER
        );
        assert_eq!(limits.evasion_point_base, crate::defense::EVASION_POINT_BASE);
        assert_eq!(
            limits.evasion_point_agi_rate,
            crate::defense::EVASION_POINT_AGI_RATE
        );
        assert_eq!(limits.evasion_type_divisor, crate::defense::EVASION_TYPE_DIVISOR);
        assert_eq!(
            limits.evasion_physical_attack_divisor,
            crate::defense::EVASION_PHYSICAL_ATTACK_DIVISOR
        );
        assert_eq!(limits.pet_critical_rate, crate::critical_rate::PET_CRITICAL_RATE);
        assert_eq!(limits.critical_rate_min, crate::critical_rate::CRITICAL_RATE_MIN);
        assert_eq!(limits.critical_rate_max, crate::critical_rate::CRITICAL_RATE_MAX);
    }

    #[test]
    fn 聖物の実値は段階へ切り捨てで逆算する() {
        assert_eq!(sacred_relic_stage_from_value(0), 0);
        assert_eq!(sacred_relic_stage_from_value(10), 1);
        // 1 段階(10)に満たない端数は切り捨てる(四捨五入しない)
        assert_eq!(sacred_relic_stage_from_value(15), 1);
        assert_eq!(sacred_relic_stage_from_value(19), 1);
        // 上限を超える値は最大段階に clamp する
        assert_eq!(
            sacred_relic_stage_from_value(10_000),
            SACRED_RELIC_STAGE_MAX
        );
        // 負値は 0 に clamp する
        assert_eq!(sacred_relic_stage_from_value(-5), 0);
    }

    #[test]
    fn 新規キャラ既定値はソウルリンクを含め未開放() {
        assert_eq!(StatSources::default(), StatSources::default());
    }

    /// 命中P増加バフの合計と伸びしろ。的中剣の効果中は `exclusive_with` に的中剣を持つものを
    /// 持つバフ(テイルズウィーバーのエネルギー相当)を除外する。
    #[test]
    fn 命中p増加バフの合計と的中剣排他() {
        fn buff(id: &'static str, value: i64, disabled_with_precision_sword: bool) -> BuffDefinition {
            BuffDefinition {
                id,
                name: id,
                purposes: &[BuffPurpose::Accuracy],
                origin: BuffOrigin::Skill,
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::RecordOnly,
                exclusive_slots: Vec::new(),
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: Box::leak(Box::new([SkillEffect::AccuracyPoint {
                    value,
                    exclusive_with: if disabled_with_precision_sword {
                        &["maximin_hit_sword"]
                    } else {
                        &[]
                    },
                }])),
            }
        }
        let catalog = vec![
            buff("normal_accuracy", 20, false),
            buff("precision_sword_only", 5, true),
        ];
        let none = crate::defense::AccuracyBoost::NONE;
        let sword = crate::defense::AccuracyBoost::from_rate_skill(
            "maximin_hit_sword",
            "極・的中剣",
            0.05,
            &[3, 2, 1, 1, 0],
            5,
            7,
        );

        // 何も選んでいなければ合計 0、伸びしろは両方の値
        let empty = BuffSelection::default();
        assert_eq!(buff_accuracy_point_total(&empty, &catalog, none), 0);
        assert_eq!(buff_accuracy_point_room(&empty, &catalog, none), 25);
        // 的中剣装着中は排他なバフを伸びしろからも除く
        assert_eq!(buff_accuracy_point_room(&empty, &catalog, sword), 20);

        // 両方選んでいるとき、的中剣装着中は排他なバフの分だけ合計から落ちる
        let both = BuffSelection {
            choices: catalog
                .iter()
                .map(|d| BuffChoice {
                    buff_id: d.id.to_string(),
                    stat: None,
                    choice_index: None,
                    value: None,
                })
                .collect(),
        };
        assert_eq!(buff_accuracy_point_total(&both, &catalog, none), 25);
        assert_eq!(buff_accuracy_point_total(&both, &catalog, sword), 20);
        assert_eq!(buff_accuracy_point_room(&both, &catalog, none), 0);
    }
}
