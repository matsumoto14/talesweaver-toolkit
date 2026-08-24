//! 敵カタログ。
//!
//! 収録範囲はユーザー提供の実測表「モンスター能力値リスト」(2026-08-24、ステータス防御/
//! 固定防御/固定減少/カット率/属性値/HP。レイティア・設計者 4 行は「一部修正」版が正)の
//! 実データ全件。値は旧リポ twtoolkit の monsters.json(28 体)と wiki「狩り場情報一覧」
//! (取得 2026-08-24)でも裏取りし、3 ソースの重複分はすべて一致
//! (唯一の差分: 最後の決戦2 のカット率は実測表の 70% を採用)。
//! wiki にしか無い敵(被害減少が不明)は収録しない(ユーザー決定 2026-08-24)。
//!
//! - 防御力は実測表の「ステータス防御+固定防御」の合計値で持つ
//! - 被害減少(damage_reduction)は実測表の固定減少(0 / 2925 / 3250 / 4550 / 5850)を
//!   符号反転して負値で保持。wiki の「6500 固定と思われる」は実測表と不一致のため不採用
//! - カット率 x%(実測表)/ -x%(wiki)は乗数 (1 - x/100) で持つ(例: 59.5% → 0.405)
//! - `element_threshold` は wiki 狩り場情報一覧の「敵属性値」(2026-08-25 に意味を確定。
//!   攻撃スキルの属性値がこれを上回った分だけ与ダメージが増える)。120 / 125 / 90 の 3 値
//! - HP(ソロ)は実測表にあるが消費する機能が無いため未収録(必要になったら追加)

use domain::Enemy;

use crate::Source;

pub const ENEMIES_SOURCE: Source = Source {
    page: "wiki 狩り場情報一覧 + 実測表「モンスター能力値リスト」+ 旧リポ twtoolkit monsters.json",
    retrieved_on: "2026-08-25",
    note: "被害減少は符号反転して負値で保持、カット率は乗数で保持。2026-08-25 に wiki を再取得して重複分を再照合し、全件一致(最後の決戦2 のカット率のみ実測表 70% を採用)。wiki にしか無い敵と、wiki にあって未消費の列は docs/damage-formula.md §10 参照",
};

struct EnemyRecord {
    id: &'static str,
    name: &'static str,
    defense: i64,
    damage_reduction: i64,
    cut_rate_a: f64,
    element_threshold: i64,
}

#[rustfmt::skip]
const ENEMIES: &[EnemyRecord] = &[
    // ---- リンゴの島 ----
    // 旧リポ。カット率 0.52 は wiki ルミナス -48%(ソロ)と一致。防 8700 は wiki 未記載(1800+6900 ならルミナスと整合)
    EnemyRecord { id: "ringo_boss", name: "リンゴボス", defense: 8700, damage_reduction: 0, cut_rate_a: 0.52, element_threshold: 120 },
    // ---- アークロン要塞 / アビス ----
    // 旧リポ。wiki 狩り場情報一覧に対応行なし(スフォルツェンド -91% とは別個体)
    EnemyRecord { id: "arklon_underground", name: "アークロン地下要塞", defense: 4350, damage_reduction: 0, cut_rate_a: 0.18, element_threshold: 120 },
    // wiki 裏取り済み: 防1500+8100=9600(ソロ)、カット率 -75% → 0.25、属性 120
    EnemyRecord { id: "abyss_hell", name: "アビスヘル", defense: 9600, damage_reduction: 0, cut_rate_a: 0.25, element_threshold: 120 },
    // wiki 裏取り済み: 防1500+8700=10200、カット率 -75% → 0.25
    EnemyRecord { id: "abyss_core_master", name: "アビスコアマスター", defense: 10200, damage_reduction: 0, cut_rate_a: 0.25, element_threshold: 120 },
    // ---- レイドボス ----
    // 旧リポ。wiki 狩り場情報一覧に対応行なし。属性閾値 90 は旧リポ知見(threshold=90 はトゥタトゥールのみ)
    EnemyRecord { id: "tutatur", name: "トゥタトゥール", defense: 990, damage_reduction: 0, cut_rate_a: 1.0, element_threshold: 90 },
    // 旧リポ。wiki 狩り場情報一覧に対応行なし
    EnemyRecord { id: "clamor", name: "クラモール", defense: 16500, damage_reduction: 0, cut_rate_a: 0.18, element_threshold: 125 },
    // 旧リポ。武器ダメージ無効(weaponTerm=0)の特殊挙動は未モデル `[仮]`
    EnemyRecord { id: "chimera", name: "キマイラ", defense: 57900, damage_reduction: 0, cut_rate_a: 0.4225, element_threshold: 120 },
    // ---- シオカンヘイム ----
    // wiki 裏取り済み: 防1050+6000=7050、カット率 -59.5% → 0.405、属性 120、被害減少 有
    EnemyRecord { id: "brothers_forge", name: "兄弟の鍛冶場", defense: 7050, damage_reduction: -5850, cut_rate_a: 0.405, element_threshold: 120 },
    // 旧リポ。wiki「シオカンヘイム ボス」のカット率 -51% → 0.49 と一致
    EnemyRecord { id: "siokan_boss", name: "シオカンボス", defense: 35220, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 125 },
    // wiki 裏取り済み: 防 53500 以下(旧リポ 53220 と整合)、カット率 -51% → 0.49、属性 125、被害減少 有
    EnemyRecord { id: "odin", name: "オーディン", defense: 53220, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 125 },
    // 旧リポ(ランキング戦個体)。wiki は通常オーディンのみ記載
    EnemyRecord { id: "odin_rank", name: "オーディン(ランク)", defense: 59220, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 125 },
    // ---- エクリプス ----
    // 旧リポ。wiki「エクリプス ボス(ソロ)」= 約41640・カット率 -51% → 0.49 と整合(個体差あり)
    EnemyRecord { id: "eclipse_1", name: "エクリプス1", defense: 41220, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 125 },
    EnemyRecord { id: "eclipse_2", name: "エクリプス2", defense: 42720, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 125 },
    // 旧リポ。wiki「討伐戦ボス」のカット率 -68.5% → 0.315 と一致
    EnemyRecord { id: "eclipse_subjugation", name: "エクリプス討伐戦", defense: 62700, damage_reduction: -4550, cut_rate_a: 0.315, element_threshold: 125 },
    // 旧リポ。wiki 狩り場情報一覧に対応行なし(エクリプス地域の派生コンテンツ)
    EnemyRecord { id: "lost_forest", name: "喪失の森", defense: 37350, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 120 },
    // ---- エタ(異界の峡谷 / 最後の決戦) ----
    EnemyRecord { id: "valley_soldier", name: "異界の峡谷 兵士", defense: 73500, damage_reduction: 0, cut_rate_a: 0.42, element_threshold: 125 },
    EnemyRecord { id: "valley_captain", name: "異界の峡谷 部隊長", defense: 82500, damage_reduction: -4550, cut_rate_a: 0.315, element_threshold: 125 },
    // 実測表 備考: 決戦1=ロカゴス・チェリア / 決戦2=ティロロス・ゴイティア / 決戦3=召喚の石像
    EnemyRecord { id: "last_battle_1", name: "最後の決戦1", defense: 82500, damage_reduction: -4550, cut_rate_a: 0.315, element_threshold: 125 },
    // 実測表のカット率 70% を採用(旧リポは 0.315。3 ソース中唯一の差分)
    EnemyRecord { id: "last_battle_2", name: "最後の決戦2", defense: 82500, damage_reduction: -4550, cut_rate_a: 0.30, element_threshold: 125 },
    EnemyRecord { id: "last_battle_3", name: "最後の決戦3", defense: 106500, damage_reduction: -4550, cut_rate_a: 0.315, element_threshold: 125 },
    // ---- アフェティリア ----
    // 旧リポ。wiki「アフェティリアNボス」のカット率 -51%(ソロ) → 0.49 と一致
    EnemyRecord { id: "aphetiria_n", name: "アフェティリア(N)", defense: 43200, damage_reduction: -4550, cut_rate_a: 0.49, element_threshold: 125 },
    // wiki カット率一致: セリニアコス(H) -65% → 0.35 / ゴイティア(H) -68.5% → 0.315 / キシニク(H) -72% → 0.28
    EnemyRecord { id: "selinacos_h", name: "セリニアコス(H)", defense: 61200, damage_reduction: -4550, cut_rate_a: 0.35, element_threshold: 125 },
    EnemyRecord { id: "goitia_h", name: "ゴイティア(H)", defense: 62700, damage_reduction: -4550, cut_rate_a: 0.315, element_threshold: 125 },
    EnemyRecord { id: "kisinik_h", name: "キシニク(H)", defense: 65700, damage_reduction: -4550, cut_rate_a: 0.28, element_threshold: 125 },
    // 旧リポ(EX)。wiki 狩り場情報一覧はアフェティリア EX の防御値未記載
    EnemyRecord { id: "selinacos_ex", name: "セリニアコス(EX)", defense: 118200, damage_reduction: -5850, cut_rate_a: 0.35, element_threshold: 125 },
    EnemyRecord { id: "goitia_ex", name: "ゴイティア(EX)", defense: 118200, damage_reduction: -5850, cut_rate_a: 0.315, element_threshold: 125 },
    EnemyRecord { id: "kisinik_ex", name: "キシニク(EX)", defense: 119550, damage_reduction: -5850, cut_rate_a: 0.28, element_threshold: 125 },
    // ---- 古代レリックの聖域 ----
    // 実測表(防 = 1500+固定防御。4 つのミニゲームクリア時のステータス)。20 段は旧リポとも一致
    EnemyRecord { id: "relic_sanctuary_10", name: "レリックの聖域10", defense: 38640, damage_reduction: 0, cut_rate_a: 0.49, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_11", name: "レリックの聖域11", defense: 59700, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_12", name: "レリックの聖域12", defense: 62280, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_13", name: "レリックの聖域13", defense: 64110, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_14", name: "レリックの聖域14", defense: 77790, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_15", name: "レリックの聖域15", defense: 78450, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_16", name: "レリックの聖域16", defense: 79110, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_17", name: "レリックの聖域17", defense: 101790, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_18", name: "レリックの聖域18", defense: 103620, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_19", name: "レリックの聖域19", defense: 105540, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    EnemyRecord { id: "relic_sanctuary_20", name: "レリックの聖域20", defense: 108360, damage_reduction: 0, cut_rate_a: 0.28, element_threshold: 125 },
    // ---- ゆがんだ村 ----
    // 実測表「一部修正」版: N = 防1950+105000=106950・固定減少 3250・カット率 67.5%
    //                      H = 防1950+130200(レイティア)/130800(設計者)・固定減少 2925・カット率 70.75%
    EnemyRecord { id: "leitia_n", name: "レイティアN", defense: 106950, damage_reduction: -3250, cut_rate_a: 0.325, element_threshold: 125 },
    EnemyRecord { id: "architect_n", name: "設計者N", defense: 106950, damage_reduction: -3250, cut_rate_a: 0.325, element_threshold: 125 },
    EnemyRecord { id: "leitia_h", name: "レイティアH", defense: 132150, damage_reduction: -2925, cut_rate_a: 0.2925, element_threshold: 125 },
    EnemyRecord { id: "architect_h", name: "設計者H", defense: 132300, damage_reduction: -2925, cut_rate_a: 0.2925, element_threshold: 125 },
];

impl EnemyRecord {
    fn to_enemy(&self) -> Enemy {
        Enemy {
            id: self.id.to_string(),
            name: self.name.to_string(),
            defense: self.defense,
            damage_reduction: self.damage_reduction,
            cut_rate_a: self.cut_rate_a,
            element_threshold: self.element_threshold,
        }
    }
}

pub fn enemies() -> Vec<Enemy> {
    ENEMIES.iter().map(EnemyRecord::to_enemy).collect()
}

pub fn find_enemy(id: &str) -> Option<Enemy> {
    ENEMIES.iter().find(|e| e.id == id).map(EnemyRecord::to_enemy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 敵は42体でidは一意() {
        let all = enemies();
        assert_eq!(all.len(), 42);
        let mut ids: Vec<&str> = ENEMIES.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 42);
    }

    #[test]
    fn id_で検索でき被害減少は負値() {
        let e = find_enemy("brothers_forge").unwrap();
        assert_eq!(e.defense, 7050);
        assert_eq!(e.damage_reduction, -5850);
        assert!((e.cut_rate_a - 0.405).abs() < 1e-12);
        assert!(find_enemy("nope").is_none());
        assert!(enemies().iter().all(|e| e.damage_reduction <= 0));
    }

    #[test]
    fn wiki裏取り値のスポットチェック() {
        // wiki 狩り場情報一覧(2026-08-24): 防1500+8100=9600(ソロ)、-75% → 0.25
        let hell = find_enemy("abyss_hell").unwrap();
        assert_eq!(hell.defense, 9600);
        assert!((hell.cut_rate_a - 0.25).abs() < 1e-12);
        // wiki: オーディン -51% → 0.49、防 53500 以下
        let odin = find_enemy("odin").unwrap();
        assert!(odin.defense <= 53_500);
        assert!((odin.cut_rate_a - 0.49).abs() < 1e-12);
        // カット率は乗数として 0 < v <= 1
        assert!(enemies().iter().all(|e| e.cut_rate_a > 0.0 && e.cut_rate_a <= 1.0));
    }

    #[test]
    fn 実測表由来の値のスポットチェック() {
        // 最後の決戦2: 実測表のカット率 70% で旧リポ値を上書き
        let lb2 = find_enemy("last_battle_2").unwrap();
        assert!((lb2.cut_rate_a - 0.30).abs() < 1e-12);
        // レイティアH: 1950+130200、固定減少 2925(一部修正版)、70.75%
        let lh = find_enemy("leitia_h").unwrap();
        assert_eq!(lh.defense, 132_150);
        assert_eq!(lh.damage_reduction, -2925);
        assert!((lh.cut_rate_a - 0.2925).abs() < 1e-12);
        // レリックの聖域: 10 段のみ 51%、11 段以降は 72%
        let r10 = find_enemy("relic_sanctuary_10").unwrap();
        assert!((r10.cut_rate_a - 0.49).abs() < 1e-12);
        let r19 = find_enemy("relic_sanctuary_19").unwrap();
        assert_eq!(r19.defense, 105_540);
        assert!((r19.cut_rate_a - 0.28).abs() < 1e-12);
    }
}
