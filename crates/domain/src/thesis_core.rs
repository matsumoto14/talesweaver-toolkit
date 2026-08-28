//! テシスコア(wiki: テシスコア)。2 次極限達成後に開放されるキャラクター単位のシステム。
//!
//! - 装着位置は 1〜6 の 6 枠。各枠に 1 個のコアを装着する
//! - コアは「タイプ(火力/補助/経験値)」と「進化段階 0〜4 × 強化段階 0〜4」を持つ
//! - 火力タイプ(突き/斬り/魔攻/魔防)の補正は装備の強化能力値に合流する
//!   (docs/damage-formula.md §4 A「強化能力値(エンチャント・テシスコア・シエナのオーラ等)」)
//! - 能力値の増加は対象ダンジョン内でのみ有効。セット効果は全地域で発動する
//!
//! 補助タイプ(物理防御力・回避率補正・敏捷度補正・命中率補正)も収録する(ユーザー要望)。
//! 装備補正 9 値に持ち場があるので強化能力値(`equipment_values`)に合流する。与ダメージ式の
//! 装備係数はこの 4 種が 0 なので攻撃力には効かず、防御側(§6)と回避P(§7)にだけ効く。
//! 入場条件「コア N」の合計には火力と同じように効く。経験値タイプはシオカンヘイム専用で、
//! シオカンヘイムのコアはセット効果も経験値獲得量なので地域ごと収録しない
//! (wiki「実装済みダンジョンコア」「コアセット効果」、ユーザー確認 2026-08-24)。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::equipment::EquipmentValues;

/// 装着位置の数(wiki: テシスコア効果「装着位置」1〜6)。
pub const CORE_SLOT_COUNT: usize = 6;
/// 進化段階の上限(wiki: 進化強化表)。
pub const CORE_EVOLUTION_MAX: u8 = 4;
/// 強化段階の上限(wiki: 進化強化表)。
pub const CORE_ENHANCEMENT_MAX: u8 = 4;
/// セット効果が発動する強化段階(wiki: コアセット効果「4段階強化まで到達したテシスコア」)。
const SET_BONUS_ENHANCEMENT: u8 = CORE_ENHANCEMENT_MAX;
/// 3〜5 セット効果が発動する個数。
const SET_BONUS_MIN_COUNT: usize = 3;

/// 火力タイプの補正値(wiki: 進化強化表の「火力」列)。添字は [進化段階][強化段階]。
/// UI がリテラルで持たず参照するための公開テーブル(`StatLimits::core_power_bonus_table`)。
pub const POWER_BONUS: [[i64; 5]; 5] = [
    [1, 2, 3, 4, 5],
    [6, 7, 8, 9, 10],
    [12, 14, 16, 18, 20],
    [23, 26, 29, 32, 35],
    [40, 50, 60, 70, 80],
];

/// 補助タイプの補正値(wiki: 進化強化表の「補助」列)。進化3 までは火力と同じで、
/// 進化4 の強化1 以降だけ分かれる(火力 50/60/70/80 に対して 45/50/55/60)。
/// UI がリテラルで持たず参照するための公開テーブル(`StatLimits::core_support_bonus_table`)。
pub const SUPPORT_BONUS: [[i64; 5]; 5] = [
    [1, 2, 3, 4, 5],
    [6, 7, 8, 9, 10],
    [12, 14, 16, 18, 20],
    [23, 26, 29, 32, 35],
    [40, 45, 50, 55, 60],
];

/// テシスコアの地域(wiki: テシスコア「実装済みダンジョンコア」)。
/// コアの能力値増加はこの地域のダンジョン内でのみ有効。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreRegion {
    /// マーキュリアル洞窟(ルミナスの試練・プシーキーの迷宮・プシーキーの虚像でも発動)
    Mercurial,
    /// アビス(アークロン要塞・守護者の部屋・深淵の狭間・アークロン地下でも発動)
    Abyss,
    /// エクリプス(アフェティリアダンジョンでも発動)
    Eclipse,
    /// ルビコナ(ゆがんだ村)
    Rubicona,
}

impl CoreRegion {
    pub const ALL: [CoreRegion; 4] =
        [CoreRegion::Mercurial, CoreRegion::Abyss, CoreRegion::Eclipse, CoreRegion::Rubicona];

    pub fn label(self) -> &'static str {
        match self {
            CoreRegion::Mercurial => "マーキュリアル洞窟",
            CoreRegion::Abyss => "アビス",
            CoreRegion::Eclipse => "エクリプス",
            CoreRegion::Rubicona => "ルビコナ",
        }
    }
}

/// コアのタイプ(wiki: テシスコア効果「タイプ」の(火力)行と(補助)行)。
/// 経験値タイプはシオカンヘイム専用なので持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreType {
    // ---- 火力(強化能力値に入る)
    Thrust,
    Slash,
    MagicAttack,
    MagicDefense,
    // ---- 補助(与ダメージ式に効かない。記録と入場条件「コア N」の合計にのみ使う)
    PhysicalDefense,
    Evasion,
    Agility,
    Accuracy,
}

impl CoreType {
    pub const ALL: [CoreType; 8] = [
        CoreType::Thrust,
        CoreType::Slash,
        CoreType::MagicAttack,
        CoreType::MagicDefense,
        CoreType::PhysicalDefense,
        CoreType::Evasion,
        CoreType::Agility,
        CoreType::Accuracy,
    ];

    /// 表示名。`EquipmentValues::fields` の対応する 9 値のうち 8 種と表記を共有する
    /// (唯一の正は `EquipmentValues`。ここで独自の表記を持たない)。
    pub fn label(self) -> &'static str {
        match self {
            CoreType::Thrust => EquipmentValues::THRUST_LABEL,
            CoreType::Slash => EquipmentValues::SLASH_LABEL,
            CoreType::MagicAttack => EquipmentValues::MAGIC_ATTACK_LABEL,
            CoreType::MagicDefense => EquipmentValues::MAGIC_DEFENSE_LABEL,
            CoreType::PhysicalDefense => EquipmentValues::PHYSICAL_DEFENSE_LABEL,
            CoreType::Evasion => EquipmentValues::EVASION_LABEL,
            CoreType::Agility => EquipmentValues::AGILITY_LABEL,
            CoreType::Accuracy => EquipmentValues::ACCURACY_LABEL,
        }
    }

    /// 装備攻撃力(強化能力値)に入る火力タイプか。補助タイプは false。
    pub fn is_power(self) -> bool {
        matches!(
            self,
            CoreType::Thrust | CoreType::Slash | CoreType::MagicAttack | CoreType::MagicDefense
        )
    }
}

/// コア 1 個。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThesisCore {
    pub core_type: CoreType,
    /// 進化段階 0..=4
    pub evolution: u8,
    /// 強化段階 0..=4
    pub enhancement: u8,
}

impl ThesisCore {
    /// 補正値(wiki: 進化強化表。火力タイプは「火力」列、命中は「補助」列)。
    /// 値域外(検証前のデータ)は 0 を返す。
    pub fn bonus(&self) -> i64 {
        let table = if self.core_type.is_power() { &POWER_BONUS } else { &SUPPORT_BONUS };
        table
            .get(self.evolution as usize)
            .and_then(|row| row.get(self.enhancement as usize))
            .copied()
            .unwrap_or(0)
    }

    fn validate(&self, region: CoreRegion, slot: usize) -> Result<(), ThesisCoreError> {
        if self.evolution > CORE_EVOLUTION_MAX {
            return Err(ThesisCoreError::EvolutionOutOfRange {
                region,
                slot,
                value: self.evolution,
                max: CORE_EVOLUTION_MAX,
            });
        }
        if self.enhancement > CORE_ENHANCEMENT_MAX {
            return Err(ThesisCoreError::EnhancementOutOfRange {
                region,
                slot,
                value: self.enhancement,
                max: CORE_ENHANCEMENT_MAX,
            });
        }
        Ok(())
    }
}

/// セット効果(wiki: コアセット効果)。最終ダメージの固定値(カテゴリK)と割合(カテゴリL)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CoreSetBonus {
    /// wiki: K 最終ダメージ(固定値)
    pub final_damage_fixed: i64,
    /// wiki: L 最終ダメージ。Σ% の小数表現(+1% → 0.01)
    pub final_damage_rate: f64,
}

/// 進化段階ごとに成立したセット効果(表示用の内訳)。`CoreSet::set_groups` が返す。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoreSetGroup {
    pub evolution: u8,
    /// 成立に使った枚数(3〜6)
    pub count: usize,
    pub bonus: CoreSetBonus,
}

/// 進化段階 evolution の強化4コアが 3〜5 個 / 6 個そろったときのセット効果(wiki の表)。
fn set_bonus_of(evolution: u8, six: bool) -> CoreSetBonus {
    let (fixed, rate) = match (evolution, six) {
        (0, false) => (500, 0.0),
        (0, true) => (800, 0.0),
        (1, false) => (700, 0.0),
        (1, true) => (1_400, 0.0),
        (2, false) => (1_000, 0.0),
        (2, true) => (0, 0.01),
        (3, false) => (0, 0.01),
        (3, true) => (0, 0.02),
        (4, false) => (0, 0.02),
        (4, true) => (0, 0.05),
        _ => (0, 0.0),
    };
    CoreSetBonus { final_damage_fixed: fixed, final_damage_rate: rate }
}

/// 1 地域分の 6 枠。None の枠は未装着。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CoreSet {
    #[serde(default)]
    pub slots: [Option<ThesisCore>; CORE_SLOT_COUNT],
}

impl CoreSet {
    fn validate(&self, region: CoreRegion) -> Result<(), ThesisCoreError> {
        for (i, core) in self.slots.iter().enumerate() {
            if let Some(core) = core {
                core.validate(region, i + 1)?;
            }
        }
        Ok(())
    }

    /// 6 枠の補正値の合計(swiki コンテンツ入場条件の「コア N」はこの値。
    /// 60 = 6×10(進化1強化4)、120 = 6×20、210 = 6×35、300 = 6×50、480 = 6×80)。
    /// 要求値の 480 は火力タイプでしか届かない(補助は進化4強化4 でも 60 で 6 枠 360)。
    pub fn total_bonus(&self) -> i64 {
        self.slots.iter().flatten().map(ThesisCore::bonus).sum()
    }

    /// 強化能力値への加算(火力 4 タイプ + 補助 4 タイプ。装備補正 9 値にすべて持ち場がある)。
    pub fn equipment_values(&self) -> EquipmentValues {
        let mut values = EquipmentValues::default();
        for core in self.slots.iter().flatten() {
            let bonus = core.bonus();
            match core.core_type {
                CoreType::Thrust => values.thrust += bonus,
                CoreType::Slash => values.slash += bonus,
                CoreType::MagicAttack => values.magic_attack += bonus,
                CoreType::MagicDefense => values.magic_defense += bonus,
                // 補助タイプ。与ダメージ式の係数は 0 なので攻撃力には効かず、防御側・回避Pに効く
                CoreType::PhysicalDefense => values.physical_defense += bonus,
                CoreType::Evasion => values.evasion += bonus,
                CoreType::Agility => values.agility += bonus,
                CoreType::Accuracy => values.accuracy += bonus,
            }
        }
        values
    }

    /// 強化 4 に達しているコアの数(進化段階を問わない。「あと何個で 1 セット目か」を言うのに使う)。
    pub fn ready_count(&self) -> usize {
        self.slots.iter().flatten().filter(|c| c.enhancement >= SET_BONUS_ENHANCEMENT).count()
    }

    /// 進化段階ごとに成立しているセットの内訳(表示用)。`set_bonus` の合算前の内訳。
    ///
    /// セットは**同じ進化段階の強化4 コア**で組む(wiki の表が「3〜5 セット効果 / 6 セット効果」
    /// と個数で分かれているのはこのため)。**進化段階ごとに成立して、成立した分は合算する**
    /// (ユーザー確認 2026-08-26。例: 進化3強化4 ×3 + 進化4強化4 ×3 = 進化3 の 3set +1% と
    /// 進化4 の 3set +2% で合計 +3%)。同じ段階が 6 個そろったときは 6 セット効果になり、
    /// 3 セット効果を重ねては数えない(進化4 ×6 = +5%、+2% は乗らない)。
    pub fn set_groups(&self) -> Vec<CoreSetGroup> {
        let mut groups = Vec::new();
        for evolution in 0..=CORE_EVOLUTION_MAX {
            let count = self
                .slots
                .iter()
                .flatten()
                .filter(|c| c.enhancement >= SET_BONUS_ENHANCEMENT && c.evolution == evolution)
                .count();
            if count < SET_BONUS_MIN_COUNT {
                continue;
            }
            groups.push(CoreSetGroup {
                evolution,
                count,
                bonus: set_bonus_of(evolution, count >= CORE_SLOT_COUNT),
            });
        }
        groups
    }

    /// この地域のセット効果(wiki: コアセット効果)。`set_groups` の合算値。
    /// 地域をまたぐ分もさらに合算する(`ThesisCores::set_bonus`)。
    pub fn set_bonus(&self) -> CoreSetBonus {
        self.set_groups().into_iter().fold(CoreSetBonus::default(), |mut total, group| {
            total.final_damage_fixed += group.bonus.final_damage_fixed;
            total.final_damage_rate += group.bonus.final_damage_rate;
            total
        })
    }
}

/// キャラクターのテシスコア一式(地域ごとに 6 枠)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThesisCores {
    #[serde(default)]
    pub mercurial: CoreSet,
    #[serde(default)]
    pub abyss: CoreSet,
    #[serde(default)]
    pub eclipse: CoreSet,
    #[serde(default)]
    pub rubicona: CoreSet,
}

impl ThesisCores {
    pub fn get(&self, region: CoreRegion) -> &CoreSet {
        match region {
            CoreRegion::Mercurial => &self.mercurial,
            CoreRegion::Abyss => &self.abyss,
            CoreRegion::Eclipse => &self.eclipse,
            CoreRegion::Rubicona => &self.rubicona,
        }
    }

    pub fn get_mut(&mut self, region: CoreRegion) -> &mut CoreSet {
        match region {
            CoreRegion::Mercurial => &mut self.mercurial,
            CoreRegion::Abyss => &mut self.abyss,
            CoreRegion::Eclipse => &mut self.eclipse,
            CoreRegion::Rubicona => &mut self.rubicona,
        }
    }

    pub fn validate(&self) -> Result<(), ThesisCoreError> {
        for region in CoreRegion::ALL {
            self.get(region).validate(region)?;
        }
        Ok(())
    }

    /// その地域での強化能力値への加算。地域が決まらない(コアが効かない)場合は 0。
    pub fn equipment_values(&self, region: Option<CoreRegion>) -> EquipmentValues {
        match region {
            Some(region) => self.get(region).equipment_values(),
            None => EquipmentValues::default(),
        }
    }

    /// 全地域のセット効果の合計。
    ///
    /// wiki「セット効果は全ての地域で発動します」+ セット効果は**重複する**(ユーザー確認
    /// 2026-08-24。進化4強化4 のコアを多く持つほど強い)。K は上限 1000、L は上限 45% で
    /// カテゴリ集計側がクランプする。
    pub fn set_bonus(&self) -> CoreSetBonus {
        let mut total = CoreSetBonus::default();
        for region in CoreRegion::ALL {
            let bonus = self.get(region).set_bonus();
            total.final_damage_fixed += bonus.final_damage_fixed;
            total.final_damage_rate += bonus.final_damage_rate;
        }
        total
    }

    /// 入場条件「コア N」の判定値。**そのコンテンツの地域のコアだけを数える**
    /// (ユーザー確認 2026-08-24)。地域が決まらないコンテンツは 0。
    /// コア要求があるのに地域が無いデータは gamedata のテストで弾く。
    pub fn total_bonus(&self, region: Option<CoreRegion>) -> i64 {
        match region {
            Some(region) => self.get(region).total_bonus(),
            None => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum ThesisCoreError {
    #[error("{region:?} の {slot} 番のテシスコアの進化段階は 0〜{max} です(指定値 {value})")]
    EvolutionOutOfRange { region: CoreRegion, slot: usize, value: u8, max: u8 },
    #[error("{region:?} の {slot} 番のテシスコアの強化段階は 0〜{max} です(指定値 {value})")]
    EnhancementOutOfRange { region: CoreRegion, slot: usize, value: u8, max: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core(core_type: CoreType, evolution: u8, enhancement: u8) -> Option<ThesisCore> {
        Some(ThesisCore { core_type, evolution, enhancement })
    }

    fn filled(evolution: u8, enhancement: u8) -> CoreSet {
        CoreSet { slots: [core(CoreType::Slash, evolution, enhancement); CORE_SLOT_COUNT] }
    }

    #[test]
    fn 火力補正値はwikiの進化強化表どおり() {
        let bonus = |evolution, enhancement| {
            ThesisCore { core_type: CoreType::Thrust, evolution, enhancement }.bonus()
        };
        assert_eq!(bonus(0, 0), 1);
        assert_eq!(bonus(1, 4), 10);
        assert_eq!(bonus(2, 4), 20);
        assert_eq!(bonus(3, 4), 35);
        assert_eq!(bonus(4, 0), 40);
        assert_eq!(bonus(4, 4), 80);
    }

    // swiki の入場条件「コア 60/120/210/300/480」は 6 枠の火力補正合計と一致する
    #[test]
    fn 合計値はswikiのコア要求値と一致する() {
        assert_eq!(filled(1, 4).total_bonus(), 60);
        assert_eq!(filled(2, 4).total_bonus(), 120);
        assert_eq!(filled(3, 4).total_bonus(), 210);
        assert_eq!(filled(4, 1).total_bonus(), 300);
        assert_eq!(filled(4, 4).total_bonus(), 480);
    }

    #[test]
    fn 強化能力値はタイプごとに合流する() {
        let set = CoreSet {
            slots: [
                core(CoreType::Thrust, 4, 4),
                core(CoreType::Slash, 4, 4),
                core(CoreType::Slash, 0, 0),
                core(CoreType::MagicAttack, 2, 0),
                core(CoreType::MagicDefense, 3, 2),
                None,
            ],
        };
        assert_eq!(
            set.equipment_values(),
            EquipmentValues { thrust: 80, slash: 81, magic_attack: 12, magic_defense: 29, ..Default::default() }
        );
        assert_eq!(set.total_bonus(), 80 + 80 + 1 + 12 + 29);
    }

    #[test]
    fn セット効果は個数と進化段階で決まる() {
        // 強化 3 以下はセット効果なし
        assert_eq!(filled(4, 3).set_bonus(), CoreSetBonus::default());

        // 進化0 強化4 が 6 個 → 最終ダメージ +800(固定値)
        assert_eq!(
            filled(0, 4).set_bonus(),
            CoreSetBonus { final_damage_fixed: 800, final_damage_rate: 0.0 }
        );

        // 進化4 強化4 が 6 個 → 最終ダメージ +5%
        let bonus = filled(4, 4).set_bonus();
        assert_eq!(bonus.final_damage_fixed, 0);
        assert!((bonus.final_damage_rate - 0.05).abs() < 1e-12);

        // 進化4 強化4 が 3 個 + 未装着 3 → +2%
        let mut set = filled(4, 4);
        set.slots[3] = None;
        set.slots[4] = None;
        set.slots[5] = None;
        assert!((set.set_bonus().final_damage_rate - 0.02).abs() < 1e-12);

        // 進化段階ごとに成立して合算する。
        // 進化4 強化4 が 3 個 + 進化0 強化4 が 3 個 → 進化4 の 3set(+2%)と 進化0 の 3set(+500)
        let mut mixed = filled(4, 4);
        for slot in mixed.slots.iter_mut().skip(3) {
            *slot = core(CoreType::Slash, 0, 4);
        }
        let bonus = mixed.set_bonus();
        assert!((bonus.final_damage_rate - 0.02).abs() < 1e-12);
        assert_eq!(bonus.final_damage_fixed, 500);

        // 進化3強化4 ×3 + 進化4強化4 ×3 → +1% と +2% で +3%(ユーザー確認 2026-08-26)
        let mut two_sets = CoreSet::default();
        for i in 0..3 {
            two_sets.slots[i] = core(CoreType::Slash, 3, 4);
        }
        for i in 3..6 {
            two_sets.slots[i] = core(CoreType::Slash, 4, 4);
        }
        let bonus = two_sets.set_bonus();
        assert!((bonus.final_damage_rate - 0.03).abs() < 1e-12);
        assert_eq!(bonus.final_damage_fixed, 0);

        // 進化3強化4 が 3 個(残りは強化3 でセット対象外)→ 進化3 の 3set(+1%)
        let mut three = CoreSet::default();
        for i in 0..3 {
            three.slots[i] = core(CoreType::Slash, 3, 4);
        }
        for i in 3..6 {
            three.slots[i] = core(CoreType::Slash, 4, 3);
        }
        assert!((three.set_bonus().final_damage_rate - 0.01).abs() < 1e-12);

        // 3 個に満たない段階は数に入らない。上位のコアが下位のセットを埋めることもない
        // (進化4強化4 が 2 個 + 進化2強化4 が 4 個 → 進化2 の 3〜5set(+1,000)だけ)
        let mut lower = filled(2, 4);
        lower.slots[0] = core(CoreType::Slash, 4, 4);
        lower.slots[1] = core(CoreType::Slash, 4, 4);
        let bonus = lower.set_bonus();
        assert_eq!(bonus.final_damage_fixed, 1_000);
        assert_eq!(bonus.final_damage_rate, 0.0);
    }

    #[test]
    fn 二個ではセット効果が発動しない() {
        let mut set = CoreSet::default();
        set.slots[0] = core(CoreType::Slash, 4, 4);
        set.slots[1] = core(CoreType::Slash, 4, 4);
        assert_eq!(set.set_bonus(), CoreSetBonus::default());
    }

    #[test]
    fn 地域ごとに独立して集計する() {
        let mut cores = ThesisCores::default();
        *cores.get_mut(CoreRegion::Abyss) = filled(2, 4);
        *cores.get_mut(CoreRegion::Eclipse) = filled(4, 4);

        assert_eq!(cores.equipment_values(Some(CoreRegion::Abyss)).slash, 120);
        assert_eq!(cores.equipment_values(Some(CoreRegion::Mercurial)), EquipmentValues::default());
        // 地域なし(コアが効かないコンテンツ)は加算しない
        assert_eq!(cores.equipment_values(None), EquipmentValues::default());

        // セット効果は地域ごとに発動して重複する(アビス 進化2の6set +1% + エクリプス 進化4の6set +5%)
        assert!((cores.set_bonus().final_damage_rate - 0.06).abs() < 1e-12);

        // 入場条件はそのコンテンツの地域のコアだけを数える(地域なしは 0)
        assert_eq!(cores.total_bonus(Some(CoreRegion::Abyss)), 120);
        assert_eq!(cores.total_bonus(Some(CoreRegion::Mercurial)), 0);
        assert_eq!(cores.total_bonus(None), 0);
    }

    #[test]
    fn 値域違反は拒否する() {
        let mut cores = ThesisCores::default();
        cores.abyss.slots[0] = core(CoreType::Slash, CORE_EVOLUTION_MAX + 1, 0);
        assert!(matches!(cores.validate(), Err(ThesisCoreError::EvolutionOutOfRange { .. })));

        let mut cores = ThesisCores::default();
        cores.abyss.slots[5] = core(CoreType::Slash, 0, CORE_ENHANCEMENT_MAX + 1);
        assert!(matches!(
            cores.validate(),
            Err(ThesisCoreError::EnhancementOutOfRange { slot: 6, .. })
        ));

        assert!(ThesisCores::default().validate().is_ok());
    }

    #[test]
    fn 補助タイプは補助列の補正値を使い強化能力値には入らない() {
        // 進化3 までは火力と同じ、進化4 の強化1 以降だけ分かれる(wiki 進化強化表)
        let support = |evolution, enhancement| {
            ThesisCore { core_type: CoreType::Accuracy, evolution, enhancement }.bonus()
        };
        assert_eq!(support(3, 4), 35);
        assert_eq!(support(4, 0), 40);
        assert_eq!(support(4, 1), 45);
        assert_eq!(support(4, 4), 60);

        // 補助 4 種はすべて補助列・非火力
        for core_type in [
            CoreType::PhysicalDefense,
            CoreType::Evasion,
            CoreType::Agility,
            CoreType::Accuracy,
        ] {
            assert!(!core_type.is_power());
            assert_eq!(ThesisCore { core_type, evolution: 4, enhancement: 4 }.bonus(), 60);
        }
        for core_type in
            [CoreType::Thrust, CoreType::Slash, CoreType::MagicAttack, CoreType::MagicDefense]
        {
            assert!(core_type.is_power());
            assert_eq!(ThesisCore { core_type, evolution: 4, enhancement: 4 }.bonus(), 80);
        }

        // 補助コアで 6 枠を埋めると命中率補正だけが積まれる(与ダメージ式の係数は 0)
        let set = CoreSet {
            slots: [core(CoreType::Accuracy, 4, 4); CORE_SLOT_COUNT],
        };
        assert_eq!(
            set.equipment_values(),
            EquipmentValues { accuracy: 360, ..Default::default() }
        );
        assert_eq!(set.total_bonus(), 360);

        // タイプ混在: 火力も補助も同じ 9 値に振り分ける
        let mixed = CoreSet {
            slots: [
                core(CoreType::Slash, 4, 4),
                core(CoreType::PhysicalDefense, 4, 4),
                core(CoreType::Evasion, 3, 0),
                core(CoreType::Agility, 2, 2),
                core(CoreType::Accuracy, 0, 0),
                None,
            ],
        };
        assert_eq!(
            mixed.equipment_values(),
            EquipmentValues {
                slash: 80,
                physical_defense: 60,
                evasion: 23,
                agility: 16,
                accuracy: 1,
                ..Default::default()
            }
        );
        assert_eq!(mixed.total_bonus(), 80 + 60 + 23 + 16 + 1);

        // 補助でもセット効果(最終ダメージ)は発動する(wiki はタイプを区別していない)
        assert!((set.set_bonus().final_damage_rate - 0.05).abs() < 1e-12);
    }

    // 固定値のセット効果も地域ごとに重複する(K の上限 1000 はカテゴリ集計側でクランプ)
    #[test]
    fn セット効果は地域をまたいで加算する() {
        let mut cores = ThesisCores::default();
        *cores.get_mut(CoreRegion::Mercurial) = filled(0, 4); // +800
        *cores.get_mut(CoreRegion::Abyss) = filled(1, 4); // +1,400
        *cores.get_mut(CoreRegion::Eclipse) = filled(3, 4); // +2%
        *cores.get_mut(CoreRegion::Rubicona) = filled(4, 4); // +5%

        let bonus = cores.set_bonus();
        assert_eq!(bonus.final_damage_fixed, 800 + 1_400);
        assert!((bonus.final_damage_rate - 0.07).abs() < 1e-12);
    }
}
