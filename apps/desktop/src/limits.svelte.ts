// crates/domain の値域上限(get_stat_limits)。起動時に 1 回だけ取得する。
// **App をマウントする前に main.ts が取り切る**(labels.ts などがモジュール評価時に読むため)。
// フォールバック値は持たない — 古い値と Rust 側の定数がずれて事故る経路を作らない。
import { getStatLimits } from "./api/commands";
import type { StatLimits } from "./api/types";

/** 取得後は必ず埋まっている(`loadStatLimits` を待たずに読む経路を作らないこと)。 */
export const limits = $state<StatLimits>({} as StatLimits);
export async function loadStatLimits(): Promise<void> {
  const v = await getStatLimits();
  Object.assign(limits, v);
}
