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
    /// ホームの「次の目標」に据えるコンテンツ(gamedata の `Content::id`)。
    /// `None` はユーザーが決めていない状態で、そのときは画面が自動で選ぶ。
    /// 「クリアできる」と「周回したい」は別なので、自動判定を置き換える例外操作として持つ。
    #[serde(default)]
    pub goal_content_id: Option<String>,
    #[serde(default)]
    pub default_buff_set_id: Option<i64>,
}

/// キャラ 1 件ぶんの検証に要るカタログ一式(保存・プレビュー共通)。
pub struct CharacterCatalogs<'a, C: crate::equipment::EquipmentCatalogEntry> {
    pub equipment: &'a [C],
    pub abilities: &'a [crate::equipment::EquipmentAbilityDef],
    pub random_options: &'a [crate::random_option::RandomOptionDef],
    pub titles: &'a [crate::title::TitleDef],
    pub character_skills: &'a crate::character_skill::CharacterSkillCatalog,
}

impl NewCharacter {
    /// 名前・素ステ・覚醒・補正源・キャラスキル・装備(カタログ照合込み)・共通スキル・称号を
    /// 検証する。commands(保存前のプレビュー)と storage(保存)の両方がこれを呼ぶ。
    /// バフ選択の検証は含まない(バフはキャラに保存しないので呼び出し側が別に見る)
    pub fn validate<C: crate::equipment::EquipmentCatalogEntry>(
        &self,
        catalogs: &CharacterCatalogs<'_, C>,
    ) -> Result<(), crate::validation::ValidationError> {
        use crate::validation::ValidationError;
        let msg = |e: &dyn std::fmt::Display| ValidationError::new(e.to_string());
        if self.name.trim().is_empty() {
            return Err(ValidationError::new("名前が空です"));
        }
        self.base_stats.validate().map_err(|e| msg(&e))?;
        self.awakening.validate().map_err(|e| msg(&e))?;
        self.stat_sources.validate().map_err(|e| msg(&e))?;
        self.stat_sources
            .character_skills
            .validate(catalogs.character_skills, &self.game_character_id)
            .map_err(|e| msg(&e))?;
        self.equipment.validate().map_err(|e| msg(&e))?;
        self.common_skills.validate().map_err(|e| msg(&e))?;
        self.equipment.validate_against_catalog(
            catalogs.equipment,
            catalogs.abilities,
            catalogs.random_options,
        )?;
        // 称号は装備部位ではないので部位ループの外で見る(1 枠・カタログ参照のみ)
        if let Some(id) = &self.equipment.title {
            if !catalogs.titles.iter().any(|t| t.id == id.as_str()) {
                return Err(ValidationError::new(format!("未知の称号 '{id}' です")));
            }
        }
        Ok(())
    }
}
