//! 装備研磨(wiki は旧値なので使わない。出典はクライアント DB のアイテム説明
//! dm_00000_0425(研磨剤)/ dm_00000_0481(ワックス)、ユーザー確認 2026-09-15)。
//!
//! 消耗品で、装備 1 部位の能力値 1 つを上げる。種類は 3 つ: ピカピカ = 素の補正
//! (エンチャント除く `part.base`)の 3% 切り上げ、職人 = 5% 切り上げ、聖なる = 固定値
//! (武器・鎧 +4 / それ以外 +2)。「研磨剤」(武器・鎧)/「ワックス」(それ以外)の呼び分けは
//! 部位から決まるので型には持たず、表示ラベルだけ分ける(`PolishKind::product_label`)。
//!
//! 1 部位に同時 1 つ。レリック(段階成長の別モデル)は対象外。期限は持たない
//! (このリポジトリに期限つき値の前例が無い。`avatar_enhance.rs` と同じ扱い)。
//! 効き先は装備の**基本能力値**側(`Equipment::base_sources`)。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::equipment::{EquipmentStatKind, EquipmentValues, PartSlot};
use crate::rounding::ceil_int;

/// 研磨の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolishKind {
    /// ピカピカ研磨剤/ワックス: 素の補正(part.base)の 3% 切り上げ
    Sparkle,
    /// 職人の研磨剤/ワックス: 素の補正(part.base)の 5% 切り上げ
    Artisan,
    /// 聖なる研磨剤/ワックス: 固定値(武器・鎧 +4 / それ以外 +2)
    Holy,
}

impl PolishKind {
    pub const ALL: [PolishKind; 3] = [PolishKind::Sparkle, PolishKind::Artisan, PolishKind::Holy];

    /// 表示名。
    pub fn label(self) -> &'static str {
        match self {
            PolishKind::Sparkle => "ピカピカ",
            PolishKind::Artisan => "職人",
            PolishKind::Holy => "聖なる",
        }
    }

    /// この部位に付ける消耗品の呼び名(武器・鎧は「研磨剤」、それ以外は「ワックス」)。
    pub fn product_label(slot: PartSlot) -> &'static str {
        if slot.allows_enhance() {
            "研磨剤"
        } else {
            "ワックス"
        }
    }

    /// `base` 1 値に研磨をかけた加算量(切り上げ・固定値はこの部位の武器・鎧判定を要る)。
    fn amount(self, slot: PartSlot, base: i64) -> i64 {
        match self {
            PolishKind::Sparkle => ceil_int(base as f64 * 0.03),
            PolishKind::Artisan => ceil_int(base as f64 * 0.05),
            PolishKind::Holy => {
                if slot.allows_enhance() {
                    4
                } else {
                    2
                }
            }
        }
    }
}

/// 装備 1 部位ぶんの研磨記録。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentPolish {
    pub slot: PartSlot,
    pub kind: PolishKind,
    pub stat: EquipmentStatKind,
}

/// キャラの装備研磨一覧(1 部位に同時 1 つ)。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EquipmentPolishes {
    #[serde(default)]
    pub entries: Vec<EquipmentPolish>,
}

impl EquipmentPolishes {
    /// この部位の記録(無ければ `None`)。
    pub fn get(&self, slot: PartSlot) -> Option<&EquipmentPolish> {
        self.entries.iter().find(|e| e.slot == slot)
    }

    /// この部位に研磨をかけたときの加算量(`base` は `part.base`。記録が無ければ全 0)。
    pub fn bonus(&self, slot: PartSlot, base: EquipmentValues) -> EquipmentValues {
        let mut values = EquipmentValues::default();
        if let Some(entry) = self.get(slot) {
            *values.get_mut(entry.stat) = entry.kind.amount(slot, base.get(entry.stat));
        }
        values
    }

    pub fn validate(&self) -> Result<(), EquipmentPolishError> {
        let mut seen: Vec<PartSlot> = Vec::new();
        for entry in &self.entries {
            if matches!(entry.slot, PartSlot::RelicPendant | PartSlot::RelicBracelet) {
                return Err(EquipmentPolishError::RelicNotAllowed { slot: entry.slot });
            }
            if seen.contains(&entry.slot) {
                return Err(EquipmentPolishError::DuplicateSlot { slot: entry.slot });
            }
            seen.push(entry.slot);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum EquipmentPolishError {
    #[error("{slot:?} に研磨を複数登録しています(1 部位に 1 つまで)")]
    DuplicateSlot { slot: PartSlot },
    #[error("{slot:?} は段階成長の別モデルなので研磨の対象外です")]
    RelicNotAllowed { slot: PartSlot },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ピカピカは素の補正の3パーセント切り上げ() {
        // base 33 の 3% = 0.99 → 切り上げで 1
        assert_eq!(PolishKind::Sparkle.amount(PartSlot::Weapon, 33), 1);
    }

    #[test]
    fn 職人は素の補正の5パーセント切り上げ() {
        // base 100 の 5% = 5.0 → 5
        assert_eq!(PolishKind::Artisan.amount(PartSlot::Weapon, 100), 5);
        // base 21 の 5% = 1.05 → 切り上げで 2
        assert_eq!(PolishKind::Artisan.amount(PartSlot::Weapon, 21), 2);
    }

    #[test]
    fn 聖なるは武器と鎧が4それ以外が2() {
        assert_eq!(PolishKind::Holy.amount(PartSlot::Weapon, 999), 4);
        assert_eq!(PolishKind::Holy.amount(PartSlot::Armor, 999), 4);
        assert_eq!(PolishKind::Holy.amount(PartSlot::Shield, 999), 2);
        assert_eq!(PolishKind::Holy.amount(PartSlot::Helm, 999), 2);
    }

    #[test]
    fn bonusは対象ステだけに乗る() {
        let mut polishes = EquipmentPolishes::default();
        polishes.entries.push(EquipmentPolish {
            slot: PartSlot::Weapon,
            kind: PolishKind::Artisan,
            stat: EquipmentStatKind::Thrust,
        });
        let mut base = EquipmentValues::default();
        base.thrust = 100;
        base.slash = 50;
        let bonus = polishes.bonus(PartSlot::Weapon, base);
        assert_eq!(bonus.thrust, 5);
        assert_eq!(bonus.slash, 0);
        assert_eq!(polishes.bonus(PartSlot::Armor, base), EquipmentValues::default());
    }

    #[test]
    fn 同じ部位の重複は拒否する() {
        let mut polishes = EquipmentPolishes::default();
        polishes.entries.push(EquipmentPolish {
            slot: PartSlot::Weapon,
            kind: PolishKind::Sparkle,
            stat: EquipmentStatKind::Thrust,
        });
        polishes.entries.push(EquipmentPolish {
            slot: PartSlot::Weapon,
            kind: PolishKind::Holy,
            stat: EquipmentStatKind::Slash,
        });
        assert!(matches!(
            polishes.validate(),
            Err(EquipmentPolishError::DuplicateSlot { .. })
        ));
    }

    #[test]
    fn レリックは拒否する() {
        let mut polishes = EquipmentPolishes::default();
        polishes.entries.push(EquipmentPolish {
            slot: PartSlot::RelicPendant,
            kind: PolishKind::Sparkle,
            stat: EquipmentStatKind::Thrust,
        });
        assert!(matches!(
            polishes.validate(),
            Err(EquipmentPolishError::RelicNotAllowed { .. })
        ));
        assert!(EquipmentPolishes::default().validate().is_ok());
    }
}
