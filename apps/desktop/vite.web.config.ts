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
    ],
  },
  // dev サーバーはリポジトリ外の pkg を読むので、配信を許す範囲をリポジトリルートまで広げる
  server: { fs: { allow: [resolve("../..")] } },
  build: { outDir: "dist-web", emptyOutDir: true },
});
