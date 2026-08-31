/**
 * コマンド呼び出しの実体。中身はビルド設定の alias が決める
 * (vite.config.ts → invoke.tauri.ts / vite.web.config.ts → invoke.wasm.ts)。
 *
 * ここに置いてある再エクスポートは型解決とエディタ用。実行時分岐にしないのは、
 * デスクトップ版に WASM を、Web 版に Tauri の IPC を積みたくないため。
 */
export { invoke } from "./invoke.tauri";
