// crates/domain の並び・ラベル・部位ルール・段階表(get_game_tables)。起動時に 1 回だけ取得する。
// **App をマウントする前に main.ts が取り切る**(labels.ts などがモジュール評価時に読むため)。
// フォールバック値は持たない — 古い表と Rust 側の定義がずれて事故る経路を作らない。
import { getGameTables } from "./api/commands";
import type { GameTables } from "./api/types";

/** 取得後は必ず埋まっている(`loadGameTables` を待たずに読む経路を作らないこと)。 */
export const tables = $state<GameTables>({} as GameTables);
export async function loadGameTables(): Promise<void> {
  const v = await getGameTables();
  Object.assign(tables, v);
}
