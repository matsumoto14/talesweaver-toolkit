//! 「追加機能の解除」(情報パネルのバージョン表記 7 連打)で Cloudflare R2 から取得した装備
//! (†テネブリス 38 件)を、実行中のプロセスへ合流させる。配布物・git には含めない
//! (docs/adr/009-public-release.md)。生成元・スキーマは
//! `tools/gamedata/import_client_db.py` の `write_tenebris_json` 参照。
//!
//! Tauri 側(`install_downloaded_equipment` 呼び出し元)がローカル保存とアプリ起動時の
//! 再インストールを担い、ここは「妥当な JSON なら合流させる」だけを持つ。

use std::sync::RwLock;

use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct DownloadedFile {
    items: Vec<DownloadedItem>,
}

#[derive(Deserialize)]
struct DownloadedItem {
    id: String,
    slot: PartSlot,
    name: String,
    values_min: EquipmentValues,
    values_max: EquipmentValues,
    enchant_total_caps: EquipmentValues,
    weapon_class: Option<WeaponClass>,
    /// 腕装備(サブアーム)の分類。client DB 由来の行はページ文字列を持たないので JSON で受け取る
    wrist_type: Option<WristType>,
    usable_by: Option<Vec<String>>,
    source: DownloadedSource,
}

#[derive(Deserialize)]
struct DownloadedSource {
    page: String,
    retrieved_on: String,
    note: String,
}

/// インストール済みの装備。再インストールは追記ではなく丸ごと置き換え(重複させない)。
static DOWNLOADED: RwLock<Vec<DownloadedEquipment>> = RwLock::new(Vec::new());

#[derive(Clone, Copy)]
pub(super) struct DownloadedEquipment {
    pub(super) item: WikiEquipmentItem,
    pub(super) wrist_type: Option<WristType>,
}

pub(super) fn downloaded_equipment_catalog() -> Vec<DownloadedEquipment> {
    DOWNLOADED
        .read()
        .expect("DOWNLOADED ロックが失敗")
        .clone()
}

/// `client_wrist_type` と同じ役目(build 時に id で引く)。
pub(super) fn downloaded_wrist_type(id: &str) -> Option<WristType> {
    downloaded_equipment_catalog()
        .iter()
        .find(|entry| entry.item.id == id)
        .and_then(|entry| entry.wrist_type)
}

/// JSON をパースして検証し、装備カタログへ合流させる(次回 `equipment_catalog()` /
/// `find_equipment_item()` から見える)。妥当性チェックに失敗したら既存のインストール状態は
/// 変えずに `Err` を返す。
///
/// 呼び出し頻度はアプリ起動時に 1 回・解除操作のたびに 1 回程度なので、文字列は
/// `Box::leak` して `WikiEquipmentItem` が要求する `'static` ライフタイムに合わせる。
pub fn install_downloaded_equipment(json: &str) -> Result<usize, String> {
    let parsed: DownloadedFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let items: Vec<DownloadedEquipment> = parsed
        .items
        .into_iter()
        .map(|item| {
            let wrist_type = item.wrist_type;
            let usable_by = item.usable_by.map(|list| {
                let leaked: Vec<&'static str> = list
                    .into_iter()
                    .map(|s| -> &'static str { Box::leak(s.into_boxed_str()) })
                    .collect();
                &*Box::leak(leaked.into_boxed_slice())
            });
            let item = WikiEquipmentItem {
                id: Box::leak(item.id.into_boxed_str()),
                slot: item.slot,
                name: Box::leak(item.name.into_boxed_str()),
                values_min: item.values_min,
                values_max: item.values_max,
                growth_cap: None,
                enchant_total_caps: item.enchant_total_caps,
                weapon_class: item.weapon_class,
                enhance_type: None,
                damage_effects: &[],
                no_ability_or_random_option_slots: false,
                survival_effects: &[],
                recommended_dependency: None,
                damage_dependency: None,
                usable_by,
                source: Source {
                    page: Box::leak(item.source.page.into_boxed_str()),
                    retrieved_on: Box::leak(item.source.retrieved_on.into_boxed_str()),
                    note: Box::leak(item.source.note.into_boxed_str()),
                },
            };
            DownloadedEquipment { item, wrist_type }
        })
        .collect();
    let count = items.len();
    *DOWNLOADED.write().expect("DOWNLOADED ロックが失敗") = items;
    invalidate_equipment_catalog_cache();
    Ok(count)
}
