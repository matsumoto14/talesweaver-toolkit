//! スキル。倍率(wiki: カテゴリD)・段数・Cri倍率(カテゴリF)を持つ。

use serde::{Deserialize, Serialize};

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
    pub fn label(self) -> &'static str {
        match self {
            SkillDependency::Stab => "STAB",
            SkillDependency::Hack => "HACK",
            SkillDependency::Int => "INT",
            SkillDependency::Mr => "MR",
            SkillDependency::StabHack => "STAB+HACK",
            SkillDependency::HackInt => "HACK+INT",
        }
    }
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
}
