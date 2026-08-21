// Tauri コマンドの型とラッパー。形状は Rust の serde 構造体(crates/domain, storage, gamedata)に従う。
import { invoke } from "@tauri-apps/api/core";

export type StatKind = "stab" | "hack" | "int" | "def" | "mr" | "dex" | "agi";
export const STAT_KINDS: StatKind[] = ["stab", "hack", "int", "def", "mr", "dex", "agi"];
export const STAT_LABELS: Record<StatKind, string> = {
  stab: "STAB", hack: "HACK", int: "INT", def: "DEF", mr: "MR", dex: "DEX", agi: "AGI",
};

export type BaseStats = Record<StatKind, number>;

export interface Awakening {
  stage: number;
  eternal_level: number;
}

export interface GameCharacter {
  id: string;
  name: string;
}

export type SkillDependency = "stab" | "hack" | "int" | "mr" | "stab_hack" | "hack_int";

export interface Skill {
  id: string;
  name: string;
  dependency: SkillDependency;
  multiplier: number;
  hit_count: number;
  critical_multiplier: number;
}

export interface Enemy {
  id: string;
  name: string;
  defense: number;
  damage_reduction: number;
  cut_rate_a: number;
  element_threshold: number;
}

export interface RegisteredCharacter {
  id: number;
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
}

export interface NewCharacter {
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
}

export type CategoryKind = "assigned" | "fixed" | "rate";

export interface CategoryCap {
  min: number | null;
  max: number | null;
}

export interface CategoryTrace {
  category: string;
  symbol: string;
  label: string;
  kind: CategoryKind;
  value: number;
  factor: number;
  cap: CategoryCap | null;
}

export interface StatTrace {
  kind: StatKind;
  base: number;
  percent_of_base_total: number;
  fixed: number;
  multiplier_a: number;
  basic: number;
  multiplier_b: number;
  multiplier_b_bonus: number;
  final_fixed: number;
  effective: number;
}

export interface FormulaStep {
  name: string;
  expression: string;
  value: number;
}

export interface DamageTriple {
  min: number;
  max: number;
  critical: number;
}

export interface DamageTrace {
  stats: StatTrace[];
  categories: CategoryTrace[];
  steps_min: FormulaStep[];
  steps_max: FormulaStep[];
  steps_critical: FormulaStep[];
}

export interface DamageResult {
  per_hit: DamageTriple;
  total: DamageTriple;
  hit_count: number;
  trace: DamageTrace;
}

export const listGameCharacters = () => invoke<GameCharacter[]>("list_game_characters");
export const listSkills = (gameCharacterId: string) => invoke<Skill[]>("list_skills", { gameCharacterId });
export const listEnemies = () => invoke<Enemy[]>("list_enemies");
export const listCharacters = () => invoke<RegisteredCharacter[]>("list_characters");
export const createCharacter = (character: NewCharacter) =>
  invoke<RegisteredCharacter>("create_character", { character });
export const deleteCharacter = (id: number) => invoke<void>("delete_character", { id });
export const calculateDamage = (characterId: number, skillId: string, enemyId: string, comboCount: number) =>
  invoke<DamageResult>("calculate_damage", { characterId, skillId, enemyId, comboCount });

/** invoke の reject(String)を表示用文字列にする */
export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
