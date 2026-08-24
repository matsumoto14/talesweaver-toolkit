// キャラ編集で共有する編集中ドラフトの型と組み立て関数(キャラタブと登録ペインで使用)。
import type {
  Adjustments,
  BaseStats,
  Equipment,
  EquipmentParts,
  NewCharacter,
  RegisteredCharacter,
  StatSources,
} from "./api/types";
import { cloneEquipmentPart, cloneThesisCores, neutralEquipmentPart, neutralThesisCores } from "./equipment";
import { PART_SLOTS, STAT_KINDS } from "./labels";

export interface Draft {
  name: string;
  gameCharacterId: string;
  baseStats: BaseStats;
  stage: string;
  eternalLevel: string;
  statSources: StatSources;
  equipment: Equipment;
  /** 主軸スキル(攻撃力の依存種別を決める)。"" = 未選択 */
  mainSkillId: string;
}

export const cloneEquipment = (src: Equipment): Equipment => ({
  parts: Object.fromEntries(
    PART_SLOTS.map((slot) => [slot, cloneEquipmentPart(src.parts[slot])]),
  ) as unknown as EquipmentParts,
  power_weapon: src.power_weapon,
  strong_weapon_level: src.strong_weapon_level,
  thesis_cores: cloneThesisCores(src.thesis_cores),
});

export const neutralEquipment = (): Equipment => ({
  parts: Object.fromEntries(PART_SLOTS.map((slot) => [slot, neutralEquipmentPart()])) as unknown as EquipmentParts,
  power_weapon: false,
  strong_weapon_level: 0,
  thesis_cores: neutralThesisCores(),
});

export const cloneAdjustments = (src: Adjustments): Adjustments =>
  Object.fromEntries(STAT_KINDS.map((k) => [k, { add: src[k].add, pin: src[k].pin }])) as Adjustments;

export const cloneStatSources = (src: StatSources): StatSources => ({
  pet_skills: { ...src.pet_skills },
  rune_levels: { ...src.rune_levels },
  crown: { ...src.crown },
  sacred_relic: { ...src.sacred_relic },
  buffs: { choices: src.buffs.choices.map((b) => ({ ...b })) },
  adjustments: cloneAdjustments(src.adjustments),
});

export const neutralStatSources = (): StatSources => ({
  pet_skills: Object.fromEntries(STAT_KINDS.map((k) => [k, null])) as StatSources["pet_skills"],
  rune_levels: Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as StatSources["rune_levels"],
  crown: Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as StatSources["crown"],
  sacred_relic: Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as StatSources["sacred_relic"],
  buffs: { choices: [] },
  adjustments: Object.fromEntries(STAT_KINDS.map((k) => [k, { add: 0, pin: null }])) as StatSources["adjustments"],
});

export const buildDraft = (c: RegisteredCharacter): Draft => ({
  name: c.name,
  gameCharacterId: c.game_character_id,
  baseStats: { ...c.base_stats },
  stage: String(c.awakening.stage),
  eternalLevel: String(c.awakening.eternal_level),
  statSources: cloneStatSources(c.stat_sources),
  equipment: cloneEquipment(c.equipment),
  mainSkillId: c.main_skill_id ?? "",
});

/** Draft → コマンドに渡すペイロード(保存・保存前プレビューの両方で使う) */
export const draftToPayload = (draft: Draft): NewCharacter => ({
  name: draft.name.trim(),
  game_character_id: draft.gameCharacterId,
  base_stats: { ...draft.baseStats },
  awakening: { stage: Number(draft.stage), eternal_level: Number(draft.eternalLevel) },
  stat_sources: cloneStatSources(draft.statSources),
  equipment: cloneEquipment(draft.equipment),
  main_skill_id: draft.mainSkillId === "" ? null : draft.mainSkillId,
});
