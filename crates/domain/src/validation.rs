//! 検証エラーの共通形。文言だけでなく「どこの話か」を一緒に運ぶ。
//!
//! 帯に文字列を出すだけでは、どのキャラのどの部位で起きたのか分からない。
//! `location` を付けておくと、フロントはエラー帯から該当部位の詳細まで直接飛べる。

use serde::{Deserialize, Serialize};

use crate::equipment::PartSlot;

/// 検証エラー 1 件。`location` が付いていれば、そこへ飛べる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub message: String,
    pub location: Option<ValidationLocation>,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
        }
    }

    pub fn at(message: impl Into<String>, location: ValidationLocation) -> Self {
        Self {
            message: message.into(),
            location: Some(location),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ValidationError {}

impl From<String> for ValidationError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for ValidationError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// 装備のどこで起きたか。部位 → 登録(`EquipmentPart::id`)→ アビリティ / ランダムオプション。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationLocation {
    pub slot: PartSlot,
    pub part_id: u64,
    pub ability_id: Option<String>,
    pub random_option_id: Option<String>,
}

impl ValidationLocation {
    pub fn part(slot: PartSlot, part_id: u64) -> Self {
        Self {
            slot,
            part_id,
            ability_id: None,
            random_option_id: None,
        }
    }

    pub fn ability(slot: PartSlot, part_id: u64, ability_id: impl Into<String>) -> Self {
        Self {
            ability_id: Some(ability_id.into()),
            ..Self::part(slot, part_id)
        }
    }

    pub fn random_option(slot: PartSlot, part_id: u64, option_id: impl Into<String>) -> Self {
        Self {
            random_option_id: Some(option_id.into()),
            ..Self::part(slot, part_id)
        }
    }
}
