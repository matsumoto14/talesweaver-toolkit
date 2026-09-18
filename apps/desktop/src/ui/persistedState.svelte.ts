// localStorage に永続化する汎用ヘルパー。呼び出し元コンポーネントの <script> トップレベル
// (初期化中)で呼ぶこと。$effect はコンポーネント初期化中にしか登録できないため、
// モジュールトップレベルから直接呼び出すと effect_orphan エラーになる。
// 型は「単純なオブジェクト」を想定し、JSON.parse の結果が期待する形かは検証しない。
// 初期値がオブジェクトなら保存側に無い欄を初期値で埋める(あとから欄を足しても既存の保存が壊れない)。
export function persisted<T>(key: string, initial: T): { value: T } {
  let stored = initial;
  try {
    const raw = localStorage.getItem(key);
    if (raw !== null) {
      const parsed = JSON.parse(raw) as T;
      stored = isPlainObject(initial) && isPlainObject(parsed) ? { ...initial, ...parsed } : parsed;
    }
  } catch {
    // 壊れた値・private モード等は初期値にフォールバック
  }
  const state = $state({ value: stored });
  $effect(() => {
    try {
      localStorage.setItem(key, JSON.stringify(state.value));
    } catch {
      // 無視(private モード等で書き込めない場合)
    }
  });
  return state;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}
