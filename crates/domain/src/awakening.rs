//! 覚醒・エタの意志。倍率表(wiki: カテゴリN)は gamedata が持ち、domain は段階とレベルだけを表す。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Awakening {
    /// 覚醒段階 0..=5
    pub stage: u8,
    /// エタの意志レベル 0..=100
    pub eternal_level: u8,
}

impl Awakening {
    pub const MAX_STAGE: u8 = 5;
    /// wiki エタの意志「エタの成長」の表は Lv100(MAX)まで
    pub const MAX_ETERNAL_LEVEL: u8 = 100;
}

/// 覚醒・エタの意志で開放される上限(wiki: Quest/覚醒クエスト「各能力の上限値」/ エタの意志)。
/// 表は gamedata が持ち、domain は形だけを持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwakeningCaps {
    /// 与ダメージの上限。**多段スキルでも 1 段ごとに適用**(wiki: Quest/覚醒クエスト)
    pub max_damage: i64,
    /// 防御力の上限(物理/魔法/複合それぞれに掛かる)
    pub max_defense: i64,
    /// 最終能力値の上限
    pub max_stat: i64,
}
