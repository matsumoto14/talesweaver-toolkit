//! 常用バフカタログ(wiki: ステータス#jc16a054)。**消費アイテム・イベントのバフ専用**で、
//! キャラのパッシブ・自己バフ・味方バフは `character_skills.rs`(効き先がステだけではなく、
//! 同じスキルが中ディレイや攻撃ダメージにも効くため)。
//!
//! バフは個別にコードで分岐せず「カテゴリ(層)+ 数値 + 重複枠」を持つデータとして持つ
//! (CLAUDE.md 原則)。型定義は domain 側(`domain::stat_sources`)、実データはここ。

use domain::{BuffDefinition, BuffTarget, BuffValue, DamageCategory, SkillEffect, StatLayer};

use crate::Source;

/// バフカタログの出典。
pub const BUFF_CATALOG_SOURCE: Source = Source {
    page: "ステータス#jc16a054",
    retrieved_on: "2026-08-21",
    note: "常用バフのプリセット16件。値の符号・層は docs/claude/goals/2026-08-21-character-stat-sources.md 参照",
};

/// 常用バフのカタログ。
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
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
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
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
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
            damage_effects: &[],
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
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
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
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::FinalDamageRate, percent: 10.0 }],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
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
            damage_effects: &[],
        },
        BuffDefinition {
            id: "club_effect",
            name: "クラブ効果",
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(7.0),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "wiki ステータスの表記は +1〜7 だが上限の +7 固定で持つ(装備強化・ランダムOP と同じ                   「上書きが無ければ上限」の方針。ユーザー確定 2026-08-25)。+20 はクラブSエフェクト",
            default_value: None,
            damage_effects: &[],
        },
        BuffDefinition {
            id: "club_s_effect",
            name: "クラブSエフェクト",
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(20.0),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "+20固定。クラブ効果(+7)とは別のバフで、同時に掛けられる",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::FinalDamageRate, percent: 5.0 }],
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
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
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
            damage_effects: &[],
        },
        // --- ダメージにだけ効くバフ(ステは上げない。wiki ステータスの [X1]〜[X6] / [L])---
        BuffDefinition {
            id: "isabel_damage",
            name: "イザベルの秘法(ダメージ)",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X1] 上限 +50%。クリティカル率 +5% は未収録",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "isabel_special_damage",
            name: "イザベルの特選秘薬(ダメージ)",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X1]。橙色の薬・宝玉<赤眼の魔王> 等と同枠",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "moonlight_potion",
            name: "月光のポーション",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X1]。怪力のポーションと同枠",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "silver_sword_stew",
            name: "<シルバーソード>のクリームシチュー",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X1]",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
        },
        BuffDefinition {
            id: "festival_food",
            name: "おいしいフェスティバル料理",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X1]",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
        },
        BuffDefinition {
            id: "awakening_elixir",
            name: "覚醒の秘薬",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2] 上限 +30%。改・覚醒の秘薬も同値",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        BuffDefinition {
            id: "strength_ham",
            name: "怪力のハム",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2]",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 10.0 }],
        },
        BuffDefinition {
            id: "ancient_ganapoly_mana",
            name: "古代ガナポリーマナの破片",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2] wiki は +1〜15%。最大値で入れている",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 15.0 }],
        },
        BuffDefinition {
            id: "attendance_buff",
            name: "スペシャル出席チェックバフ",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2] 重複可能",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 10.0 }],
        },
        BuffDefinition {
            id: "daily_burning_buff",
            name: "定着支援バフ(デイリーバーニング)",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2]",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 10.0 }],
        },
        BuffDefinition {
            id: "soul_link_explore",
            name: "ソウルリンク探検(攻撃力 +5%)",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2] 重複不可",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        BuffDefinition {
            id: "berserker_rune",
            name: "狂戦士のルーン",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2] Lv×0.25%、最大 +10%。最大値で入れている",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 10.0 }],
        },
        BuffDefinition {
            id: "fever",
            name: "フィーバー",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X3] 上限 +80%",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageBasicTrigger, percent: 10.0 }],
        },
        BuffDefinition {
            id: "deep_rune_attack",
            name: "深化ルーン(攻撃)",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X3] +3 で +9%",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageBasicTrigger, percent: 9.0 }],
        },
        BuffDefinition {
            id: "plunder_bread",
            name: "略奪パン",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X6] 上限 +30%",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageJapan, percent: 20.0 }],
        },
        BuffDefinition {
            id: "boiled_mimic",
            name: "茹でミミック",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X6] 被ダメージ -30% は未収録",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageJapan, percent: 10.0 }],
        },
        BuffDefinition {
            id: "soul_link_status",
            name: "ソウルリンク(リンクステータス)",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[L] 上限 +45%。wiki は +4〜20%。最大値で入れている",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::FinalDamageRate, percent: 20.0 }],
        },
        BuffDefinition {
            id: "ancient_relic_minigame",
            name: "古代レリックの聖域ミニゲームバフ",
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[L] 上限 +45%",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::FinalDamageRate, percent: 15.0 }],
        },
    ]
}

const WIKI_URL: &str = "https://talewiki.com/?%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9#jc16a054";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn 常用バフは34件() {
        assert_eq!(buff_catalog().len(), 34);
    }

    /// ダメージにだけ効くバフは**ステを上げない**(`RecordOnly`)。
    /// 逆に、ステと与ダメージの両方に効くバフもある(守護者のためのポーション・クラブSエフェクト)。
    #[test]
    fn ダメージへの効き先を持つバフ() {
        let catalog = buff_catalog();
        let with_damage: Vec<&str> = catalog
            .iter()
            .filter(|d| !d.damage_effects.is_empty())
            .map(|d| d.id)
            .collect();
        assert_eq!(with_damage.len(), 24);
        // ステと与ダメージの両方に効くもの
        for id in ["guardian_potion", "club_s_effect", "snowman_potion", "tales_weaver_energy"] {
            let d = catalog.iter().find(|d| d.id == id).unwrap();
            assert!(!d.damage_effects.is_empty(), "{id}");
            assert!(!matches!(d.value, BuffValue::RecordOnly), "{id} はステにも効く");
        }
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
