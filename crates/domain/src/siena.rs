//! シエナのオーラ(wiki: 装備システム/シエナのオーラ)。Lv310 装備の 8 部位に発現できる。
//!
//! 増幅段階ごとに能力値スロットが 1 個解放され、段階 3/7/10 で追加オプションが 1/2/3 個解放される。
//! 中身は再抽選のランダム値なので wiki には**種類と値域と確率**しか無い。だからユーザーは
//! 「スロットに何が出ているか」を 1 個ずつ選んで積む。段階は積んだスロット数そのもの
//! (別に入力させない)。
//!
//! 効き先は種類ごとに決まっている:
//! - 武器/盾の能力値 → 装備補正(エンチャント扱い)= 強化能力値
//! - その他の部位の STAB〜AGI → 最終固定値増加
//! - 追加オプションの攻撃力増加 → 与ダメージ割合増加(カテゴリ New1)
//! - まだ与ダメージ式に入れていない種類(耐性・HP/MP/SP など)は**記録するだけ**

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::equipment::{EquipmentValues, PartSlot, SienaStatBonus};
use crate::stats::StatKind;

/// 増幅段階の上限(wiki: 発現・増幅の表 0→1〜9→10)。能力値スロットの最大数と等しい。
pub const SIENA_STAGE_MAX: usize = 10;
/// 追加オプションが 1 個ずつ解放される段階(wiki: 発現・増幅の表の備考「追加オプション N 個解放」)。
pub const SIENA_EXTRA_UNLOCK_STAGES: [usize; 3] = [3, 7, 10];

/// 能力値スロットに出る能力値の種類(wiki: 能力値一覧(武器/盾)・(その他の部位))。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SienaValueKind {
    // --- 武器/盾。装備補正(エンチャント扱い)が増加する
    Thrust,
    Slash,
    MagicAttack,
    MagicDefense,
    /// 物理複合攻撃力。wiki「5 の場合、突き 3・斬り 2 増加する」
    PhysicalComposite,
    /// 魔法斬り攻撃力。wiki「5 の場合、魔攻 3・斬り 2 増加する」
    MagicSlash,
    // --- その他の部位
    /// 物理ダメージ耐性 %(実際は物理防御力の最終値が増加。防御側未モデル)
    PhysicalResist,
    /// 魔法ダメージ耐性 %(実際は魔法防御力の最終値が増加。防御側未モデル)
    MagicResist,
    /// クリティカル被撃率減少 %(wiki 未検証・防御側未モデル)
    CriticalTakenReduction,
    /// 命中率 %(実際は装備命中率補正が数値分の固定値で増加。命中の配線が未着手)
    Accuracy,
    /// 回避率 %(実際は装備回避率補正が数値分の固定値で増加。回避の配線が未着手)
    Evasion,
    Stab,
    Hack,
    Int,
    Def,
    Mr,
    Dex,
    Agi,
}

/// この種類の値が実際に効く先(wiki の「実際は〜が増加する」注記に基づく分類)。
/// `SienaValueKind::allowed_on` の部位判定・`apply_to_values` の集計はここから分岐する
/// (二重分類にしない)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SienaEffect {
    /// 強化能力値(装備補正)へ加算される(武器/盾の 6 種。複合は内訳どおりに分かれる)。
    EquipmentValue,
    /// 装備命中率補正へ数値ぶんの固定値で加算される
    /// (wiki: 能力値一覧(その他の部位)「実際は装備命中率補正が増加(数値分固定値で増加)」)。
    EquipmentAccuracy,
    /// 装備回避率補正へ数値ぶんの固定値で加算される(wiki: 同表「命中率」の隣の行「回避率」)。
    EquipmentEvasion,
    /// ステの最終固定値へ加算される(STAB〜AGI)。
    Stat(StatKind),
    /// 防御力の最終値へ加算される(物理/魔法ダメージ耐性・クリティカル被撃率減少)。
    /// 防御側の最終固定値レイヤーが未モデルなので、この効き先は**記録するだけ**。
    DefenseFinal,
}

impl SienaValueKind {
    pub const ALL: [SienaValueKind; 18] = [
        SienaValueKind::Thrust,
        SienaValueKind::Slash,
        SienaValueKind::MagicAttack,
        SienaValueKind::MagicDefense,
        SienaValueKind::PhysicalComposite,
        SienaValueKind::MagicSlash,
        SienaValueKind::PhysicalResist,
        SienaValueKind::MagicResist,
        SienaValueKind::CriticalTakenReduction,
        SienaValueKind::Accuracy,
        SienaValueKind::Evasion,
        SienaValueKind::Stab,
        SienaValueKind::Hack,
        SienaValueKind::Int,
        SienaValueKind::Def,
        SienaValueKind::Mr,
        SienaValueKind::Dex,
        SienaValueKind::Agi,
    ];

    /// 表示名(wiki 一覧表の「能力値の種類」列)。
    pub fn label(self) -> &'static str {
        use SienaValueKind::*;
        match self {
            Thrust => "突き攻撃力",
            Slash => "斬り攻撃力",
            MagicAttack => "魔法攻撃力",
            MagicDefense => "魔法防御力",
            PhysicalComposite => "物理複合攻撃力",
            MagicSlash => "魔法斬り攻撃力",
            PhysicalResist => "物理ダメージ耐性",
            MagicResist => "魔法ダメージ耐性",
            CriticalTakenReduction => "クリティカル被撃率減少",
            Accuracy => "命中率",
            Evasion => "回避率",
            Stab => "STAB",
            Hack => "HACK",
            Int => "INT",
            Def => "DEF",
            Mr => "MR",
            Dex => "DEX",
            Agi => "AGI",
        }
    }

    /// 一覧の行に並べる短い名前(「物理複合攻撃力」をそのまま並べると 1 行に入らない)。
    pub fn short(self) -> &'static str {
        use SienaValueKind::*;
        match self {
            Thrust => "突",
            Slash => "斬",
            MagicAttack => "魔攻",
            MagicDefense => "魔防",
            PhysicalComposite => "物複",
            MagicSlash => "魔斬",
            PhysicalResist => "物耐",
            MagicResist => "魔耐",
            CriticalTakenReduction => "被Cri",
            Accuracy => "命中",
            Evasion => "回避",
            Stab | Hack | Int | Def | Mr | Dex | Agi => self.label(),
        }
    }

    /// この種類の効き先(§効き先を型にする)。
    pub fn effect(self) -> SienaEffect {
        use SienaValueKind::*;
        match self {
            Thrust | Slash | MagicAttack | MagicDefense | PhysicalComposite | MagicSlash => {
                SienaEffect::EquipmentValue
            }
            Accuracy => SienaEffect::EquipmentAccuracy,
            Evasion => SienaEffect::EquipmentEvasion,
            PhysicalResist | MagicResist | CriticalTakenReduction => SienaEffect::DefenseFinal,
            Stab => SienaEffect::Stat(StatKind::Stab),
            Hack => SienaEffect::Stat(StatKind::Hack),
            Int => SienaEffect::Stat(StatKind::Int),
            Def => SienaEffect::Stat(StatKind::Def),
            Mr => SienaEffect::Stat(StatKind::Mr),
            Dex => SienaEffect::Stat(StatKind::Dex),
            Agi => SienaEffect::Stat(StatKind::Agi),
        }
    }

    /// この種類が出る部位かどうか。武器/盾の一覧(=強化能力値の 6 種)と、
    /// その他の部位の一覧(それ以外全部)で丸ごと違う
    /// (wiki: 能力値一覧(武器/盾)・能力値一覧(その他の部位)は 2 つの独立した表で、
    /// 後者は兜/鎧/頭/体/手/足の 6 部位に共通 ── 部位ごとの内訳は wiki に無い。
    /// 取得日 2026-09-01)。
    pub fn allowed_on(self, slot: PartSlot) -> bool {
        self.is_equipment_value() == slot.siena_values_are_equipment()
    }

    /// 装備補正(強化能力値)へ入る種類 = 武器/盾の一覧。
    pub fn is_equipment_value(self) -> bool {
        matches!(self.effect(), SienaEffect::EquipmentValue)
    }

    /// 値域(wiki 一覧表の「数値」列。確率帯をまたいだ最小〜最大)。
    pub fn range(self) -> (i64, i64) {
        use SienaValueKind::*;
        match self {
            PhysicalResist | MagicResist | CriticalTakenReduction => (1, 3),
            Accuracy | Evasion => (1, 6),
            _ => (1, 10),
        }
    }

    /// 単位(値の隣に常設する。§07「上限は値の隣に常設する」)。
    pub fn unit(self) -> &'static str {
        use SienaValueKind::*;
        match self {
            PhysicalResist | MagicResist | CriticalTakenReduction | Accuracy | Evasion => "%",
            _ => "",
        }
    }

    /// 与ダメージ式に入るか。`false` は**記録するだけ**(ランダムOP のグレー枠と同じ扱い)。
    /// 防御側の最終固定値レイヤーが未モデルな `DefenseFinal` だけが未収録。
    pub fn is_modeled(self) -> bool {
        !matches!(self.effect(), SienaEffect::DefenseFinal)
    }

    /// 記録のみの理由・効き先の注記(画面にそのまま出す)。モデル済みで注記不要なら空。
    pub fn note(self) -> &'static str {
        use SienaValueKind::*;
        match self {
            PhysicalResist => "実際は物理防御力の最終値が増加(防御側は未収録)",
            MagicResist => "実際は魔法防御力の最終値が増加(防御側は未収録)",
            CriticalTakenReduction => "被撃側の値(wiki 未検証・防御側は未収録)",
            Accuracy => "実際は装備命中率補正が固定値で増加",
            Evasion => "実際は装備回避率補正が固定値で増加",
            PhysicalComposite => "突き・斬りに分かれて入る",
            MagicSlash => "魔攻・斬りに分かれて入る",
            _ => "",
        }
    }

    /// STAB〜AGI の種類ならその `StatKind`。
    fn stat_kind(self) -> Option<StatKind> {
        match self.effect() {
            SienaEffect::Stat(kind) => Some(kind),
            _ => None,
        }
    }

    /// この種類の値 `value` が装備補正のどこに入るか。
    /// - `EquipmentValue`(武器/盾の 6 種)は強化能力値へ。複合(物理複合・魔法斬り)は
    ///   wiki の内訳どおりに分かれる(5 = 3 + 2)
    /// - `EquipmentAccuracy` / `EquipmentEvasion`(命中率・回避率)は数値そのまま
    ///   装備命中率補正・装備回避率補正へ固定値加算
    /// - `Stat` / `DefenseFinal` はここでは扱わない(`stat_kind` / 防御側 未モデル)
    fn apply_to_values(self, value: i64, values: &mut EquipmentValues) {
        use SienaValueKind::*;
        // 複合は「大きいほうが先」。wiki の例 5 → 3 + 2
        let major = (value + 1) / 2;
        match self.effect() {
            SienaEffect::EquipmentValue => match self {
                Thrust => values.thrust += value,
                Slash => values.slash += value,
                MagicAttack => values.magic_attack += value,
                MagicDefense => values.magic_defense += value,
                PhysicalComposite => {
                    values.thrust += major;
                    values.slash += value - major;
                }
                MagicSlash => {
                    values.magic_attack += major;
                    values.slash += value - major;
                }
                _ => unreachable!("EquipmentValue effect の種類はここで網羅済み"),
            },
            SienaEffect::EquipmentAccuracy => values.accuracy += value,
            SienaEffect::EquipmentEvasion => values.evasion += value,
            SienaEffect::Stat(_) | SienaEffect::DefenseFinal => {}
        }
    }
}

/// 追加オプションの種類(wiki: 追加オプション一覧。全部位で共通)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SienaExtraKind {
    /// 攻撃力増加 %。実際は与ダメージ割合増加(新規カテゴリ)= New1
    AttackRate,
    /// 防御力増加 %。実際は装備防御力倍率増加(プロテクトアーマーなどと加算)
    DefenseRate,
    /// 防御無視攻撃確率 %(確率発動なので与ダメージ式には入れない)
    DefenseIgnoreChance,
    /// 中ディレイ減少 %(倍率B へ合流)
    ActualDelay,
    /// 全ステータス増加。STAB〜AGI 全部にこの値がそのまま加算される(最終固定値増加)
    AllStats,
    /// クリティカル確率 %。クリティカル率の AGI 由来の項に乗算で効く
    CriticalRate,
    /// HP 増加 %(与ダメージ式に入らない)
    Hp,
    /// MP 増加 %(同上)
    Mp,
    /// SP 増加 %(同上)
    Sp,
}

impl SienaExtraKind {
    pub const ALL: [SienaExtraKind; 9] = [
        SienaExtraKind::AttackRate,
        SienaExtraKind::DefenseRate,
        SienaExtraKind::DefenseIgnoreChance,
        SienaExtraKind::ActualDelay,
        SienaExtraKind::AllStats,
        SienaExtraKind::CriticalRate,
        SienaExtraKind::Hp,
        SienaExtraKind::Mp,
        SienaExtraKind::Sp,
    ];

    /// 一覧の行に並べる短い名前。
    pub fn short(self) -> &'static str {
        use SienaExtraKind::*;
        match self {
            AttackRate => "攻撃",
            DefenseRate => "防御",
            DefenseIgnoreChance => "防無",
            ActualDelay => "中D",
            AllStats => "全ステ",
            CriticalRate => "Cri",
            Hp => "HP",
            Mp => "MP",
            Sp => "SP",
        }
    }

    pub fn label(self) -> &'static str {
        use SienaExtraKind::*;
        match self {
            AttackRate => "攻撃力増加",
            DefenseRate => "防御力増加",
            DefenseIgnoreChance => "防御無視攻撃確率",
            ActualDelay => "中ディレイ減少",
            AllStats => "全ステータス増加",
            CriticalRate => "クリティカル確率",
            Hp => "HP増加",
            Mp => "MP増加",
            Sp => "SP増加",
        }
    }

    /// 取りうる値(wiki 一覧表の「数値」列)。飛び飛びなので**選択肢そのもの**を返す。
    /// 段階選択(チップ)に並べられる数に収まっている(§07)。
    pub fn choices(self) -> Vec<f64> {
        use SienaExtraKind::*;
        match self {
            AttackRate | DefenseRate | CriticalRate => (1..=10).map(f64::from).collect(),
            DefenseIgnoreChance => vec![1.0, 2.0, 3.0],
            ActualDelay => vec![0.5, 1.0, 2.0],
            AllStats => (5..=30).map(f64::from).collect(),
            Hp => (5..=20).map(f64::from).collect(),
            Mp | Sp => (1..=15).map(f64::from).collect(),
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            SienaExtraKind::AllStats => "",
            _ => "%",
        }
    }

    /// 与ダメージ式に入るか。`false` は記録するだけ。
    pub fn is_modeled(self) -> bool {
        use SienaExtraKind::*;
        !matches!(self, DefenseIgnoreChance | Hp | Mp | Sp)
    }

    /// 効き先(画面にそのまま出す)。
    pub fn note(self) -> &'static str {
        use SienaExtraKind::*;
        match self {
            AttackRate => "与ダメージ割合増加(New1)",
            DefenseRate => "装備防御力倍率(防御タブ)",
            DefenseIgnoreChance => "確率発動なので未収録",
            ActualDelay => "中ディレイ倍率B(1 秒あたり)",
            AllStats => "STAB〜AGI 全部に加算",
            CriticalRate => "クリティカル率(AGI 由来の項に乗算)",
            Hp | Mp | Sp => "与ダメージ式に入らない",
        }
    }
}

/// 画面が能力値スロットの選択肢を並べるためのカタログ 1 行。
/// wiki の一覧表(種類・数値・備考)をそのまま持つ。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SienaValueKindDef {
    pub kind: SienaValueKind,
    pub label: &'static str,
    /// 一覧の行に並べる短い名前
    pub short: &'static str,
    /// 武器/盾の一覧か(`false` = その他の部位の一覧)
    pub is_equipment_value: bool,
    pub min: i64,
    pub max: i64,
    pub unit: &'static str,
    /// 与ダメージ式に入るか(`false` = 記録するだけ)
    pub is_modeled: bool,
    pub note: &'static str,
}

/// 追加オプションのカタログ 1 行。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SienaExtraKindDef {
    pub kind: SienaExtraKind,
    pub label: &'static str,
    /// 一覧の行に並べる短い名前
    pub short: &'static str,
    /// 取りうる値そのもの(飛び飛びなので min/max では表せない)
    pub choices: Vec<f64>,
    pub unit: &'static str,
    pub is_modeled: bool,
    pub note: &'static str,
}

/// シエナのオーラで選べるもの一式。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SienaCatalog {
    pub values: Vec<SienaValueKindDef>,
    pub extras: Vec<SienaExtraKindDef>,
    /// 追加オプションが 1 個ずつ解放される段階
    pub extra_unlock_stages: [usize; 3],
    pub stage_max: usize,
}

/// wiki の一覧表をそのまま画面へ渡す。
pub fn siena_catalog() -> SienaCatalog {
    SienaCatalog {
        values: SienaValueKind::ALL
            .iter()
            .map(|&kind| {
                let (min, max) = kind.range();
                SienaValueKindDef {
                    kind,
                    label: kind.label(),
                    short: kind.short(),
                    is_equipment_value: kind.is_equipment_value(),
                    min,
                    max,
                    unit: kind.unit(),
                    is_modeled: kind.is_modeled(),
                    note: kind.note(),
                }
            })
            .collect(),
        extras: SienaExtraKind::ALL
            .iter()
            .map(|&kind| SienaExtraKindDef {
                kind,
                label: kind.label(),
                short: kind.short(),
                choices: kind.choices(),
                unit: kind.unit(),
                is_modeled: kind.is_modeled(),
                note: kind.note(),
            })
            .collect(),
        extra_unlock_stages: SIENA_EXTRA_UNLOCK_STAGES,
        stage_max: SIENA_STAGE_MAX,
    }
}

/// 能力値スロット 1 個。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SienaSlot {
    pub kind: SienaValueKind,
    /// wiki 一覧表の「数値」列の実測値。単位は `SienaValueKind::unit`
    pub value: i64,
}

/// 追加オプションスロット 1 個。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SienaExtraSlot {
    pub kind: SienaExtraKind,
    /// 中ディレイ減少に 0.5% があるので実数
    pub value: f64,
}

/// 1 部位分のシエナのオーラ。**段階は `slots.len()`** で、別に保持しない。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SienaAura {
    /// 解放済み能力値スロットの中身。並び順は入力順(ゲーム内のスロット順)
    #[serde(default)]
    pub slots: Vec<SienaSlot>,
    /// 解放済み追加オプションスロットの中身
    #[serde(default)]
    pub extras: Vec<SienaExtraSlot>,
}

impl SienaAura {
    /// 増幅段階 = 能力値スロットの数(wiki: 段階ごとに 1 個解放)。
    pub fn stage(&self) -> usize {
        self.slots.len()
    }

    /// いま解放されている追加オプションの枠数(段階 3/7/10 で 1/2/3)。
    pub fn extra_capacity(&self) -> usize {
        SIENA_EXTRA_UNLOCK_STAGES
            .iter()
            .filter(|s| self.stage() >= **s)
            .count()
    }

    pub fn is_neutral(&self) -> bool {
        self.slots.is_empty() && self.extras.is_empty()
    }

    /// 装備補正へ入る合計(武器/盾)。複合は wiki の内訳どおりに分かれる。
    pub fn values(&self) -> EquipmentValues {
        let mut values = EquipmentValues::default();
        for slot in &self.slots {
            slot.kind.apply_to_values(slot.value, &mut values);
        }
        values
    }

    /// 能力値スロットの STAB〜AGI 合計(武器/盾以外)。
    pub fn stats(&self) -> SienaStatBonus {
        let mut stats = SienaStatBonus::default();
        for slot in &self.slots {
            if let Some(kind) = slot.kind.stat_kind() {
                *stats.get_mut(kind) += slot.value;
            }
        }
        stats
    }

    /// 追加オプション「全ステータス増加」。同じ種類は 1 部位に 1 個までなので合計で足りる。
    pub fn all_stats(&self) -> i64 {
        crate::rounding::round_int(self.extra_total(SienaExtraKind::AllStats))
    }

    /// この部位のステ加算の合計(能力値スロット + 全ステータス増加)。
    pub fn stat_bonus(&self) -> SienaStatBonus {
        let mut total = self.stats();
        let all = self.all_stats();
        if all != 0 {
            for kind in StatKind::ALL {
                *total.get_mut(kind) += all;
            }
        }
        total
    }

    pub fn attack_rate_percent(&self) -> f64 {
        self.extra_total(SienaExtraKind::AttackRate)
    }

    pub fn defense_rate_percent(&self) -> f64 {
        self.extra_total(SienaExtraKind::DefenseRate)
    }

    pub fn actual_delay_percent(&self) -> f64 {
        self.extra_total(SienaExtraKind::ActualDelay)
    }

    pub fn critical_rate_percent(&self) -> f64 {
        self.extra_total(SienaExtraKind::CriticalRate)
    }

    fn extra_total(&self, kind: SienaExtraKind) -> f64 {
        self.extras
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.value)
            .sum()
    }

    pub(crate) fn validate(&self, slot: PartSlot) -> Result<(), SienaError> {
        if self.is_neutral() {
            return Ok(());
        }
        if !slot.allows_siena() {
            return Err(SienaError::NotAllowed { slot });
        }
        if self.slots.len() > SIENA_STAGE_MAX {
            return Err(SienaError::TooManySlots {
                slot,
                count: self.slots.len(),
                max: SIENA_STAGE_MAX,
            });
        }
        for value_slot in &self.slots {
            if !value_slot.kind.allowed_on(slot) {
                return Err(SienaError::KindNotAllowed {
                    slot,
                    kind: value_slot.kind,
                });
            }
            let (min, max) = value_slot.kind.range();
            if !(min..=max).contains(&value_slot.value) {
                return Err(SienaError::ValueOutOfRange {
                    slot,
                    kind: value_slot.kind,
                    value: value_slot.value,
                    min,
                    max,
                });
            }
        }
        if self.extras.len() > self.extra_capacity() {
            return Err(SienaError::TooManyExtras {
                slot,
                count: self.extras.len(),
                capacity: self.extra_capacity(),
                stage: self.stage(),
            });
        }
        for (i, extra) in self.extras.iter().enumerate() {
            // wiki:「同じ種類のオプションは別のスロットには登場しない」(同じ装備の中で)
            if self.extras[..i].iter().any(|e| e.kind == extra.kind) {
                return Err(SienaError::DuplicateExtra {
                    slot,
                    kind: extra.kind,
                });
            }
            if !extra
                .kind
                .choices()
                .iter()
                .any(|c| (c - extra.value).abs() < 1e-9)
            {
                return Err(SienaError::ExtraValueOutOfRange {
                    slot,
                    kind: extra.kind,
                    value: extra.value,
                });
            }
        }
        Ok(())
    }
}

/// 抽出して所持しているシエナのオーラ 1 個。
/// wiki「抽出・注入」により、オーラは装備から分離して同一部位へ付け替えられる。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RegisteredSienaAura {
    /// キャラ内で不変の登録ID。
    #[serde(default)]
    pub id: u64,
    /// 同じ部位の複数オーラを見分ける任意ラベル。
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub aura: SienaAura,
}

/// 同一部位に注入できるオーラの所持一覧と、現在装着中の 1 個。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SienaAuraList {
    #[serde(default)]
    pub registered: Vec<RegisteredSienaAura>,
    #[serde(default)]
    pub selected_id: Option<u64>,
}

impl SienaAuraList {
    pub fn selected(&self) -> Option<&RegisteredSienaAura> {
        self.selected_id
            .and_then(|id| self.registered.iter().find(|entry| entry.id == id))
    }

    pub fn selected_mut(&mut self) -> Option<&mut RegisteredSienaAura> {
        let id = self.selected_id?;
        self.registered.iter_mut().find(|entry| entry.id == id)
    }

    pub fn validate(&self, slot: PartSlot) -> Result<(), SienaError> {
        if self.selected_id.is_some() && self.selected().is_none() {
            return Err(SienaError::UnknownSelectedId { slot });
        }
        let mut ids = std::collections::HashSet::new();
        for entry in &self.registered {
            if entry.id == 0 || !ids.insert(entry.id) {
                return Err(SienaError::DuplicateRegistrationId { slot, id: entry.id });
            }
            entry.aura.validate(slot)?;
        }
        Ok(())
    }
}

/// 発現可能な 8 部位のオーラ所持一覧。装備品の登録一覧とは独立して切り替える。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SienaAuras {
    #[serde(default)]
    pub weapon: SienaAuraList,
    #[serde(default)]
    pub armor: SienaAuraList,
    #[serde(default)]
    pub helm: SienaAuraList,
    #[serde(default)]
    pub shield: SienaAuraList,
    #[serde(default)]
    pub head: SienaAuraList,
    #[serde(default)]
    pub body: SienaAuraList,
    #[serde(default)]
    pub hand: SienaAuraList,
    #[serde(default)]
    pub leg: SienaAuraList,
}

impl SienaAuras {
    const SLOTS: [PartSlot; 8] = [
        PartSlot::Weapon,
        PartSlot::Armor,
        PartSlot::Helm,
        PartSlot::Shield,
        PartSlot::Head,
        PartSlot::Body,
        PartSlot::Hand,
        PartSlot::Leg,
    ];

    pub fn get(&self, slot: PartSlot) -> Option<&SienaAuraList> {
        match slot {
            PartSlot::Weapon => Some(&self.weapon),
            PartSlot::Armor => Some(&self.armor),
            PartSlot::Helm => Some(&self.helm),
            PartSlot::Shield => Some(&self.shield),
            PartSlot::Head => Some(&self.head),
            PartSlot::Body => Some(&self.body),
            PartSlot::Hand => Some(&self.hand),
            PartSlot::Leg => Some(&self.leg),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, slot: PartSlot) -> Option<&mut SienaAuraList> {
        match slot {
            PartSlot::Weapon => Some(&mut self.weapon),
            PartSlot::Armor => Some(&mut self.armor),
            PartSlot::Helm => Some(&mut self.helm),
            PartSlot::Shield => Some(&mut self.shield),
            PartSlot::Head => Some(&mut self.head),
            PartSlot::Body => Some(&mut self.body),
            PartSlot::Hand => Some(&mut self.hand),
            PartSlot::Leg => Some(&mut self.leg),
            _ => None,
        }
    }

    pub fn iter_selected(&self) -> impl Iterator<Item = (PartSlot, &SienaAura)> {
        Self::SLOTS
            .into_iter()
            .filter_map(|slot| self.get(slot)?.selected().map(|entry| (slot, &entry.aura)))
    }

    pub fn validate(&self) -> Result<(), SienaError> {
        for slot in Self::SLOTS {
            self.get(slot).expect("allowed Siena slot").validate(slot)?;
        }
        Ok(())
    }
}

/// シエナのオーラの入力値・部位制約違反。
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum SienaError {
    #[error("{slot:?} の選択中オーラIDは登録一覧にありません")]
    UnknownSelectedId { slot: PartSlot },
    #[error("{slot:?} のオーラ登録ID {id} が重複または0です")]
    DuplicateRegistrationId { slot: PartSlot, id: u64 },
    #[error("{slot:?} はシエナのオーラの対象外です(兜/鎧/武器/盾/頭/体/手/足のみ)")]
    NotAllowed { slot: PartSlot },
    #[error("{slot:?} の能力値スロットは {max} 個までです(指定 {count} 個)")]
    TooManySlots {
        slot: PartSlot,
        count: usize,
        max: usize,
    },
    #[error("{slot:?} に「{}」は出ません", kind.label())]
    KindNotAllowed {
        slot: PartSlot,
        kind: SienaValueKind,
    },
    #[error("{slot:?} の「{}」は {min}〜{max} です(指定値 {value})", kind.label())]
    ValueOutOfRange {
        slot: PartSlot,
        kind: SienaValueKind,
        value: i64,
        min: i64,
        max: i64,
    },
    #[error(
        "{slot:?} は段階 {stage} なので追加オプションは {capacity} 個までです(指定 {count} 個)"
    )]
    TooManyExtras {
        slot: PartSlot,
        count: usize,
        capacity: usize,
        stage: usize,
    },
    #[error("{slot:?} に「{}」は 1 個までです", kind.label())]
    DuplicateExtra {
        slot: PartSlot,
        kind: SienaExtraKind,
    },
    #[error("{slot:?} の「{}」に {value} はありません", kind.label())]
    ExtraValueOutOfRange {
        slot: PartSlot,
        kind: SienaExtraKind,
        value: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(kind: SienaValueKind, value: i64) -> SienaSlot {
        SienaSlot { kind, value }
    }

    #[test]
    fn stage_is_slot_count_and_extra_capacity_follows_it() {
        let mut aura = SienaAura::default();
        assert_eq!((aura.stage(), aura.extra_capacity()), (0, 0));
        for _ in 0..3 {
            aura.slots.push(slot(SienaValueKind::Thrust, 1));
        }
        assert_eq!((aura.stage(), aura.extra_capacity()), (3, 1));
        for _ in 3..7 {
            aura.slots.push(slot(SienaValueKind::Thrust, 1));
        }
        assert_eq!((aura.stage(), aura.extra_capacity()), (7, 2));
        for _ in 7..10 {
            aura.slots.push(slot(SienaValueKind::Thrust, 1));
        }
        assert_eq!((aura.stage(), aura.extra_capacity()), (10, 3));
    }

    #[test]
    fn composite_kinds_split_the_way_the_wiki_shows() {
        let aura = SienaAura {
            slots: vec![
                slot(SienaValueKind::PhysicalComposite, 5),
                slot(SienaValueKind::MagicSlash, 5),
            ],
            extras: vec![],
        };
        let values = aura.values();
        // 物理複合5 = 突き3 + 斬り2、魔法斬り5 = 魔攻3 + 斬り2
        assert_eq!(values.thrust, 3);
        assert_eq!(values.magic_attack, 3);
        assert_eq!(values.slash, 4);
    }

    #[test]
    fn accuracy_and_evasion_slots_add_to_equipment_values() {
        // wiki: 能力値一覧(その他の部位)「命中率」「回避率」は
        // 「実際は装備命中率補正/装備回避率補正が数値分固定値で増加」
        let aura = SienaAura {
            slots: vec![slot(SienaValueKind::Accuracy, 6), slot(SienaValueKind::Evasion, 3)],
            extras: vec![],
        };
        let values = aura.values();
        assert_eq!(values.accuracy, 6);
        assert_eq!(values.evasion, 3);
        // 命中率・回避率は強化能力値(突き〜魔法斬り)には乗らない
        assert_eq!(values.thrust, 0);
        assert_eq!(values.slash, 0);
    }

    #[test]
    fn defense_final_kinds_stay_unmodeled() {
        // 物理/魔法ダメージ耐性・被Cri減少は防御側未モデルなので、
        // 装備補正にもステの最終固定値にも入らない(記録するだけ)。
        let aura = SienaAura {
            slots: vec![
                slot(SienaValueKind::PhysicalResist, 2),
                slot(SienaValueKind::MagicResist, 2),
                slot(SienaValueKind::CriticalTakenReduction, 1),
            ],
            extras: vec![],
        };
        assert_eq!(aura.values(), EquipmentValues::default());
        assert_eq!(aura.stat_bonus(), SienaStatBonus::default());
        for k in [
            SienaValueKind::PhysicalResist,
            SienaValueKind::MagicResist,
            SienaValueKind::CriticalTakenReduction,
        ] {
            assert_eq!(k.effect(), SienaEffect::DefenseFinal);
            assert!(!k.is_modeled());
        }
    }

    #[test]
    fn stat_slots_and_all_stats_add_up() {
        let aura = SienaAura {
            slots: vec![slot(SienaValueKind::Stab, 10), slot(SienaValueKind::Agi, 4)],
            extras: vec![SienaExtraSlot {
                kind: SienaExtraKind::AllStats,
                value: 30.0,
            }],
        };
        let bonus = aura.stat_bonus();
        assert_eq!(bonus.stab, 40);
        assert_eq!(bonus.agi, 34);
        assert_eq!(bonus.hack, 30);
    }

    #[test]
    fn kind_lists_do_not_mix_between_weapon_and_other_parts() {
        let weapon = SienaAura {
            slots: vec![slot(SienaValueKind::Stab, 1)],
            extras: vec![],
        };
        assert!(matches!(
            weapon.validate(PartSlot::Weapon),
            Err(SienaError::KindNotAllowed { .. })
        ));
        let helm = SienaAura {
            slots: vec![slot(SienaValueKind::Thrust, 1)],
            extras: vec![],
        };
        assert!(matches!(
            helm.validate(PartSlot::Helm),
            Err(SienaError::KindNotAllowed { .. })
        ));
    }

    #[test]
    fn extras_are_capped_by_stage_and_unique_by_kind() {
        let two_slots = vec![slot(SienaValueKind::Stab, 1), slot(SienaValueKind::Agi, 1)];
        let aura = SienaAura {
            slots: two_slots.clone(),
            extras: vec![SienaExtraSlot {
                kind: SienaExtraKind::AttackRate,
                value: 10.0,
            }],
        };
        // 段階 2 は追加オプション 0 枠
        assert!(matches!(
            aura.validate(PartSlot::Helm),
            Err(SienaError::TooManyExtras { .. })
        ));

        let mut slots = two_slots;
        slots.extend(std::iter::repeat(slot(SienaValueKind::Stab, 1)).take(5));
        let aura = SienaAura {
            slots,
            extras: vec![
                SienaExtraSlot {
                    kind: SienaExtraKind::AttackRate,
                    value: 10.0,
                },
                SienaExtraSlot {
                    kind: SienaExtraKind::AttackRate,
                    value: 3.0,
                },
            ],
        };
        assert!(matches!(
            aura.validate(PartSlot::Helm),
            Err(SienaError::DuplicateExtra { .. })
        ));
    }

    #[test]
    fn values_outside_the_wiki_range_are_rejected() {
        let aura = SienaAura {
            slots: vec![slot(SienaValueKind::Accuracy, 7)],
            extras: vec![],
        };
        assert!(matches!(
            aura.validate(PartSlot::Helm),
            Err(SienaError::ValueOutOfRange { min: 1, max: 6, .. })
        ));
        let aura = SienaAura {
            slots: vec![
                slot(SienaValueKind::Stab, 1),
                slot(SienaValueKind::Stab, 1),
                slot(SienaValueKind::Stab, 1),
            ],
            extras: vec![SienaExtraSlot {
                kind: SienaExtraKind::ActualDelay,
                value: 1.5,
            }],
        };
        assert!(matches!(
            aura.validate(PartSlot::Helm),
            Err(SienaError::ExtraValueOutOfRange { .. })
        ));
    }
}
