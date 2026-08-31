//! 登録キャラクターの入力データ。
//!
//! 保存先(SQLite)を知らない純粋なドメイン型なので domain に置く。
//! ブラウザ(WASM)側も同じ形でキャラを組み立てて計算に渡せるようにするため、
//! rusqlite に依存する `storage` からは切り離してある。

use serde::{Deserialize, Serialize};

use crate::awakening::Awakening;
use crate::common_skill::CommonSkills;
use crate::equipment::Equipment;
use crate::stat_sources::StatSources;
use crate::stats::BaseStats;

/// 登録リクエスト。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewCharacter {
    pub name: String,
    /// gamedata の `GameCharacter::id`
    pub game_character_id: String,
    pub base_stats: BaseStats,
    pub awakening: Awakening,
    pub stat_sources: StatSources,
    pub equipment: Equipment,
    /// 共通スキル(wiki: Skill/共通)
    #[serde(default)]
    pub common_skills: CommonSkills,
    pub main_skill_id: Option<String>,
    #[serde(default)]
    pub default_buff_set_id: Option<i64>,
}
