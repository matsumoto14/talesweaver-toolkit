//! 召喚獣命中時の追加ダメージ(アナイス 極・ダメージプラス)。
//!
//! 出典: wiki「Skill/アナイス」「召喚関連スキル」の別表(`#DamagePlus`、2026-09-23 取得)、
//! 韓国公式 ActionInfo/11_Anais/3002610.htm。
//!
//! > 10秒間、敵に破壊精霊の攻撃を受けた時に追加ダメージが発生する状態異常を付与する。
//! > 対象には追加ダメージを2回与え、半径15以内の敵にも追加ダメージを1回与える。
//! > 追加ダメージ[%]＝100+(素INT＋魔法攻撃力)/10、発動クールタイム1s
//! > 対象指定: 単体 / SLv 5 / ダメージ上限 175% / 動作 0.9s(採用値は skills.rs 参照)/ CT30s
//!
//! 周囲 15 への 1 回は単体 DPS では数えない(対象への 2 回だけ持つ)。
#[rustfmt::skip]
pub(crate) const SKILL_SUMMON_HIT_BONUS: &[(&str, domain::SummonHitBonus)] = &[
    ("anais_damage_plus", domain::SummonHitBonus {
        duration_seconds: 10.0,
        base_ratio_percent: 100,
        max_ratio_percent: 175,
        reactivation_min_seconds: 1.0,
        hits_per_activation: 2,
    }),
];
