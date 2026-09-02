//! 防御側の戦闘能力値(docs/damage-formula.md §6)、カット率 J(§4 カテゴリJ)、回避(§7)。
//!
//! 与ダメージ式(`damage`)とは別の経路。ここで出すのは「自分がどれだけ耐えるか」で、
//! 攻撃力(A)と違って与ダメージには入らない。
//!
//! 回避の出典は wiki 計算式まとめ `#HitRate` / `#EvasionPoint` / `#HitRateCap`、
//! ステータス「命中率/回避率」(取得 2026-08-25)。
//!
//! 未実装で値を出せない項目は `Option` の `None` にする。0 と区別できないと
//! 「防御力 0」なのか「まだ計算していない」なのか画面で判断できないため。

use serde::{Deserialize, Serialize};

use crate::character_skill::{CharacterSkillDef, SkillEffect};

use crate::awakening::AwakeningCaps;
use crate::common_skill::DefenseRates;
use crate::equipment::{Equipment, EquipmentValues, PartSlot};
use crate::random_option::RandomOptionTotals;
use crate::rounding::floor_int;
use crate::siena::{SienaValueKind, SIENA_STAGE_MAX};
use crate::stats::{EffectiveStats, StatKind};

/// カット率 J の分母定数(wiki カテゴリJ: `r = 1 − a/(a+80)`)。
pub const CUT_RATE_DENOMINATOR: f64 = 80.0;
/// カット率 J の `a` の定数項(wiki カテゴリJ: `a = 3 + [(合計 − 1) / 除数]`)。
pub const CUT_RATE_A_BASE: f64 = 3.0;
/// カット率 J の `a` の除数(物理 / 魔法)。
pub const CUT_RATE_DIVISOR: f64 = 10.0;
/// カット率 J の `a` の除数(複合)。
pub const CUT_RATE_COMPOSITE_DIVISOR: f64 = 20.0;
/// 防御力(物理 / 魔法)のステ係数(wiki 戦闘能力値: `DEF*3 + 装備物防*倍率*6`)。
pub const DEFENSE_STAT_MULTIPLIER: f64 = 3.0;
/// 防御力(物理 / 魔法)の装備係数。
pub const DEFENSE_EQUIPMENT_MULTIPLIER: f64 = 6.0;
/// 複合防御力のステ係数(wiki 戦闘能力値: `(DEF+MR)*1.5 + 装備×3`)。
pub const COMPOSITE_DEFENSE_STAT_MULTIPLIER: f64 = 1.5;
/// 複合防御力の装備係数。
pub const COMPOSITE_DEFENSE_EQUIPMENT_MULTIPLIER: f64 = 3.0;

/// 特殊回避(コンボ回避)の下限・上限 %(wiki §7)。
const COMBO_EVASION_MIN_PERCENT: f64 = 20.0;
const COMBO_EVASION_MAX_PERCENT: f64 = 63.0;

/// 回避P の定数項と AGI 係数(wiki `#EvasionPoint`:
/// `回避P = [15 + (AGI + 装備回避率)*1.2 + 装備敏捷度/7 + 回避P増加 + 攻撃タイプに応じた回避P増加]`)。
pub const EVASION_POINT_BASE: f64 = 15.0;
pub const EVASION_POINT_AGI_RATE: f64 = 1.2;
/// 攻撃タイプ別 回避P増加の共通除数(wiki `#EvasionPoint`)。
pub const EVASION_TYPE_DIVISOR: f64 = 7.0;
/// 物理の回避P増加に入る `[(STAB+HACK)/100]` の除数。
pub const EVASION_PHYSICAL_ATTACK_DIVISOR: f64 = 100.0;

/// 攻撃タイプ別の回避P(wiki `#EvasionPoint`)。敵の攻撃タイプに合わせた回避Pが要る。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvasionPoints {
    /// 物理。回避P増加 `(DEF*2 + [(STAB+HACK)/100]) / 7`
    pub physical: i64,
    /// 魔法。回避P増加 `(MR*2) / 7`
    pub magic: i64,
    /// 複合。回避P増加 `(DEF+MR) / 7`
    pub composite: i64,
}

impl EvasionPoints {
    /// 突き合わせる攻撃タイプに応じた回避Pを 1 つ選ぶ(対人の命中率判定)。
    pub fn for_attack_type(&self, attack_type: AttackType) -> i64 {
        match attack_type {
            AttackType::Physical => self.physical,
            AttackType::Magic => self.magic,
        }
    }
}

/// 命中判定で突き合わせる回避Pの種類(wiki `#EvasionPoint`「攻撃タイプに応じた回避P増加」)。
///
/// wiki は 物理/魔法/複合 の 3 分類だが、スキル依存種別(`SkillDependency`)から複合を
/// 判定する規則が wiki に無い。`RandomOptionTotals::damage_amplify_for` が依存種別を
/// 物理/魔法へ振り分けているのと同じ規則にそろえ、ここでは 2 分類とする `[仮]`
/// (突き / 斬り / 突き斬り = 物理、知力 / 魔防 / 斬り知力 = 魔法。複合は未対応)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackType {
    Physical,
    Magic,
}

/// 攻撃タイプに応じた回避P増加(wiki `#EvasionPoint`)。物理 `(DEF*2 + [(STAB+HACK)/100]) / 7`、
/// 魔法 `(MR*2) / 7`。`evasion_point` に渡す `type_bonus` と同じ式(表示用の内訳に使う)。
pub fn attack_type_bonus(stats: &EffectiveStats, attack_type: AttackType) -> f64 {
    match attack_type {
        AttackType::Physical => {
            (stats.def as f64 * 2.0
                + floor_int((stats.stab + stats.hack) as f64 / EVASION_PHYSICAL_ATTACK_DIVISOR)
                    as f64)
                / EVASION_TYPE_DIVISOR
        }
        AttackType::Magic => stats.mr as f64 * 2.0 / EVASION_TYPE_DIVISOR,
    }
}

/// 防御側の戦闘能力値一式。割合(カット率・回避)は小数表現(50% → 0.5)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DefenseProfile {
    /// 物理防御力 `[DEF*3 + 装備物防 * 倍率 * 6]`(上限適用後)
    pub physical_defense: i64,
    /// 魔法防御力 `[MR*3 + 装備魔防 * 倍率 * 6]`(上限適用後)
    pub magic_defense: i64,
    /// 複合防御力 `[(DEF+MR)*1.5 + (装備物防*倍率 + 装備魔防*倍率) * 3]`(上限適用後)
    pub composite_defense: i64,
    /// 防御力の上限(覚醒段階とエタの意志 Lv で決まる。wiki: Quest/覚醒クエスト・エタの意志)
    pub defense_cap: i64,
    /// 上限で捨てられた分(物理 / 魔法 / 複合)。すべて 0 なら上限に当たっていない
    pub physical_defense_loss: i64,
    pub magic_defense_loss: i64,
    pub composite_defense_loss: i64,
    /// カット率 J(物理)`r = 1 − a/(a+80)`、`a = 3 + [(DEF+装備物防−1)/10]`
    pub physical_cut_rate: f64,
    /// カット率 J(魔法)。`a` は MR 版
    pub magic_cut_rate: f64,
    /// カット率 J(複合)。`a = 3 + [(DEF+装備物防+MR+装備魔防−1)/20]`
    pub composite_cut_rate: f64,
    /// 特殊回避(コンボ回避)`(10 + MR/15 + AGI/7.5)%`、下限 20% / 上限 63%
    pub combo_evasion: f64,
    /// 攻撃タイプ別の回避P。通常回避「率」は敵の命中Pが要り、その入力(wiki 狩り場情報一覧の
    /// 「上限回避P」)が全行未記載なので出さない(ユーザー決定 2026-08-25)
    pub evasion_point: EvasionPoints,
    /// 装備物防(基本能力値 + 強化能力値の合計)
    pub equipment_physical_defense: i64,
    /// 装備魔防(基本能力値 + 強化能力値の合計)
    pub equipment_magic_defense: i64,
    /// 装備回避率補正(基本能力値 + 強化能力値の合計)
    pub equipment_evasion: i64,
    /// 装備敏捷度補正(基本能力値 + 強化能力値の合計)
    pub equipment_agility: i64,
    /// 適用した装備防御力倍率(共通スキル + シエナのオーラ)。UI が「何倍で計算したか」を出す
    pub defense_rates: DefenseRates,
}

/// カット率 J。`a` から `r = 1 − a/(a+80)`。
fn cut_rate(a: f64) -> f64 {
    1.0 - a / (a + CUT_RATE_DENOMINATOR)
}

/// カット率 J の `a`。`3 + [(合計 − 1) / 除数]`。
fn cut_rate_a(sum: i64, divisor: f64) -> f64 {
    CUT_RATE_A_BASE + floor_int((sum - 1) as f64 / divisor) as f64
}

/// 回避P。`type_bonus` は攻撃タイプに応じた回避P増加、`random_option` はランダムオプションの
/// 「回避率が X 増加」の合計。
///
/// 回避P増加(バフ)はバフカタログが「回避率+x%」のバフを持たないので 0。
fn evasion_point(
    stats: &EffectiveStats,
    equipment: &EquipmentValues,
    type_bonus: f64,
    random_option: i64,
) -> i64 {
    floor_int(
        EVASION_POINT_BASE
            + (stats.agi + equipment.evasion) as f64 * EVASION_POINT_AGI_RATE
            + equipment.agility as f64 / EVASION_TYPE_DIVISOR
            + random_option as f64
            + type_bonus,
    )
}

/// スキル依存種別ごとの命中P補正の係数(wiki 計算式まとめ の依存表「命中P補正」)。
/// 値は gamedata が持ち、ここは形と評価だけ。ペナルティは最大 2 ステの和(表のいちばん広い行が
/// `(INT+HACK)/250`)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccuracyCorrection {
    /// 依存ボーナス = `stat(kind) × rate`。ボーナスが無い依存は `None`
    pub bonus: Option<(StatKind, f64)>,
    /// 依存ペナルティ = `(stat(primary) + stat(secondary)) / divisor`
    pub penalty_primary: StatKind,
    pub penalty_secondary: Option<StatKind>,
    pub penalty_divisor: f64,
}

impl AccuracyCorrection {
    /// 依存ボーナス(切り捨て前)。
    pub fn bonus_value(&self, stats: &EffectiveStats) -> f64 {
        self.bonus
            .map_or(0.0, |(kind, rate)| stats.get(kind) as f64 * rate)
    }

    /// 依存ペナルティ(切り捨て前)。
    pub fn penalty_value(&self, stats: &EffectiveStats) -> f64 {
        let sum =
            stats.get(self.penalty_primary) + self.penalty_secondary.map_or(0, |k| stats.get(k));
        sum as f64 / self.penalty_divisor
    }
}

/// wiki のスキル命中は実測から 15 引いた値が載っている(wiki `#AccuracyPoint` の赤字注記:
/// 「初期の命中解析が間違えてため、現状まで引きずっている。特に的中剣の計算式が一番影響を
/// 受けていた」)。gamedata は wiki の表記どおり持ち、計算時にここで戻す。
pub const SKILL_ACCURACY_OFFSET: i64 = 15;
/// ペット集中の命中P割合増加(wiki `#AccuracyPoint`: +5%。的中剣 Lv1 相当)。
pub const CONCENTRATION_ACCURACY_RATE: f64 = 1.05;
/// ペット集中の固定の命中P変動(wiki `#AccuracyPoint` の表。的中剣 Lv1 の行と共通 = +3)。
pub const CONCENTRATION_ACCURACY_SHIFT: i64 = 3;
/// 感電・雷電の命中P割合減少(同: −30%)。
pub const SHOCK_ACCURACY_RATE: f64 = 0.70;

/// 命中P割合増加の枠の出どころ。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccuracyBoostSource {
    None,
    /// ペット集中(wiki `PET`: 「集中が優先されて的中剣が無効」)
    Concentration,
    /// 命中P割合増加を持つキャラスキル(`SkillEffect::AccuracyRate`。極・的中剣)を SLv で解決したもの
    Skill {
        id: &'static str,
        name: &'static str,
        level: u8,
        max_level: u8,
    },
}

/// 命中P割合増加の枠(wiki `#AccuracyPoint`)を解決した値。**集中と的中剣はいずれか 1 つだけ**
/// 適用され、優先度は 集中 > 的中剣(2024/7/4 以降も変化なし)。どちらも割合とは別に固定の
/// 命中P変動(`shift`)を持つ。
///
/// 的中剣はキャラスキル(`SkillEffect::AccuracyRate`)であって装着アビリティではない。
/// 装着アビリティ側の「命中率補正 +n」は `EquipmentValues.accuracy` に入る別物。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AccuracyBoost {
    /// 命中Pに掛かる倍率(中立 1.0)
    pub rate: f64,
    /// 割合とは別に乗る固定の命中P変動。wiki は「追加で命中Pが変動する模様(誤差にしては
    /// 大きすぎる。原因不明)」として割合増加の節に表だけ載せており、掛け算の内か外かは
    /// 書いていない。±3 なので影響は小さいが、**外(割合を掛けたあと)**として扱う `[仮]`
    pub shift: i64,
    pub source: AccuracyBoostSource,
}

impl Default for AccuracyBoost {
    fn default() -> Self {
        AccuracyBoost::NONE
    }
}

impl AccuracyBoost {
    pub const NONE: AccuracyBoost = AccuracyBoost {
        rate: 1.0,
        shift: 0,
        source: AccuracyBoostSource::None,
    };

    pub fn concentration() -> AccuracyBoost {
        AccuracyBoost {
            rate: CONCENTRATION_ACCURACY_RATE,
            shift: CONCENTRATION_ACCURACY_SHIFT,
            source: AccuracyBoostSource::Concentration,
        }
    }

    /// 命中P割合増加を持つスキルを SLv で解決する。Lv 0(未習得)は `NONE`。
    /// スキルがその効果を持たなければ `None`
    pub fn from_skill(def: &CharacterSkillDef, level: u8) -> Option<AccuracyBoost> {
        def.effects.iter().find_map(|e| match e {
            SkillEffect::AccuracyRate { per_level, shift } => Some(AccuracyBoost::from_rate_skill(
                def.id,
                def.name,
                *per_level,
                shift,
                level,
                def.max_level,
            )),
            _ => None,
        })
    }

    /// 倍率 `1 + per_level × Lv`、固定変動 `shift[Lv-1]`(表が短ければ最後の値)。
    pub fn from_rate_skill(
        id: &'static str,
        name: &'static str,
        per_level: f64,
        shift: &[i64],
        level: u8,
        max_level: u8,
    ) -> AccuracyBoost {
        let level = level.min(max_level);
        if level == 0 {
            return AccuracyBoost::NONE;
        }
        AccuracyBoost {
            rate: 1.0 + f64::from(level) * per_level,
            shift: shift
                .get(usize::from(level) - 1)
                .or(shift.last())
                .copied()
                .unwrap_or(0),
            source: AccuracyBoostSource::Skill {
                id,
                name,
                level,
                max_level,
            },
        }
    }

    /// 集中(ペット)と的中剣(スキル)の優先度を解決する(集中 > 的中剣。wiki `PET`)。
    pub fn resolve(concentration: bool, skill: Option<AccuracyBoost>) -> AccuracyBoost {
        if concentration {
            return AccuracyBoost::concentration();
        }
        skill.unwrap_or(AccuracyBoost::NONE)
    }

    pub fn rate(self) -> f64 {
        self.rate
    }

    pub fn shift(self) -> i64 {
        self.shift
    }

    /// 出どころがキャラスキルならその id。
    pub fn skill_id(&self) -> Option<&'static str> {
        match self.source {
            AccuracyBoostSource::Skill { id, .. } => Some(id),
            _ => None,
        }
    }
}

/// 命中P(wiki `#AccuracyPoint`)。
///
/// `命中P = [(DEX + 装備命中率補正 + (スキル命中 + 15) + 依存ボーナス − 依存ペナルティ
///          + 命中P増加) × 命中P割合増加 × 命中P割合減少 + ランダムOP]`
///
/// `skill_accuracy` は wiki 表記のまま渡す(`SKILL_ACCURACY_OFFSET` はここで足す)。
/// `bonus` は命中P増加の合計(射手のルーン +20・ハードウエポン +15・遊び用チンキ剤 +20 等)。
/// `shocked` は感電/雷電(割合減少)。`random_option` は式の末項で、割合を掛けたあとに足す。
pub fn accuracy_point(
    stats: &EffectiveStats,
    correction: &AccuracyCorrection,
    equipment_accuracy: i64,
    skill_accuracy: i64,
    bonus: i64,
    boost: AccuracyBoost,
    shocked: bool,
    random_option: i64,
) -> i64 {
    let inner = stats.dex
        + equipment_accuracy
        + skill_accuracy
        + SKILL_ACCURACY_OFFSET
        + floor_int(correction.bonus_value(stats))
        - floor_int(correction.penalty_value(stats))
        + bonus;
    let rate = boost.rate() * if shocked { SHOCK_ACCURACY_RATE } else { 1.0 };
    floor_int(inner as f64 * rate) + boost.shift() + random_option
}

/// 命中率の下限の定数項(wiki `#HitRateCap`: `命中率下限 = 15 + 最小命中率補正 − 最小回避率補正`)。
pub const HIT_RATE_MIN_BASE: i64 = 15;
/// プレイヤーが行う攻撃の命中率上限は `85 + 命中率下限`(同)。モンスターの攻撃は 100。
pub const HIT_RATE_PLAYER_SPAN: i64 = 85;
/// 対人戦における最小回避率の上限(同)。
pub const PVP_MIN_EVASION_CAP: i64 = 10;

/// 命中率(wiki `#HitRate` / `#HitRateCap`)。`命中P − 対象の回避P` を下限・上限で挟む。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitRate {
    /// 挟む前の `命中P − 回避P`
    pub raw: i64,
    /// 下限 `15 + 攻撃側の最小命中率補正 − 対象の最小回避率補正`
    pub min: i64,
    /// 上限 `85 + 下限`(プレイヤーが行う攻撃)
    pub max: i64,
    /// 下限・上限で挟んだ命中率 %
    pub value: i64,
    /// 上限に張り付いている = そのぶんは外れない。wiki の「回避Pを 100 上回ると必中」に相当する。
    /// **判定はここ(domain)で済ませて持たせる** — 画面で `raw >= max` を書き直させない
    pub capped: bool,
}

/// 対人の命中率。`min_hit_rate` は攻撃側の最小命中率補正、`min_evasion_rate` は
/// 対象の最小回避率補正(対人は上限 `PVP_MIN_EVASION_CAP`)。
pub fn hit_rate(
    accuracy_point: i64,
    evasion_point: i64,
    min_hit_rate: i64,
    min_evasion_rate: i64,
) -> HitRate {
    let raw = accuracy_point - evasion_point;
    let min = HIT_RATE_MIN_BASE + min_hit_rate - min_evasion_rate.min(PVP_MIN_EVASION_CAP);
    let max = HIT_RATE_PLAYER_SPAN + min;
    HitRate {
        raw,
        min,
        max,
        value: raw.clamp(min, max),
        capped: raw >= max,
    }
}

/// 防御側の戦闘能力値を出す。
///
/// `equipment` は装備補正 9 値の合計(基本 + 強化)。呼び出し側が `Equipment::base_totals` /
/// `enhanced_totals` を足して渡す(domain は gamedata のアビリティカタログを持たないため)。
/// `caps` は覚醒・エタの意志で開放される上限(表は gamedata)。防御力は上限に当たると
/// そこで頭打ちになり、以降の軽減はカット率 J が担う(wiki §6)。
/// `rates` は装備防御力倍率(コートアーマー / プロテクトアーマー / 改・プロテクトアーマー /
/// シエナのオーラの防御力増加)。**リンゴの島・ベリネンルミでは常に 100%** なので、
/// そのコンテンツを見ているときは呼び出し側が `DefenseRates::NEUTRAL` を渡す(wiki §6)。
pub fn defense_profile(
    stats: &EffectiveStats,
    equipment: &EquipmentValues,
    caps: AwakeningCaps,
    random_options: &RandomOptionTotals,
    rates: DefenseRates,
) -> DefenseProfile {
    let def = stats.def as f64;
    let mr = stats.mr as f64;
    let eq_physical = equipment.physical_defense as f64 * rates.physical;
    let eq_magic = equipment.magic_defense as f64 * rates.magic;

    let combo_evasion_percent = (10.0 + mr / 15.0 + stats.agi as f64 / 7.5)
        .clamp(COMBO_EVASION_MIN_PERCENT, COMBO_EVASION_MAX_PERCENT);
    let combo_evasion = combo_evasion_percent / 100.0;

    let physical_type_bonus = attack_type_bonus(stats, AttackType::Physical);

    let raw_physical =
        floor_int(def * DEFENSE_STAT_MULTIPLIER + eq_physical * DEFENSE_EQUIPMENT_MULTIPLIER);
    let raw_magic =
        floor_int(mr * DEFENSE_STAT_MULTIPLIER + eq_magic * DEFENSE_EQUIPMENT_MULTIPLIER);
    let raw_composite = floor_int(
        (def + mr) * COMPOSITE_DEFENSE_STAT_MULTIPLIER
            + (eq_physical + eq_magic) * COMPOSITE_DEFENSE_EQUIPMENT_MULTIPLIER,
    );
    let cap = |value: i64| value.min(caps.max_defense);

    DefenseProfile {
        physical_defense: cap(raw_physical),
        magic_defense: cap(raw_magic),
        composite_defense: cap(raw_composite),
        defense_cap: caps.max_defense,
        physical_defense_loss: raw_physical - cap(raw_physical),
        magic_defense_loss: raw_magic - cap(raw_magic),
        composite_defense_loss: raw_composite - cap(raw_composite),
        physical_cut_rate: cut_rate(cut_rate_a(
            stats.def + equipment.physical_defense,
            CUT_RATE_DIVISOR,
        )),
        magic_cut_rate: cut_rate(cut_rate_a(stats.mr + equipment.magic_defense, CUT_RATE_DIVISOR)),
        composite_cut_rate: cut_rate(cut_rate_a(
            stats.def + equipment.physical_defense + stats.mr + equipment.magic_defense,
            CUT_RATE_COMPOSITE_DIVISOR,
        )),
        combo_evasion,
        evasion_point: EvasionPoints {
            physical: evasion_point(
                stats,
                equipment,
                physical_type_bonus,
                random_options.evasion_point,
            ),
            magic: evasion_point(
                stats,
                equipment,
                attack_type_bonus(stats, AttackType::Magic),
                random_options.evasion_point,
            ),
            composite: evasion_point(
                stats,
                equipment,
                (def + mr) / EVASION_TYPE_DIVISOR,
                random_options.evasion_point,
            ),
        },
        equipment_physical_defense: equipment.physical_defense,
        equipment_magic_defense: equipment.magic_defense,
        equipment_evasion: equipment.evasion,
        equipment_agility: equipment.agility,
        defense_rates: rates,
    }
}

/// 命中P・回避Pの伸びしろの材料(§伸びしろの定義)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthSource {
    /// ステ上限まで(命中P = DEX、回避P = AGI)
    Stat,
    /// エンチャント枠の残り(命中率補正 / 回避率補正)
    Enchant,
    /// シエナのオーラの空きスロット(命中率 / 回避率)。実際は種類を選べないので上振れの見積り
    Siena,
    /// 命中P割合増加のキャラスキル(極・的中剣)を上限 SLv まで(命中Pのみ)
    AccuracySkill,
    /// まだ選んでいない命中P増加バフを乗せる(命中Pのみ。wiki `#AccuracyPoint`)
    AccuracyBuff,
}

/// 伸びしろ 1 件。「いまのキャラのまま、その材料を上限まで積んだら」と「いま」の差
/// (装備の買い替えは含めない)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowthRoom {
    pub source: GrowthSource,
    /// 「DEX を上限まで」など、画面にそのまま出す一言
    pub label: String,
    /// 命中P(または回避P)がいくつ増えるか
    pub gain: i64,
    /// 「1178 → 2200」のように、その材料の**いま → 上限まで積んだら**を矢印で出す内訳。
    /// 材料ごとに言い方を変えない(ステだけ矢印で他は「+825」だと読む側が揃え直す)。出せないときは `None`
    pub detail: Option<String>,
    /// 見積りが `[仮]` か(シエナのように上振れするもの)
    pub provisional: bool,
    /// この材料を積んだら命中率(%)が何動くか。攻撃側の材料は正、防御側の材料は負
    /// (回避Pが増えるほど攻撃側の命中率は下がる)。命中率は下限 15 / 上限 100 で
    /// 挟まれるため、命中P(または回避P)が増えても `0` のままのことがある
    /// (ダメージ計算タブの「表記 ±0%」と同じで、正直に出る)
    pub hit_rate_gain: i64,
}

/// 装備の部位ごとに、シエナのオーラの空き段階へ `kind`(命中率 / 回避率)を最大値まで
/// 積んだ場合の増分。`kind` が出ない部位(命中率・回避率は武器/盾に出ない)は数えない
/// (`SienaValueKind::allowed_on`)。既にオーラを装着している部位だけが対象
/// (`iter_selected` は選択中オーラが無い部位を返さない)。
fn siena_room(equipment: &Equipment, kind: SienaValueKind) -> i64 {
    let (_, max) = kind.range();
    equipment
        .siena
        .iter_selected()
        .filter(|&(slot, _)| kind.allowed_on(slot))
        .map(|(_, aura)| (SIENA_STAGE_MAX - aura.stage()) as i64 * max)
        .sum()
}

/// 装備の部位ごとに、エンチャント枠の残り(`EquipmentValues` の 1 フィールド `get`)の合計。
/// `enchant_caps` は呼び出し側が `resolve_enchant_caps` で解決した、部位ごとの実測上限
/// (カタログ item が付いている部位だけ渡ってくる)。
fn enchant_room(
    equipment: &Equipment,
    enchant_caps: &[(PartSlot, EquipmentValues)],
    get: fn(&EquipmentValues) -> i64,
) -> i64 {
    enchant_caps
        .iter()
        .filter_map(|&(slot, cap)| {
            let part = equipment.parts.get(slot).selected()?;
            let room = get(&cap) - get(&part.enchant);
            (room > 0).then_some(room)
        })
        .sum()
}

/// 攻撃側の命中Pの伸びしろ(gain 降順)。「材料を差し替えて `accuracy_point` をもう一度通す」
/// (`list_enchant_gains` が `rank_candidates` を再利用しているのと同じ考え方。丸めの
/// 食い違いを作らない)。`stat_cap` は覚醒段階 + エタの意志 Lv で決まる DEX の上限
/// (`AwakeningCaps::max_stat`)。
#[allow(clippy::too_many_arguments)]
fn accuracy_growth(
    stats: &EffectiveStats,
    correction: &AccuracyCorrection,
    equipment_accuracy: i64,
    skill_accuracy: i64,
    bonus: i64,
    boost: AccuracyBoost,
    // そのキャラが覚えられる命中P割合増加スキル(極・的中剣はマキシミン専用)。覚えられない
    // キャラに「Lv7 まで」を伸びしろとして出さないためだけに要る
    learnable_accuracy_skill: Option<&CharacterSkillDef>,
    random_option: i64,
    current: i64,
    stat_cap: i64,
    equipment: &Equipment,
    enchant_caps: &[(PartSlot, EquipmentValues)],
    buff_catalog: &crate::stat_sources::BuffCatalog,
    buff_selection: &crate::stat_sources::BuffSelection,
    defender_evasion_point: i64,
    min_hit_rate: i64,
    min_evasion_rate: i64,
    current_hit_rate: i64,
) -> (Vec<GrowthRoom>, i64) {
    // `extra_bonus` は命中P増加バフの伸びしろ(§下記)を足し込むための追加枠。
    // 通常の材料(ステ・エンチャント・シエナ・的中剣)は 0 を渡す(いまの bonus のまま)。
    let recompute = |dex: i64, eq_accuracy: i64, extra_bonus: i64, boost: AccuracyBoost| {
        accuracy_point(
            &EffectiveStats { dex, ..*stats },
            correction,
            eq_accuracy,
            skill_accuracy,
            bonus + extra_bonus,
            boost,
            false,
            random_option,
        )
    };
    // 的中剣(スキル)を Lv7 まで積んだ場合の目標 boost。未習得(None)なら Lv0 → Lv7、
    // 習得済みで Lv7 未満ならそこから Lv7 までが伸びしろ。集中(Lv1 相当)が有効なときは
    // 的中剣より優先されて効果が出ないので材料にしない(`None` を返す)
    // **そのキャラが覚えられるスキルでなければ材料にしない**。極・的中剣はマキシミン専用
    // なので、イサックの伸びしろに「極・的中剣を Lv7 まで」が出ていた(実機で検出)
    let accuracy_skill_growth = learnable_accuracy_skill.and_then(|def| {
        let current_level = match boost.source {
            AccuracyBoostSource::None => 0,
            AccuracyBoostSource::Skill { id, level, .. } if id == def.id => level,
            _ => return None,
        };
        (current_level < def.max_level).then_some((def, current_level))
    });
    // その材料を積んだ命中Pで命中率をもう一度通し、いまとの差を取る(結果への効き)。
    let hit_rate_gain = |new_accuracy_point: i64| {
        hit_rate(new_accuracy_point, defender_evasion_point, min_hit_rate, min_evasion_rate).value
            - current_hit_rate
    };

    let enchant_gain = enchant_room(equipment, enchant_caps, |v| v.accuracy);
    let siena_gain = siena_room(equipment, SienaValueKind::Accuracy);
    // まだ選んでいない命中P増加バフの合計(的中剣装着中は排他なバフを除く)
    let buff_gain = crate::stat_sources::buff_accuracy_point_room(buff_selection, buff_catalog, boost);

    let mut out = Vec::new();
    if stats.dex < stat_cap {
        let gain = recompute(stat_cap, equipment_accuracy, 0, boost) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::Stat,
                label: format!("{} を上限まで", StatKind::Dex.label()),
                gain,
                detail: Some(format!("{} → {stat_cap}", stats.dex)),
                provisional: false,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    if enchant_gain > 0 {
        let gain = recompute(stats.dex, equipment_accuracy + enchant_gain, 0, boost) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::Enchant,
                label: format!("エンチャント枠の{}を上限まで", EquipmentValues::ACCURACY_LABEL),
                gain,
                detail: Some(format!("{equipment_accuracy} → {}", equipment_accuracy + enchant_gain)),
                provisional: false,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    if siena_gain > 0 {
        let gain = recompute(stats.dex, equipment_accuracy + siena_gain, 0, boost) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::Siena,
                label: "シエナの空きスロットに命中率を上限まで".to_string(),
                gain,
                detail: Some(format!("{equipment_accuracy} → {}", equipment_accuracy + siena_gain)),
                provisional: true,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    if let Some((def, current_level)) = accuracy_skill_growth {
        let target = AccuracyBoost::from_skill(def, def.max_level).unwrap_or(boost);
        let gain = recompute(stats.dex, equipment_accuracy, 0, target) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::AccuracySkill,
                label: if current_level == 0 {
                    format!("{}を Lv{} まで", def.name, def.max_level)
                } else {
                    format!("{}を Lv{current_level} → Lv{} まで", def.name, def.max_level)
                },
                gain,
                detail: Some(format!("命中P割合 ×{:.2} → ×{:.2}", boost.rate(), target.rate())),
                provisional: false,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    if buff_gain > 0 {
        let gain = recompute(stats.dex, equipment_accuracy, buff_gain, boost) - current;
        if gain > 0 {
            let buff_now = crate::stat_sources::buff_accuracy_point_total(buff_selection, buff_catalog, boost);
            out.push(GrowthRoom {
                source: GrowthSource::AccuracyBuff,
                label: "命中P増加バフを乗せる".to_string(),
                gain,
                detail: Some(format!("{buff_now} → {}", buff_now + buff_gain)),
                provisional: false,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    out.sort_by(|a, b| b.gain.cmp(&a.gain));

    let max_dex = stats.dex.max(stat_cap);
    let max_equipment_accuracy = equipment_accuracy + enchant_gain + siena_gain;
    let max_boost = accuracy_skill_growth
        .and_then(|(def, _)| AccuracyBoost::from_skill(def, def.max_level))
        .unwrap_or(boost);
    let max = recompute(max_dex, max_equipment_accuracy, buff_gain, max_boost);
    (out, max)
}

/// 防御側の回避Pの伸びしろ(gain 降順)。`accuracy_growth` と同じ考え方
/// (材料を差し替えて `evasion_point` をもう一度通す)。`stat_cap` は AGI の上限。
#[allow(clippy::too_many_arguments)]
fn evasion_growth(
    stats: &EffectiveStats,
    equipment_evasion: i64,
    equipment_agility: i64,
    type_bonus: f64,
    random_option: i64,
    current: i64,
    stat_cap: i64,
    equipment: &Equipment,
    enchant_caps: &[(PartSlot, EquipmentValues)],
    attacker_accuracy_point: i64,
    min_hit_rate: i64,
    min_evasion_rate: i64,
    current_hit_rate: i64,
) -> (Vec<GrowthRoom>, i64) {
    let recompute = |agi: i64, evasion: i64| {
        evasion_point(
            &EffectiveStats { agi, ..*stats },
            &EquipmentValues {
                evasion,
                agility: equipment_agility,
                ..Default::default()
            },
            type_bonus,
            random_option,
        )
    };
    // 防御側の材料を積んだ回避Pで命中率をもう一度通す(攻撃側の命中率は下がる方向)。
    let hit_rate_gain = |new_evasion_point: i64| {
        hit_rate(attacker_accuracy_point, new_evasion_point, min_hit_rate, min_evasion_rate).value
            - current_hit_rate
    };

    let enchant_gain = enchant_room(equipment, enchant_caps, |v| v.evasion);
    let siena_gain = siena_room(equipment, SienaValueKind::Evasion);

    let mut out = Vec::new();
    if stats.agi < stat_cap {
        let gain = recompute(stat_cap, equipment_evasion) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::Stat,
                label: format!("{} を上限まで", StatKind::Agi.label()),
                gain,
                detail: Some(format!("{} → {stat_cap}", stats.agi)),
                provisional: false,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    if enchant_gain > 0 {
        let gain = recompute(stats.agi, equipment_evasion + enchant_gain) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::Enchant,
                label: format!("エンチャント枠の{}を上限まで", EquipmentValues::EVASION_LABEL),
                gain,
                detail: Some(format!("{equipment_evasion} → {}", equipment_evasion + enchant_gain)),
                provisional: false,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    if siena_gain > 0 {
        let gain = recompute(stats.agi, equipment_evasion + siena_gain) - current;
        if gain > 0 {
            out.push(GrowthRoom {
                source: GrowthSource::Siena,
                label: "シエナの空きスロットに回避率を上限まで".to_string(),
                gain,
                detail: Some(format!("{equipment_evasion} → {}", equipment_evasion + siena_gain)),
                provisional: true,
                hit_rate_gain: hit_rate_gain(current + gain),
            });
        }
    }
    out.sort_by(|a, b| b.gain.cmp(&a.gain));

    let max_agi = stats.agi.max(stat_cap);
    let max = recompute(max_agi, equipment_evasion + enchant_gain + siena_gain);
    (out, max)
}

/// 対人の命中率一式(wiki `#AccuracyPoint` / `#EvasionPoint` / `#HitRate`)。
/// 攻撃側の命中Pの内訳・防御側の採用回避Pの内訳を画面がそのまま出せるように、
/// 途中式の値も持つ(対人タブの結果面専用)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VersusAccuracy {
    /// 突き合わせた攻撃タイプ(攻撃側スキルの依存種別から判定)
    pub attack_type: AttackType,
    /// 攻撃側 DEX
    pub attacker_dex: i64,
    /// 攻撃側の装備命中率補正(基本 + 強化の合計)
    pub equipment_accuracy: i64,
    /// 攻撃側のスキル命中(wiki 表記のまま。`SKILL_ACCURACY_OFFSET` を足す前)
    pub skill_accuracy: i64,
    /// 依存ボーナス(切り捨て後)。ボーナスの無い依存は 0
    pub correction_bonus: i64,
    /// 依存ペナルティ(切り捨て後)
    pub correction_penalty: i64,
    /// 命中P増加の合計(射手のルーン等)
    pub accuracy_bonus: i64,
    pub accuracy_boost: AccuracyBoost,
    /// 攻撃側の命中P(最終)
    pub accuracy_point: i64,
    /// 防御側 AGI
    pub defender_agi: i64,
    /// 防御側の装備回避率補正
    pub equipment_evasion: i64,
    /// 防御側の装備敏捷度補正
    pub equipment_agility: i64,
    /// 攻撃タイプに応じた回避P増加(採用した攻撃タイプの分だけ)
    pub attack_type_bonus: f64,
    /// 防御側の回避P(採用したもの。最終)
    pub evasion_point: i64,
    pub hit_rate: HitRate,
    /// 攻撃側の最小命中率補正(供給源が無いためいまは常に 0)
    pub min_hit_rate: i64,
    /// `min_hit_rate` に供給源があるか。`false` のとき、画面は下限の命中側だけ `?` を出す
    /// (プレイヤー側の供給源表が wiki に無い)
    pub min_hit_rate_recorded: bool,
    /// 対象の最小回避率補正(対人の上限 `PVP_MIN_EVASION_CAP` でクランプ済み)
    pub min_evasion_rate: i64,
    /// `min_evasion_rate` に供給源があるか(`VersusDefender::min_evasion_rate` が `Some`)
    pub min_evasion_rate_recorded: bool,
    /// 攻撃側の命中Pの伸びしろ(材料ごと。gain 降順。伸びしろ無しの材料は入らない)
    pub accuracy_growth: Vec<GrowthRoom>,
    /// 攻撃側の命中Pの伸びしろを全部積んだときの命中P
    pub accuracy_max: i64,
    /// 攻撃側の命中Pの伸びしろを全部積んだときの命中率(結果への効き。フロントで % を導出させない)
    pub accuracy_max_hit_rate: HitRate,
    /// 防御側の回避Pの伸びしろ(材料ごと。gain 降順。伸びしろ無しの材料は入らない)
    pub evasion_growth: Vec<GrowthRoom>,
    /// 防御側の回避Pの伸びしろを全部積んだときの回避P
    pub evasion_max: i64,
    /// 防御側の回避Pの伸びしろを全部積んだときの命中率(攻撃側から見た数字。下がる方向)
    pub evasion_max_hit_rate: HitRate,
}

/// 対人計算の攻撃側ぶん。防御側と取り違えないよう型で分ける
/// (`versus_accuracy` の引数を構造体化。攻撃側と防御側を位置引数の並びで
/// 見分けさせない)。
pub struct VersusAttacker<'a> {
    pub stats: &'a EffectiveStats,
    pub correction: &'a AccuracyCorrection,
    pub equipment: &'a Equipment,
    pub enchant_caps: &'a [(PartSlot, EquipmentValues)],
    /// `AwakeningCaps::max_stat`(DEX の上限)
    pub stat_cap: i64,
    pub equipment_accuracy: i64,
    /// wiki 表記のまま(`SKILL_ACCURACY_OFFSET` は `versus_accuracy` 内で足す)
    pub skill_accuracy: i64,
    /// 命中P増加の合計(射手のルーン等。呼び出し側が `buff_accuracy_point_total` で集計する)
    pub accuracy_bonus: i64,
    pub accuracy_boost: AccuracyBoost,
    /// このキャラが覚えられる命中P割合増加スキル(極・的中剣はマキシミン専用)。伸びしろの
    /// 材料に「Lv7 まで」を出してよいかの判定にだけ使う ── 覚えられないキャラに出さない
    pub learnable_accuracy_skill: Option<&'a CharacterSkillDef>,
    pub accuracy_random_option: i64,
    /// 命中P増加バフの伸びしろ材料の解決に要る(`buff_accuracy_point_room`)。
    /// `accuracy_bonus` 自体は呼び出し側が集計済みの値を渡すので、ここは伸びしろ専用
    pub accuracy_buff_catalog: &'a crate::stat_sources::BuffCatalog,
    pub accuracy_buff_selection: &'a crate::stat_sources::BuffSelection,
    /// 攻撃側の最小命中率補正(wiki `#HitRateCap`)。プレイヤー側の供給源表が wiki に無い
    /// (載っているのはマップ側の値だけ)ため、いまは常に `None` を渡す。`Some` を渡せるように
    /// なったら `VersusAccuracy::min_hit_rate_recorded` が自動で `true` になる
    pub min_hit_rate: Option<i64>,
}

/// 対人計算の防御側ぶん
pub struct VersusDefender<'a> {
    pub stats: &'a EffectiveStats,
    pub profile: &'a DefenseProfile,
    pub equipment: &'a Equipment,
    pub enchant_caps: &'a [(PartSlot, EquipmentValues)],
    /// `AwakeningCaps::max_stat`(AGI の上限)
    pub stat_cap: i64,
    /// 回避Pのランダムオプション増加(「回避率が X 増加」の合計)。
    /// `defense_profile` が既に足し込んだ `profile.evasion_point` を伸びしろ計算でも
    /// そのまま再現するために要る(`RandomOptionTotals::evasion_point` と同じ値)。
    pub evasion_random_option: i64,
    /// 対象の最小回避率補正(wiki `#HitRateCap`「最小回避率補正に該当するもの」)。
    /// ランダムオプション(固定回避・最大回避率)+ バフ(テイルズウィーバーのエネルギー)の合計。
    /// 呼び出し側(`commands`)が集計して渡す
    pub min_evasion_rate: Option<i64>,
}

/// 対人の命中率を組み立てる(`preview_versus` コマンド専用)。
///
/// 伸びしろ(§伸びしろの定義)の材料解決はカタログが要るので呼び出し側(`commands`)の役目:
/// `equipment` はそれぞれの装備一式(エンチャント現在値・シエナのオーラ)、`enchant_caps` は
/// `resolve_enchant_caps` で解決した部位ごとの実測上限。
pub fn versus_accuracy(
    attacker: &VersusAttacker,
    defender: &VersusDefender,
    attack_type: AttackType,
) -> VersusAccuracy {
    // 感電・雷電は今回まだ入力を持たない([仮] 中立値。build_damage_material と同じ扱い)。
    let attacker_accuracy_point = accuracy_point(
        attacker.stats,
        attacker.correction,
        attacker.equipment_accuracy,
        attacker.skill_accuracy,
        attacker.accuracy_bonus,
        attacker.accuracy_boost,
        false,
        attacker.accuracy_random_option,
    );
    let defender_evasion_point = defender.profile.evasion_point.for_attack_type(attack_type);
    let hit = hit_rate(
        attacker_accuracy_point,
        defender_evasion_point,
        attacker.min_hit_rate.unwrap_or(0),
        defender.min_evasion_rate.unwrap_or(0),
    );
    let (accuracy_growth, accuracy_max) = accuracy_growth(
        attacker.stats,
        attacker.correction,
        attacker.equipment_accuracy,
        attacker.skill_accuracy,
        attacker.accuracy_bonus,
        attacker.accuracy_boost,
        attacker.learnable_accuracy_skill,
        attacker.accuracy_random_option,
        attacker_accuracy_point,
        attacker.stat_cap,
        attacker.equipment,
        attacker.enchant_caps,
        attacker.accuracy_buff_catalog,
        attacker.accuracy_buff_selection,
        defender_evasion_point,
        attacker.min_hit_rate.unwrap_or(0),
        defender.min_evasion_rate.unwrap_or(0),
        hit.value,
    );
    let defender_type_bonus = attack_type_bonus(defender.stats, attack_type);
    let (evasion_growth, evasion_max) = evasion_growth(
        defender.stats,
        defender.profile.equipment_evasion,
        defender.profile.equipment_agility,
        defender_type_bonus,
        defender.evasion_random_option,
        defender_evasion_point,
        defender.stat_cap,
        defender.equipment,
        defender.enchant_caps,
        attacker_accuracy_point,
        attacker.min_hit_rate.unwrap_or(0),
        defender.min_evasion_rate.unwrap_or(0),
        hit.value,
    );
    let accuracy_max_hit_rate = hit_rate(
        accuracy_max,
        defender_evasion_point,
        attacker.min_hit_rate.unwrap_or(0),
        defender.min_evasion_rate.unwrap_or(0),
    );
    let evasion_max_hit_rate = hit_rate(
        attacker_accuracy_point,
        evasion_max,
        attacker.min_hit_rate.unwrap_or(0),
        defender.min_evasion_rate.unwrap_or(0),
    );
    VersusAccuracy {
        attack_type,
        attacker_dex: attacker.stats.dex,
        equipment_accuracy: attacker.equipment_accuracy,
        skill_accuracy: attacker.skill_accuracy,
        correction_bonus: floor_int(attacker.correction.bonus_value(attacker.stats)),
        correction_penalty: floor_int(attacker.correction.penalty_value(attacker.stats)),
        accuracy_bonus: attacker.accuracy_bonus,
        accuracy_boost: attacker.accuracy_boost,
        accuracy_point: attacker_accuracy_point,
        defender_agi: defender.stats.agi,
        equipment_evasion: defender.profile.equipment_evasion,
        equipment_agility: defender.profile.equipment_agility,
        attack_type_bonus: defender_type_bonus,
        evasion_point: defender_evasion_point,
        hit_rate: hit,
        min_hit_rate: attacker.min_hit_rate.unwrap_or(0),
        min_hit_rate_recorded: attacker.min_hit_rate.is_some(),
        min_evasion_rate: defender.min_evasion_rate.unwrap_or(0).min(PVP_MIN_EVASION_CAP),
        min_evasion_rate_recorded: defender.min_evasion_rate.is_some(),
        accuracy_growth,
        accuracy_max,
        accuracy_max_hit_rate,
        evasion_growth,
        evasion_max,
        evasion_max_hit_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// wiki Skill/マキシミン `#HitSword`: Lv×5%、命中P変動の表(Lv1〜7)
    const HIT_SWORD_SHIFT: [i64; 7] = [3, 2, 1, 1, 0, -1, -2];
    const HIT_SWORD_MAX_LEVEL: u8 = 7;
    fn precision_sword(level: u8) -> AccuracyBoost {
        AccuracyBoost::from_rate_skill(
            "maximin_hit_sword",
            "極・的中剣",
            0.05,
            &HIT_SWORD_SHIFT,
            level,
            HIT_SWORD_MAX_LEVEL,
        )
    }
    const HIT_SWORD_DEF: CharacterSkillDef = CharacterSkillDef {
        id: "maximin_hit_sword",
        game_character_id: "maximin",
        name: "極・的中剣",
        audience: crate::character_skill::SkillAudience::SelfOnly,
        max_level: HIT_SWORD_MAX_LEVEL,
        effects: &[SkillEffect::AccuracyRate {
            per_level: 0.05,
            shift: &HIT_SWORD_SHIFT,
        }],
        mastery_overrides: &[],
        source_url: "",
        note: "",
    };

    fn stats(def: i64, mr: i64, agi: i64) -> EffectiveStats {
        EffectiveStats {
            def,
            mr,
            agi,
            ..Default::default()
        }
    }

    /// 上限に当たらない値(上限の挙動は専用テストで見る)
    fn no_caps() -> AwakeningCaps {
        AwakeningCaps {
            max_damage: i64::MAX,
            max_defense: i64::MAX,
            max_stat: i64::MAX,
        }
    }

    #[test]
    fn 防御力はステ3倍と装備防御6倍() {
        let p = defense_profile(
            &stats(200, 150, 0),
            &EquipmentValues {
                physical_defense: 60,
                magic_defense: 40,
                ..Default::default()
            },
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        assert_eq!(p.physical_defense, 960); // 200*3 + 60*6
        assert_eq!(p.magic_defense, 690); // 150*3 + 40*6
                                          // (200+150)*1.5 + (60 + 40)*3 = 525 + 300 = 825
        assert_eq!(p.composite_defense, 825);
        assert_eq!(p.equipment_physical_defense, 60);
        assert_eq!(p.equipment_magic_defense, 40);
    }

    #[test]
    fn カット率は1マイナスaを80足したaで割った値() {
        let p = defense_profile(
            &stats(200, 150, 0),
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        // a = 3 + [(200-1)/10] = 3 + 19 = 22 → 1 − 22/102
        assert!((p.physical_cut_rate - (1.0 - 22.0 / 102.0)).abs() < 1e-9);
        // a = 3 + [(150-1)/10] = 3 + 14 = 17 → 1 − 17/97
        assert!((p.magic_cut_rate - (1.0 - 17.0 / 97.0)).abs() < 1e-9);
        // a = 3 + [(200+150-1)/20] = 3 + 17 = 20 → 1 − 20/100
        assert!((p.composite_cut_rate - 0.8).abs() < 1e-9);
    }

    #[test]
    fn カット率の装備防御は生の値で足す() {
        let p = defense_profile(
            &stats(200, 150, 0),
            &EquipmentValues {
                physical_defense: 100,
                magic_defense: 50,
                ..Default::default()
            },
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        // a = 3 + [(200+100-1)/10] = 3 + 29 = 32
        assert!((p.physical_cut_rate - (1.0 - 32.0 / 112.0)).abs() < 1e-9);
        // a = 3 + [(150+50-1)/10] = 3 + 19 = 22
        assert!((p.magic_cut_rate - (1.0 - 22.0 / 102.0)).abs() < 1e-9);
        // a = 3 + [(200+100+150+50-1)/20] = 3 + 24 = 27
        assert!((p.composite_cut_rate - (1.0 - 27.0 / 107.0)).abs() < 1e-9);
    }

    #[test]
    fn 特殊回避は下限20上限63に収まる() {
        // MR/AGI が 0 なら 10% → 下限 20%
        let zero = EquipmentValues::default();
        assert!(
            (defense_profile(
                &stats(0, 0, 0),
                &zero,
                no_caps(),
                &RandomOptionTotals::default(),
                DefenseRates::NEUTRAL
            )
            .combo_evasion
                - 0.20)
                .abs()
                < 1e-9
        );
        // 10 + 150/15 + 200/7.5 = 10 + 10 + 26.666.. = 46.666..%
        let p = defense_profile(
            &stats(0, 150, 200),
            &zero,
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        assert!((p.combo_evasion - 0.4666666666666667).abs() < 1e-9);
        // 上限 63%
        assert!(
            (defense_profile(
                &stats(0, 310, 310),
                &zero,
                no_caps(),
                &RandomOptionTotals::default(),
                DefenseRates::NEUTRAL
            )
            .combo_evasion
                - 0.63)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn 回避Pは15足すAGI1_2倍足す攻撃タイプ別増加() {
        // DEF200 / MR150 / AGI100、STAB+HACK は 0、装備なし
        let p = defense_profile(
            &stats(200, 150, 100),
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        // 物理: 15 + 120 + (400 + 0)/7 = 135 + 57.142.. = 192.14.. → 192
        assert_eq!(p.evasion_point.physical, 192);
        // 魔法: 15 + 120 + 300/7 = 135 + 42.857.. → 177
        assert_eq!(p.evasion_point.magic, 177);
        // 複合: 15 + 120 + 350/7 = 135 + 50 = 185
        assert_eq!(p.evasion_point.composite, 185);
    }

    #[test]
    fn 回避Pは装備回避率を1_2倍で装備敏捷度を7分の1で足す() {
        let p = defense_profile(
            &stats(200, 150, 100),
            &EquipmentValues {
                evasion: 50,
                agility: 70,
                ..Default::default()
            },
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        // 複合: 15 + (100+50)*1.2 + 70/7 + 350/7 = 15 + 180 + 10 + 50 = 255
        assert_eq!(p.evasion_point.composite, 255);
        assert_eq!(p.equipment_evasion, 50);
        assert_eq!(p.equipment_agility, 70);
    }

    #[test]
    fn 回避Pにランダムオプションの回避率増加が足される() {
        let eq = EquipmentValues::default();
        let totals = RandomOptionTotals {
            evasion_point: 15,
            ..Default::default()
        };
        let base = defense_profile(
            &stats(200, 150, 100),
            &eq,
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let with_ro = defense_profile(
            &stats(200, 150, 100),
            &eq,
            no_caps(),
            &totals,
            DefenseRates::NEUTRAL,
        );
        assert_eq!(
            with_ro.evasion_point.physical,
            base.evasion_point.physical + 15
        );
        assert_eq!(with_ro.evasion_point.magic, base.evasion_point.magic + 15);
        assert_eq!(
            with_ro.evasion_point.composite,
            base.evasion_point.composite + 15
        );
    }

    #[test]
    fn 物理の回避P増加は突き足す斬りを100で割って切捨ててから足す() {
        let s = EffectiveStats {
            def: 0,
            mr: 0,
            agi: 0,
            stab: 250,
            hack: 260,
            ..Default::default()
        };
        let p = defense_profile(
            &s,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        // [(250+260)/100] = 5 → 15 + 0 + 5/7 = 15.714.. → 15
        assert_eq!(p.evasion_point.physical, 15);
    }

    /// 依存ボーナス・依存ペナルティが両方 0 になる中立な `AccuracyCorrection`。
    fn neutral_correction() -> AccuracyCorrection {
        AccuracyCorrection {
            bonus: None,
            penalty_primary: StatKind::Def,
            penalty_secondary: None,
            penalty_divisor: 1.0,
        }
    }

    #[test]
    fn 命中pのスキル依存にはオフセット15が足される() {
        let s = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let p = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(p, 100 + SKILL_ACCURACY_OFFSET);
    }

    #[test]
    fn 的中剣はslvごとにlv掛ける5パーセントの倍率とlvごとの命中p変動が乗る() {
        let s = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        // inner = 100 + 15(オフセット) = 115
        let inner = 115.0;
        let lv5 = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            precision_sword(5),
            false,
            0,
        );
        // Lv5 の命中P変動は 0(HIT_SWORD_SHIFT[4])。倍率は 1 + 5*5% = 1.25
        assert_eq!(lv5, floor_int(inner * 1.25));
        let lv7 = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            precision_sword(7),
            false,
            0,
        );
        // Lv7 の命中P変動は −2(HIT_SWORD_SHIFT[6])。倍率は 1 + 7*5% = 1.35
        assert_eq!(lv7, floor_int(inner * 1.35) - 2);
    }

    /// SLv ごとの倍率が 5%/10%/…/35% になること(wiki Skill/マキシミン #HitSword:
    /// `Lv*5%`)。
    #[test]
    fn 的中剣のslvごとの倍率は5パーセント刻み() {
        for level in 1..=7u8 {
            let expected = 1.0 + level as f64 * 0.05;
            assert!(
                (precision_sword(level).rate() - expected).abs() < 1e-12,
                "Lv{level}"
            );
        }
        assert_eq!(precision_sword(1).rate(), 1.05);
        assert_eq!(precision_sword(7).rate(), 1.35);
    }

    /// 集中と的中剣が両方あるとき、集中(Lv1相当)が優先される(wiki `PET`:
    /// 「集中が優先されて的中剣が無効」)。
    #[test]
    fn 集中と的中剣が両方あるとき集中が勝つ() {
        assert_eq!(
            AccuracyBoost::resolve(true, Some(precision_sword(7))),
            AccuracyBoost::concentration()
        );
        assert_eq!(
            AccuracyBoost::resolve(false, Some(precision_sword(7))),
            precision_sword(7)
        );
        assert_eq!(AccuracyBoost::resolve(false, None), AccuracyBoost::NONE);
        assert_eq!(AccuracyBoost::resolve(false, Some(precision_sword(0))), AccuracyBoost::NONE);
    }

    #[test]
    fn 感電は命中p割合を07倍にする() {
        let s = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let normal = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        let shocked = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            AccuracyBoost::NONE,
            true,
            0,
        );
        assert_eq!(shocked, floor_int(normal as f64 * SHOCK_ACCURACY_RATE));
    }

    #[test]
    fn ランダムオプションは割合を掛けたあとに足される() {
        let s = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let without = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            precision_sword(5),
            false,
            0,
        );
        let with = accuracy_point(
            &s,
            &neutral_correction(),
            0,
            0,
            0,
            precision_sword(5),
            false,
            20,
        );
        assert_eq!(with, without + 20);
    }

    #[test]
    fn hit_rateは下限上限で挟み対人回避率補正は10で頭打ち() {
        // raw が上限を超えると必中(capped)扱いで上限に張り付く
        let capped = hit_rate(200, 0, 0, 0);
        assert_eq!(capped.min, HIT_RATE_MIN_BASE);
        assert_eq!(capped.max, HIT_RATE_MIN_BASE + HIT_RATE_PLAYER_SPAN);
        assert_eq!(capped.value, capped.max);
        assert!(capped.capped);

        // raw が下限を下回っても下限で頭打ち
        let low = hit_rate(-500, 0, 0, 0);
        assert_eq!(low.value, HIT_RATE_MIN_BASE);
        assert!(!low.capped);

        // 対人: 対象の最小回避率補正20は上限10で頭打ちされる → min = 15 − 10 = 5
        let pvp = hit_rate(50, 0, 0, 20);
        assert_eq!(pvp.min, HIT_RATE_MIN_BASE - PVP_MIN_EVASION_CAP);
        // 上限は常に 85 + 下限で連動する
        assert_eq!(pvp.max, HIT_RATE_PLAYER_SPAN + pvp.min);

        // 最小回避率補正が上限10ちょうどでも同じ(頭打ちの境界)
        let at_cap = hit_rate(50, 0, 0, PVP_MIN_EVASION_CAP);
        assert_eq!(at_cap.min, pvp.min);

        // 最小回避率補正9(上限未満)ならそのまま反映される → min = 15 − 9 = 6
        let below_cap = hit_rate(50, 0, 0, 9);
        assert_eq!(below_cap.min, HIT_RATE_MIN_BASE - 9);

        // 最小命中率補正は攻撃側。下限を押し上げ、上限も連動する
        let with_min_hit = hit_rate(50, 0, 8, 0);
        assert_eq!(with_min_hit.min, HIT_RATE_MIN_BASE + 8);
        assert_eq!(with_min_hit.max, HIT_RATE_PLAYER_SPAN + with_min_hit.min);
    }


    #[test]
    fn 攻撃タイプは依存種別を物理魔法の2分類に振り分ける() {
        use crate::skill::SkillDependency;
        for dep in [
            SkillDependency::Stab,
            SkillDependency::Hack,
            SkillDependency::StabHack,
        ] {
            assert_eq!(dep.attack_type(), AttackType::Physical);
        }
        for dep in [
            SkillDependency::Int,
            SkillDependency::Mr,
            SkillDependency::HackInt,
        ] {
            assert_eq!(dep.attack_type(), AttackType::Magic);
        }
    }

    #[test]
    fn versus_accuracyは命中pと採用した回避pの差でhit_rateを出す() {
        let attacker = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let defender_stats = EffectiveStats {
            def: 200,
            mr: 150,
            agi: 100,
            ..Default::default()
        };
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: crate::stats::BASE_STAT_MAX as i64,
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: crate::stats::BASE_STAT_MAX as i64,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert_eq!(v.accuracy_point, 100 + SKILL_ACCURACY_OFFSET);
        assert_eq!(v.evasion_point, defender.evasion_point.physical);
        assert_eq!(
            v.hit_rate,
            hit_rate(v.accuracy_point, v.evasion_point, 0, 0)
        );
        assert!(!v.min_hit_rate_recorded);
        assert!(!v.min_evasion_rate_recorded);
        assert_eq!(v.defender_agi, 100);
    }

    #[test]
    fn versus_accuracyは防御側の最小回避率補正だけ収録済みとして下限を下げる() {
        let attacker = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let defender_stats = EffectiveStats {
            def: 200,
            mr: 150,
            agi: 100,
            ..Default::default()
        };
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: crate::stats::BASE_STAT_MAX as i64,
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: crate::stats::BASE_STAT_MAX as i64,
                evasion_random_option: 0,
                // 対人上限(10)を超えて積んでいる(鎧 + 手 + バフの合計を想定)
                min_evasion_rate: Some(23),
            },
            AttackType::Physical,
        );
        // 命中側は未収録、回避側は収録済み
        assert!(!v.min_hit_rate_recorded);
        assert!(v.min_evasion_rate_recorded);
        // 対人上限 10 で頭打ちされた値が乗る(23 積んでも 10 のまま)
        assert_eq!(v.min_evasion_rate, PVP_MIN_EVASION_CAP);
        assert_eq!(v.hit_rate.min, HIT_RATE_MIN_BASE - PVP_MIN_EVASION_CAP);
        assert_eq!(v.hit_rate.max, HIT_RATE_PLAYER_SPAN + v.hit_rate.min);
    }

    #[test]
    fn ステが上限に張り付いていたらstatの伸びしろが出ない() {
        let attacker = EffectiveStats {
            dex: 200,
            ..Default::default()
        };
        let defender_stats = EffectiveStats {
            agi: 150,
            ..Default::default()
        };
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: attacker.dex, // ステ上限 = 現在値(張り付いている)
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi, // 同上(AGI)
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert!(!v.accuracy_growth.iter().any(|g| g.source == GrowthSource::Stat));
        assert!(!v.evasion_growth.iter().any(|g| g.source == GrowthSource::Stat));
    }

    #[test]
    fn 的中剣がlv7未満のキャラだけprecision_swordの伸びしろが出る() {
        let attacker = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let defender_stats = EffectiveStats::default();
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let without = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: attacker.dex, // ステ伸びしろは無関係にするため上限=現在値
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert!(without
            .accuracy_growth
            .iter()
            .any(|g| g.source == GrowthSource::AccuracySkill));

        // Lv5(Lv7 未満)は「残り Lv ぶん」の伸びしろが出る
        let partial = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: attacker.dex,
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: precision_sword(5),
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert!(partial
            .accuracy_growth
            .iter()
            .any(|g| g.source == GrowthSource::AccuracySkill));

        // Lv7(上限)まで積んだキャラはもう伸びしろが無い
        let maxed = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: attacker.dex,
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: precision_sword(7),
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert!(!maxed
            .accuracy_growth
            .iter()
            .any(|g| g.source == GrowthSource::AccuracySkill));
    }

    #[test]
    fn accuracy_maxは材料を全部積んで再計算した命中pと一致する() {
        let attacker = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let defender_stats = EffectiveStats::default();
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let mut equipment = Equipment::default();
        equipment.parts.weapon = crate::equipment::EquipmentPartList::from(crate::equipment::EquipmentPart {
            item_id: Some("w1".to_string()),
            enchant: EquipmentValues {
                accuracy: 10,
                ..Default::default()
            },
            ..Default::default()
        });
        let enchant_caps = [(
            PartSlot::Weapon,
            EquipmentValues {
                accuracy: 40,
                ..Default::default()
            },
        )];
        let stat_cap = 250;
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &equipment,
                enchant_caps: &enchant_caps,
                stat_cap,
                equipment_accuracy: 10,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: crate::stats::BASE_STAT_MAX as i64,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        // 全材料を積み直した命中P(ステ上限 + エンチャント上限 + 的中剣 Lv7)と突き合わせる
        let boosted_accuracy = 10 + (enchant_caps[0].1.accuracy - 10);
        let target = precision_sword(HIT_SWORD_MAX_LEVEL);
        let recomputed = accuracy_point(
            &EffectiveStats {
                dex: stat_cap,
                ..attacker
            },
            &neutral_correction(),
            boosted_accuracy,
            0,
            0,
            target,
            false,
            0,
        );
        assert_eq!(v.accuracy_max, recomputed);
        assert!(v.accuracy_max > v.accuracy_point);
    }

    #[test]
    fn 命中率が上限に張り付いていたら攻撃側の材料のhit_rate_gainは0() {
        let attacker = EffectiveStats {
            dex: 200,
            ..Default::default()
        };
        // 防御側は evasion_point = 15(AGI 0)。命中率は既に上限(100)で張り付く。
        let defender_stats = EffectiveStats::default();
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: 300, // DEX をさらに積める
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert!(v.hit_rate.capped);
        assert_eq!(v.hit_rate.value, v.hit_rate.max);
        let stat_room = v
            .accuracy_growth
            .iter()
            .find(|g| g.source == GrowthSource::Stat)
            .expect("DEX の伸びしろが出るはず");
        assert_eq!(stat_room.hit_rate_gain, 0);
    }

    #[test]
    fn 命中率が下限に張り付いていたら閾値を超えるまでhit_rate_gainは0() {
        let attacker = EffectiveStats::default(); // DEX 0
        let mut equipment = Equipment::default();
        equipment.parts.weapon = crate::equipment::EquipmentPartList::from(crate::equipment::EquipmentPart {
            item_id: Some("w1".to_string()),
            enchant: EquipmentValues::default(),
            ..Default::default()
        });
        // エンチャント枠の伸びしろを大きく取り、命中率下限(15)を超えて動く材料にする。
        let enchant_caps = [(
            PartSlot::Weapon,
            EquipmentValues {
                accuracy: 300,
                ..Default::default()
            },
        )];
        // 防御側 AGI 163 → evasion_point.physical = floor(15 + 163*1.2) = 210。
        // 攻撃側の命中P(現在)は 0+0+15=15、raw = 15-210 = -195 で下限 15 に張り付く。
        let defender_stats = EffectiveStats {
            agi: 163,
            ..Default::default()
        };
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        assert_eq!(defender.evasion_point.physical, 210);
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &equipment,
                enchant_caps: &enchant_caps,
                stat_cap: 5, // DEX を少しだけ積める(下限を超えない)
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi,
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert_eq!(v.hit_rate.value, v.hit_rate.min);

        // 少し積んだだけ(DEX +5)では下限のまま動かない。
        let stat_room = v
            .accuracy_growth
            .iter()
            .find(|g| g.source == GrowthSource::Stat)
            .expect("DEX の伸びしろが出るはず");
        assert_eq!(stat_room.hit_rate_gain, 0);

        // 閾値を超える量(エンチャント枠 +300)を積むと動く。
        let enchant_room = v
            .accuracy_growth
            .iter()
            .find(|g| g.source == GrowthSource::Enchant)
            .expect("エンチャントの伸びしろが出るはず");
        assert!(enchant_room.hit_rate_gain > 0);
    }

    #[test]
    fn 防御側の材料はhit_rate_gainが負になる() {
        // 攻撃側の命中P = 65(DEX 50 + オフセット 15)、防御側 evasion_point(現在) = 15(AGI 0)。
        // raw = 65 - 15 = 50 で上限・下限のどちらにも当たらない(挟まれない領域)。
        let attacker = EffectiveStats {
            dex: 50,
            ..Default::default()
        };
        let defender_stats = EffectiveStats::default();
        let defender = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let v = versus_accuracy(
            &VersusAttacker {
                learnable_accuracy_skill: Some(&HIT_SWORD_DEF),
                stats: &attacker,
                correction: &neutral_correction(),
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: attacker.dex, // 攻撃側は動かさない
                equipment_accuracy: 0,
                skill_accuracy: 0,
                accuracy_bonus: 0,
                accuracy_boost: AccuracyBoost::NONE,
                accuracy_random_option: 0,
                accuracy_buff_catalog: &[],
                accuracy_buff_selection: &crate::stat_sources::BuffSelection::default(),
                min_hit_rate: None,
            },
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: 10, // AGI を 0 → 10 まで積める
                evasion_random_option: 0,
                min_evasion_rate: None,
            },
            AttackType::Physical,
        );
        assert!(!v.hit_rate.capped);
        assert_eq!(v.hit_rate.value, 50);
        let stat_room = v
            .evasion_growth
            .iter()
            .find(|g| g.source == GrowthSource::Stat)
            .expect("AGI の伸びしろが出るはず");
        assert!(stat_room.hit_rate_gain < 0);
        // AGI 10 → evasion_point = floor(15 + 10*1.2) = 27、raw = 65-27 = 38 → hit_rate 38(挟まれない)
        assert_eq!(stat_room.hit_rate_gain, -12);
    }

    #[test]
    fn 極的中剣を覚えられないキャラには伸びしろの材料を出さない() {
        // 極・的中剣はマキシミン専用。ほかのキャラの伸びしろに「Lv7 まで」が出ていた(実機で検出)
        let stats = EffectiveStats { dex: 500, ..Default::default() };
        let defender_stats = EffectiveStats { agi: 100, ..Default::default() };
        let equipment = Equipment::default();
        let correction = neutral_correction();
        let buffs = crate::stat_sources::BuffSelection::default();
        let profile = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            AwakeningCaps { max_damage: 0, max_defense: 9_999, max_stat: 2_200 },
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        let attacker = |can_learn: bool| VersusAttacker {
            learnable_accuracy_skill: can_learn.then_some(&HIT_SWORD_DEF),
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: 2_200,
            equipment_accuracy: 0,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &buffs,
            min_hit_rate: None,
        };
        let defender = VersusDefender {
            stats: &defender_stats,
            profile: &profile,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: 2_200,
            evasion_random_option: 0,
            min_evasion_rate: None,
        };
        let has = |can_learn: bool| {
            versus_accuracy(&attacker(can_learn), &defender, AttackType::Physical)
                .accuracy_growth
                .iter()
                .any(|r| r.source == GrowthSource::AccuracySkill)
        };
        assert!(has(true), "覚えられるキャラには出る");
        assert!(!has(false), "覚えられないキャラには出さない");
    }
}
