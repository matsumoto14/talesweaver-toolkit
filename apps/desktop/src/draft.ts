// キャラ編集で共有する編集中ドラフトの型と組み立て関数(キャラタブと登録ペインで使用)。
import type {
  Adjustments,
  BaseStats,
  CommonSkills,
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
  /** 共通スキル(wiki: Skill/共通) */
  commonSkills: CommonSkills;
  /** 主軸スキル(攻撃力の依存種別を決める)。"" = 未選択 */
  mainSkillId: string;
}

export const cloneEquipment = (src: Equipment): Equipment => ({
  parts: Object.fromEntries(
    PART_SLOTS.map((slot) => [slot, cloneEquipmentPart(src.parts[slot])]),
  ) as unknown as EquipmentParts,
  thesis_cores: cloneThesisCores(src.thesis_cores),
  title: src.title ?? null,
});

/** 新規登録キャラの装備の初期値(全部位 未装備)。 */
export const defaultEquipment = (): Equipment => ({
  parts: Object.fromEntries(PART_SLOTS.map((slot) => [slot, neutralEquipmentPart()])) as unknown as EquipmentParts,
  thesis_cores: neutralThesisCores(),
  title: null,
});

/** ストロングウェポンの既定 Lv(上限。wiki Skill/共通: Lv6 = +18%) */
const DEFAULT_STRONG_WEAPON_LEVEL = 6;

export const cloneCommonSkills = (src: CommonSkills): CommonSkills => ({
  ...src,
  ultimate: { ...src.ultimate, slots: [...src.ultimate.slots] },
});

/**
 * 新規登録キャラの共通スキルの初期値。パワーウェポン ON・ストロングウェポン Lv6(合計 +20%)を
 * 既定にする(2026-08-24 決定2: 値は人によって変わるが、取っていないユーザーはほぼいない)。
 * ストロングウェポン Lv6 にはオーグメント Lv5 が要る(wiki Skill/共通)ので合わせて入れる。
 * **保存済みキャラの値は書き換えない**ので、ここを使うのは新規登録だけにすること。
 */
export const defaultCommonSkills = (): CommonSkills => ({
  power_weapon: true,
  strong_weapon_level: DEFAULT_STRONG_WEAPON_LEVEL,
  coat_armor: false,
  protect_armor_level: 0,
  kai_protect_armor_level: 0,
  sharpness_vision_level: 0,
  augment_level: DEFAULT_STRONG_WEAPON_LEVEL - 1,
  ultimate: { slots: [null, null], super_limit: false, hyper_limit_level: 0 },
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
  elements: { ...src.elements },
});

export const neutralStatSources = (): StatSources => ({
  pet_skills: Object.fromEntries(STAT_KINDS.map((k) => [k, null])) as StatSources["pet_skills"],
  rune_levels: Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as StatSources["rune_levels"],
  crown: Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as StatSources["crown"],
  sacred_relic: Object.fromEntries(STAT_KINDS.map((k) => [k, 0])) as StatSources["sacred_relic"],
  buffs: { choices: [] },
  adjustments: Object.fromEntries(STAT_KINDS.map((k) => [k, { add: 0, pin: null }])) as StatSources["adjustments"],
  elements: { pet: null, monster_card: null, rune: null, helm_ability: null, cuffs_ability: null },
});

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
});
