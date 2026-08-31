//! 前回起動時のダメージ計算記録。ホームの影響カード(前回起動からの目安火力差分)用に
//! キャラ 1 件につき最新値のみを保持する(履歴は持たない)。

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{CharacterRepository, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageSnapshot {
    pub character_id: i64,
    pub skill_id: String,
    pub content_id: String,
    pub per_hit: i64,
    pub taken_at: String,
}

fn row_to_snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<DamageSnapshot> {
    Ok(DamageSnapshot {
        character_id: row.get(0)?,
        skill_id: row.get(1)?,
        content_id: row.get(2)?,
        per_hit: row.get(3)?,
        taken_at: row.get(4)?,
    })
}

impl CharacterRepository {
    pub fn get_damage_snapshot(&self, character_id: i64) -> Result<Option<DamageSnapshot>> {
        self.conn
            .query_row(
                "SELECT character_id, skill_id, content_id, per_hit, taken_at FROM damage_snapshots WHERE character_id = ?1",
                [character_id],
                row_to_snapshot,
            )
            .optional()
            .map_err(Into::into)
    }

    /// 1 キャラ 1 行の upsert。既存の記録は上書きし、`taken_at` はその都度更新する。
    pub fn set_damage_snapshot(
        &self,
        character_id: i64,
        skill_id: &str,
        content_id: &str,
        per_hit: i64,
    ) -> Result<DamageSnapshot> {
        self.conn.execute(
            "INSERT INTO damage_snapshots (character_id, skill_id, content_id, per_hit, taken_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(character_id) DO UPDATE SET
                skill_id = excluded.skill_id,
                content_id = excluded.content_id,
                per_hit = excluded.per_hit,
                taken_at = excluded.taken_at",
            params![character_id, skill_id, content_id, per_hit],
        )?;
        Ok(self
            .get_damage_snapshot(character_id)?
            .expect("直前に upsert した行が見つからない"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::NewCharacter;

    fn new_character(name: &str) -> NewCharacter {
        NewCharacter {
            name: name.to_string(),
            game_character_id: "boris".to_string(),
            base_stats: domain::BaseStats {
                stab: 300,
                hack: 250,
                int: 10,
                def: 200,
                mr: 150,
                dex: 280,
                agi: 250,
            },
            awakening: domain::Awakening {
                stage: 5,
                eternal_level: 40,
            },
            stat_sources: domain::StatSources::default(),
            equipment: domain::Equipment::default(),
            common_skills: domain::CommonSkills::default(),
            main_skill_id: None,
            goal_content_id: None,
            default_buff_set_id: None,
        }
    }

    #[test]
    fn 未記録のキャラはnoneを返す() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let character = repo
            .create(&new_character("a"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(repo.get_damage_snapshot(character.id).unwrap(), None);
    }

    #[test]
    fn set_damage_snapshotはupsertで1キャラ1行を保つ() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let character = repo
            .create(&new_character("a"), &[], &[], &[], &[], &[], &[])
            .unwrap();

        repo.set_damage_snapshot(character.id, "skill_1", "content_1", 1000)
            .unwrap();
        let updated = repo
            .set_damage_snapshot(character.id, "skill_2", "content_2", 2000)
            .unwrap();
        assert_eq!(updated.skill_id, "skill_2");
        assert_eq!(updated.content_id, "content_2");
        assert_eq!(updated.per_hit, 2000);

        let count: i64 = repo
            .conn
            .query_row("SELECT count(*) FROM damage_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn キャラ削除でスナップショットもカスケード削除される() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let character = repo
            .create(&new_character("a"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        repo.set_damage_snapshot(character.id, "skill_1", "content_1", 1000)
            .unwrap();

        repo.delete(character.id).unwrap();

        let count: i64 = repo
            .conn
            .query_row("SELECT count(*) FROM damage_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
