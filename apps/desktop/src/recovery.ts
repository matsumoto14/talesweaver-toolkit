/**
 * 保存先が開けなくなったときの復旧。実体は vite の alias で切り替える(ブラウザ版は src/web/recovery.ts)。
 *
 * デスクトップ版の保存先は SQLite で、起動不能にしない手当ては storage 側(`open_with_backup` が
 * 移行前に `.bak.<版>` を取る)にあるので、画面からデータを消す口は出さない。
 */
export const CAN_RESET_STORE = false;

export async function resetStore(): Promise<void> {}
