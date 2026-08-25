// 動きの仕事は「いま何が変わったか」を探させずに伝えること(design-system §10)。
// 装飾のアニメーションは置かない — 置くと本当の変化がその中に埋もれる。
//
// 実際の見た目(keyframes)は app.css がグローバルに持つ。ここは「いつ動かすか」だけ。
// prefers-reduced-motion のときは app.css 側で全アニメーションが実質 0 になり、
// 色・バッジ・数値そのもので既に伝わっている状態が残る。

/**
 * 数値が変わったことを認知させる。変わった要素**だけ**を跳ねさせ、
 * 増減を色で 0.3s だけ伝えてから元に戻す(色を残すと状態色 §03 と意味が混ざる)。
 *
 * Svelte 5 の action は引数が変わっても再実行されないので、**値そのものではなく
 * getter を渡す**(`use:bump={() => perHit}`)。中の `$effect` がそれを読む。
 * 連打されても途中から再スタートできるよう、クラスを外して reflow を挟んでから付け直す。
 */
export function bump(node: HTMLElement, get: () => number | null) {
  const clear = () => node.classList.remove("bump-up", "bump-down");
  let prev = get();
  $effect(() => {
    const next = get();
    if (next === null || prev === null || next === prev) {
      prev = next;
      return;
    }
    clear();
    void node.offsetWidth; // 再スタートさせるための強制 reflow
    node.classList.add(next > prev ? "bump-up" : "bump-down");
    prev = next;
  });
  $effect(() => {
    node.addEventListener("animationend", clear);
    return () => node.removeEventListener("animationend", clear);
  });
}
