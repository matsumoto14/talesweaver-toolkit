//! 敵(攻撃対象)。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Enemy {
    pub id: String,
    pub name: String,
    /// 防御力(wiki: カテゴリC)
    pub defense: i64,
    /// 被害減少(wiki: カテゴリM、固定値)。与ダメージに加算するので減少は負値で持つ(旧リポ af63 の符号反転)
    pub damage_reduction: i64,
    /// カット率A(wiki: カテゴリV1)。乗数そのもの(1.0 = 減少なし)。旧リポ af64
    pub cut_rate_a: f64,
    /// 敵の属性値(wiki 狩り場情報一覧「敵属性値」= 属性差ボーナス カテゴリI の起点)。
    /// 攻撃スキルの属性値がこれを上回った分だけ与ダメージが増える(+80 で上限 1.5 倍)
    pub element_threshold: i64,
}
