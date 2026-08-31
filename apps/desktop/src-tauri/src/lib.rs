//! Tauri シェル。コマンドは「storage/gamedata から読む → domain を呼ぶ → 結果を返す」だけの薄いアダプタ。

mod commands;

use std::fs;
use std::sync::Mutex;

use storage::{CharacterRepository, StartupNotice};
use tauri::Manager;

/// アプリ全体で共有する状態。
pub struct AppState {
    pub repo: Mutex<CharacterRepository>,
    /// 起動時にバックアップ復元などが起きたときだけ入る。フロントがエラー帯に出す。
    pub startup_notice: Option<StartupNotice>,
}

/// 情報パネルに出すアプリ情報。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// 登録キャラの保存先。「データは端末内だけ」を裏づけるために出す。
    pub database_path: String,
}

/// 登録キャラの保存先ファイル名。情報パネルにも同じ値を出すので、ここだけに置く。
pub const DATABASE_FILE_NAME: &str = "tw-context.sqlite";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let database_path = data_dir.join(DATABASE_FILE_NAME);
            // 開く前にマイグレーション前の状態を残し、開けなければ復元する(起動不能にしない)。
            let outcome = storage::open_with_backup(&database_path, APP_VERSION)?;
            app.manage(AppState {
                repo: Mutex::new(outcome.repo),
                startup_notice: outcome.notice,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_startup_notice,
            commands::list_game_characters,
            commands::list_skills,
            commands::list_enemies,
            commands::list_buff_catalog,
            commands::summarize_buff_selection,
            commands::list_buff_sets,
            commands::create_buff_set,
            commands::update_buff_set,
            commands::duplicate_buff_set,
            commands::delete_buff_set,
            commands::set_default_buff_set,
            commands::list_element_sources,
            commands::equipment_element_values,
            commands::preview_elements,
            commands::list_contents,
            commands::list_equipment_catalog,
            commands::list_equipment_abilities,
            commands::list_random_options,
            commands::list_siena_kinds,
            commands::list_masteries,
            commands::list_titles,
            commands::list_character_skills,
            commands::resolve_character_skill_effects,
            commands::preview_effective_stats,
            commands::buff_target_stat_gains,
            commands::preview_defense,
            commands::get_stat_limits,
            commands::get_new_character_stat_sources,
            commands::list_characters,
            commands::create_character,
            commands::update_character,
            commands::delete_character,
            commands::list_character_icons,
            commands::set_character_icon,
            commands::reset_character_icon,
            commands::get_damage_snapshot,
            commands::set_damage_snapshot,
            commands::calculate_damage,
            commands::preview_damage,
            commands::evaluate_contents,
            commands::list_upgrade_candidates,
            commands::list_enchant_gains,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri アプリの起動に失敗");
}
