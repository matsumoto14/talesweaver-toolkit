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
  enhance_added_damage_max: 9999999,
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
};

export const limits = $state<StatLimits>({ ...FALLBACK });

export async function loadStatLimits(): Promise<void> {
  const v = await getStatLimits();
  Object.assign(limits, v);
}
