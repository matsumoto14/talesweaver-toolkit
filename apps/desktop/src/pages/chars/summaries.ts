// Workspace(補正源リストの行サブタイトル)と各補正源ペイン(先頭の「効いている量」inset ブロック)の
// 両方から呼ぶ純関数。preview(計算結果)/ draft(編集中の値)を受け取り、表示用の値だけを返す
// (draft を書き換える副作用は持たない)。ロジックを 2 か所にコピーしないための置き場所。
import type { EquipmentStatKind } from "../../labels";
import { EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT, PART_SLOT_LABELS, POLISH_KIND_LABELS, STAT_LABELS } from "../../labels";
import { tables } from "../../tables.svelte";
import type { EquipmentValues, SkillDependency, StatPreview } from "../../api/types";
import type { Draft } from "../../draft";
import { avatarEnhanceTotals, zeroValues } from "../../equipment";
import { fmtSigned, fmtSignedPct } from "../../format";

/** wiki の装備攻撃力係数が 0 でない補正だけを、主軸スキルの依存種別から絞る。 */
export function equipmentAttackKindsFor(dependency: SkillDependency | null): EquipmentStatKind[] {
  if (dependency === "hack_int") return ["slash", "magic_attack"];
  if (dependency === "int" || dependency === "mr") return ["magic_attack", "magic_defense"];
  if (dependency !== null) return ["thrust", "slash"];
  return ["thrust", "slash", "magic_attack", "magic_defense"];
}

/**
 * 熊(魔法人形)の装備攻撃力係数が 0 でない補正(斬り・魔攻・魔防、wiki `STAB(熊)` 行)。
 * 熊は依存種別を持たない固定係数なので `equipmentAttackKindsFor` のような分岐表を持たず、
 * Rust 側が起動時に配る `tables.magic_doll_enchant_keys` をそのまま返す(TS に係数を書き写さない)。
 */
export function equipmentAttackKindsForMagicDoll(): EquipmentStatKind[] {
  return tables.magic_doll_enchant_keys;
}

/** 基本能力値の合計(Σ part.base + 装備アビリティ + 称号 + ソウルリンク)。計算は Rust 側(preview) */
export function equipmentBaseTotal(preview: StatPreview | null): EquipmentValues {
  return preview?.equipment_base_total ?? zeroValues();
}

/** 強化能力値の合計(Σ part.enchant + シエナのオーラ武器/盾分)。計算は Rust 側(preview) */
export function equipmentEnhancedTotal(preview: StatPreview | null): EquipmentValues {
  return preview?.equipment_enhanced_total ?? zeroValues();
}

/**
 * テシスコアで一番伸びる地域の合計。**地域ごとに別々のセットを組む**ので合算はできず、
 * 「いま一番良い地域でいくつか」だけが意味を持つ。どの地域が一番かはドメインの問いなので
 * 計算は Rust 側(`ThesisCores::best_total_bonus`)。ここは読むだけ
 */
export function thesisCoreBestTotal(preview: StatPreview | null): number {
  return preview?.thesis_core_best_total ?? 0;
}

/** 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン)。計算は Rust 側 */
export function equipmentAttackRatePercent(preview: StatPreview | null): number {
  return Math.round((preview?.common_skill.equipment_attack_rate ?? 0) * 100);
}

/** 装備防御力倍率(共通スキル + シエナのオーラの防御力増加)の**増加分**(100% を含まない)。計算は Rust 側 */
export function defenseRatePercent(preview: StatPreview | null): { physical: number; magic: number } {
  const rates = preview?.common_skill.defense_rates;
  if (!rates) return { physical: 0, magic: 0 };
  return { physical: Math.round((rates.physical - 1) * 100), magic: Math.round((rates.magic - 1) * 100) };
}

/** シャープネスビジョンの割合追加ダメージ %。正は crates/domain/src/common_skill.rs の SHARPNESS_VISION */
export function sharpnessRatePercent(draft: Draft): number {
  const level = draft.commonSkills.sharpness_vision_level;
  if (level === 0) return 0;
  return Math.round(tables.sharpness_vision_rates[level - 1] * 100);
}

/** アンリーシュ(能力解放)の効き先要約。正は crates/domain/src/common_skill.rs の UNLEASH */
export function unleashSummary(draft: Draft): string {
  return (
    (draft.commonSkills.unleash ?? [])
      .filter((u) => u.stat !== null && u.level > 0)
      .map((u) => `${STAT_LABELS[u.stat!]} ${fmtSignedPct(tables.unleash_rates[u.level - 1])}`)
      .join(" / ") || "未使用"
  );
}

/** ランダムOP のうち、発動条件付きで記録するだけの枠数。計算は Rust 側(preview) */
export function randomOptionRecordOnlyCount(preview: StatPreview | null): number {
  return preview?.random_option_totals.record_only_count ?? 0;
}

/**
 * 装備研磨の記録一覧の要約。例「武器 職人 突き ・ 鎧 聖なる 物防」。
 * ペイン見出しと Workspace の行サブタイトルの両方がこの関数を呼ぶ(2 か所にロジックを置かない)。
 * 実際の加算値(効いている量)はバフ「装備研磨」の ON/OFF に依存するので、ここでは種類・対象だけ出す。
 */
export function polishSummary(draft: Draft): string {
  return (
    draft.equipment.polish.entries
      .map((entry) => `${PART_SLOT_LABELS[entry.slot]} ${POLISH_KIND_LABELS[entry.kind]} ${EQUIPMENT_STAT_SHORT[entry.stat]}`)
      .join(" ・ ") || "未使用"
  );
}

/**
 * アバター強化の「効いている量」要約(能力値ごとの Σ、非 0 だけ)。例「突き +60 ・ 命中 +24」。
 * ペイン見出しと Workspace の行サブタイトルの両方がこの関数を呼ぶ(2 か所にロジックを置かない)。
 */
export function avatarEnhanceSummary(draft: Draft): string {
  const totals = avatarEnhanceTotals(draft.equipment.avatar);
  return (
    EQUIPMENT_STAT_KINDS.filter((k) => totals[k] > 0)
      .map((k) => `${EQUIPMENT_STAT_SHORT[k]} ${fmtSigned(totals[k])}`)
      .join(" ・ ") || "未使用"
  );
}
