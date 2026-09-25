//! 称号(`title_catalog()`)と装備カタログ(`equipment_catalog()`)のうち、静的データだけで
//! 答えられる項目(9 値・上限)を JSON 配列で stdout に出す。
//!
//! ADR-021(出典の格付け: 公式お知らせ > アプリの静的データ > wiki)・
//! scratchpad/design/stage3-spec.md の B(5〜8)を実装する。値は gamedata の構造体から機械で作り、
//! 手では書かない。
//!
//!     cargo run -p gamedata --bin export_app_data > tools/gamedata/wiki/app_data.json

use std::collections::BTreeMap;

use gamedata::{enemies, equipment_catalog, title_catalog, EquipmentItem};
use domain::{AddedDamageCondition, EquipmentValues, GameRegion, TitleDef};
use serde::Serialize;

#[derive(Serialize)]
struct AppDataItem {
    subject: String,
    kind: &'static str,
    section: String,
    cells: BTreeMap<String, String>,
    source_title: String,
}

/// wiki の装備補正 9 値の列名(stage3-spec 項 10 の対応表と揃える)。0 の列は省く。
fn equipment_values_cells(v: &EquipmentValues) -> BTreeMap<String, String> {
    let mut cells = BTreeMap::new();
    let pairs: [(&str, i64); 9] = [
        ("突き", v.thrust),
        ("斬り", v.slash),
        ("物防", v.physical_defense),
        ("魔攻", v.magic_attack),
        ("魔防", v.magic_defense),
        ("命中", v.accuracy),
        ("回避", v.evasion),
        ("敏捷", v.agility),
        ("Cri補正", v.critical),
    ];
    for (name, value) in pairs {
        if value != 0 {
            cells.insert(name.to_string(), value.to_string());
        }
    }
    cells
}

fn fmt_percent(v: f64) -> String {
    let sign = if v >= 0.0 { "+" } else { "" };
    format!("{sign}{v}%")
}

/// `client.rs` / `sacred_kr.rs` はクライアント展開データ由来、`generated.rs` は wiki 抽出ぶん
/// (`EQUIPMENT_CATALOG_SOURCE`: `Link/装備Item とリンク先の部位別 Item ページ`)。
/// `Source::page` の書式(`client DB dm_…` で始まるか)で判別する(二重管理しない)。
fn equipment_source_title(item: &EquipmentItem) -> &'static str {
    if item.source.page.starts_with("client DB") {
        "アプリのデータ(クライアント DB 由来)"
    } else {
        "アプリのデータ(wiki 由来)"
    }
}

fn equipment_item_row(item: &EquipmentItem) -> Option<AppDataItem> {
    let mut cells = equipment_values_cells(&item.values_max);
    if let Some(cap) = item.growth_cap {
        cells.insert("上限".to_string(), cap.to_string());
    }
    if cells.is_empty() {
        return None;
    }
    Some(AppDataItem {
        subject: item.name.to_string(),
        kind: "equipment",
        section: format!("装備({})", item.slot.label()),
        cells,
        source_title: equipment_source_title(item).to_string(),
    })
}

/// 条件付き追加ダメージの条件を人が読む日本語に(内部の enum 名を出さない)。
fn condition_label(condition: &AddedDamageCondition) -> String {
    match condition {
        AddedDamageCondition::Region(region) => format!("{}で", region_label(*region)),
        AddedDamageCondition::Enemy(id) => {
            // 敵カタログに無い id(deep_apostle = 深淵の使徒。titles.rs の note 由来)はここで名前を持つ
            let name = enemies()
                .into_iter()
                .find(|e| e.id == *id)
                .map(|e| e.name)
                .unwrap_or_else(|| match *id { "deep_apostle" => "深淵の使徒".to_string(), other => other.to_string() });
            format!("{}に", name)
        }
    }
}

fn region_label(region: GameRegion) -> &'static str {
    match region {
        GameRegion::LostIsland => "喪失の島",
        GameRegion::ShinchouNest => "神鳥の塒",
        GameRegion::ArklonUnderground => "アークロン地下要塞",
        GameRegion::Praba => "プラバ",
    }
}

/// 称号の 9 値はクライアントの値(titles.rs 冒頭のコメント参照)。攻撃ダメージ・追加ダメージ・
/// 条件付き追加ダメージは wiki 抽出ぶんだが、1 件 1 出典で持つ器なので主要な 9 値の出典を代表させる。
fn title_row(t: &TitleDef) -> Option<AppDataItem> {
    let mut cells = equipment_values_cells(&t.values);
    if t.attack_damage_percent != 0.0 {
        cells.insert("攻撃ダメージ".to_string(), fmt_percent(t.attack_damage_percent));
    }
    if t.added_damage_percent != 0.0 {
        cells.insert("追加ダメージ".to_string(), fmt_percent(t.added_damage_percent));
    }
    if let Some(cad) = &t.conditional_added_damage {
        cells.insert(
            "条件付き追加ダメージ".to_string(),
            format!("{}({})", fmt_percent(cad.percent), condition_label(&cad.condition)),
        );
    }
    if cells.is_empty() {
        return None;
    }
    Some(AppDataItem {
        subject: t.name.to_string(),
        kind: "title",
        section: "称号".to_string(),
        cells,
        source_title: "アプリのデータ(クライアント DB 由来)".to_string(),
    })
}

fn main() {
    let mut out: Vec<AppDataItem> = Vec::new();
    out.extend(title_catalog().iter().filter_map(title_row));
    out.extend(equipment_catalog().iter().filter_map(equipment_item_row));

    let json = serde_json::to_string_pretty(&out).expect("app_data を JSON にできませんでした");
    println!("{json}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_parses_as_json_array_and_is_not_empty() {
        let mut out: Vec<AppDataItem> = Vec::new();
        out.extend(title_catalog().iter().filter_map(title_row));
        out.extend(equipment_catalog().iter().filter_map(equipment_item_row));
        let json = serde_json::to_string(&out).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert!(!parsed.as_array().unwrap().is_empty());
    }

    #[test]
    fn equipment_item_without_any_values_is_skipped() {
        let empty = EquipmentValues::default();
        assert!(equipment_values_cells(&empty).is_empty());
    }
}
