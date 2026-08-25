//! 称号のカタログ。
//!
//! 出典: wiki「称号/normal」「称号/special」(取得 2026-08-25)。
//!
//! **主要称号のみ収録**(ユーザー決定 2026-08-25。全 565 件を入れる必要はない)。
//! 基準は**補正値 9 種の合計が 15 以上**。称号は 1 枠しか装備できないので、
//! それ未満は選択肢として意味を持たない。この基準で 72 件。
//!
//! wiki の表の列は 名称 / 習得Lv / 突き / 斬り / 物防 / 魔攻 / 魔防 / 命中 / 回避 / 敏捷 / Cri で、
//! 装備補正 9 値と同じ顔ぶれ(並びだけ違う)。 の並びに直して持つ。
//!
//! 備考欄の条件付き効果(「喪失の島関連マップで追加ダメージ+20%」など)は発動条件を
//! 計算対象に持っていないので  に残すだけで計算には入れない。
//! グループボーナス(「N 個完成で +α」)は所持状況の入力が要るのでスコープ外。

use domain::{EquipmentValues, TitleDef, TitleKind};

use crate::Source;

/// 称号カタログの出典。
pub const TITLE_SOURCE: Source = Source {
    page: "称号/normal, 称号/special",
    retrieved_on: "2026-08-25",
    note: "主要称号のみ収録(補正値 9 種の合計 15 以上 = 72 件)。備考の条件付き効果とグループボーナスは未実装",
};

use TitleKind::{Normal, Special};

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

const fn t(
    id: &'static str,
    name: &'static str,
    kind: TitleKind,
    group: &'static str,
    level: Option<u16>,
    values: EquipmentValues,
    note: &'static str,
) -> TitleDef {
    TitleDef { id, name, kind, group, level, values, note }
}

/// 補正値の合計が大きい順。UI はこの順でそのまま出す(強い称号が上に来る)。
pub fn title_catalog() -> Vec<TitleDef> {
    vec![
        t("eclipse", "エクリプス", Special, "喪失の島", None,
          v(40, 40, 40, 40, 40, 40, 40, 40, 40), "移動速度+5 / 喪失の島関連マップで追加ダメージ+20%"),
        t("shinchou_no_negura", "神鳥の塒", Special, "神鳥の塒", None,
          v(30, 30, 30, 30, 30, 30, 30, 30, 30), "移動速度+5 / 神鳥の塒関連マップで追加ダメージ+20%"),
        t("arklon_death_knight", "死の騎士", Special, "アークロン要塞", None,
          v(20, 20, 20, 20, 20, 20, 20, 20, 20), "移動速度+5 / アークロン地下要塞関連マップでの追加ダメージ+20%"),
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
        t("golmodaf_slayer", "ゴルモダフスレイヤー", Special, "プラバ前哨基地", None,
          v(10, 10, 0, 10, 10, 0, 0, 0, 0), "ゴルモダフ討伐(24時間) / 移動速度+5、プラバ関連マップで追加ダメージ+10%"),
        t("haou", "覇王", Special, "星の戦場", None,
          v(4, 4, 5, 4, 4, 5, 3, 4, 4), "星の戦場ランキング報酬(シーズン)3位 / 経験値追加獲得、レア取得確率+10％"),
        t("ken_no_shisai_kouhosha", "剣の司祭の候補者", Special, "剣の才能", None,
          v(3, 3, 4, 3, 3, 4, 2, 4, 4), "週間順位4〜50位"),
        t("bishokuka", "美食家", Normal, "月の島", None,
          v(5, 5, 3, 5, 5, 2, 2, 3, 0), "シノンのクエスト「食べ物をよこせ (反復)」を20回以上クリア"),
        t("golron_slayer", "ゴルロンスレイヤー", Special, "プラバ前哨基地", None,
          v(7, 7, 0, 7, 7, 0, 0, 0, 0), "ゴルロン討伐(24時間) / 移動速度+5、プラバ関連マップで追加ダメージ+10%"),
        t("kyojin_gyakusatsusha", "巨人虐殺者", Special, "プラバ前哨基地", None,
          v(3, 3, 3, 3, 3, 3, 3, 3, 3), "巨人族殲滅戦ランキング1〜3位 / 移動速度+5、プラバ関連マップで追加ダメージ+10%"),
        t("kongen_hakaisha", "根源破壊者", Special, "プラバ前哨基地", None,
          v(3, 3, 3, 3, 3, 3, 3, 3, 3), "スルトの力の根源ミッションランキング1〜3位 / 移動速度+5、プラバ関連マップで追加ダメージ+10%"),
        t("senjou_no_shihaisha", "戦場の支配者", Special, "星の戦場", None,
          v(3, 3, 2, 3, 3, 4, 3, 2, 3), "星の戦場ランキング報酬(シーズン)4〜50位 / 経験値追加獲得、レア取得確率+5％"),
        t("genkai_toppa", "限界突破！", Normal, "TalesWeaverマニア", None,
          v(3, 3, 2, 4, 2, 2, 4, 1, 1), "2次覚醒達成"),
        t("kouki_naru_hane", "高貴なる羽", Special, "新テイルズウィーバー★", None,
          v(2, 2, 0, 0, 0, 5, 4, 5, 4), "Lv270達成"),
        t("arklon_guardian_hack", "アークロン要塞守護者 - HACK", Special, "アークロン要塞", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("arklon_guardian_int", "アークロン要塞守護者 - INT", Special, "アークロン要塞", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~"),
        t("arklon_guardian_stab", "アークロン要塞守護者 - STAB", Special, "アークロン要塞", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "/ 深淵の使徒に追加ダメージ +10%"),
        t("arklon_guardian_stab_hack", "アークロン要塞守護者 - 物理複合", Special, "アークロン要塞", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("arklon_guardian_hack_int", "アークロン要塞守護者 - 魔法斬り", Special, "アークロン要塞", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~"),
        t("arklon_guardian_mr", "アークロン要塞守護者 - 魔法防御", Special, "アークロン要塞", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~"),
        t("negai_no_kagami", "願いの鏡の放浪者", Special, "エピソード3:共鳴", None,
          v(4, 4, 4, 4, 4, 0, 0, 0, 0), "EP3CP7クリア"),
        t("mercurial_shirairon_hack", "シライロン - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_shirairon_stab_hack", "シライロン - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_shirairon_stab", "シライロン - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "シライロン（レアドロップ） / シライロンに追加ダメージ +10%"),
        t("mercurial_shirairon_int", "シライロン - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~"),
        t("mercurial_shirairon_hack_int", "シライロン - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~"),
        t("mercurial_shirairon_mr", "シライロン - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~"),
        t("mercurial_silvan_hack", "シルバン - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_silvan_stab_hack", "シルバン - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_silvan_stab", "シルバン - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "シルバン（レアドロップ） / シルバンに追加ダメージ +10%"),
        t("mercurial_silvan_int", "シルバン - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~"),
        t("mercurial_silvan_hack_int", "シルバン - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~"),
        t("mercurial_silvan_mr", "シルバン - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~"),
        t("mercurial_serion_hack", "セリオン - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_serion_stab_hack", "セリオン - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_serion_stab", "セリオン - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "セリオン（レアドロップ） / セリオンに追加ダメージ +10%"),
        t("mercurial_serion_int", "セリオン - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~"),
        t("mercurial_serion_hack_int", "セリオン - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~"),
        t("mercurial_serion_mr", "セリオン - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~"),
        t("mercurial_sereana_hack", "セレアナ - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_sereana_stab_hack", "セレアナ - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_sereana_stab", "セレアナ - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "セレアナ（レアドロップ） / セレアナに追加ダメージ +10%"),
        t("mercurial_sereana_int", "セレアナ - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~"),
        t("mercurial_sereana_hack_int", "セレアナ - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~"),
        t("mercurial_sereana_mr", "セレアナ - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~"),
        t("mercurial_luminous_hack", "ルミナス - 斬り", Special, "マーキュリアル洞窟", None,
          v(0, 20, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_luminous_stab_hack", "ルミナス - 物理複合", Special, "マーキュリアル洞窟", None,
          v(10, 10, 0, 0, 0, 0, 0, 0, 0), "~"),
        t("mercurial_luminous_stab", "ルミナス - 突き", Special, "マーキュリアル洞窟", None,
          v(20, 0, 0, 0, 0, 0, 0, 0, 0), "ルミナス（レアドロップ） / ルミナスに追加ダメージ +10%"),
        t("mercurial_luminous_int", "ルミナス - 魔法攻撃", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 20, 0, 0, 0, 0, 0), "~"),
        t("mercurial_luminous_hack_int", "ルミナス - 魔法斬り", Special, "マーキュリアル洞窟", None,
          v(0, 10, 0, 10, 0, 0, 0, 0, 0), "~"),
        t("mercurial_luminous_mr", "ルミナス - 魔法防御", Special, "マーキュリアル洞窟", None,
          v(0, 0, 0, 0, 20, 0, 0, 0, 0), "~"),
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
    ]
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

    /// 収録基準(補正値 9 種の合計 15 以上)を全件が満たす。
    #[test]
    fn 全件が収録基準を満たす() {
        for t in title_catalog() {
            let sum: i64 = t.values.fields().iter().map(|(_, v)| v).sum();
            assert!(sum >= 15, "{} の合計が {}", t.name, sum);
        }
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
}
