//! 登録キャラクターのリポジトリ。

use std::path::Path;

use domain::{Awakening, BaseStats, StatKind};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{Result, StorageError};

/// 登録済みキャラクター。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredCharacter {
    pub id: i64,
    pub name: String,
    /// gamedata の `GameCharacter::id`
    pub game_character_id: String,
    pub base_stats: BaseStats,
    pub awakening: Awakening,
}

/// 登録リクエスト。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewCharacter {
    pub name: String,
    pub game_character_id: String,
    pub base_stats: BaseStats,
    pub awakening: Awakening,
}

const MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS characters (
    id                INTEGER PRIMARY KEY,
    name              TEXT    NOT NULL,
    game_character_id TEXT    NOT NULL,
    stab              INTEGER NOT NULL,
    hack              INTEGER NOT NULL,
    int               INTEGER NOT NULL,
    def               INTEGER NOT NULL,
    mr                INTEGER NOT NULL,
    dex               INTEGER NOT NULL,
    agi               INTEGER NOT NULL,
    awakening_stage   INTEGER NOT NULL,
    eternal_level     INTEGER NOT NULL,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
";

const SELECT_COLUMNS: &str = "id, name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level";

pub struct CharacterRepository {
    conn: Connection,
}

impl CharacterRepository {
    /// ファイルを開く(無ければ作成)。マイグレーションを適用する。
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// テスト用のインメモリ DB。
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.execute_batch(MIGRATION)?;
        Ok(Self { conn })
    }

    pub fn create(&self, new: &NewCharacter) -> Result<RegisteredCharacter> {
        validate(new)?;
        let s = &new.base_stats;
        self.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                new.name,
                new.game_character_id,
                s.stab,
                s.hack,
                s.int,
                s.def,
                s.mr,
                s.dex,
                s.agi,
                new.awakening.stage,
                new.awakening.eternal_level,
            ],
        )?;
        self.get(self.conn.last_insert_rowid())
    }

    pub fn list(&self) -> Result<Vec<RegisteredCharacter>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {SELECT_COLUMNS} FROM characters ORDER BY id"))?;
        let rows = stmt.query_map([], row_to_character)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get(&self, id: i64) -> Result<RegisteredCharacter> {
        self.conn
            .query_row(
                &format!("SELECT {SELECT_COLUMNS} FROM characters WHERE id = ?1"),
                [id],
                row_to_character,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StorageError::CharacterNotFound(id),
                other => StorageError::Database(other),
            })
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let affected = self.conn.execute("DELETE FROM characters WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(StorageError::CharacterNotFound(id));
        }
        Ok(())
    }
}

/// 素ステータスの値域(wiki §1: 下限 1、上限 1500、エタの意志で最大 2000)。
const STAT_RANGE: std::ops::RangeInclusive<u32> = 1..=2000;

fn validate(new: &NewCharacter) -> Result<()> {
    if new.name.trim().is_empty() {
        return Err(StorageError::InvalidValue("名前が空です".into()));
    }
    for kind in StatKind::ALL {
        let value = new.base_stats.get(kind);
        if !STAT_RANGE.contains(&value) {
            return Err(StorageError::InvalidValue(format!(
                "{kind:?} は {}〜{} の範囲で指定してください(指定値 {value})",
                STAT_RANGE.start(),
                STAT_RANGE.end()
            )));
        }
    }
    if new.awakening.stage > Awakening::MAX_STAGE {
        return Err(StorageError::InvalidValue(format!(
            "覚醒段階は 0〜{} です",
            Awakening::MAX_STAGE
        )));
    }
    if new.awakening.eternal_level > Awakening::MAX_ETERNAL_LEVEL {
        return Err(StorageError::InvalidValue(format!(
            "エタの意志 Lv は 0〜{} です",
            Awakening::MAX_ETERNAL_LEVEL
        )));
    }
    Ok(())
}

fn row_to_character(row: &Row<'_>) -> rusqlite::Result<RegisteredCharacter> {
    Ok(RegisteredCharacter {
        id: row.get("id")?,
        name: row.get("name")?,
        game_character_id: row.get("game_character_id")?,
        base_stats: BaseStats {
            stab: row.get("stab")?,
            hack: row.get("hack")?,
            int: row.get("int")?,
            def: row.get("def")?,
            mr: row.get("mr")?,
            dex: row.get("dex")?,
            agi: row.get("agi")?,
        },
        awakening: Awakening {
            stage: row.get("awakening_stage")?,
            eternal_level: row.get("eternal_level")?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_character(name: &str) -> NewCharacter {
        NewCharacter {
            name: name.to_string(),
            game_character_id: "boris".to_string(),
            base_stats: BaseStats { stab: 500, hack: 600, int: 10, def: 200, mr: 150, dex: 300, agi: 250 },
            awakening: Awakening { stage: 5, eternal_level: 40 },
        }
    }

    #[test]
    fn 登録して一覧と取得ができる() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        assert!(repo.list().unwrap().is_empty());

        let created = repo.create(&new_character("メイン")).unwrap();
        assert_eq!(created.name, "メイン");
        assert_eq!(created.base_stats.hack, 600);
        assert_eq!(created.awakening, Awakening { stage: 5, eternal_level: 40 });

        let second = repo.create(&new_character("サブ")).unwrap();
        assert_ne!(created.id, second.id);

        let list = repo.list().unwrap();
        assert_eq!(list, vec![created.clone(), second]);
        assert_eq!(repo.get(created.id).unwrap(), created);
    }

    #[test]
    fn 削除できる_存在しないidはエラー() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン")).unwrap();
        repo.delete(created.id).unwrap();
        assert!(repo.list().unwrap().is_empty());
        assert!(matches!(repo.delete(created.id), Err(StorageError::CharacterNotFound(_))));
        assert!(matches!(repo.get(created.id), Err(StorageError::CharacterNotFound(_))));
    }

    #[test]
    fn 不正な値は拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("  ");
        assert!(matches!(repo.create(&c), Err(StorageError::InvalidValue(_))));
        c.name = "x".into();
        c.awakening.stage = 6;
        assert!(matches!(repo.create(&c), Err(StorageError::InvalidValue(_))));
        c.awakening.stage = 5;
        c.awakening.eternal_level = 81;
        assert!(matches!(repo.create(&c), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn 素ステは1から2000の範囲() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.base_stats.int = 0;
        assert!(matches!(repo.create(&c), Err(StorageError::InvalidValue(_))));
        c.base_stats.int = 2001;
        assert!(matches!(repo.create(&c), Err(StorageError::InvalidValue(_))));
        c.base_stats.int = 1;
        c.base_stats.agi = 2000;
        assert!(repo.create(&c).is_ok());
    }

    #[test]
    fn マイグレーションは再適用しても壊れない() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        repo.conn.execute_batch(MIGRATION).unwrap();
        repo.create(&new_character("a")).unwrap();
        assert_eq!(repo.list().unwrap().len(), 1);
    }
}
