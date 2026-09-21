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

use domain::{
    Attacker, CharacterSkills, ComboSkillType, ComboSkillVariant, Element, FullCharge, Masteries,
    Skill, SkillDependency, SkillForm, SummonForm, SwiftSword, WeaponClass,
};

use crate::skill_channeling::SKILL_CHANNELING;
use crate::skill_cooldowns::SKILL_COOLDOWNS;
use crate::skill_targets::SKILL_TARGETS;

use crate::Source;

/// 依存能力だけでは実用武器を絞れないスキルの武器種。
///
/// ボリスは刀(HACK)・太刀(STAB+HACK)・大剣(INT+HACK)を装備でき、いずれも斬り依存の
/// スキルを撃てるが、各スキルは特定の武器でしか実用にならない(wiki: Skill/ボリス)。
/// ここに載っていないスキルは依存能力の系統(`WeaponSystem::for_dependency`)で絞る。
const SKILL_WEAPON_CLASSES: &[(&str, &[WeaponClass])] = &[
    ("boris_continuous", &[WeaponClass::Katana]),
    ("boris_blur_sword", &[WeaponClass::Tachi]),
    ("boris_ice_attack_sword", &[WeaponClass::GreatSword]),
];

/// 基本中ディレイ(秒)。wiki スキル性能一覧の「動作」列(取得 2026-08-25)。
///
/// `s(...)` の引数に足さず別表にしているのは、303 行の主表は wiki の「攻撃力・Cri倍・命中・Cri値」の並びを保ちたいため。
/// - `A/B` 表記(イェフネンのパイク系)は条件付き([加速])の値なので**基本値の A** を採る
/// - `(固定)` 付き(ティチエル 極・ギガブレイズ / クロエ 極・メテオストライク)は
///   中ディレイ減少が効かない。`ACTUAL_DELAY_FIXED` で持つ
/// - 秒数として読めない行は `None`(中ディレイ・DPS を出さない)
#[rustfmt::skip]
const ACTUAL_DELAYS: &[(&str, Option<f64>)] = &[
    // ---- lucian ----
    ("lucian_butt", Some(0.6)),
    ("lucian_horizontal_sword", Some(0.7)),
    ("lucian_vertical_sword", Some(0.7)),
    ("lucian_killing", Some(0.8)),
    ("lucian_vortex", Some(1.4)),
    ("lucian_streak", Some(10.0)),
    ("lucian_crescent_slash", Some(0.8)),
    ("lucian_continuous", Some(1.4)),
    ("lucian_warriors_dance", Some(10.0)),
    ("lucian_circle", Some(0.8)),
    ("lucian_flying_burst", Some(1.2)),
    ("lucian_fei", Some(0.6)),
    ("lucian_waltz", Some(1.4)),
    ("lucian_whirlwind_sword", Some(10.0)),
    ("lucian_sylph_cutter", Some(1.2)),
    ("lucian_wind_slice", Some(1.1)),
    // ---- boris ----
    ("boris_horizontal_sword", Some(0.8)),
    ("boris_vertical_sword", Some(0.8)),
    ("boris_ice_break", Some(0.7)),
    ("boris_blur_sword", Some(1.4)),
    ("boris_explosion", Some(0.8)),
    ("boris_smash_crusher", Some(1.2)),
    ("boris_inherited", Some(0.8)),
    ("boris_continuous", Some(1.4)),
    ("boris_crash_bomb", Some(1.2)),
    ("boris_ice_attack_sword", Some(1.4)),
    ("boris_frozen_sleigh", Some(1.5)),
    ("boris_frozen_break", Some(1.2)),
    ("boris_ice_missile", Some(1.1)),
    ("boris_ice_fog", Some(1.4)),
    ("boris_icing_earrings", Some(1.4)),
    ("boris_gracia", Some(1.2)),
    // ---- ispin ----
    ("ispin_step_in", Some(0.7)),
    ("ispin_scratch", Some(0.7)),
    ("ispin_over_cut", Some(0.7)),
    ("ispin_gale_butt", Some(1.0)),
    ("ispin_counter_spear", Some(1.2)),
    ("ispin_ren", Some(1.4)),
    ("ispin_sanfamu", Some(0.8)),
    ("ispin_grand_cross", Some(1.0)),
    ("ispin_double_cross_slash", Some(0.8)),
    ("ispin_killing", Some(0.9)),
    // ---- maximin ----
    ("maximin_sword", Some(0.7)),
    ("maximin_attracted_sword", Some(0.8)),
    ("maximin_air_break", Some(0.7)),
    ("maximin_zan", Some(0.8)),
    ("maximin_continuous", Some(1.4)),
    ("maximin_wind_storm", Some(0.1)),
    ("maximin_roll_hash", Some(0.8)),
    ("maximin_explosion", Some(0.8)),
    ("maximin_storm_eye", Some(1.0)),
    ("maximin_moonlight_sword", Some(1.0)),
    ("maximin_wind_tooth_knife", Some(1.4)),
    ("maximin_wind_slice", Some(1.1)),
    ("maximin_sylph_lance", Some(0.8)),
    ("maximin_mistral_blade", Some(1.2)),
    // ---- tichiel ----
    ("tichiel_twinkle", Some(0.8)),
    ("tichiel_smash", Some(0.7)),
    ("tichiel_fire_ball", Some(0.8)),
    ("tichiel_burning_air", Some(0.8)),
    ("tichiel_giga_blaze", Some(0.8)),
    ("tichiel_fire_arrow", Some(1.0)),
    ("tichiel_cold_snap", Some(1.0)),
    ("tichiel_frost_coating", Some(0.8)),
    ("tichiel_blizzard", Some(8.0)),
    ("tichiel_ice_missile", Some(1.0)),
    ("tichiel_lightning_rod", Some(0.8)),
    ("tichiel_calling_thunder", Some(0.8)),
    // 極・スパークリングカイト: 一覧の「動作」は "0" だが、詳細表(Skill/ティチエル#SparklingKite)は
    // 中 =「ｹﾞｰｼﾞ(10s)」= 最大 10 秒のチャネリング(1 秒ごとに 492% × 10 段)。他のチャネリング技
    // (lucian_streak = 10.0 等)と同じく撃ち切る時間を採る(2026-09-21)
    ("tichiel_sparkling_kite", Some(10.0)),
    ("tichiel_lightning_bolt", Some(1.0)),
    ("tichiel_holy_bolt", Some(1.0)),
    ("tichiel_sunrise", Some(1.2)),
    ("tichiel_aurora_wall", Some(1.1)),
    ("tichiel_beating", Some(1.4)),
    ("tichiel_break_armor", Some(0.8)),
    ("tichiel_blade_wall", Some(0.8)),
    // ---- nayatorei ----
    ("nayatorei_cross_thrust", Some(0.7)),
    ("nayatorei_dual_hit", Some(0.8)),
    ("nayatorei_slash", Some(0.7)),
    ("nayatorei_back_stab", Some(1.0)),
    ("nayatorei_avatar", Some(0.8)),
    ("nayatorei_mausoleum", Some(10.0)),
    ("nayatorei_assault", Some(0.8)),
    ("nayatorei_wide_assault", Some(0.8)),
    ("nayatorei_ren", Some(1.4)),
    ("nayatorei_dance", Some(0.8)),
    ("nayatorei_heart", Some(1.0)),
    ("nayatorei_shuriken", Some(0.8)),
    ("nayatorei_flash", Some(0.6)),
    // ---- siberin ----
    ("siberin_thrust", Some(0.7)),
    ("siberin_brandy", Some(0.8)),
    ("siberin_beat_down", Some(0.8)),
    ("siberin_turning", Some(0.8)),
    ("siberin_continuous_thrust", Some(1.4)),
    ("siberin_twin_dragon_strike", Some(10.0)),
    ("siberin_throw_dragon", Some(0.8)),
    ("siberin_even_fly", Some(1.0)),
    ("siberin_twin_dragon_slash", Some(0.8)),
    ("siberin_red_dragon_strike", Some(10.0)),
    ("siberin_red_dragon_climb", Some(1.2)),
    ("siberin_bombing", Some(8.0)),
    // ---- mira ----
    ("mira_hit_whip", Some(0.7)),
    ("mira_hard_whip", Some(0.7)),
    ("mira_whip", Some(0.7)),
    ("mira_cool_whip", Some(0.9)),
    ("mira_card_spray_a", Some(0.5)),
    ("mira_mad_bite_viper", Some(1.2)),
    ("mira_bite_viper", Some(1.0)),
    ("mira_dew_storm", Some(0.8)),
    ("mira_dirty_strike", Some(0.7)),
    ("mira_dancing_viper", Some(1.0)),
    ("mira_crazy_viper", Some(1.2)),
    ("mira_crimson_shooter", Some(2.0)),
    // ---- joshua ----
    ("joshua_sting", Some(0.7)),
    ("joshua_death_claw", Some(1.1)),
    ("joshua_soul_burst", Some(8.0)),
    ("joshua_ghost_burst", Some(1.0)),
    ("joshua_staccato", Some(1.0)),
    ("joshua_vertical_infinity", Some(0.8)),
    ("joshua_finale", Some(1.0)),
    ("joshua_soul_slayer", Some(1.2)),
    ("joshua_shadow_vision", Some(1.0)),
    ("joshua_iron_mist", Some(0.8)),
    ("joshua_ruin", Some(0.8)),
    ("joshua_soul_grab", Some(0.8)),
    // ---- chloe ----
    ("chloe_fire_beat", Some(0.8)),
    ("chloe_ice_beat", Some(0.8)),
    ("chloe_lightning_beat", Some(0.8)),
    ("chloe_air_beat", Some(0.8)),
    ("chloe_stone_beat", Some(0.8)),
    ("chloe_fire_arrow", Some(1.0)),
    ("chloe_fire_ball", Some(0.8)),
    ("chloe_mega_blaze", Some(0.8)),
    ("chloe_meteor_strike", Some(0.8)),
    ("chloe_ice_missile", Some(1.0)),
    ("chloe_snow_flake", Some(0.8)),
    ("chloe_extraction", Some(0.8)),
    ("chloe_icicle_rain", Some(8.0)),
    ("chloe_thunder_strike", Some(1.0)),
    ("chloe_radial_thunder", Some(0.8)),
    ("chloe_static_field", Some(0.8)),
    ("chloe_electric_ball", Some(0.8)),
    ("chloe_gast", Some(1.0)),
    ("chloe_vacuumize", Some(0.8)),
    ("chloe_tornado", Some(0.8)),
    ("chloe_stone_arrow", Some(1.0)),
    ("chloe_square_shock", Some(0.8)),
    ("chloe_gravity", Some(0.8)),
    ("chloe_sand_storm", Some(8.0)),
    // ---- ranjie ----
    ("ranjie_gunshot", Some(0.9)),
    ("ranjie_dual_shot", Some(1.1)),
    ("ranjie_hard_shot", Some(1.0)),
    ("ranjie_magic_bullet", Some(0.9)),
    ("ranjie_magical_dual_shot", Some(1.1)),
    ("ranjie_magical_hard_shot", Some(1.0)),
    ("ranjie_crazy_shot", Some(1.4)),
    ("ranjie_multi_shot", Some(0.8)),
    ("ranjie_piercing_shot", Some(0.9)),
    ("ranjie_ice_shot", Some(0.9)),
    ("ranjie_misty_shot", Some(0.8)),
    ("ranjie_ice_pierce_shot", Some(1.0)),
    // ---- isaac ----
    ("isaac_straight", Some(1.0)),
    ("isaac_jab", Some(0.8)),
    ("isaac_double_kick", Some(1.0)),
    ("isaac_forefist_punch", Some(1.0)),
    ("isaac_jab_punch", Some(0.8)),
    ("isaac_backfist_strike", Some(0.9)),
    ("isaac_slam_bang", Some(1.4)),
    ("isaac_blasting_blow", Some(1.2)),
    ("isaac_break_through", Some(0.8)),
    ("isaac_demise_furious", Some(10.0)),
    ("isaac_power_punch", Some(0.9)),
    ("isaac_energy_punch", Some(0.9)),
    ("isaac_chris_cross", Some(1.4)),
    ("isaac_energy_wave", Some(1.0)),
    ("isaac_destrudo", Some(1.2)),
    ("isaac_lion_fear", Some(1.5)),
    ("isaac_energy_field", Some(8.0)),
    // ---- anais ----
    ("anais_fairy_light", Some(0.6)),
    ("anais_angry_pixie", Some(0.8)),
    ("anais_thrust", Some(0.5)),
    ("anais_mica_even_bear", Some(1.0)),
    ("anais_mica_footstep", Some(0.8)),
    ("anais_judgment_spin", Some(0.8)),
    ("anais_mica_bear_step", Some(0.8)),
    ("anais_strike", Some(0.5)),
    ("anais_rucy_even_bear", Some(1.0)),
    ("anais_deathmoment", Some(1.0)),
    ("anais_rucy_footstep", Some(0.8)),
    ("anais_rucy_bear_step", Some(1.2)),
    ("anais_lightning_attack", Some(0.3)),
    ("anais_chain_lightning", Some(1.0)),
    ("anais_tesla_coil", Some(1.1)),
    ("anais_crystal_attack", Some(0.3)),
    ("anais_crystal_sprinter", Some(1.0)),
    ("anais_ring_of_ice", Some(0.8)),
    ("anais_ice_age", Some(1.1)),
    ("anais_flame_attack", Some(0.3)),
    ("anais_fire_blast", Some(0.8)),
    ("anais_detonate", Some(0.8)),
    ("anais_flare_field", Some(1.1)),
    ("anais_dissonance", Some(1.4)),
    ("anais_cacophony", Some(1.2)),
    // ---- isolet ----
    ("isolet_butt", Some(0.8)),
    ("isolet_horizontal_sword", Some(0.8)),
    ("isolet_devine_beat", Some(0.7)),
    ("isolet_dash_blade", Some(0.6)),
    ("isolet_gravity_field", Some(0.8)),
    ("isolet_vacuum_sword", Some(0.8)),
    ("isolet_gale_sword", Some(0.8)),
    ("isolet_wind_spear", Some(0.8)),
    ("isolet_whirl_wind", Some(1.0)),
    ("isolet_back_blade", Some(0.5)),
    ("isolet_continuous", Some(1.4)),
    ("isolet_circle", Some(0.8)),
    ("isolet_storm_blade", Some(1.1)),
    ("isolet_storm_dance", Some(1.1)),
    ("isolet_storm_blast", Some(1.1)),
    ("isolet_holy_light", Some(0.8)),
    ("isolet_zone_burst", Some(0.8)),
    ("isolet_holy_phoenix", Some(0.8)),
    ("isolet_sonic_wave", Some(0.1)),
    ("isolet_gloria", Some(1.0)),
    ("isolet_holy_bird", Some(1.0)),
    // ---- benya ----
    ("benya_curse_of_blood", Some(1.0)),
    ("benya_guillotine", Some(0.8)),
    ("benya_soul_steal", Some(0.1)),
    ("benya_death_chain", Some(1.0)),
    ("benya_hell_gate", Some(0.1)),
    ("benya_scythe_dancing", Some(1.4)),
    ("benya_strike_blow", Some(1.0)),
    ("benya_soul_scream", Some(0.8)),
    ("benya_sharp_hellfire", Some(1.0)),
    ("benya_earth_dive", Some(1.0)),
    ("benya_counter_hammer", Some(0.2)),
    ("benya_paul_hammer", Some(1.2)),
    ("benya_space_cutting", Some(0.8)),
    ("benya_meteor_soul", Some(1.0)),
    // ---- roamini ----
    ("roamini_darkness_flare", Some(0.8)),
    ("roamini_poison_dart", Some(0.8)),
    ("roamini_curse_nova", Some(0.8)),
    ("roamini_darkness_gazer", Some(0.8)),
    ("roamini_carse_flame", Some(1.0)),
    ("roamini_poison_mist", Some(0.8)),
    ("roamini_mastary1_2", Some(0.3)),
    ("roamini_mastary3_2", Some(0.4)),
    ("roamini_mastary3_3", Some(0.8)),
    ("roamini_mastary2_4", Some(1.0)),
    ("roamini_mastary2_3", Some(1.0)),
    // ---- nocturne ----
    ("nocturne_launcher", Some(0.8)),
    ("nocturne_lightning_bomb", Some(0.8)),
    ("nocturne_shining_laser", Some(1.0)),
    ("nocturne_magnetic_force", Some(0.1)),
    ("nocturne_buster_launcher", Some(1.0)),
    ("nocturne_laser_canon", Some(1.0)),
    ("nocturne_plasma_canon", Some(1.0)),
    ("nocturne_satellite_canon", Some(0.1)),
    ("nocturne_cluster_rocket", Some(1.0)),
    ("nocturne_quantum_nuclear", Some(1.0)),
    // ---- leeche ----
    ("leeche_monpureine_skill_1", Some(0.7)),
    ("leeche_monpureine_skill_2", Some(0.8)),
    ("leeche_monpureine_skill_3", Some(0.6)),
    ("leeche_monpureine_skill_4", Some(1.0)),
    ("leeche_monpureine_skill_5", Some(1.4)),
    ("leeche_monpureine_skill_6", Some(1.6)),
    ("leeche_monpureine_skill_7", Some(0.8)),
    ("leeche_monpureine_skill_8", Some(0.8)),
    ("leeche_monpureine_skill_9", Some(0.8)),
    ("leeche_monpureine_skill_10", Some(0.8)),
    ("leeche_monpureine_skill_11", Some(0.8)),
    ("leeche_monpureine_skill_12", Some(0.8)),
    ("leeche_monpureine_skill_19", Some(0.8)),
    ("leeche_monpureine_skill_21", Some(0.8)),
    ("leeche_armor_of_evil_skill_2", Some(0.8)),
    ("leeche_armor_of_evil_skill_3", Some(0.8)),
    ("leeche_armor_of_evil_skill_8", Some(0.8)),
    ("leeche_armor_of_evil_skill_9", Some(0.8)),
    ("leeche_armor_of_evil_skill_11", Some(0.8)),
    ("leeche_armor_of_evil_skill_12", Some(0.8)),
    ("leeche_armor_of_evil_skill_13", Some(0.1)),
    ("leeche_anarose_skill_5", Some(0.8)),
    ("leeche_anarose_skill_6", Some(0.8)),
    ("leeche_runaway_skill_2", Some(0.8)),
    // ---- yefnen ----
    ("yefnen_continuous", Some(1.4)),
    ("yefnen_explosion", Some(1.2)),
    ("yefnen_slay", Some(1.4)),
    ("yefnen_crash", Some(1.2)),
    ("yefnen_continuous_pike", Some(1.5)),
    ("yefnen_explosion_pike", Some(1.0)),
    ("yefnen_slay_pike", Some(1.5)),
    ("yefnen_crash_pike", Some(1.0)),
    ("yefnen_continuous_axe", Some(1.8)),
    ("yefnen_explosion_axe", Some(1.5)),
    ("yefnen_slay_axe", Some(1.8)),
    ("yefnen_crash_axe", Some(1.5)),
    ("yefnen_continuous_urumi", Some(1.4)),
    ("yefnen_explosion_urumi", Some(1.2)),
    ("yefnen_slay_urumi", Some(8.0)),
    ("yefnen_crash_urumi", Some(8.0)),
    ("yefnen_continuous_chisel", Some(0.8)),
    ("yefnen_explosion_chisel", Some(0.8)),
    ("yefnen_slay_chisel", Some(0.9)),
    ("yefnen_crash_chisel", Some(0.8)),
];

/// 中ディレイが固定で減少が効かないスキル(wiki スキル性能一覧の「動作」が `(固定)`)。
const ACTUAL_DELAY_FIXED: [&str; 2] = [
    "tichiel_giga_blaze",  // 極・ギガブレイズ
    "chloe_meteor_strike", // 極・メテオストライク
];

/// チャネリング技の tick(`SKILL_CHANNELING`)。
fn channeling_of(skill_id: &str) -> Option<domain::Channeling> {
    SKILL_CHANNELING
        .iter()
        .find(|(id, _, _)| *id == skill_id)
        .map(|&(_, ticks, tick_seconds)| domain::Channeling { ticks, tick_seconds })
}

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

/// コンボインターバル(秒)。wiki 計算式まとめ `#g7881516` の CI 値表(取得 2026-08-31)。
///
/// 通常攻撃(`†` 基本攻撃)ごとの値で、通常攻撃の中ディレイが終わってから数え始める。
/// 最速コンボでは「次に使うスキルの中ディレイの下限」として効くので、コンボの DPS に要る。
/// - `270(実測250)` と併記された行(イェフネン)は**実測値**を採る
/// - CI 値表にあってもスキル性能一覧の † 行が無いもの(依存が未対応で除外した行)は入らない
/// - 表に無い通常攻撃(アナイスの 5 件)は `None`。下限を出せないので、その旨を画面に出す
///
/// 取り込みは `tools/gamedata/import_combo_intervals.py`(突き合わせできない行は標準エラーに出る)。
#[rustfmt::skip]
const COMBO_INTERVALS: &[(&str, f64)] = &[
    ("lucian_butt", 0.32),  // ルシアン 突き
    ("lucian_horizontal_sword", 0.4),  // ルシアン 横斬り
    ("lucian_vertical_sword", 0.35),  // ルシアン 縦斬り
    ("boris_vertical_sword", 0.45),  // ボリス 縦斬り
    ("boris_horizontal_sword", 0.4),  // ボリス 横斬り
    ("boris_ice_break", 0.5),  // ボリス アイスブレイク
    ("ispin_step_in", 0.32),  // イスピン ステップイン
    ("ispin_scratch", 0.4),  // イスピン スクラッチ
    ("ispin_over_cut", 0.35),  // イスピン オーバーカット
    ("maximin_sword", 0.45),  // マキシミン スラッシュ
    ("maximin_air_break", 0.5),  // マキシミン エアブレイク
    ("maximin_attracted_sword", 0.4),  // マキシミン 引き付け斬り
    ("tichiel_twinkle", 0.3),  // ティチエル トゥインクル
    ("tichiel_smash", 0.35),  // ティチエル スマッシュ
    ("nayatorei_cross_thrust", 0.3),  // ナヤトレイ クロススラスト
    ("nayatorei_slash", 0.35),  // ナヤトレイ スラッシュ
    ("nayatorei_dual_hit", 0.35),  // ナヤトレイ デュアルヒット
    ("siberin_thrust", 0.35),  // シベリン 突き
    ("siberin_brandy", 0.45),  // シベリン ブランディ
    ("siberin_beat_down", 0.4),  // シベリン ビートダウン
    ("siberin_turning", 0.55),  // シベリン ターニング
    ("mira_whip", 0.4),  // ミラ ウィップ
    ("mira_hit_whip", 0.35),  // ミラ ヒットウィップ
    ("mira_hard_whip", 0.45),  // ミラ ハードウィップ
    ("mira_cool_whip", 0.3),  // ミラ クールウィップ
    ("joshua_sting", 0.32),  // ジョシュア スティング
    ("joshua_death_claw", 0.3),  // ジョシュア デスクロー
    ("chloe_fire_beat", 0.3),  // クロエ ファイヤービート
    ("chloe_air_beat", 0.3),  // クロエ エアビート
    ("chloe_stone_beat", 0.3),  // クロエ ストーンビート
    ("chloe_ice_beat", 0.3),  // クロエ アイスビート
    ("chloe_lightning_beat", 0.3),  // クロエ ライトニングビート
    ("ranjie_gunshot", 0.3),  // ランジエ 射撃
    ("ranjie_dual_shot", 0.32),  // ランジエ デュアルショット
    ("ranjie_hard_shot", 0.35),  // ランジエ ハードショット
    ("ranjie_magic_bullet", 0.35),  // ランジエ 魔弾
    ("ranjie_magical_dual_shot", 0.37),  // ランジエ ﾏｼﾞｶﾙﾃﾞｭｱﾙｼｮｯﾄ
    ("ranjie_magical_hard_shot", 0.4),  // ランジエ ﾏｼﾞｶﾙﾊｰﾄﾞｼｮｯﾄ
    ("isaac_jab", 0.25),  // イサック ジャブ
    ("isaac_straight", 0.3),  // イサック ストレート
    ("isaac_double_kick", 0.27),  // イサック ダブルキック
    ("isaac_jab_punch", 0.25),  // イサック 刻み突き
    ("isaac_forefist_punch", 0.3),  // イサック 正拳突き
    ("isaac_backfist_strike", 0.27),  // イサック 裏拳打ち
    ("anais_fairy_light", 0.3),  // アナイス フェアリーライト
    ("isolet_devine_beat", 0.3),  // イソレット ディバインビート
    ("isolet_butt", 0.35),  // イソレット 突き
    ("isolet_horizontal_sword", 0.3),  // イソレット 横切り
    ("roamini_darkness_flare", 0.3),  // ロアミニ ダークネスフレア
    ("roamini_poison_dart", 0.3),  // ロアミニ ポイズンダーツ
    ("nocturne_launcher", 0.3),  // ノクターン ランチャー
    ("leeche_monpureine_skill_1", 0.32),  // リーチェ トンド
    ("leeche_monpureine_skill_3", 0.32),  // リーチェ プンタ
    ("leeche_monpureine_skill_2", 0.35),  // リーチェ フェンデンテ
];

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

/// 魔法人形(アナイスのミカベア / ルシベア)が自分で撃つスキル(wiki 計算式まとめ
/// `STAB(熊)` 行、2026-09-18 取得)。ベアステップ(`anais_mica_bear_step` /
/// `anais_rucy_bear_step`)は本体が撃つので含めない。
const MAGIC_DOLL_SKILLS: &[&str] = &[
    "anais_thrust",
    "anais_mica_even_bear",
    "anais_mica_footstep",
    "anais_judgment_spin",
    "anais_strike",
    "anais_rucy_even_bear",
    "anais_deathmoment",
    "anais_rucy_footstep",
];

/// 破壊精霊(アンフェル / グレシス / イグニー)が自分で撃つスキル(wiki「Skill/アナイス」
/// 「計算式まとめ」2026-09-18 取得)。陣の 3 件(`anais_tesla_coil` / `anais_ice_age` /
/// `anais_flare_field`)は wiki スキル性能一覧に「陣による攻撃はアナイス本体がダメージを
/// 与えた扱いになる」と明記されているため本体扱いのまま含めない。守護精霊のスキルも同様。
const DESTRUCTION_SPIRIT_SKILLS: &[&str] = &[
    "anais_lightning_attack",
    "anais_chain_lightning",
    "anais_crystal_attack",
    "anais_crystal_sprinter",
    "anais_ring_of_ice",
    "anais_flame_attack",
    "anais_fire_blast",
    "anais_detonate",
];

/// このスキルを実際に撃つ主体。`MAGIC_DOLL_SKILLS` / `DESTRUCTION_SPIRIT_SKILLS` に載って
/// いるスキルだけそれぞれ `MagicDoll` / `DestructionSpirit`、他は本体 `Player`。
pub fn attacker_of(skill_id: &str) -> Attacker {
    if MAGIC_DOLL_SKILLS.contains(&skill_id) {
        Attacker::MagicDoll
    } else if DESTRUCTION_SPIRIT_SKILLS.contains(&skill_id) {
        Attacker::DestructionSpirit
    } else {
        Attacker::Player
    }
}

/// 主軸スキル → 召喚獣の型、召喚スキル → 自分が属する型(wiki「Skill/アナイス」の型分け、
/// 2026-09-18 追記)。ベアステップ・陣(本体スキル)は型を「決める」側、召喚スキル 16 件は
/// 型に「属する」側で、どちらもこの 1 つの表に載せる(フロントに対応表を書き写させない)。
/// 型を決めない共通スキル(`anais_fairy_light` / `anais_angry_pixie`)と守護精霊
/// (`anais_dissonance` / `anais_cacophony` 他)はここに載せない = `None`。
const SUMMON_FORMS: &[(&str, SummonForm)] = &[
    ("anais_mica_bear_step", SummonForm::MicaBear),
    ("anais_thrust", SummonForm::MicaBear),
    ("anais_mica_even_bear", SummonForm::MicaBear),
    ("anais_mica_footstep", SummonForm::MicaBear),
    ("anais_judgment_spin", SummonForm::MicaBear),
    ("anais_rucy_bear_step", SummonForm::RucyBear),
    ("anais_strike", SummonForm::RucyBear),
    ("anais_rucy_even_bear", SummonForm::RucyBear),
    ("anais_rucy_footstep", SummonForm::RucyBear),
    ("anais_deathmoment", SummonForm::RucyBear),
    ("anais_tesla_coil", SummonForm::Anferu),
    ("anais_lightning_attack", SummonForm::Anferu),
    ("anais_chain_lightning", SummonForm::Anferu),
    ("anais_ice_age", SummonForm::Gureshisu),
    ("anais_crystal_attack", SummonForm::Gureshisu),
    ("anais_crystal_sprinter", SummonForm::Gureshisu),
    ("anais_ring_of_ice", SummonForm::Gureshisu),
    ("anais_flare_field", SummonForm::Igni),
    ("anais_flame_attack", SummonForm::Igni),
    ("anais_fire_blast", SummonForm::Igni),
    ("anais_detonate", SummonForm::Igni),
];

/// 武器形態(wiki「Skill/イェフネン」スキル性能一覧、2026-09-21 取得)。
/// イェフネンは同じ 4 技(連 / 爆 / スレイ / クラッシュ)を 5 形態で撃ち分ける。
/// 形態はスキル id の接尾で決まるが、**接尾の綴りをフロントに解釈させない**ため副表で持つ
/// (`SUMMON_FORMS` と同じ作法)。
#[rustfmt::skip]
const SKILL_FORMS: &[(&str, SkillForm)] = &[
    ("yefnen_continuous", SkillForm::Sword),
    ("yefnen_explosion", SkillForm::Sword),
    ("yefnen_slay", SkillForm::Sword),
    ("yefnen_crash", SkillForm::Sword),
    ("yefnen_continuous_pike", SkillForm::Pike),
    ("yefnen_explosion_pike", SkillForm::Pike),
    ("yefnen_slay_pike", SkillForm::Pike),
    ("yefnen_crash_pike", SkillForm::Pike),
    ("yefnen_continuous_axe", SkillForm::Axe),
    ("yefnen_explosion_axe", SkillForm::Axe),
    ("yefnen_slay_axe", SkillForm::Axe),
    ("yefnen_crash_axe", SkillForm::Axe),
    ("yefnen_continuous_urumi", SkillForm::Urumi),
    ("yefnen_explosion_urumi", SkillForm::Urumi),
    ("yefnen_slay_urumi", SkillForm::Urumi),
    ("yefnen_crash_urumi", SkillForm::Urumi),
    ("yefnen_continuous_chisel", SkillForm::Chisel),
    ("yefnen_explosion_chisel", SkillForm::Chisel),
    ("yefnen_slay_chisel", SkillForm::Chisel),
    ("yefnen_crash_chisel", SkillForm::Chisel),
];

/// <フラグ>(イェフネンの、技とは別枠のダメージ)を積む技。
/// 韓国公式「귀환」+ wiki「Skill/イェフネン」#Flag(2026-09-21 取得): 連 / 爆 が全形態で積む。
/// 値と計算は `crate::flag`。ここは**技に印を立てる**だけ(`Skill::summon_form` と同じ作法)。
#[rustfmt::skip]
const FLAG_APPLIERS: &[&str] = &[
    "yefnen_continuous", "yefnen_explosion",
    "yefnen_continuous_pike", "yefnen_explosion_pike",
    "yefnen_continuous_axe", "yefnen_explosion_axe",
    "yefnen_continuous_urumi", "yefnen_explosion_urumi",
    "yefnen_continuous_chisel", "yefnen_explosion_chisel",
];

/// 積まれた <フラグ> を爆発させる技(スレイ / クラッシュ。全形態)。出典は `FLAG_APPLIERS` と同じ。
#[rustfmt::skip]
const FLAG_DETONATORS: &[&str] = &[
    "yefnen_slay", "yefnen_crash",
    "yefnen_slay_pike", "yefnen_crash_pike",
    "yefnen_slay_axe", "yefnen_crash_axe",
    "yefnen_slay_urumi", "yefnen_crash_urumi",
    "yefnen_slay_chisel", "yefnen_crash_chisel",
];

/// クールタイム(秒)のうち、生成表(`SKILL_COOLDOWNS`)から外した技。
/// wiki の詳細表に値が 1 つに決まらない形で載っているので手で決める。
/// ここに載っている技は生成表より優先し、`None` なら CT なしとして扱う。
///
/// - `mira_crimson_shooter`(極・紅い射手の砲撃): wiki `Skill/ミラ#CrimsonShooter` の
///   CT 列は **Lv1 だけ 120s、Lv2 以降は 0**(Master = Lv11)。カタログが持つのは SLv11 の
///   性能なので **CT なし**。120s は Lv1 の旧性能(350% x1)に付いた値で、
///   Lv2 以降(760% x4)になると消える。
/// - `roamini_mastary1_2`(極・ベノムノヴァ): wiki `Skill/ロアミニ#Mastary1_2` の CT 列が
///   `30s&br;60s&br;90s`。マスタリー No.1 の選択(【シンボル・オブ・フラッシュ】30s /
///   【〜ステディ】60s / 【〜スピリット】90s)で変わる。カタログはマスタリー選択を
///   持たないので、半減も 1.5 倍も掛からない素の **60s** を既定として収録する
///   (2026-09-21 ユーザー判断。クライアント DB も 60s)。
const MANUAL_COOLDOWNS: &[(&str, Option<f64>)] =
    &[("mira_crimson_shooter", None), ("roamini_mastary1_2", Some(60.0))];

/// クールタイム(秒)を引く。載っていなければ `None`(CT なし = 連打できる)。
pub fn cooldown_of(skill_id: &str) -> Option<f64> {
    if let Some((_, seconds)) = MANUAL_COOLDOWNS.iter().find(|(id, _)| *id == skill_id) {
        return *seconds;
    }
    SKILL_COOLDOWNS
        .iter()
        .find(|(id, _)| *id == skill_id)
        .map(|(_, seconds)| *seconds)
}

/// `SKILL_FORMS` を引く。載っていなければ `None`(形態を持たないキャラのスキル)。
pub fn form_of(skill_id: &str) -> Option<SkillForm> {
    SKILL_FORMS
        .iter()
        .find(|(id, _)| *id == skill_id)
        .map(|(_, form)| *form)
}

/// 速剣(パッシブ)を習得しているときの性能(wiki スキル性能一覧の「(速剣適用時)」行、
/// 2026-09-21 取得)。**ソードシェイプ系 4 技だけ**に載っている。
///
/// wiki ステータスのカテゴリ表は「スキル倍率増加(割合)−10% / スキル段数 +10%」の 2 行だが、
/// 段数の実値は一覧側の行(11→12 / 5→6 / 12→13 / 6→7)を採る。倍率は素の ×0.9
/// (381 → 342.9% 等)。
#[rustfmt::skip]
const SWIFT_SWORDS: &[(&str, SwiftSword)] = &[
    ("yefnen_continuous", SwiftSword { multiplier: 3.429, hit_count: 12 }),
    ("yefnen_explosion", SwiftSword { multiplier: 4.086, hit_count: 6 }),
    ("yefnen_slay", SwiftSword { multiplier: 6.3, hit_count: 13 }),
    ("yefnen_crash", SwiftSword { multiplier: 6.075, hit_count: 7 }),
];

/// チャージで段数が増える技(wiki スキル性能一覧の段数が `8〜17` のように幅で書かれている行、
/// 2026-09-21 取得)。アックスシェイプの スレイ / クラッシュ だけ。チャージタイムは 1 秒で、
/// マスタリー【アックス特化】(`yefnen_m1_2`)を取ると半減する。
#[rustfmt::skip]
const FULL_CHARGES: &[(&str, FullCharge)] = &[
    ("yefnen_slay_axe", FullCharge { hit_count: 17, seconds: 1.0 }),
    ("yefnen_crash_axe", FullCharge { hit_count: 10, seconds: 1.0 }),
];

/// 速剣(パッシブ)の `CharacterSkillDef::id`。習得していると(= `skill_ids` にあると)
/// ソードシェイプ系 4 技の性能が `Skill::swift_sword` に差し替わる。
pub const SWIFT_SWORD_SKILL_ID: &str = "yefnen_swift_sword";
/// 「最大までチャージ」の `CharacterSkillDef::id`。ON のときチャージ技が最大段数になる。
pub const FULL_CHARGE_SKILL_ID: &str = "yefnen_full_charge";
/// マスタリー【アックス特化】の `MasteryDef::id`。チャージタイムが半減する。
pub const AXE_SPECIALIZATION_MASTERY_ID: &str = "yefnen_m1_2";

/// キャラスキル(習得の印)で変わる技の性能を解決する。
///
/// 速剣はソードシェイプ系以外には一切効かず、チャージはチャージ技以外には効かない
/// (どちらも判定は `Skill` 側の印 = `swift_sword` / `full_charge` の有無)。
/// 計算・プレビュー・実測がすべてこの 1 関数を通るので、形態の if を各所に書かない。
pub fn resolve_skill_variants(
    skill: Skill,
    character_skills: &CharacterSkills,
    masteries: &Masteries,
) -> Skill {
    let has = |id: &str| character_skills.skill_ids.iter().any(|s| s == id);
    let mut resolved = skill;
    if has(SWIFT_SWORD_SKILL_ID) {
        resolved = resolved.resolve_swift_sword();
    }
    if has(FULL_CHARGE_SKILL_ID) {
        let halved = masteries
            .picked
            .iter()
            .any(|id| id == AXE_SPECIALIZATION_MASTERY_ID);
        resolved = resolved.resolve_full_charge(halved);
    }
    resolved
}

/// `SUMMON_FORMS` を引く。載っていなければ `None`。
pub fn summon_form_of(skill_id: &str) -> Option<SummonForm> {
    SUMMON_FORMS
        .iter()
        .find(|(id, _)| *id == skill_id)
        .map(|(_, form)| *form)
}

/// 主軸(`main_skill_id`)に召喚スキル(本体以外の攻撃者が撃つスキル)が紛れているときの正規化
/// (2026-09-18 追記)。破壊精霊を足す前は `validate_main_skill` が攻撃者を見ておらず、召喚
/// スキルも本体の主軸に選べてしまっていた。召喚欄(`summon_skill_id`)が空ならそこへ移し、
/// 主軸は未選択(`None`)に戻す。召喚欄が既に埋まっているなら、ユーザーが選んだ値を捨てず
/// 主軸だけ未選択にする(上書きしない)。SQLite の v17 移行(`storage::migrate_summon_skill_out_of_main`)
/// ・IndexedDB の v7 移行・書き出し JSON の読み込み(`transfer.ts`)が同じこの関数を使う
/// (3 か所に同じ if を書かない)。`main_skill_id` が未選択、または本体スキルならそのまま返す。
pub fn normalize_summon_skill_selection(
    main_skill_id: Option<String>,
    summon_skill_id: Option<String>,
) -> (Option<String>, Option<String>) {
    let Some(main_id) = main_skill_id else {
        return (None, summon_skill_id);
    };
    if attacker_of(&main_id) == Attacker::Player {
        return (Some(main_id), summon_skill_id);
    }
    match summon_skill_id {
        None => (None, Some(main_id)),
        // 召喚欄が既に埋まっているなら、主軸に入っていた召喚スキルは**捨てる**。召喚枠は 1 つ
        // (ADR-016 決定 12)なので 2 件は持てず、どちらかを諦めるしかない。既に召喚欄で選んで
        // ある側(意図がはっきりしている方)を残す(レビュー指摘 2026-09-18)。
        Some(existing) => (None, Some(existing)),
    }
}

impl SkillRecord {
    fn skill_id(&self) -> String {
        format!("{}_{}", self.character_id, self.id)
    }

    fn to_skill(&self) -> Skill {
        let base_actual_delay = ACTUAL_DELAYS
            .iter()
            .find(|(id, _)| *id == self.skill_id().as_str())
            .and_then(|(_, delay)| *delay);
        let channeling = channeling_of(&self.skill_id());
        // チャネリング技の 1 回は tick 数ぶん撃つ(wiki の段数は 1 tick ぶん)
        let power = Skill::compute_power(self.multiplier, self.hit_count)
            * f64::from(channeling.map_or(1, |c: domain::Channeling| c.ticks.max(1)));
        Skill {
            id: self.skill_id(),
            name: self.name.to_string(),
            dependency: self.dependency,
            multiplier: self.multiplier,
            hit_count: self.hit_count,
            critical_multiplier: self.critical_multiplier,
            element: self.element,
            weapon_classes: SKILL_WEAPON_CLASSES
                .iter()
                .find(|(id, _)| *id == self.skill_id().as_str())
                .map(|(_, classes)| classes.to_vec())
                .unwrap_or_default(),
            target: SKILL_TARGETS
                .iter()
                .find(|(id, _)| *id == self.skill_id().as_str())
                .map(|(_, target)| *target),
            accuracy: self.accuracy,
            critical_rate: self.critical_rate,
            level: self.level,
            channeling,
            base_actual_delay,
            // チャネリング技にも中ディレイ減少が効く(公式 2025-10-29 no=154871 4-1:
            // 反復周期 × (1 − 減少)。攻撃回数は変わらず、持続が同じ率で縮む)
            actual_delay_fixed: ACTUAL_DELAY_FIXED.contains(&self.skill_id().as_str()),
            // 通常攻撃は wiki スキル性能一覧の † (基本攻撃)。名前がそのまま印になっている
            normal_attack: self.name.starts_with('†'),
            combo_interval: COMBO_INTERVALS
                .iter()
                .find(|(id, _)| *id == self.skill_id().as_str())
                .map(|(_, seconds)| *seconds),
            combo_variants: if self.skill_id() == "maximin_continuous" {
                vec![
                    ComboSkillVariant {
                        combo_type: ComboSkillType::General,
                        multiplier: 5.55,
                        hit_count: 11,
                        base_actual_delay: 1.4,
                    },
                    ComboSkillVariant {
                        combo_type: ComboSkillType::Instant,
                        multiplier: 5.20,
                        hit_count: 10,
                        base_actual_delay: 1.0,
                    },
                    ComboSkillVariant {
                        combo_type: ComboSkillType::Chain,
                        multiplier: 5.20,
                        hit_count: 12,
                        base_actual_delay: 1.6,
                    },
                ]
            } else {
                Vec::new()
            },
            power,
            power_per_second: Skill::compute_power_per_second(
                power,
                base_actual_delay,
                cooldown_of(&self.skill_id()),
            ),
            attacker: attacker_of(&self.skill_id()),
            summon_form: summon_form_of(&self.skill_id()),
            form: form_of(&self.skill_id()),
            swift_sword: SWIFT_SWORDS
                .iter()
                .find(|(id, _)| *id == self.skill_id().as_str())
                .map(|(_, variant)| *variant),
            full_charge: FULL_CHARGES
                .iter()
                .find(|(id, _)| *id == self.skill_id().as_str())
                .map(|(_, charge)| *charge),
            charge_seconds: 0.0,
            applies_flag: FLAG_APPLIERS.contains(&self.skill_id().as_str()),
            detonates_flag: FLAG_DETONATORS.contains(&self.skill_id().as_str()),
            cooldown_seconds: cooldown_of(&self.skill_id()),
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

/// 全キャラのスキルを 1 件ずつ。id ではなく**技に立っている印**(形態・対象指定・
/// <フラグ> を積むか)から技を引きたいときに使う(`flag::flag_applier_for`)。
pub fn all_skills() -> impl Iterator<Item = Skill> {
    SKILLS.iter().map(SkillRecord::to_skill)
}

/// 回しに差し込む CT 技(`NewCharacter::rotation_skill_ids`)から、**選べなくなった id** を
/// 落とす。選べるのは「そのキャラが自分で撃つ攻撃技」かつ「クールタイムを持つ」技だけ
/// (`commands::validate_rotation_skills` と同じ規則を 1 か所に持つ)。
///
/// カタログから技が消える・改名される・キャラ種を変えると保存済みの id が選べなくなり、
/// そのままだと保存の検証で弾かれて自動保存が止まる。キャラスキルの
/// `normalize_character_skill_selection` と同じ形で、落としたら `true` を返す。
pub fn retain_rotation_skills(ids: &mut Vec<String>, character_id: &str) -> bool {
    let before = ids.len();
    let selectable: Vec<String> = skills_for(character_id)
        .into_iter()
        .filter(|s| s.attacker == Attacker::Player && s.cooldown_seconds.is_some())
        .map(|s| s.id)
        .collect();
    ids.retain(|id| selectable.contains(id));
    ids.len() != before
}

pub fn find_skill(id: &str) -> Option<Skill> {
    SKILLS
        .iter()
        .find(|s| s.skill_id() == id)
        .map(SkillRecord::to_skill)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// wiki `STAB(熊)` 行の対象 8 件だけ `MagicDoll`。ベアステップ 2 件は本体、
    /// 他キャラの代表 id も `Player`(2026-09-18 取得)。
    #[test]
    fn 魔法人形が撃つスキルは8件で他は本体() {
        assert_eq!(MAGIC_DOLL_SKILLS.len(), 8);
        for id in MAGIC_DOLL_SKILLS {
            assert_eq!(attacker_of(id), Attacker::MagicDoll, "{id}");
            assert!(find_skill(id).is_some(), "{id} がカタログに無い");
        }
        assert_eq!(attacker_of("anais_mica_bear_step"), Attacker::Player);
        assert_eq!(attacker_of("anais_rucy_bear_step"), Attacker::Player);
        assert_eq!(attacker_of("lucian_butt"), Attacker::Player);
    }

    /// 主軸スキル(ベアステップ・陣)が召喚獣の型を決める。型を決めない共通スキルは None
    /// (2026-09-18 追記)。
    #[test]
    fn 主軸スキルは召喚獣の型を決める() {
        assert_eq!(summon_form_of("anais_mica_bear_step"), Some(SummonForm::MicaBear));
        assert_eq!(summon_form_of("anais_rucy_bear_step"), Some(SummonForm::RucyBear));
        assert_eq!(summon_form_of("anais_tesla_coil"), Some(SummonForm::Anferu));
        assert_eq!(summon_form_of("anais_ice_age"), Some(SummonForm::Gureshisu));
        assert_eq!(summon_form_of("anais_flare_field"), Some(SummonForm::Igni));
        assert_eq!(summon_form_of("anais_fairy_light"), None);
        assert_eq!(summon_form_of("anais_angry_pixie"), None);
        assert_eq!(summon_form_of("anais_dissonance"), None);
        assert_eq!(summon_form_of("anais_cacophony"), None);
    }

    /// 召喚スキル(熊 4 件 × 2 体・精霊 8 件)は自分が属する型を持ち、型ごとの候補が
    /// `MAGIC_DOLL_SKILLS` / `DESTRUCTION_SPIRIT_SKILLS` と過不足なく一致する(2026-09-18 追記)。
    #[test]
    fn 召喚スキルは自分の型を持つ() {
        let mica = ["anais_thrust", "anais_mica_even_bear", "anais_mica_footstep", "anais_judgment_spin"];
        let rucy = ["anais_strike", "anais_rucy_even_bear", "anais_rucy_footstep", "anais_deathmoment"];
        let anferu = ["anais_lightning_attack", "anais_chain_lightning"];
        let gureshisu = ["anais_crystal_attack", "anais_crystal_sprinter", "anais_ring_of_ice"];
        let igni = ["anais_flame_attack", "anais_fire_blast", "anais_detonate"];
        for id in mica {
            assert_eq!(summon_form_of(id), Some(SummonForm::MicaBear), "{id}");
        }
        for id in rucy {
            assert_eq!(summon_form_of(id), Some(SummonForm::RucyBear), "{id}");
        }
        for id in anferu {
            assert_eq!(summon_form_of(id), Some(SummonForm::Anferu), "{id}");
        }
        for id in gureshisu {
            assert_eq!(summon_form_of(id), Some(SummonForm::Gureshisu), "{id}");
        }
        for id in igni {
            assert_eq!(summon_form_of(id), Some(SummonForm::Igni), "{id}");
        }
        // 型を持つ召喚スキルの総数は MAGIC_DOLL_SKILLS + DESTRUCTION_SPIRIT_SKILLS と一致する
        let typed_summon_skills = MAGIC_DOLL_SKILLS
            .iter()
            .chain(DESTRUCTION_SPIRIT_SKILLS)
            .filter(|id| summon_form_of(id).is_some())
            .count();
        assert_eq!(typed_summon_skills, MAGIC_DOLL_SKILLS.len() + DESTRUCTION_SPIRIT_SKILLS.len());
    }

    /// 主軸に召喚スキルが紛れていたら召喚欄へ移し、主軸は未選択に戻す(2026-09-18 追記)。
    #[test]
    fn 正規化は主軸の召喚スキルを召喚欄へ移す() {
        let (main, summon) = normalize_summon_skill_selection(
            Some("anais_mica_even_bear".to_string()),
            None,
        );
        assert_eq!(main, None);
        assert_eq!(summon, Some("anais_mica_even_bear".to_string()));
    }

    /// 召喚欄が既に埋まっているなら上書きしない。主軸だけ未選択にする(2026-09-18 追記)。
    #[test]
    fn 正規化は召喚欄が埋まっていれば上書きしない() {
        let (main, summon) = normalize_summon_skill_selection(
            Some("anais_lightning_attack".to_string()),
            Some("anais_mica_even_bear".to_string()),
        );
        assert_eq!(main, None);
        assert_eq!(summon, Some("anais_mica_even_bear".to_string()));
    }

    /// 本体スキル・未選択はそのまま返す(2026-09-18 追記)。
    #[test]
    fn 正規化は本体スキルと未選択をそのまま返す() {
        assert_eq!(
            normalize_summon_skill_selection(Some("anais_angry_pixie".to_string()), None),
            (Some("anais_angry_pixie".to_string()), None)
        );
        assert_eq!(normalize_summon_skill_selection(None, None), (None, None));
        assert_eq!(
            normalize_summon_skill_selection(None, Some("anais_mica_even_bear".to_string())),
            (None, Some("anais_mica_even_bear".to_string()))
        );
    }

    /// 破壊精霊が撃つスキルは 8 件、陣 3 件は本体扱いのまま(2026-09-18 取得)。
    #[test]
    fn 破壊精霊が撃つスキルは8件で陣は本体扱い() {
        assert_eq!(DESTRUCTION_SPIRIT_SKILLS.len(), 8);
        for id in DESTRUCTION_SPIRIT_SKILLS {
            assert_eq!(attacker_of(id), Attacker::DestructionSpirit, "{id}");
            assert!(find_skill(id).is_some(), "{id} がカタログに無い");
        }
        for id in ["anais_tesla_coil", "anais_ice_age", "anais_flare_field"] {
            assert_eq!(attacker_of(id), Attacker::Player, "{id}");
        }
    }

    #[test]
    fn マキシミンの連は3つのコンボタイプを持つ() {
        let skill = find_skill("maximin_continuous").unwrap();
        assert_eq!(skill.combo_variants.len(), 3);
        assert_eq!(skill.combo_variants[0].combo_type, ComboSkillType::General);
        assert_eq!(
            (
                skill.combo_variants[0].multiplier,
                skill.combo_variants[0].hit_count,
                skill.combo_variants[0].base_actual_delay,
            ),
            (5.55, 11, 1.4),
        );
        assert_eq!(
            (
                skill.combo_variants[1].multiplier,
                skill.combo_variants[1].hit_count,
                skill.combo_variants[1].base_actual_delay,
            ),
            (5.20, 10, 1.0),
        );
        assert_eq!(
            (
                skill.combo_variants[2].multiplier,
                skill.combo_variants[2].hit_count,
                skill.combo_variants[2].base_actual_delay,
            ),
            (5.20, 12, 1.6),
        );
    }

    /// wiki スキル性能一覧の「動作」列。全 303 件ぶん引けて、秒が読めなかったのは
    /// ティチエル 極・スパークリングカイト(wiki 表記が `0`)の 1 件だけ。
    #[test]
    fn 基本中ディレイは全スキルぶん引けて未取得は1件() {
        assert_eq!(ACTUAL_DELAYS.len(), SKILLS.len());
        let mut ids: Vec<&str> = ACTUAL_DELAYS.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SKILLS.len());
        for record in SKILLS {
            let id = record.skill_id();
            assert!(
                ACTUAL_DELAYS.iter().any(|(k, _)| *k == id),
                "{id} の動作が無い"
            );
        }
        // 「動作」が秒として読めない行はもう無い(スパークリングカイトは詳細表から採った)
        let missing: Vec<&str> = ACTUAL_DELAYS
            .iter()
            .filter(|(_, d)| d.is_none())
            .map(|(id, _)| *id)
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
        // 秒が読めた行はすべて正の値
        assert!(ACTUAL_DELAYS
            .iter()
            .filter_map(|(_, d)| *d)
            .all(|d| d > 0.0));
    }

    /// チャネリング技の表(生成物)。区分 `続` の 16 件で、単体チャネリング 7 件はその部分集合。
    /// どれも 1 回の使用で 10 tick(持続 ÷ tick 間隔)。
    #[test]
    fn チャネリング技は16件で1回に10tick撃つ() {
        assert_eq!(SKILL_CHANNELING.len(), 16);
        for (id, ticks, tick_seconds) in SKILL_CHANNELING {
            let skill = find_skill(id).unwrap_or_else(|| panic!("{id} がカタログに無い"));
            let channeling = skill.channeling.unwrap_or_else(|| panic!("{id} に印が立っていない"));
            assert_eq!(channeling.ticks, *ticks, "{id}");
            assert_eq!(*ticks, 10, "{id}");
            assert!(*tick_seconds > 0.0, "{id}");
            // 持続(動作)= tick 間隔 × tick 数
            let lasts = skill.base_actual_delay.unwrap_or_else(|| panic!("{id} の動作が無い"));
            assert!(
                (lasts - tick_seconds * f64::from(*ticks)).abs() < 1e-9,
                "{id} 動作 {lasts} / {tick_seconds} × {ticks}"
            );
            // 中ディレイ減少が効く(公式 2025-10-29 no=154871 4-1)
            assert!(!skill.actual_delay_fixed, "{id}");
            // 火力の目安は tick 込み(段数は 1 tick ぶん)
            assert!(
                (skill.power - skill.multiplier * f64::from(skill.hit_count) * f64::from(*ticks))
                    .abs()
                    < 1e-9,
                "{id}"
            );
        }
        // 単体も範囲も入る
        assert!(SKILL_CHANNELING.iter().any(|(k, _, _)| *k == "lucian_streak"));
        assert!(SKILL_CHANNELING.iter().any(|(k, _, _)| *k == "tichiel_blizzard"));
    }

    #[test]
    fn 固定の中ディレイは2件でカタログにある() {
        for id in ACTUAL_DELAY_FIXED {
            let skill = find_skill(id).unwrap_or_else(|| panic!("{id} がカタログに無い"));
            assert!(skill.actual_delay_fixed, "{id} にフラグが立っていない");
            assert_eq!(skill.base_actual_delay, Some(0.8), "{id} の動作");
        }
    }

    #[test]
    fn 中ディレイのスポットチェック() {
        // wiki Skill/ボリス: 極・残影斬 1.4s / †極・横斬り 0.8s
        assert_eq!(
            find_skill("boris_blur_sword").unwrap().base_actual_delay,
            Some(1.4)
        );
        assert_eq!(
            find_skill("boris_horizontal_sword")
                .unwrap()
                .base_actual_delay,
            Some(0.8)
        );
        // wiki Skill/ルシアン: 極・連撃(チャネリング)は 10s
        assert_eq!(
            find_skill("lucian_streak").unwrap().base_actual_delay,
            Some(10.0)
        );
        // wiki Skill/イェフネン: 極・連・パイクは `1.5s/1s`([加速]時)。基本値の 1.5s を採る
        assert_eq!(
            find_skill("yefnen_continuous_pike")
                .unwrap()
                .base_actual_delay,
            Some(1.5)
        );
    }

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
            assert!(
                (s.critical_multiplier - critical).abs() < 1e-9,
                "{id} の Cri倍"
            );
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
            assert!(
                crate::find_character(s.character_id).is_some(),
                "{} が一覧に無い",
                s.character_id
            );
        }
    }

    // --- クールタイム ---

    #[test]
    fn ct表のidは実在して重複しない() {
        let mut ids: Vec<&str> = SKILL_COOLDOWNS.iter().map(|(id, _)| *id).collect();
        for id in &ids {
            assert!(find_skill(id).is_some(), "{id} がカタログに無い");
        }
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "CT 表に重複がある");
        for (id, _) in MANUAL_COOLDOWNS {
            assert!(find_skill(id).is_some(), "{id} がカタログに無い");
            assert!(
                !ids.contains(id),
                "{id} は手書きの例外なので生成表に載ってはいけない"
            );
        }
    }

    /// wiki 各キャラ `Skill/<キャラ名>` のスキル詳細表 CT 列(ミラーの 2026-09-20 版)。
    /// ボリス / イスピンは CT を持つスキルが 1 つも無い。
    #[test]
    fn ct表の件数はキャラ別に固定() {
        let expected = [
            ("anais", 9),
            ("benya", 6),
            ("chloe", 6),
            ("isaac", 4),
            ("isolet", 10),
            ("joshua", 3),
            ("leeche", 3),
            ("lucian", 3),
            ("maximin", 1),
            ("mira", 1),
            ("nayatorei", 4),
            ("nocturne", 4),
            ("ranjie", 1),
            ("roamini", 5), // 生成表 4 + 例外表のベノムノヴァ(60s)
            ("siberin", 3),
            ("tichiel", 5),
            ("yefnen", 10),
        ];
        assert_eq!(SKILL_COOLDOWNS.len(), 77);
        // 生成表 77 件 + 例外表で CT を持つ 1 件
        assert_eq!(expected.iter().map(|(_, n)| n).sum::<usize>(), 78);
        for (character_id, count) in expected {
            let found = SKILLS
                .iter()
                .filter(|s| s.character_id == character_id)
                .filter(|s| cooldown_of(&s.skill_id()).is_some())
                .count();
            assert_eq!(found, count, "{character_id} の CT 持ち件数");
        }
        for character_id in ["boris", "ispin"] {
            assert!(SKILLS
                .iter()
                .filter(|s| s.character_id == character_id)
                .all(|s| cooldown_of(&s.skill_id()).is_none()));
        }
    }

    #[test]
    fn ctの代表値() {
        for (id, seconds) in [
            ("lucian_streak", Some(10.0)),
            ("chloe_meteor_strike", Some(60.0)),
            ("nayatorei_assault", Some(5.0)),
            ("benya_hell_gate", Some(60.0)),
            ("tichiel_fire_ball", None),
            ("lucian_butt", None),
            // 例外表(実用 SLv では CT なし / マスタリーで変わる値は素の 60s)
            ("mira_crimson_shooter", None),
            ("roamini_mastary1_2", Some(60.0)),
        ] {
            assert_eq!(find_skill(id).unwrap().cooldown_seconds, seconds, "{id}");
        }
        // イェフネンの スレイ / クラッシュ 10 件は全形態 10s(旧 COOLDOWNS と同じ)
        for id in [
            "yefnen_slay",
            "yefnen_crash",
            "yefnen_slay_pike",
            "yefnen_crash_pike",
            "yefnen_slay_axe",
            "yefnen_crash_axe",
            "yefnen_slay_urumi",
            "yefnen_crash_urumi",
            "yefnen_slay_chisel",
            "yefnen_crash_chisel",
        ] {
            assert_eq!(cooldown_of(id), Some(10.0), "{id}");
        }
    }
}
