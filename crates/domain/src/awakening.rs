//! 覚醒・エタの意志。倍率表(wiki: カテゴリN)は gamedata が持ち、domain は段階とレベルだけを表す。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Awakening {
    /// 覚醒段階 0..=5
    pub stage: u8,
    /// エタの意志レベル 0..=80
    pub eternal_level: u8,
}

impl Awakening {
    pub const MAX_STAGE: u8 = 5;
    pub const MAX_ETERNAL_LEVEL: u8 = 80;
}
