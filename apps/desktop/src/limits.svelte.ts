// crates/domain の値域上限(get_stat_limits)。起動時に App.svelte から 1 回取得する。
// 取得完了までの一瞬は既存のフロントリテラルと同じ値をフォールバックとして使う。
import { getStatLimits } from "./api/commands";
import type { StatLimits } from "./api/types";

const FALLBACK: StatLimits = {
  base_stat_max: 310,
  rune_level_max: 20,
  crown_max: 300,
  sacred_relic_stage_max: 40,
  adjustment_add_min: -999,
  adjustment_add_max: 999,
  adjustment_pin_min: 1,
  adjustment_pin_max: 2400,
  equipment_value_max: 9999,
  strong_weapon_level_max: 6,
};

export const limits = $state<StatLimits>({ ...FALLBACK });

export async function loadStatLimits(): Promise<void> {
  const v = await getStatLimits();
  Object.assign(limits, v);
}
