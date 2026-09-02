// バフ選択の共通ロジック。バフカタログは消費アイテム・イベントの常用バフ専用で、
// キャラのパッシブ・自己バフ・味方バフは characterSkills(api/types の CharacterSkillDef)。
// 旧 CharacterSettings.svelte のヘルパーを純関数として切り出したもの。
import type { BuffChoice, BuffDefinition, BuffPurpose, BuffTarget, BuffValue, StatKind, StatLayer } from "./api/types";
import { STAT_KINDS } from "./labels";

/** バフを分ける「目的」。バフタブの目的タブと計算タブのグループで同じ切り口を使う */
export const BUFF_PURPOSES: { id: BuffPurpose; label: string; description: string }[] = [
  { id: "stats", label: "ステータスを上げたい", description: "能力値が伸びる効果" },
  { id: "damage", label: "火力を上げたい", description: "攻撃ダメージ効果を持つバフ" },
  { id: "durability", label: "耐久を上げたい", description: "受けるダメージや生存力に関わる効果" },
  { id: "accuracy", label: "命中を上げたい", description: "命中Pが伸びる効果" },
];

/** 「火力」だけは purposes ではなく**攻撃ダメージ効果を持つか**で拾う(カタログの
 *  purposes は能力値side の分類なので、ダメージ系がそこに入っていない) */
export const matchesPurpose = (def: BuffDefinition, purpose: BuffPurpose): boolean =>
  purpose === "damage" ? def.damage_effects.length > 0 : def.purposes.includes(purpose);

export const isChoiceValue = (v: BuffValue): v is { choice: number[] } =>
  typeof v === "object" && v !== null && "choice" in v;

export const userInputRange = (v: BuffValue): { min: number; max: number } | null =>
  typeof v === "object" && v !== null && "user_input" in v ? v.user_input : null;

export const isFixedValue = (v: BuffValue): v is { fixed: number } =>
  typeof v === "object" && v !== null && "fixed" in v;

/** 記録するだけ(計算に入らない)のバフか */
export const isRecordOnly = (v: BuffValue): boolean => v === "record_only";

export const isUserSelectedTarget = (t: BuffTarget): boolean =>
  t === "user_selected" || t === "user_selected_multi";

/** 同じバフを複数のステに、それぞれ別の値で掛けられる対象(クラブエフェクト)。
 *  1 ステ = 1 choice で表す — 同じステを 2 回は置けない(domain が弾く) */
export const isMultiTarget = (t: BuffTarget): boolean => t === "user_selected_multi";

export const isPercentLayer = (layer: StatLayer): boolean =>
  layer === "percent_of_base" || layer === "multiplier_b";

/** ON にしたときの初期選択は Rust(`BuffDefinition::default_choice`)。対象ステだけ画面が選ぶ */
function initialChoice(def: BuffDefinition, stat?: StatKind): BuffChoice {
  return { ...def.default_choice, stat: isUserSelectedTarget(def.target) ? (stat ?? STAT_KINDS[0]) : null };
}

/** バフの ON/OFF を反映した新しい選択配列を返す(元の配列は変更しない) */
export function toggleBuff(
  choices: BuffChoice[],
  def: BuffDefinition,
  checked: boolean,
  stat?: StatKind,
): BuffChoice[] {
  if (checked) return [...choices, initialChoice(def, stat)];
  return choices.filter((c) => c.buff_id !== def.id);
}

/** 複数ステ対象バフ(クラブエフェクト)の、1 ステぶんの ON/OFF。
 *  並びは常に `STAT_KINDS` の順に揃える — 選ぶ順で行が入れ替わると、
 *  あとから足した行のせいで既にある行が動く(§09 規則 2) */
export function toggleBuffStat(
  choices: BuffChoice[],
  def: BuffDefinition,
  stat: StatKind,
  checked: boolean,
): BuffChoice[] {
  const rest = choices.filter((c) => c.buff_id !== def.id);
  const mine = choices.filter((c) => c.buff_id === def.id && c.stat !== stat);
  if (checked) mine.push(initialChoice(def, stat));
  mine.sort((a, b) => STAT_KINDS.indexOf(a.stat!) - STAT_KINDS.indexOf(b.stat!));
  return [...rest, ...mine];
}

/** 複数ステ対象バフで、いま選ばれているステ(`STAT_KINDS` 順) */
export function pickedStats(choices: BuffChoice[], def: BuffDefinition): StatKind[] {
  return STAT_KINDS.filter((kind) =>
    choices.some((c) => c.buff_id === def.id && c.stat === kind),
  );
}
