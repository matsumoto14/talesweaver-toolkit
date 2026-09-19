//! キャラごとの基礎属性値(wiki: 属性システム「スキル属性」の各キャラ表で属性名に付く括弧の数値)。
//!
//! 装備の属性強化(部位ごとに 1 属性・0〜9)とは別枠で、キャラ本来の属性値。合計が敵の属性値を
//! 上回った分だけカテゴリI(属性差ボーナス)が乗る。
//!
//! wiki の表に括弧が無い属性は 0 として扱う(スキルは載っているが数値が無い行がある。
//! 例: ボリスの土属性「大地系」・黒属性「黒魔法系」)。
//! ロアミニ・ノクターン・リーチェ・イェフネンは表そのものが無い(実装が新しい)ため全属性 0 `[仮]`。

use domain::{ElementSourceDef, ElementSourceId, ElementValues};

use crate::Source;

pub const ELEMENT_BASE_SOURCE: Source = Source {
    page: "属性システム",
    retrieved_on: "2026-08-25",
    note: "各キャラ表の属性名に付く括弧の数値。括弧が無い属性は 0。\
           ロアミニ/ノクターン/リーチェ/イェフネンは wiki に表が無く全属性 0 `[仮]`",
};

/// 装備の外から来る属性値の供給源(ユーザー提供 2026-08-25)。
///
/// wiki には一覧が無く、ユーザーが実プレイから提供した値。もとは 5 件 110 だったが、
/// 頭アビリティ(月石)+20 とカフス(盾+)アビリティ+30 は**装備に登録した実物から読む**ように
/// 変えた(2026-09-19。段が N/R/L/G で +5〜+20 と変わり、追加枠の属性値も実測に幅がある)。
/// 残る 3 件 60 + 装備の属性強化(部位ごとに 0〜9、盾+(カフス)とレリックは対象外 = 10 部位で
/// 最大 90)+ 装備アビリティ(月石 20 + 別属性枠 20 + カフス 30)で 200 に届き、敵属性値
/// 120〜125 に対して属性差 +80(カテゴリI の上限 +50%)を満たす。
pub const ELEMENT_SOURCE_CATALOG_SOURCE: Source = Source {
    page: "ユーザー提供(実プレイ)",
    retrieved_on: "2026-08-25",
    note: "ペット+10 / モンスターカード+30 / ルーンスキル+20。頭アビリティ(月石)と           カフス(盾+)アビリティは装備アビリティから読む。wiki 属性システムには供給源の一覧が無い",
};

const ELEMENT_SOURCES: &[ElementSourceDef] = &[
    ElementSourceDef {
        id: ElementSourceId::Pet,
        name: "ペット",
        value: 10,
    },
    ElementSourceDef {
        id: ElementSourceId::MonsterCard,
        name: "モンスターカード",
        value: 30,
    },
    ElementSourceDef {
        id: ElementSourceId::Rune,
        name: "ルーンスキル",
        value: 20,
    },
];

/// 属性値の供給源カタログ(装備の外から来るぶん)。
pub fn element_source_catalog() -> &'static [ElementSourceDef] {
    ELEMENT_SOURCES
}

/// (character_id, 火, 水, 風, 土, 雷, 白, 黒, 無)
#[rustfmt::skip]
const ELEMENT_BASES: &[(&str, i64, i64, i64, i64, i64, i64, i64, i64)] = &[
    ("lucian",    0,  0, 10,  0, 10,  0,  0, 0),
    ("boris",     0, 10,  0,  0,  5,  0,  0, 0),
    ("ispin",    10,  0,  0,  0,  0,  0,  0, 0),
    ("maximin",   0,  0, 10,  0, 10,  0,  0, 0),
    ("tichiel",  10, 10,  5,  5, 10, 10,  0, 5),
    ("nayatorei",10,  0,  0,  0,  0,  0,  5, 5),
    ("siberin",  10,  0,  0,  0,  0,  0,  0, 0),
    ("mira",      0,  0, 10,  5,  0,  0,  0, 0),
    ("joshua",    0,  5,  0,  0,  0,  5, 10, 0),
    ("chloe",    10, 10,  5,  5, 10,  0, 10, 5),
    ("ranjie",    0, 10,  0,  0,  0,  0,  0, 0),
    ("isaac",     0,  0,  0,  0, 10,  0,  0, 5),
    ("anais",    10, 10, 10, 10, 10, 10, 10, 5),
    ("isolet",    0,  0, 10,  0,  0, 10,  0, 0),
    ("benya",     5,  5,  5, 10,  5,  5, 10, 5),
];

/// キャラの基礎属性値。wiki に表が無いキャラは全属性 0。
pub fn element_base(character_id: &str) -> ElementValues {
    ELEMENT_BASES
        .iter()
        .find(|(id, ..)| *id == character_id)
        .map(
            |&(_, fire, water, wind, earth, thunder, white, black, neutral)| ElementValues {
                fire,
                water,
                wind,
                earth,
                thunder,
                white,
                black,
                neutral,
            },
        )
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::Element;

    #[test]
    fn 基礎属性値はwikiの括弧の数値() {
        let boris = element_base("boris");
        assert_eq!(boris.get(Element::Water), 10);
        assert_eq!(boris.get(Element::Thunder), 5);
        // 大地系・黒魔法系はスキルが載っているが数値が無いので 0
        assert_eq!(boris.get(Element::Earth), 0);
        assert_eq!(boris.get(Element::Black), 0);
    }

    #[test]
    fn wikiに表が無いキャラは全属性0() {
        assert_eq!(element_base("roamini"), ElementValues::default());
        assert_eq!(element_base("nope"), ElementValues::default());
    }

    #[test]
    fn 装備外の供給源は3件で60() {
        let defs = element_source_catalog();
        assert_eq!(defs.len(), 3);
        let sources: i64 = defs.iter().map(|d| d.value).sum();
        assert_eq!(sources, 60);
        // 残りは装備から読む: 属性強化 10 部位 × 9 = 90、装備アビリティ(G-月石 20 +
        // 別属性枠 20 + カフス 30)= 70。合わせて 220 で、実プレイの 200 を満たす
        assert_eq!(sources + 10 * domain::EQUIPMENT_ELEMENT_VALUE_MAX + 70, 220);
    }

    #[test]
    fn 収録キャラはすべてプレイアブル一覧にある() {
        for (id, ..) in ELEMENT_BASES {
            assert!(
                crate::find_character(id).is_some(),
                "{id} がキャラ一覧に無い"
            );
        }
    }
}
