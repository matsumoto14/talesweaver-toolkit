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
//! アバター本体(補正付きアバター = アイテム名末尾が「Ａ」)も装備補正 9 値を持ち、
//! 5 部位すべてを揃えると 5 セット効果が乗る(`AvatarCorrections`)。強化剤とは別枠だが
//! 合流先は同じ強化能力値。
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

/// 補正付きアバター(アイテム名の末尾に「Ａ」が付く)1 点が装備補正 9 値すべてに足す値。
/// 出典: ゲーム内アイテム説明「†スウィートスターの足あとＡ - 脚」(突き〜敏捷度がすべて 1)、
/// ユーザー確認 2026-09-21。
pub const AVATAR_CORRECTION_PER_PART: i64 = 1;

/// 補正付きアバターを 5 部位すべて揃えたときのセット効果。装備補正 9 値すべてに足す。
/// 出典: ゲーム内「5 セット効果」表示(9 値すべて +10、ほかに移動速度 +10)、
/// wiki「移動速度」(アバター5点セット +10)。移動速度はこのツールでは扱わない。
pub const AVATAR_SET_BONUS: i64 = 10;

/// 補正付きアバターをどの部位に着けているか。値は部位によらず一定なので ON/OFF だけを持つ。
/// 5 部位すべてが ON のときだけセット効果(`AVATAR_SET_BONUS`)が乗る。
/// アバター強化剤(`AvatarEnhancements`)とは別物で、合流先は同じ強化能力値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AvatarCorrections {
    #[serde(default)]
    pub helm: bool,
    #[serde(default)]
    pub head: bool,
    #[serde(default)]
    pub body: bool,
    #[serde(default)]
    pub legs: bool,
    #[serde(default)]
    pub effect: bool,
}

impl AvatarCorrections {
    pub fn get(&self, part: AvatarPart) -> bool {
        match part {
            AvatarPart::Helm => self.helm,
            AvatarPart::Head => self.head,
            AvatarPart::Body => self.body,
            AvatarPart::Legs => self.legs,
            AvatarPart::Effect => self.effect,
        }
    }

    pub fn get_mut(&mut self, part: AvatarPart) -> &mut bool {
        match part {
            AvatarPart::Helm => &mut self.helm,
            AvatarPart::Head => &mut self.head,
            AvatarPart::Body => &mut self.body,
            AvatarPart::Legs => &mut self.legs,
            AvatarPart::Effect => &mut self.effect,
        }
    }

    /// 補正付きアバターを着けている部位の数(0〜5)。
    pub fn equipped_count(&self) -> usize {
        AvatarPart::ALL.into_iter().filter(|p| self.get(*p)).count()
    }

    /// 5 部位すべてが補正付きか(セット効果が乗るか)。
    pub fn is_set(&self) -> bool {
        self.equipped_count() == AvatarPart::ALL.len()
    }

    /// アバター本体の補正(点数 × `AVATAR_CORRECTION_PER_PART`)。セット効果は含まない。
    pub fn item_values(&self) -> EquipmentValues {
        uniform(self.equipped_count() as i64 * AVATAR_CORRECTION_PER_PART)
    }

    /// 5 点セット効果。揃っていなければ中立値。
    pub fn set_bonus_values(&self) -> EquipmentValues {
        uniform(if self.is_set() { AVATAR_SET_BONUS } else { 0 })
    }

    pub fn is_neutral(&self) -> bool {
        *self == AvatarCorrections::default()
    }
}

/// 装備補正 9 値すべてに同じ値を入れた `EquipmentValues`。
fn uniform(value: i64) -> EquipmentValues {
    let mut values = EquipmentValues::default();
    for kind in EquipmentStatKind::ALL {
        *values.get_mut(kind) = value;
    }
    values
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

    #[test]
    fn 補正付きアバターは点数ぶんと5点セット効果を足す() {
        let mut c = AvatarCorrections::default();
        assert!(c.is_neutral());
        assert_eq!(c.item_values(), EquipmentValues::default());
        assert_eq!(c.set_bonus_values(), EquipmentValues::default());

        *c.get_mut(AvatarPart::Helm) = true;
        *c.get_mut(AvatarPart::Head) = true;
        assert_eq!(c.equipped_count(), 2);
        assert!(!c.is_set());
        assert_eq!(c.item_values().thrust, 2);
        assert_eq!(c.item_values().agility, 2);
        assert_eq!(c.set_bonus_values(), EquipmentValues::default());

        for part in AvatarPart::ALL {
            *c.get_mut(part) = true;
        }
        assert!(c.is_set());
        for kind in EquipmentStatKind::ALL {
            assert_eq!(c.item_values().get(kind), 5);
            assert_eq!(c.set_bonus_values().get(kind), 10);
        }
    }
}
