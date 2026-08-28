// ドラッグして並べ替える操作の核心(index の判定 + splice)。CharacterRail(縦・単一リスト)/
// Workspace の補正源リスト(縦・お気に入り⇄そのほかの2リスト)/ EquipmentPane の装備登録切替
// (横・部位ごとの単一リスト)で、この計算がほぼ同じコードとして重複していたので集約する。
// ドラッグの状態($state)・DOM 属性・アニメーションは呼び出し側の構造差が大きいのでここには
// 持たず、各コンポーネントが持ったまま呼び出す(見た目・挙動は変えない)。

/**
 * ドラッグ中の要素をどこに落とすかを、ポインタ位置と要素の矩形から判定する。
 * 要素の前半分なら手前(index)、後半分なら後ろ(index + 1)。
 */
export function dropHalfIndex(rect: DOMRect, pos: number, index: number, axis: "x" | "y" = "y"): number {
  const start = axis === "y" ? rect.top : rect.left;
  const size = axis === "y" ? rect.height : rect.width;
  return index + (pos < start + size / 2 ? 0 : 1);
}

/**
 * 削除ぶんのズレを補正した挿入先 index を返す。fromIndex は同じ配列内の元位置
 * (無ければ -1)、toIndex は削除前基準で望む挿入位置。
 */
export function adjustDropIndex(fromIndex: number, toIndex: number): number {
  return fromIndex !== -1 && fromIndex < toIndex ? toIndex - 1 : toIndex;
}

/** 単一配列内で要素を並べ替える(新しい配列を返す。from が見つからなければ元の配列のまま)。 */
export function moveItem<T>(list: readonly T[], from: number, to: number): T[] {
  if (from === -1) return [...list];
  const next = [...list];
  const [item] = next.splice(from, 1);
  const index = adjustDropIndex(from, to);
  next.splice(Math.max(0, Math.min(next.length, index)), 0, item);
  return next;
}
