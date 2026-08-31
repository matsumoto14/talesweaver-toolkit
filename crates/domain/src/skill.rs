//! スキル。倍率(wiki: カテゴリD)・段数・Cri倍率(カテゴリF)を持つ。

use serde::{Deserialize, Serialize};

use crate::element::Element;

/// スキルの依存種別。ステ由来攻撃力の係数(`AttackCoefficients`)を選ぶキー。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDependency {
    Stab,
    Hack,
    Int,
    Mr,
    StabHack,
    HackInt,
}

impl SkillDependency {
    pub const ALL: [SkillDependency; 6] = [
        SkillDependency::Stab,
        SkillDependency::Hack,
        SkillDependency::Int,
        SkillDependency::Mr,
        SkillDependency::StabHack,
        SkillDependency::HackInt,
    ];
}

/// 対象指定(wiki スキル性能一覧の「対象指定」列)。
///
/// 単体は 1 体、範囲は位置指定・方向指定・自分中心・設置などをまとめたもの。
/// 計算には使わない — **どのスキルを主軸にするかを選ぶときの手がかり**として持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillTarget {
    Single,
    Area,
}

/// 「連」系スキルで選べるコンボスキルタイプ。
/// 3 コンボ以上のダメージボーナス(`DamageInput::combo_count`)とは別の仕組み。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComboSkillType {
    General,
    Instant,
    Chain,
}

/// コンボスキルタイプごとの基礎性能。対応スキルだけがこの一覧を持つ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComboSkillVariant {
    pub combo_type: ComboSkillType,
    pub multiplier: f64,
    pub hit_count: u32,
    pub base_actual_delay: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboSkillTypeError {
    pub skill_id: String,
    pub combo_type: ComboSkillType,
}

impl std::fmt::Display for ComboSkillTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "スキル '{}' はコンボタイプ '{:?}' に対応していません",
            self.skill_id, self.combo_type
        )
    }
}

impl std::error::Error for ComboSkillTypeError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub dependency: SkillDependency,
    /// スキル倍率(wiki: カテゴリD)
    pub multiplier: f64,
    /// 段数
    pub hit_count: u32,
    /// Cri倍率(wiki: カテゴリF)
    pub critical_multiplier: f64,
    /// スキルの属性(wiki: 各キャラのスキルページ「スキル性能一覧」の属性列)
    pub element: Element,
    /// 単体 / 範囲(wiki: 同じ表の対象指定列)。`None` = wiki の行と突き合わせできなかった
    /// (未収録として `?` で出す。0 や「単体」で埋めない)
    #[serde(default)]
    pub target: Option<SkillTarget>,
    /// スキル命中(wiki 計算式まとめ `#AccuracyPoint`: 「当Wikiのスキル命中は実際の数値から
    /// 15 引いた値が記載されているため +15 する必要がある」。**+15 済みの実値**を持つ)。
    /// wiki の表が `-` の行は `None` = 未記載で、命中Pを出せない
    pub accuracy: Option<i64>,
    /// スキルクリティカル率(wiki スキル性能一覧の「Cri値」)。`None` = wiki 未記載
    pub critical_rate: Option<i64>,
    /// スキル Lv(wiki スキル性能一覧の SLv。倍率は Lv 別未対応なのでこの Lv の値を持つ)
    pub level: u8,
    /// 単体チャネリングスキルか(wiki スキル性能一覧の 区分に `続` を含み、対象指定が `単体`。
    /// 凡例は wiki「Skill#f8e303fb」)。極限スキル「フルスロットル」の段数増加はこれにだけ乗る
    #[serde(default)]
    pub single_target_channeling: bool,
    /// 基本中ディレイ(秒)。wiki スキル性能一覧の「動作」列。
    /// 秒数として読めない行(表記が `0` 等)は `None` = 中ディレイ・DPS を出せない
    #[serde(default)]
    pub base_actual_delay: Option<f64>,
    /// 中ディレイが固定で減少が効かない(wiki スキル性能一覧の「(固定)」表記)
    #[serde(default)]
    pub actual_delay_fixed: bool,
    /// 通常攻撃(wiki スキル性能一覧の `†` = 基本攻撃)。コンボで間に挟むのはこれ
    #[serde(default)]
    pub normal_attack: bool,
    /// コンボインターバル(秒)。通常攻撃だけが持つ(wiki 計算式まとめ `#g7881516`)。
    /// 通常攻撃の中ディレイが終わってから数え始め、終わるまで次の行動が撃てないので、
    /// **最速コンボでは「次に使うスキルの中ディレイの下限」**として効く。
    /// `None` = wiki の CI 値表に無い(下限を出せないので、その旨を表示する)
    #[serde(default)]
    pub combo_interval: Option<f64>,
    /// 対応するコンボスキルタイプ。空ならタイプ選択非対応。
    #[serde(default)]
    pub combo_variants: Vec<ComboSkillVariant>,
    /// 1 回ぶんの火力の目安(倍率 × 段数)。主軸スキル候補の並び順に使う(UI 側は再計算しない)
    pub power: f64,
    /// 継続火力の目安(倍率 × 段数 ÷ 基本中ディレイ)。基本中ディレイ不明なら比較不能
    pub power_per_second: Option<f64>,
}

impl Skill {
    /// 1 回ぶんの火力の目安(倍率 × 段数)。0 段のスキルは無いので段数は最低 1 扱い。
    pub fn compute_power(multiplier: f64, hit_count: u32) -> f64 {
        multiplier * f64::from(hit_count.max(1))
    }

    /// 継続火力の目安(倍率 × 段数 ÷ 基本中ディレイ)。基本中ディレイが未収録 / 0 以下なら比較不能
    pub fn compute_power_per_second(power: f64, base_actual_delay: Option<f64>) -> Option<f64> {
        base_actual_delay
            .filter(|&delay| delay > 0.0)
            .map(|delay| power / delay)
    }

    /// 選択したコンボスキルタイプとシエナのオーラから、今回の計算に使う性能を解決する。
    pub fn resolve_combo_variant(
        &self,
        combo_type: ComboSkillType,
        siena_actual_delay_reduction: f64,
    ) -> Result<Self, ComboSkillTypeError> {
        let variant = self
            .combo_variants
            .iter()
            .find(|variant| variant.combo_type == combo_type)
            .ok_or_else(|| ComboSkillTypeError {
                skill_id: self.id.clone(),
                combo_type,
            })?;
        let mut resolved = self.clone();
        resolved.multiplier = variant.multiplier;
        resolved.hit_count = variant.hit_count;
        resolved.base_actual_delay = Some(variant.base_actual_delay);
        if combo_type == ComboSkillType::Chain {
            // wiki 表は 2% 刻み。段数は 6% ごと、倍率は各 6% 区間で +0/+10/+20pt。
            let step = ((siena_actual_delay_reduction.max(0.0) * 100.0 + 1e-9) / 2.0)
                .floor()
                .min(8.0) as u32;
            resolved.hit_count += step / 3;
            resolved.multiplier += f64::from(step % 3) * 0.10;
        }
        resolved.power = Self::compute_power(resolved.multiplier, resolved.hit_count);
        resolved.power_per_second =
            Self::compute_power_per_second(resolved.power, resolved.base_actual_delay);
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo_skill() -> Skill {
        Skill {
            id: "continuous".into(),
            name: "極・連".into(),
            dependency: SkillDependency::Hack,
            multiplier: 5.55,
            hit_count: 11,
            critical_multiplier: 2.5,
            element: Element::Neutral,
            target: Some(SkillTarget::Single),
            accuracy: Some(100),
            critical_rate: Some(5),
            level: 10,
            single_target_channeling: false,
            base_actual_delay: Some(1.4),
            actual_delay_fixed: false,
            normal_attack: false,
            combo_interval: None,
            combo_variants: vec![
                ComboSkillVariant {
                    combo_type: ComboSkillType::General,
                    multiplier: 5.55,
                    hit_count: 11,
                    base_actual_delay: 1.4,
                },
                ComboSkillVariant {
                    combo_type: ComboSkillType::Instant,
                    multiplier: 5.20,
                    hit_count: 10,
                    base_actual_delay: 1.0,
                },
                ComboSkillVariant {
                    combo_type: ComboSkillType::Chain,
                    multiplier: 5.20,
                    hit_count: 12,
                    base_actual_delay: 1.6,
                },
            ],
            power: Skill::compute_power(5.55, 11),
            power_per_second: Skill::compute_power_per_second(
                Skill::compute_power(5.55, 11),
                Some(1.4),
            ),
        }
    }

    #[test]
    fn 連撃はシエナ減少率を2パーセント刻みの下側閾値で解決する() {
        let skill = combo_skill();
        let cases = [
            (0.00, 5.20, 12),
            (0.02, 5.30, 12),
            (0.04, 5.40, 12),
            (0.06, 5.20, 13),
            (0.08, 5.30, 13),
            (0.10, 5.40, 13),
            (0.12, 5.20, 14),
            (0.14, 5.30, 14),
            (0.16, 5.40, 14),
        ];
        for (reduction, multiplier, hit_count) in cases {
            let resolved = skill
                .resolve_combo_variant(ComboSkillType::Chain, reduction)
                .unwrap();
            assert!(
                (resolved.multiplier - multiplier).abs() < 1e-12,
                "{reduction}"
            );
            assert_eq!(resolved.hit_count, hit_count, "{reduction}");
            assert_eq!(resolved.base_actual_delay, Some(1.6));
        }
        let below = skill
            .resolve_combo_variant(ComboSkillType::Chain, 0.039)
            .unwrap();
        assert!((below.multiplier - 5.30).abs() < 1e-12);
    }

    #[test]
    fn 未対応タイプは拒否する() {
        let mut skill = combo_skill();
        skill.combo_variants.clear();
        assert!(skill
            .resolve_combo_variant(ComboSkillType::General, 0.0)
            .is_err());
    }
}
