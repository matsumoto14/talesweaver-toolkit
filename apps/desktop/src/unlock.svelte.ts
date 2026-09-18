// 一部機能のロック(対人タブ・インクリタブ・追加装備)。解除は情報パネルのバージョン表記を続けて押す
// ジェスチャーで、合言葉や鍵は持たない(ユーザー決定 2026-09-03: 秘匿ではなく「見せない・使わせない」)。
// 解除状態はこの PC の localStorage にだけ残す(キャラデータの書き出しには含めない)。
//
// 追加装備の数値(2026-09-14 決定)は配布物・リポジトリに同梱せず、解除操作のたびに R2 から
// 取得してこの起動中のカタログへ合流させる(`fetchLockedEquipment`)。アイコン画像は同梱のまま
// 据え置く(ADR-009「秘匿ではなく見せない・使わせない」に沿う。画像は外さない)。

import { fetch } from "@tauri-apps/plugin-http";
import {
  installDownloadedEquipment,
  listDownloadedEquipmentIds,
  listEquipmentCatalog,
} from "./api/commands";
import { app } from "./state.svelte";

const STORAGE_KEY = "tw-v4-unlocked";
/** バージョン表記をこの回数続けて押すと切り替わる */
export const UNLOCK_TAPS = 7;
/** 押す間隔がこれを超えたら数え直す(ms) */
export const UNLOCK_TAP_WINDOW_MS = 1500;

function read(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export const unlock = $state({ on: read() });

export function setUnlocked(on: boolean): void {
  unlock.on = on;
  try {
    if (on) localStorage.setItem(STORAGE_KEY, "1");
    else localStorage.removeItem(STORAGE_KEY);
  } catch {
    // private モード等で書けなければ、この起動中だけ有効
  }
}

/**
 * R2 から合流させた追加装備の id。ロック中に候補から外す判定はこの集合で行う
 * (カタログはロックの概念を持たない。名前で判定しない)。起動時と取得のたびに入れ直す。
 */
const downloadedIds = $state({ set: new Set<string>() });

export async function refreshDownloadedEquipmentIds(): Promise<void> {
  downloadedIds.set = new Set(await listDownloadedEquipmentIds());
}

/** ロック中は候補に出さない装備 */
export function isLockedEquipment(item: { id: string }): boolean {
  return !unlock.on && downloadedIds.set.has(item.id);
}

/** ロック中は出さないタブ */
const LOCKED_TABS = new Set(["versus", "inkri"]);

export function isLockedTab(tab: string): boolean {
  return !unlock.on && LOCKED_TABS.has(tab);
}

/** 配信元。CORS を要求しないよう Rust 側(plugin-http)から取る(news.ts と同じ理由) */
const EXTRA_EQUIPMENT_ENDPOINT = "https://dl.tw-context.dev/data/extra-equipment.json";

/**
 * 追加装備を R2 から取り直し、Rust 側でカタログへ合流させてローカル保存する。
 * 成功したら装備カタログを入れ直して呼び出し側へ件数を返す。取得・保存に失敗したら投げる
 * (お知らせと違って「解除したのに増えていない」を黙って隠さない)。
 */
export async function fetchLockedEquipment(): Promise<number> {
  const response = await fetch(EXTRA_EQUIPMENT_ENDPOINT, { cache: "no-cache" });
  if (!response.ok) throw new Error(`追加装備の取得に失敗しました(${response.status})`);
  const json = await response.text();
  const count = await installDownloadedEquipment(json);
  await refreshDownloadedEquipmentIds();
  app.equipmentCatalog = await listEquipmentCatalog();
  return count;
}
