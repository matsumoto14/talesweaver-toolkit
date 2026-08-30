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

// 値域上限(get_stat_limits)は labels.ts などがモジュール評価時に読む。フォールバック値は
// 持たない方針(唯一の正は crates/domain)なので、**取得を終えてから** App を読み込む。
// 静的 import だとモジュールグラフが先に評価されて間に合わないため、動的 import にしている。
// 待っている間は index.html の #app に置いた「読み込み中…」が見えている。
void loadStatLimits()
  .then(() => import("./App.svelte"))
  .then(({ default: App }) => {
    document.getElementById("app")!.replaceChildren();
    mount(App, { target: document.getElementById("app")! });
  })
  .catch((e) => {
    document.getElementById("app")!.textContent = `起動に失敗しました: ${String(e)}`;
  });
