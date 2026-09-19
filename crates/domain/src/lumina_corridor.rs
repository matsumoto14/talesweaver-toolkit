//! ルミナの回廊の回廊効果(wiki: ミニゲーム/ルミナの回廊 `#CorridorBuff`、取得 2026-09-19)。
//!
//! 回廊ポイントを払って習得する**恒常バフ**で、テイルズID 内の全キャラに適用される。
//! 2 週間のコンテンツリセットでも消えない(消す場合は別途告知)。
//!
//! wiki の表は 8 行あるが、このツールが持つのは**キャラの強さに効く 4 行**だけ。
//! 残る 4 行(週間 SEED 上限 / 週間 ELSO 上限 / ELSO 獲得量 / 回廊ポイント獲得量)は
//! 経済の値で、与ダメージにも能力値にも効かないので収録しない(0 で埋めず、画面で
//! 「ここでは扱わない」と言う)。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::category::DamageCategory;
use crate::damage::DamageContribution;
use crate::element::{Element, ElementValues};

/// 最終ダメージ(カテゴリL)。1 レベルあたり +1%、最大 Lv5 = +5%
pub const CORRIDOR_FINAL_DAMAGE_RATE_PER_LEVEL: f64 = 0.01;
pub const CORRIDOR_FINAL_DAMAGE_LEVEL_MAX: u8 = 5;
/// 全属性増加。1 レベルあたり全属性 +1、最大 Lv10 = +10
/// (**各属性にそれぞれ +10**。ユーザー確定 2026-09-19 — wiki の表は「全属性」としか書かない)
pub const CORRIDOR_ELEMENT_PER_LEVEL: i64 = 1;
pub const CORRIDOR_ELEMENT_LEVEL_MAX: u8 = 10;
/// ダメージ減少(被ダメージ側)。1 レベルあたり +1%、最大 Lv5
pub const CORRIDOR_DAMAGE_REDUCTION_LEVEL_MAX: u8 = 5;
/// HP / MP / SP 増加。1 レベルあたり +0.5%、最大 Lv10
pub const CORRIDOR_HP_MP_SP_LEVEL_MAX: u8 = 10;

/// 習得している回廊効果のレベル。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LuminaCorridor {
    /// 最終ダメージ(カテゴリL)。与ダメージに乗る
    #[serde(default)]
    pub final_damage_level: u8,
    /// 全属性増加。全 8 属性にレベルぶん加算する
    #[serde(default)]
    pub all_element_level: u8,
    /// ダメージ減少(被ダメージ側)。**記録のみ** — 被ダメージ側はまだモデルが無い
    #[serde(default)]
    pub damage_reduction_level: u8,
    /// HP / MP / SP 増加。**記録のみ** — HP / MP / SP はこのツールの能力値 7 種に無い
    #[serde(default)]
    pub hp_mp_sp_level: u8,
}

#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum LuminaCorridorError {
    #[error("ルミナの回廊「{effect}」は Lv{max} までです(Lv{level})")]
    LevelOutOfRange {
        effect: &'static str,
        level: u8,
        max: u8,
    },
}

impl LuminaCorridor {
    pub fn validate(&self) -> Result<(), LuminaCorridorError> {
        for (effect, level, max) in [
            (
                "最終ダメージ",
                self.final_damage_level,
                CORRIDOR_FINAL_DAMAGE_LEVEL_MAX,
            ),
            (
                "全属性増加",
                self.all_element_level,
                CORRIDOR_ELEMENT_LEVEL_MAX,
            ),
            (
                "ダメージ減少",
                self.damage_reduction_level,
                CORRIDOR_DAMAGE_REDUCTION_LEVEL_MAX,
            ),
            (
                "HP MP SP 増加",
                self.hp_mp_sp_level,
                CORRIDOR_HP_MP_SP_LEVEL_MAX,
            ),
        ] {
            if level > max {
                return Err(LuminaCorridorError::LevelOutOfRange { effect, level, max });
            }
        }
        Ok(())
    }

    /// 最終ダメージ(カテゴリL)の割合。Σ% の小数表現
    pub fn final_damage_rate(self) -> f64 {
        f64::from(self.final_damage_level) * CORRIDOR_FINAL_DAMAGE_RATE_PER_LEVEL
    }

    /// 全属性への加算。**8 属性それぞれ**にレベルぶん乗る
    pub fn element_values(self) -> ElementValues {
        let bonus = i64::from(self.all_element_level) * CORRIDOR_ELEMENT_PER_LEVEL;
        let mut values = ElementValues::default();
        for element in Element::ALL {
            *values.get_mut(element) = bonus;
        }
        values
    }

    pub fn damage_contributions(self) -> Vec<DamageContribution> {
        let rate = self.final_damage_rate();
        if rate == 0.0 {
            return Vec::new();
        }
        vec![DamageContribution {
            source: "ルミナの回廊".to_string(),
            category: DamageCategory::FinalDamageRate,
            value: rate,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 全属性増加は8属性それぞれに乗る() {
        let corridor = LuminaCorridor {
            all_element_level: 10,
            ..Default::default()
        };
        let values = corridor.element_values();
        for element in Element::ALL {
            assert_eq!(values.get(element), 10, "{element:?}");
        }
    }

    #[test]
    fn 最終ダメージはlvごとに1パーセントでカテゴリlに乗る() {
        let corridor = LuminaCorridor {
            final_damage_level: 5,
            ..Default::default()
        };
        assert!((corridor.final_damage_rate() - 0.05).abs() < f64::EPSILON);
        let contributions = corridor.damage_contributions();
        assert_eq!(contributions.len(), 1);
        assert_eq!(contributions[0].category, DamageCategory::FinalDamageRate);
        assert_eq!(contributions[0].source, "ルミナの回廊");
    }

    #[test]
    fn 未習得なら寄与を出さない() {
        assert!(LuminaCorridor::default().damage_contributions().is_empty());
        assert_eq!(
            LuminaCorridor::default().element_values(),
            ElementValues::default()
        );
    }

    #[test]
    fn 上限を超えたレベルは拒否する() {
        let corridor = LuminaCorridor {
            all_element_level: CORRIDOR_ELEMENT_LEVEL_MAX + 1,
            ..Default::default()
        };
        assert_eq!(
            corridor.validate(),
            Err(LuminaCorridorError::LevelOutOfRange {
                effect: "全属性増加",
                level: 11,
                max: 10,
            })
        );
    }
}
