// アプリ全体で共有する状態(タブ・カタログ・登録キャラ・選択・コンテンツ判定・試し変更)。
// モジュールスコープ $state(toast.svelte.ts と同じパターン)。$effect はここでは使わない
// (effect_orphan になるため。ロードは App.svelte の onMount から呼ぶ)。
import {
  errorLocation,
  errorMessage,
  evaluateContents,
  listBuffCatalog,
  listBuffSets,
  listCharacters,
  listContents,
  listElementSources,
  listEquipmentAbilities,
  listEquipmentCatalog,
  listGameCharacters,
  listRandomOptions,
  listMasteries,
  listSienaKinds,
  listCharacterSkills,
  listSkills,
  listTitles,
} from "./api/commands";
import { latestByKey } from "./ui/latest.svelte";
import type {
  CharacterSkillDef,
  BuffDefinition,
  BuffSelection,
  BuffSet,
  ElementSourceDef,
  Content,
  ContentArea,
  ContentEvaluation,
  EquipmentAbilityDef,
  EquipmentItem,
  GameCharacter,
  NewCharacter,
  RandomOptionDef,
  MasteryDef,
  RegisteredCharacter,
  SienaCatalog,
  Skill,
  TitleDef,
  ValidationLocation,
} from "./api/types";
import { reportError, type ErrorTarget } from "./toast.svelte";
import type { SourceId } from "./pages/chars/sourceId";

export type Tab = "home" | "calc" | "buffs" | "chars";

export const app = $state({
  tab: "home" as Tab,
  loading: true,
  characters: [] as RegisteredCharacter[],
  gameCharacters: [] as GameCharacter[],
  areas: [] as ContentArea[],
  catalog: [] as BuffDefinition[],
  buffSets: [] as BuffSet[],
  equipmentCatalog: [] as EquipmentItem[],
  equipmentAbilities: [] as EquipmentAbilityDef[],
  /** ランダムオプションのカタログ(wiki: ランダムオプション) */
  randomOptions: [] as RandomOptionDef[],
  /** シエナのオーラで選べる能力値・追加オプション(wiki: 装備システム/シエナのオーラ) */
  siena: { values: [], extras: [], extra_unlock_stages: [3, 7, 10], stage_max: 10 } as SienaCatalog,
  /** 称号のカタログ(主要称号のみ) */
  titles: [] as TitleDef[],
  /** 中ディレイ減少スキル(wiki: ステータス「中ディレイ倍率B」)。キャラ固有のパッシブのみ */
  characterSkills: [] as CharacterSkillDef[],
  /** マスタリー(wiki: 各キャラの Skill ページ。段ごとに 1 つ選ぶ) */
  masteries: [] as MasteryDef[],
  /** 属性値の供給源カタログ(装備の属性強化以外) */
  elementSources: [] as ElementSourceDef[],
  selectedId: null as number | null,
  /** キャラ id → コンテンツ判定(保存済みデータ基準) */
  evaluations: {} as Record<number, ContentEvaluation[]>,
  /** ダメージ計算タブの試し変更(保存されない)。null = 登録どおり */
  sim: null as NewCharacter | null,
  /** ダメージ計算タブの対象コンテンツ。null = 未選択(先頭にフォールバック) */
  calcTargetId: null as string | null,
  /** 計算で使うセットと、その計算だけの追加・除外を含む選択。 */
  calcBuffSetId: null as number | null,
  calcBuffs: { choices: [] } as BuffSelection,
  /** キャラタブで登録ペインを開く(レールの「＋ キャラを登録」から) */
  registerOpen: false,
});

/**
 * ゲームキャラ id → スキル一覧。静的データなので一度引いたら使い回す。
 * キャラ画面(主軸スキルの選択肢)で参照する。未取得のキーは空配列を返す。
 */
export const skillsByCharacter = $state<Record<string, Skill[]>>({});

// 取得済み・取得中のキー(非リアクティブ)。$effect から呼ぶので、重複判定に
// skillsByCharacter 自身を読むと「書いた瞬間に呼び出し元の effect が再実行される」ため分ける。
const requestedSkills = new Set<string>();

export async function loadSkills(gameCharacterId: string): Promise<void> {
  if (gameCharacterId === "" || requestedSkills.has(gameCharacterId)) return;
  requestedSkills.add(gameCharacterId);
  try {
    skillsByCharacter[gameCharacterId] = await listSkills(gameCharacterId);
  } catch (e) {
    requestedSkills.delete(gameCharacterId);
    reportError(errorMessage(e));
  }
}

/**
 * エラー帯の「ここを開く」が置く「開いてほしい装備の場所」。
 * 装備ペイン / ランダムOPペインが読んで開き、読んだら null に戻す。
 * `seq` は同じ場所を続けて指されても動きが出るようにするための連番。
 */
export const equipmentFocus = $state<{ request: (ValidationLocation & { seq: number }) | null }>({
  request: null,
});
/** 別タブの案内から、キャラ画面の特定の補正源を直接開くための要求。 */
export const characterSourceFocus = $state<{ request: { sourceId: SourceId; seq: number } | null }>({
  request: null,
});
let focusSeq = 0;

export function focusCharacterSource(sourceId: SourceId) {
  app.tab = "chars";
  app.registerOpen = false;
  focusSeq += 1;
  characterSourceFocus.request = { sourceId, seq: focusSeq };
}

/** エラーが指す場所へ画面を移す(キャラタブ → 該当キャラ → 該当部位)。 */
export function focusErrorTarget(target: ErrorTarget) {
  app.tab = "chars";
  app.registerOpen = false;
  app.selectedId = target.characterId;
  focusSeq += 1;
  equipmentFocus.request = { ...target.location, seq: focusSeq };
}

export function selectedCharacter(): RegisteredCharacter | null {
  return app.characters.find((c) => c.id === app.selectedId) ?? null;
}

export function gameCharacterName(id: string): string {
  return app.gameCharacters.find((g) => g.id === id)?.name ?? id;
}

/** RegisteredCharacter → コマンドに渡す保存前ペイロード(ディープコピー) */
export function payloadOf(c: RegisteredCharacter): NewCharacter {
  const { id: _id, ...rest } = c;
  return JSON.parse(JSON.stringify(rest)) as NewCharacter;
}

export function buffSelectionFor(c: RegisteredCharacter): BuffSelection {
  const choices = app.buffSets.find((set) => set.id === c.default_buff_set_id)?.choices ?? { choices: [] };
  return JSON.parse(JSON.stringify(choices)) as BuffSelection;
}

export function syncCalcBuffs(c: RegisteredCharacter | null): void {
  app.calcBuffSetId = c?.default_buff_set_id ?? null;
  app.calcBuffs = c ? buffSelectionFor(c) : { choices: [] };
}

/** コンテンツの平坦リスト(エリア名付き、表示順) */
export function flatContents(): { areaId: string; areaName: string; content: Content }[] {
  return app.areas.flatMap((a) =>
    a.contents.map((content) => ({ areaId: a.id, areaName: a.name, content })),
  );
}

/**
 * 全コンテンツ数(エリア横断)。「クリアできるのは N / 全体」系の分母を
 * ここ 1 箇所に集約する(キャラレール・ホーム・キャラワークスペース・計算タブで共通)。
 */
export function totalContents(): number {
  return app.areas.reduce((n, a) => n + a.contents.length, 0);
}

export function findContent(contentId: string): Content | null {
  for (const a of app.areas) {
    const c = a.contents.find((x) => x.id === contentId);
    if (c) return c;
  }
  return null;
}

export function evaluationFor(characterId: number, contentId: string): ContentEvaluation | null {
  return app.evaluations[characterId]?.find((e) => e.content_id === contentId) ?? null;
}

// キャラ id ごとの最新リクエストだけを反映するレースガード。古い応答を捨てる。
const evaluationLatest = latestByKey<number>();

export async function refreshEvaluation(c: RegisteredCharacter): Promise<void> {
  await evaluationLatest.run(c.id, async (isCurrent) => {
    try {
      const evaluations = await evaluateContents(payloadOf(c), undefined, buffSelectionFor(c));
      // 古い応答、および削除済みキャラの応答は反映しない(削除済みキーの復活防止)
      if (isCurrent() && app.characters.some((x) => x.id === c.id)) {
        app.evaluations[c.id] = evaluations;
      }
    } catch (e) {
      // どこの話か分かるエラーは帯から飛べるようにする(判定はキャラ単位なので id は分かっている)
      const location = errorLocation(e);
      reportError(errorMessage(e), location ? { characterId: c.id, location } : null);
    }
  });
}

export function selectCharacter(id: number | null): void {
  if (app.selectedId === id) return;
  app.selectedId = id;
  // 試し変更はキャラに紐づく。切替時に破棄(前キャラの変更を引き継がない)
  app.sim = null;
  syncCalcBuffs(selectedCharacter());
}

export async function loadAll(): Promise<void> {
  try {
    const [
      characters, gameCharacters, areas, catalog, buffSets, equipmentCatalog, equipmentAbilities, elementSources,
      randomOptions, titles, characterSkills, siena, masteries,
    ] = await Promise.all([
      listCharacters(),
      listGameCharacters(),
      listContents(),
      listBuffCatalog(),
      listBuffSets(),
      listEquipmentCatalog(),
      listEquipmentAbilities(),
      listElementSources(),
      listRandomOptions(),
      listTitles(),
      listCharacterSkills(),
      listSienaKinds(),
      listMasteries(),
    ]);
    app.characters = characters;
    app.gameCharacters = gameCharacters;
    app.areas = areas;
    app.catalog = catalog;
    app.buffSets = buffSets;
    app.equipmentCatalog = equipmentCatalog;
    app.equipmentAbilities = equipmentAbilities;
    app.elementSources = elementSources;
    app.randomOptions = randomOptions;
    app.titles = titles;
    app.characterSkills = characterSkills;
    app.siena = siena;
    app.masteries = masteries;
    if (app.selectedId === null && characters.length > 0) app.selectedId = characters[0].id;
    syncCalcBuffs(selectedCharacter());
    await Promise.all(characters.map(refreshEvaluation));
  } catch (e) {
    reportError(errorMessage(e));
  } finally {
    app.loading = false;
  }
}

/** 登録・更新後の反映(コンテンツ判定も更新する) */
export function upsertCharacter(c: RegisteredCharacter): void {
  const i = app.characters.findIndex((x) => x.id === c.id);
  if (i >= 0) app.characters[i] = c;
  else app.characters.push(c);
  // 選択中キャラが(どのタブからでも)保存されたら試し変更は破棄する。
  // sim は保存時点のスナップショットなので、残すと古い値で最新の保存を上書きできてしまう
  // (独立レビュー指摘: クロスタブのサイレントなデータ消失)。
  if (c.id === app.selectedId) app.sim = null;
  if (c.id === app.selectedId && app.calcBuffSetId !== c.default_buff_set_id) syncCalcBuffs(c);
  void refreshEvaluation(c);
}

export function removeCharacter(id: number): void {
  app.characters = app.characters.filter((c) => c.id !== id);
  delete app.evaluations[id];
  if (app.selectedId === id) {
    app.selectedId = app.characters[0]?.id ?? null;
    app.sim = null;
  }
}
