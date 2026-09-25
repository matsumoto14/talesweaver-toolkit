// 対人タブのロック。解除は情報パネルのバージョン表記を続けて押すジェスチャーで、合言葉や鍵は
// 持たない(ユーザー決定 2026-09-03: 秘匿ではなく「見せない・使わせない」)。
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

/** ロック中は出さないタブ */
const LOCKED_TABS = new Set(["versus"]);

export function isLockedTab(tab: string): boolean {
  return !unlock.on && LOCKED_TABS.has(tab);
}
