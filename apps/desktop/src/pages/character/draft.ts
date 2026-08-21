// CharacterWorkspace/CharacterData/CharacterSettings で共有する編集中ドラフトの型と組み立て関数。
import type { Adjustments, BaseStats, RegisteredCharacter, StatSources } from "../../api/types";
import { STAT_KINDS } from "../../labels";

export interface Draft {
  name: string;
  gameCharacterId: string;
  baseStats: BaseStats;
  stage: string;
  eternalLevel: string;
  statSources: StatSources;
}

export const cloneAdjustments = (src: Adjustments): Adjustments =>
  Object.fromEntries(STAT_KINDS.map((k) => [k, { add: src[k].add, pin: src[k].pin }])) as Adjustments;

export const buildDraft = (c: RegisteredCharacter): Draft => ({
  name: c.name,
  gameCharacterId: c.game_character_id,
  baseStats: { ...c.base_stats },
  stage: String(c.awakening.stage),
  eternalLevel: String(c.awakening.eternal_level),
  statSources: {
    pet_skills: { ...c.stat_sources.pet_skills },
    rune_levels: { ...c.stat_sources.rune_levels },
    crown: { ...c.stat_sources.crown },
    sacred_relic: { ...c.stat_sources.sacred_relic },
    buffs: { choices: c.stat_sources.buffs.choices.map((b) => ({ ...b })) },
    adjustments: cloneAdjustments(c.stat_sources.adjustments),
  },
});
