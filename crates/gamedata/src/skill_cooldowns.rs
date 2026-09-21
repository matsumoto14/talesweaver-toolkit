//! スキルのクールタイム(CT、秒)。
//!
//! 出典: wiki 各キャラの `Skill/<キャラ名>` ページ。スキル詳細表(`&aname(…);` の直後)の
//! ヘッダ末尾 `CT` 列。CT 列が無い / `0` の技は CT なしなのでここに載せない。
//! `[一覧]` 印の行は詳細表が未記入(`?`)なのでスキル性能一覧の `CT/消費` 列から採った。
//! `tools/gamedata/skill_cooldowns.py` が生成する。手で編集しない。
//!
//! クライアント DB と食い違う技のうち DPS に効く 2 件は韓国公式スキル情報で裏取り済み(2026-09-21)。
//! いずれも wiki が正: `nocturne_magnetic_force` 재사용 대기 시간 1 분(ActionInfo/16_Nocturne/3004075.htm)、
//! `nocturne_satellite_canon` 30 초(同 3004076.htm、21.07.12 V820 で CT の誤記を修正済みとある)。

#[rustfmt::skip]
pub(crate) const SKILL_COOLDOWNS: &[(&str, f64)] = &[
    ("anais_chain_lightning", 1.0),  // アナイス 極・チェーンライトニング
    ("anais_crystal_sprinter", 1.0),  // アナイス 極・クリスタルスプリンター
    ("anais_deathmoment", 1.0),  // アナイス 極・デスモーメント
    ("anais_detonate", 1.0),  // アナイス 極・ディトネート
    ("anais_fire_blast", 1.0),  // アナイス 極・ファイアブラスト
    ("anais_judgment_spin", 1.0),  // アナイス 極・ジャッジメントスピン
    ("anais_ring_of_ice", 1.0),  // アナイス 極・リングオブアイス
    ("anais_rucy_even_bear", 1.0),  // アナイス 極・熊連
    ("anais_rucy_footstep", 1.0),  // アナイス 極・足踏み
    ("benya_death_chain", 8.0),  // ベンヤ 極・デスチェーン
    ("benya_earth_dive", 3.0),  // ベンヤ 極・アースダイブ
    ("benya_guillotine", 3.0),  // ベンヤ 極・ギロチン
    ("benya_hell_gate", 60.0),  // ベンヤ 極・ヘルゲート
    ("benya_paul_hammer", 10.0),  // ベンヤ 極・ポールハンマー
    ("benya_soul_steal", 60.0),  // ベンヤ 極・ソウルスチール
    ("chloe_electric_ball", 60.0),  // クロエ 極・エレクトリックボール
    ("chloe_extraction", 5.0),  // クロエ 極・エクストーション
    ("chloe_icicle_rain", 60.0),  // クロエ 極・アイシクルレイン
    ("chloe_meteor_strike", 60.0),  // クロエ 極・メテオストライク
    ("chloe_sand_storm", 60.0),  // クロエ 極・サンドストーム
    ("chloe_tornado", 60.0),  // クロエ 極・トルネード
    ("isaac_break_through", 5.0),  // イサック 極・ブレイクスルー
    ("isaac_demise_furious", 10.0),  // イサック 極・滅神乱舞
    ("isaac_energy_field", 10.0),  // イサック 極・エネルギーフィールド
    ("isaac_lion_fear", 5.0),  // イサック 極・獅子吼
    ("isolet_back_blade", 3.0),  // イソレット 極・バックブレイド
    ("isolet_dash_blade", 3.0),  // イソレット 極・ダッシュブレイド
    ("isolet_gravity_field", 3.0),  // イソレット 極・グラビティフィールド
    ("isolet_holy_bird", 6.0),  // イソレット 極・ホーリーバード
    ("isolet_holy_light", 1.0),  // イソレット 極・ホーリーライト
    ("isolet_sonic_wave", 6.0),  // イソレット 極・ソニックウェーブ
    ("isolet_storm_blade", 6.0),  // イソレット 極・ストームブレード
    ("isolet_storm_blast", 8.0),  // イソレット 極・ストームブラスト
    ("isolet_storm_dance", 6.0),  // イソレット 極・ストームダンス
    ("isolet_zone_burst", 1.0),  // イソレット 極・ゾーンバースト
    ("joshua_finale", 5.0),  // ジョシュア 極・フィナーレ
    ("joshua_iron_mist", 30.0),  // ジョシュア 極・黒霧雲
    ("joshua_soul_burst", 30.0),  // ジョシュア 極・ソウルバースト
    ("leeche_anarose_skill_5", 3.0),  // リーチェ 極・クラヴァータ
    ("leeche_armor_of_evil_skill_11", 3.0),  // リーチェ 極・スピロ
    ("leeche_armor_of_evil_skill_13", 3.0),  // リーチェ 極・アーゴグロッソ
    ("lucian_streak", 10.0),  // ルシアン 極・連撃
    ("lucian_warriors_dance", 10.0),  // ルシアン 極・無双乱舞
    ("lucian_whirlwind_sword", 10.0),  // ルシアン 極・旋風斬
    ("maximin_wind_storm", 60.0),  // マキシミン 極・ウィンドストーム
    ("mira_dew_storm", 5.0),  // ミラ 極・ダガーストーム
    ("nayatorei_assault", 5.0),  // ナヤトレイ 極・襲撃
    ("nayatorei_flash", 5.0),  // ナヤトレイ 極・忍術 閃
    ("nayatorei_mausoleum", 10.0),  // ナヤトレイ 極・狂猫
    ("nayatorei_wide_assault", 5.0),  // ナヤトレイ 極・無差別的な襲撃
    ("nocturne_cluster_rocket", 5.0),  // ノクターン 極・クラスターロケット
    ("nocturne_magnetic_force", 60.0),  // ノクターン 極・マグネティックフォース
    ("nocturne_quantum_nuclear", 7.0),  // ノクターン 極・クアンタムニュークリア
    ("nocturne_satellite_canon", 30.0),  // ノクターン 極・サテライトカノン
    ("ranjie_piercing_shot", 5.0),  // ランジエ 極・ピアシングショット
    ("roamini_mastary2_3", 5.0),  // ロアミニ 極・怨恨
    ("roamini_mastary2_4", 5.0),  // ロアミニ 極・カース・エンド
    ("roamini_mastary3_2", 30.0),  // ロアミニ 極・逃走
    ("roamini_mastary3_3", 5.0),  // ロアミニ 極・浸透
    ("siberin_bombing", 10.0),  // シベリン 極・爆撃
    ("siberin_red_dragon_strike", 10.0),  // シベリン 極・紅龍連撃
    ("siberin_twin_dragon_strike", 10.0),  // シベリン 極・双龍撃
    ("tichiel_blizzard", 60.0),  // ティチエル 極・ブリザード
    ("tichiel_burning_air", 10.0),  // ティチエル 極・バーニングエア
    ("tichiel_frost_coating", 5.0),  // ティチエル 極・フロストコーティング
    ("tichiel_giga_blaze", 60.0),  // ティチエル 極・ギガブレイズ
    ("tichiel_sparkling_kite", 10.0),  // ティチエル 極・スパークリングカイト
    ("yefnen_crash", 10.0),  // [一覧] イェフネン 極・クラッシュ
    ("yefnen_crash_axe", 10.0),  // [一覧] イェフネン 極・クラッシュ・アックス
    ("yefnen_crash_chisel", 10.0),  // [一覧] イェフネン 極・クラッシュ・チゼル
    ("yefnen_crash_pike", 10.0),  // [一覧] イェフネン 極・クラッシュ・パイク
    ("yefnen_crash_urumi", 10.0),  // [一覧] イェフネン 極・クラッシュ・ウルミ
    ("yefnen_slay", 10.0),  // [一覧] イェフネン 極・スレイ
    ("yefnen_slay_axe", 10.0),  // [一覧] イェフネン 極・スレイ・アックス
    ("yefnen_slay_chisel", 10.0),  // [一覧] イェフネン 極・スレイ・チゼル
    ("yefnen_slay_pike", 10.0),  // [一覧] イェフネン 極・スレイ・パイク
    ("yefnen_slay_urumi", 10.0),  // [一覧] イェフネン 極・スレイ・ウルミ
];
