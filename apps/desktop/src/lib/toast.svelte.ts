// 画面上部に出すエラー帯。どのコンポーネントからも報告できるよう共有状態にする。
export const toast = $state<{ message: string | null }>({ message: null });

let timer: ReturnType<typeof setTimeout> | undefined;

export function reportError(message: string) {
  toast.message = message;
  clearTimeout(timer);
  timer = setTimeout(() => (toast.message = null), 8000);
}

export function dismissError() {
  toast.message = null;
  clearTimeout(timer);
}
