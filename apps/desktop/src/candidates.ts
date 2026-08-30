// 「次に変えるなら / もし〜だったら」の強化候補の表示定数。
// 列挙・試算(旧 candidatesFor / tryCandidates)は Rust 側(crates/domain/src/candidate.rs +
// list_upgrade_candidates コマンド)に移した。ここは cost タグの表示だけ残す。
import type { CandidateCost } from "./api/types";
import { STATE } from "./ui/states";

/** cost タグの表示文字列(日本語)。 */
export const COST_LABELS: Record<CandidateCost, string> = {
  quick_win: "すぐできる",
  enchant: "エンチャント",
  equipment_update: "装備更新",
  enhance: "強化",
  aura: "オーラ強化",
};

/** cost タグ → [面, 枠, 文字]。状態の 6 系統をそのまま流用する(design-system §03) */
export const COST_COLORS: Record<CandidateCost, [string, string, string]> = {
  quick_win: [STATE.met.bg, STATE.met.bd, STATE.met.fg],
  enchant: [STATE.goal.bg, STATE.goal.bd, STATE.goal.fg],
  equipment_update: [STATE.short.bg, STATE.short.bd, STATE.short.fg],
  enhance: [STATE.edge.bg, STATE.edge.bd, STATE.edge.fg],
  aura: [STATE.temp.bg, STATE.temp.bd, STATE.temp.fg],
};
