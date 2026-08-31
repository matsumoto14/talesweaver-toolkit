// 画面上部に出す帯。どのコンポーネントからも報告できるよう共有状態にする。
//
// 3 種類ある:
// - `error` … 操作の失敗。8 秒で自動的に消える(飛べる場所を持つものは自動では消さない)
// - `notice` … 起動時の復元など、消してはいけない事実。ユーザーが閉じるまで残す
// - `undo` … 消したものを戻せる帯。取り消せる操作の直後だけ出す
import type { ValidationLocation } from "./api/types";

export type ToastKind = "error" | "notice" | "undo";

/** エラーが指している場所。帯の「ここを開く」はここへ飛ぶ(§00 ⑤ 考えさせない)。 */
export type ErrorTarget = { characterId: number; location: ValidationLocation };

export const toast = $state<{
  message: string | null;
  kind: ToastKind;
  target: ErrorTarget | null;
  /** 「元に戻す」を出すか。復元そのもの(関数)は $state に入れない —
   *  リアクティブプロキシに関数を持たせると呼び出し時に壊れる */
  undoable: boolean;
}>({
  message: null,
  kind: "error",
  target: null,
  undoable: false,
});

/** 「元に戻す」の中身。帯が消えたら捨てる */
let undoAction: (() => void | Promise<void>) | null = null;

let timer: ReturnType<typeof setTimeout> | undefined;

/**
 * 操作の失敗。`target` 付き(飛べるエラー)は自動で消さない — 押す前に消えたら
 * 「どこの話か」を確かめる手立てが無くなる(§00 ⑤ 考えさせない)。
 * 消えるのは × で閉じたときと「ここを開く」で飛んだとき。
 */
export function reportError(message: string, target: ErrorTarget | null = null) {
  toast.message = message;
  toast.kind = "error";
  toast.target = target;
  toast.undoable = false;
  undoAction = null;
  clearTimeout(timer);
  if (target !== null) return;
  timer = setTimeout(() => (toast.message = null), 8000);
}

/** 自動では消えない告知。起動時の復元など「読み飛ばされては困る」ものに使う。 */
export function reportNotice(message: string) {
  toast.message = message;
  toast.kind = "notice";
  toast.target = null;
  toast.undoable = false;
  undoAction = null;
  clearTimeout(timer);
}

/**
 * 消したものを戻せる帯。**取り消せる操作の直後だけ**出す — 出しっぱなしにすると
 * 「まだ戻せるのか」を毎回考えることになる(§00 ⑤)。
 * 猶予は 12 秒。確認を挟んだうえでの取り消しなので、読んで決める時間を少し長く取る。
 */
export function reportUndo(message: string, undo: () => void | Promise<void>) {
  toast.message = message;
  toast.kind = "undo";
  toast.target = null;
  toast.undoable = true;
  undoAction = undo;
  clearTimeout(timer);
  timer = setTimeout(() => {
    toast.message = null;
    toast.undoable = false;
    undoAction = null;
  }, 12000);
}

/** 「元に戻す」を押したとき。帯は先に畳んで、二重に押せないようにする */
export function runUndo(): void {
  const undo = undoAction;
  dismissError();
  void undo?.();
}

export function dismissError() {
  toast.message = null;
  toast.target = null;
  toast.undoable = false;
  undoAction = null;
  clearTimeout(timer);
}
