//! チャネリングスキル(押している間、一定間隔で攻撃を繰り返す技)の tick。
//!
//! 出典: wiki 各キャラの `Skill/<キャラ名>` ページ「スキル性能一覧」の区分列に
//! `続` を含む行(凡例は wiki「Skill#f8e303fb」)。攻撃力列の `(Ns毎)` が 1 回の攻撃
//! (tick)の間隔で、動作列の持続秒 ÷ 間隔 = **1 回の使用で撃つ tick 数**。
//! `[対象指定]` 印の行は動作列が秒として読めないので対象指定列の `持続Ns` から採った。
//! `tools/gamedata/skill_channeling.py` が生成する。手で編集しない。
//!
//! 表記の `492%x10` の `x10` は **1 tick の段数**(`Skill::hit_count`)で、
//! 1 回の使用ぶんの合計は `段数 × tick 数`。

/// `(スキル id, 1 回の使用で撃つ tick 数, tick の間隔(秒))`
#[rustfmt::skip]
pub(crate) const SKILL_CHANNELING: &[(&str, u32, f64)] = &[
    ("chloe_icicle_rain", 10, 0.8),  // クロエ 極・アイシクルレイン(600%x4&br;(0.8s毎) 持続 8s)
    ("chloe_sand_storm", 10, 0.8),  // クロエ 極・サンドストーム(600%x4&br;(0.8s毎) 持続 8s)
    ("isaac_demise_furious", 10, 1.0),  // イサック 極・滅神乱舞(525%x10&br;(1s毎) 持続 10s)
    ("isaac_energy_field", 10, 0.8),  // イサック 極・エネルギーフィールド(643%x4&br;(0.8s毎) 持続 8s)
    ("joshua_soul_burst", 10, 0.8),  // ジョシュア 極・ソウルバースト(361%x4&br;(0.8s間隔) 持続 8s)
    ("lucian_streak", 10, 1.0),  // ルシアン 極・連撃(486%x10&br;(1s毎) 持続 10s)
    ("lucian_warriors_dance", 10, 1.0),  // ルシアン 極・無双乱舞(440%x10&br;(1s毎) 持続 10s)
    ("lucian_whirlwind_sword", 10, 1.0),  // ルシアン 極・旋風斬(535%x10&br;(1s毎) 持続 10s)
    ("nayatorei_mausoleum", 10, 1.0),  // ナヤトレイ 極・狂猫(550%x10&br;(1s毎) 持続 10s)
    ("siberin_bombing", 10, 0.8),  // シベリン 極・爆撃(469%x4&br;(0.8s毎) 持続 8s)
    ("siberin_red_dragon_strike", 10, 1.0),  // シベリン 極・紅龍連撃(465%x10&br;(1s毎) 持続 10s)
    ("siberin_twin_dragon_strike", 10, 1.0),  // シベリン 極・双龍撃(494%x10&br;(1s毎) 持続 10s)
    ("tichiel_blizzard", 10, 0.8),  // ティチエル 極・ブリザード(600%x4&br;(0.8s毎) 持続 8s)
    ("tichiel_sparkling_kite", 10, 1.0),  // [対象指定] ティチエル 極・スパークリングカイト(492%x10&br;(1s毎) 持続 0)
    ("yefnen_crash_urumi", 10, 0.8),  // イェフネン 極・クラッシュ・ウルミ(320%x4&br;(0.8s毎) 持続 8s)
    ("yefnen_slay_urumi", 10, 0.8),  // イェフネン 極・スレイ・ウルミ(350%x8&br;(0.8s毎) 持続 8s)
];
