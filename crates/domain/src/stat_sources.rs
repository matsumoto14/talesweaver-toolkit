//! キャラクターの実効ステータスに効く恒常補正源(ペット・ルーン・クラウン・聖物)と常用バフ。
//!
//! docs/goals/2026-08-21-character-stat-sources.md。バフは個別にコードで分岐せず、
//! 「カテゴリ(層)+ 数値 + 重複枠」を持つデータ(`BuffDefinition`)として解決する
//! (CLAUDE.md 原則、crates/domain/src/category.rs の設計思想を踏襲)。
//! カタログの実データ(常用バフ 16 件)は gamedata に置く。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stats::{effective_stats, BaseStats, EffectiveStats, StatKind, StatModifierSet, StatTrace};

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
}

/// キャラクターに紐づく補正源一式。`Default` は全フィールド中立
/// (ペット無し、ルーン 0、クラウン 0、聖物 0 段階、バフ無し、調整値 0)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StatSources {
    pub pet_skills: PetSkills,
    pub rune_levels: RuneLevels,
    pub crown: Crown,
    pub sacred_relic: SacredRelic,
    pub buffs: BuffSelection,
    pub adjustments: Adjustments,
}

impl StatSources {
    /// ルーンスキル(0..=20)/クラウン(0..=300)/聖物(0..=40段階)の値域を検証する。
    /// ペットは enum で構造的に制約済み、調整値は検証・未収録バフ用の自由加算なので対象外。
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
}

/// `StatSources` と バフカタログから `StatModifierSet` と寄与内訳を組み立てる。
pub fn build_modifiers(
    sources: &StatSources,
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
/// `pin` はここでは扱わない(呼び出し側が `merge_pins` でキャラの調整値と合成し、`apply_pins` で適用する)。
pub fn apply_temporary_adjustments(
    modifiers: &mut StatModifierSet,
    contributions: &mut Vec<StatContribution>,
    adjustments: &Adjustments,
) {
    apply_adjustments(modifiers, contributions, adjustments, "一時調整");
}

/// 調整の「固定(pin)」を反映する。対象ステの `StatTrace.pinned_from` に上書き前の値を保存してから、
/// `stats`/`trace.effective` を pin 値で上書きする。能力値計算(`effective_stats`)の後に呼ぶ。
pub fn apply_pins(stats: &mut EffectiveStats, traces: &mut [StatTrace], adjustments: &Adjustments) {
    for kind in StatKind::ALL {
        if let Some(pin) = adjustments.get(kind).pin {
            if let Some(trace) = traces.iter_mut().find(|t| t.kind == kind) {
                trace.pinned_from = Some(trace.effective);
                trace.effective = pin;
            }
            stats.set(kind, pin);
        }
    }
}

/// キャラの調整値(`base`)と計算リクエストの一時調整(`temporary`)から pin だけを合成する。
/// ステごとに temporary 側の pin があればそちらを優先し、無ければ base 側を使う。`add` は使わない。
pub fn merge_pins(base: &Adjustments, temporary: &Adjustments) -> Adjustments {
    let mut merged = Adjustments::default();
    for kind in StatKind::ALL {
        merged.get_mut(kind).pin = temporary.get(kind).pin.or(base.get(kind).pin);
    }
    merged
}

/// `preview_effective_stats` の結果(最終能力値・トレース・寄与内訳)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatPreview {
    pub stats: EffectiveStats,
    pub traces: Vec<StatTrace>,
    pub contributions: Vec<StatContribution>,
}

/// `BaseStats` + `StatSources` から最終能力値を組み立てる(pin 込み)。
/// キャラ編集画面で「設定を触ると即時に最終能力値を再計算する」ために使う(保存はしない)。
pub fn preview_effective_stats(
    base: &BaseStats,
    sources: &StatSources,
    catalog: &BuffCatalog,
) -> Result<StatPreview, StatSourceError> {
    let (modifiers, contributions) = build_modifiers(sources, catalog)?;
    let (mut stats, mut traces) = effective_stats(base, &modifiers);
    apply_pins(&mut stats, &mut traces, &sources.adjustments);
    Ok(StatPreview { stats, traces, contributions })
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
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 20);
        let c = contributions.iter().find(|c| c.kind == StatKind::Stab).unwrap();
        assert_eq!(c.layer, StatLayer::Fixed);
        assert_eq!(c.value, 20.0);
    }

    #[test]
    fn ルーンスキルは固定値層に積まれる() {
        let sources = StatSources { rune_levels: RuneLevels { hack: 15, ..Default::default() }, ..Default::default() };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Hack).fixed, 15);
        let c = contributions.iter().find(|c| c.kind == StatKind::Hack).unwrap();
        assert_eq!(c.layer, StatLayer::Fixed);
    }

    #[test]
    fn クラウンは最終固定値層に積まれる() {
        let sources = StatSources { crown: Crown { def: 250, ..Default::default() }, ..Default::default() };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog()).unwrap();
        assert_eq!(modifiers.get(StatKind::Def).final_fixed, 250);
        let c = contributions.iter().find(|c| c.kind == StatKind::Def).unwrap();
        assert_eq!(c.layer, StatLayer::FinalFixed);
    }

    #[test]
    fn 聖物は段階を10倍して最終固定値層に積まれる() {
        let sources = StatSources { sacred_relic: SacredRelic { mr: 12, ..Default::default() }, ..Default::default() };
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog()).unwrap();
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
        let (modifiers, contributions) = build_modifiers(&sources, &test_catalog()).unwrap();
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
        let (modifiers, _) = build_modifiers(&sources, &test_catalog()).unwrap();
        let (mut stats, mut traces) = effective_stats(&base, &modifiers);
        apply_pins(&mut stats, &mut traces, &sources.adjustments);

        let dex_trace = traces.iter().find(|t| t.kind == StatKind::Dex).unwrap();
        assert_eq!(dex_trace.pinned_from, Some(100));
        assert_eq!(dex_trace.effective, 999);
        assert_eq!(stats.get(StatKind::Dex), 999);

        // pin していないステの pinned_from は None のまま
        let stab_trace = traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.pinned_from, None);
    }

    #[test]
    fn merge_pinsはtemporaryを優先しなければbaseにフォールバックする() {
        let base = Adjustments {
            stab: StatAdjustment { add: 0, pin: Some(100) },
            hack: StatAdjustment { add: 0, pin: Some(200) },
            ..Default::default()
        };
        let temporary =
            Adjustments { stab: StatAdjustment { add: 0, pin: Some(150) }, ..Default::default() };

        let merged = merge_pins(&base, &temporary);
        assert_eq!(merged.get(StatKind::Stab).pin, Some(150));
        assert_eq!(merged.get(StatKind::Hack).pin, Some(200));
        assert_eq!(merged.get(StatKind::Int).pin, None);
    }

    #[test]
    fn preview_effective_statsはpin無しとpin有りの両方で正しい結果を返す() {
        let base = BaseStats { stab: 100, ..Default::default() };

        let sources = StatSources {
            adjustments: Adjustments { stab: StatAdjustment { add: 10, pin: None }, ..Default::default() },
            ..Default::default()
        };
        let preview = preview_effective_stats(&base, &sources, &test_catalog()).unwrap();
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
        let pinned_preview = preview_effective_stats(&base, &pinned_sources, &test_catalog()).unwrap();
        assert!(pinned_preview
            .contributions
            .iter()
            .any(|c| c.kind == StatKind::Stab && c.source == "調整値"));
        let pinned_trace = pinned_preview.traces.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(pinned_trace.pinned_from, Some(110));
        assert_eq!(pinned_trace.effective, 500);
        assert_eq!(pinned_preview.stats.get(StatKind::Stab), 500);
    }

    #[test]
    fn バフは層ごとに正しく積まれる() {
        let sources = StatSources {
            buffs: BuffSelection {
                choices: vec![choice("illumination_drink"), choice("tales_weaver_energy"), choice("unleash")],
            },
            ..Default::default()
        };
        let (modifiers, _) = build_modifiers(&sources, &test_catalog()).unwrap();
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
        let (modifiers, _) = build_modifiers(&sources, &test_catalog()).unwrap();
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
        let (modifiers, _) = build_modifiers(&sources, &test_catalog()).unwrap();
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
        let err = build_modifiers(&sources, &test_catalog()).unwrap_err();
        assert!(matches!(err, StatSourceError::ExclusiveSlotConflict { slot } if slot == "percent_slot_1"));
    }

    #[test]
    fn 未知のバフidはエラーになる() {
        let sources = StatSources { buffs: BuffSelection { choices: vec![choice("nope")] }, ..Default::default() };
        let err = build_modifiers(&sources, &test_catalog()).unwrap_err();
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
        let err = build_modifiers(&sources, &test_catalog()).unwrap_err();
        assert!(matches!(err, StatSourceError::DuplicateBuff { id } if id == "tales_weaver_energy"));
    }

    #[test]
    fn 手入力値が範囲外だとエラーになり境界値は成功する() {
        let over = StatSources {
            buffs: BuffSelection {
                choices: vec![BuffChoice { buff_id: "trust_potion".into(), stat: None, choice_index: None, value: Some(34.0) }],
            },
            ..Default::default()
        };
        let err = build_modifiers(&over, &test_catalog()).unwrap_err();
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
        assert!(build_modifiers(&boundary, &test_catalog()).is_ok());
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
        let (mut modifiers, mut contributions) = build_modifiers(&StatSources::default(), &test_catalog()).unwrap();
        apply_temporary_adjustments(&mut modifiers, &mut contributions, &sources.adjustments);

        assert_eq!(modifiers.get(StatKind::Mr).fixed, 12);
        let rows: Vec<_> = contributions.iter().filter(|c| c.kind == StatKind::Mr).collect();
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|c| c.source == "一時調整"));
    }

    #[test]
    fn 中立な一時調整は何も積まない() {
        let (mut modifiers, mut contributions) = build_modifiers(&StatSources::default(), &test_catalog()).unwrap();
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
        let (modifiers, _) = build_modifiers(&sources, &test_catalog()).unwrap();
        let base = BaseStats { stab: 310, ..Default::default() };
        let (value, trace) = effective_stat(StatKind::Stab, base.stab, modifiers.get(StatKind::Stab));
        assert_eq!(trace.basic, 429);
        assert_eq!(trace.multiplier_b_bonus, 85);
        assert_eq!(value, 914);
    }
}
