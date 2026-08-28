// 画面上部に出す帯。どのコンポーネントからも報告できるよう共有状態にする。
//
// 2 種類ある:
// - `error` … 操作の失敗。8 秒で自動的に消える
// - `notice` … 起動時の復元など、消してはいけない事実。ユーザーが閉じるまで残す
export type ToastKind = "error" | "notice";

export const toast = $state<{ message: string | null; kind: ToastKind }>({
  message: null,
  kind: "error",
});

let timer: ReturnType<typeof setTimeout> | undefined;

export function reportError(message: string) {
  toast.message = message;
  toast.kind = "error";
  clearTimeout(timer);
  timer = setTimeout(() => (toast.message = null), 8000);
}

/** 自動では消えない告知。起動時の復元など「読み飛ばされては困る」ものに使う。 */
export function reportNotice(message: string) {
  toast.message = message;
  toast.kind = "notice";
  clearTimeout(timer);
}

export function dismissError() {
  toast.message = null;
  clearTimeout(timer);
}
