//! Tauri シェル。コマンドは storage/gamedata → domain を呼ぶ薄いアダプタ(後続コミットで追加)。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Tauri アプリの起動に失敗");
}
