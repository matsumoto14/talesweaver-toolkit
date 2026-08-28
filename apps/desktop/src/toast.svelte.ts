// 画面上部に出す帯。どのコンポーネントからも報告できるよう共有状態にする。
//
// 2 種類ある:
// - `error` … 操作の失敗。8 秒で自動的に消える(飛べる場所を持つものは自動では消さない)
// - `notice` … 起動時の復元など、消してはいけない事実。ユーザーが閉じるまで残す
import type { ValidationLocation } from "./api/types";

export type ToastKind = "error" | "notice";

/** エラーが指している場所。帯の「ここを開く」はここへ飛ぶ(§00 ⑤ 考えさせない)。 */
export type ErrorTarget = { characterId: number; location: ValidationLocation };

export const toast = $state<{ message: string | null; kind: ToastKind; target: ErrorTarget | null }>({
  message: null,
  kind: "error",
  target: null,
});

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
  clearTimeout(timer);
  if (target !== null) return;
  timer = setTimeout(() => (toast.message = null), 8000);
}

/** 自動では消えない告知。起動時の復元など「読み飛ばされては困る」ものに使う。 */
export function reportNotice(message: string) {
  toast.message = message;
  toast.kind = "notice";
  toast.target = null;
  clearTimeout(timer);
}

export function dismissError() {
  toast.message = null;
  toast.target = null;
  clearTimeout(timer);
}
