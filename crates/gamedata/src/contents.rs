//! コンテンツカタログ(エリア → コンテンツ)。ホームの到達一覧・入場条件判定に使う。
//!
//! 入場条件は swiki「コンテンツ入場条件」(<https://erumisutoburvip.swiki.jp/>、取得 2026-08-24)を
//! 正とする。表の列 S/H/I(突き/斬り/魔攻)・M(魔防)・複合(突斬 or 斬魔)は、使うスキルの
//! 依存種別で比較先が決まる別条件で「いずれかを満たしていれば OK」(表の概要、ユーザー確認済み)。
//! これを `ContentRequirement::EquipmentBySkill` の 1 件で表し、判定時にスキル依存で
//! 比較先を選ぶ。表の "-" は 0(その系統の条件なし)。
//!
//! 判定できない条件(ルーンレベル・共通スキルコンプ・コア・テシスコア・前提クリア)は
//! `entry_note` に表示専用で持つ(キャラモデルに値が無く、条件にすると「常に未達」か
//! 「常に無視」の嘘になるため)。
//!
//! 敵データ(`enemies.rs`)があるコンテンツは火力も判定する。敵データが無いコンテンツは
//! `enemy_id`/`need_per_hit` が None で、入場条件のみで判定する。
//! 目安ダメージ(need_per_hit)はコミュニティ知識・実測がソースで全件 `[仮]`。

use domain::content::{Content, ContentArea, ContentRequirement};

use crate::Source;

pub const CONTENTS_SOURCE: Source = Source {
    page: "swiki コンテンツ入場条件(入場条件・コンテンツ構成)+ 暫定値(目安ダメージ)",
    retrieved_on: "2026-08-24",
    note: "装備条件 S/H/I・M・複合はスキル依存で比較先が決まる(いずれか 1 つ充足で OK)。判定できない条件(ルーン Lv・共通スキル・コア等)は entry_note に表示専用。目安ダメージは全件 [仮]",
};

/// 装備補正条件。`single` = S/H/I 列、`mr` = M 列、`composite` = 複合列(表の "-" は 0)。
const fn equip(single: i64, mr: i64, composite: i64) -> ContentRequirement {
    ContentRequirement::EquipmentBySkill { single, mr, composite }
}

const fn stage(v: u8) -> ContentRequirement {
    ContentRequirement::AwakeningStage(v)
}

const fn eternal(v: u8) -> ContentRequirement {
    ContentRequirement::EternalLevel(v)
}

/// コンテンツ 1 件の定義。
struct Def {
    id: &'static str,
    name: &'static str,
    /// 対応する敵(enemies.rs)。無ければ None で入場条件のみ判定する
    enemy_id: Option<&'static str>,
    /// 目安ダメージ `[仮]`。敵が無ければ None
    need_per_hit: Option<i64>,
    requirements: &'static [ContentRequirement],
    entry_note: Option<&'static str>,
    team_note: Option<&'static str>,
}

impl Def {
    fn to_content(&self) -> Content {
        Content {
            id: self.id.to_string(),
            name: self.name.to_string(),
            enemy_id: self.enemy_id.map(String::from),
            need_per_hit: self.need_per_hit,
            requirements: self.requirements.to_vec(),
            entry_note: self.entry_note.map(String::from),
            team_note: self.team_note.map(String::from),
        }
    }
}

/// 下位コンテンツ共通の注記(swiki: リンゴと煩わしい怒り以外はルーンレベル30 必要)。
const RUNE30: Option<&str> = Some("ルーンレベル 30 必要(判定対象外)");
/// 上位コンテンツ共通の注記(swiki: 5次覚醒・ルーンレベル40・共通スキルコンプリート)。
const UPPER: &str = "ルーンレベル 40・共通スキルコンプリート必要(判定対象外)";

/// 上位コンテンツの entry_note(共通注記 + コア要求)。コアはキャラモデルに無いため表示専用。
const CORE_0: Option<&str> = Some(UPPER);
const CORE_60: Option<&str> = Some("ルーンレベル 40・共通スキルコンプリート・コア 60 必要(判定対象外)");
const CORE_120: Option<&str> = Some("ルーンレベル 40・共通スキルコンプリート・コア 120 必要(判定対象外)");
const CORE_210: Option<&str> = Some("ルーンレベル 40・共通スキルコンプリート・コア 210 必要(判定対象外)");
const CORE_300: Option<&str> = Some("ルーンレベル 40・共通スキルコンプリート・コア 300 必要(判定対象外)");
const CORE_480: Option<&str> = Some("ルーンレベル 40・共通スキルコンプリート・コア 480 必要(判定対象外)");

/// 上位コンテンツ共通の覚醒条件(5 次覚醒)。
const STAGE5: ContentRequirement = stage(5);

#[rustfmt::skip]
const AREAS: &[(&str, &str, &[Def])] = &[
    // ================= 下位コンテンツ(swiki *下位コンテンツ) =================
    ("lower", "下位コンテンツ", &[
        Def { id: "ringo", name: "リンゴ", enemy_id: Some("ringo_boss"), need_per_hit: Some(2_000),
              requirements: &[stage(3), equip(800, 980, 0)],
              entry_note: Some("配布インファ程度"), team_note: None },
        Def { id: "abyss_normal", name: "アビス(ノーマル)", enemy_id: None, need_per_hit: None,
              requirements: &[stage(3), equip(900, 1_100, 1_650)],
              entry_note: Some("ルーンレベル 30 必要(判定対象外)/ 配布インファ+α程度"), team_note: None },
        Def { id: "shinchou_normal", name: "神鳥の塒(ノーマル)", enemy_id: None, need_per_hit: None,
              requirements: &[stage(3), equip(900, 1_100, 1_650)],
              entry_note: Some("ルーンレベル 30 必要(判定対象外)/ 配布インファ+α程度"), team_note: None },
        Def { id: "luminous_ex", name: "ルミナスEX", enemy_id: None, need_per_hit: None,
              requirements: &[stage(3), equip(900, 1_300, 1_500)],
              entry_note: Some("ルーンレベル 30・コア 60 必要(判定対象外)"), team_note: None },
        Def { id: "annoying_anger", name: "煩わしい怒り", enemy_id: None, need_per_hit: None,
              requirements: &[stage(3), equip(1_000, 1_180, 1_750)],
              entry_note: None, team_note: None },
        Def { id: "abyss_hard", name: "アビス(ハード)", enemy_id: None, need_per_hit: None,
              requirements: &[stage(4), equip(1_100, 1_300, 1_850)],
              entry_note: Some("ルーンレベル 30 必要(判定対象外)/ アクィルス未強化〜+α程度"), team_note: None },
        Def { id: "shinchou_hard", name: "神鳥の塒(ハード)", enemy_id: None, need_per_hit: None,
              requirements: &[stage(4), equip(1_150, 1_350, 1_900)],
              entry_note: Some("ルーンレベル 30 必要(判定対象外)/ アクィルス未強化〜+α程度"), team_note: None },
        Def { id: "abyss_hell", name: "アビス(ヘル)", enemy_id: Some("abyss_hell"), need_per_hit: Some(3_000),
              requirements: &[stage(4), equip(1_250, 1_450, 2_000)],
              entry_note: Some("ルーンレベル 30 必要(判定対象外)/ アクィルス+10しまくったらいける"), team_note: None },
        Def { id: "longed_pleasure", name: "憧れの楽しみ", enemy_id: None, need_per_hit: None,
              requirements: &[stage(4), equip(1_250, 1_450, 2_000)],
              entry_note: RUNE30, team_note: None },
        Def { id: "relic_sanctuary_shinchou", name: "古代レリックの聖域(神鳥)", enemy_id: None, need_per_hit: None,
              requirements: &[stage(3), equip(1_250, 1_450, 2_000)],
              entry_note: RUNE30, team_note: None },
    ]),
    // ============ エタレベルのみが条件(swiki *エタレベルのみが条件のコンテンツ) ============
    ("eternal_only", "エタレベルのみが条件", &[
        Def { id: "vestige_ruins", name: "ヴェスティージの廃墟", enemy_id: None, need_per_hit: None,
              requirements: &[eternal(1)], entry_note: None, team_note: None },
        Def { id: "orlie_defense_hell", name: "オルリー防衛戦(ヘル)", enemy_id: None, need_per_hit: None,
              requirements: &[eternal(1)], entry_note: Some("ノーマル 1 回クリア必要(判定対象外)"), team_note: None },
        Def { id: "detachment_subjugation", name: "別動隊討伐", enemy_id: None, need_per_hit: None,
              requirements: &[eternal(1)], entry_note: None, team_note: None },
        Def { id: "siokan_boss_subjugation", name: "シオカンヘイムボス討伐戦", enemy_id: Some("siokan_boss"), need_per_hit: Some(5_000),
              requirements: &[eternal(1)], entry_note: None, team_note: None },
        Def { id: "odin_total_war", name: "オーディン全面戦争", enemy_id: Some("odin"), need_per_hit: Some(8_000),
              requirements: &[eternal(10)], entry_note: None, team_note: None },
    ]),
    // ================= 上位コンテンツ(swiki *上位コンテンツ) =================
    ("upper", "上位コンテンツ", &[
        Def { id: "abyss_ex", name: "アビスEX", enemy_id: Some("abyss_core_master"), need_per_hit: Some(3_500),
              requirements: &[STAGE5, equip(1_500, 1_700, 2_100)],
              entry_note: CORE_120, team_note: Some("改IHは不要") },
        Def { id: "eclipse_boss", name: "エクリプスボス", enemy_id: Some("eclipse_1"), need_per_hit: Some(6_000),
              requirements: &[STAGE5, equip(1_600, 1_800, 2_350)],
              entry_note: CORE_0, team_note: Some("ソロは入場条件よりもだいぶ難易度低い") },
        Def { id: "aphetiria_normal", name: "アフェティリア(ノーマル)", enemy_id: Some("aphetiria_n"), need_per_hit: Some(7_000),
              requirements: &[STAGE5, eternal(5), equip(1_600, 1_800, 2_350)],
              entry_note: CORE_0, team_note: Some("ソロの場合エタ制限のみだがソロはきつい") },
        Def { id: "moon_queen_training", name: "月の女王の軍の訓練所", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900)],
              entry_note: CORE_120, team_note: None },
        Def { id: "eclipse_subjugation", name: "エクリプスボス討伐戦", enemy_id: Some("eclipse_subjugation"), need_per_hit: Some(12_000),
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900)],
              entry_note: CORE_120, team_note: None },
        Def { id: "aphetiria_hard", name: "アフェティリア(ハード)", enemy_id: Some("kisinik_h"), need_per_hit: Some(16_000),
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900)],
              entry_note: CORE_120, team_note: Some("活躍するには靴エフェ合わせて 400 くらいほしい") },
        Def { id: "relic_sanctuary_kisinik", name: "古代レリックの聖域(キシニク)", enemy_id: Some("relic_sanctuary_20"), need_per_hit: Some(16_000),
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900)],
              entry_note: CORE_0, team_note: None },
        Def { id: "chaotic_land", name: "混乱した大地", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(20), equip(2_200, 2_600, 3_500)],
              entry_note: CORE_0, team_note: None },
        Def { id: "colorless_land", name: "色を失った大地", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(20), equip(2_200, 2_600, 3_500)],
              entry_note: CORE_0, team_note: None },
        Def { id: "architect_mine", name: "設計者の採掘場", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(20), equip(2_200, 2_600, 3_500)],
              entry_note: Some("ルーンレベル 40・共通スキルコンプリート・カフス(盾+)の上限 140 以上 必要(判定対象外)"), team_note: None },
        Def { id: "valley_defense", name: "異界の峡谷防衛戦", enemy_id: Some("valley_captain"), need_per_hit: Some(12_000),
              requirements: &[STAGE5, eternal(21), equip(2_500, 2_700, 3_700)],
              entry_note: CORE_300, team_note: None },
        Def { id: "last_battle", name: "最後の決戦", enemy_id: Some("last_battle_3"), need_per_hit: Some(15_000),
              requirements: &[STAGE5, eternal(21), equip(2_500, 2_700, 3_700)],
              entry_note: CORE_300, team_note: None },
        Def { id: "aphetiria_ex", name: "アフェティリアEX", enemy_id: Some("kisinik_ex"), need_per_hit: Some(20_000),
              requirements: &[STAGE5, eternal(41), equip(2_500, 3_000, 4_000)],
              entry_note: CORE_480, team_note: None },
        Def { id: "void_domain", name: "空虚の領域", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(41), equip(3_100, 3_500, 4_900)],
              entry_note: CORE_0, team_note: None },
        Def { id: "leitia_n", name: "追従する喜び(ノーマル)", enemy_id: Some("leitia_n"), need_per_hit: Some(18_000),
              requirements: &[STAGE5, eternal(41), equip(3_100, 3_500, 4_900)],
              entry_note: CORE_0, team_note: None },
        Def { id: "architect_n", name: "見つめる悲しみ(ノーマル)", enemy_id: Some("architect_n"), need_per_hit: Some(18_000),
              requirements: &[STAGE5, eternal(41), equip(3_100, 3_500, 4_900)],
              entry_note: CORE_60, team_note: None },
        Def { id: "pleasure_afterimage", name: "喜びの残像", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(51), equip(3_500, 3_850, 5_500)],
              entry_note: CORE_60, team_note: None },
        Def { id: "leitia_h", name: "追従する喜び(ハード)", enemy_id: Some("leitia_h"), need_per_hit: Some(22_000),
              requirements: &[STAGE5, eternal(61), equip(3_900, 4_000, 5_900)],
              entry_note: CORE_120, team_note: None },
        Def { id: "architect_h", name: "見つめる悲しみ(ハード)", enemy_id: Some("architect_h"), need_per_hit: Some(22_000),
              requirements: &[STAGE5, eternal(61), equip(3_900, 4_000, 5_900)],
              entry_note: CORE_210, team_note: None },
    ]),
    // ======== 入場条件表に無い敵(実測表由来。火力の目安確認用。条件は未収録) ========
    ("other_targets", "その他の対象(条件データなし)", &[
        Def { id: "tutatur", name: "トゥタトゥール", enemy_id: Some("tutatur"), need_per_hit: Some(2_000),
              requirements: &[], entry_note: None, team_note: Some("参加型レイド") },
        Def { id: "arklon_underground", name: "アークロン地下要塞", enemy_id: Some("arklon_underground"), need_per_hit: Some(1_500),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "clamor", name: "クラモール", enemy_id: Some("clamor"), need_per_hit: Some(4_000),
              requirements: &[], entry_note: None, team_note: Some("参加型レイド") },
        Def { id: "brothers_forge", name: "兄弟の鍛冶場", enemy_id: Some("brothers_forge"), need_per_hit: Some(3_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "odin_rank", name: "オーディン(ランク)", enemy_id: Some("odin_rank"), need_per_hit: Some(9_000),
              requirements: &[], entry_note: None, team_note: Some("ランキング戦") },
        Def { id: "chimera", name: "キマイラ", enemy_id: Some("chimera"), need_per_hit: Some(20_000),
              requirements: &[], entry_note: None, team_note: Some("参加型レイド") },
        Def { id: "lost_forest", name: "喪失の森", enemy_id: Some("lost_forest"), need_per_hit: Some(6_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "eclipse_2", name: "エクリプス ボス2", enemy_id: Some("eclipse_2"), need_per_hit: Some(6_500),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "selinacos_h", name: "セリニアコス(H)", enemy_id: Some("selinacos_h"), need_per_hit: Some(14_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "goitia_h", name: "ゴイティア(H)", enemy_id: Some("goitia_h"), need_per_hit: Some(15_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "selinacos_ex", name: "セリニアコス(EX)", enemy_id: Some("selinacos_ex"), need_per_hit: Some(18_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "goitia_ex", name: "ゴイティア(EX)", enemy_id: Some("goitia_ex"), need_per_hit: Some(18_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "valley_soldier", name: "異界の峡谷 兵士", enemy_id: Some("valley_soldier"), need_per_hit: Some(10_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "last_battle_1", name: "最後の決戦1", enemy_id: Some("last_battle_1"), need_per_hit: Some(12_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "last_battle_2", name: "最後の決戦2", enemy_id: Some("last_battle_2"), need_per_hit: Some(13_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_10", name: "レリックの聖域 10段", enemy_id: Some("relic_sanctuary_10"), need_per_hit: Some(6_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_11", name: "レリックの聖域 11段", enemy_id: Some("relic_sanctuary_11"), need_per_hit: Some(8_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_12", name: "レリックの聖域 12段", enemy_id: Some("relic_sanctuary_12"), need_per_hit: Some(8_500),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_13", name: "レリックの聖域 13段", enemy_id: Some("relic_sanctuary_13"), need_per_hit: Some(9_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_14", name: "レリックの聖域 14段", enemy_id: Some("relic_sanctuary_14"), need_per_hit: Some(10_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_15", name: "レリックの聖域 15段", enemy_id: Some("relic_sanctuary_15"), need_per_hit: Some(10_500),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_16", name: "レリックの聖域 16段", enemy_id: Some("relic_sanctuary_16"), need_per_hit: Some(11_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_17", name: "レリックの聖域 17段", enemy_id: Some("relic_sanctuary_17"), need_per_hit: Some(13_000),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_18", name: "レリックの聖域 18段", enemy_id: Some("relic_sanctuary_18"), need_per_hit: Some(13_500),
              requirements: &[], entry_note: None, team_note: None },
        Def { id: "relic_sanctuary_19", name: "レリックの聖域 19段", enemy_id: Some("relic_sanctuary_19"), need_per_hit: Some(14_000),
              requirements: &[], entry_note: None, team_note: None },
    ]),
];

/// エリアごとのコンテンツ一覧。表示順 = この配列の順(swiki の節順 = おおむね難度昇順)。
pub fn content_areas() -> Vec<ContentArea> {
    AREAS
        .iter()
        .map(|(id, name, defs)| ContentArea {
            id: (*id).to_string(),
            name: (*name).to_string(),
            contents: defs.iter().map(Def::to_content).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enemies::{enemies, find_enemy};

    fn all_contents() -> Vec<Content> {
        content_areas().into_iter().flat_map(|a| a.contents).collect()
    }

    #[test]
    fn コンテンツidとエリアidは一意() {
        let mut ids: Vec<String> = all_contents().into_iter().map(|c| c.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "コンテンツ id が重複している");

        let mut area_ids: Vec<String> = content_areas().into_iter().map(|a| a.id).collect();
        let n = area_ids.len();
        area_ids.sort_unstable();
        area_ids.dedup();
        assert_eq!(area_ids.len(), n);
    }

    #[test]
    fn enemy_idは敵カタログに存在しneedと対で持つ() {
        for c in all_contents() {
            match (&c.enemy_id, c.need_per_hit) {
                (Some(id), Some(need)) => {
                    assert!(find_enemy(id).is_some(), "enemy_id '{id}' が enemies.rs に無い");
                    assert!(need > 0, "'{}' の目安ダメージが 0 以下", c.id);
                }
                (None, None) => {}
                _ => panic!("'{}' は enemy_id と need_per_hit を対で持つべき", c.id),
            }
        }
    }

    #[test]
    fn 全敵がいずれかのコンテンツから参照される() {
        let referenced: Vec<String> = all_contents().into_iter().filter_map(|c| c.enemy_id).collect();
        for enemy in enemies() {
            assert!(referenced.contains(&enemy.id), "敵 '{}' を参照するコンテンツが無い", enemy.id);
        }
    }

    #[test]
    fn 装備条件は各コンテンツに高々1件() {
        for c in all_contents() {
            let n = c
                .requirements
                .iter()
                .filter(|r| matches!(r, ContentRequirement::EquipmentBySkill { .. }))
                .count();
            assert!(n <= 1, "'{}' に装備条件が複数ある(スキル依存で 1 件に畳む設計)", c.id);
        }
    }

    #[test]
    fn swiki表の代表値を転記できている() {
        let by_id = |id: &str| all_contents().into_iter().find(|c| c.id == id).unwrap();

        // リンゴ: 覚醒3・S/H/I 800・M 980・複合 "-"(=0)
        let ringo = by_id("ringo");
        assert!(ringo.requirements.contains(&stage(3)));
        assert!(ringo.requirements.contains(&equip(800, 980, 0)));

        // アビスEX: 5次覚醒・1500/1700/2100、エタ条件なし("-")
        let ex = by_id("abyss_ex");
        assert!(ex.requirements.contains(&equip(1_500, 1_700, 2_100)));
        assert!(!ex.requirements.iter().any(|r| matches!(r, ContentRequirement::EternalLevel(_))));

        // 追従する喜び(ハード) = レイティアH: エタ61・3900/4000/5900
        let leitia_h = by_id("leitia_h");
        assert_eq!(leitia_h.enemy_id.as_deref(), Some("leitia_h"));
        assert!(leitia_h.requirements.contains(&eternal(61)));
        assert!(leitia_h.requirements.contains(&equip(3_900, 4_000, 5_900)));

        // 見つめる悲しみ(ノーマル) = 設計者N
        assert_eq!(by_id("architect_n").enemy_id.as_deref(), Some("architect_n"));

        // エタのみ条件のコンテンツは装備条件を持たない
        let vestige = by_id("vestige_ruins");
        assert_eq!(vestige.requirements, vec![eternal(1)]);
    }
}
