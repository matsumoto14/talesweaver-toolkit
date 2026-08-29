// crates/domain の値域上限(get_stat_limits)。起動時に App.svelte から 1 回取得する。
// 取得完了までの一瞬は既存のフロントリテラルと同じ値をフォールバックとして使う。
// **crates/domain の定数を変えたらここも直す。**ずれていると、起動直後に触った値が
// Rust 側の検証に落ちる(実際に equipment_value_max で起きた)。
import { getStatLimits } from "./api/commands";
import type { StatLimits } from "./api/types";

const FALLBACK: StatLimits = {
  base_stat_max: 310,
  rune_level_max: 20,
  crown_base_max: 100,
  crown_selected_max: 300,
  crown_step: 10,
  monster_card_max: 70,
  sacred_relic_stage_max: 40,
  soul_link_equipment_level_max: 25,
  soul_link_critical_damage_level_max: 20,
  soul_link_final_damage_level_max: 5,
  soul_link_weapon_enhance_level_max: 20,
  soul_link_armor_enhance_level_max: 20,
  adjustment_add_min: -3000,
  adjustment_add_max: 3000,
  adjustment_pin_min: 1,
  adjustment_pin_max: 3000,
  equipment_value_max: 1000,
  strong_weapon_level_max: 6,
  enhance_level_max: 15,
  core_slot_count: 6,
  core_evolution_max: 4,
  core_enhancement_max: 4,
  equipment_element_value_max: 9,
  element_value_max: 255,
  awakening_stage_max: 5,
  eternal_level_max: 100,
  random_option_value_max: 100,
  protect_armor_level_max: 6,
  kai_protect_armor_level_max: 5,
  sharpness_vision_level_max: 10,
  augment_level_max: 5,
  unleash_level_max: 10,
  unleash_slots: 2,
  reinforce_level_max: 5,
  hyper_limit_level_max: 6,
  critical_rate_bonus_max: 100,
  architect_lab_stage_max: 10,
  architect_lab_per_stage: 3,
  ultimate_rune_bonus_max: 20,
  deadly_blow_bonus_max: 100,
  power_weapon_rate: 0.02,
  strong_weapon_rate_per_level: 0.03,
  coat_armor_physical_rate: 0.18,
  coat_armor_magic_rate: 0.12,
  protect_armor_physical_rates: [0.36, 0.45, 0.54, 0.63, 0.72, 0.81],
  protect_armor_magic_rates: [0.24, 0.30, 0.36, 0.42, 0.48, 0.54],
  kai_protect_armor_physical_rates: [0.09, 0.18, 0.27, 0.36, 0.45],
  kai_protect_armor_magic_rates: [0.06, 0.12, 0.18, 0.24, 0.30],
  sharpness_vision_rates: [0.05, 0.10, 0.15, 0.20, 0.25, 0.28, 0.31, 0.34, 0.37, 0.40],
  unleash_rates: [0.01, 0.02, 0.03, 0.04, 0.05, 0.08, 0.11, 0.14, 0.17, 0.20],
  pet_skill_tier_bonus: [
    { tier: "basic", bonus: 20 },
    { tier: "true_lv1", bonus: 30 },
    { tier: "true_lv2", bonus: 40 },
    { tier: "true_lv3", bonus: 50 },
    { tier: "true_lv4", bonus: 60 },
  ],
  sacred_relic_value_per_stage: 10,
  core_power_bonus_table: [
    [1, 2, 3, 4, 5],
    [6, 7, 8, 9, 10],
    [12, 14, 16, 18, 20],
    [23, 26, 29, 32, 35],
    [40, 50, 60, 70, 80],
  ],
  core_support_bonus_table: [
    [1, 2, 3, 4, 5],
    [6, 7, 8, 9, 10],
    [12, 14, 16, 18, 20],
    [23, 26, 29, 32, 35],
    [40, 45, 50, 55, 60],
  ],
  part_slot_rules: [
    { slot: "weapon", label: "武器", ability_slots: 3, allows_ability: true, allows_enhance: true, allows_siena: true, siena_counts_as_equipment: true, allows_random_option: true, random_option_slots: 3, allows_element: true },
    { slot: "armor", label: "鎧", ability_slots: 2, allows_ability: true, allows_enhance: true, allows_siena: true, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "helm", label: "兜", ability_slots: 1, allows_ability: true, allows_enhance: false, allows_siena: true, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "shield", label: "盾", ability_slots: 1, allows_ability: true, allows_enhance: false, allows_siena: true, siena_counts_as_equipment: true, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "shield_plus", label: "盾+", ability_slots: 2, allows_ability: true, allows_enhance: false, allows_siena: false, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: false },
    { slot: "head", label: "頭", ability_slots: 1, allows_ability: true, allows_enhance: false, allows_siena: true, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "body", label: "体", ability_slots: 0, allows_ability: false, allows_enhance: false, allows_siena: true, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "hand", label: "手", ability_slots: 2, allows_ability: true, allows_enhance: false, allows_siena: true, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "leg", label: "足", ability_slots: 1, allows_ability: true, allows_enhance: false, allows_siena: true, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: true },
    { slot: "effect", label: "効果", ability_slots: 0, allows_ability: false, allows_enhance: false, allows_siena: false, siena_counts_as_equipment: false, allows_random_option: false, random_option_slots: null, allows_element: true },
    { slot: "artifact", label: "AF", ability_slots: 0, allows_ability: false, allows_enhance: false, allows_siena: false, siena_counts_as_equipment: false, allows_random_option: false, random_option_slots: null, allows_element: true },
    { slot: "relic_pendant", label: "レリック(ペンダント)", ability_slots: 1, allows_ability: true, allows_enhance: false, allows_siena: false, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: false },
    { slot: "relic_bracelet", label: "レリック(ブレスレット)", ability_slots: 1, allows_ability: true, allows_enhance: false, allows_siena: false, siena_counts_as_equipment: false, allows_random_option: true, random_option_slots: 2, allows_element: false },
  ],
  damage_category_labels: [
    { category: "attack_power", label: "攻撃力" },
    { category: "attack_random", label: "攻撃力乱数部分" },
    { category: "target_defense", label: "攻撃対象の防御力" },
    { category: "skill_multiplier", label: "スキル倍率" },
    { category: "skill_multiplier_rate", label: "スキル倍率増加(割合)" },
    { category: "skill_multiplier_fixed", label: "スキル倍率増加(固定値)" },
    { category: "critical_multiplier", label: "Cri倍率" },
    { category: "critical_damage_rate", label: "クリティカルダメージ増加" },
    { category: "combo_bonus", label: "コンボボーナス" },
    { category: "element_bonus", label: "属性差ボーナス" },
    { category: "player_cut_rate", label: "カット率(プレイヤー)" },
    { category: "siena_aura_attack_rate", label: "攻撃力増加(シエナのオーラ)" },
    { category: "final_damage_fixed", label: "最終ダメージ(固定値)" },
    { category: "final_damage_rate", label: "最終ダメージ" },
    { category: "cut_rate_a", label: "カット率A" },
    { category: "damage_reduction", label: "被害減少" },
    { category: "attack_damage_legacy", label: "攻撃ダメージII" },
    { category: "awakening_damage", label: "覚醒ダメージ" },
    { category: "physical_magic_damage_rate", label: "物理/魔法ダメージ増加" },
    { category: "dependency_damage_rate", label: "特定依存ダメージ増加" },
    { category: "damage_absorb", label: "物理/魔法ダメージ吸収" },
    { category: "taken_damage_rate", label: "物理/魔法被ダメージ倍率" },
    { category: "taken_damage_reduction", label: "被ダメージ減少" },
    { category: "damage_amplify", label: "ダメージ増幅" },
    { category: "damage_resistance", label: "ダメージ耐性" },
    { category: "damage_mitigation", label: "ダメージ緩和" },
    { category: "cut_rate_b", label: "カット率B" },
    { category: "basic_trigger_damage_fixed", label: "攻撃ダメージ(基本発動)(固定値)" },
    { category: "attack_damage_rate", label: "攻撃ダメージ" },
    { category: "attack_damage_isabel", label: "攻撃ダメージ(イザベル)" },
    { category: "attack_damage_general", label: "攻撃ダメージ(一般)" },
    { category: "attack_damage_basic_trigger", label: "攻撃ダメージ(基本発動)" },
    { category: "attack_damage_skill", label: "攻撃ダメージ(スキル)" },
    { category: "attack_damage_special", label: "攻撃ダメージ(特殊)" },
    { category: "attack_damage_japan", label: "攻撃ダメージ(日本独自)" },
    { category: "pvp_correction", label: "PVP補正" },
  ],
  equipment_stat_labels: [
    { kind: "thrust", label: "突き攻撃力" },
    { kind: "slash", label: "斬り攻撃力" },
    { kind: "physical_defense", label: "物理防御力" },
    { kind: "magic_attack", label: "魔法攻撃力" },
    { kind: "magic_defense", label: "魔法防御力" },
    { kind: "accuracy", label: "命中率補正" },
    { kind: "critical", label: "クリティカル補正" },
    { kind: "evasion", label: "回避率補正" },
    { kind: "agility", label: "敏捷度補正" },
  ],
};

export const limits = $state<StatLimits>({ ...FALLBACK });

/**
 * 起動時に FALLBACK と実取得値を deep-compare し、差異があれば console.warn する
 * (差異のあったキー一覧つき)。ユーザー向け UI には出さない — 開発時に手動同期のズレへ
 * 気付くための防波堤(ファイル冒頭コメントの事故対策)。
 */
function warnOnFallbackMismatch(actual: StatLimits): void {
  const mismatched = (Object.keys(FALLBACK) as (keyof StatLimits)[]).filter(
    (key) => JSON.stringify(FALLBACK[key]) !== JSON.stringify(actual[key]),
  );
  if (mismatched.length > 0) {
    console.warn(
      `[limits] FALLBACK が get_stat_limits の実取得値とずれています。該当キー: ${mismatched.join(", ")}`,
    );
  }
}

export async function loadStatLimits(): Promise<void> {
  const v = await getStatLimits();
  warnOnFallbackMismatch(v);
  Object.assign(limits, v);
}
