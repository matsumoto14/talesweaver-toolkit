/**
 * ブラウザ版の外部リンク(`@tauri-apps/plugin-opener` の差し替え。alias は vite.web.config.ts)。
 * ブラウザなら OS に頼まなくても自分で開ける。元のタブを触らせないよう noopener を付ける。
 */
export async function openUrl(url: string): Promise<void> {
  window.open(url, "_blank", "noopener,noreferrer");
}
