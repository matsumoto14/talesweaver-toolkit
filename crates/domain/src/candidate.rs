//! 「次に変えるなら / おすすめ強化」候補の手間タグ(表示専用。判定・計算には使わない)。
//!
//! 候補そのものの列挙は TS 側(apps/desktop/src/candidates.ts)に残る。ここは種別の型だけを
//! ドメインに置く — シエナのオーラ強化などタグの種類は今後も増えるため、TS の 3 値リテラル
//! ユニオンではなく domain の enum で一元管理する(ユーザー指示 2026-08-29)。
//! 新種別を足すときは既存の 3 つと同格の「手間の大きさ」を表す言葉にする。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateCost {
    /// 保存済みの設定を ON にする・段を上げるだけ(共通スキルのトグル等)。手間がほぼ無い
    QuickWin,
    /// 装備のエンチャントを伸ばす
    Enchant,
    /// 装備そのものを差し替える
    EquipmentUpdate,
}
