//! アバター強化(wiki: 計算式まとめ「強化能力値: エンチャント能力値、テシスコア、
//! アバター強化剤を含む期間制能力値、シエナのオーラ」)。
//!
//! アバターは兜・頭・体・脚・エフェクトの 5 部位。各部位に強化剤で装備補正 9 値
//! (`EquipmentStatKind`)の固定値を付与でき、**同じ部位に複数の能力値を重ねられる**
//! (1 部位に突き +12 と命中 +12 の両方など)。同じ能力値を 5 部位すべてに付けてもよい。
//! 現行の強化剤は +10 / +12(30 日)。旧品に +1 / +3 もある。移動速度の強化剤は対象外。
//! 出典: クライアント DB のアバター強化剤アイテム(dm_00000_0408/0425/0481)、
//! ユーザー確認 2026-09-15。
//!
//! 期限(期間制)はモデルに持たない。このリポジトリに期限つき値の前例が無く、
//! 値だけを保持する(docs/adr/005-siena-thesis-core.md 2026-09-15 追記)。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::equipment::{EquipmentStatKind, EquipmentValues};

/// アバター強化剤 1 個あたりの値の上限(wiki: 現行アバター強化剤は +10 / +12)。
pub const AVATAR_ENHANCE_MAX: i64 = 12;

/// アバターの部位(wiki: アバター強化剤の対象「兜・頭・体・脚・エフェクト」)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarPart {
    Helm,
    Head,
    Body,
    Legs,
    Effect,
}

impl AvatarPart {
    pub const ALL: [AvatarPart; 5] = [
        AvatarPart::Helm,
        AvatarPart::Head,
        AvatarPart::Body,
        AvatarPart::Legs,
        AvatarPart::Effect,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AvatarPart::Helm => "兜",
            AvatarPart::Head => "頭",
            AvatarPart::Body => "体",
            AvatarPart::Legs => "脚",
            AvatarPart::Effect => "エフェクト",
        }
    }
}

/// アバター5部位の強化値一式。部位ごとに装備補正 9 値を重ねられる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AvatarEnhancements {
    #[serde(default)]
    pub helm: EquipmentValues,
    #[serde(default)]
    pub head: EquipmentValues,
    #[serde(default)]
    pub body: EquipmentValues,
    #[serde(default)]
    pub legs: EquipmentValues,
    #[serde(default)]
    pub effect: EquipmentValues,
}

impl AvatarEnhancements {
    pub fn get(&self, part: AvatarPart) -> &EquipmentValues {
        match part {
            AvatarPart::Helm => &self.helm,
            AvatarPart::Head => &self.head,
            AvatarPart::Body => &self.body,
            AvatarPart::Legs => &self.legs,
            AvatarPart::Effect => &self.effect,
        }
    }

    pub fn get_mut(&mut self, part: AvatarPart) -> &mut EquipmentValues {
        match part {
            AvatarPart::Helm => &mut self.helm,
            AvatarPart::Head => &mut self.head,
            AvatarPart::Body => &mut self.body,
            AvatarPart::Legs => &mut self.legs,
            AvatarPart::Effect => &mut self.effect,
        }
    }

    /// 5 部位の合計(強化能力値へ合流させる値)。
    pub fn equipment_values(&self) -> EquipmentValues {
        self.helm
            .add(self.head)
            .add(self.body)
            .add(self.legs)
            .add(self.effect)
    }

    /// 全部位・全値が 0 か(強化能力値の供給源に出す必要が無いか)。
    pub fn is_neutral(&self) -> bool {
        *self == AvatarEnhancements::default()
    }

    pub fn validate(&self) -> Result<(), AvatarEnhanceError> {
        for part in AvatarPart::ALL {
            let values = self.get(part);
            for kind in EquipmentStatKind::ALL {
                let value = values.get(kind);
                if !(0..=AVATAR_ENHANCE_MAX).contains(&value) {
                    return Err(AvatarEnhanceError::ValueOutOfRange {
                        part,
                        kind,
                        value,
                        max: AVATAR_ENHANCE_MAX,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum AvatarEnhanceError {
    #[error("アバター強化({part:?} の {kind:?})は 0〜{max} の範囲で指定してください(指定値 {value})")]
    ValueOutOfRange {
        part: AvatarPart,
        kind: EquipmentStatKind,
        value: i64,
        max: i64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 部位ごとに重ねた値が合計に入る() {
        let mut avatar = AvatarEnhancements::default();
        avatar.get_mut(AvatarPart::Helm).thrust = 12;
        avatar.get_mut(AvatarPart::Helm).accuracy = 12;
        avatar.get_mut(AvatarPart::Head).thrust = 12;

        let totals = avatar.equipment_values();
        assert_eq!(totals.thrust, 24);
        assert_eq!(totals.accuracy, 12);
        assert!(!avatar.is_neutral());
    }

    #[test]
    fn 中立なら合計はゼロで供給源に出ない() {
        let avatar = AvatarEnhancements::default();
        assert!(avatar.is_neutral());
        assert_eq!(avatar.equipment_values(), EquipmentValues::default());
    }

    #[test]
    fn 値域違反は拒否する() {
        let mut avatar = AvatarEnhancements::default();
        avatar.get_mut(AvatarPart::Body).slash = AVATAR_ENHANCE_MAX + 1;
        assert!(matches!(
            avatar.validate(),
            Err(AvatarEnhanceError::ValueOutOfRange { .. })
        ));

        let mut avatar = AvatarEnhancements::default();
        avatar.get_mut(AvatarPart::Legs).critical = -1;
        assert!(matches!(
            avatar.validate(),
            Err(AvatarEnhanceError::ValueOutOfRange { .. })
        ));

        assert!(AvatarEnhancements::default().validate().is_ok());
    }
}
