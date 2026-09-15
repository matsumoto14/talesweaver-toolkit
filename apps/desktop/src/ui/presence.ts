// 行の出入り(§10 型 1b の「行」版)。称号を切り替えると供給源の行は「値が変わる」のではなく
// 「旧称号の行が抜け、新称号の行が入る」。index キーで並べると新行が旧行を上書きして、
// 値の ↑↓ は出るがラベルが変わったことは何も残らない。
// ここでは出典キーごとに前回の集合を覚え、抜けた行を「次に集合が変わるまで」残す
// (数値の delta と同じ「消さずに次の変化で書き換える」ルール)。

export type Presence = "same" | "added" | "gone";
export interface Marked<T> {
  item: T;
  state: Presence;
  /** 行の識別子(keyOf の値。同名が並ぶときは `#2` を足して一意にする)。`{#each}` のキーに使う */
  key: string;
}

interface Slot<T> {
  keys: string[];
  marked: Marked<T>[];
}

/** 同名の行が並んでも(同じ効果の装備 2 つなど)キーが衝突しないよう 2 つ目から `#2` を足す */
function uniqueKeys(keys: string[]): string[] {
  const seen = new Map<string, number>();
  return keys.map((k) => {
    const n = (seen.get(k) ?? 0) + 1;
    seen.set(k, n);
    return n === 1 ? k : `${k}#${n}`;
  });
}
const sameKeys = (a: string[], b: string[]) => a.length === b.length && a.every((k, i) => k === b[i]);

/** slot(カテゴリなど)ごとに行の集合を覚える。reactive にしない — 呼び出し側の derived の中で使う */
export class PresenceMemo<T> {
  private slots = new Map<string, Slot<T>>();

  mark(slot: string, items: T[], keyOf: (t: T) => string): Marked<T>[] {
    const keys = uniqueKeys(items.map(keyOf));
    const prev = this.slots.get(slot);
    if (!prev) {
      const marked = items.map((item, i): Marked<T> => ({ item, state: "same", key: keys[i] }));
      this.slots.set(slot, { keys, marked });
      return marked;
    }
    if (sameKeys(prev.keys, keys)) {
      // 集合は同じ。値だけ差し替えて印(added / gone)は保つ
      const live = new Map(items.map((it, i) => [keys[i], it]));
      prev.marked = prev.marked.map((m) => (m.state === "gone" ? m : { ...m, item: live.get(m.key) ?? m.item }));
      return prev.marked;
    }
    const before = new Set(prev.marked.filter((m) => m.state !== "gone").map((m) => m.key));
    const now = new Set(keys);
    const marked: Marked<T>[] = items.map((item, i) => ({
      item, key: keys[i], state: before.has(keys[i]) ? "same" : "added",
    }));
    for (const m of prev.marked) {
      if (m.state !== "gone" && !now.has(m.key)) marked.push({ ...m, state: "gone" });
    }
    this.slots.set(slot, { keys, marked });
    return marked;
  }
}

/** 「A → B」の一行。抜けた行・入った行のどちらかだけなら − A / + B */
export function swapNote(gone: string[], added: string[]): string | undefined {
  if (gone.length === 0 && added.length === 0) return undefined;
  if (gone.length === 0) return `+ ${added.join("・")}`;
  if (added.length === 0) return `− ${gone.join("・")}`;
  return `${gone.join("・")} → ${added.join("・")}`;
}

/**
 * 「直近の計算で値が変わったか」を key ごとに覚える。`delta` action(要素が前回値を持つ)の
 * データ版で、「↑ を押すと、その下で変わった行だけを開く」(緑を辿る)ために使う。
 * 世代は `token`(結果オブジェクト)の同一性で区切る。同じ結果の再描画で何度呼ばれても
 * 判定は変わらない。
 */
export class ChangeMemo {
  private last = new Map<string, { n: number | null; gen: number }>();
  private token: unknown = undefined;
  private gen = 0;

  touch(key: string, n: number | null, token: unknown): boolean {
    if (token !== this.token) {
      this.token = token;
      this.gen += 1;
    }
    const p = this.last.get(key);
    if (!p) {
      this.last.set(key, { n, gen: 0 });
      return false;
    }
    if (p.n !== n) {
      const changed = p.n !== null && n !== null;
      p.n = n;
      p.gen = changed ? this.gen : 0;
    }
    return p.gen === this.gen;
  }
}
