// キャラスキル(パッシブ・自己バフ・味方バフ)の選択と表示。
// マスタリーによる効果の差し替え・中ディレイ/ダメージの合算は Rust 側(preview_effective_stats /
// resolve_character_skill_effects)がすべて解決した結果を返すので、ここでは再実装しない
// (crates/domain/src/character_skill.rs の effects() / actual_delay_contributions() /
// damage_contributions() が唯一の正)。
import type {
  Attacker, CharacterSkillDef, CharacterSkillEffectsView, DamageCategory, Skill, SkillEffect, SummonForm,
} from "./api/types";
import { fmtPct, fmtSigned } from "./format";
import { ELEMENT_LABELS, STAT_LABELS } from "./labels";
import { tables } from "./tables.svelte";
import type { PickerOption } from "./ui/Picker.svelte";

/** 与ダメージ式のカテゴリの日本語名。唯一の正は Rust の DamageCategory::label
 * (StatLimits.damage_category_labels 経由。crates/domain/src/category.rs)。 */
export const damageCategoryLabel = (c: DamageCategory): string =>
  tables.damage_category_labels.find((d) => d.category === c)?.label ?? c;

/** 単一の効果を 1 行の要約文字列にする。record_only は null(呼び出し側が既定文言を出す)。
 * キャラスキル(複数効果を並べる effectLabel)とマスタリー(1 択なのでこれをそのまま使う)で共通 */
export function singleEffectLabel(e: SkillEffect): string | null {
  if (e === "record_only") return null;
  if ("stat_rate" in e) {
    const stats = e.stat_rate.stats.map((k) => STAT_LABELS[k]).join(" / ");
    return `${stats} ${fmtSigned(e.stat_rate.percent, { max: 2 }, "%")}`;
  }
  if ("actual_delay" in e) return `中ディレイ ${fmtSigned(-e.actual_delay.percent, { max: 2 }, "%")}`;
  if ("accuracy_point" in e) return `命中P ${fmtSigned(e.accuracy_point.value)}`;
  if ("min_evasion_rate" in e) return `最小回避率補正 ${fmtSigned(e.min_evasion_rate.value, { max: 2 }, "%")}`;
  // Rust の SkillEffect::label と同じ文言
  if ("accuracy_rate" in e) return `命中P割合増加(SLv×${fmtPct(e.accuracy_rate.per_level, { max: 2 })})`;
  const { category, percent } = e.damage;
  // 敵にかけるデバフは S(被ダメージ減少)に負値で積む。画面はプレイヤーの語彙で出す
  // (「被ダメージ減少 −10%」は意味が逆に読める)。唯一の正は Rust の SkillEffect::label
  if (category === "taken_damage_reduction" && percent < 0) {
    return `敵被ダメージ ${fmtSigned(-percent, { max: 2 }, "%")}`;
  }
  return `${damageCategoryLabel(category)} ${fmtSigned(percent, { max: 2 }, "%")}`;
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

/** 名前だけでは選べない。単 / 範・段数・属性を名前の隣に出す */
export const skillMeta = (s: Skill): string =>
  `${s.target === null ? "?" : s.target === "single" ? "単" : "範"} ・ ` +
  `${s.hit_count} 段 ・ ${ELEMENT_LABELS[s.element]} ・ ` +
  `中 ${s.base_actual_delay === null ? "?" : `${s.base_actual_delay}s`}`;

/** 主軸に選ばれるのはほぼ火力上位。この件数をチップで手前に固定し、それ以外は候補面に送る */
export const MAIN_SKILL_PINNED = 3;

/**
 * 主軸スキルの選択肢。並びは list_skills(Rust `Skill::main_skill_order`: 単体優先 →
 * 中ディレイ込みの継続火力順)のまま。先頭 MAIN_SKILL_PINNED 件を「よく使う」として固定する
 * (§07「1 つ選ぶ」: ドメイン知識で固定、使用履歴で並べない)。
 * 空欄は候補の 1 行(value = "")で、文言は呼び出し側の文脈で変える。
 *
 * `attacker`(既定 `player`)で候補を絞る。本体の主軸は召喚獣(熊・破壊精霊)が撃つスキルを
 * 選べない(ADR-016)ので、召喚欄の Picker は `attacker: "summon"` を渡して呼ぶ
 * (`!== "player"` = 熊と精霊をまとめて 1 つの召喚欄の候補にする。攻撃者を 2 系統に分けない)。
 *
 * `summonForm` を渡すと、召喚欄の候補をさらにその型(主軸が決めた「どの人形 / 精霊か」)だけに
 * 絞る(2026-09-18 追記)。主軸が型を決めないとき(`summonForm` が `null` / 未指定)は絞らない。
 */
export function mainSkillOptions(
  skills: Skill[],
  emptyLabel: string,
  emptyMeta: string,
  attacker: Attacker | "summon" = "player",
  summonForm?: SummonForm | null,
): PickerOption[] {
  return [
    { value: "", name: emptyLabel, meta: emptyMeta, iconId: null },
    ...skills
      .filter((s) => (attacker === "summon" ? s.attacker !== "player" : s.attacker === attacker))
      .filter((s) => summonForm == null || s.summon_form === summonForm)
      .map((s, i) => ({
        value: s.id, name: s.name, meta: skillMeta(s), iconId: s.id, iconKind: "skill" as const,
        pinned: i < MAIN_SKILL_PINNED,
      })),
  ];
}

/** ON/OFF を反映した新しい id 配列を返す(元の配列は変更しない) */
export function toggleCharacterSkill(
  skillIds: string[], id: string, on: boolean, catalog: CharacterSkillDef[] = [],
): string[] {
  // 同じスキルの強さ違い(カース・ペンジュラムの通常と【シンボルオブスピリット】)は
  // 両方 ON にすると二重計上になる。押した方を採って相手を落とす —
  // 「押した瞬間に結果が動く」ので、警告を出して考えさせない
  const exclusive = catalog.find((d) => d.id === id)?.exclusive_with ?? [];
  const rest = skillIds.filter((x) => x !== id && !(on && exclusive.includes(x)));
  return on ? [...rest, id] : rest;
}

/** このキャラが ON にできるスキル(自分のスキル / 味方から受けるスキル) */
export const ownSkills = (catalog: CharacterSkillDef[], gameCharacterId: string) =>
  catalog.filter((d) => d.audience === "self_only" && d.game_character_id === gameCharacterId);
export const allySkills = (catalog: CharacterSkillDef[]) =>
  catalog.filter((d) => d.audience === "ally");
/** 敵にかけるデバフ。同行者がかける前提なので誰でも ON にできる */
export const enemySkills = (catalog: CharacterSkillDef[]) =>
  catalog.filter((d) => d.audience === "enemy");

