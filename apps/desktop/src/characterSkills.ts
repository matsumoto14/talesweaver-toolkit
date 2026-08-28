// キャラスキル(パッシブ・自己バフ・味方バフ)の選択と効果の解決。
// crates/domain/src/character_skill.rs と同じ規則で解く:
// - 味方スキルは相手のマスタリーが分からないので差し替えを見ない
// - 自分のスキルは、取っているマスタリーで効果が丸ごと差し替わる
import type { CharacterSkillDef, DamageCategory, SkillEffect } from "./api/types";
import { STAT_LABELS } from "./labels";
import { limits } from "./limits.svelte";

/** 与ダメージ式のカテゴリの日本語名。唯一の正は Rust の DamageCategory::label
 * (StatLimits.damage_category_labels 経由。crates/domain/src/category.rs)。 */
export const damageCategoryLabel = (c: DamageCategory): string =>
  limits.damage_category_labels.find((d) => d.category === c)?.label ?? c;

/** 取っているマスタリーを踏まえた実際の効果 */
export function resolvedEffects(def: CharacterSkillDef, pickedMasteries: string[]): SkillEffect[] {
  if (def.audience === "ally") return def.effects;
  const override = def.mastery_overrides.find((o) => pickedMasteries.includes(o.mastery_id));
  return override ? override.effects : def.effects;
}

export const isRecordOnlyEffect = (e: SkillEffect): boolean => e === "record_only";

/** 効き先の要約(1 行)。記録のみしか無いスキルは null(呼び出し側が note を出す) */
export function effectLabel(def: CharacterSkillDef, pickedMasteries: string[]): string | null {
  const labels = resolvedEffects(def, pickedMasteries)
    .map((e) => {
      if (e === "record_only") return null;
      if ("stat_rate" in e) {
        const stats = e.stat_rate.stats.map((k) => STAT_LABELS[k]).join(" / ");
        return `${stats} +${e.stat_rate.percent}%`;
      }
      if ("actual_delay" in e) return `中ディレイ −${e.actual_delay.percent}%`;
      const { category, percent } = e.damage;
      const sign = percent < 0 ? "−" : "+";
      return `${damageCategoryLabel(category)} ${sign}${Math.abs(percent)}%`;
    })
    .filter((s): s is string => s !== null);
  return labels.length === 0 ? null : labels.join(" ・ ");
}

/** ON にしているスキルの中ディレイ減少の合計 %(このキャラのぶんだけ) */
export function actualDelayPercent(
  skillIds: string[],
  catalog: CharacterSkillDef[],
  pickedMasteries: string[],
): number {
  let sum = 0;
  for (const id of skillIds) {
    const def = catalog.find((d) => d.id === id);
    if (!def) continue;
    for (const e of resolvedEffects(def, pickedMasteries)) {
      if (e !== "record_only" && "actual_delay" in e) sum += e.actual_delay.percent;
    }
  }
  return sum;
}

/** ON にしているスキルの、与ダメージ式カテゴリごとの合計 %(効き先ごとに上限が違う) */
export function damagePercentByCategory(
  skillIds: string[],
  catalog: CharacterSkillDef[],
  pickedMasteries: string[],
): Map<DamageCategory, number> {
  const out = new Map<DamageCategory, number>();
  for (const id of skillIds) {
    const def = catalog.find((d) => d.id === id);
    if (!def) continue;
    for (const e of resolvedEffects(def, pickedMasteries)) {
      if (e === "record_only" || !("damage" in e)) continue;
      const { category, percent } = e.damage;
      out.set(category, (out.get(category) ?? 0) + percent);
    }
  }
  return out;
}

/** ON/OFF を反映した新しい id 配列を返す(元の配列は変更しない) */
export function toggleCharacterSkill(skillIds: string[], id: string, on: boolean): string[] {
  const rest = skillIds.filter((x) => x !== id);
  return on ? [...rest, id] : rest;
}

/** このキャラが ON にできるスキル(自分のスキル / 味方から受けるスキル) */
export const ownSkills = (catalog: CharacterSkillDef[], gameCharacterId: string) =>
  catalog.filter((d) => d.audience === "self_only" && d.game_character_id === gameCharacterId);
export const allySkills = (catalog: CharacterSkillDef[]) =>
  catalog.filter((d) => d.audience === "ally");

/** キャラ種を変えたときに残ってはいけない id(旧キャラ専用のスキル)を落とす */
export function dropForeignSkills(
  skillIds: string[],
  catalog: CharacterSkillDef[],
  gameCharacterId: string,
): string[] {
  return skillIds.filter((id) => {
    const def = catalog.find((d) => d.id === id);
    if (!def) return false;
    return def.audience === "ally" || def.game_character_id === gameCharacterId;
  });
}
