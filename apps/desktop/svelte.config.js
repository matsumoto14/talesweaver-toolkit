import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// svelte-check と vite-plugin-svelte が共有する設定。lang="ts" の <script> を TS として前処理する
export default {
  preprocess: vitePreprocess(),
};
