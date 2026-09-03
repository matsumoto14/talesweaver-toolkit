//! 登録キャラクターのリポジトリ。

use std::collections::HashSet;
use std::path::Path;

use domain::{
    Awakening, BaseStats, BuffCatalog, CharacterSkillCatalog, CommonSkills, EnhanceGrade,
    Equipment, EquipmentAbilityDef, EquipmentValues, NewCharacter, RandomOptionDef, StatSources,
    TitleDef,
};
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
    /// 共通スキル(wiki: Skill/共通)。装備攻撃力強化倍率・装備防御力倍率・割合追加ダメージ
    #[serde(default)]
    pub common_skills: CommonSkills,
    /// 主軸スキル(gamedata の `Skill::id`)。攻撃力(A)の依存種別を決める。
    /// スキル未収録のキャラがあるので未選択(`None`)を許す
    pub main_skill_id: Option<String>,
    /// ホームの「次の目標」に据えるコンテンツ(gamedata の `Content::id`)。
    /// 未設定(`None`)なら画面が自動で選ぶ。
    pub goal_content_id: Option<String>,
    /// このキャラで計算時に最初に選ぶバフセット。
    pub default_buff_set_id: Option<i64>,
    /// 最終保存日時(ISO8601 UTC)。v11 未満で作られた既存行は NULL(表示しない)。
    pub updated_at: Option<String>,
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
/// v6 で `common_skills` 列が加わった。パワーウェポン / ストロングウェポンは
/// v5 まで `equipment` 列の中にあり、移行で `common_skills` へ移す。
/// v11 で `characters.updated_at`(最終保存日時)と `damage_snapshots` テーブルが加わり、
/// v12 で登録キャラごとの表示画像 `character_icons` が加わった。
/// (ホームの影響カード用。docs/claude/goals 参照)。
/// v13 で `goal_content_id`(ホームの「次の目標」をユーザーが選んだときの保存先)が加わった。
const SCHEMA_VERSION: i64 = 13;

const SELECT_COLUMNS: &str = "id, name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills, main_skill_id, goal_content_id, default_buff_set_id, updated_at";

/// v9: キャラ JSON に埋め込まれていた常用バフを独立したセットへ移す。
/// 1キャラずつ作り、同じ内容でも統合しない。全処理を単一 transaction にする。
fn migrate_buff_sets(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS buff_sets (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            choices TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;
    let columns: HashSet<String> = {
        let mut stmt = tx.prepare("PRAGMA table_info(characters)")?;
        let values = stmt
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        values
    };
    if !columns.contains("default_buff_set_id") {
        tx.execute_batch(
            "ALTER TABLE characters ADD COLUMN default_buff_set_id INTEGER REFERENCES buff_sets(id) ON DELETE SET NULL;"
        )?;
    }
    let rows: Vec<(i64, String, String, Option<i64>)> = {
        let mut stmt =
            tx.prepare("SELECT id, name, stat_sources, default_buff_set_id FROM characters")?;
        let values = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        values
    };
    for (id, name, json, default_id) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let choices = value.get("buffs").and_then(|v| v.get("choices")).cloned();
        let Some(map) = value.as_object_mut() else {
            continue;
        };
        let had_buffs = map.remove("buffs").is_some();
        if default_id.is_none()
            && choices
                .as_ref()
                .and_then(|v| v.as_array())
                .is_some_and(|v| !v.is_empty())
        {
            let selection = serde_json::json!({ "choices": choices.unwrap() });
            tx.execute(
                "INSERT INTO buff_sets (name, choices) VALUES (?1, ?2)",
                params![
                    format!("{}の常用バフ", name),
                    serde_json::to_string(&selection)?
                ],
            )?;
            tx.execute(
                "UPDATE characters SET default_buff_set_id = ?1 WHERE id = ?2",
                params![tx.last_insert_rowid(), id],
            )?;
        }
        if had_buffs {
            tx.execute(
                "UPDATE characters SET stat_sources = ?1 WHERE id = ?2",
                params![serde_json::to_string(&value)?, id],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// v10: バフではなく共通スキルで設定するアンリーシュを独立済みのセットから除き、
/// 固定 +7 から 1〜7 入力へ変わったクラブ効果には従来値の 7 を補う。
fn migrate_unleash_from_buff_sets(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, choices FROM buff_sets")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (id, json) in rows {
        let mut selection: serde_json::Value = serde_json::from_str(&json)?;
        let Some(choices) = selection.get_mut("choices").and_then(|v| v.as_array_mut()) else {
            continue;
        };
        let before = choices.len();
        choices.retain(|choice| choice.get("buff_id").and_then(|v| v.as_str()) != Some("unleash"));
        let mut changed = choices.len() != before;
        for choice in choices.iter_mut() {
            if choice.get("buff_id").and_then(|v| v.as_str()) == Some("club_effect")
                && choice.get("value").is_none_or(|value| value.is_null())
            {
                choice["value"] = serde_json::json!(7.0);
                changed = true;
            }
        }
        if changed {
            conn.execute(
                "UPDATE buff_sets SET choices = ?1 WHERE id = ?2",
                params![serde_json::to_string(&selection)?, id],
            )?;
        }
    }
    Ok(())
}

/// v11: 最終保存日時(`characters.updated_at`)と前回ダメージ計算の記録(`damage_snapshots`)を追加する。
/// 既存行の `updated_at` は NULL のまま(フロントは NULL なら表示しない)。
fn migrate_damage_snapshots(conn: &Connection) -> Result<()> {
    let columns: HashSet<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(characters)")?;
        let values = stmt
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        values
    };
    if !columns.contains("updated_at") {
        conn.execute_batch("ALTER TABLE characters ADD COLUMN updated_at TEXT;")?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS damage_snapshots (
            character_id INTEGER PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
            skill_id     TEXT    NOT NULL,
            content_id   TEXT    NOT NULL,
            per_hit      INTEGER NOT NULL,
            taken_at     TEXT    NOT NULL
        );",
    )?;
    Ok(())
}

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

struct CommonSkillsColumn(CommonSkills);

impl FromSql for CommonSkillsColumn {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        serde_json::from_str(text)
            .map(CommonSkillsColumn)
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
        // 旧形式から部位は再構成できない。パワーウェポン / ストロングウェポンは
        // `migrate_weapon_skills_to_common` が `equipment` 列の JSON から拾うので、
        // ここでは触らず中立値の部位別装備に差し替えるだけにする。
        let migrated = Equipment::default();
        let mut migrated_value = serde_json::to_value(migrated)?;
        for key in ["power_weapon", "strong_weapon_level"] {
            if let Some(v) = value.get(key) {
                migrated_value[key] = v.clone();
            }
        }
        let migrated_json = serde_json::to_string(&migrated_value)?;
        conn.execute(
            "UPDATE characters SET equipment = ?1 WHERE id = ?2",
            params![migrated_json, id],
        )?;
    }
    Ok(())
}

/// v7: レリックを**ペンダント**と**ブレスレット**の 2 部位に分ける
/// (wiki: Item/アクセサリ/レリック。ペンダントは突き/斬り/魔攻/命中/Cri、
/// ブレスレットは物防/魔防/回避/敏捷)。
///
/// 旧 `parts.relic` に入っていた値は**ペンダント**へ移す(火力側の入力がそこに入っている)。
/// ブレスレットは中立値で始まる。
fn migrate_relic_to_pendant(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, equipment FROM characters")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (id, json) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let Some(parts) = value.get_mut("parts").and_then(|p| p.as_object_mut()) else {
            continue;
        };
        let Some(relic) = parts.remove("relic") else {
            continue;
        };
        parts.insert("relic_pendant".to_string(), relic);
        let migrated = serde_json::to_string(&value)?;
        conn.execute(
            "UPDATE characters SET equipment = ?1 WHERE id = ?2",
            params![migrated, id],
        )?;
    }
    Ok(())
}

/// v8: 各部位1件の装備を、登録一覧と選択IDへ一度だけ包む。
fn migrate_equipment_to_registered_lists(conn: &Connection) -> Result<()> {
    fn classify_old_added_damage(
        slot: &str,
        map: &serde_json::Map<String, serde_json::Value>,
        actual: i64,
        level: u8,
    ) -> EnhanceGrade {
        let values = |key: &str| -> EquipmentValues {
            map.get(key)
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default()
        };
        let values = values("base").add(values("enchant"));
        let kind = map
            .get("item_id")
            .and_then(|v| v.as_str())
            .and_then(gamedata::equipment_enhance_type);
        let candidate = |grade| {
            if slot == "weapon" {
                let rates = kind.and_then(gamedata::enhance_rates_for_type)?;
                let multiplier = gamedata::enhance_grade_multiplier(level, grade)?;
                Some(domain::weapon_added_damage(&values, &rates, multiplier))
            } else if slot == "armor" {
                let class = kind.and_then(gamedata::armor_class_for_type)?;
                let multiplier = gamedata::armor_enhance_multiplier(level, Some(grade))?;
                let rates = gamedata::armor_enhance_rates(class);
                Some(domain::armor_added_hp(
                    &values,
                    rates.physical_defense,
                    rates.magic_defense,
                    multiplier,
                ))
            } else {
                None
            }
        };
        EnhanceGrade::closest_to(actual, candidate)
    }
    fn values_are_zero(value: Option<&serde_json::Value>) -> bool {
        value
            .and_then(serde_json::Value::as_object)
            .is_none_or(|values| {
                values
                    .values()
                    .all(|value| value.as_i64().unwrap_or(0) == 0)
            })
    }
    fn array_is_empty(value: Option<&serde_json::Value>) -> bool {
        value
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty)
    }
    fn old_part_is_empty(part: &serde_json::Value) -> bool {
        let Some(map) = part.as_object() else {
            return true;
        };
        map.get("item_id").is_none_or(serde_json::Value::is_null)
            && map
                .get("custom_name")
                .is_none_or(|value| value.is_null() || value.as_str().is_some_and(str::is_empty))
            && values_are_zero(map.get("base"))
            && values_are_zero(map.get("enchant"))
            && map
                .get("enhance_level")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            && array_is_empty(map.get("abilities"))
            && array_is_empty(map.get("random_options"))
            && map.get("siena").is_none_or(|siena| {
                array_is_empty(siena.get("slots")) && array_is_empty(siena.get("extras"))
            })
    }

    let mut stmt = conn.prepare("SELECT id, equipment FROM characters")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (character_id, json) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let Some(parts) = value.get_mut("parts").and_then(|p| p.as_object_mut()) else {
            continue;
        };
        let mut changed = false;
        for (slot, part) in parts.iter_mut() {
            if part.get("registered").is_some() {
                continue;
            }
            let mut old = part.take();
            if old_part_is_empty(&old) {
                *part = serde_json::json!({ "registered": [], "selected_id": null });
                changed = true;
                continue;
            }
            if let Some(map) = old.as_object_mut() {
                map.insert("id".into(), serde_json::json!(1));
                map.insert("label".into(), serde_json::json!(""));
                let level = map
                    .get("enhance_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let old_added_damage = map.get("enhance_added_damage").and_then(|v| v.as_i64());
                let grade = old_added_damage
                    .map(|actual| classify_old_added_damage(slot, map, actual, level as u8))
                    .unwrap_or(EnhanceGrade::Highest);
                map.remove("enhance_added_damage");
                map.insert(
                    "enhance_grade".into(),
                    if level >= 12 {
                        serde_json::to_value(grade)?
                    } else {
                        serde_json::Value::Null
                    },
                );
                if let Some(kind) = map
                    .get("item_id")
                    .and_then(|v| v.as_str())
                    .and_then(gamedata::equipment_enhance_type)
                {
                    map.insert("enhance_type".into(), serde_json::to_value(kind)?);
                }
                map.remove("element");
                map.remove("element_value");
            }
            *part = serde_json::json!({ "registered": [old], "selected_id": 1 });
            changed = true;
        }
        if changed {
            conn.execute(
                "UPDATE characters SET equipment = ?1 WHERE id = ?2",
                params![serde_json::to_string(&value)?, character_id],
            )?;
        }
    }
    Ok(())
}

/// 現行カタログから消えた装備アビリティを除去し、補正式メタデータを補完する。
/// カタログ更新後も装備以外の編集・保存を妨げないため、起動時に冪等に実行する。
fn migrate_equipment_registration_metadata(conn: &Connection) -> Result<()> {
    let ability_defs = gamedata::equipment_abilities();
    let valid_abilities: HashSet<&str> = ability_defs.iter().map(|a| a.id).collect();
    let mut stmt = conn.prepare("SELECT id, equipment FROM characters")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (character_id, json) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let Some(parts) = value.get_mut("parts").and_then(|p| p.as_object_mut()) else {
            continue;
        };
        let mut changed = false;
        for list in parts.values_mut() {
            let Some(registered) = list.get_mut("registered").and_then(|v| v.as_array_mut()) else {
                continue;
            };
            for part in registered {
                let Some(map) = part.as_object_mut() else {
                    continue;
                };
                if let Some(abilities) = map.get_mut("abilities").and_then(|v| v.as_array_mut()) {
                    let before = abilities.len();
                    abilities
                        .retain(|id| id.as_str().is_some_and(|id| valid_abilities.contains(id)));
                    // 旧実装は「効果系統」を排他単位にしていたが、正しくはカテゴリー単位。
                    // 新定義で同一カテゴリーになった保存値は先頭を残し、武器の3枠までに正規化する。
                    let mut categories = HashSet::new();
                    abilities.retain(|id| {
                        id.as_str()
                            .and_then(|id| ability_defs.iter().find(|def| def.id == id))
                            .is_some_and(|def| categories.insert(def.category))
                    });
                    abilities.truncate(domain::WEAPON_ABILITY_SLOTS);
                    changed |= abilities.len() != before;
                }
                if map
                    .get("enhance_type")
                    .is_none_or(serde_json::Value::is_null)
                {
                    if let Some(kind) = map
                        .get("item_id")
                        .and_then(|v| v.as_str())
                        .and_then(gamedata::equipment_enhance_type)
                    {
                        map.insert("enhance_type".into(), serde_json::to_value(kind)?);
                        changed = true;
                    }
                }
                let level = map
                    .get("enhance_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if level >= 12
                    && map
                        .get("enhance_grade")
                        .is_none_or(serde_json::Value::is_null)
                {
                    map.insert("enhance_grade".into(), serde_json::json!("highest"));
                    changed = true;
                }
            }
        }
        if changed {
            conn.execute(
                "UPDATE characters SET equipment = ?1 WHERE id = ?2",
                params![serde_json::to_string(&value)?, character_id],
            )?;
        }
    }
    Ok(())
}

/// v5 以前の `equipment` 列にあったパワーウェポン / ストロングウェポンを
/// `common_skills` 列へ移す(wiki: どちらも Skill/共通 の共通スキルで、装備ではない)。
///
/// `common_skills` 側にすでに値が入っている行(= 移行済み)は触らない。
/// カタログから消えた id を保存済みの `stat_sources` から取り除く。
///
/// カタログに無い id は `build_modifiers` / `CharacterSkills::validate` が弾くので、
/// **残っているとそのキャラの計算がまるごと止まる**(実機で「未知のバフです」
/// 「未知の中ディレイ減少スキルです」が出た)。カタログを入れ替えるたびにここへ 1 行足す。
const REMOVED_BUFF_IDS: &[&str] = &[
    // マスタリー【シルバースカル優勝者】→ `masteries` の boris_m2_3 へ移動(2026-08-27)
    "boris_silver_skull",
    // 共通スキル「アンリーシュ」で設定するため、常用バフから除外(2026-08-29)
    "unleash",
];

/// 中ディレイ減少スキル(いまはキャラスキル)のカタログから消えた id。
/// `migrate_character_skills` が `character_skills` に移す前に落とす。
const REMOVED_ACTUAL_DELAY_SKILL_IDS: &[&str] = &[
    // マスタリー【一閃】→ `masteries` の boris_m1_1 へ移動(2026-08-27)
    "boris_mastery_issen",
];

fn migrate_removed_buffs(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, stat_sources FROM characters")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (id, json) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let mut changed = false;

        if let Some(choices) = value
            .get_mut("buffs")
            .and_then(|b| b.get_mut("choices"))
            .and_then(|c| c.as_array_mut())
        {
            let before = choices.len();
            choices.retain(|c| {
                c.get("buff_id")
                    .and_then(|v| v.as_str())
                    .is_none_or(|id| !REMOVED_BUFF_IDS.contains(&id))
            });
            changed |= choices.len() != before;
        }

        if let Some(ids) = value
            .get_mut("actual_delay_skills")
            .and_then(|a| a.get_mut("skill_ids"))
            .and_then(|c| c.as_array_mut())
        {
            let before = ids.len();
            ids.retain(|v| {
                v.as_str()
                    .is_none_or(|id| !REMOVED_ACTUAL_DELAY_SKILL_IDS.contains(&id))
            });
            changed |= ids.len() != before;
        }

        if !changed {
            continue;
        }
        let migrated = serde_json::to_string(&value)?;
        conn.execute(
            "UPDATE characters SET stat_sources = ?1 WHERE id = ?2",
            params![migrated, id],
        )?;
    }
    Ok(())
}

/// 中ディレイ減少スキルと、バフカタログにあったキャラスキルを `character_skills` に寄せる
/// (2026-08-27)。器を 1 本にしたので、保存済みの選択も 1 つのキーにまとめる。
const MOVED_BUFF_TO_CHARACTER_SKILL: &[(&str, &str)] = &[
    ("benya_soul_gate", "benya_soul_gate"),
    ("ispin_encourage", "ispin_encourage"),
    ("roamini_ha_petit", "roamini_ha_petit"),
    ("roamini_powatun", "roamini_powatun"),
    ("siberin_charm", "siberin_charm"),
    ("joshua_elite_swordsman", "joshua_possession_swordsman"),
    ("joshua_elite_mage", "joshua_possession_mage"),
    ("tichiel_magic_teacher", "tichiel_magic_teacher"),
];

fn migrate_character_skills(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, stat_sources FROM characters")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (id, json) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&json)?;
        let Some(object) = value.as_object_mut() else {
            continue;
        };
        let mut moved: Vec<String> = Vec::new();

        // 中ディレイ減少スキルは id を変えずにそのまま移す
        if let Some(old) = object.remove("actual_delay_skills") {
            if let Some(ids) = old.get("skill_ids").and_then(|v| v.as_array()) {
                moved.extend(ids.iter().filter_map(|v| v.as_str()).map(str::to_string));
            }
        }

        // バフとして持っていたキャラスキルは、新しい id に読み替えて移す
        if let Some(choices) = object
            .get_mut("buffs")
            .and_then(|b| b.get_mut("choices"))
            .and_then(|c| c.as_array_mut())
        {
            choices.retain(|c| {
                let Some(buff_id) = c.get("buff_id").and_then(|v| v.as_str()) else {
                    return true;
                };
                match MOVED_BUFF_TO_CHARACTER_SKILL
                    .iter()
                    .find(|(from, _)| *from == buff_id)
                {
                    Some((_, to)) => {
                        moved.push((*to).to_string());
                        false
                    }
                    None => true,
                }
            });
        }

        if moved.is_empty() && !object.contains_key("character_skills") {
            continue;
        }
        let existing = object
            .get("character_skills")
            .and_then(|v| v.get("skill_ids"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_else(Vec::new);
        let mut skill_ids: Vec<String> = existing;
        for id in moved {
            if !skill_ids.contains(&id) {
                skill_ids.push(id);
            }
        }
        object.insert(
            "character_skills".to_string(),
            serde_json::json!({ "skill_ids": skill_ids }),
        );
        let migrated = serde_json::to_string(&value)?;
        conn.execute(
            "UPDATE characters SET stat_sources = ?1 WHERE id = ?2",
            params![migrated, id],
        )?;
    }
    Ok(())
}

fn migrate_weapon_skills_to_common(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, equipment, common_skills FROM characters")?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (id, equipment_json, common_json) in rows {
        let mut equipment: serde_json::Value = serde_json::from_str(&equipment_json)?;
        let power_weapon = equipment.get("power_weapon").and_then(|v| v.as_bool());
        let strong_weapon_level = equipment
            .get("strong_weapon_level")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8);
        if power_weapon.is_none() && strong_weapon_level.is_none() {
            continue;
        }
        let mut common: CommonSkills = serde_json::from_str(&common_json)?;
        if common == CommonSkills::default() {
            common.power_weapon = power_weapon.unwrap_or(false);
            common.strong_weapon_level = strong_weapon_level.unwrap_or(0);
            // ストロングウェポン Lv2 以降はオーグメントが要る。移行前のデータには
            // オーグメント Lv が無いので、その Lv を取れるだけの値を入れておく
            // (検証で弾かれて保存できなくなるのを避ける)。
            common.augment_level = common.strong_weapon_level.saturating_sub(1);
        }
        if let Some(map) = equipment.as_object_mut() {
            map.remove("power_weapon");
            map.remove("strong_weapon_level");
        }
        conn.execute(
            "UPDATE characters SET equipment = ?1, common_skills = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(&equipment)?,
                serde_json::to_string(&common)?,
                id
            ],
        )?;
    }
    Ok(())
}

pub struct CharacterRepository {
    pub(crate) conn: Connection,
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
        // v13: ホームの「次の目標」。既存キャラは未設定(NULL)= 自動判定のままで読める。
        if !existing_columns.contains("goal_content_id") {
            conn.execute_batch("ALTER TABLE characters ADD COLUMN goal_content_id TEXT;")?;
        }
        // v6: 共通スキル。既存キャラは `{}`(全部未習得)で読める。
        if !existing_columns.contains("common_skills") {
            conn.execute_batch(
                "ALTER TABLE characters ADD COLUMN common_skills TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        migrate_equipment_to_parts(&conn)?;
        migrate_relic_to_pendant(&conn)?;
        migrate_equipment_to_registered_lists(&conn)?;
        migrate_equipment_registration_metadata(&conn)?;
        migrate_weapon_skills_to_common(&conn)?;
        migrate_removed_buffs(&conn)?;
        migrate_character_skills(&conn)?;
        // v9 は旧バフに混在していたキャラスキルを分離した後の choices を抽出する。
        migrate_buff_sets(&conn)?;
        migrate_unleash_from_buff_sets(&conn)?;
        migrate_damage_snapshots(&conn)?;
        crate::character_icon_repository::migrate_character_icons(&conn)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

        Ok(Self { conn })
    }

    pub fn create(
        &self,
        new: &NewCharacter,
        _catalog: &BuffCatalog,
        equipment_catalog: &[EquipmentItem],
        equipment_abilities: &[EquipmentAbilityDef],
        random_options: &[RandomOptionDef],
        titles: &[TitleDef],
        character_skills: &CharacterSkillCatalog,
    ) -> Result<RegisteredCharacter> {
        validate(
            new,
            equipment_catalog,
            equipment_abilities,
            random_options,
            titles,
            character_skills,
        )?;
        let s = &new.base_stats;
        let stat_sources_json = serde_json::to_string(&new.stat_sources)?;
        let equipment_json = serde_json::to_string(&new.equipment)?;
        let common_skills_json = serde_json::to_string(&new.common_skills)?;
        self.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills, main_skill_id, goal_content_id, default_buff_set_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
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
                common_skills_json,
                new.main_skill_id,
                new.goal_content_id,
                new.default_buff_set_id,
            ],
        )?;
        self.get(self.conn.last_insert_rowid())
    }

    /// 既存キャラクターの内容を丸ごと置き換える。存在しない id は `CharacterNotFound`。
    pub fn update(
        &self,
        id: i64,
        update: &NewCharacter,
        _catalog: &BuffCatalog,
        equipment_catalog: &[EquipmentItem],
        equipment_abilities: &[EquipmentAbilityDef],
        random_options: &[RandomOptionDef],
        titles: &[TitleDef],
        character_skills: &CharacterSkillCatalog,
    ) -> Result<RegisteredCharacter> {
        validate(
            update,
            equipment_catalog,
            equipment_abilities,
            random_options,
            titles,
            character_skills,
        )?;
        let s = &update.base_stats;
        let stat_sources_json = serde_json::to_string(&update.stat_sources)?;
        let equipment_json = serde_json::to_string(&update.equipment)?;
        let common_skills_json = serde_json::to_string(&update.common_skills)?;
        let affected = self.conn.execute(
            "UPDATE characters SET
                name = ?1, game_character_id = ?2,
                stab = ?3, hack = ?4, int = ?5, def = ?6, mr = ?7, dex = ?8, agi = ?9,
                awakening_stage = ?10, eternal_level = ?11, stat_sources = ?12, equipment = ?13,
                common_skills = ?14, main_skill_id = ?15, goal_content_id = ?16,
                default_buff_set_id = ?17,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?18",
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
                common_skills_json,
                update.main_skill_id,
                update.goal_content_id,
                update.default_buff_set_id,
                id,
            ],
        )?;
        if affected == 0 {
            return Err(StorageError::CharacterNotFound(id));
        }
        self.get(id)
    }

    pub fn list(&self) -> Result<Vec<RegisteredCharacter>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM characters ORDER BY id"
        ))?;
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
        let affected = self
            .conn
            .execute("DELETE FROM characters WHERE id = ?1", [id])?;
        if affected == 0 {
            return Err(StorageError::CharacterNotFound(id));
        }
        Ok(())
    }
}

/// 登録リクエストの検証(値域・バフ整合性・装備カタログ整合性)。保存前プレビュー(preview_damage 等)からも使う。
pub fn validate(
    new: &NewCharacter,
    equipment_catalog: &[EquipmentItem],
    equipment_abilities: &[EquipmentAbilityDef],
    random_options: &[RandomOptionDef],
    titles: &[TitleDef],
    character_skills: &CharacterSkillCatalog,
) -> Result<()> {
    new.validate(&domain::CharacterCatalogs {
        equipment: equipment_catalog,
        abilities: equipment_abilities,
        random_options,
        titles,
        character_skills,
    })
    .map_err(StorageError::InvalidValue)
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
        common_skills: row.get::<_, CommonSkillsColumn>("common_skills")?.0,
        main_skill_id: row.get("main_skill_id")?,
        goal_content_id: row.get("goal_content_id")?,
        default_buff_set_id: row.get("default_buff_set_id")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use domain::{
        BuffChoice, BuffDefinition, BuffOrigin, BuffPurpose, BuffSelection, BuffTarget, BuffValue, Crown,
        PetSkillTier, PetSkills, RuneLevels, SacredRelic, StatLayer, StatSources,
    };

    use super::*;

    #[test]
    fn v11からv12で既存キャラを変えず画像テーブルを追加する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        repo.conn.execute("INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills)
            VALUES ('既存', 'boris', 1,1,1,1,1,1,1,4,0,'{}','{}','{}')", []).unwrap();
        repo.conn.execute_batch("DROP TABLE character_icons; PRAGMA user_version = 11;").unwrap();
        let conn = repo.conn;
        let migrated = CharacterRepository::from_connection(conn).unwrap();
        assert_eq!(migrated.conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0)).unwrap(), SCHEMA_VERSION);
        assert_eq!(migrated.conn.query_row("SELECT name FROM characters", [], |row| row.get::<_, String>(0)).unwrap(), "既存");
        assert_eq!(migrated.conn.query_row("SELECT count(*) FROM character_icons", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }

    fn v8_buff_migration_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION).unwrap();
        conn.execute_batch(
            "ALTER TABLE characters ADD COLUMN stat_sources TEXT NOT NULL DEFAULT '{}';
             ALTER TABLE characters ADD COLUMN default_dummy TEXT;
             PRAGMA user_version = 8;",
        )
        .unwrap();
        conn
    }

    #[test]
    fn v9移行はキャラごとにバフセットを作り再実行しても増えない() {
        let conn = v8_buff_migration_connection();
        let sources = serde_json::json!({
            "buffs": { "choices": [{"buff_id":"trust_potion","stat":null,"choice_index":null,"value":10.0}] }
        }).to_string();
        for (id, name) in [(1, "A"), (2, "B")] {
            conn.execute(
                "INSERT INTO characters (id,name,game_character_id,stab,hack,int,def,mr,dex,agi,awakening_stage,eternal_level,stat_sources)
                 VALUES (?1,?2,'boris',1,1,1,1,1,1,1,0,0,?3)",
                params![id, name, sources],
            ).unwrap();
        }
        migrate_buff_sets(&conn).unwrap();
        migrate_buff_sets(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM buff_sets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            count, 2,
            "同じ choices でもキャラごとに1件、再openでは増えない"
        );
        let defaults: Vec<Option<i64>> = {
            let mut stmt = conn
                .prepare("SELECT default_buff_set_id FROM characters ORDER BY id")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        assert!(defaults.iter().all(Option::is_some));
        assert_ne!(defaults[0], defaults[1]);
        let json: String = conn
            .query_row(
                "SELECT stat_sources FROM characters WHERE id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&json)
            .unwrap()
            .get("buffs")
            .is_none());
    }

    #[test]
    fn v10移行はアンリーシュを除きクラブ効果へ従来値を補う() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION).unwrap();
        conn.execute_batch(
            "CREATE TABLE buff_sets (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                choices TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );",
        ).unwrap();
        let choices = serde_json::json!({
            "choices": [
                {"buff_id":"unleash","stat":null,"choice_index":null,"value":null},
                {"buff_id":"trust_potion","stat":null,"choice_index":null,"value":10.0},
                {"buff_id":"club_effect","stat":"stab","choice_index":null,"value":null}
            ]
        });
        conn.execute(
            "INSERT INTO buff_sets (name, choices) VALUES ('移行前', ?1)",
            [choices.to_string()],
        ).unwrap();

        migrate_unleash_from_buff_sets(&conn).unwrap();
        migrate_unleash_from_buff_sets(&conn).unwrap();

        let json: String = conn.query_row(
            "SELECT choices FROM buff_sets WHERE name = '移行前'",
            [],
            |row| row.get(0),
        ).unwrap();
        let migrated: BuffSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(migrated.choices.len(), 2);
        assert_eq!(migrated.choices[0].buff_id, "trust_potion");
        assert_eq!(migrated.choices[1].buff_id, "club_effect");
        assert_eq!(migrated.choices[1].value, Some(7.0));
    }

    #[test]
    fn v11移行は既存キャラを保ったままupdated_at列とdamage_snapshotsテーブルを追加する() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION).unwrap();
        conn.execute(
            "INSERT INTO characters (id,name,game_character_id,stab,hack,int,def,mr,dex,agi,awakening_stage,eternal_level)
             VALUES (1,'既存','boris',1,1,1,1,1,1,1,0,0)",
            [],
        ).unwrap();

        migrate_damage_snapshots(&conn).unwrap();
        migrate_damage_snapshots(&conn).unwrap();

        let name: String = conn
            .query_row("SELECT name FROM characters WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "既存");
        let updated_at: Option<String> = conn
            .query_row("SELECT updated_at FROM characters WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(updated_at, None, "既存行の updated_at は NULL のまま");
        let has_table: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='damage_snapshots')",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!(has_table);
    }

    #[test]
    fn create_updateはupdated_atを埋める() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(&new_character("a"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert!(created.updated_at.is_some());

        let updated = new_character("a改");
        let result = repo
            .update(created.id, &updated, &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert!(result.updated_at.is_some());
    }

    #[test]
    fn v9移行失敗はテーブル追加と抽出をまとめてrollbackする() {
        let conn = v8_buff_migration_connection();
        conn.execute(
            "INSERT INTO characters (id,name,game_character_id,stab,hack,int,def,mr,dex,agi,awakening_stage,eternal_level,stat_sources)
             VALUES (1,'broken','boris',1,1,1,1,1,1,1,0,0,'{')",
            [],
        ).unwrap();
        assert!(migrate_buff_sets(&conn).is_err());
        let has_column: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(characters)").unwrap();
            stmt.query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
                .iter()
                .any(|name| name == "default_buff_set_id")
        };
        assert!(!has_column);
        let has_table: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='buff_sets')", [], |row| row.get(0)
        ).unwrap();
        assert!(!has_table);
    }

    #[test]
    fn バフセットcrudと削除時の既定解除が動く() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let choices = BuffSelection {
            choices: vec![buff_choice("illumination_drink")],
        };
        let created = repo
            .create_buff_set("普段", &choices, &test_catalog())
            .unwrap();
        let renamed = repo
            .update_buff_set(created.id, "ボス", &choices, &test_catalog())
            .unwrap();
        assert_eq!(renamed.name, "ボス");
        let copy = repo.duplicate_buff_set(created.id).unwrap();
        assert_eq!(copy.choices, choices);
        let mut character = new_character("x");
        character.default_buff_set_id = Some(created.id);
        let registered = repo
            .create(&character, &test_catalog(), &[], &[], &[], &[], &[])
            .unwrap();
        repo.delete_buff_set(created.id).unwrap();
        assert_eq!(repo.get(registered.id).unwrap().default_buff_set_id, None);
        assert!(repo
            .create_buff_set("  ", &BuffSelection::default(), &test_catalog())
            .is_err());
    }

    fn new_character(name: &str) -> NewCharacter {
        NewCharacter {
            name: name.to_string(),
            game_character_id: "boris".to_string(),
            base_stats: BaseStats {
                stab: 300,
                hack: 250,
                int: 10,
                def: 200,
                mr: 150,
                dex: 280,
                agi: 250,
            },
            awakening: Awakening {
                stage: 5,
                eternal_level: 40,
            },
            stat_sources: StatSources::default(),
            equipment: Equipment::default(),
            common_skills: CommonSkills::default(),
            main_skill_id: None,
            goal_content_id: None,
            default_buff_set_id: None,
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
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Item,
                target: BuffTarget::AllStats,
                layer: StatLayer::Fixed,
                value: BuffValue::UserInput {
                    min: 0.0,
                    max: 33.0,
                },
                exclusive_slots: vec!["trust_potion"],
                source_url: "",
                note: "",
                default_value: Some(33.0),
                damage_effects: &[],
            },
            BuffDefinition {
                id: "illumination_drink",
                name: "イルミネーション祭りのドリンク",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Item,
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.30),
                exclusive_slots: vec!["percent_slot_1", "percent_slot_2"],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
            BuffDefinition {
                id: "charge_potion",
                name: "充填の秘薬",
                purposes: &[BuffPurpose::Stats],
                origin: BuffOrigin::Item,
                target: BuffTarget::AllStats,
                layer: StatLayer::PercentOfBase,
                value: BuffValue::Fixed(0.20),
                exclusive_slots: vec!["percent_slot_1"],
                source_url: "",
                note: "",
                default_value: None,
                damage_effects: &[],
            },
        ]
    }

    fn buff_choice(id: &str) -> BuffChoice {
        BuffChoice {
            buff_id: id.to_string(),
            stat: None,
            choice_index: None,
            value: None,
        }
    }

    #[test]
    fn 登録して一覧と取得ができる() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        assert!(repo.list().unwrap().is_empty());

        let created = repo
            .create(&new_character("メイン"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(created.name, "メイン");
        assert_eq!(created.base_stats.hack, 250);
        assert_eq!(
            created.awakening,
            Awakening {
                stage: 5,
                eternal_level: 40
            }
        );

        let second = repo
            .create(&new_character("サブ"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_ne!(created.id, second.id);

        let list = repo.list().unwrap();
        assert_eq!(list, vec![created.clone(), second]);
        assert_eq!(repo.get(created.id).unwrap(), created);
    }

    #[test]
    fn 削除できる_存在しないidはエラー() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(&new_character("メイン"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        repo.delete(created.id).unwrap();
        assert!(repo.list().unwrap().is_empty());
        assert!(matches!(
            repo.delete(created.id),
            Err(StorageError::CharacterNotFound(_))
        ));
        assert!(matches!(
            repo.get(created.id),
            Err(StorageError::CharacterNotFound(_))
        ));
    }

    #[test]
    fn 不正な値は拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("  ");
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
        c.name = "x".into();
        c.awakening.stage = 6;
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
        c.awakening.stage = 5;
        c.awakening.eternal_level = 101;
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn 素ステは1から310の範囲() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.base_stats.int = 0;
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
        c.base_stats.int = 311;
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
        c.base_stats.int = 1;
        c.base_stats.agi = 310;
        assert!(repo.create(&c, &[], &[], &[], &[], &[], &[]).is_ok());
    }

    #[test]
    fn マイグレーションは再適用しても壊れない() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        repo.conn.execute_batch(MIGRATION).unwrap();
        repo.create(&new_character("a"), &[], &[], &[], &[], &[], &[])
            .unwrap();
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
        let created = repo
            .create(&new_character("新データ"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(created.stat_sources, StatSources::default());
        assert_eq!(created.equipment, Equipment::default());

        let mut updated = new_character("旧データ改");
        updated.stat_sources.rune_levels.stab = 10;
        let result = repo
            .update(list[0].id, &updated, &[], &[], &[], &[], &[], &[])
            .unwrap();
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
        // パワーウェポン / ストロングウェポンは共通スキル(v6)へ移る。
        assert_eq!(list[0].common_skills.power_weapon, true);
        assert_eq!(list[0].common_skills.strong_weapon_level, 6);
        // Lv6 を取れるだけのオーグメント Lv を補って、検証で弾かれないようにする
        assert_eq!(list[0].common_skills.augment_level, 5);
        // 旧合計値(base/enhanced相当)は破棄され、部位はすべて default(未装備)になる。
        assert_eq!(list[0].equipment.parts, domain::EquipmentParts::default());

        // DB 上の JSON 自体も書き換わっている(再度開いても同じ結果)ことを確認する。
        let raw: String = repo
            .conn
            .query_row(
                "SELECT equipment FROM characters WHERE id = ?1",
                [list[0].id],
                |r| r.get(0),
            )
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
        assert_eq!(
            conn.query_row::<i64, _, _>("PRAGMA user_version", [], |r| r.get(0))
                .unwrap(),
            0
        );

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
        repo.create(&new_character("追加データ"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(repo.list().unwrap().len(), 2);
    }

    #[test]
    fn v8装備移行は空部位を登録せず実装備だけ選択中にする() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE characters (id INTEGER PRIMARY KEY, equipment TEXT NOT NULL);",
        )
        .unwrap();
        let old = serde_json::json!({
            "parts": {
                "weapon": {
                    "item_id": null, "custom_name": null, "base": {}, "enchant": {},
                    "enhance_level": 0, "enhance_added_damage": null,
                    "abilities": [], "siena": { "slots": [], "extras": [] }, "random_options": []
                },
                "armor": {
                    "item_id": "abyss-armor", "custom_name": null,
                    "base": { "physical_defense": 280, "magic_defense": 260 }, "enchant": {},
                    "enhance_level": 15, "enhance_added_damage": 746200,
                    "abilities": [], "siena": { "slots": [], "extras": [] }, "random_options": []
                }
            }
        });
        conn.execute(
            "INSERT INTO characters (id, equipment) VALUES (1, ?1)",
            [old.to_string()],
        )
        .unwrap();

        migrate_equipment_to_registered_lists(&conn).unwrap();
        let raw: String = conn
            .query_row("SELECT equipment FROM characters WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let migrated: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let weapon = &migrated["parts"]["weapon"];
        assert_eq!(weapon["registered"].as_array().unwrap().len(), 0);
        assert!(weapon["selected_id"].is_null());
        let armor = &migrated["parts"]["armor"];
        assert_eq!(armor["registered"].as_array().unwrap().len(), 1);
        assert_eq!(armor["selected_id"], 1);
        assert_eq!(armor["registered"][0]["enhance_grade"], "lowest");
        assert_eq!(armor["registered"][0]["enhance_type"], "armor_light");
        assert!(armor["registered"][0].get("enhance_added_damage").is_none());
    }

    #[test]
    fn 現行装備メタデータ移行は旧アビリティを除去して保存可能にする() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE characters (id INTEGER PRIMARY KEY, equipment TEXT NOT NULL);",
        )
        .unwrap();
        let equipment = serde_json::json!({
            "parts": {
                "weapon": { "registered": [{
                    "id": 1, "item_id": "abyss-scimitar", "abilities": ["sharp-blade-high", "night-star-sharp-blade"],
                    "enhance_level": 12, "enhance_grade": null
                }], "selected_id": 1 }
            }
        });
        conn.execute(
            "INSERT INTO characters (id, equipment) VALUES (1, ?1)",
            [equipment.to_string()],
        )
        .unwrap();

        migrate_equipment_registration_metadata(&conn).unwrap();
        let raw: String = conn
            .query_row("SELECT equipment FROM characters WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let migrated: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let weapon = &migrated["parts"]["weapon"]["registered"][0];
        assert_eq!(
            weapon["abilities"],
            serde_json::json!(["night-star-sharp-blade"])
        );
        assert_eq!(weapon["enhance_grade"], "highest");
        assert_eq!(weapon["enhance_type"], "weapon_hack");
    }

    #[test]
    fn stat_sourcesはjsonで往復する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("メイン");
        c.stat_sources = StatSources {
            pet_skills: PetSkills {
                stab: Some(PetSkillTier::TrueLv4),
                ..Default::default()
            },
            rune_levels: RuneLevels {
                hack: 20,
                ..Default::default()
            },
            sacred_relic: SacredRelic {
                int: 40,
                ..Default::default()
            },
            ..Default::default()
        };
        let created = repo
            .create(&c, &test_catalog(), &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(created.stat_sources, c.stat_sources);

        let fetched = repo.get(created.id).unwrap();
        assert_eq!(fetched.stat_sources, c.stat_sources);

        let listed = repo.list().unwrap();
        assert_eq!(listed[0].stat_sources, c.stat_sources);
    }

    #[test]
    fn equipmentはjsonで往復する() {
        use domain::{
            EquipmentPart, EquipmentParts, EquipmentValues, RegisteredSienaAura, SienaAura,
            SienaAuraList, SienaAuras, SienaSlot, SienaValueKind,
        };

        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("メイン");
        c.equipment = Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart {
                    base: EquipmentValues {
                        thrust: 150,
                        slash: 150,
                        magic_attack: 0,
                        magic_defense: 0,
                        ..Default::default()
                    },
                    enchant: EquipmentValues {
                        thrust: 60,
                        slash: 60,
                        magic_attack: 0,
                        magic_defense: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            siena: SienaAuras {
                weapon: SienaAuraList {
                    registered: vec![RegisteredSienaAura {
                        id: 1,
                        label: "火力用".into(),
                        aura: SienaAura {
                            slots: vec![SienaSlot {
                                kind: SienaValueKind::Thrust,
                                value: 10,
                            }],
                            extras: vec![],
                        },
                    }],
                    selected_id: Some(1),
                },
                ..Default::default()
            },
            ..Default::default()
        };
        c.common_skills = CommonSkills {
            power_weapon: true,
            strong_weapon_level: 6,
            augment_level: 5,
            ..Default::default()
        };
        let created = repo.create(&c, &[], &[], &[], &[], &[], &[]).unwrap();
        assert_eq!(created.equipment, c.equipment);
        assert_eq!(created.common_skills, c.common_skills);

        let fetched = repo.get(created.id).unwrap();
        assert_eq!(fetched.equipment, c.equipment);

        let listed = repo.list().unwrap();
        assert_eq!(listed[0].equipment, c.equipment);
    }

    #[test]
    fn 同じカテゴリーのアビリティを2つ持つと拒否する() {
        use domain::{EquipmentAbilityFamily, EquipmentPart, EquipmentParts};

        let repo = CharacterRepository::open_in_memory().unwrap();
        let abilities = [
            EquipmentAbilityDef {
                id: "pointed-blade-low",
                name: "(下)尖った刃",
                family: EquipmentAbilityFamily::PointedBlade,
                category: 1,
                slot: domain::PartSlot::Weapon,
                value_option: None,
                exclusive_group: "weapon-category-1",
                additional_slots: 0,
                additional_effects: "",
                additional_options: vec![],
                record_only: false,
                grade: None,
                ladder: String::new(),
                priority: 0,
                effect_summary: "突き +2",
                values: domain::EquipmentValues {
                    thrust: 2,
                    ..Default::default()
                },
                damage_effects: &[],
            },
            EquipmentAbilityDef {
                id: "pointed-blade-e",
                name: "E-尖った刃",
                family: EquipmentAbilityFamily::PointedBlade,
                category: 1,
                slot: domain::PartSlot::Weapon,
                value_option: None,
                exclusive_group: "weapon-category-1",
                additional_slots: 0,
                additional_effects: "",
                additional_options: vec![],
                record_only: false,
                grade: None,
                ladder: String::new(),
                priority: 0,
                effect_summary: "突き +9",
                values: domain::EquipmentValues {
                    thrust: 9,
                    ..Default::default()
                },
                damage_effects: &[],
            },
            EquipmentAbilityDef {
                id: "night-star-pointed-blade",
                name: "夜星の尖った刃",
                family: EquipmentAbilityFamily::PointedBlade,
                category: 4,
                slot: domain::PartSlot::Weapon,
                value_option: None,
                exclusive_group: "weapon-category-4",
                additional_slots: 2,
                additional_effects: "",
                additional_options: vec![],
                record_only: false,
                grade: None,
                ladder: String::new(),
                priority: 0,
                effect_summary: "突き +20",
                values: domain::EquipmentValues {
                    thrust: 20,
                    ..Default::default()
                },
                damage_effects: &[],
            },
        ];
        let mut c = new_character("メイン");
        c.equipment = Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart {
                    abilities: vec!["pointed-blade-low".into(), "pointed-blade-e".into()],
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = repo
            .create(&c, &[], &[], &abilities, &[], &[], &[])
            .unwrap_err();
        assert!(err.to_string().contains("系統"), "{err}");

        // 同じ突き系統でもカテゴリーが違えば通る
        c.equipment.parts.weapon.selected_or_register().abilities =
            vec!["pointed-blade-e".into(), "night-star-pointed-blade".into()];
        repo.create(&c, &[], &[], &abilities, &[], &[], &[])
            .unwrap();
    }

    #[test]
    fn main_skill_idは往復し未選択はnullで読める() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("メイン");
        c.main_skill_id = Some("boris_goku_zaneizan".to_string());
        let created = repo.create(&c, &[], &[], &[], &[], &[], &[]).unwrap();
        assert_eq!(
            created.main_skill_id.as_deref(),
            Some("boris_goku_zaneizan")
        );
        assert_eq!(
            repo.get(created.id).unwrap().main_skill_id.as_deref(),
            Some("boris_goku_zaneizan")
        );

        // 未選択(None)も保存できる。update で解除できること。
        let mut cleared = c.clone();
        cleared.main_skill_id = None;
        let updated = repo
            .update(created.id, &cleared, &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(updated.main_skill_id, None);
    }

    #[test]
    fn goal_content_idは往復し未設定はnullで読める() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("目標");
        c.goal_content_id = Some("relic-sanctuary-15".to_string());
        let created = repo.create(&c, &[], &[], &[], &[], &[], &[]).unwrap();
        assert_eq!(created.goal_content_id.as_deref(), Some("relic-sanctuary-15"));
        assert_eq!(
            repo.get(created.id).unwrap().goal_content_id.as_deref(),
            Some("relic-sanctuary-15")
        );

        // 「自動で選ぶ」に戻す = None を保存できること(自動をやめないための出口)。
        let mut cleared = c.clone();
        cleared.goal_content_id = None;
        let updated = repo
            .update(created.id, &cleared, &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(updated.goal_content_id, None);
    }

    /// v12 の DB(`goal_content_id` 列が無い)を開いても既存キャラは壊れず、目標は未設定で読める。
    #[test]
    fn goal_content_id列の無いv12dbを開くと既存キャラは未設定で読める() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE characters (
                id                  INTEGER PRIMARY KEY,
                name                TEXT    NOT NULL,
                game_character_id   TEXT    NOT NULL,
                stab                INTEGER NOT NULL,
                hack                INTEGER NOT NULL,
                int                 INTEGER NOT NULL,
                def                 INTEGER NOT NULL,
                mr                  INTEGER NOT NULL,
                dex                 INTEGER NOT NULL,
                agi                 INTEGER NOT NULL,
                awakening_stage     INTEGER NOT NULL,
                eternal_level       INTEGER NOT NULL,
                stat_sources        TEXT    NOT NULL,
                equipment           TEXT    NOT NULL,
                common_skills       TEXT    NOT NULL,
                main_skill_id       TEXT,
                default_buff_set_id INTEGER,
                updated_at          TEXT,
                created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills, main_skill_id)
            VALUES ('v12データ', 'boris', 300, 250, 10, 200, 150, 280, 250, 5, 40, '{}', '{\"parts\":{}}', '{}', 'boris_goku_zaneizan');
            PRAGMA user_version = 12;
            ",
        )
        .unwrap();

        let repo = CharacterRepository::from_connection(conn).unwrap();
        let list = repo.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "v12データ");
        // 既存の値は壊れない
        assert_eq!(list[0].main_skill_id.as_deref(), Some("boris_goku_zaneizan"));
        assert_eq!(list[0].base_stats.stab, 300);
        // 目標は未設定 = 自動判定のまま
        assert_eq!(list[0].goal_content_id, None);

        // 移行後の行も含めて create/update が使えること。
        let created = repo
            .create(&new_character("追加データ"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(created.goal_content_id, None);
        let mut existing = new_character("v12データ");
        existing.goal_content_id = Some("relic-sanctuary-15".to_string());
        let updated = repo
            .update(list[0].id, &existing, &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(updated.goal_content_id.as_deref(), Some("relic-sanctuary-15"));
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
        repo.create(&new_character("追加データ"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(repo.list().unwrap().len(), 2);
    }

    #[test]
    fn 装備の値域違反は拒否する() {
        use domain::EquipmentValues;

        let repo = CharacterRepository::open_in_memory().unwrap();

        let mut over_value = new_character("x");
        over_value.equipment.parts.weapon.selected_or_register().base = EquipmentValues {
            thrust: 10000,
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&over_value, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut over_level = new_character("x");
        over_level.common_skills.strong_weapon_level = 7;
        over_level.common_skills.augment_level = 5;
        assert!(matches!(
            repo.create(&over_level, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        // オーグメントが足りない Lv も弾く(wiki: Lv2 以降はオーグメントの LvUp が必要)
        let mut no_augment = new_character("x");
        no_augment.common_skills.strong_weapon_level = 6;
        assert!(matches!(
            repo.create(&no_augment, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    fn test_equipment_item() -> EquipmentItem {
        EquipmentItem {
            id: "test-weapon",
            icon_id: "test-weapon",
            slot: domain::PartSlot::Weapon,
            name: "テスト武器",
            values_min: domain::EquipmentValues::default(),
            values_max: domain::EquipmentValues {
                thrust: 100,
                slash: 100,
                magic_attack: 0,
                magic_defense: 0,
                ..Default::default()
            },
            growth_cap: None,
            growth_caps: None,
            ability_slots: domain::PartSlot::Weapon.ability_slots(),
            random_option_slots: domain::PartSlot::Weapon.random_option_slots(),
            enchant_caps: domain::EquipmentValues {
                thrust: 50,
                slash: 50,
                magic_attack: 0,
                magic_defense: 0,
                ..Default::default()
            },
            wrist_type: None,
            weapon_class: None,
            enhance_type: None,
            armor_class: None,
            damage_effects: &[],
            survival_effects: &[],
            recommended_dependency: None,
            damage_dependency: None,
            relic: None,
            source: gamedata::EQUIPMENT_CATALOG_SOURCE,
        }
    }

    fn test_equipment_ability() -> EquipmentAbilityDef {
        EquipmentAbilityDef {
            id: "test-ability",
            name: "テストアビリティ",
            family: domain::EquipmentAbilityFamily::PointedBlade,
            category: 1,
            slot: domain::PartSlot::Weapon,
            value_option: None,
            exclusive_group: "weapon-category-1",
            additional_slots: 0,
            additional_effects: "",
            additional_options: vec![],
            record_only: false,
            grade: None,
            ladder: String::new(),
            priority: 0,
            effect_summary: "突き +2",
            values: domain::EquipmentValues::default(),
            damage_effects: &[],
        }
    }

    const TEST_RO_TIERS: &[domain::RandomOptionTier] = &[domain::RandomOptionTier {
        rank: domain::RandomOptionRank::Rare,
        min: 6.0,
        max: 8.0,
    }];

    /// 盾のカテゴリー15 を 2 件(排他)と、カテゴリー 0 を 1 件(共存できる)。
    fn test_random_options() -> Vec<domain::RandomOptionDef> {
        let def = |id, category, effect| domain::RandomOptionDef {
            id,
            name: id,
            slot: domain::PartSlot::Shield,
            category,
            effect,
            tiers: TEST_RO_TIERS,
            note: "",
            common: false,
            short: id,
        };
        vec![
            def("ro-a", 15, domain::RandomOptionEffect::AttackDamageRate),
            def("ro-b", 15, domain::RandomOptionEffect::AccuracyPoint),
            def("ro-free", 0, domain::RandomOptionEffect::RecordOnly),
        ]
    }

    fn ro_slot(id: &str) -> domain::RandomOptionSlot {
        domain::RandomOptionSlot {
            option_id: id.to_string(),
            rank: domain::RandomOptionRank::Rare,
            value: None,
        }
    }

    /// カタログから消えたバフ id は起動時に落とす。残っていると `UnknownBuff` で
    /// そのキャラの計算がまるごと止まる(マスタリーをバフから移した 2026-08-27)。
    /// あわせて、バフ・中ディレイ減少スキルとして持っていたキャラスキルを
    /// `character_skills` に寄せる(器を 1 本にした 2026-08-27)。
    #[test]
    fn 消えたバフidは移行で落ちてキャラスキルは1つのキーに寄る() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(&new_character("メイン"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        repo.conn
            .execute(
                "UPDATE characters SET stat_sources = ?1 WHERE id = ?2",
                params![
                    r#"{
                        "buffs":{"choices":[
                            {"buff_id":"boris_silver_skull","stat":null,"choice_index":null,"value":null},
                            {"buff_id":"siberin_charm","stat":null,"choice_index":null,"value":null}
                        ]},
                        "actual_delay_skills":{"skill_ids":["boris_mastery_issen","boris_sword_priest"]}
                    }"#,
                    created.id
                ],
            )
            .unwrap();

        migrate_removed_buffs(&repo.conn).unwrap();
        migrate_character_skills(&repo.conn).unwrap();

        let migrated_json: String = repo
            .conn
            .query_row(
                "SELECT stat_sources FROM characters WHERE id = ?1",
                [created.id],
                |row| row.get(0),
            )
            .unwrap();
        let migrated_value: serde_json::Value = serde_json::from_str(&migrated_json).unwrap();
        let ids: Vec<String> = migrated_value
            .get("buffs")
            .and_then(|v| v.get("choices"))
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| {
                v.get("buff_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();
        // 消えた id は落ち、キャラスキルだった siberin_charm はバフから抜けている
        assert!(ids.is_empty(), "{ids:?}");
        // 中ディレイ減少スキルとバフのキャラスキルが 1 つのキーに寄る
        assert_eq!(
            repo.get(created.id)
                .unwrap()
                .stat_sources
                .character_skills
                .skill_ids,
            vec![
                "boris_sword_priest".to_string(),
                "siberin_charm".to_string()
            ]
        );
    }

    fn test_titles() -> Vec<domain::TitleDef> {
        vec![domain::TitleDef {
            common: false,
            id: "test-title",
            name: "テスト称号",
            group: "テスト",
            level: None,
            values: domain::EquipmentValues {
                thrust: 40,
                ..Default::default()
            },
            attack_damage_percent: 0.0,
            conditional_added_damage: None,
            note: "",
        }]
    }

    /// キャラスキル(wiki: 各キャラの Skill ページ / ステータスの各カテゴリ表)。
    fn test_character_skills() -> Vec<domain::CharacterSkillDef> {
        vec![
            domain::CharacterSkillDef {
                id: "boris_sword_priest",
                game_character_id: "boris",
                name: "剣の司祭",
                audience: domain::SkillAudience::SelfOnly,
                max_level: 1,
                effects: &[domain::SkillEffect::ActualDelay { percent: 5.0 }],
                mastery_overrides: &[],
                source_url: "",
                note: "",
            },
            domain::CharacterSkillDef {
                id: "mira_spurt",
                game_character_id: "mira",
                name: "極・スパート",
                audience: domain::SkillAudience::SelfOnly,
                max_level: 1,
                effects: &[domain::SkillEffect::RecordOnly],
                mastery_overrides: &[],
                source_url: "",
                note: "",
            },
        ]
    }

    #[test]
    fn キャラスキルは他キャラのものを拒否しjsonで往復する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let catalog = test_character_skills();
        let mut c = new_character("x"); // ボリス
        c.stat_sources.character_skills = domain::CharacterSkills {
            skill_ids: vec!["mira_spurt".to_string()],
            skill_levels: Default::default(),
        };
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &[], &catalog),
            Err(StorageError::InvalidValue(_))
        ));

        c.stat_sources.character_skills = domain::CharacterSkills {
            skill_ids: vec!["boris_sword_priest".to_string()],
            skill_levels: Default::default(),
        };
        let created = repo.create(&c, &[], &[], &[], &[], &[], &catalog).unwrap();
        let loaded = repo.get(created.id).unwrap();
        assert_eq!(
            loaded.stat_sources.character_skills,
            c.stat_sources.character_skills
        );
    }

    #[test]
    fn 未知の称号idは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.title = Some("nope".to_string());
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &[], &test_titles(), &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn 称号は基本能力値に加算されjsonで往復する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.title = Some("test-title".to_string());
        let created = repo
            .create(&c, &[], &[], &[], &[], &test_titles(), &[])
            .unwrap();
        let loaded = repo.get(created.id).unwrap();
        assert_eq!(loaded.equipment.title.as_deref(), Some("test-title"));
        assert_eq!(loaded.equipment.base_totals(&[], &test_titles()).thrust, 40);
    }

    #[test]
    fn 未知のランダムオプションidは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.shield.selected_or_register().random_options = vec![ro_slot("nope")];
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &test_random_options(), &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn ランダムオプションは部位が一致しないと拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.selected_or_register().random_options = vec![ro_slot("ro-a")];
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &test_random_options(), &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn ランダムオプションに無いランクは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        let mut slot = ro_slot("ro-a");
        slot.rank = domain::RandomOptionRank::STrue;
        c.equipment.parts.shield.selected_or_register().random_options = vec![slot];
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &test_random_options(), &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    // wiki 転移: 同じカテゴリーは共存できない。カテゴリー 0(カテゴリーなし)は例外
    #[test]
    fn 同じカテゴリーのランダムオプションは1部位に1つまで() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.shield.selected_or_register().random_options = vec![ro_slot("ro-a"), ro_slot("ro-b")];
        assert!(matches!(
            repo.create(&c, &[], &[], &[], &test_random_options(), &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut ok = new_character("y");
        ok.equipment.parts.shield.selected_or_register().random_options = vec![ro_slot("ro-a"), ro_slot("ro-free")];
        assert!(repo
            .create(&ok, &[], &[], &[], &test_random_options(), &[], &[])
            .is_ok());
    }

    #[test]
    fn ランダムオプションは部位別装備のjsonで往復する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.shield.selected_or_register().random_options = vec![domain::RandomOptionSlot {
            option_id: "ro-a".to_string(),
            rank: domain::RandomOptionRank::Rare,
            value: Some(7.5),
        }];
        let created = repo
            .create(&c, &[], &[], &[], &test_random_options(), &[], &[])
            .unwrap();
        let loaded = repo.get(created.id).unwrap();
        assert_eq!(
            loaded.equipment.parts.shield.random_options,
            c.equipment.parts.shield.random_options
        );
    }

    #[test]
    fn 未知の装備アイテムidは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.selected_or_register().item_id = Some("nope".to_string());
        assert!(matches!(
            repo.create(&c, &[], &[test_equipment_item()], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn 部位とカタログのslotが不一致なら拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        // test_equipment_item は Weapon 用だが helm 部位に指定する。
        c.equipment.parts.helm.selected_or_register().item_id = Some("test-weapon".to_string());
        assert!(matches!(
            repo.create(&c, &[], &[test_equipment_item()], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn エンチャントが装備固有の固定枠を超えたら拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.selected_or_register().item_id = Some("test-weapon".to_string());
        c.equipment.parts.weapon.selected_or_register().base = domain::EquipmentValues {
            thrust: 100,
            ..Default::default()
        };
        c.equipment.parts.weapon.selected_or_register().enchant = domain::EquipmentValues {
            thrust: 51,
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&c, &[], &[test_equipment_item()], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut ok = new_character("x");
        ok.equipment.parts.weapon.selected_or_register().item_id = Some("test-weapon".to_string());
        ok.equipment.parts.weapon.selected_or_register().base = domain::EquipmentValues {
            thrust: 100,
            ..Default::default()
        };
        ok.equipment.parts.weapon.selected_or_register().enchant = domain::EquipmentValues {
            thrust: 50,
            ..Default::default()
        };
        assert!(repo
            .create(&ok, &[], &[test_equipment_item()], &[], &[], &[], &[])
            .is_ok());
    }

    #[test]
    fn 成長装備の基礎値が段階の下限外なら拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut item = test_equipment_item();
        item.growth_cap = Some(200);
        item.growth_caps = Some(domain::EquipmentValues {
            thrust: 200,
            slash: 200,
            physical_defense: 200,
            magic_attack: 200,
            magic_defense: 200,
            accuracy: 200,
            critical: 200,
            evasion: 200,
            agility: 200,
        });
        item.values_min.thrust = 30;

        let mut under = new_character("under");
        under.equipment.parts.weapon.selected_or_register().item_id = Some("test-weapon".to_string());
        under.equipment.parts.weapon.selected_or_register().base = domain::EquipmentValues {
            thrust: 29,
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&under, &[], &[item], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut over = new_character("over");
        over.equipment.parts.weapon.selected_or_register().item_id = Some("test-weapon".to_string());
        over.equipment.parts.weapon.selected_or_register().base = domain::EquipmentValues {
            thrust: 201,
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&over, &[], &[item], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut at_cap = new_character("at-cap");
        at_cap.equipment.parts.weapon.selected_or_register().item_id = Some("test-weapon".to_string());
        at_cap.equipment.parts.weapon.selected_or_register().base = domain::EquipmentValues {
            thrust: 200,
            ..Default::default()
        };
        assert!(repo
            .create(&at_cap, &[], &[item], &[], &[], &[], &[])
            .is_ok());
    }

    #[test]
    fn 未知の装備アビリティidは拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.selected_or_register().abilities = vec!["nope".to_string()];
        assert!(matches!(
            repo.create(&c, &[], &[], &[test_equipment_ability()], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut ok = new_character("x");
        ok.equipment.parts.weapon.selected_or_register().abilities = vec!["test-ability".to_string()];
        assert!(repo
            .create(&ok, &[], &[], &[test_equipment_ability()], &[], &[], &[])
            .is_ok());
    }

    #[test]
    fn updateで登録内容を更新できる() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(&new_character("メイン"), &[], &[], &[], &[], &[], &[])
            .unwrap();

        let mut updated = new_character("メイン改");
        updated.base_stats.stab = 310;
        updated.awakening.eternal_level = 60;
        updated.stat_sources.rune_levels.stab = 20;

        let result = repo
            .update(created.id, &updated, &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(result.id, created.id);
        assert_eq!(result.name, "メイン改");
        assert_eq!(result.base_stats.stab, 310);
        assert_eq!(result.awakening.eternal_level, 60);
        assert_eq!(result.stat_sources.rune_levels.stab, 20);

        assert_eq!(repo.get(created.id).unwrap(), result);
    }

    #[test]
    fn ソウルリンクlvはstat_sourcesのjsonで往復する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut character = new_character("リンク済み");
        character.stat_sources.soul_link = domain::SoulLinkStatus {
            thrust_level: 1,
            slash_level: 2,
            magic_attack_level: 3,
            magic_defense_level: 4,
            critical_damage_level: 7,
            final_damage_level: 3,
            weapon_enhance_level: 11,
            armor_enhance_level: 12,
        };
        let created = repo
            .create(&character, &[], &[], &[], &[], &[], &[])
            .unwrap();
        assert_eq!(
            repo.get(created.id).unwrap().stat_sources.soul_link,
            character.stat_sources.soul_link
        );
    }

    #[test]
    fn updateは存在しないidをエラーにする() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let c = new_character("メイン");
        assert!(matches!(
            repo.update(999, &c, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::CharacterNotFound(999))
        ));
    }

    #[test]
    fn updateも310の範囲バリデーションが効く() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(&new_character("メイン"), &[], &[], &[], &[], &[], &[])
            .unwrap();
        let mut invalid = new_character("メイン");
        invalid.base_stats.stab = 311;
        assert!(matches!(
            repo.update(created.id, &invalid, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn createはクラウン_ルーン_聖物の範囲超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();

        let mut over_crown = new_character("x");
        over_crown.stat_sources.crown = Crown {
            stab: Crown::SELECTED_MAX_VALUE + Crown::STEP,
            selected_stat: Some(domain::StatKind::Stab),
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&over_crown, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut over_rune = new_character("x");
        over_rune.stat_sources.rune_levels = RuneLevels {
            hack: domain::RUNE_LEVEL_MAX + 1,
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&over_rune, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));

        let mut over_relic = new_character("x");
        over_relic.stat_sources.sacred_relic = SacredRelic {
            int: domain::SACRED_RELIC_STAGE_MAX + 1,
            ..Default::default()
        };
        assert!(matches!(
            repo.create(&over_relic, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn updateはクラウン_ルーン_聖物の範囲超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(&new_character("メイン"), &[], &[], &[], &[], &[], &[])
            .unwrap();

        let mut invalid = new_character("メイン");
        invalid.stat_sources.crown = Crown {
            stab: Crown::SELECTED_MAX_VALUE + Crown::STEP,
            selected_stat: Some(domain::StatKind::Stab),
            ..Default::default()
        };
        assert!(matches!(
            repo.update(created.id, &invalid, &[], &[], &[], &[], &[], &[]),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn createは排他枠が重複するバフ選択を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let choices = BuffSelection {
            choices: vec![
                buff_choice("illumination_drink"),
                buff_choice("charge_potion"),
            ],
        };
        assert!(matches!(
            repo.create_buff_set("x", &choices, &test_catalog()),
            Err(StorageError::InvalidValue(_))
        ));
    }

    #[test]
    fn updateは排他枠が重複するバフ選択を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let created = repo
            .create(
                &new_character("メイン"),
                &test_catalog(),
                &[],
                &[],
                &[],
                &[],
                &[],
            )
            .unwrap();

        let invalid = BuffSelection {
            choices: vec![
                buff_choice("illumination_drink"),
                buff_choice("charge_potion"),
            ],
        };
        assert!(matches!(
            repo.update_buff_set(created.id, "invalid", &invalid, &test_catalog()),
            Err(StorageError::InvalidValue(_))
        ));
    }

    /// エラー帯から該当部位へ飛べるように、装備の検証エラーは「どこの話か」を持つ。
    #[test]
    fn 装備アビリティ本体値の孤児は部位とアビリティidを指す() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.weapon.selected_or_register().item_id = Some("test-weapon".to_string());
        c.equipment.parts.weapon.selected_or_register().base = domain::EquipmentValues {
            thrust: 100,
            slash: 100,
            ..Default::default()
        };
        // abilities には無いのに本体値だけ残っている(検証を足す前に保存された旧データ)。
        c.equipment.parts.weapon.selected_or_register().ability_values = vec![domain::EquipmentAbilityAdditional {
            ability_id: "test-ability".to_string(),
            kind: domain::EquipmentAbilityAdditionalKind::Thrust,
            value: 1,
        }];
        let part_id = c.equipment.parts.weapon.registered[0].id;
        let Err(StorageError::InvalidValue(error)) = repo.create(
            &c,
            &[],
            &[test_equipment_item()],
            &[test_equipment_ability()],
            &[],
            &[],
            &[],
        ) else {
            panic!("孤児の本体値は拒否されるはず");
        };
        assert_eq!(
            error.location,
            Some(domain::ValidationLocation {
                slot: domain::PartSlot::Weapon,
                part_id,
                ability_id: Some("test-ability".to_string()),
                random_option_id: None,
            })
        );
    }

    #[test]
    fn ランダムオプションの検証エラーは部位とオプションidを指す() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let mut c = new_character("x");
        c.equipment.parts.shield.selected_or_register().random_options = vec![domain::RandomOptionSlot {
            option_id: "nope".to_string(),
            rank: domain::RandomOptionRank::Rare,
            value: None,
        }];
        let part_id = c.equipment.parts.shield.registered[0].id;
        let Err(StorageError::InvalidValue(error)) = repo.create(&c, &[], &[], &[], &[], &[], &[])
        else {
            panic!("未知のランダムオプションは拒否されるはず");
        };
        assert_eq!(
            error.location,
            Some(domain::ValidationLocation {
                slot: domain::PartSlot::Shield,
                part_id,
                ability_id: None,
                random_option_id: Some("nope".to_string()),
            })
        );
    }
}
