// crates/domain の値域上限(get_stat_limits)。起動時に App.svelte から 1 回取得する。
// 取得完了までの一瞬は既存のフロントリテラルと同じ値をフォールバックとして使う。
// **crates/domain の定数を変えたらここも直す。**ずれていると、起動直後に触った値が
// Rust 側の検証に落ちる(実際に equipment_value_max で起きた)。
import { getStatLimits } from "./api/commands";
import type { StatLimits } from "./api/types";

const FALLBACK: StatLimits = {
  base_stat_max: 310,
  rune_level_max: 20,
  crown_base_max: 100,
  crown_selected_max: 300,
  crown_step: 10,
  monster_card_max: 70,
  sacred_relic_stage_max: 40,
  adjustment_add_min: -3000,
  adjustment_add_max: 3000,
  adjustment_pin_min: 1,
  adjustment_pin_max: 3000,
  equipment_value_max: 1000,
  strong_weapon_level_max: 6,
  enhance_level_max: 15,
  core_slot_count: 6,
  core_evolution_max: 4,
  core_enhancement_max: 4,
  equipment_element_value_max: 9,
  element_value_max: 255,
  awakening_stage_max: 5,
  eternal_level_max: 100,
  random_option_value_max: 100,
  protect_armor_level_max: 6,
  kai_protect_armor_level_max: 5,
  sharpness_vision_level_max: 10,
  augment_level_max: 5,
  unleash_level_max: 10,
  unleash_slots: 2,
  reinforce_level_max: 5,
  hyper_limit_level_max: 6,
  critical_rate_bonus_max: 100,
  architect_lab_stage_max: 10,
  architect_lab_per_stage: 3,
  ultimate_rune_bonus_max: 20,
  deadly_blow_bonus_max: 100,
  power_weapon_rate: 0.02,
  strong_weapon_rate_per_level: 0.03,
  coat_armor_physical_rate: 0.18,
  coat_armor_magic_rate: 0.12,
  protect_armor_physical_rates: [0.36, 0.45, 0.54, 0.63, 0.72, 0.81],
  protect_armor_magic_rates: [0.24, 0.30, 0.36, 0.42, 0.48, 0.54],
  kai_protect_armor_physical_rates: [0.09, 0.18, 0.27, 0.36, 0.45],
  kai_protect_armor_magic_rates: [0.06, 0.12, 0.18, 0.24, 0.30],
  sharpness_vision_rates: [0.05, 0.10, 0.15, 0.20, 0.25, 0.28, 0.31, 0.34, 0.37, 0.40],
  unleash_rates: [0.01, 0.02, 0.03, 0.04, 0.05, 0.08, 0.11, 0.14, 0.17, 0.20],
  pet_skill_tier_bonus: [
    { tier: "basic", bonus: 20 },
    { tier: "true_lv1", bonus: 30 },
    { tier: "true_lv2", bonus: 40 },
    { tier: "true_lv3", bonus: 50 },
    { tier: "true_lv4", bonus: 60 },
  ],
  sacred_relic_value_per_stage: 10,
  core_power_bonus_table: [
    [1, 2, 3, 4, 5],
    [6, 7, 8, 9, 10],
    [12, 14, 16, 18, 20],
    [23, 26, 29, 32, 35],
    [40, 50, 60, 70, 80],
  ],
  core_support_bonus_table: [
    [1, 2, 3, 4, 5],
    [6, 7, 8, 9, 10],
    [12, 14, 16, 18, 20],
    [23, 26, 29, 32, 35],
    [40, 45, 50, 55, 60],
  ],
  part_slot_rules: [
    { slot: "weapon", ability_slots: 3, random_option_slots: 3 },
    { slot: "armor", ability_slots: 2, random_option_slots: 2 },
    { slot: "helm", ability_slots: 1, random_option_slots: 2 },
    { slot: "shield", ability_slots: 1, random_option_slots: 2 },
    { slot: "shield_plus", ability_slots: 2, random_option_slots: 2 },
    { slot: "head", ability_slots: 1, random_option_slots: 2 },
    { slot: "body", ability_slots: 0, random_option_slots: 2 },
    { slot: "hand", ability_slots: 2, random_option_slots: 2 },
    { slot: "leg", ability_slots: 1, random_option_slots: 2 },
    { slot: "effect", ability_slots: 0, random_option_slots: null },
    { slot: "artifact", ability_slots: 0, random_option_slots: null },
    { slot: "relic_pendant", ability_slots: 1, random_option_slots: 2 },
    { slot: "relic_bracelet", ability_slots: 1, random_option_slots: 2 },
  ],
};

export const limits = $state<StatLimits>({ ...FALLBACK });

export async function loadStatLimits(): Promise<void> {
  const v = await getStatLimits();
  Object.assign(limits, v);
}
