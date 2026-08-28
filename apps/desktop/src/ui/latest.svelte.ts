// 「一番最後に呼んだ非同期処理の結果だけを反映する」レースガードを1箇所にまとめる。
// 各画面がバラバラに持っていた `let xSeq = 0` + `if (seq === xSeq) ...` + debounce の
// setTimeout 管理を、呼び出し側の形をほぼ変えずに置き換えるための薄いヘルパー。
//
// debounce 値は呼び出し側ごとに意味が違う(体感の追従速度と API 呼び出し頻度のトレードオフ)ので、
// このヘルパーは値を統一しない — 既存の値をそのまま opts.debounce に渡すだけ。

/**
 * 単一系列のレースガード。`run` を呼ぶたびに前回の debounce タイマーを消して番号を進め、
 * task に「自分がまだ最新か」を返す `isCurrent()` を渡す。
 *
 * 使い方(既存の seq パターンとほぼ同型):
 * ```ts
 * const requestLatest = latest({ debounce: 120 });
 * $effect(() => {
 *   ...
 *   requestLatest.run(async (isCurrent) => {
 *     const r = await previewDamage(...);
 *     if (isCurrent()) result = r;
 *   });
 *   return () => requestLatest.cancel();
 * });
 * ```
 */
export function latest(opts: { debounce?: number } = {}) {
  let seq = 0;
  let handle: ReturnType<typeof setTimeout> | undefined;

  function run(task: (isCurrent: () => boolean) => void | Promise<void>): void {
    if (handle) clearTimeout(handle);
    const mySeq = ++seq;
    const isCurrent = () => mySeq === seq;
    if (opts.debounce) {
      handle = setTimeout(() => void task(isCurrent), opts.debounce);
    } else {
      void task(isCurrent);
    }
  }

  function cancel(): void {
    if (handle) clearTimeout(handle);
  }

  return { run, cancel };
}

/**
 * キーごとに独立したレースガードを持つ版(例: キャラ id ごとの再判定)。
 * debounce は使わない用途向け(state.svelte.ts の再判定リクエストがこれ)。
 */
export function latestByKey<K extends string | number>() {
  const seqs = new Map<K, number>();

  function run<T>(key: K, task: (isCurrent: () => boolean) => T | Promise<T>): Promise<T> {
    const mySeq = (seqs.get(key) ?? 0) + 1;
    seqs.set(key, mySeq);
    const isCurrent = () => seqs.get(key) === mySeq;
    return Promise.resolve(task(isCurrent));
  }

  return { run };
}
