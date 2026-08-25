//! 属性(wiki: 属性システム / 装備システム/属性強化、取得 2026-08-25)。docs/damage-formula.md §8、§4 カテゴリI。
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

/// 装備の属性強化以外の属性値の供給源(ユーザー提供 2026-08-25)。
/// 供給源ごとに「どの属性に乗せているか」だけを持ち、加算値は gamedata が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ElementSources {
    /// ペット
    #[serde(default)]
    pub pet: Option<Element>,
    /// モンスターカード
    #[serde(default)]
    pub monster_card: Option<Element>,
    /// ルーンスキル
    #[serde(default)]
    pub rune: Option<Element>,
    /// 頭アビリティ
    #[serde(default)]
    pub helm_ability: Option<Element>,
    /// カフス(盾+)のアビリティ(神秘鉱の鋭い刃 等)
    #[serde(default)]
    pub cuffs_ability: Option<Element>,
}

/// 供給源 1 つ分の定義(表示名と加算値)。実データは gamedata。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ElementSourceDef {
    pub id: ElementSourceId,
    pub name: &'static str,
    pub value: i64,
}

/// 供給源の種別。`ElementSources` のどのフィールドかを指す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementSourceId {
    Pet,
    MonsterCard,
    Rune,
    HelmAbility,
    CuffsAbility,
}

impl ElementSources {
    pub fn get(&self, id: ElementSourceId) -> Option<Element> {
        match id {
            ElementSourceId::Pet => self.pet,
            ElementSourceId::MonsterCard => self.monster_card,
            ElementSourceId::Rune => self.rune,
            ElementSourceId::HelmAbility => self.helm_ability,
            ElementSourceId::CuffsAbility => self.cuffs_ability,
        }
    }

    /// 供給源の加算値を属性ごとに集計する。`defs` は gamedata のカタログ。
    pub fn values(&self, defs: &[ElementSourceDef]) -> ElementValues {
        let mut total = ElementValues::default();
        for def in defs {
            if let Some(element) = self.get(def.id) {
                *total.get_mut(element) += def.value;
            }
        }
        total
    }
}

/// 属性値の内訳(キャラ基礎 / 装備の属性強化 / 装備以外の供給源 / 合計)。画面表示用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementPreview {
    pub base: ElementValues,
    pub equipment: ElementValues,
    pub sources: ElementValues,
    /// 3 つを足して上限 255 で頭打ちにした値
    pub total: ElementValues,
}

impl ElementPreview {
    pub fn new(base: ElementValues, equipment: ElementValues, sources: ElementValues) -> Self {
        Self { base, equipment, sources, total: base.add(equipment).add(sources).clamp_to_max() }
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
    fn 供給源は選んだ属性にカタログの値を足す() {
        let defs = [
            ElementSourceDef { id: ElementSourceId::Pet, name: "ペット", value: 10 },
            ElementSourceDef { id: ElementSourceId::MonsterCard, name: "モンスターカード", value: 30 },
            ElementSourceDef { id: ElementSourceId::Rune, name: "ルーンスキル", value: 20 },
        ];
        let sources = ElementSources {
            pet: Some(Element::Water),
            monster_card: Some(Element::Water),
            rune: Some(Element::Fire),
            ..Default::default()
        };
        let values = sources.values(&defs);
        assert_eq!(values.get(Element::Water), 40);
        assert_eq!(values.get(Element::Fire), 20);
        // 未選択(None)の供給源は足さない
        assert_eq!(ElementSources::default().values(&defs), ElementValues::default());
    }

    #[test]
    fn 無属性は装備に付与できない() {
        assert!(!Element::Neutral.can_enchant_equipment());
        for element in Element::ALL.iter().filter(|e| **e != Element::Neutral) {
            assert!(element.can_enchant_equipment());
        }
    }
}
