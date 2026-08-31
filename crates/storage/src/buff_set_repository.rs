//! ユーザー定義の常用バフセット。カタログ自体は gamedata に置き、ここでは選択だけを保存する。

use domain::{BuffCatalog, BuffSelection, StatSources};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{CharacterRepository, Result, StorageError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuffSet {
    pub id: i64,
    pub name: String,
    pub choices: BuffSelection,
}

impl CharacterRepository {
    pub fn list_buff_sets(&self) -> Result<Vec<BuffSet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, choices FROM buff_sets ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            let json: String = row.get(2)?;
            let choices = serde_json::from_str(&json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(BuffSet {
                id: row.get(0)?,
                name: row.get(1)?,
                choices,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_buff_set(&self, id: i64) -> Result<BuffSet> {
        self.conn
            .query_row(
                "SELECT id, name, choices FROM buff_sets WHERE id = ?1",
                [id],
                |row| {
                    let json: String = row.get(2)?;
                    let choices = serde_json::from_str(&json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(BuffSet {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        choices,
                    })
                },
            )
            .optional()?
            .ok_or(StorageError::BuffSetNotFound(id))
    }

    pub fn create_buff_set(
        &self,
        name: &str,
        choices: &BuffSelection,
        catalog: &BuffCatalog,
    ) -> Result<BuffSet> {
        validate_buff_set(name, choices, catalog)?;
        self.conn.execute(
            "INSERT INTO buff_sets (name, choices) VALUES (?1, ?2)",
            params![name.trim(), serde_json::to_string(choices)?],
        )?;
        self.get_buff_set(self.conn.last_insert_rowid())
    }

    pub fn update_buff_set(
        &self,
        id: i64,
        name: &str,
        choices: &BuffSelection,
        catalog: &BuffCatalog,
    ) -> Result<BuffSet> {
        validate_buff_set(name, choices, catalog)?;
        let affected = self.conn.execute(
            "UPDATE buff_sets SET name = ?1, choices = ?2 WHERE id = ?3",
            params![name.trim(), serde_json::to_string(choices)?, id],
        )?;
        if affected == 0 {
            return Err(StorageError::BuffSetNotFound(id));
        }
        self.get_buff_set(id)
    }

    pub fn duplicate_buff_set(&self, id: i64) -> Result<BuffSet> {
        let source = self.get_buff_set(id)?;
        self.conn.execute(
            "INSERT INTO buff_sets (name, choices) VALUES (?1, ?2)",
            params![
                format!("{}のコピー", source.name),
                serde_json::to_string(&source.choices)?
            ],
        )?;
        self.get_buff_set(self.conn.last_insert_rowid())
    }

    pub fn delete_buff_set(&self, id: i64) -> Result<()> {
        let affected = self
            .conn
            .execute("DELETE FROM buff_sets WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(StorageError::BuffSetNotFound(id));
        }
        Ok(())
    }

    pub fn set_default_buff_set(&self, character_id: i64, buff_set_id: Option<i64>) -> Result<()> {
        if let Some(id) = buff_set_id {
            self.get_buff_set(id)?;
        }
        let affected = self.conn.execute(
            "UPDATE characters SET default_buff_set_id = ?1 WHERE id = ?2",
            params![buff_set_id, character_id],
        )?;
        if affected == 0 {
            return Err(StorageError::CharacterNotFound(character_id));
        }
        Ok(())
    }
}

fn validate_buff_set(name: &str, choices: &BuffSelection, catalog: &BuffCatalog) -> Result<()> {
    if name.trim().is_empty() {
        return Err(StorageError::InvalidValue("バフセット名が空です".into()));
    }
    domain::stat_sources::build_modifiers(&StatSources::default(), choices, catalog)
        .map_err(|e| StorageError::InvalidValue(e.to_string().into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::StatKind;

    /// クラブSエフェクト(ステータス+20)は「1 ステだけ」の形で保存されてきた。
    /// 複数ステを選べる形に変えても保存形(`stat` 付きの 1 件)は同じなので、
    /// 既存のバフセットはそのまま同じ +20 として読める(移行は要らない)。
    /// ここが崩れるとユーザーのバフセットが黙って弱くなるため、実データのカタログで確かめる。
    #[test]
    fn クラブsエフェクトの既存バフセットは複数選択化後も同じ効果() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        repo.conn
            .execute(
                "INSERT INTO buff_sets (name, choices) VALUES ('移行前', ?1)",
                [r#"{"choices":[{"buff_id":"club_s_effect_single_stat","stat":"stab","choice_index":null,"value":null}]}"#],
            )
            .unwrap();

        let set = repo.list_buff_sets().unwrap().pop().unwrap();
        let catalog = gamedata::buff_catalog();
        let (modifiers, _) =
            domain::stat_sources::build_modifiers(&StatSources::default(), &set.choices, &catalog)
                .unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 20);
        assert_eq!(modifiers.get(StatKind::Hack).fixed, 0);

        // 新しい形では 2 ステ目を足せる(wiki はステ別に別アイテム / ユーザー実測 2026-09-01)
        let mut choices = set.choices.clone();
        choices.choices.push(domain::BuffChoice {
            buff_id: "club_s_effect_single_stat".to_string(),
            stat: Some(StatKind::Hack),
            choice_index: None,
            value: None,
        });
        let saved = repo
            .update_buff_set(set.id, &set.name, &choices, &catalog)
            .unwrap();
        let (modifiers, _) =
            domain::stat_sources::build_modifiers(&StatSources::default(), &saved.choices, &catalog)
                .unwrap();
        assert_eq!(modifiers.get(StatKind::Stab).fixed, 20);
        assert_eq!(modifiers.get(StatKind::Hack).fixed, 20);
    }
}
