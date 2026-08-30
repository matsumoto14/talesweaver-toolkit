// キャラスキル(パッシブ・自己バフ・味方バフ)の選択と表示。
// マスタリーによる効果の差し替え・中ディレイ/ダメージの合算は Rust 側(preview_effective_stats /
// resolve_character_skill_effects)がすべて解決した結果を返すので、ここでは再実装しない
// (crates/domain/src/character_skill.rs の effects() / actual_delay_contributions() /
// damage_contributions() が唯一の正)。
import type {
  CharacterSkillDef, CharacterSkillEffectsView, DamageCategory, Skill, SkillEffect,
} from "./api/types";
import { ELEMENT_LABELS, STAT_LABELS } from "./labels";
import { limits } from "./limits.svelte";
import type { PickerOption } from "./ui/Picker.svelte";

/** 与ダメージ式のカテゴリの日本語名。唯一の正は Rust の DamageCategory::label
 * (StatLimits.damage_category_labels 経由。crates/domain/src/category.rs)。 */
export const damageCategoryLabel = (c: DamageCategory): string =>
  limits.damage_category_labels.find((d) => d.category === c)?.label ?? c;

/** 単一の効果を 1 行の要約文字列にする。record_only は null(呼び出し側が既定文言を出す)。
 * キャラスキル(複数効果を並べる effectLabel)とマスタリー(1 択なのでこれをそのまま使う)で共通 */
export function singleEffectLabel(e: SkillEffect): string | null {
  if (e === "record_only") return null;
  if ("stat_rate" in e) {
    const stats = e.stat_rate.stats.map((k) => STAT_LABELS[k]).join(" / ");
    return `${stats} +${e.stat_rate.percent}%`;
  }
  if ("actual_delay" in e) return `中ディレイ −${e.actual_delay.percent}%`;
  const { category, percent } = e.damage;
  const sign = percent < 0 ? "−" : "+";
  return `${damageCategoryLabel(category)} ${sign}${Math.abs(percent)}%`;
}

/** 効き先の要約(1 行)。`effects` はマスタリー解決済み(resolve_character_skill_effects の結果)。
 * 記録のみしか無いスキルは null(呼び出し側が note を出す) */
export function effectLabel(effects: SkillEffect[]): string | null {
  const labels = effects.map(singleEffectLabel).filter((s): s is string => s !== null);
  return labels.length === 0 ? null : labels.join(" ・ ");
}

/** キャラスキル 1 件ぶんの、マスタリー解決済みの効果(resolve_character_skill_effects の結果から引く)。
 * まだ取得できていなければ空配列(record_only 扱いと同じ表示になる) */
export const resolvedEffectsOf = (id: string, resolved: CharacterSkillEffectsView[]): SkillEffect[] =>
  resolved.find((e) => e.id === id)?.effects ?? [];

// --- 主軸スキル(攻撃力の依存種別を決める、Skill 由来)------------------------
// キャラ登録(RegisterPane)とキャラワークスペース(StatusPane)で同じ選び方をする。
// 火力の目安(power / power_per_second)は gamedata 側で確定済みの値をそのまま使う
// (正は crates/domain/src/skill.rs の Skill::compute_power / compute_power_per_second)。

/**
 * 主軸候補の順。対ボスで使う単体スキルを先にし、
 * その中を中ディレイ込みの継続火力順にする。依存種別では絞らない。
 * 斬り・物理複合・魔剣など、別ビルドの入口を候補から消さないため。
 * 中ディレイ不明のものは既知のものより後ろで、1 回ぶんの火力順にする。
 */
export function compareMainSkills(a: Skill, b: Skill): number {
  if (a.target === "single" && b.target !== "single") return -1;
  if (a.target !== "single" && b.target === "single") return 1;
  if (a.power_per_second !== null && b.power_per_second !== null) return b.power_per_second - a.power_per_second;
  if (a.power_per_second !== null) return -1;
  if (b.power_per_second !== null) return 1;
  return b.power - a.power;
}

/** 名前だけでは選べない。単 / 範・段数・属性を名前の隣に出す */
export const skillMeta = (s: Skill): string =>
  `${s.target === null ? "?" : s.target === "single" ? "単" : "範"} ・ ` +
  `${s.hit_count} 段 ・ ${ELEMENT_LABELS[s.element]} ・ ` +
  `中 ${s.base_actual_delay === null ? "?" : `${s.base_actual_delay}s`}`;

/** 単体優先・中ディレイ込みの継続火力順の選択肢。空欄の文言は呼び出し側の文脈で変える。 */
export function mainSkillOptions(
  skills: Skill[],
  emptyLabel: string,
): PickerOption[] {
  return [
    { value: "", name: emptyLabel, iconId: null },
    ...[...skills]
      .sort(compareMainSkills)
      .map((s) => ({ value: s.id, name: s.name, meta: skillMeta(s), iconId: s.id, iconKind: "skill" as const })),
  ];
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
