// アプリ全体で共有する状態(タブ・カタログ・登録キャラ・選択・コンテンツ判定・試し変更)。
// モジュールスコープ $state(toast.svelte.ts と同じパターン)。$effect はここでは使わない
// (effect_orphan になるため。ロードは App.svelte の onMount から呼ぶ)。
import {
  errorMessage,
  evaluateContents,
  listBuffCatalog,
  listCharacters,
  listContents,
  listEquipmentAbilities,
  listEquipmentCatalog,
  listGameCharacters,
} from "./api/commands";
import type {
  BuffDefinition,
  Content,
  ContentArea,
  ContentEvaluation,
  EquipmentAbilityDef,
  EquipmentItem,
  GameCharacter,
  NewCharacter,
  RegisteredCharacter,
} from "./api/types";
import { reportError } from "./toast.svelte";

export type Tab = "home" | "calc" | "chars";

export const app = $state({
  tab: "home" as Tab,
  loading: true,
  characters: [] as RegisteredCharacter[],
  gameCharacters: [] as GameCharacter[],
  areas: [] as ContentArea[],
  catalog: [] as BuffDefinition[],
  equipmentCatalog: [] as EquipmentItem[],
  equipmentAbilities: [] as EquipmentAbilityDef[],
  selectedId: null as number | null,
  /** キャラ id → コンテンツ判定(保存済みデータ基準) */
  evaluations: {} as Record<number, ContentEvaluation[]>,
  /** ダメージ計算タブの試し変更(保存されない)。null = 登録どおり */
  sim: null as NewCharacter | null,
  /** ダメージ計算タブの対象コンテンツ。null = 未選択(先頭にフォールバック) */
  calcTargetId: null as string | null,
  /** ホームから計算タブへ渡すスキル(判定に使った最大ダメージのスキル)。使用後に null へ戻す */
  calcSkillId: null as string | null,
  /** キャラタブで登録ペインを開く(レールの「＋ キャラを登録」から) */
  registerOpen: false,
});

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

/** コンテンツの平坦リスト(エリア名付き、表示順) */
export function flatContents(): { areaId: string; areaName: string; content: Content }[] {
  return app.areas.flatMap((a) =>
    a.contents.map((content) => ({ areaId: a.id, areaName: a.name, content })),
  );
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

// キャラ id ごとの最新リクエスト番号(非リアクティブ)。古い応答を捨てるためのガード。
const evaluationSeqs: Record<number, number> = {};

export async function refreshEvaluation(c: RegisteredCharacter): Promise<void> {
  const seq = (evaluationSeqs[c.id] = (evaluationSeqs[c.id] ?? 0) + 1);
  try {
    const evaluations = await evaluateContents(payloadOf(c));
    // 古い応答、および削除済みキャラの応答は反映しない(削除済みキーの復活防止)
    if (evaluationSeqs[c.id] === seq && app.characters.some((x) => x.id === c.id)) {
      app.evaluations[c.id] = evaluations;
    }
  } catch (e) {
    reportError(errorMessage(e));
  }
}

export function selectCharacter(id: number | null): void {
  if (app.selectedId === id) return;
  app.selectedId = id;
  // 試し変更はキャラに紐づく。切替時に破棄(前キャラの変更を引き継がない)
  app.sim = null;
}

export async function loadAll(): Promise<void> {
  try {
    const [characters, gameCharacters, areas, catalog, equipmentCatalog, equipmentAbilities] = await Promise.all([
      listCharacters(),
      listGameCharacters(),
      listContents(),
      listBuffCatalog(),
      listEquipmentCatalog(),
      listEquipmentAbilities(),
    ]);
    app.characters = characters;
    app.gameCharacters = gameCharacters;
    app.areas = areas;
    app.catalog = catalog;
    app.equipmentCatalog = equipmentCatalog;
    app.equipmentAbilities = equipmentAbilities;
    if (app.selectedId === null && characters.length > 0) app.selectedId = characters[0].id;
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
