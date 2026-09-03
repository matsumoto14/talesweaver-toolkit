// 一部機能のロック(対人タブ・テネブリス装備)。解除は情報パネルのバージョン表記を続けて押す
// ジェスチャーで、合言葉や鍵は持たない(ユーザー決定 2026-09-03: 秘匿ではなく「見せない・使わせない」)。
// 解除状態はこの PC の localStorage にだけ残す(キャラデータの書き出しには含めない)。

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

/** ロック中は候補に出さない装備。名前で判定する(カタログはロックの概念を持たない) */
export function isLockedEquipment(item: { name: string }): boolean {
  return !unlock.on && item.name.startsWith("†テネブリス");
}

/** ロック中は出さないタブ */
export function isLockedTab(tab: string): boolean {
  return !unlock.on && tab === "versus";
}
