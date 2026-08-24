//! 属性(wiki: 属性システム / 装備システム/属性強化、取得 2026-08-25)。docs/damage-formula.md §4 カテゴリI。
//!
//! 属性値は「攻撃側(スキルの属性に対応するキャラの属性値)」と「防御側(敵の属性値)」の**差**でのみ
//! 効く。差が +1 増えるごとに +0.625%、+80 で上限 +50%。マイナス側の減少は無い。

use serde::{Deserialize, Serialize};

/// 属性 8 種(wiki 属性システム「火・水・風・地・雷・白・黒・無の8つ」)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    Fire,
    Water,
    Wind,
    Earth,
    Thunder,
    White,
    Black,
    /// 無属性
    Neutral,
}

impl Element {
    pub const ALL: [Element; 8] = [
        Element::Fire,
        Element::Water,
        Element::Wind,
        Element::Earth,
        Element::Thunder,
        Element::White,
        Element::Black,
        Element::Neutral,
    ];

    /// 装備に付与できるか(wiki 装備システム/属性強化「1属性のみ装着可能(火、水、風、土、雷、白、黒)」。
    /// 無属性は付与対象に無い)。
    pub fn can_enchant_equipment(self) -> bool {
        self != Element::Neutral
    }
}

/// キャラの属性値の上限(wiki 属性システム「属性値の上限は255です」)。
pub const ELEMENT_VALUE_MAX: i64 = 255;
/// 装備 1 部位に付与できる属性値の上限
/// (wiki 装備システム/属性強化。旧方式の宝石は 7、属性強化石で 8、費用表に 0→9 がある)。
pub const EQUIPMENT_ELEMENT_VALUE_MAX: i64 = 9;

/// 属性ごとの値。キャラの基礎属性値・装備の付与分・その合計に同じ型を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ElementValues {
    #[serde(default)]
    pub fire: i64,
    #[serde(default)]
    pub water: i64,
    #[serde(default)]
    pub wind: i64,
    #[serde(default)]
    pub earth: i64,
    #[serde(default)]
    pub thunder: i64,
    #[serde(default)]
    pub white: i64,
    #[serde(default)]
    pub black: i64,
    #[serde(default)]
    pub neutral: i64,
}

impl ElementValues {
    pub fn get(&self, element: Element) -> i64 {
        match element {
            Element::Fire => self.fire,
            Element::Water => self.water,
            Element::Wind => self.wind,
            Element::Earth => self.earth,
            Element::Thunder => self.thunder,
            Element::White => self.white,
            Element::Black => self.black,
            Element::Neutral => self.neutral,
        }
    }

    pub fn get_mut(&mut self, element: Element) -> &mut i64 {
        match element {
            Element::Fire => &mut self.fire,
            Element::Water => &mut self.water,
            Element::Wind => &mut self.wind,
            Element::Earth => &mut self.earth,
            Element::Thunder => &mut self.thunder,
            Element::White => &mut self.white,
            Element::Black => &mut self.black,
            Element::Neutral => &mut self.neutral,
        }
    }

    pub fn add(self, other: ElementValues) -> ElementValues {
        let mut total = self;
        for element in Element::ALL {
            *total.get_mut(element) += other.get(element);
        }
        total
    }

    /// 属性値の上限(255)で頭打ちにする。
    pub fn clamp_to_max(self) -> ElementValues {
        let mut clamped = self;
        for element in Element::ALL {
            let value = clamped.get_mut(element);
            *value = (*value).min(ELEMENT_VALUE_MAX);
        }
        clamped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 属性値は属性ごとに足して255で頭打ち() {
        let base = ElementValues { water: 10, thunder: 5, ..Default::default() };
        let equipment = ElementValues { water: 90, fire: 300, ..Default::default() };
        let total = base.add(equipment).clamp_to_max();
        assert_eq!(total.get(Element::Water), 100);
        assert_eq!(total.get(Element::Thunder), 5);
        assert_eq!(total.get(Element::Fire), ELEMENT_VALUE_MAX);
        assert_eq!(total.get(Element::Neutral), 0);
    }

    #[test]
    fn 無属性は装備に付与できない() {
        assert!(!Element::Neutral.can_enchant_equipment());
        for element in Element::ALL.iter().filter(|e| **e != Element::Neutral) {
            assert!(element.can_enchant_equipment());
        }
    }
}
