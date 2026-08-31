// トリガの直下に重なるもの(design-system §09 規則 3)。周りのレイアウトを押さず、
// 閉じたときに何も動かないことが条件。見た目(面・影・角丸)は app.css の `.popover`。
// ここは「画面からはみ出さないように置き直す」振る舞いだけを持つ。

/**
 * 開いた場所の直下に置いたまま、画面外へはみ出さないよう位置決めする Svelte action。
 *
 * 下に入らなければ上に開く(フリップ)。上にも入り切らなければ、収まる高さにクランプして
 * 中身だけスクロールにする(画面外に開いたまま放置しない)。**トリガ自身は動かさない** —
 * 位置合わせは絶対配置のまま完結させる(§09 規則 1・3)。
 */
export function positionPopover(node: HTMLElement) {
  const margin = 8;
  const rect = node.getBoundingClientRect();
  if (rect.bottom > window.innerHeight - margin) {
    node.classList.add("flip-up");
    const flipped = node.getBoundingClientRect();
    if (flipped.top < margin) {
      const available = Math.max(flipped.bottom - margin, 60);
      node.style.maxHeight = `${available}px`;
      node.style.overflowY = "auto";
    }
  }
  return {
    destroy() {
      node.classList.remove("flip-up");
      node.style.maxHeight = "";
      node.style.overflowY = "";
    },
  };
}
