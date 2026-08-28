//! コンテンツカタログ(エリア → コンテンツ)。ホームの到達一覧・入場条件判定に使う。
//!
//! 入場条件は swiki「コンテンツ入場条件」(<https://erumisutoburvip.swiki.jp/>、取得 2026-08-24)を
//! 正とする。表の列 S/H/I(突き/斬り/魔攻)・M(魔防)・複合(突斬 or 斬魔)は、使うスキルの
//! 依存種別で比較先が決まる別条件で「いずれかを満たしていれば OK」(表の概要、ユーザー確認済み)。
//! これを `ContentRequirement::EquipmentBySkill` の 1 件で表し、判定時にスキル依存で
//! 比較先を選ぶ。表の "-" は 0(その系統の条件なし)。
//!
//! 判定できない条件(ルーンレベル・共通スキルコンプ・前提クリア)は `entry_note` に
//! 表示専用で持つ(キャラモデルに値が無く、条件にすると「常に未達」か「常に無視」の
//! 嘘になるため)。表の「コア N」はテシスコアの火力補正合計と一致するため
//! (wiki 進化強化表: 60 = 6枠×10、120 = 6×20、210 = 6×35、300 = 6×50、480 = 6×80)、
//! `ContentRequirement::ThesisCoreTotal` として実判定する。
//!
//! 敵データ(`enemies.rs`)があるコンテンツは火力も判定する。敵データが無いコンテンツは
//! `enemy_id`/`need_per_hit` が None で、入場条件のみで判定する。
//! 目安ダメージ(need_per_hit)はコミュニティ知識・実測がソース。wiki 狩り場情報一覧には
//! 対応する列が無く(2026-08-25 再確認)、wiki の取り込みでは埋まらない項目なので
//! `[仮]`(= wiki 待ち)ではなく「出典がコミュニティ知識」として運用する。

use domain::content::{Content, ContentArea, ContentRequirement, ContentSeries, GameRegion};
use domain::thesis_core::CoreRegion;

use crate::Source;

pub const CONTENTS_SOURCE: Source = Source {
    page: "swiki コンテンツ入場条件(入場条件・コンテンツ構成)+ 暫定値(目安ダメージ)",
    retrieved_on: "2026-08-24",
    note: "装備条件 S/H/I・M・複合はスキル依存で比較先が決まる(いずれか 1 つ充足で OK)。「コア N」はテシスコアの火力補正合計として実判定する。判定できない条件(ルーン Lv・共通スキル等)は entry_note に表示専用。目安ダメージは全件 [仮]",
};

/// 装備補正条件。`single` = S/H/I 列、`mr` = M 列、`composite` = 複合列(表の "-" は 0)。
const fn equip(single: i64, mr: i64, composite: i64) -> ContentRequirement {
    ContentRequirement::EquipmentBySkill {
        single,
        mr,
        composite,
    }
}

const fn stage(v: u8) -> ContentRequirement {
    ContentRequirement::AwakeningStage(v)
}

const fn eternal(v: u8) -> ContentRequirement {
    ContentRequirement::EternalLevel(v)
}

/// swiki の「コア N」= テシスコアの火力補正合計(wiki 進化強化表と一致)。
const fn core(v: i64) -> ContentRequirement {
    ContentRequirement::ThesisCoreTotal(v)
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
            core_region: core_region_of(self.id),
            game_region: game_region_of(self.id),
            series: series_of(self.id),
            entry_note: self.entry_note.map(String::from),
            team_note: self.team_note.map(String::from),
        }
    }
}

/// 称号など、テシスコアとは別のゲーム内地域限定効果に使う対応。
pub fn game_region_of(content_id: &str) -> Option<GameRegion> {
    use GameRegion::*;
    match content_id {
        "eclipse_boss" | "eclipse_2" | "eclipse_subjugation" | "lost_forest" => Some(LostIsland),
        "shinchou_normal" | "shinchou_hard" | "relic_sanctuary_shinchou" => Some(ShinchouNest),
        "arklon_underground" => Some(ArklonUnderground),
        _ => None,
    }
}

/// 段数違いの系列(一覧では 1 行 + 難易度ステッパーに畳む)。
///
/// id の接頭辞 + 末尾の数値で機械的に決める。系列を手で並べた表にすると、段を足したときに
/// 2 箇所直すことになる。現在の系列は「レリックの聖域 10段〜19段」のみ。
const SERIES: &[(&str, &str)] = &[("relic_sanctuary_", "レリックの聖域")];

fn series_of(id: &str) -> Option<ContentSeries> {
    SERIES.iter().find_map(|(prefix, name)| {
        let step = id.strip_prefix(prefix)?.parse::<u32>().ok()?;
        Some(ContentSeries {
            id: prefix.trim_end_matches('_').to_string(),
            name: name.to_string(),
            step,
        })
    })
}

/// テシスコアの能力値増加が有効なコンテンツと地域の対応。
///
/// 出典は wiki「テシスコア」の実装済みダンジョンコア表(コアと「その他発動場所」の対応):
/// - マーキュリアル洞窟: マーキュリアル洞窟 / ルミナスの試練 / プシーキーの迷宮 / プシーキーの虚像
/// - アビス: アビス / アークロン要塞 / 守護者の部屋(レイド)/ 深淵の狭間 / アークロン地下
/// - エクリプス: エクリプスダンジョン / アフェティリアダンジョン
/// - ルビコナ: ゆがんだ村(狩り場情報一覧の「ゆがんだ村」節 = 空虚の領域・レイティア・設計者)
///
/// wiki の表に名前が無いがユーザー確認済み(2026-08-24): リンゴ = マーキュリアル、
/// 月の女王の軍の訓練所・最後の決戦(1/2 含む)・異界の峡谷 兵士 = エクリプス、
/// 混乱した大地・異界の峡谷防衛戦・喜びの残像 = ルビコナ。
/// 「異界の峡谷」は防衛戦がルビコナ、通常の峡谷(兵士)がエクリプスで分かれる(同確認)。
///
/// **ここに無いコンテンツはコア効果が無い**(ユーザー確認 2026-08-24。コアの効かない
/// コンテンツが実在する)。セット効果だけは全地域で発動するので別枠で常に乗る。
/// シオカンヘイムのコアは経験値タイプのみで火力に効かないため、地域自体を持たない(同確認)。
const CORE_REGIONS: &[(&str, CoreRegion)] = &[
    // マーキュリアル洞窟(ルミナスの試練。リンゴはユーザー確認)
    ("ringo", CoreRegion::Mercurial),
    ("luminous_ex", CoreRegion::Mercurial),
    // アビス / アークロン地下
    ("abyss_normal", CoreRegion::Abyss),
    ("abyss_hard", CoreRegion::Abyss),
    ("abyss_hell", CoreRegion::Abyss),
    ("abyss_ex", CoreRegion::Abyss),
    ("arklon_underground", CoreRegion::Abyss),
    // エクリプス / アフェティリア(月の女王の軍の訓練所・最後の決戦・異界の峡谷 兵士はユーザー確認)
    ("moon_queen_training", CoreRegion::Eclipse),
    ("last_battle", CoreRegion::Eclipse),
    ("last_battle_1", CoreRegion::Eclipse),
    ("last_battle_2", CoreRegion::Eclipse),
    ("valley_soldier", CoreRegion::Eclipse),
    ("eclipse_boss", CoreRegion::Eclipse),
    ("eclipse_2", CoreRegion::Eclipse),
    ("eclipse_subjugation", CoreRegion::Eclipse),
    ("aphetiria_normal", CoreRegion::Eclipse),
    ("aphetiria_hard", CoreRegion::Eclipse),
    ("aphetiria_ex", CoreRegion::Eclipse),
    ("selinacos_h", CoreRegion::Eclipse),
    ("selinacos_ex", CoreRegion::Eclipse),
    ("goitia_h", CoreRegion::Eclipse),
    ("goitia_ex", CoreRegion::Eclipse),
    // ルビコナ(ゆがんだ村。混乱した大地・異界の峡谷・喜びの残像はユーザー確認)
    ("chaotic_land", CoreRegion::Rubicona),
    ("valley_defense", CoreRegion::Rubicona),
    ("pleasure_afterimage", CoreRegion::Rubicona),
    ("void_domain", CoreRegion::Rubicona),
    ("leitia_n", CoreRegion::Rubicona),
    ("leitia_h", CoreRegion::Rubicona),
    ("architect_n", CoreRegion::Rubicona),
    ("architect_h", CoreRegion::Rubicona),
];

/// コンテンツ id → テシスコアの地域。表に無ければ None(コアの能力値増加は乗らない)。
pub fn core_region_of(content_id: &str) -> Option<CoreRegion> {
    CORE_REGIONS
        .iter()
        .find(|(id, _)| *id == content_id)
        .map(|(_, region)| *region)
}

/// 下位コンテンツ共通の注記(swiki: リンゴと煩わしい怒り以外はルーンレベル30 必要)。
const RUNE30: Option<&str> = Some("ルーンレベル 30 必要(判定対象外)");
/// 上位コンテンツ共通の注記(swiki: 5次覚醒・ルーンレベル40・共通スキルコンプリート)。
const UPPER: &str = "ルーンレベル 40・共通スキルコンプリート必要(判定対象外)";

/// 上位コンテンツの entry_note(判定対象外の共通条件のみ。コア要求は `core()` で実判定する)。
const UPPER_NOTE: Option<&str> = Some(UPPER);

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
              requirements: &[stage(3), equip(900, 1_300, 1_500), core(60)],
              entry_note: RUNE30, team_note: None },
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
              requirements: &[STAGE5, equip(1_500, 1_700, 2_100), core(120)],
              entry_note: UPPER_NOTE, team_note: Some("改IHは不要") },
        Def { id: "eclipse_boss", name: "エクリプスボス", enemy_id: Some("eclipse_1"), need_per_hit: Some(6_000),
              requirements: &[STAGE5, equip(1_600, 1_800, 2_350)],
              entry_note: UPPER_NOTE, team_note: Some("ソロは入場条件よりもだいぶ難易度低い") },
        Def { id: "aphetiria_normal", name: "アフェティリア(ノーマル)", enemy_id: Some("aphetiria_n"), need_per_hit: Some(7_000),
              requirements: &[STAGE5, eternal(5), equip(1_600, 1_800, 2_350)],
              entry_note: UPPER_NOTE, team_note: Some("ソロの場合エタ制限のみだがソロはきつい") },
        Def { id: "moon_queen_training", name: "月の女王の軍の訓練所", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900), core(120)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "eclipse_subjugation", name: "エクリプスボス討伐戦", enemy_id: Some("eclipse_subjugation"), need_per_hit: Some(12_000),
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900), core(120)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "aphetiria_hard", name: "アフェティリア(ハード)", enemy_id: Some("kisinik_h"), need_per_hit: Some(16_000),
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900), core(120)],
              entry_note: UPPER_NOTE, team_note: Some("活躍するには靴エフェ合わせて 400 くらいほしい") },
        Def { id: "relic_sanctuary_kisinik", name: "古代レリックの聖域(キシニク)", enemy_id: Some("relic_sanctuary_20"), need_per_hit: Some(16_000),
              requirements: &[STAGE5, eternal(10), equip(1_700, 1_900, 2_900)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "chaotic_land", name: "混乱した大地", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(20), equip(2_200, 2_600, 3_500)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "colorless_land", name: "色を失った大地", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(20), equip(2_200, 2_600, 3_500)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "architect_mine", name: "設計者の採掘場", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(20), equip(2_200, 2_600, 3_500)],
              entry_note: Some("ルーンレベル 40・共通スキルコンプリート・カフス(盾+)の上限 140 以上 必要(判定対象外)"), team_note: None },
        Def { id: "valley_defense", name: "異界の峡谷防衛戦", enemy_id: Some("valley_captain"), need_per_hit: Some(12_000),
              requirements: &[STAGE5, eternal(21), equip(2_500, 2_700, 3_700), core(300)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "last_battle", name: "最後の決戦", enemy_id: Some("last_battle_3"), need_per_hit: Some(15_000),
              requirements: &[STAGE5, eternal(21), equip(2_500, 2_700, 3_700), core(300)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "aphetiria_ex", name: "アフェティリアEX", enemy_id: Some("kisinik_ex"), need_per_hit: Some(20_000),
              requirements: &[STAGE5, eternal(41), equip(2_500, 3_000, 4_000), core(480)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "void_domain", name: "空虚の領域", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(41), equip(3_100, 3_500, 4_900)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "leitia_n", name: "追従する喜び(ノーマル)", enemy_id: Some("leitia_n"), need_per_hit: Some(18_000),
              requirements: &[STAGE5, eternal(41), equip(3_100, 3_500, 4_900)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "architect_n", name: "見つめる悲しみ(ノーマル)", enemy_id: Some("architect_n"), need_per_hit: Some(18_000),
              requirements: &[STAGE5, eternal(41), equip(3_100, 3_500, 4_900), core(60)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "pleasure_afterimage", name: "喜びの残像", enemy_id: None, need_per_hit: None,
              requirements: &[STAGE5, eternal(51), equip(3_500, 3_850, 5_500), core(60)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "leitia_h", name: "追従する喜び(ハード)", enemy_id: Some("leitia_h"), need_per_hit: Some(22_000),
              requirements: &[STAGE5, eternal(61), equip(3_900, 4_000, 5_900), core(120)],
              entry_note: UPPER_NOTE, team_note: None },
        Def { id: "architect_h", name: "見つめる悲しみ(ハード)", enemy_id: Some("architect_h"), need_per_hit: Some(22_000),
              requirements: &[STAGE5, eternal(61), equip(3_900, 4_000, 5_900), core(210)],
              entry_note: UPPER_NOTE, team_note: None },
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
        content_areas()
            .into_iter()
            .flat_map(|a| a.contents)
            .collect()
    }

    /// 系列は id の接頭辞 + 末尾の数値で決まる。数値でない末尾(神鳥・キシニク)は系列に入れない。
    #[test]
    fn レリックの聖域は10段から19段の系列になる() {
        let series: Vec<_> = all_contents()
            .into_iter()
            .filter_map(|c| c.series.map(|s| (s.id, s.step)))
            .collect();
        assert!(series.iter().all(|(id, _)| id == "relic_sanctuary"));
        let mut steps: Vec<u32> = series.iter().map(|(_, step)| *step).collect();
        steps.sort_unstable();
        assert_eq!(steps, (10..=19).collect::<Vec<_>>());

        // 末尾が数値でないものは系列に入らない(1 行に畳むと別コンテンツが混ざる)
        let standalone = all_contents()
            .into_iter()
            .find(|c| c.id == "relic_sanctuary_kisinik")
            .unwrap();
        assert_eq!(standalone.series, None);
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
                    assert!(
                        find_enemy(id).is_some(),
                        "enemy_id '{id}' が enemies.rs に無い"
                    );
                    assert!(need > 0, "'{}' の目安ダメージが 0 以下", c.id);
                }
                (None, None) => {}
                _ => panic!("'{}' は enemy_id と need_per_hit を対で持つべき", c.id),
            }
        }
    }

    #[test]
    fn 全敵がいずれかのコンテンツから参照される() {
        let referenced: Vec<String> = all_contents()
            .into_iter()
            .filter_map(|c| c.enemy_id)
            .collect();
        for enemy in enemies() {
            assert!(
                referenced.contains(&enemy.id),
                "敵 '{}' を参照するコンテンツが無い",
                enemy.id
            );
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
            assert!(
                n <= 1,
                "'{}' に装備条件が複数ある(スキル依存で 1 件に畳む設計)",
                c.id
            );
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
        assert!(!ex
            .requirements
            .iter()
            .any(|r| matches!(r, ContentRequirement::EternalLevel(_))));

        // 追従する喜び(ハード) = レイティアH: エタ61・3900/4000/5900
        let leitia_h = by_id("leitia_h");
        assert_eq!(leitia_h.enemy_id.as_deref(), Some("leitia_h"));
        assert!(leitia_h.requirements.contains(&eternal(61)));
        assert!(leitia_h.requirements.contains(&equip(3_900, 4_000, 5_900)));

        // 見つめる悲しみ(ノーマル) = 設計者N
        assert_eq!(
            by_id("architect_n").enemy_id.as_deref(),
            Some("architect_n")
        );

        // エタのみ条件のコンテンツは装備条件を持たない
        let vestige = by_id("vestige_ruins");
        assert_eq!(vestige.requirements, vec![eternal(1)]);
    }

    #[test]
    fn コア要求は実条件として持ちテシスコア合計で判定する() {
        let areas = content_areas();
        let by_id = |id: &str| {
            areas
                .iter()
                .flat_map(|a| &a.contents)
                .find(|c| c.id == id)
                .unwrap()
                .clone()
        };

        // swiki「コア 480」= 6 枠すべて進化4強化4(火力補正 80 × 6)
        let ex = by_id("aphetiria_ex");
        assert!(ex
            .requirements
            .contains(&ContentRequirement::ThesisCoreTotal(480)));
        // コア要求は entry_note から外し、判定できない条件だけを注記に残す
        assert_eq!(ex.entry_note.as_deref(), Some(UPPER));

        assert!(by_id("luminous_ex")
            .requirements
            .contains(&ContentRequirement::ThesisCoreTotal(60)));
        assert!(by_id("abyss_ex")
            .requirements
            .contains(&ContentRequirement::ThesisCoreTotal(120)));
        assert!(by_id("architect_h")
            .requirements
            .contains(&ContentRequirement::ThesisCoreTotal(210)));
        assert!(by_id("last_battle")
            .requirements
            .contains(&ContentRequirement::ThesisCoreTotal(300)));
        // コア要求が無いコンテンツには条件を足さない
        assert!(!by_id("ringo")
            .requirements
            .iter()
            .any(|r| matches!(r, ContentRequirement::ThesisCoreTotal(_))));
    }

    // 「コア N」はそのコンテンツの地域のコアだけで判定する(ユーザー確認 2026-08-24)ため、
    // 地域が無いまま要求だけあると「常に未達」の嘘になる。
    #[test]
    fn コア要求のあるコンテンツは必ず地域を持つ() {
        for area in content_areas() {
            for content in area.contents {
                let requires_core = content
                    .requirements
                    .iter()
                    .any(|r| matches!(r, ContentRequirement::ThesisCoreTotal(n) if *n > 0));
                if requires_core {
                    assert!(
                        content.core_region.is_some(),
                        "'{}' はコア要求があるのに core_region が無い",
                        content.id
                    );
                }
            }
        }
    }

    #[test]
    fn テシスコアの地域はwikiの発動場所どおり() {
        let areas = content_areas();
        let region = |id: &str| {
            areas
                .iter()
                .flat_map(|a| &a.contents)
                .find(|c| c.id == id)
                .unwrap()
                .core_region
        };
        assert_eq!(region("luminous_ex"), Some(CoreRegion::Mercurial));
        assert_eq!(region("abyss_ex"), Some(CoreRegion::Abyss));
        assert_eq!(region("arklon_underground"), Some(CoreRegion::Abyss));
        assert_eq!(region("aphetiria_ex"), Some(CoreRegion::Eclipse));
        assert_eq!(region("leitia_h"), Some(CoreRegion::Rubicona));
        assert_eq!(region("void_domain"), Some(CoreRegion::Rubicona));
        // wiki の表に無いがユーザー確認済み
        assert_eq!(region("ringo"), Some(CoreRegion::Mercurial));
        assert_eq!(region("chaotic_land"), Some(CoreRegion::Rubicona));
        assert_eq!(region("moon_queen_training"), Some(CoreRegion::Eclipse));
        assert_eq!(region("last_battle"), Some(CoreRegion::Eclipse));
        // 異界の峡谷は防衛戦(ルビコナ)と通常の峡谷(エクリプス)で分かれる
        assert_eq!(region("valley_defense"), Some(CoreRegion::Rubicona));
        assert_eq!(region("valley_soldier"), Some(CoreRegion::Eclipse));
        assert_eq!(region("last_battle_1"), Some(CoreRegion::Eclipse));
        assert_eq!(region("last_battle_2"), Some(CoreRegion::Eclipse));
        assert_eq!(region("pleasure_afterimage"), Some(CoreRegion::Rubicona));
        // コア効果が無いコンテンツ(ユーザー確認)
        assert_eq!(region("odin_total_war"), None);
        assert_eq!(region("shinchou_normal"), None);

        // 対応表の id はすべて実在する(タイポで黙って無効化されないように)
        for (id, _) in CORE_REGIONS {
            assert!(
                areas.iter().flat_map(|a| &a.contents).any(|c| c.id == *id),
                "CORE_REGIONS の '{id}' に対応するコンテンツが無い"
            );
        }
    }
    #[test]
    fn ゲーム内地域は称号の対象だけに限定する() {
        assert_eq!(game_region_of("eclipse_boss"), Some(GameRegion::LostIsland));
        assert_eq!(game_region_of("eclipse_2"), Some(GameRegion::LostIsland));
        assert_eq!(
            game_region_of("eclipse_subjugation"),
            Some(GameRegion::LostIsland)
        );
        assert_eq!(game_region_of("lost_forest"), Some(GameRegion::LostIsland));
        assert_eq!(
            game_region_of("shinchou_normal"),
            Some(GameRegion::ShinchouNest)
        );
        assert_eq!(
            game_region_of("shinchou_hard"),
            Some(GameRegion::ShinchouNest)
        );
        assert_eq!(
            game_region_of("relic_sanctuary_shinchou"),
            Some(GameRegion::ShinchouNest)
        );
        assert_eq!(
            game_region_of("arklon_underground"),
            Some(GameRegion::ArklonUnderground)
        );
        // テシスコアでは同じアビス地域でも、死の騎士の対象には広げない。
        assert_eq!(game_region_of("abyss_hell"), None);
        // 喪失の島称号はアフェティリア等には広げない。
        assert_eq!(game_region_of("aphetiria_ex"), None);
    }
}
