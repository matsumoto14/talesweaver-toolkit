// キャラ編集で共有する編集中ドラフトの型と組み立て関数(キャラタブと登録ペインで使用)。
import type {
  BaseStats,
  CommonSkills,
  Equipment,
  EquipmentParts, EquipmentPartList,
  NewCharacter,
  RegisteredCharacter,
  StatSources,
} from "./api/types";
import { cloneAvatarEnhancements, cloneEquipmentPart, cloneEquipmentPolishes, cloneSienaAuras, cloneThesisCores, neutralAvatarEnhancements, neutralEquipmentPart, neutralEquipmentPolishes, neutralSienaAuras, neutralThesisCores } from "./equipment";
import { PART_SLOTS, STAT_KINDS } from "./labels";

/**
 * 新規登録の覚醒段階。**このツールのターゲット層は覚醒 5**(遅くても 4)で、
 * エタの意志を解放している時点で 5 が前提になる。既定を 0 にすると、ほぼ全員が
 * 毎回上書きすることになる(ux-guidelines「初期値は実用値」)。
 */
export const DEFAULT_AWAKENING_STAGE = 5;

/**
 * エタの意志 Lv の節目。ここを超えると上限の増え方が一段上がる
 * (crates/gamedata/src/awakening.rs の ETERNAL_CAPS。最大ダメージは Lv20→21 で +70 万、
 * それ以外の区間は +10〜35 万)。育成の目標地点なので、入力はこの節目から選べるようにする。
 */
export const ETERNAL_MILESTONES = [0, 20, 40, 60, 80, 90, 100] as const;

export interface Draft {
  name: string;
  gameCharacterId: string;
  baseStats: BaseStats;
  stage: string;
  eternalLevel: string;
  statSources: StatSources;
  equipment: Equipment;
  /** 共通スキル(wiki: Skill/共通) */
  commonSkills: CommonSkills;
  /** 主軸スキル(攻撃力の依存種別を決める)。"" = 未選択 */
  mainSkillId: string;
  /** 召喚スキル(アナイスの魔法人形に撃たせるスキル)。"" = 未選択 */
  summonSkillId: string;
  /**
   * ホームの「次の目標」。"" = 未設定(自動で選ぶ)。
   * キャラタブでは編集しないが、保存はキャラ全体の上書きなので**必ず持ち回る** —
   * ここから落とすとキャラタブを保存した瞬間にホームで選んだ目標が消える。
   */
  goalContentId: string;
  defaultBuffSetId: number | null;
}

export const cloneEquipment = (src: Equipment): Equipment => ({
  parts: Object.fromEntries(
    PART_SLOTS.map((slot) => [slot, { selected_id: src.parts[slot].selected_id, registered: src.parts[slot].registered.map(cloneEquipmentPart) }]),
  ) as unknown as EquipmentParts,
  siena: cloneSienaAuras(src.siena),
  thesis_cores: cloneThesisCores(src.thesis_cores),
  avatar: cloneAvatarEnhancements(src.avatar),
  polish: cloneEquipmentPolishes(src.polish),
  title: src.title ?? null,
  owned_titles: [...src.owned_titles],
});

/** 新規登録キャラの装備の初期値(全部位 未装備)。 */
export const defaultEquipment = (): Equipment => ({
  parts: Object.fromEntries(PART_SLOTS.map((slot) => [slot, { registered: [], selected_id: null } satisfies EquipmentPartList])) as unknown as EquipmentParts,
  siena: neutralSienaAuras(),
  thesis_cores: neutralThesisCores(),
  avatar: neutralAvatarEnhancements(),
  polish: neutralEquipmentPolishes(),
  title: null,
  owned_titles: [],
});

export const cloneCommonSkills = (src: CommonSkills): CommonSkills => ({
  ...src,
  // v6 以前に保存したキャラには unleash / reinforce_level が無い(serde default で 0)
  unleash: [
    { ...(src.unleash?.[0] ?? { stat: null, level: 0 }) },
    { ...(src.unleash?.[1] ?? { stat: null, level: 0 }) },
  ],
  reinforce_level: src.reinforce_level ?? 0,
  ultimate: { ...src.ultimate, slots: [...src.ultimate.slots] },
});

export const cloneStatSources = (src: StatSources): StatSources => ({
  pet_skills: { ...src.pet_skills },
  rune_levels: { ...src.rune_levels },
  crown: { ...src.crown },
  monster_cards: { ...(src.monster_cards ?? {}) } as StatSources["monster_cards"],
  sacred_relic: { ...src.sacred_relic },
  elements: { ...src.elements },
  character_skills: {
    skill_ids: [...(src.character_skills?.skill_ids ?? [])],
    skill_levels: { ...(src.character_skills?.skill_levels ?? {}) },
  },
  masteries: { picked: [...(src.masteries?.picked ?? [])] },
  critical_rate: { ...src.critical_rate },
  soul_link: { ...src.soul_link },
  // 旧い保存データ(欄が無い)は中立値で埋める。IndexedDB は素の JSON を返すので
  // `undefined` のまま画面へ渡すとキャラタブが開けなくなる(browserStore の v8 と同じ扱い)
  lumina_corridor: { ...(src.lumina_corridor ?? NEUTRAL_LUMINA_CORRIDOR) },
});

/** ルミナの回廊の未習得状態。正は crates/domain/src/lumina_corridor.rs の `LuminaCorridor` */
export const NEUTRAL_LUMINA_CORRIDOR = {
  final_damage_level: 0,
  all_element_level: 0,
  damage_reduction_level: 0,
  hp_mp_sp_level: 0,
} as const;

export const buildDraft = (c: RegisteredCharacter): Draft => ({
  name: c.name,
  gameCharacterId: c.game_character_id,
  baseStats: { ...c.base_stats },
  stage: String(c.awakening.stage),
  eternalLevel: String(c.awakening.eternal_level),
  statSources: cloneStatSources(c.stat_sources),
  equipment: cloneEquipment(c.equipment),
  commonSkills: cloneCommonSkills(c.common_skills),
  mainSkillId: c.main_skill_id ?? "",
  summonSkillId: c.summon_skill_id ?? "",
  goalContentId: c.goal_content_id ?? "",
  defaultBuffSetId: c.default_buff_set_id,
});

/** Draft → コマンドに渡すペイロード(保存・保存前プレビューの両方で使う) */
export const draftToPayload = (draft: Draft): NewCharacter => ({
  name: draft.name.trim(),
  game_character_id: draft.gameCharacterId,
  base_stats: { ...draft.baseStats },
  awakening: { stage: Number(draft.stage), eternal_level: Number(draft.eternalLevel) },
  stat_sources: cloneStatSources(draft.statSources),
  equipment: cloneEquipment(draft.equipment),
  common_skills: cloneCommonSkills(draft.commonSkills),
  main_skill_id: draft.mainSkillId === "" ? null : draft.mainSkillId,
  summon_skill_id: draft.summonSkillId === "" ? null : draft.summonSkillId,
  goal_content_id: draft.goalContentId === "" ? null : draft.goalContentId,
  default_buff_set_id: draft.defaultBuffSetId,
});
