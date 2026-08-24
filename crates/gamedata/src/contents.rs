//! コンテンツカタログ(エリア → コンテンツ)。ホームの到達一覧・入場条件判定に使う。
//!
//! 敵データは `enemies.rs` の 3 体を流用する。目安ダメージ(1 ヒット最大の実用ライン)と
//! 入場条件は `[仮]`: wiki「狩り場情報一覧」の取込後に置換する(docs/claude/decisions.md)。

use domain::content::{Content, ContentArea, ContentRequirement};

use crate::Source;

pub const CONTENTS_SOURCE: Source = Source {
    page: "(仮)暫定値",
    retrieved_on: "2026-08-24",
    note: "敵は enemies.rs の 3 体を流用。目安ダメージ・入場条件は wiki 狩り場情報一覧の取込後に置換する",
};

/// エリアごとのコンテンツ一覧。表示順 = この配列の順。
pub fn content_areas() -> Vec<ContentArea> {
    vec![
        ContentArea {
            id: "fields".into(),
            name: "狩り場".into(),
            contents: vec![Content {
                id: "tutatur".into(),
                name: "トゥタトゥール".into(),
                enemy_id: "tutatur".into(),
                need_per_hit: 2_000,
                requirements: vec![],
                team_note: None,
            }],
        },
        ContentArea {
            id: "bosses".into(),
            name: "ボス".into(),
            contents: vec![
                Content {
                    id: "brothers_forge".into(),
                    name: "兄弟の鍛冶場".into(),
                    enemy_id: "brothers_forge".into(),
                    need_per_hit: 3_000,
                    requirements: vec![ContentRequirement::EquipmentThrustSlash(600)],
                    team_note: None,
                },
                Content {
                    id: "odin_rank".into(),
                    name: "オーディン(ランク)".into(),
                    enemy_id: "odin_rank".into(),
                    need_per_hit: 9_000,
                    requirements: vec![
                        ContentRequirement::EquipmentThrustSlash(1_200),
                        ContentRequirement::EternalLevel(41),
                    ],
                    team_note: Some("ランキング戦".into()),
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enemies::find_enemy;

    #[test]
    fn コンテンツidは一意() {
        let mut ids: Vec<String> = content_areas()
            .iter()
            .flat_map(|a| a.contents.iter().map(|c| c.id.clone()))
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);
        assert_eq!(total, 3);
    }

    #[test]
    fn 全コンテンツのenemy_idが敵カタログに存在する() {
        for area in content_areas() {
            for content in &area.contents {
                assert!(
                    find_enemy(&content.enemy_id).is_some(),
                    "enemy_id '{}' が enemies.rs に無い",
                    content.enemy_id
                );
            }
        }
    }
}
