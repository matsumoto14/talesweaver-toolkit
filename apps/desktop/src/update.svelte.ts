// アプリ自身の更新。配信元は tauri.conf.json の `plugins.updater.endpoints`
// (紹介ページのダウンロードと同じ R2 の固定 URL)。
//
// 勝手に落として当てることはしない。起動時に「あるかどうか」だけ見て、
// お知らせタブに出す。当てるのはユーザーが押したときだけ(§00 03)。
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type UpdateStatus =
  /** まだ見ていない / 見に行っている途中 */
  | "idle" | "checking"
  /** 最新だった。見に行けなかったときもここ(オフラインを不具合として出さない) */
  | "current"
  /** 新しい版がある */
  | "available"
  | "downloading" | "installing"
  /** 当て終わった。再起動すると新しい版になる */
  | "ready"
  /** ダウンロードや適用に失敗した。理由を出して、もう一度押せる */
  | "failed";

export const updater = $state({
  status: "idle" as UpdateStatus,
  /** 新しい版の版番号 */
  version: "",
  /** その版の一言(配信元の latest.json の notes) */
  notes: "",
  /** ダウンロードの進み(0〜100)。総量が分からない配信元では -1 */
  percent: -1,
  /** 失敗したときの理由 */
  error: "",
});

let pending: Update | null = null;

/** 起動時に 1 回だけ見に行く。落ちても黙って「最新」にする(通信は本質ではない) */
export async function checkForUpdate(): Promise<void> {
  if (updater.status !== "idle") return;
  updater.status = "checking";
  try {
    const update = await check();
    if (!update) {
      updater.status = "current";
      return;
    }
    pending = update;
    updater.version = update.version;
    updater.notes = update.body ?? "";
    updater.status = "available";
  } catch {
    updater.status = "current";
  }
}

/** ユーザーが「更新する」を押したとき。落として当てるところまで(再起動は別操作) */
export async function installUpdate(): Promise<void> {
  if (!pending || updater.status === "downloading" || updater.status === "installing") return;
  updater.status = "downloading";
  updater.percent = -1;
  updater.error = "";
  let downloaded = 0;
  let total = 0;
  try {
    await pending.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? 0;
          updater.percent = total > 0 ? 0 : -1;
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          if (total > 0) updater.percent = Math.min(100, Math.round((downloaded / total) * 100));
          break;
        case "Finished":
          updater.percent = 100;
          updater.status = "installing";
          break;
      }
    });
    updater.status = "ready";
  } catch (error) {
    updater.status = "failed";
    updater.error = error instanceof Error ? error.message : String(error);
  }
}

/** 当て終わったあとの再起動 */
export async function restartApp(): Promise<void> {
  await relaunch();
}
