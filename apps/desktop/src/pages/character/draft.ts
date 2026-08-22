// CharacterWorkspace/CharacterData/CharacterSettings で共有する編集中ドラフトの型と組み立て関数。
import type { Adjustments, BaseStats, Equipment, RegisteredCharacter, StatSources } from "../../api/types";
import { STAT_KINDS } from "../../labels";

export interface Draft {
  name: string;
  gameCharacterId: string;
  baseStats: BaseStats;
  stage: string;
  eternalLevel: string;
  statSources: StatSources;
  equipment: Equipment;
}

export const cloneEquipment = (src: Equipment): Equipment => ({
  base: { ...src.base },
  enhanced: { ...src.enhanced },
  power_weapon: src.power_weapon,
  strong_weapon_level: src.strong_weapon_level,
});

export const neutralEquipment = (): Equipment => ({
  base: { thrust: 0, slash: 0, magic_attack: 0, magic_defense: 0 },
  enhanced: { thrust: 0, slash: 0, magic_attack: 0, magic_defense: 0 },
  power_weapon: false,
  strong_weapon_level: 0,
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
});
