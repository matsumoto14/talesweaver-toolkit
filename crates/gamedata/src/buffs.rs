//! 常用バフカタログ(wiki: ステータス#jc16a054)。
//!
//! バフは個別にコードで分岐せず「カテゴリ(層)+ 数値 + 重複枠」を持つデータとして持つ
//! (CLAUDE.md 原則)。型定義は domain 側(`domain::stat_sources`)、実データはここ。

use domain::{BuffDefinition, BuffGroup, BuffTarget, BuffValue, StatKind, StatLayer};

use crate::Source;

/// バフカタログの出典。
pub const BUFF_CATALOG_SOURCE: Source = Source {
    page: "ステータス#jc16a054",
    retrieved_on: "2026-08-21",
    note: "常用バフのプリセット16件。値の符号・層は docs/claude/goals/2026-08-21-character-stat-sources.md 参照",
};

/// 常用バフの初期カタログ(16件)。
pub fn buff_catalog() -> Vec<BuffDefinition> {
    vec![
        BuffDefinition {
            id: "illumination_drink",
            name: "イルミネーション祭りのドリンク",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.30),
            exclusive_slots: vec!["percent_slot_1", "percent_slot_2"],
            source_url: WIKI_URL,
            note: "①+②",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "snowman_potion",
            name: "ユキダルマン族の特製ポーション",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.30),
            exclusive_slots: vec!["percent_slot_1", "percent_slot_2"],
            source_url: WIKI_URL,
            note: "①+②",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "charge_potion",
            name: "充填の秘薬",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.20),
            exclusive_slots: vec!["percent_slot_1"],
            source_url: WIKI_URL,
            note: "①",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "buff_concentrate",
            name: "バフ濃縮液",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec!["percent_slot_2"],
            source_url: WIKI_URL,
            note: "②",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "guardian_potion",
            name: "守護者のためのポーション",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "独立。「週五社のためのポーション」は本項の別名",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "isabelle_ratio",
            name: "イザベルの秘法(比率)",
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierA,
            value: BuffValue::Fixed(1.1),
            exclusive_slots: vec!["blessing"],
            source_url: WIKI_URL,
            note: "退魔師の恵み・祝福の聖水・河童神の涙と同枠(テイルズウィーバーのエネルギーとは別枠)",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        // 旧カタログでは isabelle_fixed の値を +100 としていたが、これは特選秘薬(固定)側の値の
        // 誤転記だった。wiki 通り秘法(固定)は +20、特選秘薬(固定)は +100 が正しい。
        BuffDefinition {
            id: "isabelle_fixed",
            name: "イザベルの秘法(固定)",
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(20.0),
            // wiki は秘法(固定)・特選秘薬(固定)を「併用可」とする(docs/claude/decisions.md 2026-08-21
            // キャラ画面v2)。どちらも「祝福のポーション」系とは排他なので、将来そのアイテムを
            // 追加するときは blessing_potion_a/blessing_potion_b の両方を持たせて両方を塞ぐ。
            exclusive_slots: vec!["blessing_potion_a"],
            source_url: WIKI_URL,
            note: "祝福のポーション・カレーライスと同枠",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "isabelle_rare_percent",
            name: "イザベルの特選秘薬(割合)",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.50),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "特別な時のみ",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "isabelle_rare_fixed",
            name: "イザベルの特選秘薬(固定)",
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(100.0),
            // isabelle_fixed とは別枠(blessing_potion_b)にし、wiki の「秘法(固定)とは併用可」を
            // 反映する。同時に選べないのは将来の「祝福のポーション」自身とだけ。
            exclusive_slots: vec!["blessing_potion_b"],
            source_url: WIKI_URL,
            note: "特別な時のみ",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "event_buff",
            name: "イベントバフ",
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Choice(vec![0.10, 0.20, 0.30, 0.50]),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "段階選択(+10%/+20%/+30%/+50%)",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "trust_potion",
            name: "改・信頼の薬",
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::UserInput { min: 0.0, max: 33.0 },
            exclusive_slots: vec!["trust_potion"],
            source_url: WIKI_URL,
            note: "最大+33、人により異なる。信頼の薬と排他",
            default_value: Some(33.0),
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "fixed_increase",
            name: "固定増加系(メバルのフライ等)",
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            // wiki に明記の上限が無いため、実用上の安全域として暫定 999(docs/claude/decisions.md 参照)。
            value: BuffValue::UserInput { min: 0.0, max: 999.0 },
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "手入力(1枠)",
            default_value: Some(50.0),
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "club_effect",
            name: "クラブ効果",
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(7.0),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "+7固定(値欄なし)",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "club_s_effect",
            name: "クラブSエフェクト",
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(20.0),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "+20固定",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "tales_weaver_energy",
            name: "テイルズウィーバーのエネルギー",
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierA,
            value: BuffValue::Fixed(1.1),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        BuffDefinition {
            id: "unleash",
            name: "アンリーシュ",
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.20),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "ON/OFFのみ、+20%固定",
            default_value: None,
            group: BuffGroup::Consumable,
        },
        // --- キャラスキル(自身/味方のステ上昇。9件、docs/claude/goals/2026-08-21-character-screen-v2.md) ---
        // source_url はキャラの wiki Skill ページ(https://talewiki.com/?Skill/<キャラ>)を英語 id で
        // 組み立てたもの。実際の PukiWiki は EUC-JP percent-encode された URL を使うため、
        // ここでの URL はクリック可能な形での検証が済んでいない(docs/damage-formula.md 取得メモ参照)。
        BuffDefinition {
            id: "benya_soul_gate",
            name: "極・ソウルゲート",
            target: BuffTarget::Stat(StatKind::Agi),
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.05),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/benya",
            note: "自身のみ",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "benya" },
        },
        BuffDefinition {
            id: "ispin_encourage",
            name: "極・エンカレッジ",
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/ispin",
            note: "味方にも(30分)",
            default_value: None,
            group: BuffGroup::AllySkill,
        },
        BuffDefinition {
            id: "roamini_ha_petit",
            name: "極・ア・プチ(マスタリー)",
            target: BuffTarget::Stat(StatKind::Int),
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/roamini",
            note: "自身のみ",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "roamini" },
        },
        BuffDefinition {
            id: "roamini_powatun",
            name: "極・パウアトゥン(マスタリー)",
            target: BuffTarget::Stats(&[StatKind::Def, StatKind::Mr]),
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/roamini",
            note: "自身のみ",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "roamini" },
        },
        // wiki 上は分類不明のため倍率Bと推定して収録(Ver8.20で固定値化されたとの記述あり)。
        BuffDefinition {
            id: "boris_silver_skull",
            name: "マスタリー【シルバースカル優勝者】",
            target: BuffTarget::Stats(&[StatKind::Hack, StatKind::Def]),
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/boris",
            note: "[仮] Ver8.20で固定値化",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "boris" },
        },
        BuffDefinition {
            id: "siberin_charm",
            name: "魅力発散",
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.01),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/siberin",
            note: "[仮] 女性キャラ同行時、味方にも",
            default_value: None,
            group: BuffGroup::AllySkill,
        },
        BuffDefinition {
            id: "joshua_elite_swordsman",
            name: "マスタリー【エリート】(剣闘士)",
            target: BuffTarget::Stats(&[StatKind::Stab, StatKind::Def]),
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/joshua",
            note: "[仮] 憑依モード時",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "joshua" },
        },
        BuffDefinition {
            id: "joshua_elite_mage",
            name: "マスタリー【エリート】(魔法師)",
            target: BuffTarget::Stats(&[StatKind::Int, StatKind::Mr]),
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/joshua",
            note: "[仮] 憑依モード時",
            default_value: None,
            group: BuffGroup::CharacterSkill { game_character_id: "joshua" },
        },
        BuffDefinition {
            id: "tichiel_magic_teacher",
            name: "魔法の先生",
            target: BuffTarget::Stat(StatKind::Int),
            layer: StatLayer::MultiplierB,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: "https://talewiki.com/?Skill/tichiel",
            note: "[仮] マキシミン/クロエ同行時、味方にも",
            default_value: None,
            group: BuffGroup::AllySkill,
        },
    ]
}

const WIKI_URL: &str = "https://talewiki.com/?%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9#jc16a054";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn 常用バフconsumableは16件() {
        let consumable = buff_catalog().into_iter().filter(|d| d.group == BuffGroup::Consumable).count();
        assert_eq!(consumable, 16);
    }

    #[test]
    fn キャラスキルは9件() {
        let skill_count =
            buff_catalog().into_iter().filter(|d| !matches!(d.group, BuffGroup::Consumable)).count();
        assert_eq!(skill_count, 9);
    }

    #[test]
    fn catalogは25件() {
        assert_eq!(buff_catalog().len(), 25);
    }

    #[test]
    fn idは重複しない() {
        let catalog = buff_catalog();
        let ids: HashSet<&str> = catalog.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), catalog.len());
    }

    #[test]
    fn イザベルの秘法_比率は1点1倍であって0点1倍ではない() {
        let catalog = buff_catalog();
        let def = catalog.iter().find(|d| d.id == "isabelle_ratio").unwrap();
        assert!(matches!(def.value, BuffValue::Fixed(v) if (v - 1.1).abs() < 1e-12));
        assert_eq!(def.layer, StatLayer::MultiplierA);
    }

    /// イザベル4行(秘法(比率)/秘法(固定)/特選秘薬(割合)/特選秘薬(固定))の値・層・排他枠を固定する。
    /// 秘法(固定)と特選秘薬(固定)は wiki 上「併用可」なので異なる排他枠(blessing_potion_a/b)を持つこと。
    #[test]
    fn イザベル4行の値_層_排他枠() {
        let catalog = buff_catalog();
        let find = |id: &str| catalog.iter().find(|d| d.id == id).unwrap();
        let fixed_value = |d: &BuffDefinition| match d.value {
            BuffValue::Fixed(v) => v,
            _ => panic!("{} は Fixed 値のはず", d.id),
        };

        let ratio = find("isabelle_ratio");
        assert!((fixed_value(ratio) - 1.1).abs() < 1e-12);
        assert_eq!(ratio.layer, StatLayer::MultiplierA);
        assert_eq!(ratio.exclusive_slots, vec!["blessing"]);

        let fixed = find("isabelle_fixed");
        assert!((fixed_value(fixed) - 20.0).abs() < 1e-12);
        assert_eq!(fixed.layer, StatLayer::Fixed);
        assert_eq!(fixed.exclusive_slots, vec!["blessing_potion_a"]);

        let rare_percent = find("isabelle_rare_percent");
        assert!((fixed_value(rare_percent) - 0.50).abs() < 1e-12);
        assert_eq!(rare_percent.layer, StatLayer::PercentOfBase);
        assert!(rare_percent.exclusive_slots.is_empty());

        let rare_fixed = find("isabelle_rare_fixed");
        assert!((fixed_value(rare_fixed) - 100.0).abs() < 1e-12);
        assert_eq!(rare_fixed.layer, StatLayer::Fixed);
        assert_eq!(rare_fixed.exclusive_slots, vec!["blessing_potion_b"]);

        // 秘法(固定)と特選秘薬(固定)は排他枠が異なる(=併用可能)こと自体を明示的に確認する。
        assert_ne!(fixed.exclusive_slots, rare_fixed.exclusive_slots);
    }

    #[test]
    fn アンリーシュは倍率bの加算2割() {
        let catalog = buff_catalog();
        let def = catalog.iter().find(|d| d.id == "unleash").unwrap();
        assert!(matches!(def.value, BuffValue::Fixed(v) if (v - 0.20).abs() < 1e-12));
        assert_eq!(def.layer, StatLayer::MultiplierB);
    }

    #[test]
    fn 排他枠を持つバフが存在する() {
        let catalog = buff_catalog();
        let illumination = catalog.iter().find(|d| d.id == "illumination_drink").unwrap();
        assert_eq!(illumination.exclusive_slots, vec!["percent_slot_1", "percent_slot_2"]);
    }
}
