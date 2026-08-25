//! スキルカタログ。
//!
//! 出典: wiki 各キャラの `Skill/<キャラ名>` ページ「スキル性能一覧」(取得 2026-08-25)。
//! 表の列をそのまま持つ: 依存 / 属性 / SLv / 攻撃力(倍率 × 段数) / Cri倍 / 命中 / Cri値。
//!
//! - **スキル命中は wiki 表記 +15 した実値**(計算式まとめ `#AccuracyPoint`「当Wikiのスキル命中は
//!   実際の数値から15引いた値が記載されている」)。wiki が `-` の行は `None`(未記載)
//! - id は wiki のページ内アンカーを snake_case にしたもの(`lucian_thrust` 等)。機械的に決まる
//! - スキル Lv 別倍率は未対応。表に載っている SLv(基本攻撃 1 / 極 10)の値だけを持つ
//! - **除外**: 銭投げ(SEED 依存で式が別)、同じアンカーの変種行((瞬撃)/(連撃)/(速剣適用時) 等)、
//!   未対応の依存(STAB+INT / HACK+MR / INT+STAB+HACK)。詳細は
//!   docs/claude/decisions.md「2026-08-25 全キャラのスキル取込」

use domain::{Element, Skill, SkillDependency};

use crate::Source;

pub const SKILLS_SOURCE: Source = Source {
    page: "Skill/<各キャラ名>「スキル性能一覧」",
    retrieved_on: "2026-08-25",
    note: "19 キャラ 303 件。ボリスの倍率 / 段数 / Cri倍は旧リポ twtoolkit boris.json           (Excel v4.00 由来)と全件一致。スキル命中は wiki 表記 +15",
};

struct SkillRecord {
    character_id: &'static str,
    /// wiki のページ内アンカーを snake_case にしたもの。id は `<character_id>_<この値>`
    id: &'static str,
    name: &'static str,
    dependency: SkillDependency,
    multiplier: f64,
    hit_count: u32,
    critical_multiplier: f64,
    element: Element,
    /// wiki 表記 +15 した実値。wiki が `-` の行は `None`
    accuracy: Option<i64>,
    critical_rate: Option<i64>,
    level: u8,
}

#[allow(clippy::too_many_arguments)]
const fn s(
    character_id: &'static str,
    id: &'static str,
    name: &'static str,
    dependency: SkillDependency,
    multiplier: f64,
    hit_count: u32,
    critical_multiplier: f64,
    element: Element,
    accuracy: Option<i64>,
    critical_rate: Option<i64>,
    level: u8,
) -> SkillRecord {
    SkillRecord {
        character_id,
        id,
        name,
        dependency,
        multiplier,
        hit_count,
        critical_multiplier,
        element,
        accuracy,
        critical_rate,
        level,
    }
}

#[rustfmt::skip]
const SKILLS: &[SkillRecord] = &[
    // ---- ルシアン (16 件) ----
    s("lucian", "butt", "†極・突き", SkillDependency::Stab, 1.2, 1, 1.75, Element::Neutral, Some(105), Some(8), 1),
    s("lucian", "horizontal_sword", "†極・横斬り", SkillDependency::Hack, 1.1, 1, 2.25, Element::Neutral, Some(93), Some(6), 1),
    s("lucian", "vertical_sword", "†極・縦斬り", SkillDependency::StabHack, 1.05, 1, 2.35, Element::Neutral, Some(99), Some(7), 1),
    s("lucian", "killing", "極・殺", SkillDependency::Stab, 4.5, 1, 3.5, Element::Neutral, Some(90), Some(15), 10),
    s("lucian", "vortex", "極・ヴォーテックス", SkillDependency::Stab, 5.04, 11, 3.0, Element::Neutral, Some(110), Some(6), 10),
    s("lucian", "streak", "極・連撃", SkillDependency::Stab, 4.86, 10, 2.8, Element::Neutral, Some(100), Some(5), 10),
    s("lucian", "crescent_slash", "極・三日月斬り", SkillDependency::Stab, 5.05, 4, 3.3, Element::Neutral, Some(100), Some(5), 10),
    s("lucian", "continuous", "極・連", SkillDependency::Hack, 5.04, 11, 3.0, Element::Neutral, Some(92), Some(6), 10),
    s("lucian", "warriors_dance", "極・無双乱舞", SkillDependency::Hack, 4.4, 10, 3.1, Element::Neutral, Some(100), Some(5), 10),
    s("lucian", "circle", "極・円", SkillDependency::Hack, 3.27, 1, 3.5, Element::Neutral, Some(90), Some(9), 10),
    s("lucian", "flying_burst", "極・飛連破", SkillDependency::Hack, 5.84, 5, 2.93, Element::Neutral, Some(110), Some(10), 10),
    s("lucian", "fei", "極・飛", SkillDependency::StabHack, 4.2, 1, 3.1, Element::Neutral, Some(95), Some(15), 10),
    s("lucian", "waltz", "極・円舞", SkillDependency::StabHack, 5.4, 11, 3.0, Element::Neutral, Some(92), Some(6), 10),
    s("lucian", "whirlwind_sword", "極・旋風斬", SkillDependency::StabHack, 5.35, 10, 2.8, Element::Neutral, Some(100), Some(5), 10),
    s("lucian", "sylph_cutter", "極・シルフカッター", SkillDependency::StabHack, 4.66, 5, 3.2, Element::Wind, Some(110), Some(2), 10),
    s("lucian", "wind_slice", "極・ウィンドスライス", SkillDependency::Int, 1.42, 10, 1.5, Element::Wind, Some(110), Some(5), 10),
    // ---- ボリス (16 件) ----
    s("boris", "horizontal_sword", "†極・横斬り", SkillDependency::StabHack, 0.99, 1, 2.0, Element::Neutral, Some(98), Some(8), 1),
    s("boris", "vertical_sword", "†極・縦斬り", SkillDependency::Hack, 1.09, 1, 2.5, Element::Neutral, Some(92), Some(7), 1),
    s("boris", "ice_break", "†極・アイスブレイク", SkillDependency::HackInt, 1.13, 1, 2.25, Element::Water, Some(92), Some(7), 1),
    s("boris", "blur_sword", "極・残影斬", SkillDependency::StabHack, 5.45, 11, 2.7, Element::Neutral, Some(102), Some(13), 10),
    s("boris", "explosion", "極・爆", SkillDependency::StabHack, 3.99, 4, 3.0, Element::Neutral, Some(100), Some(7), 10),
    s("boris", "smash_crusher", "極・スマッシュクラッシャー", SkillDependency::StabHack, 5.05, 5, 2.93, Element::Neutral, Some(100), Some(5), 10),
    s("boris", "inherited", "極・インヘリテッド", SkillDependency::StabHack, 4.52, 4, 3.1, Element::Neutral, Some(100), Some(7), 10),
    s("boris", "continuous", "極・連", SkillDependency::Hack, 5.5, 11, 2.5, Element::Neutral, Some(92), Some(6), 10),
    s("boris", "crash_bomb", "極・クラッシュボム", SkillDependency::Hack, 5.95, 5, 2.75, Element::Neutral, Some(100), Some(5), 10),
    s("boris", "ice_attack_sword", "極・氷撃斬", SkillDependency::HackInt, 5.45, 11, 2.7, Element::Water, Some(130), Some(13), 10),
    s("boris", "frozen_sleigh", "極・フローズンスレイ", SkillDependency::HackInt, 2.13, 3, 2.5, Element::Water, Some(97), Some(5), 10),
    s("boris", "frozen_break", "極・フローズンブレイク", SkillDependency::HackInt, 4.83, 5, 3.25, Element::Water, Some(130), Some(5), 10),
    s("boris", "ice_missile", "極・アイスミサイル", SkillDependency::HackInt, 1.63, 10, 1.5, Element::Water, Some(110), Some(5), 10),
    s("boris", "ice_fog", "極・アイスフォグ", SkillDependency::HackInt, 3.08, 2, 2.0, Element::Water, Some(100), Some(10), 10),
    s("boris", "icing_earrings", "極・アイシングピアス", SkillDependency::HackInt, 1.82, 8, 2.0, Element::Water, Some(105), Some(12), 10),
    s("boris", "gracia", "極・グラシア", SkillDependency::HackInt, 2.81, 1, 2.0, Element::Water, Some(110), Some(5), 10),
    // ---- イスピン (10 件) ----
    s("ispin", "step_in", "†極・ステップイン", SkillDependency::Stab, 1.02, 1, 1.5, Element::Neutral, Some(106), Some(8), 1),
    s("ispin", "scratch", "†極・スクラッチ", SkillDependency::Hack, 1.05, 1, 2.5, Element::Neutral, Some(94), Some(6), 1),
    s("ispin", "over_cut", "†極・オーバーカット", SkillDependency::StabHack, 0.99, 1, 2.0, Element::Neutral, Some(100), Some(7), 1),
    s("ispin", "gale_butt", "極・疾風突", SkillDependency::Stab, 4.65, 10, 2.8, Element::Neutral, Some(105), Some(10), 10),
    s("ispin", "counter_spear", "極・カウンタースピア", SkillDependency::Stab, 5.27, 5, 2.8, Element::Fire, Some(110), Some(12), 10),
    s("ispin", "ren", "極・連", SkillDependency::Hack, 5.5, 11, 2.5, Element::Neutral, Some(92), Some(5), 10),
    s("ispin", "sanfamu", "極・散花舞", SkillDependency::Hack, 5.18, 4, 2.8, Element::Fire, Some(105), Some(12), 10),
    s("ispin", "grand_cross", "極・グランドクロス", SkillDependency::StabHack, 4.65, 10, 3.0, Element::Neutral, Some(110), Some(10), 10),
    s("ispin", "double_cross_slash", "極・クロスブランディング", SkillDependency::StabHack, 5.0, 4, 2.8, Element::Fire, Some(105), Some(12), 10),
    s("ispin", "killing", "極・殺", SkillDependency::Stab, 17.0, 1, 2.3, Element::Neutral, Some(90), Some(15), 10),
    // ---- マキシミン (14 件) ----
    s("maximin", "sword", "†極・スラッシュ", SkillDependency::Hack, 1.07, 1, 2.5, Element::Neutral, Some(103), Some(8), 1),
    s("maximin", "attracted_sword", "†極・引き付け斬り", SkillDependency::StabHack, 1.01, 1, 2.0, Element::Neutral, Some(109), Some(7), 1),
    s("maximin", "air_break", "†極・エアブレイク", SkillDependency::HackInt, 1.11, 1, 2.25, Element::Wind, Some(102), Some(8), 1),
    s("maximin", "zan", "極・斬", SkillDependency::Hack, 4.0, 1, 3.5, Element::Neutral, Some(120), Some(14), 10),
    s("maximin", "continuous", "極・連", SkillDependency::Hack, 5.55, 11, 2.5, Element::Neutral, Some(110), Some(6), 10),
    s("maximin", "wind_storm", "極・ウィンドストーム", SkillDependency::Hack, 3.0, 2, 2.5, Element::Wind, Some(100), Some(5), 10),
    s("maximin", "roll_hash", "極・ロールハッシュ", SkillDependency::Hack, 4.74, 4, 3.1, Element::Wind, Some(116), Some(4), 10),
    s("maximin", "explosion", "極・爆", SkillDependency::StabHack, 4.79, 4, 3.2, Element::Neutral, Some(105), Some(7), 10),
    s("maximin", "storm_eye", "極・爆風の目", SkillDependency::StabHack, 5.35, 10, 2.7, Element::Wind, Some(103), Some(5), 10),
    s("maximin", "moonlight_sword", "極・閃花月光斬", SkillDependency::HackInt, 4.62, 10, 3.1, Element::Neutral, Some(120), Some(10), 10),
    s("maximin", "wind_tooth_knife", "極・風牙刀", SkillDependency::HackInt, 5.4, 11, 2.75, Element::Wind, Some(118), Some(18), 10),
    s("maximin", "wind_slice", "極・ウィンドスライス", SkillDependency::HackInt, 1.42, 10, 2.0, Element::Wind, Some(110), Some(5), 10),
    s("maximin", "sylph_lance", "極・シルフランス", SkillDependency::HackInt, 6.05, 4, 3.0, Element::Wind, Some(120), Some(10), 10),
    s("maximin", "mistral_blade", "極・ミストラルブレード", SkillDependency::HackInt, 5.85, 5, 2.78, Element::Neutral, Some(135), Some(7), 10),
    // ---- ティチエル (20 件) ----
    s("tichiel", "twinkle", "†極・トゥインクル", SkillDependency::Int, 1.2, 1, 1.5, Element::Neutral, Some(95), Some(6), 1),
    s("tichiel", "smash", "†極・スマッシュ", SkillDependency::StabHack, 0.96, 1, 2.0, Element::Neutral, Some(95), Some(7), 1),
    s("tichiel", "fire_ball", "極・ファイヤーボール", SkillDependency::Int, 6.71, 4, 2.5, Element::Fire, Some(100), Some(10), 10),
    s("tichiel", "burning_air", "極・バーニングエア", SkillDependency::Int, 1.8, 1, 2.0, Element::Fire, Some(100), Some(5), 10),
    s("tichiel", "giga_blaze", "極・ギガブレイズ", SkillDependency::Int, 12.0, 6, 2.0, Element::Fire, Some(100), Some(5), 10),
    s("tichiel", "fire_arrow", "極・ファイヤーアロー", SkillDependency::Int, 5.0, 10, 2.4, Element::Fire, Some(100), Some(5), 10),
    s("tichiel", "cold_snap", "極・コールドスナップ", SkillDependency::Int, 6.15, 4, 2.5, Element::Water, Some(100), Some(5), 10),
    s("tichiel", "frost_coating", "極・フロストコーティング", SkillDependency::Int, 1.0, 1, 2.0, Element::Water, Some(100), Some(5), 10),
    s("tichiel", "blizzard", "極・ブリザード", SkillDependency::Int, 6.0, 4, 2.2, Element::Water, Some(100), Some(5), 10),
    s("tichiel", "ice_missile", "極・アイスミサイル", SkillDependency::Int, 5.0, 10, 2.4, Element::Water, Some(100), Some(5), 10),
    s("tichiel", "lightning_rod", "極・ライトニングロード", SkillDependency::Int, 2.5, 1, 2.0, Element::Thunder, Some(100), Some(5), 10),
    s("tichiel", "calling_thunder", "極・コーリングサンダー", SkillDependency::Int, 3.5, 1, 2.0, Element::Thunder, Some(100), Some(5), 10),
    s("tichiel", "sparkling_kite", "極・スパークリングカイト", SkillDependency::Int, 4.92, 10, 2.3, Element::Thunder, Some(100), Some(5), 10),
    s("tichiel", "lightning_bolt", "極・ライトニングボルト", SkillDependency::Int, 5.0, 10, 2.4, Element::Thunder, Some(100), Some(5), 10),
    s("tichiel", "holy_bolt", "極・ホーリーボルト", SkillDependency::Mr, 5.08, 10, 2.5, Element::White, Some(120), Some(4), 10),
    s("tichiel", "sunrise", "極・サンライズ", SkillDependency::Mr, 6.37, 5, 2.5, Element::White, Some(120), Some(9), 10),
    s("tichiel", "aurora_wall", "極・オーロラウォール", SkillDependency::Mr, 2.13, 3, 2.0, Element::White, Some(100), Some(6), 10),
    s("tichiel", "beating", "極・乱打", SkillDependency::StabHack, 7.4, 11, 2.1, Element::Neutral, Some(100), Some(15), 10),
    s("tichiel", "break_armor", "極・ブレイクアーマー", SkillDependency::StabHack, 3.56, 1, 3.0, Element::Neutral, Some(100), Some(10), 10),
    s("tichiel", "blade_wall", "極・ブレイドウォール", SkillDependency::StabHack, 8.73, 4, 2.5, Element::Neutral, Some(100), Some(5), 10),
    // ---- ナヤトレイ (13 件) ----
    s("nayatorei", "cross_thrust", "†極・クロススラスト", SkillDependency::Stab, 0.71, 2, 1.5, Element::Neutral, Some(106), Some(8), 1),
    s("nayatorei", "dual_hit", "†極・デュアルヒット", SkillDependency::StabHack, 0.78, 2, 2.0, Element::Neutral, Some(100), Some(7), 1),
    s("nayatorei", "slash", "†極・スラッシュ", SkillDependency::Hack, 1.04, 1, 2.5, Element::Neutral, Some(94), Some(6), 1),
    s("nayatorei", "back_stab", "極・バックステップ", SkillDependency::Stab, 5.6, 10, 2.5, Element::Neutral, None, None, 10),
    s("nayatorei", "avatar", "極・分身乱撃", SkillDependency::Stab, 6.35, 4, 2.8, Element::Neutral, Some(100), Some(5), 10),
    s("nayatorei", "mausoleum", "極・狂猫", SkillDependency::Stab, 5.5, 10, 2.6, Element::Neutral, Some(100), Some(5), 10),
    s("nayatorei", "assault", "極・襲撃", SkillDependency::Stab, 4.15, 20, 2.3, Element::Neutral, Some(255), Some(10), 10),
    s("nayatorei", "wide_assault", "極・無差別的な襲撃", SkillDependency::Stab, 4.25, 4, 2.8, Element::Neutral, Some(150), Some(10), 10),
    s("nayatorei", "ren", "極・連", SkillDependency::Hack, 6.32, 11, 2.3, Element::Neutral, Some(100), Some(6), 10),
    s("nayatorei", "dance", "極・花蝶乱舞", SkillDependency::Hack, 6.35, 4, 2.8, Element::Neutral, Some(100), Some(5), 10),
    s("nayatorei", "heart", "極・心", SkillDependency::StabHack, 5.1, 10, 2.8, Element::Neutral, Some(100), Some(5), 10),
    s("nayatorei", "shuriken", "極・手裏剣打ち", SkillDependency::StabHack, 5.26, 4, 2.8, Element::Neutral, Some(100), Some(6), 10),
    s("nayatorei", "flash", "極・忍術 閃", SkillDependency::StabHack, 4.5, 2, 3.0, Element::Neutral, Some(120), Some(6), 10),
    // ---- シベリン (12 件) ----
    s("siberin", "thrust", "†極・突き", SkillDependency::Stab, 1.16, 1, 2.1, Element::Neutral, Some(103), Some(9), 1),
    s("siberin", "brandy", "†極・ブランディ", SkillDependency::Hack, 1.3, 1, 2.3, Element::Neutral, Some(93), Some(7), 1),
    s("siberin", "beat_down", "†極・ビートダウン", SkillDependency::StabHack, 1.25, 1, 2.3, Element::Neutral, Some(99), Some(8), 1),
    s("siberin", "turning", "†極・ターニング", SkillDependency::StabHack, 1.2, 1, 2.35, Element::Neutral, Some(95), Some(7), 1),
    s("siberin", "continuous_thrust", "極・連突き", SkillDependency::Stab, 5.27, 11, 2.5, Element::Neutral, Some(88), Some(8), 10),
    s("siberin", "twin_dragon_strike", "極・双龍撃", SkillDependency::Stab, 4.94, 10, 2.3, Element::Fire, Some(100), Some(5), 10),
    s("siberin", "throw_dragon", "極・投龍", SkillDependency::Stab, 5.32, 4, 2.85, Element::Fire, Some(120), Some(10), 10),
    s("siberin", "even_fly", "極・飛連", SkillDependency::StabHack, 4.65, 10, 2.9, Element::Fire, Some(102), Some(6), 10),
    s("siberin", "twin_dragon_slash", "極・双龍閃", SkillDependency::StabHack, 4.0, 2, 3.0, Element::Neutral, Some(100), Some(14), 10),
    s("siberin", "red_dragon_strike", "極・紅龍連撃", SkillDependency::StabHack, 4.65, 10, 2.9, Element::Fire, Some(100), Some(5), 10),
    s("siberin", "red_dragon_climb", "極・紅龍登天", SkillDependency::StabHack, 4.69, 5, 3.1, Element::Fire, Some(120), Some(10), 10),
    s("siberin", "bombing", "極・爆撃", SkillDependency::StabHack, 4.69, 4, 3.3, Element::Fire, Some(120), Some(2), 10),
    // ---- ミラ (12 件) ----
    s("mira", "hit_whip", "†極・ヒットウィップ", SkillDependency::StabHack, 1.01, 1, 2.0, Element::Wind, Some(97), Some(7), 1),
    s("mira", "hard_whip", "†極・ハードウィップ", SkillDependency::Hack, 1.09, 1, 2.5, Element::Wind, Some(88), Some(7), 1),
    s("mira", "whip", "†極・ウィップ", SkillDependency::Hack, 1.06, 1, 2.5, Element::Wind, Some(91), Some(6), 1),
    s("mira", "cool_whip", "†極・クールウィップ", SkillDependency::Stab, 0.94, 1, 1.5, Element::Wind, Some(104), Some(8), 1),
    s("mira", "card_spray_a", "極・カードスプレー A", SkillDependency::StabHack, 3.36, 1, 2.0, Element::Wind, Some(100), Some(6), 10),
    s("mira", "mad_bite_viper", "極・マッドヴァイパー", SkillDependency::StabHack, 4.92, 5, 2.7, Element::Wind, Some(98), Some(10), 10),
    s("mira", "bite_viper", "極・バイトヴァイパー", SkillDependency::StabHack, 4.95, 10, 2.75, Element::Wind, Some(110), Some(10), 10),
    s("mira", "dew_storm", "極・ダガーストーム", SkillDependency::StabHack, 3.75, 2, 2.7, Element::Wind, Some(100), Some(5), 10),
    s("mira", "dirty_strike", "極・ダーティーストライク", SkillDependency::Hack, 15.0, 1, 2.3, Element::Wind, Some(100), Some(15), 10),
    s("mira", "dancing_viper", "極・ダンシングヴァイパー", SkillDependency::Hack, 4.88, 10, 2.6, Element::Wind, Some(100), Some(10), 10),
    s("mira", "crazy_viper", "極・クレージーヴァイパー", SkillDependency::Hack, 5.02, 5, 2.7, Element::Wind, Some(90), Some(5), 10),
    s("mira", "crimson_shooter", "極・紅い射手の砲撃", SkillDependency::StabHack, 9.98, 4, 2.0, Element::Neutral, Some(110), Some(14), 11),
    // ---- ジョシュア (12 件) ----
    s("joshua", "sting", "†極・スティング", SkillDependency::Stab, 1.02, 1, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("joshua", "death_claw", "†極・デスクロー", SkillDependency::Int, 1.0, 1, 1.25, Element::Neutral, Some(90), Some(6), 1),
    s("joshua", "soul_burst", "極・ソウルバースト", SkillDependency::Stab, 3.61, 4, 2.0, Element::Black, Some(100), Some(5), 10),
    s("joshua", "ghost_burst", "極・ゴーストバースト", SkillDependency::Int, 4.49, 10, 2.6, Element::Black, Some(140), Some(13), 10),
    s("joshua", "staccato", "極・スタッカート", SkillDependency::Stab, 4.82, 10, 2.4, Element::Neutral, Some(110), Some(9), 10),
    s("joshua", "vertical_infinity", "極・バーティカルインフィニティ", SkillDependency::Stab, 5.61, 4, 2.7, Element::Neutral, Some(105), Some(7), 10),
    s("joshua", "finale", "極・フィナーレ", SkillDependency::Stab, 2.76, 12, 3.0, Element::Neutral, Some(110), Some(8), 10),
    s("joshua", "soul_slayer", "極・ソウルスレイヤー", SkillDependency::Stab, 5.43, 5, 3.0, Element::Neutral, None, None, 10),
    s("joshua", "shadow_vision", "極・メンティサイド", SkillDependency::Int, 4.83, 10, 2.4, Element::Black, Some(120), Some(3), 10),
    s("joshua", "iron_mist", "極・黒霧雲", SkillDependency::Int, 2.0, 1, 1.25, Element::Black, Some(100), Some(5), 10),
    s("joshua", "ruin", "極・ルイン", SkillDependency::Int, 4.26, 4, 2.3, Element::Black, Some(100), Some(5), 10),
    s("joshua", "soul_grab", "極・ソウルグラブ", SkillDependency::Int, 4.67, 4, 3.2, Element::Black, None, None, 10),
    // ---- クロエ (24 件) ----
    s("chloe", "fire_beat", "†極・ファイヤービート", SkillDependency::Int, 1.1, 1, 1.25, Element::Fire, Some(98), Some(6), 1),
    s("chloe", "ice_beat", "†極・アイスビート", SkillDependency::Int, 1.1, 1, 1.25, Element::Water, Some(98), Some(6), 1),
    s("chloe", "lightning_beat", "†極・ライトニングビート", SkillDependency::Int, 1.1, 1, 1.25, Element::Thunder, Some(98), Some(6), 1),
    s("chloe", "air_beat", "†極・エアビート", SkillDependency::Int, 1.1, 1, 1.25, Element::Wind, Some(98), Some(6), 1),
    s("chloe", "stone_beat", "†極・ストーンビート", SkillDependency::Int, 1.1, 1, 1.25, Element::Earth, Some(98), Some(6), 1),
    s("chloe", "fire_arrow", "極・ファイヤーアロー", SkillDependency::Int, 4.15, 10, 2.7, Element::Fire, Some(100), Some(5), 10),
    s("chloe", "fire_ball", "極・ファイヤーボール", SkillDependency::Int, 5.06, 4, 2.5, Element::Fire, Some(100), Some(10), 10),
    s("chloe", "mega_blaze", "極・メガブレイズ", SkillDependency::Int, 5.43, 4, 2.5, Element::Fire, Some(110), Some(7), 10),
    s("chloe", "meteor_strike", "極・メテオストライク", SkillDependency::Int, 12.0, 6, 2.0, Element::Fire, Some(115), Some(100), 10),
    s("chloe", "ice_missile", "極・アイスミサイル", SkillDependency::Int, 4.15, 10, 2.7, Element::Water, Some(100), Some(5), 10),
    s("chloe", "snow_flake", "極・スノーフレーク", SkillDependency::Int, 5.43, 4, 2.5, Element::Water, None, None, 10),
    s("chloe", "extraction", "極・エクストーション", SkillDependency::Int, 1.0, 1, 2.0, Element::Water, Some(100), Some(5), 10),
    s("chloe", "icicle_rain", "極・アイシクルレイン", SkillDependency::Int, 6.0, 4, 2.2, Element::Water, Some(100), Some(5), 10),
    s("chloe", "thunder_strike", "極・サンダーストライク", SkillDependency::Int, 4.15, 10, 2.7, Element::Thunder, None, None, 10),
    s("chloe", "radial_thunder", "極・ラディアルサンダー", SkillDependency::Int, 4.68, 4, 3.0, Element::Thunder, None, None, 10),
    s("chloe", "static_field", "極・スタティックフィールド", SkillDependency::Int, 1.75, 1, 1.0, Element::Thunder, Some(100), Some(5), 10),
    s("chloe", "electric_ball", "極・エレクトリックボール", SkillDependency::Int, 0.5, 1, 1.0, Element::Thunder, Some(100), Some(5), 10),
    s("chloe", "gast", "極・ガスト", SkillDependency::Int, 4.15, 10, 2.7, Element::Wind, Some(100), Some(5), 10),
    s("chloe", "vacuumize", "極・バキューマイズ", SkillDependency::Int, 4.0, 4, 2.5, Element::Wind, Some(100), Some(10), 10),
    s("chloe", "tornado", "極・トルネード", SkillDependency::Int, 0.5, 1, 1.0, Element::Wind, Some(100), Some(5), 10),
    s("chloe", "stone_arrow", "極・ストーンアロー", SkillDependency::Int, 4.15, 10, 2.7, Element::Earth, None, None, 10),
    s("chloe", "square_shock", "極・スクエアショック", SkillDependency::Int, 5.06, 4, 2.5, Element::Earth, Some(100), Some(10), 10),
    s("chloe", "gravity", "極・グラビティ", SkillDependency::Int, 3.0, 3, 2.5, Element::Earth, Some(100), Some(10), 10),
    s("chloe", "sand_storm", "極・サンドストーム", SkillDependency::Int, 6.0, 4, 2.5, Element::Earth, Some(110), Some(5), 10),
    // ---- ランジエ (12 件) ----
    s("ranjie", "gunshot", "†極・射撃", SkillDependency::Stab, 1.69, 1, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("ranjie", "dual_shot", "†極・デュアルショット", SkillDependency::Stab, 0.63, 2, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("ranjie", "hard_shot", "†極・ハードショット", SkillDependency::Stab, 0.68, 2, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("ranjie", "magic_bullet", "†極・魔弾", SkillDependency::Int, 1.05, 1, 1.5, Element::Neutral, Some(96), Some(8), 1),
    s("ranjie", "magical_dual_shot", "†極・マジカルデュアルショット", SkillDependency::Int, 0.58, 2, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("ranjie", "magical_hard_shot", "†極・マジカルハードショット", SkillDependency::Int, 0.64, 2, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("ranjie", "crazy_shot", "極・クレイジーショット", SkillDependency::Stab, 5.89, 11, 2.3, Element::Neutral, Some(100), Some(5), 10),
    s("ranjie", "multi_shot", "極・マルチショット", SkillDependency::Stab, 6.1, 4, 2.5, Element::Neutral, Some(100), Some(9), 10),
    s("ranjie", "piercing_shot", "極・ピアシングショット", SkillDependency::Stab, 4.05, 5, 2.3, Element::Neutral, Some(100), Some(10), 10),
    s("ranjie", "ice_shot", "極・アイスショット", SkillDependency::Int, 4.36, 10, 2.4, Element::Water, Some(120), Some(5), 10),
    s("ranjie", "misty_shot", "極・ミスティーショット", SkillDependency::Int, 5.95, 4, 2.3, Element::Water, Some(120), Some(10), 10),
    s("ranjie", "ice_pierce_shot", "極・アイスピアスショット", SkillDependency::Int, 5.28, 10, 2.3, Element::Water, Some(130), Some(12), 10),
    // ---- イサック (17 件) ----
    s("isaac", "straight", "†極・ストレート", SkillDependency::Stab, 1.07, 1, 1.5, Element::Neutral, Some(108), Some(8), 1),
    s("isaac", "jab", "†極・ジャブ", SkillDependency::Stab, 0.93, 1, 1.5, Element::Neutral, Some(113), Some(8), 1),
    s("isaac", "double_kick", "†極・ダブルキック", SkillDependency::StabHack, 0.51, 2, 1.5, Element::Neutral, Some(109), Some(8), 1),
    s("isaac", "forefist_punch", "†極・正拳突き", SkillDependency::Hack, 1.07, 1, 1.5, Element::Neutral, Some(100), Some(8), 1),
    s("isaac", "jab_punch", "†極・刻み突き", SkillDependency::Hack, 0.93, 1, 1.5, Element::Neutral, Some(105), Some(8), 1),
    s("isaac", "backfist_strike", "†極・裏拳打ち", SkillDependency::Hack, 0.95, 1, 1.5, Element::Neutral, Some(101), Some(8), 1),
    s("isaac", "slam_bang", "極・スラムバン", SkillDependency::Stab, 6.25, 11, 2.7, Element::Neutral, Some(110), Some(5), 10),
    s("isaac", "blasting_blow", "極・ブラスティングブロー", SkillDependency::Stab, 6.43, 5, 2.7, Element::Neutral, Some(110), Some(9), 10),
    s("isaac", "break_through", "極・ブレイクスルー", SkillDependency::Stab, 3.0, 4, 2.3, Element::Neutral, Some(120), Some(6), 10),
    s("isaac", "demise_furious", "極・滅神乱舞", SkillDependency::Stab, 5.25, 10, 2.6, Element::Neutral, Some(100), Some(5), 10),
    s("isaac", "power_punch", "極・バーストマグナム", SkillDependency::StabHack, 15.13, 1, 2.57, Element::Neutral, Some(79), Some(9), 10),
    s("isaac", "energy_punch", "極・発勁", SkillDependency::Hack, 15.13, 1, 2.57, Element::Neutral, Some(104), Some(8), 10),
    s("isaac", "chris_cross", "極・連打", SkillDependency::Hack, 6.25, 11, 2.7, Element::Neutral, Some(110), Some(10), 10),
    s("isaac", "energy_wave", "極・龍点穴", SkillDependency::Hack, 3.0, 4, 3.0, Element::Neutral, Some(110), Some(11), 10),
    s("isaac", "destrudo", "極・発勁・乱", SkillDependency::Hack, 6.52, 5, 2.7, Element::Neutral, Some(117), Some(7), 10),
    s("isaac", "lion_fear", "極・獅子吼", SkillDependency::Hack, 3.36, 5, 3.0, Element::Neutral, Some(80), Some(80), 10),
    s("isaac", "energy_field", "極・エネルギーフィールド", SkillDependency::Hack, 6.43, 4, 3.0, Element::Neutral, Some(100), Some(5), 10),
    // ---- アナイス (25 件) ----
    s("anais", "fairy_light", "†極・フェアリーライト", SkillDependency::Int, 0.84, 1, 1.5, Element::Neutral, Some(95), Some(6), 1),
    s("anais", "angry_pixie", "極・アングリーピクシー", SkillDependency::Int, 3.24, 3, 2.3, Element::Neutral, Some(88), Some(9), 10),
    s("anais", "thrust", "†極・突き", SkillDependency::Stab, 1.06, 1, 2.5, Element::Neutral, Some(93), Some(6), 5),
    s("anais", "mica_even_bear", "極・熊連", SkillDependency::Stab, 5.3, 10, 3.5, Element::Neutral, Some(92), Some(7), 10),
    s("anais", "mica_footstep", "極・足踏み", SkillDependency::Stab, 5.81, 4, 2.5, Element::Neutral, Some(90), Some(7), 10),
    s("anais", "judgment_spin", "極・ジャッジメントスピン", SkillDependency::Stab, 6.18, 5, 2.8, Element::White, Some(108), Some(8), 10),
    s("anais", "mica_bear_step", "極・ミカベアステップ", SkillDependency::Int, 3.24, 3, 2.3, Element::White, Some(92), Some(7), 10),
    s("anais", "strike", "†極・叩く", SkillDependency::Stab, 0.99, 1, 2.0, Element::Neutral, Some(98), Some(22), 5),
    s("anais", "rucy_even_bear", "極・熊連", SkillDependency::Stab, 5.3, 10, 3.5, Element::Neutral, Some(92), Some(7), 10),
    s("anais", "deathmoment", "極・デスモーメント", SkillDependency::Stab, 6.5, 11, 3.5, Element::Black, Some(95), Some(10), 10),
    s("anais", "rucy_footstep", "極・足踏み", SkillDependency::Stab, 4.48, 4, 2.5, Element::Neutral, Some(80), Some(7), 10),
    s("anais", "rucy_bear_step", "極・ルシベアステップ", SkillDependency::Int, 3.0, 4, 2.1, Element::Black, Some(92), Some(7), 10),
    s("anais", "lightning_attack", "†極・電撃攻撃", SkillDependency::Int, 0.23, 1, 1.5, Element::Thunder, Some(110), Some(8), 5),
    s("anais", "chain_lightning", "極・チェーンライトニング", SkillDependency::Int, 4.5, 10, 2.8, Element::Thunder, Some(115), Some(9), 10),
    s("anais", "tesla_coil", "極・テスラコイル", SkillDependency::Int, 6.09, 3, 2.3, Element::Thunder, Some(110), Some(5), 10),
    s("anais", "crystal_attack", "†極・結晶攻撃", SkillDependency::Int, 0.2, 1, 2.0, Element::Water, Some(110), Some(7), 5),
    s("anais", "crystal_sprinter", "極・クリスタルスプリンター", SkillDependency::Int, 5.3, 10, 2.4, Element::Water, Some(110), Some(8), 10),
    s("anais", "ring_of_ice", "極・リングオブアイス", SkillDependency::Int, 4.6, 4, 2.65, Element::Water, Some(115), Some(5), 10),
    s("anais", "ice_age", "極・アイスエイジ", SkillDependency::Int, 2.03, 1, 2.0, Element::Water, Some(110), Some(5), 10),
    s("anais", "flame_attack", "†極・火炎攻撃", SkillDependency::Int, 0.26, 1, 2.5, Element::Fire, Some(110), Some(6), 5),
    s("anais", "fire_blast", "極・ファイアブラスト", SkillDependency::Int, 3.5, 4, 4.0, Element::Fire, Some(110), Some(10), 10),
    s("anais", "detonate", "極・ディトネート", SkillDependency::Int, 4.07, 4, 3.3, Element::Fire, Some(110), Some(6), 10),
    s("anais", "flare_field", "極・フレアフィールド", SkillDependency::Int, 2.23, 2, 2.0, Element::Fire, Some(110), Some(5), 10),
    s("anais", "dissonance", "極・ディソナンス", SkillDependency::Mr, 4.6, 11, 2.5, Element::Neutral, Some(110), Some(10), 10),
    s("anais", "cacophony", "極・カコフォニー", SkillDependency::Mr, 4.46, 5, 2.78, Element::Neutral, Some(100), Some(7), 10),
    // ---- イソレット (21 件) ----
    s("isolet", "butt", "†極・突き", SkillDependency::StabHack, 0.78, 2, 2.0, Element::Neutral, Some(96), Some(7), 1),
    s("isolet", "horizontal_sword", "†極・横切り", SkillDependency::Hack, 0.71, 2, 1.75, Element::Neutral, Some(106), Some(8), 1),
    s("isolet", "devine_beat", "†極・ディバインビート", SkillDependency::Mr, 0.95, 1, 2.0, Element::Neutral, Some(95), Some(6), 1),
    s("isolet", "dash_blade", "極・ダッシュブレイド", SkillDependency::Hack, 3.5, 4, 2.5, Element::Wind, Some(120), Some(12), 10),
    s("isolet", "gravity_field", "極・グラビティフィールド", SkillDependency::Hack, 4.5, 4, 2.8, Element::Wind, Some(120), Some(12), 10),
    s("isolet", "vacuum_sword", "極・真空斬", SkillDependency::Hack, 4.35, 4, 3.1, Element::Wind, Some(120), Some(12), 10),
    s("isolet", "gale_sword", "極・烈風斬", SkillDependency::Hack, 4.35, 4, 3.2, Element::Wind, Some(120), Some(12), 10),
    s("isolet", "wind_spear", "極・ウィンドスピア", SkillDependency::Hack, 4.45, 4, 3.3, Element::Wind, Some(120), Some(12), 10),
    s("isolet", "whirl_wind", "極・かまいたち", SkillDependency::Hack, 5.11, 10, 2.7, Element::Wind, Some(120), Some(6), 10),
    s("isolet", "back_blade", "極・バックブレイド", SkillDependency::Hack, 2.69, 4, 3.0, Element::Wind, Some(120), Some(12), 10),
    s("isolet", "continuous", "極・連", SkillDependency::Hack, 5.61, 11, 2.5, Element::Neutral, Some(102), Some(6), 10),
    s("isolet", "circle", "極・円", SkillDependency::Hack, 6.44, 4, 2.3, Element::Neutral, Some(108), Some(9), 10),
    s("isolet", "storm_blade", "極・ストームブレード", SkillDependency::Hack, 4.0, 14, 3.7, Element::Neutral, Some(120), Some(14), 10),
    s("isolet", "storm_dance", "極・ストームダンス", SkillDependency::Hack, 3.2, 14, 3.0, Element::Neutral, Some(120), Some(14), 10),
    s("isolet", "storm_blast", "極・ストームブラスト", SkillDependency::Hack, 3.7, 14, 3.5, Element::Neutral, Some(120), Some(14), 10),
    s("isolet", "holy_light", "極・ホーリーライト", SkillDependency::Mr, 4.9, 4, 2.6, Element::White, Some(120), Some(12), 10),
    s("isolet", "zone_burst", "極・ゾーンバースト", SkillDependency::Mr, 4.9, 4, 2.6, Element::White, Some(120), Some(12), 10),
    s("isolet", "holy_phoenix", "極・ホーリーフェニックス", SkillDependency::Mr, 4.3, 4, 2.95, Element::White, Some(105), Some(12), 10),
    s("isolet", "sonic_wave", "極・ソニックウェーブ", SkillDependency::Mr, 0.94, 1, 2.0, Element::White, Some(108), Some(5), 10),
    s("isolet", "gloria", "極・グロリア", SkillDependency::Mr, 4.66, 10, 2.8, Element::White, Some(120), Some(6), 10),
    s("isolet", "holy_bird", "極・ホーリーバード", SkillDependency::Mr, 3.92, 10, 2.5, Element::White, Some(120), Some(14), 10),
    // ---- ベンヤ (14 件) ----
    s("benya", "curse_of_blood", "極・カース・オブ・ブラッド", SkillDependency::Hack, 1.63, 1, 2.0, Element::Black, Some(255), Some(5), 10),
    s("benya", "guillotine", "極・ギロチン", SkillDependency::Hack, 5.41, 4, 2.5, Element::Black, Some(95), Some(15), 10),
    s("benya", "soul_steal", "極・ソウルスチール", SkillDependency::Hack, 2.57, 2, 2.2, Element::Black, Some(100), Some(8), 10),
    s("benya", "death_chain", "極・デスチェーン", SkillDependency::Int, 2.5, 2, 3.0, Element::Black, Some(255), Some(5), 1),
    s("benya", "hell_gate", "極・ヘルゲート", SkillDependency::Hack, 1.13, 4, 1.5, Element::Black, Some(100), Some(8), 10),
    s("benya", "scythe_dancing", "極・サイズダンシング", SkillDependency::Hack, 4.16, 11, 2.3, Element::Black, Some(100), Some(5), 14),
    s("benya", "strike_blow", "極・ストライクブロー", SkillDependency::Hack, 15.0, 1, 3.0, Element::Black, Some(95), Some(15), 10),
    s("benya", "soul_scream", "極・ソウルスクリーム", SkillDependency::Hack, 5.41, 4, 2.5, Element::Black, Some(110), Some(2), 10),
    s("benya", "sharp_hellfire", "極・シャープヘルファイア", SkillDependency::Hack, 5.13, 10, 2.3, Element::Black, Some(101), Some(4), 10),
    s("benya", "earth_dive", "極・アースダイブ", SkillDependency::Mr, 6.24, 5, 2.5, Element::Black, Some(120), Some(15), 10),
    s("benya", "counter_hammer", "極・カウンターハンマー", SkillDependency::Mr, 2.5, 3, 2.5, Element::Black, Some(135), Some(1), 10),
    s("benya", "paul_hammer", "極・ポールハンマー", SkillDependency::Mr, 6.86, 4, 2.5, Element::Black, Some(120), Some(20), 10),
    s("benya", "space_cutting", "極・スペースカッティング", SkillDependency::Mr, 6.45, 4, 2.3, Element::Black, Some(135), Some(2), 10),
    s("benya", "meteor_soul", "極・ミーティアソウル", SkillDependency::Mr, 5.79, 10, 2.3, Element::Black, Some(135), Some(4), 10),
    // ---- ロアミニ (11 件) ----
    s("roamini", "darkness_flare", "†極・ダークネス・フレア", SkillDependency::Int, 1.04, 1, 2.0, Element::Black, Some(115), Some(10), 1),
    s("roamini", "poison_dart", "†極・ポイズンダーツ", SkillDependency::Int, 1.04, 1, 2.0, Element::Black, Some(115), Some(10), 1),
    s("roamini", "curse_nova", "極・カース・ノヴァ", SkillDependency::Int, 5.69, 4, 2.35, Element::Black, Some(115), Some(10), 10),
    s("roamini", "darkness_gazer", "極・ダークネス・ゲイザー", SkillDependency::Int, 5.69, 4, 2.35, Element::Black, Some(115), Some(3), 10),
    s("roamini", "carse_flame", "極・カース・フレイム", SkillDependency::Int, 4.15, 10, 2.6, Element::Black, Some(130), Some(10), 10),
    s("roamini", "poison_mist", "極・ポイズン・ミスト", SkillDependency::Int, 5.69, 4, 2.35, Element::Black, Some(115), Some(5), 10),
    s("roamini", "mastary1_2", "極・ベノムノヴァ", SkillDependency::Int, 1.32, 4, 1.5, Element::Black, Some(100), Some(10), 10),
    s("roamini", "mastary3_2", "極・逃走", SkillDependency::Int, 1.32, 1, 1.5, Element::Black, Some(100), Some(5), 10),
    s("roamini", "mastary3_3", "極・浸透", SkillDependency::Int, 1.32, 4, 1.5, Element::Black, Some(255), Some(0), 10),
    s("roamini", "mastary2_4", "極・カース・エンド", SkillDependency::Int, 4.05, 9, 2.0, Element::Black, Some(115), Some(5), 10),
    s("roamini", "mastary2_3", "極・怨恨", SkillDependency::Int, 4.35, 10, 2.6, Element::Black, Some(115), Some(5), 10),
    // ---- ノクターン (10 件) ----
    s("nocturne", "launcher", "†極・ランチャー", SkillDependency::Stab, 1.04, 3, 2.0, Element::Thunder, Some(104), Some(10), 1),
    s("nocturne", "lightning_bomb", "極・ライトニングボム", SkillDependency::Stab, 5.0, 4, 3.0, Element::Thunder, Some(115), Some(10), 10),
    s("nocturne", "shining_laser", "極・シャイニングレーザー", SkillDependency::Stab, 3.4, 10, 3.5, Element::Thunder, Some(100), Some(9), 10),
    s("nocturne", "magnetic_force", "極・マグネティックフォース", SkillDependency::Stab, 1.5, 3, 1.5, Element::Thunder, Some(100), Some(9), 1),
    s("nocturne", "buster_launcher", "極・バスターランチャー", SkillDependency::Stab, 3.96, 10, 3.2, Element::Thunder, Some(103), Some(5), 10),
    s("nocturne", "laser_canon", "極・レーザーカノン", SkillDependency::Stab, 0.225, 2, 3.4, Element::Thunder, Some(140), Some(2), 10),
    s("nocturne", "plasma_canon", "極・プラズマカノン", SkillDependency::Stab, 4.13, 2, 3.4, Element::Thunder, Some(140), Some(2), 10),
    s("nocturne", "satellite_canon", "極・サテライトカノン", SkillDependency::Stab, 1.88, 3, 1.5, Element::Thunder, Some(100), Some(5), 10),
    s("nocturne", "cluster_rocket", "極・クラスターロケット", SkillDependency::Stab, 4.0, 6, 3.7, Element::Thunder, Some(100), Some(9), 10),
    s("nocturne", "quantum_nuclear", "極・クアンタムニュークリア", SkillDependency::Stab, 4.5, 8, 3.5, Element::Thunder, Some(100), Some(9), 10),
    // ---- リーチェ (24 件) ----
    s("leeche", "monpureine_skill_1", "†極・トンド", SkillDependency::Hack, 1.03, 1, 2.0, Element::Neutral, Some(105), Some(6), 1),
    s("leeche", "monpureine_skill_2", "†極・フェンデンテ", SkillDependency::Hack, 1.05, 1, 2.5, Element::Neutral, Some(99), Some(7), 1),
    s("leeche", "monpureine_skill_3", "†極・プンタ", SkillDependency::StabHack, 1.03, 1, 1.5, Element::Neutral, Some(102), Some(7), 1),
    s("leeche", "monpureine_skill_4", "極・アレグロ", SkillDependency::Hack, 5.25, 10, 2.5, Element::Neutral, Some(90), Some(6), 10),
    s("leeche", "monpureine_skill_5", "極・アレグロ・ディ・モルト", SkillDependency::Hack, 5.24, 12, 2.3, Element::Neutral, Some(85), Some(2), 10),
    s("leeche", "monpureine_skill_6", "極・アレグレット", SkillDependency::Hack, 5.25, 12, 2.5, Element::Neutral, Some(95), Some(6), 10),
    s("leeche", "monpureine_skill_7", "極・フォルテ", SkillDependency::Hack, 10.44, 1, 3.0, Element::Neutral, Some(90), Some(15), 10),
    s("leeche", "monpureine_skill_8", "極・フォルティッシモ", SkillDependency::Hack, 14.4, 1, 3.0, Element::Neutral, Some(144), Some(6), 10),
    s("leeche", "monpureine_skill_9", "極・メゾフォルテ", SkillDependency::Hack, 8.4, 1, 3.0, Element::Neutral, Some(95), Some(15), 10),
    s("leeche", "monpureine_skill_10", "極・モリネト", SkillDependency::Hack, 5.23, 4, 2.5, Element::Neutral, Some(100), Some(7), 10),
    s("leeche", "monpureine_skill_11", "極・モリネトポテンテ", SkillDependency::Hack, 6.64, 4, 2.5, Element::Neutral, Some(160), Some(7), 10),
    s("leeche", "monpureine_skill_12", "極・モリネトアビーレ", SkillDependency::Hack, 4.36, 4, 2.5, Element::Neutral, Some(105), Some(7), 10),
    s("leeche", "monpureine_skill_19", "極・ペンデュラム投擲", SkillDependency::Hack, 1.9, 8, 2.3, Element::Neutral, Some(100), Some(6), 10),
    s("leeche", "monpureine_skill_21", "極・回避起動", SkillDependency::Hack, 3.3, 2, 2.5, Element::Neutral, Some(110), Some(6), 10),
    s("leeche", "armor_of_evil_skill_2", "極・圧殺", SkillDependency::Hack, 5.33, 5, 2.5, Element::Neutral, Some(110), Some(7), 10),
    s("leeche", "armor_of_evil_skill_3", "極・絞殺", SkillDependency::Hack, 3.86, 10, 2.3, Element::Neutral, Some(100), Some(6), 10),
    s("leeche", "armor_of_evil_skill_8", "極・爆気", SkillDependency::Hack, 7.6, 4, 2.5, Element::Neutral, Some(144), Some(15), 10),
    s("leeche", "armor_of_evil_skill_9", "極・酸化", SkillDependency::Hack, 3.05, 5, 2.5, Element::Neutral, None, None, 10),
    s("leeche", "armor_of_evil_skill_11", "極・スピロ", SkillDependency::Hack, 3.8, 4, 2.5, Element::Neutral, Some(110), Some(7), 10),
    s("leeche", "armor_of_evil_skill_12", "極・リフィラーレ", SkillDependency::Hack, 3.0, 3, 1.5, Element::Neutral, Some(90), Some(7), 10),
    s("leeche", "armor_of_evil_skill_13", "極・アーゴグロッソ", SkillDependency::Hack, 1.9, 2, 2.5, Element::Neutral, Some(90), Some(7), 10),
    s("leeche", "anarose_skill_5", "極・クラヴァータ", SkillDependency::Hack, 3.1, 4, 2.5, Element::Neutral, Some(110), Some(7), 10),
    s("leeche", "anarose_skill_6", "極・テセーレ", SkillDependency::Hack, 2.84, 4, 1.0, Element::Neutral, Some(90), Some(0), 10),
    s("leeche", "runaway_skill_2", "極・血を流す槍", SkillDependency::Hack, 15.0, 2, 3.0, Element::Neutral, Some(144), Some(15), 10),
    // ---- イェフネン (20 件) ----
    s("yefnen", "continuous", "極・連", SkillDependency::Hack, 3.81, 11, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "explosion", "極・爆", SkillDependency::Hack, 4.54, 5, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "slay", "極・スレイ", SkillDependency::Hack, 7.0, 12, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "crash", "極・クラッシュ", SkillDependency::Hack, 6.75, 6, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "continuous_pike", "極・連・パイク", SkillDependency::Hack, 4.35, 10, 3.6, Element::Neutral, None, None, 10),
    s("yefnen", "explosion_pike", "極・爆・パイク", SkillDependency::Hack, 4.2, 4, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "slay_pike", "極・スレイ・パイク", SkillDependency::Hack, 8.5, 10, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "crash_pike", "極・クラッシュ・パイク", SkillDependency::Hack, 6.2, 6, 3.5, Element::Neutral, None, None, 10),
    s("yefnen", "continuous_axe", "極・連・アックス", SkillDependency::Hack, 4.45, 13, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "explosion_axe", "極・爆・アックス", SkillDependency::Hack, 5.18, 6, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "slay_axe", "極・スレイ・アックス", SkillDependency::Hack, 8.4, 8, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "crash_axe", "極・クラッシュ・アックス", SkillDependency::Hack, 5.8, 6, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "continuous_urumi", "極・連・ウルミ", SkillDependency::Hack, 3.42, 11, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "explosion_urumi", "極・爆・ウルミ", SkillDependency::Hack, 3.96, 5, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "slay_urumi", "極・スレイ・ウルミ", SkillDependency::Hack, 3.5, 8, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "crash_urumi", "極・クラッシュ・ウルミ", SkillDependency::Hack, 3.2, 4, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "continuous_chisel", "極・連・チゼル", SkillDependency::Hack, 13.44, 1, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "explosion_chisel", "極・爆・チゼル", SkillDependency::Hack, 13.5, 1, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "slay_chisel", "極・スレイ・チゼル", SkillDependency::Hack, 4.5, 12, 3.0, Element::Neutral, None, None, 10),
    s("yefnen", "crash_chisel", "極・クラッシュ・チゼル", SkillDependency::Hack, 4.1, 6, 3.0, Element::Neutral, None, None, 10),
];

impl SkillRecord {
    fn skill_id(&self) -> String {
        format!("{}_{}", self.character_id, self.id)
    }

    fn to_skill(&self) -> Skill {
        Skill {
            id: self.skill_id(),
            name: self.name.to_string(),
            dependency: self.dependency,
            multiplier: self.multiplier,
            hit_count: self.hit_count,
            critical_multiplier: self.critical_multiplier,
            element: self.element,
            accuracy: self.accuracy,
            critical_rate: self.critical_rate,
            level: self.level,
        }
    }
}

/// キャラクターのスキル一覧。
pub fn skills_for(character_id: &str) -> Vec<Skill> {
    SKILLS
        .iter()
        .filter(|s| s.character_id == character_id)
        .map(SkillRecord::to_skill)
        .collect()
}

pub fn find_skill(id: &str) -> Option<Skill> {
    SKILLS.iter().find(|s| s.skill_id() == id).map(SkillRecord::to_skill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 全19キャラにスキルがある() {
        for c in crate::characters() {
            assert!(!skills_for(c.id).is_empty(), "{} のスキルが無い", c.id);
        }
        assert!(skills_for("nope").is_empty());
    }

    #[test]
    fn id_で検索できる() {
        let s = find_skill("boris_blur_sword").unwrap();
        assert_eq!(s.name, "極・残影斬");
        assert_eq!(s.hit_count, 11);
        assert_eq!(s.dependency, SkillDependency::StabHack);
        assert!(find_skill("nope").is_none());
    }

    #[test]
    fn ボリスの5件は旧リポの値と一致する() {
        // 旧リポ twtoolkit boris.json(Excel v4.00 由来)の倍率 / 段数 / Cri倍
        for (id, multiplier, hit_count, critical) in [
            ("boris_horizontal_sword", 0.99, 1, 2.0),
            ("boris_vertical_sword", 1.09, 1, 2.5),
            ("boris_ice_break", 1.13, 1, 2.25),
            ("boris_blur_sword", 5.45, 11, 2.7),
            ("boris_continuous", 5.5, 11, 2.5),
        ] {
            let s = find_skill(id).unwrap();
            assert!((s.multiplier - multiplier).abs() < 1e-9, "{id} の倍率");
            assert_eq!(s.hit_count, hit_count, "{id} の段数");
            assert!((s.critical_multiplier - critical).abs() < 1e-9, "{id} の Cri倍");
        }
    }

    #[test]
    fn id_は一意() {
        let mut ids: Vec<String> = SKILLS.iter().map(SkillRecord::skill_id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SKILLS.len());
    }

    #[test]
    fn 収録キャラはすべてプレイアブル一覧にある() {
        for s in SKILLS {
            assert!(crate::find_character(s.character_id).is_some(), "{} が一覧に無い", s.character_id);
        }
    }
}
