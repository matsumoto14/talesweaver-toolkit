// トリガの直下に重なるもの(design-system §09 規則 3)。周りのレイアウトを押さず、
// 閉じたときに何も動かないことが条件。見た目(面・影・角丸)は app.css の `.popover`。
// ここは「画面からはみ出さないように置き直す」振る舞いだけを持つ。

/**
 * 開いた場所の直下に置いたまま、画面外へはみ出さないよう位置決めする Svelte action。
 *
 * **`position: fixed` に置き換えてから位置を決める**。絶対配置のままだと、スクロールする
 * 一覧(バフタブの `.chips` は `overflow-y: auto`)の中で開いたときに一覧の枠で切られ、
 * 最下段のチップでは中身が見えなかった(実機報告: クラブ効果の「設定」)。fixed なら
 * 祖先の overflow に切られないので、画面のどこで開いても中身が全部見える。
 *
 * 下に入らなければ上に開く(フリップ)。どちらにも入り切らなければ広いほうへ置き、収まる
 * 高さにクランプして中身だけスクロールにする。**トリガ自身は動かさない**(§09 規則 1・3)。
 * fixed は画面に貼り付くので、スクロール・リサイズのたびに置き直してトリガに付いて回らせる。
 */
export function positionPopover(node: HTMLElement) {
  const margin = 8;
  const gap = 4;
  // CSS が決めた「トリガ直下」の位置を基準にする(左端・幅はページごとの指定をそのまま使う)
  const initial = node.getBoundingClientRect();
  const width = initial.width;
  const left = initial.left;
  // 絶対配置の基準 = トリガ。上に開くときの位置決めに要る
  const anchor = (node.offsetParent as HTMLElement | null) ?? node.parentElement;

  // 直前に置いたときの中身の高さ(スクロール込み)。ResizeObserver が拾う変化のうち、
  // place 自身が付けた max-height による縮みを無視して、**中身が変わったときだけ**置き直す
  let lastContent = -1;

  function place() {
    const anchorRect = anchor?.getBoundingClientRect() ?? initial;
    node.style.position = "fixed";
    node.style.width = `${width}px`;
    node.style.maxHeight = "";
    node.style.overflowY = "";
    node.style.left = `${Math.max(margin, Math.min(left, window.innerWidth - width - margin))}px`;
    node.style.top = "0px";
    const height = node.getBoundingClientRect().height;
    const below = window.innerHeight - margin - (anchorRect.bottom + gap);
    const above = anchorRect.top - gap - margin;
    if (height <= below) {
      node.style.top = `${anchorRect.bottom + gap}px`;
    } else if (height <= above) {
      node.style.top = `${anchorRect.top - gap - height}px`;
    } else {
      // どちらにも入らない: 広いほうへ置いて、収まる高さで中身だけスクロールさせる
      const limit = Math.max(Math.max(below, above), 60);
      node.style.maxHeight = `${limit}px`;
      node.style.overflowY = "auto";
      node.style.top = below >= above ? `${anchorRect.bottom + gap}px` : `${Math.max(margin, anchorRect.top - gap - limit)}px`;
    }
    lastContent = node.scrollHeight;
  }

  place();
  // 一覧をスクロールするとトリガだけが動くので、付いて回らせる(capture=true で
  // どの入れ子のスクロールも拾う)
  const reposition = () => place();
  window.addEventListener("scroll", reposition, true);
  window.addEventListener("resize", reposition);
  // 中身が増えて背が伸びることがある(クラブ効果の対象ステを足すと値の行が増える)。
  // 開いたときの高さだけで決めると、伸びたぶんが画面外へ出る — 伸びた瞬間に置き直す
  const observer = new ResizeObserver(() => {
    if (node.scrollHeight !== lastContent) place();
  });
  observer.observe(node);
  return {
    destroy() {
      observer.disconnect();
      window.removeEventListener("scroll", reposition, true);
      window.removeEventListener("resize", reposition);
      node.style.position = "";
      node.style.width = "";
      node.style.left = "";
      node.style.top = "";
      node.style.maxHeight = "";
      node.style.overflowY = "";
    },
  };
}
