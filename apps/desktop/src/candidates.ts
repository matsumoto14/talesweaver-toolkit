// 「次に変えるなら / もし〜だったら」の強化候補。
// 現行のキャラモデル(部位別装備 12 スロット + PW/SW)で実際に表現できる変更だけを挙げる。
// 効果は preview_damage(Rust 側)で再計算する。ここは候補の列挙(表示)のみ。
import type { EquipmentItem, NewCharacter } from "./api/types";
import { clampToCaps, midpointValues, sumValues } from "./equipment";
import { limits } from "./limits.svelte";

export interface Candidate {
  id: string;
  label: string;
  /** 手間の目安タグ(表示のみ) */
  cost: "すぐできる" | "エンチャント" | "装備更新";
  apply: (p: NewCharacter) => void;
}

/** cost タグ → [背景, 枠, 文字] */
/** [面, 枠, 文字] の CSS 変数参照(inline style で使う)。色の実値は app.css のトークンが持つ */
export const COST_COLORS: Record<Candidate["cost"], [string, string, string]> = {
  すぐできる: ["var(--good-bg)", "var(--good-border)", "var(--good)"],
  エンチャント: ["var(--bg-active)", "var(--accent)", "var(--accent-hover)"],
  装備更新: ["var(--danger-bg)", "var(--danger-border)", "var(--danger)"],
};

export function candidatesFor(current: NewCharacter, catalog: EquipmentItem[]): Candidate[] {
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

  const weaponItem = current.equipment.parts.weapon.item_id
    ? (catalog.find((i) => i.id === current.equipment.parts.weapon.item_id) ?? null)
    : null;
  const armorItem = current.equipment.parts.armor.item_id
    ? (catalog.find((i) => i.id === current.equipment.parts.armor.item_id) ?? null)
    : null;

  // カタログ item のときのみ(カスタム・未装備は候補から除外)
  if (weaponItem && armorItem) {
    out.push({
      id: "enchant-max",
      label: "武器と鎧のエンチャントを上限まで",
      cost: "エンチャント",
      apply: (p) => {
        p.equipment.parts.weapon.enchant = { ...weaponItem.enchant_caps };
        p.equipment.parts.armor.enchant = { ...armorItem.enchant_caps };
      },
    });
  }

  // 現武器の weapon_class と同じ slot=weapon のカタログ上位品(基本値レンジ上限の合計が大きいもの)
  if (weaponItem?.weapon_class) {
    const upgrade = catalog
      .filter(
        (i) =>
          i.slot === "weapon" &&
          i.weapon_class === weaponItem.weapon_class &&
          i.id !== weaponItem.id &&
          sumValues(i.values_max) > sumValues(weaponItem.values_max),
      )
      .sort((a, b) => sumValues(b.values_max) - sumValues(a.values_max))[0];
    if (upgrade) {
      out.push({
        id: "weapon-upgrade",
        label: `武器を${upgrade.name}に更新`,
        cost: "装備更新",
        apply: (p) => {
          const weapon = p.equipment.parts.weapon;
          weapon.item_id = upgrade.id;
          weapon.custom_name = null;
          weapon.base = midpointValues(upgrade.values_min, upgrade.values_max);
          // 新アイテムのエンチャント上限まで clamp(SourcePane の pickCatalogItem と同じ扱い。
          // 例: アクィルス(魔攻上限280)→アビス(同100)への更新で検証エラーにならないように)
          weapon.enchant = clampToCaps(weapon.enchant, upgrade.enchant_caps);
        },
      });
    }
  }

  return out;
}
