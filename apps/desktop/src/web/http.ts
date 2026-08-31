/**
 * ブラウザ版の HTTP(`@tauri-apps/plugin-http` の差し替え。alias は vite.web.config.ts)。
 *
 * デスクトップ版が Rust 経由で取りに行くのは配信元に CORS 設定を要求しないためだが、
 * ブラウザ版にその逃げ道は無いので素の fetch を使う。取れなければ呼び出し側(news.ts)が
 * 同梱ぶんに落ちるので、お知らせが白紙になることはない。
 */
export const fetch = globalThis.fetch.bind(globalThis);
