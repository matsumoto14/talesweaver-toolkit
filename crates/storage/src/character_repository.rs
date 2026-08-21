//! 登録キャラクターのリポジトリ。

use std::path::Path;

use domain::{Awakening, BaseStats, BuffCatalog, StatSources};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{Result, StorageError};

/// 登録済みキャラクター。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisteredCharacter {
    pub id: i64,
    pub name: String,
    /// gamedata の `GameCharacter::id`
    pub game_character_id: String,
    pub base_stats: BaseStats,
    pub awakening: Awakening,
    /// ペット/ルーン/クラウン/聖物/バフ/調整値(docs/goals/2026-08-21-character-stat-sources.md)
    pub stat_sources: StatSources,
}

/// 登録リクエスト。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewCharacter {
    pub name: String,
    pub game_character_id: String,
    pub base_stats: BaseStats,
    pub awakening: Awakening,
    pub stat_sources: StatSources,
}

/// v1 相当(`stat_sources` 列を含まない、main ブランチ時代の実スキーマ)。
/// v2 への移行は `from_connection` が `PRAGMA user_version` を見て `ALTER TABLE` で行う。
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

/// このスキーマバージョンで `stat_sources` 列が存在する(v2)。
const SCHEMA_VERSION: i64 = 2;

const SELECT_COLUMNS: &str =
    "id, name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources";

/// `stat_sources` 列(JSON テキスト)を `StatSources` として読み出すための橋渡し。
struct StatSourcesColumn(StatSources);

impl FromSql for StatSourcesColumn {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        serde_json::from_str(text)
            .map(StatSourcesColumn)
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

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

        // `PRAGMA user_version` だけで「列が無い」と判定しない: このブランチ以前の実スキーマは
        // `stat_sources` 列を `CREATE TABLE` に直接持っていた(`ALTER TABLE` で足したのではない)ため、
        // 一度でも起動した DB は列を持ちながら `user_version` が未設定(0)のままになっている。
        // その状態で `user_version < SCHEMA_VERSION` だけを見て `ALTER TABLE` すると
        // `duplicate column name: stat_sources` で起動不能になる。列の実在を直接確認する。
        let has_stat_sources = {
            let mut stmt = conn.prepare("PRAGMA table_info(characters)")?;
            let mut rows = stmt.query([])?;
            let mut found = false;
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                if name == "stat_sources" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_stat_sources {
            conn.execute_batch(
                "ALTER TABLE characters ADD COLUMN stat_sources TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

        Ok(Self { conn })
    }

    pub fn create(&self, new: &NewCharacter, catalog: &BuffCatalog) -> Result<RegisteredCharacter> {
        validate(new, catalog)?;
        let s = &new.base_stats;
        let stat_sources_json = serde_json::to_string(&new.stat_sources)?;
        self.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                stat_sources_json,
            ],
        )?;
        self.get(self.conn.last_insert_rowid())
    }

    /// 既存キャラクターの内容を丸ごと置き換える。存在しない id は `CharacterNotFound`。
    pub fn update(&self, id: i64, update: &NewCharacter, catalog: &BuffCatalog) -> Result<RegisteredCharacter> {
        validate(update, catalog)?;
        let s = &update.base_stats;
        let stat_sources_json = serde_json::to_string(&update.stat_sources)?;
        let affected = self.conn.execute(
            "UPDATE characters SET
                name = ?1, game_character_id = ?2,
                stab = ?3, hack = ?4, int = ?5, def = ?6, mr = ?7, dex = ?8, agi = ?9,
                awakening_stage = ?10, eternal_level = ?11, stat_sources = ?12
             WHERE id = ?13",
            params![
                update.name,
                update.game_character_id,
                s.stab,
                s.hack,
                s.int,
                s.def,
                s.mr,
                s.dex,
                s.agi,
                update.awakening.stage,
                update.awakening.eternal_level,
                stat_sources_json,
                id,
            ],
        )?;
        if affected == 0 {
            return Err(StorageError::CharacterNotFound(id));
        }
        self.get(id)
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

fn validate(new: &NewCharacter, catalog: &BuffCatalog) -> Result<()> {
    if new.name.trim().is_empty() {
        return Err(StorageError::InvalidValue("名前が空です".into()));
    }
    new.base_stats.validate().map_err(|e| StorageError::InvalidValue(e.to_string()))?;
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
    new.stat_sources.validate().map_err(|e| StorageError::InvalidValue(e.to_string()))?;
    domain::stat_sources::build_modifiers(&new.stat_sources, catalog, &new.game_character_id)
        .map_err(|e| StorageError::InvalidValue(e.to_string()))?;
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
        stat_sources: row.get::<_, StatSourcesColumn>("stat_sources")?.0,
    })
}

#[cfg(test)]
mod tests {
    use domain::{
        BuffChoice, BuffDefinition, BuffGroup, BuffSelection, BuffTarget, BuffValue, Crown,
        PetSkillTier, PetSkills, RuneLevels, SacredRelic, StatLayer, StatSources,
    };

    use super::*;

    fn new_character(name: &str) -> NewCharacter {
        NewCharacter {
            name: name.to_string(),
            game_character_id: "boris".to_string(),
            base_stats: BaseStats { stab: 300, hack: 250, int: 10, def: 200, mr: 150, dex: 280, agi: 250 },
            awakening: Awakening { stage: 5, eternal_level: 40 },
            stat_sources: StatSources::default(),
        }
    }

    /// storage は domain の型を使うだけで gamedata には依存しない。
    /// `stat_sourcesはjsonで往復する()` で使う trust_potion と、排他枠テスト用の
    /// 2 件だけを持つ最小カタログ(値は gamedata::buffs::buff_catalog() と一致させること)。
    fn test_catalog() -> Vec<BuffDefinition> {
        vec![
            BuffDefinition {
                id: "trust_potion",
                name: "改・信頼の薬",
                target: BuffTarget::AllStats,
                layer: StatLayer::Fixed,
                value: BuffValue::UserInput { min: 0.0, max: 33.0 },
                exclusive_slots: vec!["trust_potion"],
                source_url: "",
                note: "",
                default_value: Some(33.0),
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "illumination_drink",
                name: "イルミネーション祭りのドリンク",
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.30),
                exclusive_slots: vec!["percent_slot_1", "percent_slot_2"],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
            BuffDefinition {
                id: "charge_potion",
                name: "充填の秘薬",
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.20),
                exclusive_slots: vec!["percent_slot_1"],
                source_url: "",
                note: "",
                default_value: None,
                group: BuffGroup::Consumable,
            },
        ]
    }

    fn buff_choice(id: &str) -> BuffChoice {
        BuffChoice { buff_id: id.to_string(), stat: None, choice_index: None, value: None }
    }

    #[test]
    fn 登録して一覧と取得ができる() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        assert!(repo.list().unwrap().is_empty());

        let created = repo.create(&new_character("メイン"), &[]).unwrap();
        assert_eq!(created.name, "メイン");
        assert_eq!(created.base_stats.hack, 250);
        assert_eq!(created.awakening, Awakening { stage: 5, eternal_level: 40 });

        let second = repo.create(&new_character("サブ"), &[]).unwrap();
        assert_ne!(created.id, second.id);

        let list = repo.list().unwrap();
        assert_eq!(list, vec![created.clone(), second]);
        assert_eq!(repo.get(created.id).unwrap(), created);
    }

    #[test]
    fn 削除できる_存在しないidはエラー() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[]).unwrap();
        repo.delete(created.id).unwrap();
        assert!(repo.list().unwrap().is_empty());
        assert!(matches!(repo.delete(created.id), Err(StorageError::CharacterNotFound(_))));
        assert!(matches!(repo.get(created.id), Err(StorageError::CharacterNotFound(_))));
    }

    #[test]
    fn 不正な値は拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("  ");
        assert!(matches!(repo.create(&c, &[]), Err(StorageError::InvalidValue(_))));
        c.name = "x".into();
        c.awakening.stage = 6;
        assert!(matches!(repo.create(&c, &[]), Err(StorageError::InvalidValue(_))));
        c.awakening.stage = 5;
        c.awakening.eternal_level = 81;
        assert!(matches!(repo.create(&c, &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn 素ステは1から310の範囲() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.base_stats.int = 0;
        assert!(matches!(repo.create(&c, &[]), Err(StorageError::InvalidValue(_))));
        c.base_stats.int = 311;
        assert!(matches!(repo.create(&c, &[]), Err(StorageError::InvalidValue(_))));
        c.base_stats.int = 1;
        c.base_stats.agi = 310;
        assert!(repo.create(&c, &[]).is_ok());
    }

    #[test]
    fn マイグレーションは再適用しても壊れない() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        repo.conn.execute_batch(MIGRATION).unwrap();
        repo.create(&new_character("a"), &[]).unwrap();
        assert_eq!(repo.list().unwrap().len(), 1);
    }

    /// `stat_sources` 列の無い旧スキーマ(v1)から開いた場合、`from_connection` が
    /// 自動で `ALTER TABLE` して `stat_sources` を `StatSources::default()` として読めるようにする。
    #[test]
    fn 旧スキーマからでも自動マイグレーションしてstat_sourcesが中立値で読める() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE characters (
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
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level)
             VALUES ('旧データ', 'boris', 300, 250, 10, 200, 150, 280, 250, 5, 40)",
            [],
        )
        .unwrap();

        let repo = CharacterRepository::from_connection(conn).unwrap();

        let list = repo.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].stat_sources, StatSources::default());

        let fetched = repo.get(list[0].id).unwrap();
        assert_eq!(fetched.stat_sources, StatSources::default());

        // マイグレーション後も create/update が使えること
        let created = repo.create(&new_character("新データ"), &[]).unwrap();
        assert_eq!(created.stat_sources, StatSources::default());

        let mut updated = new_character("旧データ改");
        updated.stat_sources.rune_levels.stab = 10;
        let result = repo.update(list[0].id, &updated, &[]).unwrap();
        assert_eq!(result.stat_sources.rune_levels.stab, 10);
    }

    /// 実際に起きたバグの再現テスト: このブランチ以前のスキーマは `stat_sources` 列を
    /// `CREATE TABLE` に直接持っていた(`ALTER TABLE` で足したのではない)ため、
    /// 一度でも起動した DB は「列は既にあるが `user_version` は未設定(0)」という状態になる。
    /// この状態を `PRAGMA user_version` だけで判定すると `ALTER TABLE` が
    /// `duplicate column name` で失敗し、リポジトリを開けなくなる。
    #[test]
    fn 列は既にあるがuser_version未設定のdbも開ける() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE characters (
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
                stat_sources      TEXT    NOT NULL,
                created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            ",
        )
        .unwrap();
        // user_version はこの時点で未設定(0)のまま。
        assert_eq!(conn.query_row::<i64, _, _>("PRAGMA user_version", [], |r| r.get(0)).unwrap(), 0);

        conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources)
             VALUES ('既存データ', 'boris', 300, 250, 10, 200, 150, 280, 250, 5, 40, '{}')",
            [],
        )
        .unwrap();

        // 列が既にあるので from_connection は ALTER TABLE を試みてはいけない(試みれば
        // duplicate column name で Err になる)。
        let repo = CharacterRepository::from_connection(conn).unwrap();
        let list = repo.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "既存データ");
        assert_eq!(list[0].stat_sources, StatSources::default());

        // create/update も引き続き使えること。
        repo.create(&new_character("追加データ"), &[]).unwrap();
        assert_eq!(repo.list().unwrap().len(), 2);
    }

    #[test]
    fn stat_sourcesはjsonで往復する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("メイン");
        c.stat_sources = StatSources {
            pet_skills: PetSkills { stab: Some(PetSkillTier::TrueLv4), ..Default::default() },
            rune_levels: RuneLevels { hack: 20, ..Default::default() },
            sacred_relic: SacredRelic { int: 40, ..Default::default() },
            buffs: BuffSelection {
                choices: vec![BuffChoice {
                    buff_id: "trust_potion".to_string(),
                    stat: None,
                    choice_index: None,
                    value: Some(33.0),
                }],
            },
            ..Default::default()
        };
        let created = repo.create(&c, &test_catalog()).unwrap();
        assert_eq!(created.stat_sources, c.stat_sources);

        let fetched = repo.get(created.id).unwrap();
        assert_eq!(fetched.stat_sources, c.stat_sources);

        let listed = repo.list().unwrap();
        assert_eq!(listed[0].stat_sources, c.stat_sources);
    }

    #[test]
    fn updateで登録内容を更新できる() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[]).unwrap();

        let mut updated = new_character("メイン改");
        updated.base_stats.stab = 310;
        updated.awakening.eternal_level = 60;
        updated.stat_sources.rune_levels.stab = 20;

        let result = repo.update(created.id, &updated, &[]).unwrap();
        assert_eq!(result.id, created.id);
        assert_eq!(result.name, "メイン改");
        assert_eq!(result.base_stats.stab, 310);
        assert_eq!(result.awakening.eternal_level, 60);
        assert_eq!(result.stat_sources.rune_levels.stab, 20);

        assert_eq!(repo.get(created.id).unwrap(), result);
    }

    #[test]
    fn updateは存在しないidをエラーにする() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let c = new_character("メイン");
        assert!(matches!(repo.update(999, &c, &[]), Err(StorageError::CharacterNotFound(999))));
    }

    #[test]
    fn updateも310の範囲バリデーションが効く() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[]).unwrap();
        let mut invalid = new_character("メイン");
        invalid.base_stats.stab = 311;
        assert!(matches!(repo.update(created.id, &invalid, &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn createはクラウン_ルーン_聖物の範囲超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();

        let mut over_crown = new_character("x");
        over_crown.stat_sources.crown = Crown { stab: Crown::MAX_VALUE + 1, ..Default::default() };
        assert!(matches!(repo.create(&over_crown, &[]), Err(StorageError::InvalidValue(_))));

        let mut over_rune = new_character("x");
        over_rune.stat_sources.rune_levels =
            RuneLevels { hack: RuneLevels::MAX_LEVEL + 1, ..Default::default() };
        assert!(matches!(repo.create(&over_rune, &[]), Err(StorageError::InvalidValue(_))));

        let mut over_relic = new_character("x");
        over_relic.stat_sources.sacred_relic =
            SacredRelic { int: SacredRelic::MAX_STAGE + 1, ..Default::default() };
        assert!(matches!(repo.create(&over_relic, &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn updateはクラウン_ルーン_聖物の範囲超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[]).unwrap();

        let mut invalid = new_character("メイン");
        invalid.stat_sources.crown = Crown { stab: Crown::MAX_VALUE + 1, ..Default::default() };
        assert!(matches!(repo.update(created.id, &invalid, &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn createは排他枠が重複するバフ選択を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.stat_sources.buffs = BuffSelection {
            choices: vec![buff_choice("illumination_drink"), buff_choice("charge_potion")],
        };
        assert!(matches!(repo.create(&c, &test_catalog()), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn updateは排他枠が重複するバフ選択を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &test_catalog()).unwrap();

        let mut invalid = new_character("メイン");
        invalid.stat_sources.buffs = BuffSelection {
            choices: vec![buff_choice("illumination_drink"), buff_choice("charge_potion")],
        };
        assert!(matches!(
            repo.update(created.id, &invalid, &test_catalog()),
            Err(StorageError::InvalidValue(_))
        ));
    }
}
