/**
 * ブラウザ版。crates/web が公開する `invoke` を Tauri と同じシグネチャに合わせる。
 *
 * WASM 側は同期関数だが、画面は Promise を前提にしているので async で包む。
 * エラーは crates/web が投げる CommandError(message / location)がそのまま伝わる。
 */
import init, { invoke as callWasm } from "tw-web";

// 初期化は 1 回だけ。最初に呼ばれた invoke がこれを待つ(呼び出し側に初期化を意識させない)。
const ready = init();

export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  await ready;
  return callWasm(command, args) as T;
}
