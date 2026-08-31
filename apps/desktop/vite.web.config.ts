import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// ブラウザ版。コマンド呼び出しを WASM(crates/web)に差し替えてビルドする。
// 実行時分岐ではなく alias で切るのは、デスクトップ版に WASM を積みたくないため。
const resolve = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: [
      { find: "./invoke", replacement: resolve("./src/api/invoke.wasm.ts") },
      // wasm-pack build の出力(npm run build:wasm)
      { find: "tw-web", replacement: resolve("../../crates/web/pkg/web.js") },
      // デスクトップ版だけの説明(自動バックアップ)を画面から外すための旗
      { find: "./platform", replacement: resolve("./src/web/platform.ts") },
      // Tauri プラグイン前提の口(外部リンク・お知らせの取得・自動更新)をブラウザ用に差し替える。
      // 画面側で「デスクトップかどうか」を分岐させないため、ここで実体だけを入れ替える。
      { find: "@tauri-apps/plugin-opener", replacement: resolve("./src/web/opener.ts") },
      { find: "@tauri-apps/plugin-http", replacement: resolve("./src/web/http.ts") },
      { find: "@tauri-apps/plugin-updater", replacement: resolve("./src/web/updater.ts") },
      { find: "@tauri-apps/plugin-process", replacement: resolve("./src/web/process.ts") },
    ],
  },
  // dev サーバーはリポジトリ外の pkg を読むので、配信を許す範囲をリポジトリルートまで広げる
  server: { fs: { allow: [resolve("../..")] } },
  build: { outDir: "dist-web", emptyOutDir: true },
});
