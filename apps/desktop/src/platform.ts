/**
 * どちらの版で動いているか。実体は vite の alias で切り替える(ブラウザ版は src/web/platform.ts)。
 * 実行時に Tauri がいるかを探らないのは、ビルドで決まることを実行時に迷わせないため。
 */
export const IS_DESKTOP = true;
