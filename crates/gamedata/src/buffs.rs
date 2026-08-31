//! 常用バフカタログ(wiki: ステータス#jc16a054)。**消費アイテム・イベントのバフ専用**で、
//! キャラのパッシブ・自己バフ・味方バフは `character_skills.rs`(効き先がステだけではなく、
//! 同じスキルが中ディレイや攻撃ダメージにも効くため)。
//!
//! バフは個別にコードで分岐せず「カテゴリ(層)+ 数値 + 重複枠」を持つデータとして持つ
//! (CLAUDE.md 原則)。型定義は domain 側(`domain::stat_sources`)、実データはここ。

use domain::{
    BuffDefinition, BuffOrigin, BuffPurpose, BuffTarget, BuffValue, DamageCategory, SkillEffect,
    StatLayer,
};

use crate::Source;

/// バフカタログの出典。
pub const BUFF_CATALOG_SOURCE: Source = Source {
    page: "ステータス#jc16a054",
    retrieved_on: "2026-08-31",
    note: "常用バフ44件。計算カテゴリと数値は本ページ、個別アイテムと入手手段は Item/消耗品/ステータス補助・クラブを参照",
};

/// 常用バフのカタログ。
pub fn buff_catalog() -> Vec<BuffDefinition> {
    vec![
        BuffDefinition {
            id: "illumination_drink",
            name: "イルミネーション祭りのドリンク",
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.30),
            exclusive_slots: vec!["percent_slot_1", "percent_slot_2", "x1_group_a"],
            source_url: WIKI_URL,
            note: "①+②",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "snowman_potion",
            name: "ユキダルマン族の特製ポーション",
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.30),
            exclusive_slots: vec!["percent_slot_1", "percent_slot_2", "x1_group_a", "x1_group_b"],
            source_url: WIKI_URL,
            note: "①+②",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
        },
        BuffDefinition {
            id: "charge_potion",
            name: "充填の秘薬",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
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
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec!["percent_slot_2", "x1_group_a"],
            source_url: WIKI_URL,
            note: "②",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "guardian_potion",
            name: "守護者のためのポーション",
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::Minigame,
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
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
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
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
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
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
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
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
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
            id: "growth_support_potion",
            name: "成長支援ポーション(イソレット/ノクターン/アナイス)",
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.50),
            exclusive_slots: vec![
                "percent_slot_1",
                "percent_slot_2",
                "blessing",
                "blessing_potion_a",
                "blessing_potion_b",
                "x1_group_a",
                "x1_group_b",
            ],
            source_url: WIKI_URL,
            note: "①+②。退魔師の恵み・祝福のポーション・バフスクロールと重複不可。[X1] +20% も持つ",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
        },
        BuffDefinition {
            id: "blessing_potion",
            name: "祝福のポーション / ＜ログ・ホライズン＞のカレーライス",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(20.0),
            exclusive_slots: vec!["blessing_potion_a", "blessing_potion_b"],
            source_url: WIKI_URL,
            note: "イザベルの秘法(固定)・特選秘薬(固定)の両方と同時使用不可",
            default_value: None,
            damage_effects: &[],
        },
        BuffDefinition {
            id: "demon_slayer_blessing",
            name: "退魔師の恵み / 祝福の聖水 / 河童神の涙",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierA,
            value: BuffValue::Fixed(1.1),
            exclusive_slots: vec!["blessing"],
            source_url: WIKI_URL,
            note: "イザベルの秘法(比率)と同枠",
            default_value: None,
            damage_effects: &[],
        },
        BuffDefinition {
            id: "karill_buff_scroll",
            name: "カリル家のバフスクロール",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::Fixed(0.10),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "分類不明(割合)。バフスクロールと重複可能",
            default_value: None,
            damage_effects: &[],
        },
        BuffDefinition {
            id: "event_buff",
            name: "イベントバフ",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Event,
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
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::UserInput { min: 0.0, max: 34.0 },
            exclusive_slots: vec!["trust_potion"],
            source_url: WIKI_URL,
            // 上限はユーザーの実測(2026-09-01)。wiki は +33 と書いているが実際に +34 が出る
            note: "最大+34、人により異なる。信頼の薬と排他。無印の信頼の薬は最大+28",
            default_value: Some(34.0),
            damage_effects: &[],
        },
        BuffDefinition {
            id: "fixed_increase",
            name: "固定増加系(メバルのフライ等)",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            // wiki に明記の上限が無いため、実用上の安全域として暫定 999(docs/claude/decisions.md 参照)。
            value: BuffValue::UserInput { min: 0.0, max: 999.0 },
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "手入力(1枠)。ウガンポの葉っぱ・トールのしっぽ焼き(+50)・四味仙霊芝草(+30)等も本項で表す",
            default_value: Some(50.0),
            damage_effects: &[],
        },
        BuffDefinition {
            id: "club_effect",
            name: "クラブ効果",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Club,
            // エフェクトは 1 ステにつき 1 つで、クラブレベルの枠数まで併用できる
            // (Lv7:1種 → Lv15:2 → Lv20:3 → Lv25:4 → Lv30:5 → Lv31:6 → Lv36:7種)。
            // 上昇項目が同じものは併用できないので、実質の上限はステの数 = 7。
            target: BuffTarget::UserSelectedMulti,
            layer: StatLayer::Fixed,
            value: BuffValue::UserInput { min: 1.0, max: 7.0 },
            exclusive_slots: vec![],
            source_url: CLUB_WIKI_URL,
            note: "ステごとに +1〜7。使える数はクラブレベル次第。+20 はクラブSエフェクト",
            default_value: Some(7.0),
            damage_effects: &[],
        },
        BuffDefinition {
            id: "club_s_effect",
            name: "クラブSエフェクト(攻撃力+5%)",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Club,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["club_s_effect"],
            source_url: CLUB_WIKI_URL,
            note: "7日。課金箱。ステータス版のクラブSエフェクトとは同時使用不可",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::FinalDamageRate, percent: 5.0 }],
        },
        BuffDefinition {
            id: "club_s_effect_single_stat",
            name: "クラブSエフェクト(単一ステ+20)",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Club,
            target: BuffTarget::UserSelected,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(20.0),
            exclusive_slots: vec!["club_s_effect"],
            source_url: CLUB_WIKI_URL,
            note: "STAB/HACK/INT/DEF/MR/DEX/AGIから選択。7日。課金箱",
            default_value: None,
            damage_effects: &[],
        },
        BuffDefinition {
            id: "club_s_effect_all_stats",
            name: "クラブSエフェクト(ALL)",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Club,
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Choice(vec![5.0, 10.0, 15.0, 20.0]),
            exclusive_slots: vec!["club_s_effect"],
            source_url: CLUB_WIKI_URL,
            note: "ALL+5/+10/+15/+20。期間と入手方法は商品ごとに異なる",
            default_value: None,
            damage_effects: &[],
        },
        BuffDefinition {
            id: "tales_weaver_energy",
            name: "テイルズウィーバーのエネルギー",
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::Skill,
            target: BuffTarget::AllStats,
            layer: StatLayer::MultiplierA,
            value: BuffValue::Fixed(1.1),
            exclusive_slots: vec![],
            source_url: WIKI_URL,
            note: "",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        // --- ダメージにだけ効くバフ(ステは上げない。wiki ステータスの [X1]〜[X6] / [L])---
        BuffDefinition {
            id: "isabel_damage",
            name: "イザベルの秘法(ダメージ)",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_c"],
            source_url: WIKI_URL,
            note: "[X1] 上限 +50%。クリティカル率 +5% は未収録",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "isabel_special_damage",
            name: "イザベルの特選秘薬(ダメージ)",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_d"],
            source_url: WIKI_URL,
            note: "[X1]。橙色の薬・16周年ピクニックのり巻きお弁当・ラッキービースト-I型 車掌タイプ・マルクスのクナイ・デスガイニーの葉・宝玉<赤眼の魔王>・朧塚商店街のクリームパン・<シルバーソード>のクリームシチュー等と同枠",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "moonlight_potion",
            name: "月光・怪力のポーション",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_b"],
            source_url: WIKI_URL,
            note: "[X1]。月光のポーションと怪力のポーションは同値同枠なのでまとめている",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "silver_sword_stew",
            name: "<シルバーソード>のクリームシチュー",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_d"],
            source_url: WIKI_URL,
            note: "[X1]。イザベルの特選秘薬(ダメージ)等と同枠(【D】)",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
        },
        BuffDefinition {
            id: "festival_food",
            name: "おいしいフェスティバル料理",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Event,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_e"],
            source_url: WIKI_URL,
            note: "[X1]",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }],
        },
        BuffDefinition {
            id: "sakuraeda_hitokata",
            name: "桜枝のヒトカタ",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_c", "x1_group_e"],
            source_url: WIKI_URL,
            note: "[X1]。被ダメージ減少 [S1] +10% は未収録",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "oborozuka_cream_bread",
            name: "朧塚商店街のクリームパン / ロデリック商会特製マヨネーズ",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["x1_group_b", "x1_group_c", "x1_group_d"],
            source_url: WIKI_URL,
            note: "[X1]。朧塚商店街のクリームパンとロデリック商会特製マヨネーズは同値同枠なのでまとめている",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 10.0 }],
        },
        BuffDefinition {
            id: "awakening_elixir",
            name: "覚醒の秘薬",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["awakening_elixir"],
            source_url: WIKI_URL,
            note: "[X2] 上限 +30%。改・覚醒の秘薬とは重複不可",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        BuffDefinition {
            id: "improved_awakening_elixir",
            name: "改・覚醒の秘薬",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: vec!["awakening_elixir"],
            source_url: ITEM_BUFF_WIKI_URL,
            note: "[X2] 上限 +30%。覚醒の秘薬とは重複不可",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        BuffDefinition {
            id: "strength_ham",
            name: "怪力のハム",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
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
            id: "club_shop_buff_type_p",
            name: "クラブ商店バフTypeP",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Club,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: CLUB_WIKI_URL,
            note: "[X2] クラブダンジョン掲示板",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        BuffDefinition {
            id: "wednesday_attack_c_rank",
            name: "攻撃力増加(Cランク)",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Minigame,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2] 水曜日の異変(規則正しいゼリッピ)",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }],
        },
        BuffDefinition {
            id: "twin_dango",
            name: "双子のお団子 / 攻撃力・防御力10%上昇",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X2]。被ダメージ減少 [S2] +10% は未収録",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 10.0 }],
        },
        BuffDefinition {
            id: "ancient_ganapoly_mana",
            name: "古代ガナポリーマナの破片",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Minigame,
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
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Event,
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
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Event,
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
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::SoulLink,
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
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Rune,
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
            purposes: &[BuffPurpose::Stats, BuffPurpose::Damage],
            origin: BuffOrigin::BattleState,
            // ステには効かないので対象・層は使わない(`RecordOnly` で加算されない)
            target: BuffTarget::AllStats,
            layer: StatLayer::PercentOfBase,
            value: BuffValue::RecordOnly,
            exclusive_slots: Vec::new(),
            source_url: WIKI_URL,
            note: "[X3] 上限 +80%。全ステータス +30 は未収録",
            default_value: None,
            damage_effects: &[SkillEffect::Damage { category: DamageCategory::AttackDamageBasicTrigger, percent: 10.0 }],
        },
        BuffDefinition {
            id: "deep_rune_attack",
            name: "深化ルーン(攻撃)",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Rune,
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
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Item,
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
            purposes: &[BuffPurpose::Damage, BuffPurpose::Durability],
            origin: BuffOrigin::Item,
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
            id: "ancient_relic_minigame",
            name: "古代レリックの聖域ミニゲームバフ",
            purposes: &[BuffPurpose::Damage],
            origin: BuffOrigin::Minigame,
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
const ITEM_BUFF_WIKI_URL: &str = "https://talewiki.com/?cmd=read&page=Item%2F%BE%C3%CC%D7%C9%CA%2F%A5%B9%A5%C6%A1%BC%A5%BF%A5%B9%CA%E4%BD%F5";
const CLUB_WIKI_URL: &str = "https://talewiki.com/?%A5%AF%A5%E9%A5%D6#club_S_effect";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn 常用バフは44件() {
        assert_eq!(buff_catalog().len(), 44);
        assert!(!buff_catalog().iter().any(|d| d.id == "unleash"));
        assert!(!buff_catalog().iter().any(|d| d.id == "soul_link_status"));
    }

    #[test]
    fn 複数の目的に所属できる() {
        let catalog = buff_catalog();
        let snowman = catalog.iter().find(|d| d.id == "snowman_potion").unwrap();
        assert_eq!(snowman.purposes, &[BuffPurpose::Stats, BuffPurpose::Damage]);
        let mimic = catalog.iter().find(|d| d.id == "boiled_mimic").unwrap();
        assert_eq!(mimic.purposes, &[BuffPurpose::Damage, BuffPurpose::Durability]);
    }

    /// ダメージにだけ効くバフは**ステを上げない**(`RecordOnly`)。
    /// 逆に、ステと与ダメージの両方に効くバフもある(守護者のためのポーション等)。
    #[test]
    fn ダメージへの効き先を持つバフ() {
        let catalog = buff_catalog();
        for buff in &catalog {
            assert_eq!(
                buff.purposes.contains(&BuffPurpose::Damage),
                !buff.damage_effects.is_empty(),
                "{} の火力目的とダメージ効果が一致していない",
                buff.id,
            );
        }
        let with_damage: Vec<&str> = catalog
            .iter()
            .filter(|d| !d.damage_effects.is_empty())
            .map(|d| d.id)
            .collect();
        assert_eq!(with_damage.len(), 30);
        // ステと与ダメージの両方に効くもの
        for id in [
            "guardian_potion",
            "snowman_potion",
            "tales_weaver_energy",
        ] {
            let d = catalog.iter().find(|d| d.id == id).unwrap();
            assert!(!d.damage_effects.is_empty(), "{id}");
            assert!(
                !matches!(d.value, BuffValue::RecordOnly),
                "{id} はステにも効く"
            );
        }
    }

    #[test]
    fn クラブsエフェクトは効果別で同時使用できない() {
        let catalog = buff_catalog();
        let variants: Vec<_> = [
            "club_s_effect",
            "club_s_effect_single_stat",
            "club_s_effect_all_stats",
        ]
        .into_iter()
        .map(|id| catalog.iter().find(|d| d.id == id).unwrap())
        .collect();
        assert!(variants.iter().all(|d| d.exclusive_slots == vec!["club_s_effect"]));

        let attack = variants[0];
        assert_eq!(attack.purposes, &[BuffPurpose::Damage]);
        assert!(matches!(attack.value, BuffValue::RecordOnly));

        let single = variants[1];
        assert_eq!(single.purposes, &[BuffPurpose::Stats]);
        assert!(matches!(single.target, BuffTarget::UserSelected));
        assert!(matches!(single.value, BuffValue::Fixed(20.0)));

        let all = variants[2];
        assert_eq!(all.purposes, &[BuffPurpose::Stats]);
        assert!(matches!(all.target, BuffTarget::AllStats));
        assert_eq!(all.value, BuffValue::Choice(vec![5.0, 10.0, 15.0, 20.0]));
    }

    #[test]
    fn 覚醒の秘薬2種は別アイテムで同時使用できない() {
        let catalog = buff_catalog();
        let normal = catalog.iter().find(|d| d.id == "awakening_elixir").unwrap();
        let improved = catalog
            .iter()
            .find(|d| d.id == "improved_awakening_elixir")
            .unwrap();
        assert_eq!(normal.damage_effects, improved.damage_effects);
        assert_eq!(normal.exclusive_slots, improved.exclusive_slots);
        assert_eq!(normal.exclusive_slots, vec!["awakening_elixir"]);
    }

    #[test]
    fn idは重複しない() {
        let catalog = buff_catalog();
        let ids: HashSet<&str> = catalog.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), catalog.len());
        assert!(catalog.iter().all(|d| !d.purposes.is_empty()));
    }

    #[test]
    fn バフ選択サマリはカテゴリ別上限を適用する() {
        let catalog = buff_catalog();
        let buffs = domain::BuffSelection {
            choices: ["snowman_potion", "festival_food", "silver_sword_stew"]
                .into_iter()
                .map(|id| domain::BuffChoice {
                    buff_id: id.to_string(),
                    stat: None,
                    choice_index: None,
                    value: None,
                })
                .collect(),
        };
        let summary = domain::summarize_buff_selection(&buffs, &catalog).unwrap();
        let isabel = summary
            .categories
            .iter()
            .find(|row| row.category == DamageCategory::AttackDamageIsabel)
            .unwrap();
        assert!((isabel.raw - 0.60).abs() < 1e-12);
        assert!((isabel.value - 0.50).abs() < 1e-12);
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
    fn 排他枠を持つバフが存在する() {
        let catalog = buff_catalog();
        let illumination = catalog
            .iter()
            .find(|d| d.id == "illumination_drink")
            .unwrap();
        assert_eq!(
            illumination.exclusive_slots,
            vec!["percent_slot_1", "percent_slot_2", "x1_group_a"]
        );
    }

    #[test]
    fn クラブ商店バフtypepは攻撃ダメージ一般に効く() {
        let catalog = buff_catalog();
        let def = catalog
            .iter()
            .find(|d| d.id == "club_shop_buff_type_p")
            .unwrap();
        assert_eq!(
            def.damage_effects,
            &[SkillEffect::Damage { category: DamageCategory::AttackDamageGeneral, percent: 5.0 }]
        );
    }

    #[test]
    fn 成長支援ポーションは能力値割合とx1の両方に効きイザベルの秘法比率と排他() {
        let catalog = buff_catalog();
        let def = catalog
            .iter()
            .find(|d| d.id == "growth_support_potion")
            .unwrap();
        assert_eq!(def.layer, StatLayer::PercentOfBase);
        assert!(matches!(def.value, BuffValue::Fixed(v) if (v - 0.50).abs() < 1e-12));
        assert_eq!(
            def.damage_effects,
            &[SkillEffect::Damage { category: DamageCategory::AttackDamageIsabel, percent: 20.0 }]
        );
        let ratio = catalog.iter().find(|d| d.id == "isabelle_ratio").unwrap();
        let shared: Vec<_> = def
            .exclusive_slots
            .iter()
            .filter(|slot| ratio.exclusive_slots.contains(slot))
            .collect();
        assert!(!shared.is_empty(), "isabelle_ratio と排他枠を共有していない");
    }

    #[test]
    fn x1の枠がぶつかるバフは同じ排他枠を共有する() {
        let catalog = buff_catalog();
        let isabel_damage = catalog.iter().find(|d| d.id == "isabel_damage").unwrap();
        let sakuraeda = catalog
            .iter()
            .find(|d| d.id == "sakuraeda_hitokata")
            .unwrap();
        let shared: Vec<_> = isabel_damage
            .exclusive_slots
            .iter()
            .filter(|slot| sakuraeda.exclusive_slots.contains(slot))
            .collect();
        assert!(!shared.is_empty(), "isabel_damage と sakuraeda_hitokata が枠を共有していない");
    }
}
