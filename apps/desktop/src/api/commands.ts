// Tauri コマンドの呼び出し。引数・戻り値の形は api/types.ts に従う。
import { invoke } from "./invoke";
import type {
  Adjustments, AppInfo, Awakening, BaseStats, BuffDamageSummary, BuffDefinition, BuffSelection, BuffSet, BuffTargetStatGain, CharacterSkillDef, CharacterSkillEffectsView, CharacterIcon, ComboSkillType, CommonSkills, DamageResult, DamageSnapshot, Element, ElementValues, Enemy, Equipment, EquipmentAbilityDef, EquipmentItem, GameCharacter, StartupNotice,
  Masteries, NewCharacter, RegisteredCharacter, ContentArea, ContentEvaluation, DefenseProfile,
  ElementPreview, ElementSourceDef, MasteryDef, RandomOptionDef, SienaCatalog, Skill, StatLimits,
  StatPreview, StatSources,
  TitleDef, UpgradeCandidate, EnchantGain, ValidationLocation, VersusAccuracy,
} from "./types";

export const listGameCharacters = () => invoke<GameCharacter[]>("list_game_characters");
export const listSkills = (gameCharacterId: string) => invoke<Skill[]>("list_skills", { gameCharacterId });
export const listEnemies = () => invoke<Enemy[]>("list_enemies");
export const listBuffCatalog = () => invoke<BuffDefinition[]>("list_buff_catalog");
export const summarizeBuffSelection = (buffs: BuffSelection) =>
  invoke<BuffDamageSummary>("summarize_buff_selection", { buffs });
export const listBuffSets = () => invoke<BuffSet[]>("list_buff_sets");
export const createBuffSet = (name: string, choices: BuffSelection) => invoke<BuffSet>("create_buff_set", { name, choices });
export const updateBuffSet = (id: number, name: string, choices: BuffSelection) => invoke<BuffSet>("update_buff_set", { id, name, choices });
export const duplicateBuffSet = (id: number) => invoke<BuffSet>("duplicate_buff_set", { id });
export const deleteBuffSet = (id: number) => invoke<void>("delete_buff_set", { id });
/** 「対象ステを選ぶ」バフの、このキャラでのステごとの効き。並べ方は呼び出し側で決める */
export const buffTargetStatGains = (
  character: NewCharacter,
  buffs: BuffSelection,
  buffId: string,
) =>
  invoke<BuffTargetStatGain[]>("buff_target_stat_gains", {
    baseStats: character.base_stats,
    statSources: character.stat_sources,
    buffs,
    equipment: character.equipment,
    commonSkills: character.common_skills,
    awakening: character.awakening,
    buffId,
  });
export const setDefaultBuffSet = (characterId: number, buffSetId: number | null) =>
  invoke<RegisteredCharacter>("set_default_buff_set", { characterId, buffSetId });
/** 属性値の供給源カタログ(装備の属性強化以外) */
export const listElementSources = () => invoke<ElementSourceDef[]>("list_element_sources");
/** 属性値の内訳(キャラ基礎 / 装備 / 供給源 / 合計)。保存前のキャラデータで出す */
export const previewElements = (character: NewCharacter) =>
  invoke<ElementPreview>("preview_elements", { character });
/** 装備の属性強化の合計(部位ごとに +9)。対象属性は呼び出し側が決める */
export const equipmentElementValues = (equipment: Equipment, element: Element | null) =>
  invoke<ElementValues>("equipment_element_values", { equipment, element });
export const listCharacters = () => invoke<RegisteredCharacter[]>("list_characters");
export const createCharacter = (character: NewCharacter) =>
  invoke<RegisteredCharacter>("create_character", { character });
export const updateCharacter = (id: number, character: NewCharacter) =>
  invoke<RegisteredCharacter>("update_character", { id, character });
export const deleteCharacter = (id: number) => invoke<void>("delete_character", { id });
export const listCharacterIcons = () => invoke<CharacterIcon[]>("list_character_icons");
export const setCharacterIcon = (characterId: number, source: Uint8Array) =>
  invoke<CharacterIcon>("set_character_icon", { characterId, source: Array.from(source) });
export const resetCharacterIcon = (characterId: number) =>
  invoke<void>("reset_character_icon", { characterId });
export const getDamageSnapshot = (characterId: number) =>
  invoke<DamageSnapshot | null>("get_damage_snapshot", { characterId });
export const setDamageSnapshot = (characterId: number, skillId: string, contentId: string, perHit: number) =>
  invoke<DamageSnapshot>("set_damage_snapshot", { characterId, skillId, contentId, perHit });
/**
 * 保存しない試算。draft の base_stats/stat_sources/equipment から最終能力値と寄与内訳を得る。
 * `mainSkillId`(主軸スキル)を渡すとその依存種別で攻撃力(A)も返る。null なら攻撃力は出ない。
 */
export const previewEffectiveStats = (
  baseStats: BaseStats, statSources: StatSources, equipment: Equipment, commonSkills: CommonSkills,
  awakening: Awakening, mainSkillId: string | null, buffs: BuffSelection = { choices: [] },
) => invoke<StatPreview>("preview_effective_stats", {
  baseStats, statSources, equipment, commonSkills, awakening, mainSkillId, buffs,
});
export const calculateDamage = (
  characterId: number, skillId: string, contentId: string, comboCount: number, temporaryAdjustments: Adjustments,
  comboSkillType: ComboSkillType | null = null, buffs: BuffSelection = { choices: [] },
  normalAttackId: string | null = null,
) => invoke<DamageResult>("calculate_damage", { characterId, skillId, contentId, comboCount, comboSkillType, normalAttackId, temporaryAdjustments, buffs });
export const getStatLimits = () => invoke<StatLimits>("get_stat_limits");
/** Rust domain が定める新規キャラ用の未開放・未習得状態。 */
export const getNewCharacterStatSources = () =>
  invoke<StatSources>("get_new_character_stat_sources");
/** 防御側の戦闘能力値(docs/damage-formula.md §6〜7)。対象コンテンツに依らない */
export const previewDefense = (character: NewCharacter, buffs: BuffSelection = { choices: [] }) =>
  invoke<DefenseProfile>("preview_defense", { character, buffs });
/** 対人の命中率(wiki#AccuracyPoint / #EvasionPoint / #HitRate)。保存前のキャラデータで出す */
export const previewVersus = (
  attacker: NewCharacter, attackerBuffs: BuffSelection, skillId: string,
  defender: NewCharacter, defenderBuffs: BuffSelection,
) =>
  invoke<VersusAccuracy>("preview_versus", {
    attacker, attackerBuffs, skillId, defender, defenderBuffs,
  });
export const listEquipmentCatalog = () => invoke<EquipmentItem[]>("list_equipment_catalog");
export const listEquipmentAbilities = () => invoke<EquipmentAbilityDef[]>("list_equipment_abilities");
/** ランダムオプションのカタログ(wiki: ランダムオプション) */
export const listRandomOptions = () => invoke<RandomOptionDef[]>("list_random_options");
/** マスタリーのカタログ(wiki: 各キャラの Skill ページ。段ごとに 1 つ選ぶ) */
export const listMasteries = () => invoke<MasteryDef[]>("list_masteries");
/** シエナのオーラで選べる能力値・追加オプションのカタログ(wiki: 装備システム/シエナのオーラ) */
export const listSienaKinds = () => invoke<SienaCatalog>("list_siena_kinds");
/** 称号のカタログ(wiki: 称号システム。主要称号のみ) */
export const listTitles = () => invoke<TitleDef[]>("list_titles");
/** キャラスキルのカタログ(パッシブ・自己バフ・味方バフ)。味方スキルは誰でも ON にできる */
export const listCharacterSkills = () =>
  invoke<CharacterSkillDef[]>("list_character_skills");
/** キャラスキル全件ぶんの、選んでいるマスタリーを踏まえた実際の効果(マスタリー解決は Rust 側) */
export const resolveCharacterSkillEffects = (masteries: Masteries) =>
  invoke<CharacterSkillEffectsView[]>("resolve_character_skill_effects", { masteries });

/** 情報パネルに出すアプリ情報(版・保存先) */
export const getAppInfo = () => invoke<AppInfo>("get_app_info");
/** 起動時に復元などが起きたときだけ返る。通常起動は null */
export const getStartupNotice = () => invoke<StartupNotice | null>("get_startup_notice");

/** invoke の reject(String)を表示用文字列にする */
/** Tauri コマンドが返すエラー(src-tauri の CommandError)。`location` 付きなら帯からそこへ飛べる。 */
type CommandError = { message: string; location: ValidationLocation | null };

const asCommandError = (e: unknown): CommandError | null =>
  typeof e === "object" && e !== null && "message" in e && "location" in e ? (e as CommandError) : null;

export function errorMessage(e: unknown): string {
  const command = asCommandError(e);
  if (command) return command.message;
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** エラーが指している装備の場所。無ければ null(帯に「ここを開く」は出さない)。 */
export function errorLocation(e: unknown): ValidationLocation | null {
  return asCommandError(e)?.location ?? null;
}

export const listContents = () => invoke<ContentArea[]>("list_contents");
/** 保存前のキャラデータ(編集中 draft・試し変更)でダメージ計算する。DB には書き込まない */
/** `normalAttackId` はコンボで間に挟む通常攻撃。渡すと DPS が「通常攻撃 → スキル」の 1 サイクルになる */
export const previewDamage = (
  character: NewCharacter, skillId: string, contentId: string, comboCount: number,
  temporaryAdjustments: Adjustments | null = null,
  comboSkillType: ComboSkillType | null = null,
  buffs: BuffSelection = { choices: [] },
  normalAttackId: string | null = null,
) => invoke<DamageResult>("preview_damage", { character, skillId, contentId, comboCount, comboSkillType, normalAttackId, temporaryAdjustments, buffs });
/**
 * 全コンテンツの到達判定(火力は最大ダメージのスキル・コンボなしで評価)。
 * `dependencySkillId` を渡すと、装備条件(スキル依存で比較先が変わる)をそのスキルで判定する。
 */
export const evaluateContents = (character: NewCharacter, dependencySkillId?: string, buffs: BuffSelection = { choices: [] }) =>
  invoke<ContentEvaluation[]>("evaluate_contents", {
    character,
    dependencySkillId: dependencySkillId ?? null,
    buffs,
  });
/**
 * 「次に変えるなら / おすすめ強化」候補を 1 回の IPC でまとめて試算する。
 * 列挙・並び順(届かせるなら正直に・+0 除外)は Rust 側(domain::candidate)。
 */
export const listUpgradeCandidates = (
  character: NewCharacter, skillId: string, contentId: string, comboCount = 0,
  comboSkillType: ComboSkillType | null = null,
  temporaryAdjustments: Adjustments | null = null,
  buffs: BuffSelection = { choices: [] },
) => invoke<UpgradeCandidate[]>("list_upgrade_candidates", {
  character, buffs, skillId, contentId, comboCount, comboSkillType, temporaryAdjustments,
});
/**
 * 「エンチャントの伸びしろ」(部位×ステごとの MAX 試算)。選択中スキルの依存ステだけに絞り、
 * 伸び率(delta_pct)は rank_candidates と同じ式・丸めで Rust 側が返す(フロントで割り算しない)。
 */
export const listEnchantGains = (
  character: NewCharacter, skillId: string, contentId: string, comboCount = 0,
  comboSkillType: ComboSkillType | null = null,
  temporaryAdjustments: Adjustments | null = null,
  buffs: BuffSelection = { choices: [] },
) => invoke<EnchantGain[]>("list_enchant_gains", {
  character, buffs, skillId, contentId, comboCount, comboSkillType, temporaryAdjustments,
});
