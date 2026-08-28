//! 称号のカタログ。
//!
//! 出典: wiki「称号/normal」「称号/special」(取得 2026-08-25)。
//!
//! **主要称号のみ収録**(ユーザー決定 2026-08-25。全 565 件を入れる必要はない)。収録基準は 2 つ:
//!
//! 1. normal / special の**補正値 9 種の合計が 15 以上**(72 件)。称号は 1 枠しか装備できないので、
//!    それ未満は選択肢として意味を持たない
//! 2. event の**無条件「ダメージ n% 増加」を持つもの**(48 件。ユーザー指定 2026-08-26)。
//!    補正値が 0 でも入れる(「専門家 - ダメージ」)。この効果が実質的に称号を選ぶ理由になっている
//!
//! event でダメージ増加を持たないものは未収録(合計 15 以上で 200 件超あるが、
//! normal / special の同水準と入れ替わるだけで選択の役に立たない)。
//! 「名誉の証(Supporter)」は wiki の 1 行が a〜d の 4 種をまとめており、しかも中身が
//! 装備補正ではなく**ステ加算**(STAB/HACK/INT/MR +60)なので、この器では表せず未収録。
//!
//! wiki の表の列は 名称 / 習得Lv / 突き / 斬り / 物防 / 魔攻 / 魔防 / 命中 / 回避 / 敏捷 / Cri で、
//! 装備補正 9 値と同じ顔ぶれ(並びだけ違う)。 の並びに直して持つ。
//!
//! 備考欄の条件付き割合追加ダメージは、対象地域または敵とともに構造化して持つ。
//! グループボーナス(「N 個完成で +α」)は所持状況の入力が要るのでスコープ外。

use domain::content::GameRegion;
use domain::{
    AddedDamageCondition, ConditionalAddedDamage, EquipmentValues, TitleDef, TitleKind,
};

use crate::Source;

/// 称号カタログの出典。
pub const TITLE_SOURCE: Source = Source {
    page: "称号/normal, 称号/special, 称号/event",
    retrieved_on: "2026-08-26",
    note: "normal/special は補正値 9 種の合計 15 以上(72 件)、event は無条件のダメージ増加を持つもの(48 件)。条件付き割合追加ダメージは地域・敵条件で計算。グループボーナスは未実装",
};

/// wiki の列順ではなく  の並び:
/// 突き / 斬り / 物防 / 魔攻 / 魔防 / 命中 / Cri / 回避 / 敏捷。
#[rustfmt::skip]
#[allow(clippy::too_many_arguments)]
const fn v(
    thrust: i64, slash: i64, physical_defense: i64, magic_attack: i64, magic_defense: i64,
    accuracy: i64, critical: i64, evasion: i64, agility: i64,
) -> EquipmentValues {
    EquipmentValues {
        thrust, slash, physical_defense, magic_attack, magic_defense,
        accuracy, critical, evasion, agility,
    }
}

use TitleKind::{Event, Normal, Special};

/// ダメージ増加を持たない称号(normal / special のほぼ全部)。
const fn t(
    id: &'static str,
    name: &'static str,
    kind: TitleKind,
    group: &'static str,
    level: Option<u16>,
    values: EquipmentValues,
    note: &'static str,
) -> TitleDef {
    TitleDef {
        id, name, kind, group, level, values, attack_damage_percent: 0.0,
        conditional_added_damage: None, note,
    }
}

/// 無条件の「ダメージ n% 増加」を持つ称号(wiki: ステータスの [X3] 攻撃ダメージ(基本発動))。
#[allow(clippy::too_many_arguments)]
const fn td(
    id: &'static str,
    name: &'static str,
    kind: TitleKind,
    group: &'static str,
    level: Option<u16>,
    values: EquipmentValues,
    attack_damage_percent: f64,
    note: &'static str,
) -> TitleDef {
    TitleDef {
        id, name, kind, group, level, values, attack_damage_percent,
        conditional_added_damage: None, note,
    }
}

/// **ダメージ増加 → 与ダメージに効く 1 値の大きさ → その 4 値の合計 → 9 値の合計**の順。
/// UI はこの順でそのまま出す。
///
/// 9 値の合計で並べない理由: 装備攻撃力に入るのは**突き / 斬り / 魔攻 / 魔防の 4 値だけ**で、
/// しかもキャラ 1 人が使うのは**そのうち 1 つ**(スキルの依存種別)。9 値に薄く散らした称号より、
/// 1 値に寄せた称号のほうが実際の与ダメージは大きい。
/// 例: 緋馬の怪火 - 突き は 突き90 の 1 値(9 値合計 90)だが、突き依存のキャラには
/// 名誉の証(トーデン兄妹)の 突き50 + 斬り50 + …(9 値合計 205)より効く。
pub fn title_catalog() -> Vec<TitleDef> {
    let mut catalog = vec![
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 20.0, condition: AddedDamageCondition::Region(GameRegion::LostIsland) }), ..t("eclipse", "エクリプス", Special, "喪失の島", None,
          v(40, 40, 40, 40, 40, 40, 40, 40, 40), "移動速度+5 / 喪失の島関連マップで追加ダメージ+20%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 20.0, condition: AddedDamageCondition::Region(GameRegion::ShinchouNest) }), ..t("shinchou_no_negura", "神鳥の塒", Special, "神鳥の塒", None,
          v(30, 30, 30, 30, 30, 30, 30, 30, 30), "移動速度+5 / 神鳥の塒関連マップで追加ダメージ+20%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 20.0, condition: AddedDamageCondition::Region(GameRegion::ArklonUnderground) }), ..t("arklon_death_knight", "死の騎士", Special, "アークロン要塞", None,
          v(20, 20, 20, 20, 20, 20, 20, 20, 20), "移動速度+5 / アークロン地下要塞関連マップでの追加ダメージ+20%") },
        t("rune_kizuna_5", "ルーンの絆Ⅴ", Special, "ルーン", None,
          v(11, 11, 11, 11, 11, 11, 11, 11, 11), "テイルズID内の8キャラLv310達成 / レアドロップ率+100%"),
        t("rune_kizuna_3", "ルーンの絆Ⅲ", Special, "ルーン", None,
          v(10, 10, 10, 10, 10, 10, 10, 10, 10), "テイルズID内の8キャラLv285達成 / レアドロップ率+50%"),
        t("rune_kizuna_4", "ルーンの絆Ⅳ", Special, "ルーン", None,
          v(10, 10, 10, 10, 10, 10, 10, 10, 10), "テイルズID内の8キャラLv300達成 / レアドロップ率+80%"),
        t("fukugen_suru_mono", "復元する者(ペルカンダル)", Special, "剣の才能", None,
          v(8, 8, 10, 8, 8, 10, 5, 10, 10), "週間順位1位 / レアドロップ率+50%・合成成功率+30%"),
        t("rune_kizuna_2", "ルーンの絆Ⅱ", Special, "ルーン", None,
          v(8, 8, 8, 8, 8, 8, 8, 8, 8), "テイルズID内の8キャラLv270達成 / レアドロップ率+50%"),
        t("god_slayer", "ゴッドスレイヤー", Special, "グラデル", None,
          v(0, 0, 20, 0, 20, 0, 0, 20, 0), "召喚者1週間・PTメンバー1日の期限付き、地震エフェクト付き"),
        t("sonaeru_mono", "備える者(フラカン)", Special, "剣の才能", None,
          v(6, 6, 8, 6, 6, 8, 4, 8, 8), "週間順位2位 / レアドロップ率+50%・合成成功率+30%"),
        t("rune_kizuna_1", "ルーンの絆", Special, "ルーン", None,
          v(6, 6, 6, 6, 6, 6, 6, 6, 6), "テイルズID内の8キャラLv265達成 / レアドロップ率+50%"),
        t("senshin", "戦神", Special, "星の戦場", None,
          v(5, 5, 6, 5, 5, 6, 6, 6, 6), "星の戦場ランキング報酬(シーズン)1位 / 経験値追加獲得、レア取得確率+10％"),
        t("iji_suru_mono", "維持する者", Special, "剣の才能", None,
          v(4, 4, 6, 4, 4, 6, 3, 6, 6), "週間順位3位 / レアドロップ率+50%・合成成功率+30%"),
        t("eiyuu", "英雄", Special, "星の戦場", None,
          v(4, 4, 6, 4, 4, 5, 5, 5, 5), "星の戦場ランキング報酬(シーズン)2位 / 獲得経験値+10%、レア取得確率+10%"),
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Region(GameRegion::Praba) }), ..t("golmodaf_slayer", "ゴルモダフスレイヤー", Special, "プラバ前哨基地", None,
          v(10, 10, 0, 10, 10, 0, 0, 0, 0), "ゴルモダフ討伐(24時間) / 移動速度+5、プラバ関連マップで追加ダメージ+10%") },
        t("haou", "覇王", Special, "星の戦場", None,
          v(4, 4, 5, 4, 4, 5, 3, 4, 4), "星の戦場ランキング報酬(シーズン)3位 / 経験値追加獲得、レア取得確率+10％"),
        t("ken_no_shisai_kouhosha", "剣の司祭の候補者", Special, "剣の才能", None,
          v(3, 3, 4, 3, 3, 4, 2, 4, 4), "週間順位4〜50位"),
        t("bishokuka", "美食家", Normal, "月の島", None,
          v(5, 5, 3, 5, 5, 2, 2, 3, 0), "シノンのクエスト「食べ物をよこせ (反復)」を20回以上クリア"),
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Region(GameRegion::Praba) }), ..t("golron_slayer", "ゴルロンスレイヤー", Special, "プラバ前哨基地", None,
          v(7, 7, 0, 7, 7, 0, 0, 0, 0), "ゴルロン討伐(24時間) / 移動速度+5、プラバ関連マップで追加ダメージ+10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Region(GameRegion::Praba) }), ..t("kyojin_gyakusatsusha", "巨人虐殺者", Special, "プラバ前哨基地", None,
          v(3, 3, 3, 3, 3, 3, 3, 3, 3), "巨人族殲滅戦ランキング1〜3位 / 移動速度+5、プラバ関連マップで追加ダメージ+10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Region(GameRegion::Praba) }), ..t("kongen_hakaisha", "根源破壊者", Special, "プラバ前哨基地", None,
          v(3, 3, 3, 3, 3, 3, 3, 3, 3), "スルトの力の根源ミッションランキング1〜3位 / 移動速度+5、プラバ関連マップで追加ダメージ+10%") },
        t("senjou_no_shihaisha", "戦場の支配者", Special, "星の戦場", None,
          v(3, 3, 2, 3, 3, 4, 3, 2, 3), "星の戦場ランキング報酬(シーズン)4〜50位 / 経験値追加獲得、レア取得確率+5％"),
        t("genkai_toppa", "限界突破！", Normal, "TalesWeaverマニア", None,
          v(3, 3, 2, 4, 2, 2, 4, 1, 1), "2次覚醒達成"),
        t("kouki_naru_hane", "高貴なる羽", Special, "新テイルズウィーバー★", None,
          v(2, 2, 0, 0, 0, 5, 4, 5, 4), "Lv270達成"),
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("deep_apostle") }), ..t("arklon_guardian_hack", "アークロン要塞守護者 - HACK", Special, "アークロン要塞", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("deep_apostle") }), ..t("arklon_guardian_int", "アークロン要塞守護者 - INT", Special, "アークロン要塞", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("deep_apostle") }), ..t("arklon_guardian_stab", "アークロン要塞守護者 - STAB", Special, "アークロン要塞", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "/ 深淵の使徒に追加ダメージ +10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("deep_apostle") }), ..t("arklon_guardian_stab_hack", "アークロン要塞守護者 - 物理複合", Special, "アークロン要塞", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("deep_apostle") }), ..t("arklon_guardian_hack_int", "アークロン要塞守護者 - 魔法斬り", Special, "アークロン要塞", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("deep_apostle") }), ..t("arklon_guardian_mr", "アークロン要塞守護者 - 魔法防御", Special, "アークロン要塞", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~") },
        t("negai_no_kagami", "願いの鏡の放浪者", Special, "エピソード3:共鳴", None,
          v(4, 4, 4, 4, 4, 0, 0, 0, 0), "EP3CP7クリア"),
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("shirairon") }), ..t("mercurial_shirairon_hack", "シライロン - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("shirairon") }), ..t("mercurial_shirairon_stab_hack", "シライロン - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("shirairon") }), ..t("mercurial_shirairon_stab", "シライロン - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "シライロン（レアドロップ） / シライロンに追加ダメージ +10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("shirairon") }), ..t("mercurial_shirairon_int", "シライロン - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("shirairon") }), ..t("mercurial_shirairon_hack_int", "シライロン - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("shirairon") }), ..t("mercurial_shirairon_mr", "シライロン - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("silvan") }), ..t("mercurial_silvan_hack", "シルバン - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("silvan") }), ..t("mercurial_silvan_stab_hack", "シルバン - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("silvan") }), ..t("mercurial_silvan_stab", "シルバン - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "シルバン（レアドロップ） / シルバンに追加ダメージ +10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("silvan") }), ..t("mercurial_silvan_int", "シルバン - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("silvan") }), ..t("mercurial_silvan_hack_int", "シルバン - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("silvan") }), ..t("mercurial_silvan_mr", "シルバン - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("serion") }), ..t("mercurial_serion_hack", "セリオン - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("serion") }), ..t("mercurial_serion_stab_hack", "セリオン - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("serion") }), ..t("mercurial_serion_stab", "セリオン - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "セリオン（レアドロップ） / セリオンに追加ダメージ +10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("serion") }), ..t("mercurial_serion_int", "セリオン - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("serion") }), ..t("mercurial_serion_hack_int", "セリオン - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("serion") }), ..t("mercurial_serion_mr", "セリオン - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("sereana") }), ..t("mercurial_sereana_hack", "セレアナ - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("sereana") }), ..t("mercurial_sereana_stab_hack", "セレアナ - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("sereana") }), ..t("mercurial_sereana_stab", "セレアナ - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "セレアナ（レアドロップ） / セレアナに追加ダメージ +10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("sereana") }), ..t("mercurial_sereana_int", "セレアナ - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("sereana") }), ..t("mercurial_sereana_hack_int", "セレアナ - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("sereana") }), ..t("mercurial_sereana_mr", "セレアナ - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("luminous") }), ..t("mercurial_luminous_hack", "ルミナス - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("luminous") }), ..t("mercurial_luminous_stab_hack", "ルミナス - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("luminous") }), ..t("mercurial_luminous_stab", "ルミナス - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "ルミナス（レアドロップ） / ルミナスに追加ダメージ +10%") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("luminous") }), ..t("mercurial_luminous_int", "ルミナス - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("luminous") }), ..t("mercurial_luminous_hack_int", "ルミナス - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~") },
        TitleDef { conditional_added_damage: Some(ConditionalAddedDamage { percent: 10.0, condition: AddedDamageCondition::Enemy("luminous") }), ..t("mercurial_luminous_mr", "ルミナス - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~") },
        t("shouri_no_shuyaku", "勝利の主役", Special, "星の戦場", None,
          v(2, 2, 2, 2, 2, 3, 3, 2, 2), "星の戦場ランキング報酬(シーズン)51〜100位"),
        t("mankai_no_hana", "満開の花", Normal, "月の島", None,
          v(5, 5, 4, 0, 0, 3, 3, 0, 0), "ヘクトルのクエスト「[t] アンテモエサ退治依頼 (反復)」を30回以上クリア"),
        t("rune_shinshi_3", "ルーンの紳士Ⅲ", Special, "ルーン", None,
          v(2, 2, 2, 2, 2, 2, 2, 2, 2), "男性キャラクター　※キャラクター別に獲得"),
        t("kiwameshi_mono", "極めし者", Special, "新テイルズウィーバー★", None,
          v(1, 1, 0, 0, 0, 4, 4, 4, 4), "上級者入門 取得済みである事 / 遠隔クエストで受諾後、ナルビクのロングソードGNで完了"),
        t("yakousei", "夜行性", Normal, "月の島", None,
          v(0, 0, 0, 5, 5, 3, 3, 2, 0), "ヘクトルのクエスト「ナイトメア退治依頼 (反復)」を30回以上クリア"),
        t("hyoui_shita", "憑依した", Normal, "活気溢れる月の島", None,
          v(3, 3, 2, 3, 3, 1, 2, 0, 1), "記憶の殿堂の？の好感度？(3000〜10000)"),
        t("orlanne_haru", "オルランヌの春", Special, "エピソード3:共鳴", None,
          v(3, 3, 3, 3, 3, 0, 0, 0, 0), "EP3CP6クリア"),
        t("kitakaze_kansetsu", "北風寒雪", Special, "エピソード3:共鳴", None,
          v(3, 3, 3, 3, 3, 0, 0, 0, 0), "EP3CP5クリア"),
        t("soukyuu_swordsman", "蒼穹の守護剣士", Special, "ネオテシス", None,
          v(4, 4, 0, 0, 0, 3, 2, 2, 0), "ネオテシス2クリア宝箱報酬"),
        t("soukyuu_knight", "蒼穹の守護騎士", Special, "ネオテシス", None,
          v(0, 0, 6, 0, 6, 2, 1, 0, 0), "ネオテシス2クリア宝箱報酬"),
        t("ikkitousen_no_yuusha", "一騎当千の勇者", Special, "鬼哭の城", None,
          v(5, 5, 0, 5, 0, 0, 0, 0, 0), "鬼哭の城二の丸をソロで制覇"),
        // --- event(無条件のダメージ増加を持つもの。wiki「称号/event」)
        td("senmonka_damage", "専門家 - ダメージ", Event, "専門家", None,
           v(0, 0, 0, 0, 0, 0, 0, 0, 0), 10.0, "課金箱"),
        td("meikyou_shisui_thrust", "明鏡止水 - 突き", Event, "明鏡止水", None,
           v(30, 0, 0, 0, 0, 0, 0, 0, 0), 10.0, "課金箱"),
        td("meikyou_shisui_slash", "明鏡止水 - 斬り", Event, "明鏡止水", None,
           v(0, 30, 0, 0, 0, 0, 0, 0, 0), 10.0, "課金箱"),
        td("meikyou_shisui_magic_attack", "明鏡止水 - 魔法攻撃", Event, "明鏡止水", None,
           v(0, 0, 0, 30, 0, 0, 0, 0, 0), 10.0, "課金箱"),
        td("meikyou_shisui_magic_defense", "明鏡止水 - 魔法防御", Event, "明鏡止水", None,
           v(0, 0, 0, 0, 30, 0, 0, 0, 0), 10.0, "課金箱"),
        td("kouki_naru_mono_thrust", "高貴なる者 - 突き", Event, "高貴なる者", None,
           v(40, 0, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kouki_naru_mono_slash", "高貴なる者 - 斬り", Event, "高貴なる者", None,
           v(0, 40, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kouki_naru_mono_magic_attack", "高貴なる者 - 魔法攻撃", Event, "高貴なる者", None,
           v(0, 0, 0, 40, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kouki_naru_mono_magic_defense", "高貴なる者 - 魔法防御", Event, "高貴なる者", None,
           v(0, 0, 0, 0, 40, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kokko_no_aura_thrust", "黒虎のオーラ - 突き", Event, "黒虎のオーラ", None,
           v(50, 0, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kokko_no_aura_slash", "黒虎のオーラ - 斬り", Event, "黒虎のオーラ", None,
           v(0, 50, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kokko_no_aura_magic_attack", "黒虎のオーラ - 魔法攻撃", Event, "黒虎のオーラ", None,
           v(0, 0, 0, 50, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("kokko_no_aura_magic_defense", "黒虎のオーラ - 魔法防御", Event, "黒虎のオーラ", None,
           v(0, 0, 0, 0, 50, 0, 0, 0, 0), 20.0, "課金箱"),
        td("shinbou_kokuto_thrust", "辛卯黒兎 - 突き", Event, "辛卯黒兎", None,
           v(60, 0, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("shinbou_kokuto_slash", "辛卯黒兎 - 斬り", Event, "辛卯黒兎", None,
           v(0, 60, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("shinbou_kokuto_magic_attack", "辛卯黒兎 - 魔法攻撃", Event, "辛卯黒兎", None,
           v(0, 0, 0, 60, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("shinbou_kokuto_magic_defense", "辛卯黒兎 - 魔法防御", Event, "辛卯黒兎", None,
           v(0, 0, 0, 0, 60, 0, 0, 0, 0), 20.0, "課金箱"),
        td("seiryuu_shakuzen_thrust", "青龍灼然 - 突き", Event, "青龍灼然", None,
           v(70, 0, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("seiryuu_shakuzen_slash", "青龍灼然 - 斬り", Event, "青龍灼然", None,
           v(0, 70, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("seiryuu_shakuzen_magic_attack", "青龍灼然 - 魔法攻撃", Event, "青龍灼然", None,
           v(0, 0, 0, 70, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("seiryuu_shakuzen_magic_defense", "青龍灼然 - 魔法防御", Event, "青龍灼然", None,
           v(0, 0, 0, 0, 70, 0, 0, 0, 0), 20.0, "課金箱"),
        td("souda_no_yuuei_thrust", "蒼蛇の幽影 - 突き", Event, "蒼蛇の幽影", None,
           v(80, 0, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("souda_no_yuuei_slash", "蒼蛇の幽影 - 斬り", Event, "蒼蛇の幽影", None,
           v(0, 80, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("souda_no_yuuei_magic_attack", "蒼蛇の幽影 - 魔法攻撃", Event, "蒼蛇の幽影", None,
           v(0, 0, 0, 80, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("souda_no_yuuei_magic_defense", "蒼蛇の幽影 - 魔法防御", Event, "蒼蛇の幽影", None,
           v(0, 0, 0, 0, 80, 0, 0, 0, 0), 20.0, "課金箱"),
        td("hiba_no_kaika_thrust", "緋馬の怪火 - 突き", Event, "緋馬の怪火", None,
           v(90, 0, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("hiba_no_kaika_slash", "緋馬の怪火 - 斬り", Event, "緋馬の怪火", None,
           v(0, 90, 0, 0, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("hiba_no_kaika_magic_attack", "緋馬の怪火 - 魔法攻撃", Event, "緋馬の怪火", None,
           v(0, 0, 0, 90, 0, 0, 0, 0, 0), 20.0, "課金箱"),
        td("hiba_no_kaika_magic_defense", "緋馬の怪火 - 魔法防御", Event, "緋馬の怪火", None,
           v(0, 0, 0, 0, 90, 0, 0, 0, 0), 20.0, "課金箱"),
        td("meiyo_bonobono", "名誉の証（ぼのぼの）", Event, "特別EVENT 2022年", None,
           v(40, 40, 13, 11, 11, 13, 13, 13, 13), 10.0, "名誉の証"),
        td("meiyo_shimarisu", "名誉の証（シマリスくん）", Event, "特別EVENT 2022年", None,
           v(11, 11, 13, 40, 40, 13, 13, 13, 13), 10.0, "名誉の証"),
        td("meiyo_araiguma", "名誉の証（アライグマくん）", Event, "特別EVENT 2022年", None,
           v(30, 30, 30, 30, 30, 30, 30, 30, 30), 10.0, "名誉の証"),
        td("meiyo_konton", "名誉の証（混沌勢）", Event, "特別EVENT 2022年", None,
           v(50, 50, 10, 0, 0, 10, 5, 5, 10), 10.0, "名誉の証"),
        td("meiyo_chouwa", "名誉の証（調和勢）", Event, "特別EVENT 2022年", None,
           v(0, 0, 10, 50, 50, 5, 5, 10, 10), 10.0, "名誉の証"),
        td("meiyo_kiroku_no_chiheisen", "名誉の証（記録の地平線）", Event, "特別EVENT 2023年", None,
           v(10, 10, 10, 50, 50, 10, 10, 10, 10), 10.0, "名誉の証"),
        td("meiyo_ddd", "名誉の証（D.D.D）", Event, "特別EVENT 2023年", None,
           v(50, 50, 10, 10, 10, 10, 10, 10, 10), 10.0, "名誉の証"),
        td("meiyo_touzoku_goroshi", "名誉の証（盗賊殺し）", Event, "特別EVENT 2023年", None,
           v(15, 15, 15, 15, 15, 15, 15, 15, 15), 10.0, "名誉の証"),
        td("meiyo_ma_wo_messuru_mono", "名誉の証（魔を滅する者）", Event, "特別EVENT 2023年", None,
           v(30, 30, 15, 35, 35, 10, 10, 10, 10), 10.0, "名誉の証 / クリティカル倍率+3％"),
        td("meiyo_akame_no_maou", "名誉の証（赤眼の魔王）", Event, "特別EVENT 2023年", None,
           v(50, 50, 10, 10, 10, 10, 10, 10, 10), 20.0, "名誉の証"),
        td("meiyo_meikyuu_no_nushi", "名誉の証（迷宮の主）", Event, "特別EVENT 2024年", None,
           v(25, 25, 30, 35, 35, 10, 10, 10, 10), 10.0, "名誉の証 / クリティカル倍率+3％"),
        td("meiyo_toden_kyoudai", "名誉の証（トーデン兄妹）", Event, "特別EVENT 2024年", None,
           v(50, 50, 30, 10, 20, 10, 15, 10, 10), 20.0, "名誉の証"),
        td("meiyo_kyuutei_majutsushi_no_musume", "名誉の証（宮廷魔術師の娘）", Event, "特別EVENT 2024年", None,
           v(5, 5, 15, 50, 50, 10, 10, 10, 10), 10.0, "名誉の証 / 移動速度-15"),
        td("meiyo_yasai_uri", "名誉の証（野菜売り)", Event, "特別EVENT 2024年", None,
           v(15, 15, 35, 0, 10, 10, 25, 10, 10), 10.0, "名誉の証"),
        td("meiyo_kyouran_no_majutsushi", "名誉の証（狂乱の魔術師）", Event, "特別EVENT 2024年", None,
           v(0, 0, 55, 50, 10, 10, 10, 10, 10), 15.0, "名誉の証"),
        td("meiyo_tsumi_wo_seisuru_mono", "名誉の証（罪を制する者）", Event, "特別EVENT 2024年", None,
           v(20, 60, 20, 0, 20, 20, 30, 30, 20), 15.0, "名誉の証 / クリティカル倍率+3％"),
        td("meiyo_koutei_keishou_kouho_no_himegimi", "名誉の証（皇帝位継承候補の姫君）", Event, "特別EVENT 2024年", None,
           v(10, 10, 50, 30, 50, 10, 15, 10, 10), 10.0, "名誉の証"),
        td("meiyo_tatakai_wo_ketsui_suru_koujo", "名誉の証（戦いを決意する皇女）", Event, "特別EVENT 2024年", None,
           v(30, 30, 10, 30, 30, 10, 10, 10, 10), 20.0, "名誉の証"),
        td("meiyo_teikoku_kishidan_taichou", "名誉の証（帝国騎士団隊長）", Event, "特別EVENT 2024年", None,
           v(30, 20, 20, 20, 20, 20, 20, 20, 20), 20.0, "名誉の証"),
    ];
    catalog.sort_by(|a, b| {
        // 装備攻撃力に入る 4 値(wiki: カテゴリA の内訳)。ここだけが与ダメージに効く
        let attack = |t: &TitleDef| {
            let v = t.values;
            [v.thrust, v.slash, v.magic_attack, v.magic_defense]
        };
        let best = |t: &TitleDef| attack(t).into_iter().max().unwrap_or(0);
        let attack_sum = |t: &TitleDef| attack(t).iter().sum::<i64>();
        let all_sum = |t: &TitleDef| t.values.fields().iter().map(|(_, v)| *v).sum::<i64>();
        b.attack_damage_percent
            .total_cmp(&a.attack_damage_percent)
            .then_with(|| best(b).cmp(&best(a)))
            .then_with(|| attack_sum(b).cmp(&attack_sum(a)))
            .then_with(|| all_sum(b).cmp(&all_sum(a)))
    });
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn id_は一意() {
        let mut seen = HashSet::new();
        for t in title_catalog() {
            assert!(seen.insert(t.id), "id 重複: {}", t.id);
        }
    }

    /// 収録基準を全件が満たす。normal / special は補正値 9 種の合計 15 以上、
    /// event は無条件のダメージ増加を持つこと(補正値 0 の「専門家 - ダメージ」があるため)。
    #[test]
    fn 全件が収録基準を満たす() {
        for t in title_catalog() {
            let sum: i64 = t.values.fields().iter().map(|(_, v)| v).sum();
            if t.kind == TitleKind::Event {
                assert!(t.attack_damage_percent > 0.0, "{} にダメージ増加が無い", t.name);
            } else {
                assert!(sum >= 15, "{} の合計が {}", t.name, sum);
            }
        }
    }

    /// ダメージ増加を持つのは event だけ(normal / special の「〜関連マップで追加ダメージ」は
    /// 発動条件付きなので `note` 止まり)。
    #[test]
    fn ダメージ増加はeventだけ() {
        for t in title_catalog() {
            if t.attack_damage_percent > 0.0 {
                assert_eq!(t.kind, TitleKind::Event, "{}", t.name);
            }
        }
        let n = title_catalog().iter().filter(|t| t.attack_damage_percent > 0.0).count();
        assert_eq!(n, 48);
    }

    /// ユーザー指定の必須称号(2026-08-26)。課金箱シリーズは 1 種 4 変種。
    #[test]
    fn 課金箱シリーズの必須称号が入っている() {
        let catalog = title_catalog();
        for (group, value, rate) in [
            ("黒虎のオーラ", 50, 20.0),
            ("辛卯黒兎", 60, 20.0),
            ("青龍灼然", 70, 20.0),
            ("蒼蛇の幽影", 80, 20.0),
            ("緋馬の怪火", 90, 20.0),
        ] {
            let series: Vec<_> = catalog.iter().filter(|t| t.group == group).collect();
            assert_eq!(series.len(), 4, "{group}");
            for t in series {
                assert_eq!(t.attack_damage_percent, rate, "{}", t.name);
                let sum: i64 = t.values.fields().iter().map(|(_, v)| v).sum();
                assert_eq!(sum, value, "{}", t.name);
            }
        }
    }

    /// 並び順は ダメージ増加 → 与ダメージに効く 1 値の大きさ → 4 値の合計 → 9 値の合計。
    #[test]
    fn 効果の大きい称号が先頭に来る() {
        let catalog = title_catalog();
        let key = |t: &TitleDef| {
            let v = t.values;
            let attack = [v.thrust, v.slash, v.magic_attack, v.magic_defense];
            (
                attack.into_iter().max().unwrap_or(0),
                attack.iter().sum::<i64>(),
                t.values.fields().iter().map(|(_, v)| *v).sum::<i64>(),
            )
        };
        for w in catalog.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!(
                a.attack_damage_percent > b.attack_damage_percent
                    || (a.attack_damage_percent == b.attack_damage_percent && key(a) >= key(b)),
                "{} の次に {}",
                a.name,
                b.name
            );
        }
    }

    /// ユーザー指定の必須称号は、同じダメージ増加 20% の中で先頭側に来る。
    /// 9 値の合計で並べていたときは 名誉の証 に押し下げられていた(ユーザー指摘 2026-08-26)。
    #[test]
    fn 課金箱シリーズが20パーセント帯の先頭に来る() {
        let catalog = title_catalog();
        let names: Vec<&str> = catalog
            .iter()
            .filter(|t| t.attack_damage_percent == 20.0)
            .take(5)
            .map(|t| t.group)
            .collect();
        assert_eq!(names, ["緋馬の怪火", "緋馬の怪火", "緋馬の怪火", "緋馬の怪火", "蒼蛇の幽影"]);
    }

    /// wiki の最上位。全 9 値 +40(喪失の島)。
    #[test]
    fn エクリプスは全補正40() {
        let eclipse = title_catalog().into_iter().find(|t| t.id == "eclipse").unwrap();
        assert!(eclipse.values.fields().iter().all(|(_, v)| *v == 40));
    }

    /// マーキュリアル洞窟のボス別称号は 5 ボス × 6 依存 = 30 件。
    #[test]
    fn マーキュリアル洞窟は30件() {
        let n = title_catalog().into_iter().filter(|t| t.group == "マーキュリアル洞窟").count();
        assert_eq!(n, 30);
    }
    #[test]
    fn wikiのセル継承を全依存称号へ適用する() {
        let catalog = title_catalog();
        for (prefix, enemy) in [
            ("mercurial_shirairon_", "shirairon"),
            ("mercurial_silvan_", "silvan"),
            ("mercurial_serion_", "serion"),
            ("mercurial_sereana_", "sereana"),
            ("mercurial_luminous_", "luminous"),
        ] {
            let titles: Vec<_> = catalog.iter().filter(|t| t.id.starts_with(prefix)).collect();
            assert_eq!(titles.len(), 6, "{prefix}");
            for title in titles {
                assert_eq!(
                    title.conditional_added_damage,
                    Some(ConditionalAddedDamage {
                        percent: 10.0,
                        condition: AddedDamageCondition::Enemy(enemy),
                    }),
                    "{}",
                    title.id
                );
            }
        }
        let guardians: Vec<_> =
            catalog.iter().filter(|t| t.id.starts_with("arklon_guardian_")).collect();
        assert_eq!(guardians.len(), 6);
        assert!(guardians.iter().all(|t| t.conditional_added_damage == Some(ConditionalAddedDamage {
            percent: 10.0,
            condition: AddedDamageCondition::Enemy("deep_apostle"),
        })));
    }

    #[test]
    fn 地域称号の条件を解決する() {
        let catalog = title_catalog();
        assert_eq!(
            domain::title_added_damage_rate(
                Some("eclipse"),
                &catalog,
                Some(GameRegion::LostIsland),
                None,
            ),
            0.20
        );
        assert_eq!(
            domain::title_added_damage_rate(
                Some("eclipse"),
                &catalog,
                None,
                Some("eclipse_1"),
            ),
            0.0
        );
        assert_eq!(
            domain::title_added_damage_rate(
                Some("arklon_death_knight"),
                &catalog,
                Some(GameRegion::ArklonUnderground),
                Some("arklon_underground"),
            ),
            0.20
        );
        assert_eq!(
            domain::title_added_damage_rate(
                Some("arklon_death_knight"),
                &catalog,
                None,
                Some("abyss_hell"),
            ),
            0.0
        );
    }
}
