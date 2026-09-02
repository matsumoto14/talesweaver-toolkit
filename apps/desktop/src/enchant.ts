// エンチャントの「現在 / 上限」を部位横断で並べる共通ロジック(CalcPage / HomePage 共用)。
// ルール(スキル依存種別 → 見るステ 2 本、部位・アイテムごとの上限)はすべてドメイン(Rust)から
// 引く。ここは「どの部位を並べるか」「表示ラベル」など純粋な表示都合だけを持つ
// (ADR 001: ゲーム由来の係数・上限をフロントに置かない / 値域上限にフォールバックを持たない)。
import type { Equipment, EquipmentItem, EquipmentPart, PartSlot, SkillDependency } from "./api/types";
import { limits } from "./limits.svelte";

export type EnchantDepKey = "thrust" | "slash" | "magic_attack" | "magic_defense";

/** スキルの依存種別 → エンチャントで見るステ 2 本。ドメイン(装備攻撃力係数)から起動時に
 *  引いた静的テーブル(StatLimits.enchant_dependency_keys)を読むだけ — ルール表を写経しない。 */
export function enchantDepKeysFor(dependency: SkillDependency): EnchantDepKey[] {
  const row = limits.enchant_dependency_keys?.find((r) => r.dependency === dependency);
  return row ? [...row.keys] : [];
}

/** エンチャント枠を持ちうる部位(レリック・効果・AF・体は対象外。判定は PartSlot::allows_enchant)。 */
export const ENCHANT_SLOTS: PartSlot[] =
  limits.part_slot_rules.filter((r) => r.allows_enchant).map((r) => r.slot);
export const ENCHANT_SLOT_LABELS: Record<string, string> = {
  weapon: "武器", armor: "鎧", helm: "兜", shield: "盾", shield_plus: "カフス", head: "頭", hand: "手", leg: "足",
};

export const isUnequipped = (part: EquipmentPart | null): boolean =>
  !part || (part.item_id === null && part.custom_name === null);

/** この部位・ステのエンチャント上限。カタログ品が正、無ければパートの実測上限(カスタム名装備で
 *  ユーザーが入力した値)、どちらも無ければ上限を決められない = `null`(未収録)。
 *  crates/domain/src/equipment.rs の `EquipmentPart::resolve_enchant_caps` と同じ解決順
 *  (ADR 001「値域上限にフォールバックを持たない」— 共通上限で黙って埋めない)。 */
export function enchantCap(part: EquipmentPart, key: EnchantDepKey, catalog: EquipmentItem[]): number | null {
  const item = part.item_id ? catalog.find((i) => i.id === part.item_id) : null;
  if (item) return item.enchant_caps[key];
  return part.enchant_caps ? part.enchant_caps[key] : null;
}

export interface EnchantRow {
  slot: PartSlot;
  part: EquipmentPart;
  /** この行の依存ステの上限が 1 本も分からない(カタログ外でパートの実測上限も未入力)。
   *  落とさず「未収録」の行として出す(§00 05: なぜ出ないか分からない状態を作らない)。 */
  capUnknown: boolean;
}

/** 依存ステの枠を持つ装備済み部位を返す(枠 0 の部位・未装備は出さない)。上限が 1 本も
 *  分からない部位は落とさず `capUnknown: true` の行として返す — 呼び出し側は「未収録」表示 +
 *  上限入力への導線を出す(カタログ外装備の伸びしろが黙って消えないようにする)。 */
export function enchantRows(equipment: Equipment, keys: readonly EnchantDepKey[], catalog: EquipmentItem[]): EnchantRow[] {
  const rows: EnchantRow[] = [];
  for (const slot of ENCHANT_SLOTS) {
    const list = equipment.parts[slot];
    const part = list.registered.find((p) => p.id === list.selected_id) ?? null;
    if (!part || isUnequipped(part)) continue;
    const caps = keys.map((k) => enchantCap(part, k, catalog));
    const capUnknown = caps.every((c) => c === null);
    if (capUnknown || caps.some((c) => (c ?? 0) > 0)) rows.push({ slot, part, capUnknown });
  }
  return rows;
}

/** 試し変更として equipment を直接書き換える(呼び出し側は複製済みの payload を渡す)。 */
export function setEnchantValue(equipment: Equipment, slot: PartSlot, key: EnchantDepKey, value: number): void {
  const list = equipment.parts[slot];
  const part = list.registered.find((p) => p.id === list.selected_id);
  if (part) part.enchant[key] = value;
}
