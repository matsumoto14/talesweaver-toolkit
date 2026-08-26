// バフ選択の共通ロジック。バフカタログは消費アイテム・イベントの常用バフ専用で、
// キャラのパッシブ・自己バフ・味方バフは characterSkills(api/types の CharacterSkillDef)。
// 旧 CharacterSettings.svelte のヘルパーを純関数として切り出したもの。
import type { BuffChoice, BuffDefinition, BuffTarget, BuffValue, StatLayer } from "./api/types";
import { STAT_KINDS } from "./labels";

export const isChoiceValue = (v: BuffValue): v is { choice: number[] } =>
  typeof v === "object" && v !== null && "choice" in v;

export const userInputRange = (v: BuffValue): { min: number; max: number } | null =>
  typeof v === "object" && v !== null && "user_input" in v ? v.user_input : null;

export const isFixedValue = (v: BuffValue): v is { fixed: number } =>
  typeof v === "object" && v !== null && "fixed" in v;

/** 記録するだけ(計算に入らない)のバフか */
export const isRecordOnly = (v: BuffValue): boolean => v === "record_only";

export const isUserSelectedTarget = (t: BuffTarget): boolean => t === "user_selected";

export const isPercentLayer = (layer: StatLayer): boolean =>
  layer === "percent_of_base" || layer === "multiplier_b";

/** `excludingBuffId` 以外の選択が占有している排他枠の集合 */
export function usedExclusiveSlots(
  choices: BuffChoice[],
  catalog: BuffDefinition[],
  excludingBuffId: string,
): Set<string> {
  const slots = new Set<string>();
  for (const c of choices) {
    if (c.buff_id === excludingBuffId) continue;
    const d = catalog.find((x) => x.id === c.buff_id);
    if (d) for (const s of d.exclusive_slots) slots.add(s);
  }
  return slots;
}

/** 他の選択と排他枠が衝突して選べない状態か */
export function isBlocked(
  choices: BuffChoice[],
  catalog: BuffDefinition[],
  def: BuffDefinition,
): boolean {
  if (def.exclusive_slots.length === 0) return false;
  const used = usedExclusiveSlots(choices, catalog, def.id);
  return def.exclusive_slots.some((s) => used.has(s));
}

/** バフを ON にしたときの初期選択(「初期値は実用値」の原則) */
export function defaultChoice(def: BuffDefinition): BuffChoice {
  const choice: BuffChoice = { buff_id: def.id, stat: null, choice_index: null, value: null };
  if (isUserSelectedTarget(def.target)) choice.stat = STAT_KINDS[0];
  if (isChoiceValue(def.value)) choice.choice_index = 0;
  if (userInputRange(def.value)) choice.value = def.default_value ?? 0;
  return choice;
}

/** バフの ON/OFF を反映した新しい選択配列を返す(元の配列は変更しない) */
export function toggleBuff(choices: BuffChoice[], def: BuffDefinition, checked: boolean): BuffChoice[] {
  if (checked) return [...choices, defaultChoice(def)];
  return choices.filter((c) => c.buff_id !== def.id);
}
