//! キャラクターの実効ステータスに効く恒常補正源(ペット・ルーン・クラウン・聖物)と常用バフ。
//!
//! docs/claude/goals/2026-08-21-character-stat-sources.md。バフは個別にコードで分岐せず、
//! 「カテゴリ(層)+ 数値 + 重複枠」を持つデータ(`BuffDefinition`)として解決する
//! (CLAUDE.md 原則、crates/domain/src/category.rs の設計思想を踏襲)。
//! カタログの実データ(常用バフ 16 件)は gamedata に置く。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::attack_power::{
    attack_power_breakdown, stat_attack_power, AttackCoefficients, AttackPowerBreakdown,
};
use crate::equipment::{
    equipment_values_attack, Equipment, EquipmentAbilityDef, EquipmentCoefficients, EquipmentError,
    PartSlot, ENHANCE_ADDED_DAMAGE_MAX, ENHANCE_LEVEL_MAX, EQUIPMENT_VALUE_MAX,
    SIENA_ALL_STATS_BONUS_MAX, SIENA_ATTACK_RATE_PERCENT_MAX, SIENA_STAGE_MAX,
    SIENA_STAT_BONUS_MAX, STRONG_WEAPON_LEVEL_MAX,
};
use crate::thesis_core::{CORE_ENHANCEMENT_MAX, CORE_EVOLUTION_MAX, CORE_SLOT_COUNT};
use crate::stats::{
    effective_stats, BaseStats, BaseStatsError, EffectiveStats, PinSource, StatKind, StatModifierSet,
    StatTrace, BASE_STAT_MAX,
};

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

/// ペット S スキル。ステごとに 1 つ(上位段階を選ぶと置き換わる。加算にはならない)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PetSkills {
    pub stab: Option<PetSkillTier>,
    pub hack: Option<PetSkillTier>,
    pub int: Option<PetSkillTier>,
    pub def: Option<PetSkillTier>,
    pub mr: Option<PetSkillTier>,
    pub dex: Option<PetSkillTier>,
    pub agi: Option<PetSkillTier>,
}

impl PetSkills {
    pub fn get(&self, kind: StatKind) -> Option<PetSkillTier> {
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

/// ルーンスキル(閃光/斬撃/英知/才気/石壁/魔壁/瞬発、wiki: ルーンマスター#skill_atk)。
/// +1/Lv、Lv20 上限。「装備可能ステには影響しない」固定値層。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuneLevels {
    pub stab: u8,
    pub hack: u8,
    pub int: u8,
    pub def: u8,
    pub mr: u8,
    pub dex: u8,
    pub agi: u8,
}

impl RuneLevels {
    pub const MAX_LEVEL: u8 = 20;

    pub fn get(&self, kind: StatKind) -> u8 {
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

/// クラウン(wiki: クラウン)。週次ランク報酬+名声強化で、シーズンごとに戻る。
/// 枠だけ用意し値はユーザーの手入力とする。ステごと 0..=300、最終固定値層。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Crown {
    pub stab: u32,
    pub hack: u32,
    pub int: u32,
    pub def: u32,
    pub mr: u32,
    pub dex: u32,
    pub agi: u32,
}

impl Crown {
    pub const MAX_VALUE: u32 = 300;

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

/// 神鳥の聖物(wiki: 神鳥の聖物)。ステごと 0..=40 段階、+10 刻みで最終固定値に乗る。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SacredRelic {
    pub stab: u8,
    pub hack: u8,
    pub int: u8,
    pub def: u8,
    pub mr: u8,
    pub dex: u8,
    pub agi: u8,
}

impl SacredRelic {
    pub const MAX_STAGE: u8 = 40;
    /// 1 段階あたりの最終固定値。
    const VALUE_PER_STAGE: i64 = 10;

    pub fn get(&self, kind: StatKind) -> u8 {
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

    /// 段階を最終固定値(0..=+400)に変換する。
    pub fn value(&self, kind: StatKind) -> i64 {
        i64::from(self.get(kind)) * Self::VALUE_PER_STAGE
    }
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
    /// 登録時にどのステかを選ぶ(例: 固定増加系・クラブ効果)
    UserSelected,
    /// 複数の特定ステに同じ値を適用(例: ロアミニの極・パウアトゥンが DEF/MR に効く)
    Stats(&'static [StatKind]),
}

/// バフの分類。カタログ表示のグルーピングと、キャラスキルのキャラ紐付けに使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffGroup {
    /// 常用バフ(消費アイテム・イベント等)
    Consumable,
    /// 選択キャラ本人のスキル(自身のみ効果)
    CharacterSkill { game_character_id: &'static str },
    /// 味方に掛かるキャラスキル(誰でも ON にできる)
    AllySkill,
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
}

/// バフカタログの 1 エントリ。型はここ(domain)、実データ(常用バフ 16 件)は gamedata に置く。
/// `target: BuffTarget` が `Serialize` のみのため、このカタログ自体も `Serialize` のみ導出する
/// (Tauri コマンドの戻り値としてフロントへ一方向にシリアライズするだけで、デシリアライズはしない)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuffDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub target: BuffTarget,
    pub layer: StatLayer,
    pub value: BuffValue,
    /// 同時に選べない枠。空なら排他無し(独立)
    pub exclusive_slots: Vec<&'static str>,
    pub source_url: &'static str,
    pub note: &'static str,
    /// `BuffValue::UserInput` の初期値。それ以外は `None`
    pub default_value: Option<f64>,
    pub group: BuffGroup,
}

/// バフカタログ。呼び出しは `&BuffCatalog` = `&[BuffDefinition]`。
pub type BuffCatalog = [BuffDefinition];

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
pub const ADJUSTMENT_ADD_MIN: i64 = -999;
pub const ADJUSTMENT_ADD_MAX: i64 = 999;
/// 調整「固定(pin)」の妥当範囲。上限は最終能力値の理論上限(エタの意志Lv80、docs/claude/decisions.md「2400」)。
pub const ADJUSTMENT_PIN_MIN: i64 = 1;
pub const ADJUSTMENT_PIN_MAX: i64 = 2400;

/// ステ 1 つの自由な調整(検証・未収録バフ用)。
/// - `add`: このステに +N する(固定値層への加算)
/// - `pin`: 最終能力値そのものを N に固定する(実測値で計算したい時)。`Some` のとき
///   能力値計算の結果を上書きし、`StatTrace.pinned_from` に上書き前の値を残す
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatAdjustment {
    pub add: i64,
    pub pin: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Adjustments {
    pub stab: StatAdjustment,
    pub hack: StatAdjustment,
    pub int: StatAdjustment,
    pub def: StatAdjustment,
    pub mr: StatAdjustment,
    pub dex: StatAdjustment,
    pub agi: StatAdjustment,
}

impl Adjustments {
    pub fn get(&self, kind: StatKind) -> StatAdjustment {
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

    pub fn get_mut(&mut self, kind: StatKind) -> &mut StatAdjustment {
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

/// キャラクターに紐づく補正源一式。`Default` は全フィールド中立
/// (ペット無し、ルーン 0、クラウン 0、聖物 0 段階、バフ無し、調整値 0)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StatSources {
    #[serde(default)]
    pub pet_skills: PetSkills,
    #[serde(default)]
    pub rune_levels: RuneLevels,
    #[serde(default)]
    pub crown: Crown,
    #[serde(default)]
    pub sacred_relic: SacredRelic,
    #[serde(default)]
    pub buffs: BuffSelection,
    #[serde(default)]
    pub adjustments: Adjustments,
}

impl StatSources {
    /// ルーンスキル(0..=20)/クラウン(0..=300)/聖物(0..=40段階)/調整値(`Adjustments::validate`)
    /// の値域を検証する。ペットは enum で構造的に制約済みなので対象外。
    pub fn validate(&self) -> Result<(), StatSourceError> {
        for kind in StatKind::ALL {
            let rune = self.rune_levels.get(kind);
            if rune > RuneLevels::MAX_LEVEL {
                return Err(StatSourceError::OutOfRange {
                    source_name: "ルーンスキル",
                    kind,
                    value: u32::from(rune),
                    max: u32::from(RuneLevels::MAX_LEVEL),
                });
            }

            let crown = self.crown.get(kind);
            if crown > Crown::MAX_VALUE {
                return Err(StatSourceError::OutOfRange {
                    source_name: "クラウン",
                    kind,
                    value: crown,
                    max: Crown::MAX_VALUE,
                });
            }

            let relic = self.sacred_relic.get(kind);
            if relic > SacredRelic::MAX_STAGE {
                return Err(StatSourceError::OutOfRange {
                    source_name: "神鳥の聖物",
                    kind,
                    value: u32::from(relic),
                    max: u32::from(SacredRelic::MAX_STAGE),
                });
            }
        }
        self.adjustments.validate()?;
        Ok(())
    }
}

/// 寄与内訳の 1 行(ステトレース向け)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatContribution {
    pub source: String,
    pub kind: StatKind,
    pub layer: StatLayer,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum StatSourceError {
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
    OutOfRange { source_name: &'static str, kind: StatKind, value: u32, max: u32 },
    #[error("バフ '{id}' が重複して選択されています")]
    DuplicateBuff { id: String },
    #[error("バフ '{id}' の入力値が範囲外です({min}..={max}、指定値 {value})")]
    ValueOutOfRange { id: String, value: f64, min: f64, max: f64 },
    #[error("バフ '{id}' はこのキャラ(game_character_id={game_character_id})のスキルではありません")]
    ForeignCharacterSkill { id: String, game_character_id: String },
    #[error("調整の{field}は{kind:?}で{min}..={max}の範囲で指定してください(指定値 {value})")]
    AdjustmentOutOfRange { field: &'static str, kind: StatKind, value: i64, min: i64, max: i64 },
    #[error(transparent)]
    BaseStats(#[from] BaseStatsError),
    #[error(transparent)]
    Equipment(#[from] EquipmentError),
}

/// `StatSources` と バフカタログから `StatModifierSet` と寄与内訳を組み立てる。
pub fn build_modifiers(
    sources: &StatSources,
    catalog: &BuffCatalog,
    game_character_id: &str,
) -> Result<(StatModifierSet, Vec<StatContribution>), StatSourceError> {
    let mut modifiers = StatModifierSet::default();
    let mut contributions = Vec::new();

    for kind in StatKind::ALL {
        if let Some(tier) = sources.pet_skills.get(kind) {
            let bonus = tier.bonus();
            modifiers.get_mut(kind).fixed += bonus;
            contributions.push(StatContribution {
                source: format!("ペット Sスキル({tier:?})"),
                kind,
                layer: StatLayer::Fixed,
                value: bonus as f64,
            });
        }
    }

    for kind in StatKind::ALL {
        let level = sources.rune_levels.get(kind);
        if level > 0 {
            let bonus = i64::from(level);
            modifiers.get_mut(kind).fixed += bonus;
            contributions.push(StatContribution {
                source: "ルーンスキル".to_string(),
                kind,
                layer: StatLayer::Fixed,
                value: bonus as f64,
            });
        }
    }

    for kind in StatKind::ALL {
        let value = sources.crown.get(kind);
        if value > 0 {
            let bonus = i64::from(value);
            modifiers.get_mut(kind).final_fixed += bonus;
            contributions.push(StatContribution {
                source: "クラウン".to_string(),
                kind,
                layer: StatLayer::FinalFixed,
                value: bonus as f64,
            });
        }
    }

    for kind in StatKind::ALL {
        let stage = sources.sacred_relic.get(kind);
        if stage > 0 {
            let bonus = sources.sacred_relic.value(kind);
            modifiers.get_mut(kind).final_fixed += bonus;
            contributions.push(StatContribution {
                source: "神鳥の聖物".to_string(),
                kind,
                layer: StatLayer::FinalFixed,
                value: bonus as f64,
            });
        }
    }

    apply_adjustments(&mut modifiers, &mut contributions, &sources.adjustments, "調整値");

    let mut used_slots: HashSet<&'static str> = HashSet::new();
    let mut used_buff_ids: HashSet<&str> = HashSet::new();
    for choice in &sources.buffs.choices {
        if !used_buff_ids.insert(choice.buff_id.as_str()) {
            return Err(StatSourceError::DuplicateBuff { id: choice.buff_id.clone() });
        }

        let def = catalog
            .iter()
            .find(|d| d.id == choice.buff_id)
            .ok_or_else(|| StatSourceError::UnknownBuff { id: choice.buff_id.clone() })?;

        if let BuffGroup::CharacterSkill { game_character_id: owner } = def.group {
            if owner != game_character_id {
                return Err(StatSourceError::ForeignCharacterSkill {
                    id: def.id.to_string(),
                    game_character_id: game_character_id.to_string(),
                });
            }
        }

        for slot in def.exclusive_slots.iter().copied() {
            if !used_slots.insert(slot) {
                return Err(StatSourceError::ExclusiveSlotConflict { slot: slot.to_string() });
            }
        }

        let value = match &def.value {
            BuffValue::Fixed(v) => *v,
            BuffValue::Choice(options) => {
                let index = choice
                    .choice_index
                    .ok_or_else(|| StatSourceError::MissingChoice { id: def.id.to_string() })?;
                *options
                    .get(index)
                    .ok_or_else(|| StatSourceError::ChoiceOutOfRange { id: def.id.to_string() })?
            }
            BuffValue::UserInput { min, max } => {
                let v = choice
                    .value
                    .ok_or_else(|| StatSourceError::MissingValue { id: def.id.to_string() })?;
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
            BuffTarget::UserSelected => {
                vec![choice.stat.ok_or_else(|| StatSourceError::MissingStat { id: def.id.to_string() })?]
            }
            BuffTarget::Stats(kinds) => kinds.to_vec(),
        };

        for kind in targets {
            let m = modifiers.get_mut(kind);
            match def.layer {
                StatLayer::PercentOfBase => m.percent_of_base.push(value),
                StatLayer::Fixed => m.fixed += value as i64,
                StatLayer::MultiplierA => m.multiplier_a.push(value),
                StatLayer::MultiplierB => m.multiplier_b += value,
                StatLayer::FinalFixed => m.final_fixed += value as i64,
            }
            contributions.push(StatContribution {
                source: def.name.to_string(),
                kind,
                layer: def.layer,
                value,
            });
        }
    }

    Ok((modifiers, contributions))
}

/// `Adjustments` の加算(`add`)を `StatModifierSet` の固定値レイヤーに合流させる。
/// キャラの調整値(source="調整値")と計算リクエストの一時調整(source="一時調整")の両方で使う共通ロジック。
/// `pin`(能力値の固定)はここでは扱わない。能力値計算(`effective_stats`)の後に `apply_pins` が適用する。
fn apply_adjustments(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    adjustments: &Adjustments,
    source: &str,
) {
    for kind in StatKind::ALL {
        let adjustment = adjustments.get(kind);
        if adjustment.add != 0 {
            modifiers.get_mut(kind).fixed += adjustment.add;
            contributions.push(StatContribution {
                source: source.to_string(),
                kind,
                layer: StatLayer::Fixed,
                value: adjustment.add as f64,
            });
        }
    }
}

/// 計算リクエストにのみ乗る一時調整(キャラには保存しない)の加算(`add`)を `StatModifierSet` に合流させる。
/// キャラの調整値と同じ経路(固定値層)を通すが、寄与内訳の source 名を「一時調整」にして区別する。
/// `build_modifiers` が返した `modifiers`/`contributions` に対して呼び出し側(コマンド)が追加で適用する。
/// `pin` はここでは扱わない(呼び出し側が `apply_pins` に base/temporary を渡して適用する)。
pub fn apply_temporary_adjustments(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    adjustments: &Adjustments,
) {
    apply_adjustments(modifiers, contributions, adjustments, "一時調整");
}

/// 調整の「固定(pin)」を反映する。`base`(キャラの調整値)と `temporary`(計算リクエストの
/// 一時調整、無ければ None)からステごとに pin 値と出所を決め、`stats`/`trace.effective`/
/// `trace.pinned_from`/`trace.pin_source` に反映する。temporary 側に pin があれば優先する
/// (出所は Temporary)。無ければ base 側にフォールバックする(出所は Saved)。
pub fn apply_pins(
    stats: &mut EffectiveStats,
    traces: &mut [StatTrace],
    base: &Adjustments,
    temporary: Option<&Adjustments>,
) {
    for kind in StatKind::ALL {
        let temp_pin = temporary.and_then(|t| t.get(kind).pin);
        let (pin, source) = match temp_pin {
            Some(p) => (Some(p), PinSource::Temporary),
            None => (base.get(kind).pin, PinSource::Saved),
        };
        if let Some(pin) = pin {
            if let Some(trace) = traces.iter_mut().find(|t| t.kind == kind) {
                trace.pinned_from = Some(trace.effective);
                trace.pin_source = Some(source);
                trace.effective = pin;
            }
            stats.set(kind, pin);
        }
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

/// `preview_effective_stats` の結果(最終能力値・トレース・寄与内訳・攻撃力)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatPreview {
    pub stats: EffectiveStats,
    pub traces: Vec<StatTrace>,
    pub contributions: Vec<StatContribution>,
    /// 主軸スキル未選択なら `None`
    pub attack: Option<AttackPreview>,
}

/// シエナのオーラのステ加算(wiki: 能力値一覧(その他の部位)・追加オプション「全ステータス増加」)を
/// `StatModifierSet` の最終固定値層に合流させる。
///
/// シエナのオーラは装備部位に属する(`EquipmentPart::siena`)ので `StatSources` からは組み立てられない。
/// `build_modifiers` を呼んだ側が続けて呼ぶ(ダメージ計算・能力値プレビューの両方)。
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
                kind,
                layer: StatLayer::FinalFixed,
                value: value as f64,
            });
        }
    }
}

/// `BaseStats` + `StatSources` + 装備(シエナのオーラ)から最終能力値を組み立てる(pin 込み)。
/// 装備が最終能力値に効く経路(シエナのオーラのステ加算)を含むので、部位ごとの寄与を出すときは
/// 装備を差し替えてここから丸ごと引き直す。
fn effective_stats_with(
    base: &BaseStats,
    sources: &StatSources,
    equipment: &Equipment,
    catalog: &BuffCatalog,
    game_character_id: &str,
) -> Result<(EffectiveStats, Vec<StatTrace>, Vec<StatContribution>), StatSourceError> {
    let (mut modifiers, mut contributions) = build_modifiers(sources, catalog, game_character_id)?;
    apply_siena_stats(&mut modifiers, &mut contributions, equipment);
    let (mut stats, mut traces) = effective_stats(base, &modifiers);
    apply_pins(&mut stats, &mut traces, &sources.adjustments, None);
    Ok((stats, traces, contributions))
}

/// 最終能力値と装備から攻撃力(A)を内訳付きで出す。ダメージ計算(`calculate_damage`)と
/// 同じ `attack_power_breakdown` を通す(計算を二重に書かない)。
/// テシスコアは地域依存なのでキャラ画面では地域なし(`enhanced_totals(None)`)で出す。
fn attack_power_of(
    stats: &EffectiveStats,
    equipment: &Equipment,
    abilities: &[EquipmentAbilityDef],
    coefficients: &AttackPowerCoefficients,
) -> AttackPowerBreakdown {
    attack_power_breakdown(
        stat_attack_power(stats, &coefficients.stat),
        equipment_values_attack(&equipment.base_totals(abilities), &coefficients.equipment.base),
        equipment_values_attack(&equipment.enhanced_totals(None), &coefficients.equipment.enhanced),
        equipment.enhance_rate(),
    )
}

/// `BaseStats` + `StatSources` + 装備から最終能力値と(主軸スキルがあれば)攻撃力を組み立てる。
/// キャラ編集画面で「設定を触ると即時に再計算する」ために使う(保存はしない)。
///
/// `coefficients` はキャラの主軸スキルの依存種別から引いた係数。`None`(主軸スキル未選択)なら
/// 攻撃力は出さない。
pub fn preview_effective_stats(
    base: &BaseStats,
    sources: &StatSources,
    equipment: &Equipment,
    catalog: &BuffCatalog,
    abilities: &[EquipmentAbilityDef],
    game_character_id: &str,
    coefficients: Option<AttackPowerCoefficients>,
) -> Result<StatPreview, StatSourceError> {
    base.validate()?;
    sources.validate()?;
    equipment.validate()?;
    let (stats, traces, contributions) =
        effective_stats_with(base, sources, equipment, catalog, game_character_id)?;
    let attack = match coefficients {
        None => None,
        Some(coefficients) => {
            let breakdown = attack_power_of(&stats, equipment, abilities, &coefficients);
            // 部位を外すとシエナのオーラのステ加算も消える = 最終能力値まで動く。
            // 差分は「その装備を外した状態を丸ごと計算し直した A」との差にする。
            let mut part_contributions = Vec::with_capacity(12);
            for (slot, _) in equipment.parts.iter() {
                let without = equipment.without_part(slot);
                let (stats_without, _, _) =
                    effective_stats_with(base, sources, &without, catalog, game_character_id)?;
                let a_without = attack_power_of(&stats_without, &without, abilities, &coefficients);
                part_contributions
                    .push(PartAttackContribution { slot, value: breakdown.value - a_without.value });
            }
            Some(AttackPreview { breakdown, part_contributions })
        }
    };
    Ok(StatPreview { stats, traces, contributions, attack })
}

/// UI がリテラルで持たず参照するための値域上限一覧(起動時に 1 回取得する想定)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatLimits {
    pub base_stat_max: u32,
    pub rune_level_max: u8,
    pub crown_max: u32,
    pub sacred_relic_stage_max: u8,
    pub adjustment_add_min: i64,
    pub adjustment_add_max: i64,
    pub adjustment_pin_min: i64,
    pub adjustment_pin_max: i64,
    pub equipment_value_max: i64,
    pub strong_weapon_level_max: u8,
    /// 装備強化 Lv 上限(wiki: 装備システム/装備強化。+1〜+15)
    pub enhance_level_max: u8,
    pub enhance_added_damage_max: i64,
    /// シエナのオーラの増幅段階の上限(wiki: 装備システム/シエナのオーラ)
    pub siena_stage_max: u8,
    /// シエナのオーラの追加オプション「攻撃力増加」の 1 部位あたり上限 %
    pub siena_attack_rate_percent_max: f64,
    /// シエナのオーラの能力値スロットによるステ加算の 1 部位・1 ステあたり上限
    pub siena_stat_bonus_max: i64,
    /// シエナのオーラの追加オプション「全ステータス増加」の 1 部位あたり上限
    pub siena_all_stats_bonus_max: i64,
    /// テシスコアの装着枠数(wiki: テシスコア効果)
    pub core_slot_count: usize,
    pub core_evolution_max: u8,
    pub core_enhancement_max: u8,
    /// 装備 1 部位に付与できる属性値の上限(wiki: 装備システム/属性強化)
    pub equipment_element_value_max: i64,
    /// キャラの属性値の上限(wiki: 属性システム)
    pub element_value_max: i64,
}

pub fn stat_limits() -> StatLimits {
    StatLimits {
        base_stat_max: BASE_STAT_MAX,
        rune_level_max: RuneLevels::MAX_LEVEL,
        crown_max: Crown::MAX_VALUE,
        sacred_relic_stage_max: SacredRelic::MAX_STAGE,
        adjustment_add_min: ADJUSTMENT_ADD_MIN,
        adjustment_add_max: ADJUSTMENT_ADD_MAX,
        adjustment_pin_min: ADJUSTMENT_PIN_MIN,
        adjustment_pin_max: ADJUSTMENT_PIN_MAX,
        equipment_value_max: EQUIPMENT_VALUE_MAX,
        strong_weapon_level_max: STRONG_WEAPON_LEVEL_MAX,
        enhance_level_max: ENHANCE_LEVEL_MAX,
        enhance_added_damage_max: ENHANCE_ADDED_DAMAGE_MAX,
        siena_stage_max: SIENA_STAGE_MAX,
        siena_attack_rate_percent_max: SIENA_ATTACK_RATE_PERCENT_MAX,
        siena_stat_bonus_max: SIENA_STAT_BONUS_MAX,
        siena_all_stats_bonus_max: SIENA_ALL_STATS_BONUS_MAX,
        core_slot_count: CORE_SLOT_COUNT,
        core_evolution_max: CORE_EVOLUTION_MAX,
        core_enhancement_max: CORE_ENHANCEMENT_MAX,
        equipment_element_value_max: crate::element::EQUIPMENT_ELEMENT_VALUE_MAX,
        element_value_max: crate::element::ELEMENT_VALUE_MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::{effective_stat, BaseStats, BaseStatsError, BASE_STAT_MAX};

    /// domain は gamedata に依存できないため、テスト用に必要分だけ縮小したカタログを用意する。
    /// 値は gamedata::buffs::buff_catalog() の実データと一致させること。
    fn test_catalog() -> Vec<BuffDefinition> {
        vec![
            BuffDefinition {
                id: "illumination_drink",
                name: "イルミネーション祭りのドリンク",
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.30),
                exclusive_slots: vec!["percent_slot_1", "percent_slot_2"],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "charge_potion",
                name: "充填の秘薬",
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.20),
                exclusive_slots: vec!["percent_slot_1"],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "event_buff",
                name: "イベントバフ",
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Choice(vec![0.10, 0.20, 0.30, 0.50]),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "trust_potion",
                name: "改・信頼の薬",
                target: BuffTarget::AllStats,
                layer: StatLayer::Fixed,
                value: BuffValue::UserInput { min: 0.0, max: 33.0 },
                exclusive_slots: vec!["trust_potion"],
                source_url: "",
                note: "",
                default_value: Some(33.0),
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "club_effect",
                name: "クラブ効果",
                target: BuffTarget::UserSelected,
                layer: StatLayer::Fixed,
                value: BuffValue::Fixed(7.0),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "tales_weaver_energy",
                name: "テイルズウィーバーのエネルギー",
                target: BuffTarget::AllStats,
                layer: StatLayer::MultiplierA,
                value: BuffValue::Fixed(1.1),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "unleash",
                name: "アンリーシュ",
                target: BuffTarget::AllStats,
                layer: StatLayer::MultiplierB,
                value: BuffValue::Fixed(0.20),
                exclusive_slots: vec![],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
        ]
    }

    fn choice(id: &str) -> BuffChoice {
        BuffChoice { buff_id: id.to_string(), stat: None, choice_index: None, value: None }
    }

    // --- 1. 各補正源が正しいレイヤーに積まれること ---

    #[test]
    fn ペットスキルは固定値層に積まれる() {
        let sources = StatSources {
            pet_skills: PetSkills { stab: Some(PetSkillTier::Basic), ..Default::default() },
            ..Default::default()
        };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 20);
        let c = contributions.iter().find(|c| c.kind == StatKind::Stab).unwrap();
        assert_eq!(c.layer, StatLayer::Fixed);
        assert_eq!(c.value, 20.0);
    }

    #[test]
    fn ルーンスキルは固定値層に積まれる() {
        let sources = StatSources { rune_levels: RuneLevels { hack: 15, ..Default::default() }, ..Default::default() };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Hack).fixed, 15);
        let c = contributions.iter().find(|c| c.kind == StatKind::Hack).unwrap();
        assert_eq!(c.layer, StatLayer::Fixed);
    }

    #[test]
    fn クラウンは最終固定値層に積まれる() {
        let sources = StatSources { crown: Crown { def: 250, ..Default::default() }, ..Default::default() };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Def).final_fixed, 250);
        let c = contributions.iter().find(|c| c.kind == StatKind::Def).unwrap();
        assert_eq!(c.layer, StatLayer::FinalFixed);
    }

    #[test]
    fn 聖物は段階を10倍して最終固定値層に積まれる() {
        let sources = StatSources { sacred_relic: SacredRelic { mr: 12, ..Default::default() }, ..Default::default() };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Mr).final_fixed, 120);
        let c = contributions.iter().find(|c| c.kind == StatKind::Mr).unwrap();
        assert_eq!(c.layer, StatLayer::FinalFixed);
        assert_eq!(c.value, 120.0);
    }

    #[test]
    fn 調整値の加算は固定値層に積まれる() {
        let sources = StatSources {
            adjustments: Adjustments {
                dex: StatAdjustment { add: 7, pin: None },
                ..Default::default()
            },
            ..Default::default()
        };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Dex).fixed, 7);
        assert_eq!(contributions.iter().filter(|c| c.kind == StatKind::Dex).count(), 1);
    }

    // --- pin(能力値の固定) ---

    #[test]
    fn pinされたステはpinned_fromに元の値が残り最終能力値が固定される() {
        let sources = StatSources {
            adjustments: Adjustments {
                dex: StatAdjustment { add: 0, pin: Some(999) },
                ..Default::default()
            },
            ..Default::default()
        };
        let base = BaseStats { dex: 100, ..Default::default() };
        let (modifiers, _) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        let (mut stats, mut traces) = effective_stats(&base, &modifiers);
        apply_pins(&mut stats, &mut traces, &sources.adjustments, None);

        let dex_trace = traces.iter().find(|t| t.kind == StatKind::Dex).unwrap();
        assert_eq!(dex_trace.pinned_from, Some(100));
        assert_eq!(dex_trace.effective, 999);
        assert_eq!(dex_trace.pin_source, Some(PinSource::Saved));
        assert_eq!(stats.get(StatKind::Dex), 999);

        // pin していないステの pinned_from は None のまま
        let stab_trace = traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.pinned_from, None);
    }

    #[test]
    fn apply_pinsはtemporaryを優先しなければbaseにフォールバックし出所を記録する() {
        let base = Adjustments {
            stab: StatAdjustment { add: 0, pin: Some(100) },
            hack: StatAdjustment { add: 0, pin: Some(200) },
            ..Default::default()
        };
        let temporary =
            Adjustments { stab: StatAdjustment { add: 0, pin: Some(150) }, ..Default::default() };

        let base_stats = BaseStats { stab: 1, hack: 1, int: 1, ..Default::default() };
        let (mut stats, mut traces) = effective_stats(&base_stats, &StatModifierSet::default());
        apply_pins(&mut stats, &mut traces, &base, Some(&temporary));

        // temporary に pin があるステ(stab)はそちらが優先され、出所は Temporary
        let stab_trace = traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.effective, 150);
        assert_eq!(stab_trace.pinned_from, Some(1));
        assert_eq!(stab_trace.pin_source, Some(PinSource::Temporary));

        // temporary に pin が無いステ(hack)は base の pin が適用され、出所は Saved
        let hack_trace = traces.iter().find(|t| t.kind == StatKind::Hack).unwrap();
        assert_eq!(hack_trace.effective, 200);
        assert_eq!(hack_trace.pin_source, Some(PinSource::Saved));

        // どちらにも pin が無いステ(int)は pin_source が None のまま
        let int_trace = traces.iter().find(|t| t.kind == StatKind::Int).unwrap();
        assert_eq!(int_trace.pin_source, None);
        assert_eq!(int_trace.pinned_from, None);
    }

    #[test]
    fn preview_effective_statsはpin無しとpin有りの両方で正しい結果を返す() {
        // preview_effective_stats は base.validate() を呼ぶため、全ステが 1..=310 の範囲内である必要がある。
        let base = BaseStats { stab: 100, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };

        let sources = StatSources {
            adjustments: Adjustments { stab: StatAdjustment { add: 10, pin: None }, ..Default::default() },
            ..Default::default()
        };
        let preview = preview_effective_stats(&base, &sources, &Equipment::default(), &test_catalog(), &[], "boris", None).unwrap();
        assert!(preview
            .contributions
            .iter()
            .any(|c| c.kind == StatKind::Stab && c.source == "調整値"));
        let stab_trace = preview.traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.pinned_from, None);
        assert_eq!(preview.stats.get(StatKind::Stab), 110);

        let pinned_sources = StatSources {
            adjustments: Adjustments {
                stab: StatAdjustment { add: 10, pin: Some(500) },
                ..Default::default()
            },
            ..Default::default()
        };
        let pinned_preview = preview_effective_stats(&base, &pinned_sources, &Equipment::default(), &test_catalog(), &[], "boris", None).unwrap();
        assert!(pinned_preview
            .contributions
            .iter()
            .any(|c| c.kind == StatKind::Stab && c.source == "調整値"));
        let pinned_trace = pinned_preview.traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(pinned_trace.pinned_from, Some(110));
        assert_eq!(pinned_trace.effective, 500);
        assert_eq!(pinned_preview.stats.get(StatKind::Stab), 500);
    }

    /// 主軸スキル(StabHack 相当)の係数一式。値は gamedata::characters の実データに合わせる。
    fn test_attack_coefficients() -> AttackPowerCoefficients {
        use crate::equipment::EquipmentRates;
        AttackPowerCoefficients {
            stat: AttackCoefficients { primary: (StatKind::Stab, 1.8), secondary: (StatKind::Hack, 1.8) },
            equipment: EquipmentCoefficients {
                base: EquipmentRates { thrust: 14.5, slash: 14.5, magic_attack: 0.0, magic_defense: 0.0 },
                enhanced: EquipmentRates { thrust: 28.75, slash: 28.75, magic_attack: 0.0, magic_defense: 0.0 },
            },
        }
    }

    /// 武器(基本値・エンチャント・シエナのオーラのステ加算)と手(基本値のみ)を持つ装備。
    fn test_equipment() -> Equipment {
        use crate::equipment::{EquipmentPart, EquipmentParts, EquipmentValues, SienaAura};
        Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart {
                    base: EquipmentValues { thrust: 150, slash: 150, ..Default::default() },
                    enchant: EquipmentValues { thrust: 60, slash: 60, ..Default::default() },
                    siena: SienaAura {
                        stage: 3,
                        values: EquipmentValues { thrust: 20, slash: 20, ..Default::default() },
                        all_stats: 5,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                hand: EquipmentPart {
                    base: EquipmentValues { thrust: 30, slash: 30, ..Default::default() },
                    ..Default::default()
                },
                ..Default::default()
            },
            power_weapon: true,
            strong_weapon_level: 6,
            ..Default::default()
        }
    }

    #[test]
    fn 主軸スキル未選択なら攻撃力は出ない() {
        let base = BaseStats { stab: 100, hack: 100, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
        let preview = preview_effective_stats(
            &base, &StatSources::default(), &test_equipment(), &test_catalog(), &[], "boris", None,
        )
        .unwrap();
        assert!(preview.attack.is_none());
    }

    #[test]
    fn 攻撃力の内訳はステと装備基本と装備強化の和になる() {
        let base = BaseStats { stab: 100, hack: 100, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
        let equipment = test_equipment();
        let preview = preview_effective_stats(
            &base, &StatSources::default(), &equipment, &test_catalog(), &[],
            "boris", Some(test_attack_coefficients()),
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
    fn 部位の寄与は外したときの攻撃力との差に一致する() {
        let base = BaseStats { stab: 100, hack: 100, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
        let equipment = test_equipment();
        let sources = StatSources::default();
        let coefficients = test_attack_coefficients();
        let preview = preview_effective_stats(
            &base, &sources, &equipment, &test_catalog(), &[], "boris", Some(coefficients),
        )
        .unwrap();
        let attack = preview.attack.unwrap();

        for (slot, _) in equipment.parts.iter() {
            let without = equipment.without_part(slot);
            let preview_without = preview_effective_stats(
                &base, &sources, &without, &test_catalog(), &[], "boris", Some(coefficients),
            )
            .unwrap();
            let expected = attack.breakdown.value - preview_without.attack.unwrap().breakdown.value;
            let actual = attack.part_contributions.iter().find(|c| c.slot == slot).unwrap().value;
            assert_eq!(actual, expected, "{slot:?} の寄与が外したときの差と一致しない");
        }
        // 何も付いていない部位の寄与は 0、武器の寄与は正
        let weapon = attack.part_contributions.iter().find(|c| c.slot == PartSlot::Weapon).unwrap();
        let helm = attack.part_contributions.iter().find(|c| c.slot == PartSlot::Helm).unwrap();
        assert!(weapon.value > 0);
        assert_eq!(helm.value, 0);
    }

    #[test]
    fn バフは層ごとに正しく積まれる() {
        let sources = StatSources {
            buffs: BuffSelection {
                choices: vec![choice("illumination_drink"), choice("tales_weaver_energy"), choice("unleash")],
            },
            ..Default::default()
        };
        let (modifiers, _) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        let m = modifiers.get(StatKind::Stab);
        assert_eq!(m.percent_of_base, vec![0.30]);
        assert_eq!(m.multiplier_a, vec![1.1]);
        assert!((m.multiplier_b - 0.20).abs() < 1e-12);
    }

    #[test]
    fn バフの選択肢と手入力とユーザー選択ステが解決される() {
        let sources = StatSources {
            buffs: BuffSelection {
                choices: vec![
                    BuffChoice { buff_id: "event_buff".into(), stat: None, choice_index: Some(2), value: None },
                    BuffChoice { buff_id: "trust_potion".into(), stat: None, choice_index: None, value: Some(33.0) },
                    BuffChoice {
                        buff_id: "club_effect".into(),
                        stat: Some(StatKind::Agi),
                        choice_index: None,
                        value: None,
                    },
                ],
            },
            ..Default::default()
        };
        let (modifiers, _) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
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
            pet_skills: PetSkills { stab: Some(PetSkillTier::TrueLv2), ..Default::default() },
            ..Default::default()
        };
        let (modifiers, _) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 40);
        assert_ne!(modifiers.get(StatKind::Stab).fixed, 20 + 30 + 40);
    }

    // --- 3. 排他枠違反 ---

    #[test]
    fn 排他枠が重複するとエラーになる() {
        let sources = StatSources {
            buffs: BuffSelection { choices: vec![choice("illumination_drink"), choice("charge_potion")] },
            ..Default::default()
        };
        let err = build_modifiers(&sources, &test_catalog(), "boris").unwrap_err();
        assert!(matches!(err, StatSourceError::ExclusiveSlotConflict { slot } if slot == "percent_slot_1"));
    }

    #[test]
    fn 未知のバフidはエラーになる() {
        let sources = StatSources { buffs: BuffSelection { choices: vec![choice("nope")] }, ..Default::default() };
        let err = build_modifiers(&sources, &test_catalog(), "boris").unwrap_err();
        assert!(matches!(err, StatSourceError::UnknownBuff { id } if id == "nope"));
    }

    #[test]
    fn 同一buff_idを重複選択するとエラーになる() {
        // 排他枠が空の tales_weaver_energy を 2 回選んでも、排他枠チェックでは防げない
        // ことを確認しつつ、重複チェックで拒否されること。
        let sources = StatSources {
            buffs: BuffSelection {
                choices: vec![choice("tales_weaver_energy"), choice("tales_weaver_energy")],
            },
            ..Default::default()
        };
        let err = build_modifiers(&sources, &test_catalog(), "boris").unwrap_err();
        assert!(matches!(err, StatSourceError::DuplicateBuff { id } if id == "tales_weaver_energy"));
    }

    /// キャラスキル(`BuffGroup::CharacterSkill`)は所有者一致のときだけ許可する。
    fn character_skill_catalog() -> Vec<BuffDefinition> {
        vec![BuffDefinition {
            id: "boris_skill",
            name: "ボリスのスキル",
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(10.0),
            exclusive_slots: vec![],
            source_url: "",
            note: "",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "boris" },
        }]
    }

    #[test]
    fn キャラスキルは一致するキャラなら成功する() {
        let sources = StatSources { buffs: BuffSelection { choices: vec![choice("boris_skill")] }, ..Default::default() };
        let (modifiers, _) = build_modifiers(&sources, &character_skill_catalog(), "boris").unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 10);
    }

    #[test]
    fn キャラスキルは異なるキャラだとエラーになる() {
        let sources = StatSources { buffs: BuffSelection { choices: vec![choice("boris_skill")] }, ..Default::default() };
        let err = build_modifiers(&sources, &character_skill_catalog(), "other_character").unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::ForeignCharacterSkill { id, game_character_id }
                if id == "boris_skill" && game_character_id == "other_character"
        ));
    }

    #[test]
    fn 手入力値が範囲外だとエラーになり境界値は成功する() {
        let over = StatSources {
            buffs: BuffSelection {
                choices: vec![BuffChoice { buff_id: "trust_potion".into(), stat: None, choice_index: None, value: Some(34.0) }],
            },
            ..Default::default()
        };
        let err = build_modifiers(&over, &test_catalog(), "boris").unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::ValueOutOfRange { id, value, min, max }
                if id == "trust_potion" && value == 34.0 && min == 0.0 && max == 33.0
        ));

        let boundary = StatSources {
            buffs: BuffSelection {
                choices: vec![BuffChoice { buff_id: "trust_potion".into(), stat: None, choice_index: None, value: Some(33.0) }],
            },
            ..Default::default()
        };
        assert!(build_modifiers(&boundary, &test_catalog(), "boris").is_ok());
    }

    // --- 3.5. StatSources::validate() が各補正源の値域を拒否する ---

    #[test]
    fn ルーンスキルは0から20の範囲外を拒否する() {
        let mut sources = StatSources { rune_levels: RuneLevels { stab: RuneLevels::MAX_LEVEL, ..Default::default() }, ..Default::default() };
        assert!(sources.validate().is_ok());
        sources.rune_levels.stab = RuneLevels::MAX_LEVEL + 1;
        let err = sources.validate().unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::OutOfRange { source_name: "ルーンスキル", kind: StatKind::Stab, value: 21, max: 20 }
        ));
    }

    #[test]
    fn クラウンは0から300の範囲外を拒否する() {
        let mut sources = StatSources { crown: Crown { hack: Crown::MAX_VALUE, ..Default::default() }, ..Default::default() };
        assert!(sources.validate().is_ok());
        sources.crown.hack = Crown::MAX_VALUE + 1;
        let err = sources.validate().unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::OutOfRange { source_name: "クラウン", kind: StatKind::Hack, value: 301, max: 300 }
        ));
    }

    #[test]
    fn 聖物は0から40段階の範囲外を拒否する() {
        let mut sources =
            StatSources { sacred_relic: SacredRelic { mr: SacredRelic::MAX_STAGE, ..Default::default() }, ..Default::default() };
        assert!(sources.validate().is_ok());
        sources.sacred_relic.mr = SacredRelic::MAX_STAGE + 1;
        let err = sources.validate().unwrap_err();
        assert!(matches!(
            err,
            StatSourceError::OutOfRange { source_name: "神鳥の聖物", kind: StatKind::Mr, value: 41, max: 40 }
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
            Err(StatSourceError::AdjustmentOutOfRange { field: "加算", kind: StatKind::Stab, .. })
        ));

        let mut too_high = adjustments;
        too_high.hack.add = ADJUSTMENT_ADD_MAX + 1;
        assert!(matches!(
            too_high.validate(),
            Err(StatSourceError::AdjustmentOutOfRange { field: "加算", kind: StatKind::Hack, .. })
        ));

        let mut pin_ok = Adjustments::default();
        pin_ok.stab.pin = Some(ADJUSTMENT_PIN_MIN);
        pin_ok.hack.pin = Some(ADJUSTMENT_PIN_MAX);
        assert!(pin_ok.validate().is_ok());

        let mut pin_too_low = Adjustments::default();
        pin_too_low.stab.pin = Some(ADJUSTMENT_PIN_MIN - 1);
        assert!(matches!(
            pin_too_low.validate(),
            Err(StatSourceError::AdjustmentOutOfRange { field: "固定", kind: StatKind::Stab, .. })
        ));

        let mut pin_too_high = Adjustments::default();
        pin_too_high.hack.pin = Some(ADJUSTMENT_PIN_MAX + 1);
        assert!(matches!(
            pin_too_high.validate(),
            Err(StatSourceError::AdjustmentOutOfRange { field: "固定", kind: StatKind::Hack, .. })
        ));
    }

    // --- 4. BaseStats::validate() が 1..=310 の範囲外を拒否する ---

    #[test]
    fn 素ステは1から310の範囲外を拒否する() {
        let mut base = BaseStats { stab: BASE_STAT_MAX, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
        assert!(base.validate().is_ok());
        base.stab = 0;
        assert!(matches!(base.validate(), Err(BaseStatsError::OutOfRange { value: 0, .. })));
        base.stab = BASE_STAT_MAX + 1;
        assert!(matches!(base.validate(), Err(BaseStatsError::OutOfRange { value: 311, .. })));
    }

    // --- 4.5. apply_temporary_adjustments(計算リクエストにのみ乗る一時調整) ---

    #[test]
    fn 一時調整の加算は固定値に積まれてsourceが一時調整になる() {
        let sources = StatSources {
            adjustments: Adjustments { mr: StatAdjustment { add: 12, pin: None }, ..Default::default() },
            ..Default::default()
        };
        let (mut modifiers, mut contributions) = build_modifiers(&StatSources::default(), &test_catalog(), "boris").unwrap();
        apply_temporary_adjustments(&mut modifiers, &mut contributions, &sources.adjustments);

        assert_eq!(modifiers.get(StatKind::Mr).fixed, 12);
        let rows: Vec<_> = contributions.iter().filter(|c| c.kind == StatKind::Mr).collect();
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|c| c.source == "一時調整"));
    }

    #[test]
    fn 中立な一時調整は何も積まない() {
        let (mut modifiers, mut contributions) = build_modifiers(&StatSources::default(), &test_catalog(), "boris").unwrap();
        let before_contributions = contributions.len();
        apply_temporary_adjustments(&mut modifiers, &mut contributions, &Adjustments::default());

        assert_eq!(contributions.len(), before_contributions);
        for kind in StatKind::ALL {
            assert_eq!(modifiers.get(kind).fixed, 0);
            assert_eq!(modifiers.get(kind).final_fixed, 0);
        }
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
            pet_skills: PetSkills { stab: Some(PetSkillTier::TrueLv4), ..Default::default() },
            rune_levels: RuneLevels { stab: 20, ..Default::default() },
            sacred_relic: SacredRelic { stab: 40, ..Default::default() },
            buffs: BuffSelection {
                choices: vec![choice("tales_weaver_energy"), choice("unleash")],
            },
            ..Default::default()
        };
        let (modifiers, _) = build_modifiers(&sources, &test_catalog(), "boris").unwrap();
        let base = BaseStats { stab: 310, ..Default::default() };
        let (value, trace) = effective_stat(StatKind::Stab, base.stab, modifiers.get(StatKind::Stab));
        assert_eq!(trace.basic, 429);
        assert_eq!(trace.multiplier_b_bonus, 85);
        assert_eq!(value, 914);
    }

    #[test]
    fn stat_limitsは対応する定数と一致する() {
        let limits = stat_limits();
        assert_eq!(limits.base_stat_max, BASE_STAT_MAX);
        assert_eq!(limits.rune_level_max, RuneLevels::MAX_LEVEL);
        assert_eq!(limits.crown_max, Crown::MAX_VALUE);
        assert_eq!(limits.sacred_relic_stage_max, SacredRelic::MAX_STAGE);
        assert_eq!(limits.adjustment_add_min, ADJUSTMENT_ADD_MIN);
        assert_eq!(limits.adjustment_add_max, ADJUSTMENT_ADD_MAX);
        assert_eq!(limits.adjustment_pin_min, ADJUSTMENT_PIN_MIN);
        assert_eq!(limits.adjustment_pin_max, ADJUSTMENT_PIN_MAX);
        assert_eq!(limits.equipment_value_max, EQUIPMENT_VALUE_MAX);
        assert_eq!(limits.strong_weapon_level_max, STRONG_WEAPON_LEVEL_MAX);
        assert_eq!(limits.enhance_level_max, crate::equipment::ENHANCE_LEVEL_MAX);
        assert_eq!(limits.enhance_added_damage_max, crate::equipment::ENHANCE_ADDED_DAMAGE_MAX);
    }
}
