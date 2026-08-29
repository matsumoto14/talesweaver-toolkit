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
import { cloneEquipmentPart, cloneSienaAuras, cloneThesisCores, neutralEquipmentPart, neutralSienaAuras, neutralThesisCores } from "./equipment";
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
  defaultBuffSetId: number | null;
}

export const cloneEquipment = (src: Equipment): Equipment => ({
  parts: Object.fromEntries(
    PART_SLOTS.map((slot) => [slot, { selected_id: src.parts[slot].selected_id, registered: src.parts[slot].registered.map(cloneEquipmentPart) }]),
  ) as unknown as EquipmentParts,
  siena: cloneSienaAuras(src.siena),
  thesis_cores: cloneThesisCores(src.thesis_cores),
  title: src.title ?? null,
});

/** 新規登録キャラの装備の初期値(全部位 未装備)。 */
export const defaultEquipment = (): Equipment => ({
  parts: Object.fromEntries(PART_SLOTS.map((slot) => [slot, { registered: [], selected_id: null } satisfies EquipmentPartList])) as unknown as EquipmentParts,
  siena: neutralSienaAuras(),
  thesis_cores: neutralThesisCores(),
  title: null,
});

/** ストロングウェポンの既定 Lv(上限。wiki Skill/共通: Lv6 = +18%) */
const DEFAULT_STRONG_WEAPON_LEVEL = 6;
const DEFAULT_PROTECT_ARMOR_LEVEL = 6;
const DEFAULT_KAI_PROTECT_ARMOR_LEVEL = 5;
const DEFAULT_AUGMENT_LEVEL = 5;
const DEFAULT_REINFORCE_LEVEL = 5;
const DEFAULT_HYPER_LIMIT_LEVEL = 6;
const DEFAULT_SHARPNESS_VISION_LEVEL = 5;

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

/**
 * 新規登録キャラの共通スキルの初期値。パワーウェポン ON・ストロングウェポン Lv6(合計 +20%)を
 * 既定にする(2026-08-24 決定2: 値は人によって変わるが、取っていないユーザーはほぼいない)。
 * ストロングウェポン Lv6 にはオーグメント Lv5 が要る(wiki Skill/共通)ので合わせて入れる。
 * **保存済みキャラの値は書き換えない**ので、ここを使うのは新規登録だけにすること。
 */
export const defaultCommonSkills = (): CommonSkills => ({
  // 共通スキルは「ほぼ全員が取り切っている」前提で最大を入れる。ここを 0 にすると
  // 全員が毎回同じ値を入れ直すことになる(ux-guidelines「初期値は実用値」)。
  // 人によって違うのはオーグメント・極限スキル 2 枠・シャープネスビジョンだけ
  power_weapon: true,
  strong_weapon_level: DEFAULT_STRONG_WEAPON_LEVEL,
  coat_armor: true,
  protect_armor_level: DEFAULT_PROTECT_ARMOR_LEVEL,
  kai_protect_armor_level: DEFAULT_KAI_PROTECT_ARMOR_LEVEL,
  // Lv5 までは自然に上がる(ここで止まる人が多い)。Lv6 以降は習得スクロールが要るので人による
  sharpness_vision_level: DEFAULT_SHARPNESS_VISION_LEVEL,
  augment_level: DEFAULT_AUGMENT_LEVEL,
  unleash: [{ stat: null, level: 0 }, { stat: null, level: 0 }],
  // アンリーシュ Lv10 の前提。ステを選べば Lv は上限で入る
  reinforce_level: DEFAULT_REINFORCE_LEVEL,
  ultimate: { slots: [null, null], super_limit: true, hyper_limit_level: DEFAULT_HYPER_LIMIT_LEVEL },
});

export const cloneStatSources = (src: StatSources): StatSources => ({
  pet_skills: { ...src.pet_skills },
  rune_levels: { ...src.rune_levels },
  crown: { ...src.crown },
  monster_cards: { ...(src.monster_cards ?? {}) } as StatSources["monster_cards"],
  sacred_relic: { ...src.sacred_relic },
  elements: { ...src.elements },
  character_skills: { skill_ids: [...(src.character_skills?.skill_ids ?? [])] },
  masteries: { picked: [...(src.masteries?.picked ?? [])] },
  critical_rate: { ...src.critical_rate },
  soul_link: { ...src.soul_link },
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
  default_buff_set_id: draft.defaultBuffSetId,
});
