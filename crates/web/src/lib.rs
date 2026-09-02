//! ブラウザ版の入口。画面(`apps/desktop/src`)から見える形を Tauri の `invoke` と揃える。
//!
//! 公開するのは `invoke(command, args)` の 1 本だけ。こうしておくと画面側は import 先を
//! 差し替えるだけで済み、コマンドごとのバインディングを画面に持ち込まずに済む。
//! 引数は Tauri と同じ camelCase の JS オブジェクトで来るので、コマンドごとに
//! `#[serde(rename_all = "camelCase")]` の引数 struct で受ける(名前は Tauri 側の引数名が正)。

#[cfg(test)]
mod args_check;

use commands::CommandError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
struct RetainCharacterSkillsArgs {
    skill_ids: Vec<String>,
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
struct ValidateBuffSetArgs {
    name: String,
    choices: domain::BuffSelection,
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
struct PreviewVersusArgs {
    attacker: domain::NewCharacter,
    attacker_buffs: domain::BuffSelection,
    skill_id: String,
    defender: domain::NewCharacter,
    defender_buffs: domain::BuffSelection,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListEquipmentCandidatesArgs {
    game_character_id: Option<String>,
    main_skill_id: Option<String>,
    slot: domain::PartSlot,
}

/// `part_weapon_system` / `relic_state` は編集中の部位 1 つだけを取る。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartArgs {
    part: domain::EquipmentPart,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelicStepArgs {
    part: domain::EquipmentPart,
    direction: domain::RelicDirection,
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
        "get_new_character_common_skills" => ok(commands::get_new_character_common_skills()),
        "retain_character_skills" => {
            let a: RetainCharacterSkillsArgs = args_of(command, args)?;
            ok(commands::retain_character_skills(a.skill_ids, a.game_character_id))
        }

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
        // 保存の前チェック。保存自体は TS 側(IndexedDB)が行うが、検証は domain を持つこちらで見る
        "validate_character" => {
            let a: CharacterArgs = args_of(command, args)?;
            done(commands::validate_character(a.character))
        }
        "validate_buff_set" => {
            let a: ValidateBuffSetArgs = args_of(command, args)?;
            done(commands::validate_buff_set(a.name, a.choices))
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
        "preview_versus" => {
            let a: PreviewVersusArgs = args_of(command, args)?;
            done(commands::preview_versus(
                a.attacker,
                a.attacker_buffs,
                a.skill_id,
                a.defender,
                a.defender_buffs,
            ))
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
        "list_equipment_candidates" => {
            let a: ListEquipmentCandidatesArgs = args_of(command, args)?;
            ok(commands::list_equipment_candidates(
                a.game_character_id,
                a.main_skill_id,
                a.slot,
            ))
        }
        "part_weapon_system" => {
            let a: PartArgs = args_of(command, args)?;
            ok(commands::part_weapon_system(a.part))
        }
        "list_enchant_plans" => {
            let a: CharacterArgs = args_of(command, args)?;
            ok(commands::list_enchant_plans(a.character))
        }
        "relic_state" => {
            let a: PartArgs = args_of(command, args)?;
            ok(commands::relic_state(a.part))
        }
        "relic_step" => {
            let a: RelicStepArgs = args_of(command, args)?;
            ok(commands::relic_step(a.part, a.direction))
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

        other => Err(to_error(CommandError {
            message: format!("未知のコマンド '{other}'"),
            location: None,
        })),
    }
}
