//! ソウルリンクのリンクステータス。
//!
//! 1〜4 は装備の基本能力値へ直接加算、5〜7 はダメージ式に反映する。
//! リンク枠と習得条件は扱わず、条件を満たしている前提でキャラクターごとに保存する。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{DamageCategory, DamageContribution, EquipmentValueSource, EquipmentValues};

pub const SOUL_LINK_EQUIPMENT_LEVEL_MAX: u8 = 25;
pub const SOUL_LINK_EQUIPMENT_VALUE_PER_LEVEL: i64 = 2;
pub const SOUL_LINK_CRITICAL_DAMAGE_LEVEL_MAX: u8 = 20;
pub const SOUL_LINK_CRITICAL_DAMAGE_RATE_PER_LEVEL: f64 = 0.015;
pub const SOUL_LINK_FINAL_DAMAGE_LEVEL_MAX: u8 = 5;
pub const SOUL_LINK_FINAL_DAMAGE_RATE_PER_LEVEL: f64 = 0.04;
pub const SOUL_LINK_WEAPON_ENHANCE_LEVEL_MAX: u8 = 20;
pub const SOUL_LINK_WEAPON_ENHANCE_RATE_PER_LEVEL: f64 = 0.10;
pub const SOUL_LINK_ARMOR_ENHANCE_LEVEL_MAX: u8 = 20;
pub const SOUL_LINK_ARMOR_ENHANCE_HP_RATE_PER_LEVEL: f64 = 0.05;

/// リンクステータス 1〜8 の現在 Lv。
/// `Default` は計算の中立値。新規キャラの実用既定値は `maxed` で明示する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SoulLinkStatus {
    pub thrust_level: u8,
    pub slash_level: u8,
    pub magic_attack_level: u8,
    pub magic_defense_level: u8,
    pub critical_damage_level: u8,
    pub final_damage_level: u8,
    pub weapon_enhance_level: u8,
    pub armor_enhance_level: u8,
}

impl SoulLinkStatus {
    pub fn maxed() -> Self {
        Self {
            thrust_level: SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            slash_level: SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            magic_attack_level: SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            magic_defense_level: SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            critical_damage_level: SOUL_LINK_CRITICAL_DAMAGE_LEVEL_MAX,
            final_damage_level: SOUL_LINK_FINAL_DAMAGE_LEVEL_MAX,
            weapon_enhance_level: SOUL_LINK_WEAPON_ENHANCE_LEVEL_MAX,
            armor_enhance_level: SOUL_LINK_ARMOR_ENHANCE_LEVEL_MAX,
        }
    }

    pub fn validate(self) -> Result<(), SoulLinkError> {
        for (name, level, max) in [
            (
                "突き攻撃力",
                self.thrust_level,
                SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            ),
            (
                "斬り攻撃力",
                self.slash_level,
                SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            ),
            (
                "魔法攻撃力",
                self.magic_attack_level,
                SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            ),
            (
                "魔法防御力",
                self.magic_defense_level,
                SOUL_LINK_EQUIPMENT_LEVEL_MAX,
            ),
            (
                "クリティカルダメージ",
                self.critical_damage_level,
                SOUL_LINK_CRITICAL_DAMAGE_LEVEL_MAX,
            ),
            (
                "最終ダメージ",
                self.final_damage_level,
                SOUL_LINK_FINAL_DAMAGE_LEVEL_MAX,
            ),
            (
                "武器強化",
                self.weapon_enhance_level,
                SOUL_LINK_WEAPON_ENHANCE_LEVEL_MAX,
            ),
            (
                "鎧強化",
                self.armor_enhance_level,
                SOUL_LINK_ARMOR_ENHANCE_LEVEL_MAX,
            ),
        ] {
            if level > max {
                return Err(SoulLinkError::LevelOutOfRange { name, level, max });
            }
        }
        Ok(())
    }

    pub fn equipment_values(self) -> EquipmentValues {
        EquipmentValues {
            thrust: i64::from(self.thrust_level) * SOUL_LINK_EQUIPMENT_VALUE_PER_LEVEL,
            slash: i64::from(self.slash_level) * SOUL_LINK_EQUIPMENT_VALUE_PER_LEVEL,
            magic_attack: i64::from(self.magic_attack_level) * SOUL_LINK_EQUIPMENT_VALUE_PER_LEVEL,
            magic_defense: i64::from(self.magic_defense_level)
                * SOUL_LINK_EQUIPMENT_VALUE_PER_LEVEL,
            ..Default::default()
        }
    }

    pub fn equipment_source(self) -> Option<EquipmentValueSource> {
        let values = self.equipment_values();
        (values != EquipmentValues::default()).then(|| EquipmentValueSource {
            source: "ソウルリンク".to_string(),
            values,
        })
    }

    pub fn critical_damage_rate(self) -> f64 {
        f64::from(self.critical_damage_level) * SOUL_LINK_CRITICAL_DAMAGE_RATE_PER_LEVEL
    }
    pub fn final_damage_rate(self) -> f64 {
        f64::from(self.final_damage_level) * SOUL_LINK_FINAL_DAMAGE_RATE_PER_LEVEL
    }
    pub fn damage_contributions(self) -> Vec<DamageContribution> {
        [
            (
                DamageCategory::CriticalDamageRate,
                self.critical_damage_rate(),
            ),
            (DamageCategory::FinalDamageRate, self.final_damage_rate()),
        ]
        .into_iter()
        .filter(|(_, value)| *value != 0.0)
        .map(|(category, value)| DamageContribution {
            source: "ソウルリンク".to_string(),
            category,
            value,
        })
        .collect()
    }
    pub fn weapon_added_damage_multiplier(self) -> f64 {
        1.0 + f64::from(self.weapon_enhance_level) * SOUL_LINK_WEAPON_ENHANCE_RATE_PER_LEVEL
    }
    pub fn weapon_added_damage(self, base: i64) -> i64 {
        // `f64` では 10,245×1.4 が 14,342.999… になり得る。
        // 10% 刻みは整数比で正確に表し、Rust の整数除算で 0 方向へ切り捨てる。
        base * (10 + i64::from(self.weapon_enhance_level)) / 10
    }
    pub fn armor_added_hp_rate(self) -> f64 {
        f64::from(self.armor_enhance_level) * SOUL_LINK_ARMOR_ENHANCE_HP_RATE_PER_LEVEL
    }
    /// 鎧強化の追加 HP にソウルリンク8 を掛けた値。武器(ソウルリンク7)と同じく、
    /// 5% 刻みを整数比で表して 0 方向へ切り捨てる(`f64` だと 1.15 倍で桁が落ちる)。
    pub fn armor_added_hp(self, base: i64) -> i64 {
        base * (20 + i64::from(self.armor_enhance_level)) / 20
    }
    pub fn preview(self) -> SoulLinkPreview {
        SoulLinkPreview {
            equipment_values: self.equipment_values(),
            critical_damage_rate: self.critical_damage_rate(),
            final_damage_rate: self.final_damage_rate(),
            weapon_added_damage_multiplier: self.weapon_added_damage_multiplier(),
            armor_added_hp_rate: self.armor_added_hp_rate(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SoulLinkPreview {
    pub equipment_values: EquipmentValues,
    pub critical_damage_rate: f64,
    pub final_damage_rate: f64,
    pub weapon_added_damage_multiplier: f64,
    pub armor_added_hp_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum SoulLinkError {
    #[error("ソウルリンクの{name}は Lv0..={max} の範囲で指定してください(指定値 Lv{level})")]
    LevelOutOfRange {
        name: &'static str,
        level: u8,
        max: u8,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maxedは1から8を最大にする() {
        let status = SoulLinkStatus::maxed();
        assert_eq!(status.equipment_values().thrust, 50);
        assert!((status.critical_damage_rate() - 0.30).abs() < f64::EPSILON);
        assert!((status.final_damage_rate() - 0.20).abs() < f64::EPSILON);
        assert_eq!(status.weapon_added_damage(100), 300);
        assert!((status.armor_added_hp_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(status.armor_added_hp(100), 200);
    }

    #[test]
    fn 鎧強化のソウルリンクは5パーセント刻みで切り捨てる() {
        let status = SoulLinkStatus {
            armor_enhance_level: 3,
            ..Default::default()
        };
        // 1.15 倍。f64 だと 1_984_400 × 1.15 が 2_282_059.99… になって 1 落ちる
        assert_eq!(status.armor_added_hp(1_984_400), 2_282_060);
        assert_eq!(SoulLinkStatus::default().armor_added_hp(1_984_400), 1_984_400);
    }

    #[test]
    fn gとlは別カテゴリへ寄与する() {
        let status = SoulLinkStatus {
            critical_damage_level: 10,
            final_damage_level: 3,
            ..Default::default()
        };
        let contributions = status.damage_contributions();
        assert_eq!(contributions.len(), 2);
        assert_eq!(
            contributions[0].category,
            DamageCategory::CriticalDamageRate
        );
        assert_eq!(contributions[0].value, 0.15);
        assert_eq!(contributions[1].category, DamageCategory::FinalDamageRate);
        assert_eq!(contributions[1].value, 0.12);
    }

    #[test]
    fn 武器強化はlv0_10_20で1_2_3倍になる() {
        for (level, expected) in [(0, 101), (10, 202), (20, 303)] {
            let status = SoulLinkStatus {
                weapon_enhance_level: level,
                ..Default::default()
            };
            assert_eq!(status.weapon_added_damage(101), expected);
        }
    }

    #[test]
    fn 武器強化は浮動小数の境界でも1不足しない() {
        let status = SoulLinkStatus {
            weapon_enhance_level: 4,
            ..Default::default()
        };
        assert_eq!(status.weapon_added_damage(10_245), 14_343);
    }

    #[test]
    fn 値域を検証する() {
        let status = SoulLinkStatus {
            final_damage_level: 6,
            ..Default::default()
        };
        assert!(matches!(
            status.validate(),
            Err(SoulLinkError::LevelOutOfRange { max: 5, .. })
        ));
    }

    #[test]
    fn 鎧強化は与ダメージカテゴリを作らない() {
        let status = SoulLinkStatus {
            armor_enhance_level: 20,
            ..Default::default()
        };
        assert!(status.damage_contributions().is_empty());
        assert_eq!(status.weapon_added_damage(100), 100);
    }
}
