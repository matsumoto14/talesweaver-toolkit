import { mount } from "svelte";
import "@fontsource/m-plus-rounded-1c/400.css";
import "@fontsource/m-plus-rounded-1c/500.css";
import "@fontsource/m-plus-rounded-1c/700.css";
import "@fontsource/m-plus-rounded-1c/800.css";
import "@fontsource/m-plus-1-code/400.css";
import "@fontsource/m-plus-1-code/500.css";
import "@fontsource/m-plus-1-code/700.css";
import "./app.css";
import { loadStatLimits } from "./limits.svelte";
import { CAN_RESET_STORE, resetStore } from "./recovery";
import { loadGameTables } from "./tables.svelte";

const root = document.getElementById("app")!;

/**
 * 保存データを消して開き直す。開けなくなった保存先からの復旧で、消えたキャラは
 * 書き出し JSON からしか戻せないので、人が押して、さらに確認してからだけ消す。
 */
async function reset(): Promise<void> {
  if (!confirm("保存データ(登録キャラ・バフセット)を消して開き直します。元に戻せません。")) return;
  await resetStore();
  location.replace(location.pathname);
}

/**
 * `?reset` を付けて開いたときは、アプリを立ち上げる前に消す。画面が出たあとに落ちて
 * ボタンを押せない状態でも、URL だけで復旧できるようにするため(2026-09-18)。
 */
if (CAN_RESET_STORE && new URLSearchParams(location.search).has("reset")) {
  void reset().catch((e) => {
    root.textContent = `保存データを消せませんでした: ${String(e)}`;
  });
} else {
  // 値域上限(get_stat_limits)とカタログ(get_game_tables)は labels.ts などがモジュール評価時に
  // 読む。フォールバック値は持たない方針(唯一の正は crates/domain)なので、**取得を終えてから**
  // App を読み込む。
  // 静的 import だとモジュールグラフが先に評価されて間に合わないため、動的 import にしている。
  // 待っている間は index.html の #app に置いた「読み込み中…」が見えている。
  void Promise.all([loadStatLimits(), loadGameTables()])
    .then(() => import("./App.svelte"))
    .then(({ default: App }) => {
      root.replaceChildren();
      mount(App, { target: root });
    })
    .catch((e) => {
      root.replaceChildren();
      const message = document.createElement("p");
      message.textContent = `起動に失敗しました: ${String(e)}`;
      root.append(message);
      // 保存データが原因で開けないときの逃げ道(ブラウザ版だけ)。端末の設定から
      // サイトデータを探させず、ここから消して開き直せるようにする。
      if (CAN_RESET_STORE) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = "保存データを消して開き直す";
        button.addEventListener("click", () => void reset());
        root.append(button);
      }
    });
}
