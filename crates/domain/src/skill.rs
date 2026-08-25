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
    /// スキルの属性(wiki: 各キャラのスキルページ「スキル性能一覧」の属性列)。
    /// `None` = 未取込。`None` のときカテゴリI は 1.0(属性差ボーナスなし)
    pub element: Option<Element>,
    /// スキル命中(wiki 計算式まとめ `#AccuracyPoint`: 「当Wikiのスキル命中は実際の数値から
    /// 15 引いた値が記載されているため +15 する必要がある」。**+15 済みの実値**を持つ)
    pub accuracy: i64,
    /// スキルクリティカル率(wiki スキル性能一覧の「Cri値」。カテゴリではなくクリティカル確率の項)
    pub critical_rate: i64,
    /// スキル Lv(wiki スキル性能一覧の SLv。倍率は Lv 別未対応なのでこの Lv の値を持つ)
    pub level: u8,
}
