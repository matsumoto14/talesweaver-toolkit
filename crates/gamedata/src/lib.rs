//! 静的ゲームデータ(wiki / 旧リポ由来)。アプリに同梱し、ユーザーは編集しない。
//!
//! 現状は Rust のリテラルで持つ(docs/claude/decisions.md「構成・運用」)。各データに出典 `Source` を付ける。

pub mod awakening;
pub mod buffs;
pub mod characters;
pub mod contents;
pub mod elements;
pub mod enemies;
pub mod equipment_catalog;
pub mod skills;

use serde::Serialize;

pub use awakening::{awakening_caps, awakening_rate, AWAKENING_SOURCE};
pub use buffs::{buff_catalog, BUFF_CATALOG_SOURCE};
pub use characters::{
    attack_coefficients, characters, equipment_coefficients, find_character, GameCharacter,
};
pub use contents::{content_areas, core_region_of, CONTENTS_SOURCE};
pub use elements::{element_base, ELEMENT_BASE_SOURCE};
pub use enemies::{enemies, find_enemy};
pub use equipment_catalog::{
    enhance_multiplier, enhance_multiplier_range, enhance_rates, equipment_abilities,
    equipment_catalog, find_equipment_item, EquipmentItem, WeaponClass, EQUIPMENT_ABILITY_SOURCE,
    EQUIPMENT_CATALOG_SOURCE, ENHANCE_SOURCE,
};
pub use skills::{find_skill, skills_for};

/// データの出典。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Source {
    /// 出典ページ(wiki ページ名・旧リポのファイル名など)
    pub page: &'static str,
    /// 取得日(YYYY-MM-DD)
    pub retrieved_on: &'static str,
    /// 備考(裏取り状況など)
    pub note: &'static str,
}

/// 旧リポ twtoolkit(Excel ダメージ計算器 v4.00 由来)からの転記。wiki での裏取り前。
pub const LEGACY_TWTOOLKIT_RETRIEVED_ON: &str = "2026-08-21";
