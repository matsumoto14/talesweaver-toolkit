//! 登録キャラクターのリポジトリ。

use std::collections::HashSet;
use std::path::Path;

use domain::{Awakening, BaseStats, BuffCatalog, Equipment, EquipmentAbilityDef, StatSources};
use gamedata::EquipmentItem;
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
    /// ペット/ルーン/クラウン/聖物/バフ/調整値(docs/claude/goals/2026-08-21-character-stat-sources.md)
    pub stat_sources: StatSources,
    /// 装備補正(基本能力値/強化能力値/装備攻撃力強化バフ)(docs/claude/goals/2026-08-22-equipment-attack.md)
    pub equipment: Equipment,
    /// 主軸スキル(gamedata の `Skill::id`)。攻撃力(A)の依存種別を決める。
    /// スキル未収録のキャラがあるので未選択(`None`)を許す
    pub main_skill_id: Option<String>,
}

/// 登録リクエスト。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewCharacter {
    pub name: String,
    pub game_character_id: String,
    pub base_stats: BaseStats,
    pub awakening: Awakening,
    pub stat_sources: StatSources,
    pub equipment: Equipment,
    pub main_skill_id: Option<String>,
}

/// v1 相当(`stat_sources`/`equipment` 列を含まない、main ブランチ時代の実スキーマ)。
/// v2/v3 への移行は `from_connection` が列の実在を確認して `ALTER TABLE` で行う。
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

/// v4 で `equipment` 列が部位別装備(12 スロット)の JSON になり、v5 で `main_skill_id` 列
/// (攻撃力の依存種別を決める主軸スキル)が加わった。
/// v3 以前は `equipment` 列に「基本能力値/強化能力値の合計 8 値」を持っていた
/// (docs/claude/goals/2026-08-24-equipment-parts.md 決定6)。
const SCHEMA_VERSION: i64 = 5;

const SELECT_COLUMNS: &str = "id, name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, main_skill_id";

/// `stat_sources`/`equipment` 列(JSON テキスト)を domain の型として読み出すための橋渡し。
struct StatSourcesColumn(StatSources);

impl FromSql for StatSourcesColumn {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        serde_json::from_str(text)
            .map(StatSourcesColumn)
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

struct EquipmentColumn(Equipment);

impl FromSql for EquipmentColumn {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        serde_json::from_str(text)
            .map(EquipmentColumn)
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

/// v3 以前(旧形式)の `equipment` 列を部位別装備(v4)へ移行する。
///
/// 旧形式は JSON に `parts` キーを持たない。旧形式からは部位を再構成できないため、
/// 基本能力値/強化能力値の合計 8 値は破棄し、`power_weapon`/`strong_weapon_level` のみ引き継ぐ
/// (docs/claude/goals/2026-08-24-equipment-parts.md 決定6)。新形式(`parts` あり)の行は変更しない。
fn migrate_equipment_to_parts(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, equipment FROM characters")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (id, json) in rows {
        let value: serde_json::Value = serde_json::from_str(&json)?;
        if value.get("parts").is_some() {
            continue;
        }
        let power_weapon = value.get("power_weapon").and_then(|v| v.as_bool()).unwrap_or(false);
        let strong_weapon_level =
            value.get("strong_weapon_level").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        let migrated =
            Equipment { power_weapon, strong_weapon_level, ..Equipment::default() };
        let migrated_json = serde_json::to_string(&migrated)?;
        conn.execute("UPDATE characters SET equipment = ?1 WHERE id = ?2", params![migrated_json, id])?;
    }
    Ok(())
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
        // `duplicate column name` で起動不能になる。列の実在を直接確認する。
        let existing_columns: HashSet<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(characters)")?;
            let mut rows = stmt.query([])?;
            let mut columns = HashSet::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                columns.insert(name);
            }
            columns
        };
        if !existing_columns.contains("stat_sources") {
            conn.execute_batch(
                "ALTER TABLE characters ADD COLUMN stat_sources TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        if !existing_columns.contains("equipment") {
            conn.execute_batch(
                "ALTER TABLE characters ADD COLUMN equipment TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        // v5: 主軸スキル。既存キャラは未選択(NULL)で読める。
        if !existing_columns.contains("main_skill_id") {
            conn.execute_batch("ALTER TABLE characters ADD COLUMN main_skill_id TEXT;")?;
        }
        migrate_equipment_to_parts(&conn)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

        Ok(Self { conn })
    }

    pub fn create(
        &self,
        new: &NewCharacter,
        catalog: &BuffCatalog,
        equipment_catalog: &[EquipmentItem],
        equipment_abilities: &[EquipmentAbilityDef],
    ) -> Result<RegisteredCharacter> {
        validate(new, catalog, equipment_catalog, equipment_abilities)?;
        let s = &new.base_stats;
        let stat_sources_json = serde_json::to_string(&new.stat_sources)?;
        let equipment_json = serde_json::to_string(&new.equipment)?;
        self.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, main_skill_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                equipment_json,
                new.main_skill_id,
            ],
        )?;
        self.get(self.conn.last_insert_rowid())
    }

    /// 既存キャラクターの内容を丸ごと置き換える。存在しない id は `CharacterNotFound`。
    pub fn update(
        &self,
        id: i64,
        update: &NewCharacter,
        catalog: &BuffCatalog,
        equipment_catalog: &[EquipmentItem],
        equipment_abilities: &[EquipmentAbilityDef],
    ) -> Result<RegisteredCharacter> {
        validate(update, catalog, equipment_catalog, equipment_abilities)?;
        let s = &update.base_stats;
        let stat_sources_json = serde_json::to_string(&update.stat_sources)?;
        let equipment_json = serde_json::to_string(&update.equipment)?;
        let affected = self.conn.execute(
            "UPDATE characters SET
                name = ?1, game_character_id = ?2,
                stab = ?3, hack = ?4, int = ?5, def = ?6, mr = ?7, dex = ?8, agi = ?9,
                awakening_stage = ?10, eternal_level = ?11, stat_sources = ?12, equipment = ?13,
                main_skill_id = ?14
             WHERE id = ?15",
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
                equipment_json,
                update.main_skill_id,
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

/// 登録リクエストの検証(値域・バフ整合性・装備カタログ整合性)。保存前プレビュー(preview_damage 等)からも使う。
pub fn validate(
    new: &NewCharacter,
    catalog: &BuffCatalog,
    equipment_catalog: &[EquipmentItem],
    equipment_abilities: &[EquipmentAbilityDef],
) -> Result<()> {
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
    new.equipment.validate().map_err(|e| StorageError::InvalidValue(e.to_string()))?;
    validate_equipment_catalog(&new.equipment, equipment_catalog, equipment_abilities)?;
    Ok(())
}

/// 装備のカタログ整合性を検証する(未知の item_id/ability id・部位不一致・エンチャント上限超過)。
/// `custom`(`item_id` が `None`)のエンチャントは `EQUIPMENT_VALUE_MAX` まで許可する
/// (`Equipment::validate` の値域チェックで既に検証済み。ここではカタログ item のときだけ
/// より厳しい `enchant_caps` を追加でチェックする)。
fn validate_equipment_catalog(
    equipment: &Equipment,
    equipment_catalog: &[EquipmentItem],
    equipment_abilities: &[EquipmentAbilityDef],
) -> Result<()> {
    for (slot, part) in equipment.parts.iter() {
        if let Some(item_id) = &part.item_id {
            let item = equipment_catalog
                .iter()
                .find(|i| i.id == item_id.as_str())
                .ok_or_else(|| StorageError::InvalidValue(format!("未知の装備アイテム '{item_id}' です")))?;
            if item.slot != slot {
                return Err(StorageError::InvalidValue(format!(
                    "装備アイテム '{item_id}' は {:?} 用ですが {:?} 部位に指定されています",
                    item.slot, slot
                )));
            }
            let caps = item.enchant_caps;
            let over = part.enchant.thrust > caps.thrust
                || part.enchant.slash > caps.slash
                || part.enchant.magic_attack > caps.magic_attack
                || part.enchant.magic_defense > caps.magic_defense;
            if over {
                return Err(StorageError::InvalidValue(format!(
                    "装備アイテム '{item_id}' のエンチャントが上限を超えています(上限 突{} 斬{} 魔攻{} 魔防{})",
                    caps.thrust, caps.slash, caps.magic_attack, caps.magic_defense
                )));
            }
        }
        for ability_id in &part.abilities {
            if !equipment_abilities.iter().any(|a| a.id == ability_id.as_str()) {
                return Err(StorageError::InvalidValue(format!("未知の装備アビリティ '{ability_id}' です")));
            }
        }
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
        stat_sources: row.get::<_, StatSourcesColumn>("stat_sources")?.0,
        equipment: row.get::<_, EquipmentColumn>("equipment")?.0,
        main_skill_id: row.get("main_skill_id")?,
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
            equipment: Equipment::default(),
            main_skill_id: None,
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

        let created = repo.create(&new_character("メイン"), &[], &[], &[]).unwrap();
        assert_eq!(created.name, "メイン");
        assert_eq!(created.base_stats.hack, 250);
        assert_eq!(created.awakening, Awakening { stage: 5, eternal_level: 40 });

        let second = repo.create(&new_character("サブ"), &[], &[], &[]).unwrap();
        assert_ne!(created.id, second.id);

        let list = repo.list().unwrap();
        assert_eq!(list, vec![created.clone(), second]);
        assert_eq!(repo.get(created.id).unwrap(), created);
    }

    #[test]
    fn 削除できる_存在しないidはエラー() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[], &[], &[]).unwrap();
        repo.delete(created.id).unwrap();
        assert!(repo.list().unwrap().is_empty());
        assert!(matches!(repo.delete(created.id), Err(StorageError::CharacterNotFound(_))));
        assert!(matches!(repo.get(created.id), Err(StorageError::CharacterNotFound(_))));
    }

    #[test]
    fn 不正な値は拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("  ");
        assert!(matches!(repo.create(&c, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
        c.name = "x".into();
        c.awakening.stage = 6;
        assert!(matches!(repo.create(&c, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
        c.awakening.stage = 5;
        c.awakening.eternal_level = 81;
        assert!(matches!(repo.create(&c, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn 素ステは1から310の範囲() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.base_stats.int = 0;
        assert!(matches!(repo.create(&c, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
        c.base_stats.int = 311;
        assert!(matches!(repo.create(&c, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
        c.base_stats.int = 1;
        c.base_stats.agi = 310;
        assert!(repo.create(&c, &[], &[], &[]).is_ok());
    }

    #[test]
    fn マイグレーションは再適用しても壊れない() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        repo.conn.execute_batch(MIGRATION).unwrap();
        repo.create(&new_character("a"), &[], &[], &[]).unwrap();
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
        assert_eq!(list[0].equipment, Equipment::default());

        let fetched = repo.get(list[0].id).unwrap();
        assert_eq!(fetched.stat_sources, StatSources::default());
        assert_eq!(fetched.equipment, Equipment::default());

        // マイグレーション後も create/update が使えること
        let created = repo.create(&new_character("新データ"), &[], &[], &[]).unwrap();
        assert_eq!(created.stat_sources, StatSources::default());
        assert_eq!(created.equipment, Equipment::default());

        let mut updated = new_character("旧データ改");
        updated.stat_sources.rune_levels.stab = 10;
        let result = repo.update(list[0].id, &updated, &[], &[], &[]).unwrap();
        assert_eq!(result.stat_sources.rune_levels.stab, 10);
    }

    /// v3(旧形式)の `equipment` 列(基本能力値/強化能力値の合計 8 値)を実際に持つ DB を開くと、
    /// `power_weapon`/`strong_weapon_level` だけを引き継いだ v4(部位別)形式に書き換わり、
    /// 旧合計値(base/enhanced)は破棄される(docs/claude/goals/2026-08-24-equipment-parts.md 決定6)。
    #[test]
    fn v3の旧equipment列は基本強化の合計値を破棄しpw_swだけ引き継いでv4へ移行する() {
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
                equipment         TEXT    NOT NULL,
                created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            ",
        )
        .unwrap();
        let old_equipment_json = r#"{"base":{"thrust":400,"slash":400,"magic_attack":0,"magic_defense":0},"enhanced":{"thrust":200,"slash":200,"magic_attack":0,"magic_defense":0},"power_weapon":true,"strong_weapon_level":6}"#;
        conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment)
             VALUES ('v3データ', 'boris', 300, 250, 10, 200, 150, 280, 250, 5, 40, '{}', ?1)",
            params![old_equipment_json],
        )
        .unwrap();

        let repo = CharacterRepository::from_connection(conn).unwrap();

        let list = repo.list().unwrap();
        assert_eq!(list.len(), 1);
        // power_weapon/strong_weapon_level は引き継がれる。
        assert_eq!(list[0].equipment.power_weapon, true);
        assert_eq!(list[0].equipment.strong_weapon_level, 6);
        // 旧合計値(base/enhanced相当)は破棄され、部位はすべて default(未装備)になる。
        assert_eq!(list[0].equipment.parts, domain::EquipmentParts::default());

        // DB 上の JSON 自体も書き換わっている(再度開いても同じ結果)ことを確認する。
        let raw: String = repo
            .conn
            .query_row("SELECT equipment FROM characters WHERE id = ?1", [list[0].id], |r| r.get(0))
            .unwrap();
        assert!(raw.contains("\"parts\""));
        // 旧形式は強化能力値を "enhanced" キーで持っていたが、新形式は部位ごとの "enchant" になる。
        assert!(!raw.contains("\"enhanced\""));
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

        // stat_sources 列は既にあるので from_connection はこの列に ALTER TABLE を試みてはいけない
        // (試みれば duplicate column name で Err になる)。equipment 列は無いので追加される。
        let repo = CharacterRepository::from_connection(conn).unwrap();
        let list = repo.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "既存データ");
        assert_eq!(list[0].stat_sources, StatSources::default());
        assert_eq!(list[0].equipment, Equipment::default());

        // create/update も引き続き使えること。
        repo.create(&new_character("追加データ"), &[], &[], &[]).unwrap();
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
        let created = repo.create(&c, &test_catalog(), &[], &[]).unwrap();
        assert_eq!(created.stat_sources, c.stat_sources);

        let fetched = repo.get(created.id).unwrap();
        assert_eq!(fetched.stat_sources, c.stat_sources);

        let listed = repo.list().unwrap();
        assert_eq!(listed[0].stat_sources, c.stat_sources);
    }

    #[test]
    fn equipmentはjsonで往復する() {
        use domain::{EquipmentParts, EquipmentPart, EquipmentValues};

        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("メイン");
        c.equipment = Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart {
                    base: EquipmentValues { thrust: 150, slash: 150, magic_attack: 0, magic_defense: 0 },
                    enchant: EquipmentValues { thrust: 60, slash: 60, magic_attack: 0, magic_defense: 0 },
                    ..Default::default()
                },
                ..Default::default()
            },
            power_weapon: true,
            strong_weapon_level: 6,
            ..Default::default()
        };
        let created = repo.create(&c, &[], &[], &[]).unwrap();
        assert_eq!(created.equipment, c.equipment);

        let fetched = repo.get(created.id).unwrap();
        assert_eq!(fetched.equipment, c.equipment);

        let listed = repo.list().unwrap();
        assert_eq!(listed[0].equipment, c.equipment);
    }

    #[test]
    fn main_skill_idは往復し未選択はnullで読める() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("メイン");
        c.main_skill_id = Some("boris_goku_zaneizan".to_string());
        let created = repo.create(&c, &[], &[], &[]).unwrap();
        assert_eq!(created.main_skill_id.as_deref(), Some("boris_goku_zaneizan"));
        assert_eq!(repo.get(created.id).unwrap().main_skill_id.as_deref(), Some("boris_goku_zaneizan"));

        // 未選択(None)も保存できる。update で解除できること。
        let mut cleared = c.clone();
        cleared.main_skill_id = None;
        let updated = repo.update(created.id, &cleared, &[], &[], &[]).unwrap();
        assert_eq!(updated.main_skill_id, None);
    }

    /// v4 の DB(`main_skill_id` 列が無い)を開いても落ちず、既存キャラは主軸スキル未選択で読める。
    #[test]
    fn main_skill_id列の無いdbを開くと既存キャラは未選択で読める() {
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
                equipment         TEXT    NOT NULL,
                created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment)
            VALUES ('v4データ', 'boris', 300, 250, 10, 200, 150, 280, 250, 5, 40, '{}', '{\"parts\":{}}');
            ",
        )
        .unwrap();

        let repo = CharacterRepository::from_connection(conn).unwrap();
        let list = repo.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "v4データ");
        assert_eq!(list[0].main_skill_id, None);

        // 移行後も create/update が使えること。
        repo.create(&new_character("追加データ"), &[], &[], &[]).unwrap();
        assert_eq!(repo.list().unwrap().len(), 2);
    }

    #[test]
    fn 装備の値域違反は拒否する() {
        use domain::EquipmentValues;

        let repo = CharacterRepository::open_in_memory().unwrap();

        let mut over_value = new_character("x");
        over_value.equipment.parts.weapon.base = EquipmentValues { thrust: 10000, ..Default::default() };
        assert!(matches!(repo.create(&over_value, &[], &[], &[]), Err(StorageError::InvalidValue(_))));

        let mut over_level = new_character("x");
        over_level.equipment.strong_weapon_level = 7;
        assert!(matches!(repo.create(&over_level, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
    }

    fn test_equipment_item() -> EquipmentItem {
        EquipmentItem {
            id: "test-weapon",
            slot: domain::PartSlot::Weapon,
            name: "テスト武器",
            values_min: domain::EquipmentValues::default(),
            values_max: domain::EquipmentValues { thrust: 100, slash: 100, magic_attack: 0, magic_defense: 0 },
            enchant_caps: domain::EquipmentValues { thrust: 50, slash: 50, magic_attack: 0, magic_defense: 0 },
            weapon_class: None,
            source: gamedata::EQUIPMENT_CATALOG_SOURCE,
        }
    }

    fn test_equipment_ability() -> EquipmentAbilityDef {
        EquipmentAbilityDef { id: "test-ability", name: "テストアビリティ", values: domain::EquipmentValues::default() }
    }

    #[test]
    fn 未知の装備アイテムidは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.item_id = Some("nope".to_string());
        assert!(matches!(
            repo.create(&c, &[], &[test_equipment_item()], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn 部位とカタログのslotが不一致なら拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        // test_equipment_item は Weapon 用だが helm 部位に指定する。
        c.equipment.parts.helm.item_id = Some("test-weapon".to_string());
        assert!(matches!(
            repo.create(&c, &[], &[test_equipment_item()], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn エンチャントがカタログ上限を超えたら拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.item_id = Some("test-weapon".to_string());
        c.equipment.parts.weapon.enchant = domain::EquipmentValues { thrust: 51, ..Default::default() };
        assert!(matches!(
            repo.create(&c, &[], &[test_equipment_item()], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut ok = new_character("x");
        ok.equipment.parts.weapon.item_id = Some("test-weapon".to_string());
        ok.equipment.parts.weapon.enchant = domain::EquipmentValues { thrust: 50, ..Default::default() };
        assert!(repo.create(&ok, &[], &[test_equipment_item()], &[]).is_ok());
    }

    #[test]
    fn 未知の装備アビリティidは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.abilities = vec!["nope".to_string()];
        assert!(matches!(repo.create(&c, &[], &[], &[test_equipment_ability()]), Err(StorageError::InvalidValue(_))));

        let mut ok = new_character("x");
        ok.equipment.parts.weapon.abilities = vec!["test-ability".to_string()];
        assert!(repo.create(&ok, &[], &[], &[test_equipment_ability()]).is_ok());
    }

    #[test]
    fn updateで登録内容を更新できる() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[], &[], &[]).unwrap();

        let mut updated = new_character("メイン改");
        updated.base_stats.stab = 310;
        updated.awakening.eternal_level = 60;
        updated.stat_sources.rune_levels.stab = 20;

        let result = repo.update(created.id, &updated, &[], &[], &[]).unwrap();
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
        assert!(matches!(repo.update(999, &c, &[], &[], &[]), Err(StorageError::CharacterNotFound(999))));
    }

    #[test]
    fn updateも310の範囲バリデーションが効く() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[], &[], &[]).unwrap();
        let mut invalid = new_character("メイン");
        invalid.base_stats.stab = 311;
        assert!(matches!(repo.update(created.id, &invalid, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn createはクラウン_ルーン_聖物の範囲超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();

        let mut over_crown = new_character("x");
        over_crown.stat_sources.crown = Crown { stab: Crown::MAX_VALUE + 1, ..Default::default() };
        assert!(matches!(repo.create(&over_crown, &[], &[], &[]), Err(StorageError::InvalidValue(_))));

        let mut over_rune = new_character("x");
        over_rune.stat_sources.rune_levels =
            RuneLevels { hack: RuneLevels::MAX_LEVEL + 1, ..Default::default() };
        assert!(matches!(repo.create(&over_rune, &[], &[], &[]), Err(StorageError::InvalidValue(_))));

        let mut over_relic = new_character("x");
        over_relic.stat_sources.sacred_relic =
            SacredRelic { int: SacredRelic::MAX_STAGE + 1, ..Default::default() };
        assert!(matches!(repo.create(&over_relic, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn updateはクラウン_ルーン_聖物の範囲超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &[], &[], &[]).unwrap();

        let mut invalid = new_character("メイン");
        invalid.stat_sources.crown = Crown { stab: Crown::MAX_VALUE + 1, ..Default::default() };
        assert!(matches!(repo.update(created.id, &invalid, &[], &[], &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn createは排他枠が重複するバフ選択を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.stat_sources.buffs = BuffSelection {
            choices: vec![buff_choice("illumination_drink"), buff_choice("charge_potion")],
        };
        assert!(matches!(repo.create(&c, &test_catalog(), &[], &[]), Err(StorageError::InvalidValue(_))));
    }

    #[test]
    fn updateは排他枠が重複するバフ選択を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo.create(&new_character("メイン"), &test_catalog(), &[], &[]).unwrap();

        let mut invalid = new_character("メイン");
        invalid.stat_sources.buffs = BuffSelection {
            choices: vec![buff_choice("illumination_drink"), buff_choice("charge_potion")],
        };
        assert!(matches!(
            repo.update(created.id, &invalid, &test_catalog(), &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }
}
