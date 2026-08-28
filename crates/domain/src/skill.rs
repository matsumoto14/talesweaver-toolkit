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
}
