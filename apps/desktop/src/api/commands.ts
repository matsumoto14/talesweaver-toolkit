// Tauri コマンドの呼び出し。引数・戻り値の形は api/types.ts に従う。
import { invoke } from "@tauri-apps/api/core";
import type {
  Adjustments, BaseStats, BuffDefinition, DamageResult, Enemy, Equipment, EquipmentAbilityDef, EquipmentItem, GameCharacter,
  NewCharacter, RegisteredCharacter, ContentArea, ContentEvaluation, DefenseProfile,
  ElementPreview, ElementSourceDef, Skill, StatLimits, StatPreview, StatSources,
} from "./types";

export const listGameCharacters = () => invoke<GameCharacter[]>("list_game_characters");
export const listSkills = (gameCharacterId: string) => invoke<Skill[]>("list_skills", { gameCharacterId });
export const listEnemies = () => invoke<Enemy[]>("list_enemies");
export const listBuffCatalog = () => invoke<BuffDefinition[]>("list_buff_catalog");
/** 属性値の供給源カタログ(装備の属性強化以外) */
export const listElementSources = () => invoke<ElementSourceDef[]>("list_element_sources");
/** 属性値の内訳(キャラ基礎 / 装備 / 供給源 / 合計)。保存前のキャラデータで出す */
export const previewElements = (character: NewCharacter) =>
  invoke<ElementPreview>("preview_elements", { character });
export const listCharacters = () => invoke<RegisteredCharacter[]>("list_characters");
export const createCharacter = (character: NewCharacter) =>
  invoke<RegisteredCharacter>("create_character", { character });
export const updateCharacter = (id: number, character: NewCharacter) =>
  invoke<RegisteredCharacter>("update_character", { id, character });
export const deleteCharacter = (id: number) => invoke<void>("delete_character", { id });
/**
 * 保存しない試算。draft の base_stats/stat_sources/equipment から最終能力値と寄与内訳を得る。
 * `mainSkillId`(主軸スキル)を渡すとその依存種別で攻撃力(A)も返る。null なら攻撃力は出ない。
 */
export const previewEffectiveStats = (
  baseStats: BaseStats, statSources: StatSources, equipment: Equipment, gameCharacterId: string,
  mainSkillId: string | null,
) => invoke<StatPreview>("preview_effective_stats", {
  baseStats, statSources, equipment, gameCharacterId, mainSkillId,
});
export const calculateDamage = (
  characterId: number, skillId: string, contentId: string, comboCount: number, temporaryAdjustments: Adjustments,
) => invoke<DamageResult>("calculate_damage", { characterId, skillId, contentId, comboCount, temporaryAdjustments });
export const getStatLimits = () => invoke<StatLimits>("get_stat_limits");
/** 防御側の戦闘能力値(docs/damage-formula.md §6〜7)。対象コンテンツに依らない */
export const previewDefense = (character: NewCharacter) =>
  invoke<DefenseProfile>("preview_defense", { character });
export const listEquipmentCatalog = () => invoke<EquipmentItem[]>("list_equipment_catalog");
export const listEquipmentAbilities = () => invoke<EquipmentAbilityDef[]>("list_equipment_abilities");

/** invoke の reject(String)を表示用文字列にする */
export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export const listContents = () => invoke<ContentArea[]>("list_contents");
/** 保存前のキャラデータ(編集中 draft・試し変更)でダメージ計算する。DB には書き込まない */
export const previewDamage = (
  character: NewCharacter, skillId: string, contentId: string, comboCount: number,
  temporaryAdjustments: Adjustments | null = null,
) => invoke<DamageResult>("preview_damage", { character, skillId, contentId, comboCount, temporaryAdjustments });
/**
 * 全コンテンツの到達判定(火力は最大ダメージのスキル・コンボなしで評価)。
 * `dependencySkillId` を渡すと、装備条件(スキル依存で比較先が変わる)をそのスキルで判定する。
 */
export const evaluateContents = (character: NewCharacter, dependencySkillId?: string) =>
  invoke<ContentEvaluation[]>("evaluate_contents", {
    character,
    dependencySkillId: dependencySkillId ?? null,
  });
