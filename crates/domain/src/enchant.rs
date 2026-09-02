//! エンチャント(wiki: 装備システム/エンチャント)の完了プラン。
//!
//! 上限まで埋めるのに要る巻物の組み合わせ(+17 基本、端数 18/19 を避けつつ 1 回減らせる
//! 最小個数だけ +20)と、その案内を出す装備補正を決める。

use serde::{Deserialize, Serialize};

use crate::equipment::{EquipmentStatKind, EquipmentValues, PartSlot};
use crate::equipment_class::{WeaponClass, WeaponSystem};
use crate::skill::SkillDependency;

/// エンチャント巻物の刻み(wiki: 装備システム/エンチャント)。
const ENCHANT_SCROLL_SMALL: i64 = 17;
const ENCHANT_SCROLL_LARGE: i64 = 20;

/// 上限まで埋めるプラン。`remainder` は最後の 1 回で入れる端数(0 ならぴったり)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnchantPlan {
    /// 上限までの残り
    pub remaining: i64,
    pub twenty_count: i64,
    pub seventeen_count: i64,
    pub remainder: i64,
    /// 巻物を使う回数
    pub count: i64,
}

/// 上限まであと `remaining` の枠を埋めるプラン。
///
/// +17 だけで埋めるより 1 回少ない組み合わせを、+20 の個数を外側から増やして探す。
/// 最初に見つかったものが「+20 を最小限にした案」で、同数なら +17 が多い案を採る。
/// 端数は 1〜16(18・19 は +17 を 1 本足したのと変わらないので避ける)。
pub fn enchant_plan(remaining: i64) -> EnchantPlan {
    let remaining = remaining.max(0);
    if remaining == 0 {
        return EnchantPlan {
            remaining,
            twenty_count: 0,
            seventeen_count: 0,
            remainder: 0,
            count: 0,
        };
    }
    let base_count = (remaining + ENCHANT_SCROLL_SMALL - 1) / ENCHANT_SCROLL_SMALL;
    let reduced_count = base_count - 1;
    for twenty_count in 0..=reduced_count {
        for seventeen_count in (0..=reduced_count - twenty_count).rev() {
            let remainder_slots = reduced_count - twenty_count - seventeen_count;
            if remainder_slots > 1 {
                continue;
            }
            let remainder =
                remaining - twenty_count * ENCHANT_SCROLL_LARGE - seventeen_count * ENCHANT_SCROLL_SMALL;
            if (remainder_slots == 0 && remainder == 0)
                || (remainder_slots == 1 && (1..=ENCHANT_SCROLL_SMALL - 1).contains(&remainder))
            {
                return EnchantPlan {
                    remaining,
                    twenty_count,
                    seventeen_count,
                    remainder,
                    count: reduced_count,
                };
            }
        }
    }
    let seventeen_count = remaining / ENCHANT_SCROLL_SMALL;
    let remainder = remaining % ENCHANT_SCROLL_SMALL;
    EnchantPlan {
        remaining,
        twenty_count: 0,
        seventeen_count,
        remainder,
        count: seventeen_count + i64::from(remainder > 0),
    }
}

/// プランを出す装備品の分類(gamedata の `EquipmentItem` から呼び出し側が詰める)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnchantPlanItem {
    pub slot: PartSlot,
    pub weapon_class: Option<WeaponClass>,
    pub weapon_system: Option<WeaponSystem>,
    pub recommended_dependency: Option<SkillDependency>,
    pub enchant_caps: EquipmentValues,
}

/// この装備でエンチャントを案内する補正。
///
/// 刀は突き・斬りの両方(片方だけ伸ばしても攻撃力にならない)。武器以外は選んだ装備の
/// 専用系統(AF 等)を優先し、汎用品は主軸スキルの係数へ合わせる。攻撃補正を持たない
/// 鎧・盾はその部位で実際に伸ばせる耐久の主要補正へ、効果は最大枠の系統へ案内する。
pub fn enchant_plan_stats(
    item: &EnchantPlanItem,
    main_dependency: Option<SkillDependency>,
) -> Vec<EquipmentStatKind> {
    if !item.slot.allows_enchant_plan() {
        return Vec::new();
    }
    let supported = |kinds: &[EquipmentStatKind]| -> Vec<EquipmentStatKind> {
        kinds
            .iter()
            .copied()
            .filter(|&kind| item.enchant_caps.get(kind) > 0)
            .collect()
    };
    if item.weapon_class == Some(WeaponClass::Katana) {
        return vec![EquipmentStatKind::Thrust, EquipmentStatKind::Slash];
    }
    let dependency = item
        .weapon_system
        .map(WeaponSystem::dependency)
        .or(item.recommended_dependency)
        .or(main_dependency);
    if let Some(dependency) = dependency {
        let stats = supported(dependency.enchant_stats());
        if !stats.is_empty() {
            return stats;
        }
    }
    if matches!(item.slot, PartSlot::Armor | PartSlot::Shield) {
        return supported(&[
            EquipmentStatKind::PhysicalDefense,
            EquipmentStatKind::MagicDefense,
            EquipmentStatKind::Evasion,
        ]);
    }
    // 主軸スキル未選択でも案内自体を消さない。効果は最大枠の系統、その他の汎用品は
    // エンチャントできる S/H/I/M を候補にする。
    let primary = supported(&PRIMARY_ENCHANT_STATS);
    if item.slot == PartSlot::Effect && !primary.is_empty() {
        let max_cap = primary
            .iter()
            .map(|&kind| item.enchant_caps.get(kind))
            .max()
            .unwrap_or(0);
        return primary
            .into_iter()
            .filter(|&kind| item.enchant_caps.get(kind) == max_cap)
            .collect();
    }
    primary
}

/// 装備で日常的にエンチャントする 4 補正。ゲーム内の呼び方どおり S/H/I/M の順。
pub const PRIMARY_ENCHANT_STATS: [EquipmentStatKind; 4] = [
    EquipmentStatKind::Thrust,
    EquipmentStatKind::Slash,
    EquipmentStatKind::MagicAttack,
    EquipmentStatKind::MagicDefense,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(remaining: i64) -> (i64, i64, i64, i64) {
        let p = enchant_plan(remaining);
        (p.twenty_count, p.seventeen_count, p.remainder, p.count)
    }

    #[test]
    fn 上限まで0なら巻物は要らない() {
        assert_eq!(plan(0), (0, 0, 0, 0));
        assert_eq!(plan(-5), (0, 0, 0, 0));
    }

    #[test]
    fn ちょうど17の倍数ならプラス20は使わない() {
        assert_eq!(plan(17), (0, 1, 0, 1));
        assert_eq!(plan(34), (0, 2, 0, 2));
        assert_eq!(plan(51), (0, 3, 0, 3));
    }

    #[test]
    fn 端数は1から16に収める() {
        assert_eq!(plan(10), (0, 0, 10, 1));
        assert_eq!(plan(35), (1, 0, 15, 2));
        assert_eq!(plan(52), (1, 1, 15, 3));
    }

    #[test]
    fn 回数を1回減らせるときだけプラス20を使う() {
        // 37 は 17×3 ではなく 20+17 の 2 回
        assert_eq!(plan(37), (1, 1, 0, 2));
        // 40 は 20×2
        assert_eq!(plan(40), (2, 0, 0, 2));
        // 36 は 20+16
        assert_eq!(plan(36), (1, 0, 16, 2));
    }

    #[test]
    fn 減らせない残りは17を並べる() {
        // 18 は +17 1 本では足りず、+20 でも 1 回にできない(端数 18 は不可)
        assert_eq!(plan(18), (0, 1, 1, 2));
    }

    #[test]
    fn 刀は突きと斬りの両方を案内する() {
        let item = EnchantPlanItem {
            slot: PartSlot::Weapon,
            weapon_class: Some(WeaponClass::Katana),
            weapon_system: Some(WeaponSystem::Hack),
            recommended_dependency: None,
            enchant_caps: EquipmentValues::default(),
        };
        assert_eq!(
            enchant_plan_stats(&item, None),
            vec![EquipmentStatKind::Thrust, EquipmentStatKind::Slash]
        );
    }

    #[test]
    fn 攻撃補正を持たない鎧は耐久の補正を案内する() {
        let item = EnchantPlanItem {
            slot: PartSlot::Armor,
            weapon_class: None,
            weapon_system: None,
            recommended_dependency: None,
            enchant_caps: EquipmentValues {
                physical_defense: 60,
                evasion: 20,
                ..Default::default()
            },
        };
        assert_eq!(
            enchant_plan_stats(&item, Some(SkillDependency::Hack)),
            vec![EquipmentStatKind::PhysicalDefense, EquipmentStatKind::Evasion]
        );
    }

    #[test]
    fn 効果は最大枠の系統だけを案内する() {
        let item = EnchantPlanItem {
            slot: PartSlot::Effect,
            weapon_class: None,
            weapon_system: None,
            recommended_dependency: None,
            enchant_caps: EquipmentValues {
                thrust: 40,
                slash: 40,
                magic_attack: 20,
                ..Default::default()
            },
        };
        assert_eq!(
            enchant_plan_stats(&item, None),
            vec![EquipmentStatKind::Thrust, EquipmentStatKind::Slash]
        );
    }

    #[test]
    fn レリックは通常エンチャントを持たないので案内しない() {
        let item = EnchantPlanItem {
            slot: PartSlot::RelicPendant,
            weapon_class: None,
            weapon_system: None,
            recommended_dependency: None,
            enchant_caps: EquipmentValues::default(),
        };
        assert!(enchant_plan_stats(&item, Some(SkillDependency::Stab)).is_empty());
    }
}
