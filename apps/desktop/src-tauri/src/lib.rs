//! Tauri シェル。コマンドは「storage/gamedata から読む → domain を呼ぶ → 結果を返す」だけの薄いアダプタ。

mod commands;

use std::fs;
use std::sync::Mutex;

use storage::CharacterRepository;
use tauri::Manager;

/// アプリ全体で共有する状態。
pub struct AppState {
    pub repo: Mutex<CharacterRepository>,
}

const DATABASE_FILE_NAME: &str = "talesweaver-toolkit.sqlite";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let repo = CharacterRepository::open(data_dir.join(DATABASE_FILE_NAME))?;
            app.manage(AppState { repo: Mutex::new(repo) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_game_characters,
            commands::list_skills,
            commands::list_enemies,
            commands::list_buff_catalog,
            commands::list_element_sources,
            commands::preview_elements,
            commands::list_contents,
            commands::list_equipment_catalog,
            commands::list_equipment_abilities,
            commands::list_random_options,
            commands::list_titles,
            commands::preview_effective_stats,
            commands::preview_defense,
            commands::get_stat_limits,
            commands::list_characters,
            commands::create_character,
            commands::update_character,
            commands::delete_character,
            commands::calculate_damage,
            commands::preview_damage,
            commands::evaluate_contents,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri アプリの起動に失敗");
}
