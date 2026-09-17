// 動きの仕事は「いま何が変わったか」を探させずに伝えること(design-system §10)。
// 装飾のアニメーションは置かない — 置くと本当の変化がその中に埋もれる。
//
// 実際の見た目(keyframes)は app.css がグローバルに持つ。ここは「いつ動かすか」だけ。
// prefers-reduced-motion のときは app.css 側で全アニメーションが実質 0 になり、
// 色・バッジ・数値そのもので既に伝わっている状態が残る。

import { fmtNum } from "../format";

/**
 * 動きの時間(ms)。**app.css の --dur-* と同じ段**で、こちらが Svelte の transition / animate 用。
 * 2 つに分かれているのは CSS 変数が JS から素直に読めないからで、**値は必ず一致させる**
 * (機械監査 R15 が app.css と突き合わせる)。画面ごとに 220 や 260 を直接書かない。
 */
export const DUR = {
  tap: 150,    /* 触った応答 */
  pop: 170,    /* 一時的に重なった */
  swap: 200,   /* 面が入れ替わった */
  open: 220,   /* 開いた / 閉じた */
  badge: 260,  /* 状態が変わった */
  move: 260,   /* 並びが変わった(FLIP) */
  pane: 280,   /* 場所が増えた */
  bump: 300,   /* 数値が変わった */
  bar: 380,    /* 量が変わった */
} as const;

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
const reducedMotion = () =>
  typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

export const motionDuration = (ms: number) => (reducedMotion() ? 0 : ms);

/**
 * 要素を画面内に入れる(§10「動きを消しても変化が分かること」)。`scrollIntoView` の
 * `behavior: "smooth"` は CSS の `scroll-behavior` と違って動きを消す設定を見ないので、
 * ここで揃えてから渡す(app.css の `scroll-behavior: auto !important` は CSS 発火の
 * スクロールにしか効かず、JS から呼ぶこの経路は素通りする)。
 *
 * action ではなくただの関数にしたのは、呼び出し側が全て `$effect` の中や
 * `await tick()` のあとで「この 1 回だけ動かす」ために直接呼んでいて、
 * 要素の生存期間に合わせて付け外す状態を持たないため。
 *
 * `block` は呼び出し側が選ぶ("center" は遠くの行を光らせて呼ぶ用、"nearest" は
 * 追従用)。`instant` を渡すと、動きを消す設定に関わらず瞬時に動かす — 自分の操作を
 * 追いかけるだけで、動きそのもので何かを伝える必要がない場面向け(Workspace の
 * `follow()` 参照)。
 */
export function reveal(el: Element | null | undefined, block: ScrollLogicalPosition, opts?: { instant?: boolean }) {
  if (!el) return;
  el.scrollIntoView({ block, behavior: opts?.instant || reducedMotion() ? "auto" : "smooth" });
}

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
 * **面・行・文が変わった**ことを見せる(§10)。値は `<Value>` が持つので、ここに来るのは
 * 値ではないもの — 要約の文・バッジ・行の着地の印・面の中身の入れ替え。
 *
 * **どう動くかは要素が既に載せている入場クラスが決める。**`swap-in` / `pane-in` /
 * `open-in` / `badge-in` のどれかが `class` にあればそれを**もう一度再生**し、無ければ
 * 型 5(`badge-in`、弾んで出る)。同じ要素が「生まれたとき」と「変わったとき」で違う動きを
 * することはないので、動き方の宣言は入場クラス 1 か所に集まる — この action は
 * 「いつ再生するか」だけを持つ。面(`swap-in`)とバッジ(`badge-in`)の取り違えは、
 * 入場の見え方を見れば分かる形になった(取り違えると入場のときから間違って見える)。
 *
 * **何を渡すかで「変わった」の意味が決まる**(`<Value>` が `motion` の有無で跳ねと光りを
 * 分けるのと同じ。選択肢ではない):
 *
 * - **文字列** = いまの中身。書式済みの要約・状態名・id。**生まれた時点では動かさない**
 *   (生まれたときの中身は「変わった」ではない)。
 * - **文字列以外**(印のオブジェクト・`null`)= いま誰の番か。`null` は「いまはこの要素の
 *   番ではない」として無視する(`bump` が上下を判定できない `null` を無視するのと同じ)。
 *   こちらは**生まれた時点で自分の番なら動く** — 行が群をまたいで動くとき(★ を切り替える・
 *   別の群へドラッグする)、行は別の `{#each}` に移るので DOM ノードが作り直され、
 *   新しいノードが生まれた時点で既に自分の番になっているため(`get()` を初回の基準に取ると
 *   一度も弾まない。実際に ★ 切替が毎回無音だった)。
 *   同じ印で再発火させたいときは毎回新しいオブジェクトを渡す(`{}` は常に前回と不等)。
 *
 * Svelte 5 の action は引数が変わっても再実行されないので、値ではなく getter を渡す。
 */
const ENTRY = ["swap-in", "pane-in", "open-in", "badge-in"];
const FIRST = Symbol("changed/first");

export function changed(node: HTMLElement, get: () => unknown) {
  const cls = ENTRY.find((c) => node.classList.contains(c)) ?? "badge-in";
  const clear = () => node.classList.remove(cls);
  let prev: unknown = FIRST;
  $effect(() => {
    const next = get();
    const first = prev === FIRST;
    // 文字列は「中身」なので初回は動かさない。印は初回から自分の番なら動く
    if (next === null || next === prev || (first && typeof next === "string")) {
      prev = next;
      return;
    }
    prev = next;
    clear();
    void node.offsetWidth; // 再スタートさせるための強制 reflow
    node.classList.add(cls);
  });
  $effect(() => {
    node.addEventListener("animationend", clear);
    return () => node.removeEventListener("animationend", clear);
  });
}
