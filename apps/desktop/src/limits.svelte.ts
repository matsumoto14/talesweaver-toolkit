// crates/domain の値域上限(get_stat_limits)。起動時に App.svelte から 1 回取得する。
// 取得完了までの一瞬は既存のフロントリテラルと同じ値をフォールバックとして使う。
import { getStatLimits } from "./api/commands";
import type { StatLimits } from "./api/types";

const FALLBACK: StatLimits = {
  base_stat_max: 310,
  rune_level_max: 20,
  crown_max: 300,
  monster_card_max: 70,
  sacred_relic_stage_max: 40,
  adjustment_add_min: -999,
  adjustment_add_max: 999,
  adjustment_pin_min: 1,
  adjustment_pin_max: 2400,
  equipment_value_max: 9999,
  strong_weapon_level_max: 6,
  enhance_level_max: 15,
  enhance_added_damage_max: 9999999,
  siena_stage_max: 10,
  siena_attack_rate_percent_max: 10,
  siena_defense_rate_percent_max: 10,
  siena_actual_delay_percent_max: 2,
  siena_critical_rate_percent_max: 10,
  siena_stat_bonus_max: 100,
  siena_all_stats_bonus_max: 30,
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
};

export const limits = $state<StatLimits>({ ...FALLBACK });

export async function loadStatLimits(): Promise<void> {
  const v = await getStatLimits();
  Object.assign(limits, v);
}
