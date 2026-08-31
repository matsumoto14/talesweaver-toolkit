/**
 * ブラウザ版の更新(`@tauri-apps/plugin-updater` の差し替え。alias は vite.web.config.ts)。
 *
 * ブラウザ版に自動更新は無い(読み込み直せば配信中の版になる)。「新しい版は無い」を返し、
 * お知らせタブは「最新」のまま静かにしておく。
 */
export interface Update {
  version: string;
  body?: string;
  downloadAndInstall(onEvent: (event: unknown) => void): Promise<void>;
}

export async function check(): Promise<Update | null> {
  return null;
}
