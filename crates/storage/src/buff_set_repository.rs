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
