//! ブラウザ版の入口。画面(`apps/desktop/src`)から見える形を Tauri の `invoke` と揃える。
//!
//! 公開するのは `invoke(command, args)` の 1 本だけ。こうしておくと画面側は import 先を
//! 差し替えるだけで済み、コマンドごとのバインディングを画面に持ち込まずに済む。
//! 引数は Tauri と同じ camelCase の JS オブジェクトで来るので、コマンドごとに
//! `#[serde(rename_all = "camelCase")]` の引数 struct で受ける(名前は Tauri 側の引数名が正)。

use commands::CommandError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// 保存(SQLite)が要るコマンドに返すエラー。ブラウザ版はまだ保存先を持たないので、
/// 黙って空を返さず「使えない」とはっきり言う(画面のエラー帯がそのまま出す)。
fn not_available() -> Result<JsValue, JsValue> {
    Err(to_error(CommandError {
        message: "この操作はブラウザ版ではまだ使えません".to_string(),
        location: None,
    }))
}

/// 戻り値の変換規則。Tauri は JSON を経由するので、それに合わせる:
/// - map を JS の `Map` ではなく素のオブジェクトにする(`#[serde(flatten)]` を含む型が
///   map として直列化されるため。Map で返すと画面がプロパティを読めない)
/// - `None` を `undefined` ではなく `null` にする(画面は null で判定している)
fn serializer() -> serde_wasm_bindgen::Serializer {
    serde_wasm_bindgen::Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true)
}

/// `CommandError` を画面が期待する形(message / location)のまま JS に渡す。
/// 画面の `errorMessage()` / `errorLocation()` がこの形に依存している。
fn to_error(error: CommandError) -> JsValue {
    error
        .serialize(&serializer())
        .unwrap_or_else(|e| JsValue::from_str(&e.to_string()))
}

/// 引数オブジェクトを取り出す。失敗は「引数名が食い違っている」開発時のバグなので、
/// コマンド名を添えて同じエラー形で返す(帯に出れば気づける)。
fn args_of<T: DeserializeOwned>(command: &str, args: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(args).map_err(|e| {
        to_error(CommandError {
            message: format!("コマンド '{command}' の引数が不正です: {e}"),
            location: None,
        })
    })
}

fn ok<T: serde::Serialize>(value: T) -> Result<JsValue, JsValue> {
    value.serialize(&serializer()).map_err(|e| {
        to_error(CommandError {
            message: format!("戻り値を変換できません: {e}"),
            location: None,
        })
    })
}

fn done<T: serde::Serialize>(result: commands::CommandResult<T>) -> Result<JsValue, JsValue> {
    match result {
        Ok(value) => ok(value),
        Err(error) => Err(to_error(error)),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListSkillsArgs {
    game_character_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuffsArgs {
    buffs: domain::BuffSelection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EquipmentElementValuesArgs {
    equipment: domain::Equipment,
    element: Option<domain::Element>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharacterArgs {
    character: domain::NewCharacter,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MasteriesArgs {
    masteries: domain::Masteries,
}

/// `preview_effective_stats` と `buff_target_stat_gains` はキャラの素材一式を取り、
/// 最後の 1 つ(主軸スキル / バフ id)だけが違う。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewEffectiveStatsArgs {
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: domain::BuffSelection,
    equipment: domain::Equipment,
    common_skills: domain::CommonSkills,
    awakening: domain::Awakening,
    main_skill_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuffTargetStatGainsArgs {
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: domain::BuffSelection,
    equipment: domain::Equipment,
    common_skills: domain::CommonSkills,
    awakening: domain::Awakening,
    buff_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewDefenseArgs {
    character: domain::NewCharacter,
    buffs: domain::BuffSelection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewDamageArgs {
    character: domain::NewCharacter,
    buffs: domain::BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    normal_attack_id: Option<String>,
    temporary_adjustments: Option<domain::Adjustments>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluateContentsArgs {
    character: domain::NewCharacter,
    buffs: domain::BuffSelection,
    dependency_skill_id: Option<String>,
}

/// 候補列挙(`list_upgrade_candidates` / `list_enchant_gains`)は引数が同じ。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateArgs {
    character: domain::NewCharacter,
    buffs: domain::BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
}

/// Tauri の `invoke` と同じ形。`command` で分岐して `commands` crate を呼ぶ。
#[wasm_bindgen]
pub fn invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue> {
    match command {
        // --- 静的データのカタログ(引数なし) ---
        "list_game_characters" => ok(commands::list_game_characters()),
        "list_enemies" => ok(commands::list_enemies()),
        "list_buff_catalog" => ok(commands::list_buff_catalog()),
        "list_element_sources" => ok(commands::list_element_sources()),
        "list_contents" => ok(commands::list_contents()),
        "list_equipment_catalog" => ok(commands::list_equipment_catalog()),
        "list_equipment_abilities" => ok(commands::list_equipment_abilities()),
        "list_random_options" => ok(commands::list_random_options()),
        "list_masteries" => ok(commands::list_masteries()),
        "list_siena_kinds" => ok(commands::list_siena_kinds()),
        "list_character_skills" => ok(commands::list_character_skills()),
        "list_titles" => ok(commands::list_titles()),
        "get_stat_limits" => ok(commands::get_stat_limits()),
        "get_new_character_stat_sources" => ok(commands::get_new_character_stat_sources()),

        // --- 引数を取る計算系 ---
        "list_skills" => {
            let a: ListSkillsArgs = args_of(command, args)?;
            ok(commands::list_skills(a.game_character_id))
        }
        "summarize_buff_selection" => {
            let a: BuffsArgs = args_of(command, args)?;
            done(commands::summarize_buff_selection(a.buffs))
        }
        "equipment_element_values" => {
            let a: EquipmentElementValuesArgs = args_of(command, args)?;
            ok(commands::equipment_element_values(a.equipment, a.element))
        }
        "preview_elements" => {
            let a: CharacterArgs = args_of(command, args)?;
            done(commands::preview_elements(a.character))
        }
        "resolve_character_skill_effects" => {
            let a: MasteriesArgs = args_of(command, args)?;
            ok(commands::resolve_character_skill_effects(a.masteries))
        }
        "preview_effective_stats" => {
            let a: PreviewEffectiveStatsArgs = args_of(command, args)?;
            done(commands::preview_effective_stats(
                a.base_stats,
                a.stat_sources,
                a.buffs,
                a.equipment,
                a.common_skills,
                a.awakening,
                a.main_skill_id,
            ))
        }
        "buff_target_stat_gains" => {
            let a: BuffTargetStatGainsArgs = args_of(command, args)?;
            done(commands::buff_target_stat_gains(
                a.base_stats,
                a.stat_sources,
                a.buffs,
                a.equipment,
                a.common_skills,
                a.awakening,
                a.buff_id,
            ))
        }
        "preview_defense" => {
            let a: PreviewDefenseArgs = args_of(command, args)?;
            done(commands::preview_defense(a.character, a.buffs))
        }
        "preview_damage" => {
            let a: PreviewDamageArgs = args_of(command, args)?;
            done(commands::preview_damage(
                a.character,
                a.buffs,
                a.skill_id,
                a.content_id,
                a.combo_count,
                a.combo_skill_type,
                a.normal_attack_id,
                a.temporary_adjustments,
            ))
        }
        "evaluate_contents" => {
            let a: EvaluateContentsArgs = args_of(command, args)?;
            done(commands::evaluate_contents(
                a.character,
                a.buffs,
                a.dependency_skill_id,
            ))
        }
        "list_upgrade_candidates" => {
            let a: CandidateArgs = args_of(command, args)?;
            done(commands::list_upgrade_candidates(
                a.character,
                a.buffs,
                a.skill_id,
                a.content_id,
                a.combo_count,
                a.combo_skill_type,
                a.temporary_adjustments,
            ))
        }
        "list_enchant_gains" => {
            let a: CandidateArgs = args_of(command, args)?;
            done(commands::list_enchant_gains(
                a.character,
                a.buffs,
                a.skill_id,
                a.content_id,
                a.combo_count,
                a.combo_skill_type,
                a.temporary_adjustments,
            ))
        }

        // --- 保存(SQLite)が要るコマンド。ブラウザ版にはまだ保存先が無い ---
        "get_app_info"
        | "get_startup_notice"
        | "list_characters"
        | "create_character"
        | "update_character"
        | "delete_character"
        | "list_buff_sets"
        | "create_buff_set"
        | "update_buff_set"
        | "duplicate_buff_set"
        | "delete_buff_set"
        | "set_default_buff_set"
        | "list_character_icons"
        | "set_character_icon"
        | "reset_character_icon"
        | "get_damage_snapshot"
        | "set_damage_snapshot"
        | "calculate_damage" => not_available(),

        other => Err(to_error(CommandError {
            message: format!("未知のコマンド '{other}'"),
            location: None,
        })),
    }
}
