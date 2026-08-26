//! 称号(wiki: 称号システム。取得 2026-08-25)。
//!
//! **装備枠 1 つで、表示中の 1 件だけが効く**(所持ぶんの累積ではない。ユーザー確定 2026-08-25)。
//! 効き先は**装備の基本能力値への加算**で、wiki の表の列(突き / 斬り / 物防 / 魔攻 / 魔防 /
//! 命中 / 回避 / 敏捷 / Cri)は装備補正 9 値とまったく同じ形なので `EquipmentValues` をそのまま器に使う。
//!
//! 称号にはエンチャント・装備強化・属性強化・シエナのオーラ・ランダムオプションが無いので、
//! `PartSlot` を 13 個目に増やさず `Equipment::title` として独立に持つ。
//!
//! グループボーナス(「N 個完成で +α」)は所持状況の入力が要るのでスコープ外(goal S15)。

use serde::{Deserialize, Serialize};

use crate::equipment::EquipmentValues;

/// 称号の区分(wiki のページ分け)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleKind {
    Normal,
    Special,
    /// wiki「称号/event」。課金箱シリーズと名誉の証。**無条件のダメージ増加を持つのはここだけ**
    Event,
}

/// 称号定義(gamedata がカタログを持つ。`EquipmentAbilityDef` と同じ依存方向)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TitleDef {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: TitleKind,
    /// wiki の見出し(グループボーナスの単位。ボーナス自体は未実装)
    pub group: &'static str,
    /// 習得 Lv。wiki が `-` の行は `None`
    pub level: Option<u16>,
    /// 装備の基本能力値への加算
    pub values: EquipmentValues,
    /// 無条件の「ダメージ n% 増加」(wiki: ステータス `#z4747f51` の
    /// **[X3] 攻撃ダメージ(基本発動)(上限 +80%)** に称号の「ダメージ増加」が載っている)。
    /// 課金箱シリーズ(明鏡止水〜緋馬の怪火)と一部の名誉の証が持つ。単位は %
    pub attack_damage_percent: f64,
    /// 入手方法・備考。**条件付き**の追加効果(特定マップで追加ダメージ +20% など)は
    /// 発動条件を持っていないので計算に入れず、ここに残すだけ
    pub note: &'static str,
}

/// 称号の値域・カタログ整合性違反。
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum TitleError {
    #[error("未知の称号 '{id}' です")]
    Unknown { id: String },
}

/// 選択中の称号の補正値。カタログに無い id は `None`(保存時に `storage` が弾いている)。
pub fn title_values(title: Option<&str>, titles: &[TitleDef]) -> EquipmentValues {
    find(title, titles).map_or(EquipmentValues::default(), |t| t.values)
}

/// 選択中の称号の「ダメージ n% 増加」を Σ% の小数表現で返す(カテゴリX へ入る)。
pub fn title_attack_damage_rate(title: Option<&str>, titles: &[TitleDef]) -> f64 {
    find(title, titles).map_or(0.0, |t| t.attack_damage_percent / 100.0)
}

fn find<'a>(title: Option<&str>, titles: &'a [TitleDef]) -> Option<&'a TitleDef> {
    let id = title?;
    titles.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> Vec<TitleDef> {
        vec![TitleDef {
            id: "eclipse",
            name: "エクリプス",
            kind: TitleKind::Special,
            group: "喪失の島",
            level: None,
            values: EquipmentValues {
                thrust: 40,
                slash: 40,
                physical_defense: 40,
                magic_attack: 40,
                magic_defense: 40,
                accuracy: 40,
                critical: 40,
                evasion: 40,
                agility: 40,
            },
            attack_damage_percent: 0.0,
            note: "",
        }]
    }

    #[test]
    fn 未選択は中立値() {
        assert_eq!(title_values(None, &defs()), EquipmentValues::default());
    }

    #[test]
    fn 選択した称号の補正値を返す() {
        assert_eq!(title_values(Some("eclipse"), &defs()).thrust, 40);
    }

    #[test]
    fn カタログに無いidは中立値() {
        assert_eq!(title_values(Some("nope"), &defs()), EquipmentValues::default());
    }
}
