//! 伸びしろの手(`GrowthAction`)を素材(`StatSources` / `Equipment` / `BuffSelection`)に
//! 当てる。「対人タブでその手を試したら」を再現するための書き込み専用の関数群
//! (対人の伸びしろ機能。決定は docs/adr/007-input-ux.md と 002-damage-formula-sources.md)。
//!
//! `EffectiveStats` などの計算済み値ではなく、計算前の素材を書き換える。呼び出し側
//! (`commands::preview_versus`)が書き換え後の素材で計算パイプラインを再度通す。
//!
//! variant ごとに「当てられない(部位が無い・スロットが無い・カタログに無い)」ときは
//! **静かに無視する**(`Result` にしない)。試しは検証用の一時操作であり、失敗して
//! 落とすより「動かない」のほうが伸びしろ一覧の表示を壊さない。

use crate::defense::GrowthAction;
use crate::equipment::{Equipment, EquipmentAbilityDef, EquipmentStatKind, EquipmentValues, PartSlot};
use crate::equipment_class::WeaponSystem;
use crate::random_option::RandomOptionSlot;
use crate::siena::{SienaSlot, SienaValueKind, SIENA_STAGE_MAX};
use crate::stat_sources::{
    BuffCatalog, BuffSelection, StatFixedSource, StatSources, MONSTER_CARD_VALUE_MAX,
    RUNE_LEVEL_MAX, SACRED_RELIC_STAGE_MAX,
};
use crate::stats::StatKind;

/// `apply_growth_action` が当てるのに要るカタログ一式(呼び出し側の攻撃側 / 防御側ごとに解決する)。
pub struct GrowthApplyContext<'a> {
    /// `Buff` / `StatBuff` の解決に要る(`BuffDefinition::default_choice`)
    pub buff_catalog: &'a BuffCatalog,
    /// `AbilityAttach` の解決に要る(空き枠に付けるアビリティの `category` を引く)
    pub abilities: &'a [EquipmentAbilityDef],
    /// `Enchant` の上限(呼び出し側が `resolve_enchant_caps` で解決した部位ごとの実測上限)
    pub enchant_caps: &'a [(PartSlot, EquipmentValues)],
    /// `AbilityAttach` が武器のカテゴリー枠へ入れるときの系統適合判定に要る
    pub weapon_system: Option<WeaponSystem>,
}

/// `EquipmentStatKind::Accuracy` / `Evasion` を対応するシエナの能力値種類へ移す
/// (`GrowthAction::Enchant` / `GrowthAction::Siena` はこの 2 種にしか出ない)。
fn siena_value_kind(stat: EquipmentStatKind) -> Option<SienaValueKind> {
    match stat {
        EquipmentStatKind::Accuracy => Some(SienaValueKind::Accuracy),
        EquipmentStatKind::Evasion => Some(SienaValueKind::Evasion),
        _ => None,
    }
}

/// クラウンの該当ステ実値を上限まで書く(`Crown` はステごとの named field で、setter が無い)。
fn set_crown_to_max(sources: &mut StatSources, stat: StatKind) {
    let max = sources.crown.max_value(stat);
    match stat {
        StatKind::Stab => sources.crown.stab = max,
        StatKind::Hack => sources.crown.hack = max,
        StatKind::Int => sources.crown.int = max,
        StatKind::Def => sources.crown.def = max,
        StatKind::Mr => sources.crown.mr = max,
        StatKind::Dex => sources.crown.dex = max,
        StatKind::Agi => sources.crown.agi = max,
    }
}

/// 1 つの伸びしろの手を素材に当てる(§モジュール doc)。
/// 排他枠を選択中のバフと取り合うバフか。列挙時点では通っていても、先に当てた別の手が枠を
/// 塞ぐことがある(同じ区分の手を 2 つ ON にしたとき)。当てるとステ計算が排他衝突で
/// エラーになり、方向ごと結果が消えるので、当てずに無視する
fn buff_blocked(buffs: &BuffSelection, ctx: &GrowthApplyContext, buff_id: &str) -> bool {
    crate::stat_sources::blocked_buffs(buffs, ctx.buff_catalog)
        .iter()
        .any(|b| b.buff_id == buff_id)
}

pub fn apply_growth_action(
    sources: &mut StatSources,
    equipment: &mut Equipment,
    buffs: &mut BuffSelection,
    action: &GrowthAction,
    ctx: &GrowthApplyContext,
) {
    match action {
        GrowthAction::Buff { buff_id, .. } => {
            if buffs.choices.iter().any(|c| &c.buff_id == buff_id) || buff_blocked(buffs, ctx, buff_id) {
                return;
            }
            if let Some(def) = ctx.buff_catalog.iter().find(|d| d.id == buff_id.as_str()) {
                buffs.choices.push(def.default_choice(None));
            }
        }
        GrowthAction::StatBuff { buff_id, stat, .. } => {
            if buffs.choices.iter().any(|c| &c.buff_id == buff_id) || buff_blocked(buffs, ctx, buff_id) {
                return;
            }
            if let Some(def) = ctx.buff_catalog.iter().find(|d| d.id == buff_id.as_str()) {
                buffs.choices.push(def.default_choice(Some(*stat)));
            }
        }
        GrowthAction::AbilityAttach {
            slot, ability_id, ..
        } => {
            let Some(def) = ctx.abilities.iter().find(|d| d.id == ability_id.as_str()) else {
                return;
            };
            let Some(part) = equipment.parts.get_mut(*slot).selected_mut() else {
                return;
            };
            if part.abilities.iter().any(|id| id == ability_id) {
                return;
            }
            if *slot == PartSlot::Weapon {
                part.set_ability_for_category(
                    ctx.abilities,
                    *slot,
                    def.category,
                    Some(ability_id.as_str()),
                    ctx.weapon_system,
                );
            } else {
                part.abilities.push(ability_id.clone());
            }
        }
        GrowthAction::AbilityReplace {
            slot,
            from_ability_id,
            ability_id,
            ..
        } => {
            let Some(part) = equipment.parts.get_mut(*slot).selected_mut() else {
                return;
            };
            if !part.abilities.iter().any(|id| id == from_ability_id) {
                return;
            }
            part.abilities.retain(|id| id != from_ability_id);
            part.abilities.push(ability_id.clone());
        }
        GrowthAction::RandomOptionAttach {
            slot,
            option_id,
            rank,
            ..
        } => {
            let Some(part) = equipment.parts.get_mut(*slot).selected_mut() else {
                return;
            };
            part.random_options.push(RandomOptionSlot {
                option_id: option_id.clone(),
                rank: *rank,
                value: None,
            });
        }
        GrowthAction::RandomOptionRankUp {
            slot,
            option_id,
            rank,
            ..
        } => {
            let Some(part) = equipment.parts.get_mut(*slot).selected_mut() else {
                return;
            };
            let Some(existing) = part
                .random_options
                .iter_mut()
                .find(|o| &o.option_id == option_id)
            else {
                return;
            };
            existing.rank = *rank;
            // 実測上書きが残ると新しいランクの `default_value` とずれる
            existing.value = None;
        }
        GrowthAction::StatFixed { stat, source } => match source {
            StatFixedSource::PetSkill { target } => sources.pet_skills.set(*stat, Some(*target)),
            StatFixedSource::Rune => sources.rune_levels.set(*stat, RUNE_LEVEL_MAX),
            StatFixedSource::MonsterCard => {
                sources.monster_cards.set(*stat, MONSTER_CARD_VALUE_MAX)
            }
            StatFixedSource::SacredRelic => {
                sources.sacred_relic.set(*stat, SACRED_RELIC_STAGE_MAX)
            }
            StatFixedSource::Crown => set_crown_to_max(sources, *stat),
        },
        GrowthAction::Enchant { slot, stat } => {
            let Some(&(_, cap)) = ctx.enchant_caps.iter().find(|(s, _)| s == slot) else {
                return;
            };
            let Some(part) = equipment.parts.get_mut(*slot).selected_mut() else {
                return;
            };
            match stat {
                EquipmentStatKind::Accuracy => part.enchant.accuracy = cap.accuracy,
                EquipmentStatKind::Evasion => part.enchant.evasion = cap.evasion,
                _ => {}
            }
        }
        GrowthAction::Siena { stat } => {
            let Some(kind) = siena_value_kind(*stat) else {
                return;
            };
            let (_, max) = kind.range();
            for slot in PartSlot::ALL {
                // 武器 / 盾のオーラは強化能力値の一覧で、命中率・回避率は出ない(`siena_room` と同じ規則)。
                // ここを見ないと武器に命中スロットを積んでしまい、ゲーム内で実現できない値になる
                if !kind.allowed_on(slot) {
                    continue;
                }
                let Some(list) = equipment.siena.get_mut(slot) else {
                    continue;
                };
                let Some(entry) = list.selected_mut() else {
                    continue;
                };
                while entry.aura.slots.len() < SIENA_STAGE_MAX {
                    entry.aura.slots.push(SienaSlot { kind, value: max });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awakening::AwakeningCaps;
    use crate::character_skill::SkillEffect;
    use crate::common_skill::{CommonSkills, DefenseRates};
    use crate::defense::{
        accuracy_point, defense_profile, versus_accuracy, AccuracyBoost, AccuracyCorrection,
        AttackType, GrowthAction, GrowthGroupRooms, GrowthRoom, VersusAccuracy, VersusAttacker,
        VersusDefender,
    };
    use crate::equipment::EquipmentPart;
    use crate::random_option::{RandomOptionDef, RandomOptionEffect, RandomOptionRank, RandomOptionSlot, RandomOptionTier, RandomOptionTotals};
    use crate::siena::{RegisteredSienaAura, SienaAura, SienaAuraList};
    use crate::stat_sources::{
        stat_buff_rooms, BuffDefinition, BuffOrigin, BuffPurpose, BuffSelection, BuffTarget,
        BuffValue, Crown, PetSkillTier, StatCatalogs, StatLayer,
    };
    use crate::stats::{BaseStats, EffectiveStats, StatKind};

    fn no_caps() -> AwakeningCaps {
        AwakeningCaps {
            max_damage: i64::MAX,
            max_defense: i64::MAX,
            max_stat: i64::MAX,
        }
    }

    fn neutral_correction() -> AccuracyCorrection {
        AccuracyCorrection {
            bonus: None,
            penalty_primary: StatKind::Def,
            penalty_secondary: None,
            penalty_divisor: 1.0,
        }
    }

    /// 区分をまたいで手を平らに見る(defense.rs のテストと同じヘルパー)。
    fn rooms(groups: &[GrowthGroupRooms]) -> impl Iterator<Item = &GrowthRoom> {
        groups.iter().flat_map(|g| g.rooms.iter())
    }

    /// 防御側は中立値で固定し、攻撃側だけを変えて `versus_accuracy` を呼ぶ。
    fn compute(attacker: VersusAttacker) -> VersusAccuracy {
        let defender_stats = EffectiveStats::default();
        let defender_profile = defense_profile(
            &defender_stats,
            &EquipmentValues::default(),
            no_caps(),
            &RandomOptionTotals::default(),
            DefenseRates::NEUTRAL,
        );
        versus_accuracy(
            &attacker,
            &VersusDefender {
                stats: &defender_stats,
                profile: &defender_profile,
                equipment: &Equipment::default(),
                enchant_caps: &[],
                stat_cap: defender_stats.agi,
                evasion_random_option: 0,
                stat_sources: &StatSources::default(),
                abilities: &[],
                random_option_catalog: &[],
                weapon_system: None,
                stat_buff_rooms: &[],
                min_evasion_rate: None,
            },
            AttackType::Physical,
        )
    }

    fn ability_def(
        id: &'static str,
        name: &'static str,
        slot: PartSlot,
        category: u8,
        ladder: &str,
        accuracy: i64,
    ) -> EquipmentAbilityDef {
        EquipmentAbilityDef {
            id,
            name,
            family: crate::equipment::EquipmentAbilityFamily::Accuracy,
            category,
            slot,
            value_option: None,
            exclusive_group: "test",
            additional_slots: 0,
            additional_effects: "",
            additional_options: vec![],
            record_only: false,
            effect_summary: "",
            values: EquipmentValues {
                accuracy,
                ..Default::default()
            },
            damage_effects: &[],
            grade: None,
            ladder: ladder.to_string(),
            priority: 0,
        }
    }

    #[test]
    fn buffを当てると命中pがバフ分動く() {
        let buff_catalog = vec![BuffDefinition {
            id: "acc_buff",
            name: "命中バフ",
            purposes: &[BuffPurpose::Accuracy],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(0.0),
            exclusive_slots: vec![],
            source_url: "",
            note: "",
            default_value: None,
            damage_effects: &[SkillEffect::AccuracyPoint {
                value: 20,
                exclusive_with: &[],
            }],
        }];
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let buffs = BuffSelection::default();
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &Equipment::default(),
            enchant_caps: &[],
            stat_cap: stats.dex,
            equipment_accuracy: 0,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &buff_catalog,
            accuracy_buff_selection: &buffs,
            stat_sources: &StatSources::default(),
            abilities: &[],
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::Buff { .. }))
            .expect("命中バフの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &buff_catalog,
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut equipment = Equipment::default();
        let mut tried_buffs = buffs.clone();
        apply_growth_action(&mut sources, &mut equipment, &mut tried_buffs, &room.action, &ctx);

        let new_bonus = crate::stat_sources::buff_accuracy_point_total(
            &tried_buffs,
            &buff_catalog,
            AccuracyBoost::NONE,
        );
        let before = accuracy_point(&stats, &correction, 0, 0, 0, AccuracyBoost::NONE, false, 0);
        let after = accuracy_point(
            &stats,
            &correction,
            0,
            0,
            new_bonus,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn statbuffを当てると命中pがdex上昇分動く() {
        let buff_catalog = vec![BuffDefinition {
            id: "dex_buff",
            name: "DEXバフ",
            purposes: &[BuffPurpose::Stats],
            origin: BuffOrigin::Item,
            target: BuffTarget::UserSelected,
            layer: StatLayer::FinalFixed,
            value: BuffValue::Fixed(30.0),
            exclusive_slots: vec![],
            source_url: "",
            note: "",
            default_value: None,
            damage_effects: &[],
        }];
        let base = BaseStats {
            stab: 1,
            hack: 1,
            int: 1,
            def: 1,
            mr: 1,
            dex: 100,
            agi: 1,
        };
        let sources = StatSources::default();
        let buffs = BuffSelection::default();
        let equipment = Equipment::default();
        let common = CommonSkills::default();
        let catalogs = StatCatalogs {
            buffs: &buff_catalog,
            masteries: &[],
            character_skills: &[],
        };
        let stat_cap = i64::MAX;

        let dex_stat_buff_rooms = stat_buff_rooms(
            &base,
            &sources,
            &buffs,
            &equipment,
            &common,
            catalogs,
            StatKind::Dex,
            stat_cap,
        )
        .unwrap();
        let stats = crate::stat_sources::effective_stats_of(
            &base, &sources, &buffs, &equipment, &common, catalogs, stat_cap,
        )
        .unwrap();
        let correction = neutral_correction();

        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap,
            equipment_accuracy: 0,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &buffs,
            stat_sources: &sources,
            abilities: &[],
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &dex_stat_buff_rooms,
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::StatBuff { .. }))
            .expect("DEX バフの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &buff_catalog,
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut tried_sources = sources.clone();
        let mut tried_equipment = equipment.clone();
        let mut tried_buffs = buffs.clone();
        apply_growth_action(
            &mut tried_sources,
            &mut tried_equipment,
            &mut tried_buffs,
            &room.action,
            &ctx,
        );

        let after_stats = crate::stat_sources::effective_stats_of(
            &base,
            &tried_sources,
            &tried_buffs,
            &tried_equipment,
            &common,
            catalogs,
            stat_cap,
        )
        .unwrap();

        let before = accuracy_point(&stats, &correction, 0, 0, 0, AccuracyBoost::NONE, false, 0);
        let after = accuracy_point(
            &after_stats,
            &correction,
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn abilityattachを当てると命中pがアビリティ分動く() {
        let defs = vec![ability_def(
            "acc_ability",
            "命中アビリティ",
            PartSlot::Hand,
            1,
            "命中",
            15,
        )];
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart::default());
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let equipment_accuracy = equipment.base_totals(&defs, &[]).accuracy;
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: stats.dex,
            equipment_accuracy,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &StatSources::default(),
            abilities: &defs,
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::AbilityAttach { .. }))
            .expect("アビリティの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &defs,
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        let mut tried_equipment = equipment.clone();
        apply_growth_action(&mut sources, &mut tried_equipment, &mut buffs, &room.action, &ctx);

        let new_equipment_accuracy = tried_equipment.base_totals(&defs, &[]).accuracy;
        let before = accuracy_point(
            &stats,
            &correction,
            equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        let after = accuracy_point(
            &stats,
            &correction,
            new_equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn abilityreplaceを当てると命中pが上位アビリティ分動く() {
        let defs = vec![
            ability_def("low", "上級の命中", PartSlot::Hand, 1, "命中", 5),
            ability_def("high", "夜星の命中", PartSlot::Hand, 1, "命中", 15),
        ];
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart {
            abilities: vec!["low".to_string()],
            ..Default::default()
        });
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let equipment_accuracy = equipment.base_totals(&defs, &[]).accuracy;
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: stats.dex,
            equipment_accuracy,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &StatSources::default(),
            abilities: &defs,
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::AbilityReplace { .. }))
            .expect("差し替えの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &defs,
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        let mut tried_equipment = equipment.clone();
        apply_growth_action(&mut sources, &mut tried_equipment, &mut buffs, &room.action, &ctx);

        let new_equipment_accuracy = tried_equipment.base_totals(&defs, &[]).accuracy;
        let before = accuracy_point(
            &stats,
            &correction,
            equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        let after = accuracy_point(
            &stats,
            &correction,
            new_equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    const RANDOM_OPTION_TIERS: &[RandomOptionTier] = &[
        RandomOptionTier {
            rank: RandomOptionRank::Special,
            min: 10.0,
            max: 20.0,
        },
        RandomOptionTier {
            rank: RandomOptionRank::STrue,
            min: 21.0,
            max: 30.0,
        },
    ];

    fn random_option_def() -> RandomOptionDef {
        RandomOptionDef {
            id: "acc_op",
            name: "命中OP",
            short: "命中",
            slot: PartSlot::Hand,
            category: 5,
            effect: RandomOptionEffect::AccuracyPoint,
            tiers: RANDOM_OPTION_TIERS,
            note: "",
            common: false,
        }
    }

    #[test]
    fn randomoptionattachを当てると命中pがop分動く() {
        let defs = vec![random_option_def()];
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart::default());
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let accuracy_random_option = equipment.random_option_totals(&defs).accuracy_point;
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: stats.dex,
            equipment_accuracy: 0,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &StatSources::default(),
            abilities: &[],
            random_option_catalog: &defs,
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::RandomOptionAttach { .. }))
            .expect("ランダムOP の伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        let mut tried_equipment = equipment.clone();
        apply_growth_action(&mut sources, &mut tried_equipment, &mut buffs, &room.action, &ctx);

        let new_random_option = tried_equipment.random_option_totals(&defs).accuracy_point;
        let before = accuracy_point(
            &stats,
            &correction,
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            accuracy_random_option,
        );
        let after = accuracy_point(
            &stats,
            &correction,
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            new_random_option,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn randomoptionrankupを当てると命中pがs真分動く() {
        let defs = vec![random_option_def()];
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart {
            random_options: vec![RandomOptionSlot {
                option_id: "acc_op".to_string(),
                rank: RandomOptionRank::Special,
                value: None,
            }],
            ..Default::default()
        });
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let accuracy_random_option = equipment.random_option_totals(&defs).accuracy_point;
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: stats.dex,
            equipment_accuracy: 0,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &StatSources::default(),
            abilities: &[],
            random_option_catalog: &defs,
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::RandomOptionRankUp { .. }))
            .expect("ランク上げの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        let mut tried_equipment = equipment.clone();
        apply_growth_action(&mut sources, &mut tried_equipment, &mut buffs, &room.action, &ctx);

        let new_random_option = tried_equipment.random_option_totals(&defs).accuracy_point;
        let before = accuracy_point(
            &stats,
            &correction,
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            accuracy_random_option,
        );
        let after = accuracy_point(
            &stats,
            &correction,
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            new_random_option,
        );
        assert_eq!(after - before, room.gain);
    }

    /// `PetSkill` / `Crown` 共通: `StatFixed` を当てると DEX 上昇ぶんだけ命中Pが動く。
    /// `stat_fixed_rooms` は 5 源ぶんの行を返す(gain 降順)ので、`want_petskill` で狙う源を選ぶ。
    fn assert_stat_fixed_moves_accuracy_point(
        mut sources: StatSources,
        stats: EffectiveStats,
        want_petskill: bool,
    ) {
        let correction = neutral_correction();
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &Equipment::default(),
            enchant_caps: &[],
            stat_cap: i64::MAX,
            equipment_accuracy: 0,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &sources,
            abilities: &[],
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| match &r.action {
                GrowthAction::StatFixed {
                    stat: StatKind::Dex,
                    source: crate::stat_sources::StatFixedSource::PetSkill { .. },
                } => want_petskill,
                GrowthAction::StatFixed {
                    stat: StatKind::Dex,
                    source: crate::stat_sources::StatFixedSource::Crown,
                } => !want_petskill,
                _ => false,
            })
            .expect("DEX の固定上昇の伸びしろが出るはず")
            .clone();

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut equipment = Equipment::default();
        let mut buffs = BuffSelection::default();
        // DEX 換算の効き(ペット S スキルの `bonus()` / クラウンの実値そのもの)を、
        // 適用前後の `StatFixed` の生値から測る(`GrowthRoom::current` / `max` と同じ量)。
        let before_source_value = match &room.action {
            GrowthAction::StatFixed {
                source: crate::stat_sources::StatFixedSource::PetSkill { .. },
                ..
            } => sources.pet_skills.get(StatKind::Dex).map_or(0, |t| t.bonus()),
            GrowthAction::StatFixed {
                source: crate::stat_sources::StatFixedSource::Crown,
                ..
            } => i64::from(sources.crown.get(StatKind::Dex)),
            _ => unreachable!("この関数は PetSkill / Crown 専用"),
        };
        apply_growth_action(&mut sources, &mut equipment, &mut buffs, &room.action, &ctx);
        let after_source_value = match &room.action {
            GrowthAction::StatFixed {
                source: crate::stat_sources::StatFixedSource::PetSkill { .. },
                ..
            } => sources.pet_skills.get(StatKind::Dex).map_or(0, |t| t.bonus()),
            GrowthAction::StatFixed {
                source: crate::stat_sources::StatFixedSource::Crown,
                ..
            } => i64::from(sources.crown.get(StatKind::Dex)),
            _ => unreachable!("この関数は PetSkill / Crown 専用"),
        };
        let effective = after_source_value - before_source_value;

        let before = accuracy_point(&stats, &correction, 0, 0, 0, AccuracyBoost::NONE, false, 0);
        let after = accuracy_point(
            &EffectiveStats {
                dex: stats.dex + effective,
                ..stats
            },
            &correction,
            0,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn statfixed_petskillを当てると命中pがdex上昇分動く() {
        let sources = StatSources {
            pet_skills: crate::stat_sources::PetSkills {
                dex: Some(PetSkillTier::Basic),
                ..Default::default()
            },
            ..Default::default()
        };
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        assert_stat_fixed_moves_accuracy_point(sources, stats, true);
    }

    #[test]
    fn statfixed_crownを当てると命中pがdex上昇分動く() {
        let sources = StatSources {
            crown: Crown {
                dex: 100,
                selected_stat: Some(StatKind::Dex),
                ..Default::default()
            },
            ..Default::default()
        };
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        assert_stat_fixed_moves_accuracy_point(sources, stats, false);
    }

    #[test]
    fn enchantを当てると命中pが上限分動く() {
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart {
            enchant: EquipmentValues {
                accuracy: 5,
                ..Default::default()
            },
            ..Default::default()
        });
        let enchant_caps = [(
            PartSlot::Hand,
            EquipmentValues {
                accuracy: 20,
                ..Default::default()
            },
        )];
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let equipment_accuracy = equipment.enhanced_totals(None).accuracy;
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &enchant_caps,
            stat_cap: stats.dex,
            equipment_accuracy,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &StatSources::default(),
            abilities: &[],
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::Enchant { .. }))
            .expect("エンチャントの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &[],
            enchant_caps: &enchant_caps,
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        let mut tried_equipment = equipment.clone();
        apply_growth_action(&mut sources, &mut tried_equipment, &mut buffs, &room.action, &ctx);

        let new_equipment_accuracy = tried_equipment.enhanced_totals(None).accuracy;
        let before = accuracy_point(
            &stats,
            &correction,
            equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        let after = accuracy_point(
            &stats,
            &correction,
            new_equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn sienaを当てると命中pが上限分動く() {
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart::default());
        equipment.siena.hand = SienaAuraList {
            registered: vec![RegisteredSienaAura {
                id: 1,
                label: String::new(),
                aura: SienaAura {
                    slots: vec![SienaSlot {
                        kind: SienaValueKind::Accuracy,
                        value: 1,
                    }],
                    extras: vec![],
                },
            }],
            selected_id: Some(1),
        };
        let stats = EffectiveStats {
            dex: 100,
            ..Default::default()
        };
        let correction = neutral_correction();
        let equipment_accuracy = equipment.enhanced_totals(None).accuracy;
        let v = compute(VersusAttacker {
            learnable_accuracy_skill: None,
            stats: &stats,
            correction: &correction,
            equipment: &equipment,
            enchant_caps: &[],
            stat_cap: stats.dex,
            equipment_accuracy,
            skill_accuracy: 0,
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::NONE,
            accuracy_random_option: 0,
            accuracy_buff_catalog: &[],
            accuracy_buff_selection: &BuffSelection::default(),
            stat_sources: &StatSources::default(),
            abilities: &[],
            random_option_catalog: &[],
            weapon_system: None,
            stat_buff_rooms: &[],
            min_hit_rate: None,
        });
        let room = rooms(&v.accuracy_growth)
            .find(|r| matches!(r.action, GrowthAction::Siena { .. }))
            .expect("シエナの伸びしろが出るはず");

        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        let mut tried_equipment = equipment.clone();
        apply_growth_action(&mut sources, &mut tried_equipment, &mut buffs, &room.action, &ctx);

        let new_equipment_accuracy = tried_equipment.enhanced_totals(None).accuracy;
        let before = accuracy_point(
            &stats,
            &correction,
            equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        let after = accuracy_point(
            &stats,
            &correction,
            new_equipment_accuracy,
            0,
            0,
            AccuracyBoost::NONE,
            false,
            0,
        );
        assert_eq!(after - before, room.gain);
    }

    #[test]
    fn 排他枠を取り合うバフは先に当てた手が塞いでいたら当てない() {
        let mk = |id: &'static str, name: &'static str| BuffDefinition {
            id,
            name,
            purposes: &[BuffPurpose::Accuracy],
            origin: BuffOrigin::Item,
            target: BuffTarget::AllStats,
            layer: StatLayer::Fixed,
            value: BuffValue::Fixed(0.0),
            exclusive_slots: vec!["potion"],
            source_url: "",
            note: "",
            default_value: None,
            damage_effects: &[SkillEffect::AccuracyPoint {
                value: 20,
                exclusive_with: &[],
            }],
        };
        let buff_catalog = vec![mk("acc_a", "命中バフ A"), mk("acc_b", "命中バフ B")];
        let ctx = GrowthApplyContext {
            buff_catalog: &buff_catalog,
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut equipment = Equipment::default();
        let mut buffs = BuffSelection::default();
        let a = GrowthAction::Buff {
            buff_id: "acc_a".into(),
            name: "命中バフ A".into(),
        };
        let b = GrowthAction::Buff {
            buff_id: "acc_b".into(),
            name: "命中バフ B".into(),
        };
        apply_growth_action(&mut sources, &mut equipment, &mut buffs, &a, &ctx);
        apply_growth_action(&mut sources, &mut equipment, &mut buffs, &b, &ctx);
        let ids: Vec<&str> = buffs.choices.iter().map(|c| c.buff_id.as_str()).collect();
        assert_eq!(ids, vec!["acc_a"], "枠を塞がれた B は当てない(排他衝突でエラーにしない)");
    }

    #[test]
    fn sienaは命中スロットが出ない武器には積まない() {
        let mut equipment = Equipment::default();
        equipment.parts.hand = crate::equipment::EquipmentPartList::from(EquipmentPart::default());
        let aura_list = || SienaAuraList {
            registered: vec![RegisteredSienaAura {
                id: 1,
                label: String::new(),
                aura: SienaAura {
                    slots: vec![],
                    extras: vec![],
                },
            }],
            selected_id: Some(1),
        };
        equipment.siena.hand = aura_list();
        equipment.siena.weapon = aura_list();
        let ctx = GrowthApplyContext {
            buff_catalog: &[],
            abilities: &[],
            enchant_caps: &[],
            weapon_system: None,
        };
        let mut sources = StatSources::default();
        let mut buffs = BuffSelection::default();
        apply_growth_action(
            &mut sources,
            &mut equipment,
            &mut buffs,
            &GrowthAction::Siena {
                stat: EquipmentStatKind::Accuracy,
            },
            &ctx,
        );
        let slots_of = |slot: PartSlot| {
            equipment
                .siena
                .get(slot)
                .and_then(|l| l.selected())
                .map(|e| e.aura.slots.len())
                .unwrap_or(0)
        };
        assert_eq!(slots_of(PartSlot::Hand), SIENA_STAGE_MAX, "手には上限まで積む");
        assert_eq!(slots_of(PartSlot::Weapon), 0, "武器のオーラに命中スロットは無い");
    }
}
