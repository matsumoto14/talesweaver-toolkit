/**
 * ブラウザ版のプロセス操作(`@tauri-apps/plugin-process` の差し替え。alias は vite.web.config.ts)。
 * 更新が無いので呼ばれることは無いが、ブラウザでの「再起動」は読み込み直しに当たる。
 */
export async function relaunch(): Promise<void> {
  window.location.reload();
}
