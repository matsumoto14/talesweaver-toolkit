//! 画面に出る名前・説明文を JSON へ書き出す(docs/adr/022-i18n.md 段階 4)。
//!
//! `tools/i18n/import_names.py`(装備名の韓国語照合)と、将来の説明文翻訳の材料にする。
//! 通常のテスト実行には含めない(`--ignored` で明示実行)。出力先は `tools/i18n/out/`
//! (gitignore)。統合後・dedup 後のデータを Rust の公開関数から取るので、`.rs` を正規表現で
//! 読むより取りこぼしが無い。
//!
//! 実行: `cargo test -p gamedata -- --ignored dump_names`

use std::fs;
use std::path::PathBuf;

fn out_dir() -> PathBuf {
    // crates/gamedata/tests/ から見て ../../../tools/i18n/out
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("i18n")
        .join("out");
    fs::create_dir_all(&dir).expect("tools/i18n/out の作成に失敗");
    dir
}

fn write_json<T: serde::Serialize>(name: &str, value: &T) {
    let path = out_dir().join(format!("{name}.json"));
    let json = serde_json::to_string_pretty(value).expect("シリアライズに失敗");
    fs::write(&path, json).unwrap_or_else(|e| panic!("{path:?} への書き込みに失敗: {e}"));
}

#[test]
#[ignore]
fn dump_names() {
    write_json("equipment_catalog", &gamedata::equipment_catalog());
    write_json("equipment_abilities", &gamedata::equipment_abilities());
    write_json("skills", &gamedata::all_skills().collect::<Vec<_>>());
    write_json("masteries", &gamedata::mastery_catalog());
    write_json("titles", &gamedata::title_catalog());
    write_json("character_skills", &gamedata::character_skill_catalog());
    write_json("buffs", &gamedata::buff_catalog());
    write_json("enemies", &gamedata::enemies());
    write_json("contents", &gamedata::content_areas());
    write_json("random_options", &gamedata::random_option_catalog());
    write_json("characters", &gamedata::characters());
    write_json("inkri", &gamedata::inkri_targets());
    write_json("elements", &gamedata::element_source_catalog());
}
