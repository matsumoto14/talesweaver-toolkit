//! 覚醒・エタの意志。倍率表(wiki: カテゴリN)は gamedata が持ち、domain は段階とレベルだけを表す。

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    /// エタの意志は覚醒 5 の先にあるもの(wiki: エタの意志)。Lv > 0 なら覚醒段階はこれで確定
    pub const ETERNAL_STAGE: u8 = 5;

    pub fn validate(&self) -> Result<(), AwakeningError> {
        if self.stage > Self::MAX_STAGE {
            return Err(AwakeningError::StageOutOfRange {
                stage: self.stage,
                max: Self::MAX_STAGE,
            });
        }
        if self.eternal_level > Self::MAX_ETERNAL_LEVEL {
            return Err(AwakeningError::EternalLevelOutOfRange {
                level: self.eternal_level,
                max: Self::MAX_ETERNAL_LEVEL,
            });
        }
        if self.eternal_level > 0 && self.stage != Self::ETERNAL_STAGE {
            return Err(AwakeningError::EternalRequiresStage {
                stage: self.stage,
                required: Self::ETERNAL_STAGE,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AwakeningError {
    #[error("覚醒段階は 0〜{max} です(指定値 {stage})")]
    StageOutOfRange { stage: u8, max: u8 },
    #[error("エタの意志 Lv は 0〜{max} です(指定値 {level})")]
    EternalLevelOutOfRange { level: u8, max: u8 },
    #[error("エタの意志 Lv > 0 なら覚醒段階は {required} です(指定値 {stage})")]
    EternalRequiresStage { stage: u8, required: u8 },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn エタの意志があるなら覚醒は5で確定する() {
        assert!(Awakening { stage: 5, eternal_level: 3 }.validate().is_ok());
        assert!(Awakening { stage: 4, eternal_level: 0 }.validate().is_ok());
        assert_eq!(
            Awakening { stage: 4, eternal_level: 1 }.validate(),
            Err(AwakeningError::EternalRequiresStage { stage: 4, required: 5 })
        );
        assert!(Awakening { stage: 6, eternal_level: 0 }.validate().is_err());
        assert!(Awakening { stage: 5, eternal_level: 101 }.validate().is_err());
    }
}
