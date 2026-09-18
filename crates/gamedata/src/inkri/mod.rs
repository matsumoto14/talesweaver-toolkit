//! ビアヌのインクリ対象装備カタログ(部位・ビアヌ費用・エタインクリ費用)。
//!
//! 出典: クライアント DB `db/dm_*_0*.csv`(`EquippableItemTemplate` で合成回数列 `c51_995dd080` を
//! 持つ装備)。表示名・アイコンは `tw_assets/item_icons/items.csv`
//! (クライアント DB の `Name` 列は一部が文字化けしているため使わない)。
//!
//! ビアヌのインクリ費用(SEED)は wiki「装備システム/インクリ」の表(2026-09-12 更新、
//! ユーザー確認 2026-09-17)をそのまま転記。エタインクリ費用(SEED、呪文書 1 枚は別)は同ページ
//! 「エタインクリ費用」節(セイクリッド 296,680,000 / 改セイクリッド 313,680,000)。系列ごとの収録判定・費用の割り当ては
//! `tools/gamedata/import_inkri_targets.py` 冒頭のコメント参照。ロード/加護/祝福/王室の
//! 費用は資料が無いため常に `None`(画面は「?」を出す)。
//!
//! 再生成: `python tools/gamedata/import_inkri_targets.py`

use domain::PartSlot;

use crate::Source;

pub const INKRI_TARGET_SOURCE: Source = Source {
    page: "装備システム/インクリ(wiki) / クライアント DB db/dm_*_0*.csv・dm_00000_0419.csv",
    retrieved_on: "2026-09-17",
    note: "ビアヌ費用は wiki 表。合成回数上限はクライアント DB c51_995dd080。\
        収録系列・費用の割り当ては import_inkri_targets.py 参照",
};

/// ビアヌのインクリ対象装備 1 件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct InkriTarget {
    /// クライアント DB の ItemId(アイコンファイル名・保存データ突き合わせのキー)
    pub client_item_id: u32,
    pub name: &'static str,
    /// 表示グルーピング用の系列名(例: "アクィルス"、"地神")
    pub series: &'static str,
    pub part: PartSlot,
    /// ビアヌのインクリ費用(SEED)。wiki に資料が無い装備は `None`。
    pub bianu_seed_cost: Option<i64>,
    /// エタインクリ費用(SEED、呪文書 1 枚は別)。エタレベル装備(セイクリッド系)以外は `None` = 使えない。
    pub eta_seed_cost: Option<i64>,
}

#[path = "generated.rs"]
mod generated;

/// 収録済みの対象装備一覧。
pub fn inkri_targets() -> &'static [InkriTarget] {
    generated::INKRI_TARGETS
}

/// クライアント DB の ItemId から 1 件引く。
pub fn find_inkri_target(client_item_id: u32) -> Option<&'static InkriTarget> {
    inkri_targets()
        .iter()
        .find(|target| target.client_item_id == client_item_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 収録件数がゼロではない() {
        assert!(!inkri_targets().is_empty());
    }

    #[test]
    fn idの重複が無い() {
        let mut ids: Vec<u32> = inkri_targets().iter().map(|t| t.client_item_id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "client_item_id が重複しています");
    }

    #[test]
    fn 収録しない系列が混ざっていない() {
        // デックストシューズ・アベルシューズ・サクヤの雪駄・真ブリニクル武器は外す(ユーザー判断 2026-09-18)
        for target in inkri_targets() {
            for dropped in ["デックスト", "アベル", "サクヤ", "ブリニクル"] {
                assert!(
                    !target.series.contains(dropped) && !target.name.contains(dropped),
                    "{} (ItemId {}) は収録対象外の系列です",
                    target.name,
                    target.client_item_id
                );
            }
        }
    }

    #[test]
    fn エタインクリ費用はセイクリッド系にだけある() {
        for target in inkri_targets() {
            let eta = target.series == "セイクリッド" || target.series == "改・セイクリッド";
            assert_eq!(
                target.eta_seed_cost.is_some(),
                eta,
                "{} (ItemId {})",
                target.name,
                target.client_item_id
            );
        }
    }

    #[test]
    fn find_inkri_targetは存在するidを引ける() {
        let first = inkri_targets()[0];
        assert_eq!(
            find_inkri_target(first.client_item_id).map(|t| t.client_item_id),
            Some(first.client_item_id)
        );
        assert!(find_inkri_target(0).is_none());
    }
}
