// Tauri コマンドの呼び出し。引数・戻り値の形は api/types.ts に従う。
import { invoke } from "@tauri-apps/api/core";
import type {
  Adjustments, BaseStats, BuffDefinition, DamageResult, Enemy, GameCharacter, NewCharacter, RegisteredCharacter,
  ContentArea, ContentEvaluation, Skill, StatLimits, StatPreview, StatSources,
} from "./types";

export const listGameCharacters = () => invoke<GameCharacter[]>("list_game_characters");
export const listSkills = (gameCharacterId: string) => invoke<Skill[]>("list_skills", { gameCharacterId });
export const listEnemies = () => invoke<Enemy[]>("list_enemies");
export const listBuffCatalog = () => invoke<BuffDefinition[]>("list_buff_catalog");
export const listCharacters = () => invoke<RegisteredCharacter[]>("list_characters");
export const createCharacter = (character: NewCharacter) =>
  invoke<RegisteredCharacter>("create_character", { character });
export const updateCharacter = (id: number, character: NewCharacter) =>
  invoke<RegisteredCharacter>("update_character", { id, character });
export const deleteCharacter = (id: number) => invoke<void>("delete_character", { id });
/** 保存しない試算。draft の base_stats/stat_sources から最終能力値と寄与内訳を得る */
export const previewEffectiveStats = (baseStats: BaseStats, statSources: StatSources, gameCharacterId: string) =>
  invoke<StatPreview>("preview_effective_stats", { baseStats, statSources, gameCharacterId });
export const calculateDamage = (
  characterId: number, skillId: string, enemyId: string, comboCount: number, temporaryAdjustments: Adjustments,
) => invoke<DamageResult>("calculate_damage", { characterId, skillId, enemyId, comboCount, temporaryAdjustments });
export const getStatLimits = () => invoke<StatLimits>("get_stat_limits");

/** invoke の reject(String)を表示用文字列にする */
export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export const listContents = () => invoke<ContentArea[]>("list_contents");
/** 保存前のキャラデータ(編集中 draft・試し変更)でダメージ計算する。DB には書き込まない */
export const previewDamage = (
  character: NewCharacter, skillId: string, enemyId: string, comboCount: number,
  temporaryAdjustments: Adjustments | null = null,
) => invoke<DamageResult>("preview_damage", { character, skillId, enemyId, comboCount, temporaryAdjustments });
/** 全コンテンツの到達判定(火力は最大ダメージのスキル・コンボなしで評価) */
export const evaluateContents = (character: NewCharacter) =>
  invoke<ContentEvaluation[]>("evaluate_contents", { character });
