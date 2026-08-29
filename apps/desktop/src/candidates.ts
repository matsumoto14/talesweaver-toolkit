// 「次に変えるなら / もし〜だったら」の強化候補。
// 現行のキャラモデル(部位別装備 12 スロット + PW/SW)で実際に表現できる変更だけを挙げる。
// 効果は preview_damage(Rust 側)で再計算する。ここは候補の列挙(表示)のみ。
import type { CandidateCost, EquipmentItem, NewCharacter } from "./api/types";
import { clampToCaps, selectedEquipmentPartOrNeutral, sumValues } from "./equipment";
import { limits } from "./limits.svelte";
import { STATE } from "./ui/states";

export interface Candidate {
  id: string;
  label: string;
  /** 手間の目安タグ(表示のみ)。種別は domain の CandidateCost(crates/domain/src/candidate.rs) */
  cost: CandidateCost;
  apply: (p: NewCharacter) => void;
}

/** cost タグの表示文字列(日本語)。 */
export const COST_LABELS: Record<CandidateCost, string> = {
  quick_win: "すぐできる",
  enchant: "エンチャント",
  equipment_update: "装備更新",
};

/** cost タグ → [面, 枠, 文字]。状態の 6 系統をそのまま流用する(design-system §03) */
export const COST_COLORS: Record<CandidateCost, [string, string, string]> = {
  quick_win: [STATE.met.bg, STATE.met.bd, STATE.met.fg],
  enchant: [STATE.goal.bg, STATE.goal.bd, STATE.goal.fg],
  equipment_update: [STATE.short.bg, STATE.short.bd, STATE.short.fg],
};

export interface CandidateResult {
  candidate: Candidate;
  perHit: number;
  deltaPct: number;
}

/**
 * 候補を実際に適用してダメージを引き直し、いまの値との差分(%)付きで返す。
 * CalcPage の「もし〜だったら」と HomePage の「次に変えるなら」で同じ形をしていたブロック
 * (Promise.allSettled → apply → previewDamage → deltaPct 計算 → 並び替え)をここに集約する。
 * 1 候補の失敗(装備検証エラー等)で他候補まで消さない(独立レビュー指摘)。並びは perHit 降順。
 */
export async function tryCandidates(
  candidates: Candidate[],
  build: () => NewCharacter,
  run: (p: NewCharacter) => Promise<{ per_hit: { max: number } }>,
  base: number,
  filter?: (r: CandidateResult) => boolean,
): Promise<CandidateResult[]> {
  const settled = await Promise.allSettled(
    candidates.map(async (candidate) => {
      const p = build();
      candidate.apply(p);
      const r = await run(p);
      return {
        candidate,
        perHit: r.per_hit.max,
        deltaPct: base > 0 ? Math.round((r.per_hit.max / base - 1) * 100) : 0,
      };
    }),
  );
  const results = settled.flatMap((s) => (s.status === "fulfilled" ? [s.value] : []));
  return (filter ? results.filter(filter) : results).sort((a, b) => b.perHit - a.perHit);
}

export function candidatesFor(current: NewCharacter, catalog: EquipmentItem[]): Candidate[] {
  const out: Candidate[] = [];
  if (!current.common_skills.power_weapon) {
    out.push({
      id: "pw",
      label: "パワーウェポンを ON に",
      cost: "quick_win",
      apply: (p) => {
        p.common_skills.power_weapon = true;
      },
    });
  }
  if (current.common_skills.strong_weapon_level < limits.strong_weapon_level_max) {
    out.push({
      id: "sw",
      label: `ストロングウェポンを Lv${limits.strong_weapon_level_max} に`,
      cost: "quick_win",
      apply: (p) => {
        p.common_skills.strong_weapon_level = limits.strong_weapon_level_max;
        // Lv2 以降はオーグメントの Lv が要る(wiki Skill/共通)
        p.common_skills.augment_level = Math.max(
          p.common_skills.augment_level,
          limits.strong_weapon_level_max - 1,
        );
      },
    });
  }

  const currentWeapon = selectedEquipmentPartOrNeutral(current.equipment.parts.weapon);
  const currentArmor = selectedEquipmentPartOrNeutral(current.equipment.parts.armor);
  const weaponItem = currentWeapon.item_id
    ? (catalog.find((i) => i.id === currentWeapon.item_id) ?? null)
    : null;
  const armorItem = currentArmor.item_id
    ? (catalog.find((i) => i.id === currentArmor.item_id) ?? null)
    : null;

  // カタログ item のときのみ(カスタム・未装備は候補から除外)
  if (weaponItem && armorItem) {
    out.push({
      id: "enchant-max",
      label: "武器と鎧のエンチャントを上限まで",
      cost: "enchant",
      apply: (p) => {
        const weapon = selectedEquipmentPartOrNeutral(p.equipment.parts.weapon);
        const armor = selectedEquipmentPartOrNeutral(p.equipment.parts.armor);
        weapon.enchant = { ...weaponItem.enchant_caps };
        armor.enchant = { ...armorItem.enchant_caps };
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
        cost: "equipment_update",
        apply: (p) => {
          const weapon = selectedEquipmentPartOrNeutral(p.equipment.parts.weapon);
          weapon.item_id = upgrade.id;
          weapon.custom_name = null;
          weapon.base = { ...upgrade.values_max };
          // 新アイテムで追加できる量まで clamp(SourcePane の pickCatalogItem と同じ扱い)。
          weapon.enchant = clampToCaps(
            weapon.enchant,
            upgrade.enchant_caps,
          );
        },
      });
    }
  }

  return out;
}
