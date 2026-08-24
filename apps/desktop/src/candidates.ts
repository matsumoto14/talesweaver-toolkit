// 「次に変えるなら / もし〜だったら」の強化候補。
// 現行のキャラモデル(装備 8 値 + PW/SW)で実際に表現できる変更だけを挙げる。
// 効果は preview_damage(Rust 側)で再計算する。ここは候補の列挙(表示)のみ。
import type { NewCharacter } from "./api/types";
import { limits } from "./limits.svelte";

export interface Candidate {
  id: string;
  label: string;
  /** 手間の目安タグ(表示のみ) */
  cost: "すぐできる" | "エンチャント" | "装備更新";
  apply: (p: NewCharacter) => void;
}

/** cost タグ → [背景, 枠, 文字] */
export const COST_COLORS: Record<Candidate["cost"], [string, string, string]> = {
  すぐできる: ["#DFF3E6", "#6FA98A", "#2E6B4C"],
  エンチャント: ["#DCEBFF", "#426DD6", "#2B4FA8"],
  装備更新: ["#F6E8E5", "#B08480", "#8C4A42"],
};

export function candidatesFor(current: NewCharacter): Candidate[] {
  const out: Candidate[] = [];
  if (!current.equipment.power_weapon) {
    out.push({
      id: "pw",
      label: "パワーウェポンを ON に",
      cost: "すぐできる",
      apply: (p) => {
        p.equipment.power_weapon = true;
      },
    });
  }
  if (current.equipment.strong_weapon_level < limits.strong_weapon_level_max) {
    out.push({
      id: "sw",
      label: `ストロングウェポンを Lv${limits.strong_weapon_level_max} に`,
      cost: "すぐできる",
      apply: (p) => {
        p.equipment.strong_weapon_level = limits.strong_weapon_level_max;
      },
    });
  }
  out.push({
    id: "enh",
    label: "強化能力値(突き・斬り)を +100",
    cost: "エンチャント",
    apply: (p) => {
      p.equipment.enhanced.thrust = Math.min(limits.equipment_value_max, p.equipment.enhanced.thrust + 100);
      p.equipment.enhanced.slash = Math.min(limits.equipment_value_max, p.equipment.enhanced.slash + 100);
    },
  });
  out.push({
    id: "base",
    label: "基本能力値(突き・斬り)を +100",
    cost: "装備更新",
    apply: (p) => {
      p.equipment.base.thrust = Math.min(limits.equipment_value_max, p.equipment.base.thrust + 100);
      p.equipment.base.slash = Math.min(limits.equipment_value_max, p.equipment.base.slash + 100);
    },
  });
  return out;
}
