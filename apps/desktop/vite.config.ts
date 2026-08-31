import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri の devUrl(tauri.conf.json)と合わせる
export default defineConfig({
  plugins: [svelte()],
  // コマンド呼び出しの実体を明示する。Web 版(vite.web.config.ts)は同じ口を
  // WASM に差し替える。実行時分岐にしないのは、デスクトップ版に WASM を積まないため。
  resolve: {
    alias: [
      {
        find: "./invoke",
        replacement: fileURLToPath(new URL("./src/api/invoke.tauri.ts", import.meta.url)),
      },
    ],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
