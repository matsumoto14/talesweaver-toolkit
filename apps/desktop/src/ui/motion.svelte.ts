// 動きの仕事は「いま何が変わったか」を探させずに伝えること(design-system §10)。
// 装飾のアニメーションは置かない — 置くと本当の変化がその中に埋もれる。
//
// 実際の見た目(keyframes)は app.css がグローバルに持つ。ここは「いつ動かすか」だけ。
// prefers-reduced-motion のときは app.css 側で全アニメーションが実質 0 になり、
// 色・バッジ・数値そのもので既に伝わっている状態が残る。

import { fmtNum } from "../format";

/**
 * 数値が変わったことを認知させる。変わった要素**だけ**を跳ねさせ、
 * 増減を色で 0.3s だけ伝えてから元に戻す(色を残すと状態色 §03 と意味が混ざる。
 * 本体の色を残す案は試して「わかりづらい」と却下 — 2026-09-15。差分は delta 側が持つ)。
 *
 * Svelte 5 の action は引数が変わっても再実行されないので、**値そのものではなく
 * getter を渡す**(`use:bump={() => perHit}`)。中の `$effect` がそれを読む。
 * 連打されても途中から再スタートできるよう、クラスを外して reflow を挟んでから付け直す。
 */
/**
 * Svelte の animate / transition に渡す時間(ms)。動きを消す設定(prefers-reduced-motion)のときは 0。
 * CSS のアニメーションは app.css が一括で殺しているが、Svelte の animate / transition は JS なので
 * ここで見る必要がある(§10「動きを消しても変化が分かること」)。
 */
export const motionDuration = (ms: number) =>
  typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : ms;

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

/**
 * **いくつ変わったか**を出す差分枠(§10 型 1b)。跳ねと色は 0.3s で戻るので、同時に
 * 10 か所が動くと「動いた」は分かっても「いくつ」は読み切れない — 前回値との差を
 * 緑 ↑1,234 / 赤 ↓3.2% で出し、消さずにその数値が次に変わるまで残す(書き換えるだけ)。
 *
 * 付ける先は数値そのものではなく**差分専用の空 span**。数値の中に差し込むと、出た瞬間・
 * 桁が変わった瞬間に隣を押してがたつく(実機 2026-09-15)。枠は最初から場所を取り、
 * 幅は増える方向にしか変えない(§09 規則 4)。浮かせて重ねる案は却下(ユーザー判断)。
 * `get` は表示単位で渡す(15% と出すなら 0.15 ではなく 15 を渡して unit "%")。
 */
export interface DeltaSpec {
  get: () => number | null;
  /** 差分の後ろに付ける単位("%" / "s" など)。省略なら無し */
  unit?: string;
  /** 小数桁。省略なら前回値・今回値が両方整数のとき 0、それ以外 2(末尾の 0 は落とす) */
  digits?: number;
}

function formatDelta(prev: number, next: number, spec: DeltaSpec): string {
  const d = next - prev;
  const digits = spec.digits ?? (Number.isInteger(prev) && Number.isInteger(next) ? 0 : 2);
  const body = fmtNum(Math.abs(d), { max: digits });
  return `${d > 0 ? "↑" : "↓"}${body}${spec.unit ?? ""}`;
}

export function delta(node: HTMLElement, spec: DeltaSpec) {
  node.classList.add("delta", "num");
  node.setAttribute("aria-hidden", "true");
  let prev = spec.get();
  // CSS 側の min-width(列の幅)より広くなったときだけ inline で広げる。常に inline で上書きすると
  // 列幅(64px)が文言の幅(50px)に縮んで、右の列が行ごとにずれる(実機 2026-09-15)
  const base = parseFloat(getComputedStyle(node).minWidth) || 0;
  let width = base;
  $effect(() => {
    const next = spec.get();
    if (next === null || prev === null || next === prev) {
      prev = next;
      return;
    }
    node.textContent = formatDelta(prev, next, spec);
    node.classList.remove("delta-in", "up", "down");
    width = Math.max(width, node.offsetWidth); // reflow を兼ねる(再スタート用)
    if (width > base) node.style.minWidth = `${width}px`;
    node.classList.add("delta-in", next > prev ? "up" : "down");
    prev = next;
  });
}

/**
 * 数値ではない**要約が変わった**ことを見せる(§10 型 5「状態が変わった → 弾んで出る」)。
 * 補正源リストの行サマリーのように、右のペインを触ると左の要約も変わるもので使う。
 * 片方だけ動かないと、動かないほうが古い値に見える(§10 規則 2)。
 */
/**
 * **面の中身が入れ替わった**ことを見せる(§10 型 3b「入ってくる面だけ短く動かす」)。
 * タブで中身を差し替えたのに何も動かないと、切り替えたのか元からこうだったのかが
 * 一瞬わからない。`flash`(型 5)と取り違えると、面ぜんたいが中心から膨らんで
 * 他のタブ切り替えと動きが揃わなくなる — バッジは flash、面は swap。
 */
export function swap(node: HTMLElement, get: () => string) {
  const clear = () => node.classList.remove("swap-in");
  let prev = get();
  $effect(() => {
    const next = get();
    if (next === prev) return;
    prev = next;
    clear();
    void node.offsetWidth;
    node.classList.add("swap-in");
  });
  $effect(() => {
    node.addEventListener("animationend", clear);
    return () => node.removeEventListener("animationend", clear);
  });
}

export function flash(node: HTMLElement, get: () => string) {
  const clear = () => node.classList.remove("badge-in");
  let prev = get();
  $effect(() => {
    const next = get();
    if (next === prev) return;
    prev = next;
    clear();
    void node.offsetWidth;
    node.classList.add("badge-in");
  });
  $effect(() => {
    node.addEventListener("animationend", clear);
    return () => node.removeEventListener("animationend", clear);
  });
}

