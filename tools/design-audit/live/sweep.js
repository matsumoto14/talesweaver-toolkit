// 押せる要素を**画面から自動列挙して全部押す**。手書きのリストだと漏れる
// (12 周目のバフ、14 周目のシエナがそれ)。
//
//  §09 規則 1 — 押した要素がその場に残るか(レイアウト上の位置で測る)
//  §09 規則 3 — 押しても周囲(直前・直後の要素)が動かないか
//
// 押すと「開く」ものだけでなく「状態が変わる」ものも含める。
// 意図した移動(★ で並べ替え・タブ切替・ドリルダウンの行)は別扱いにする。
const { chromium } = require("playwright-core");

/** 押せるものの探し方。role=button の div もキャラ一覧の行なので拾う */
const CLICKABLE =
  'button:not([disabled]), [role="button"], label.check, .src-line, .row';

/**
 * **押してはいけないもの。**保存・削除・登録は DB を変える。
 * 監査スクリプトが開発 DB を書き換えると、あとで「実装のバグ」と区別が付かなくなる。
 */
const DESTRUCTIVE = /保存|削除|登録|コピー|ぜんぶ戻す/;

/** 押すと画面が入れ替わるので、位置が変わって当たり前のもの */
const EXPECTED_TO_MOVE = [
  "tab", // タブ切替
  "fav", // ★ = 並べ替えを頼む操作(§09 規則 5)
  "pin",
  "arrange",
  "side-tab",
  "close-detail",
  "overlay",
  "part-row", // ドリルダウンは一覧が細くなるので幅は変わる(位置は変わらない)
];

const POS = `(el) => {
  if (!el || !el.isConnected) return null;
  // レイアウト上の位置。ビューポート座標だと click のスクロールで動いて見える
  let x = 0, y = 0, n = el;
  while (n && n.offsetParent) { x += n.offsetLeft; y += n.offsetTop; n = n.offsetParent; }
  return [x, y];
}`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms) => page.waitForTimeout(ms);

  const findings = [];

  /** いまの画面の押せる要素を全部押して、押した要素と隣の位置を測る */
  async function sweep(where, opts = {}) {
    const skip = new Set(opts.skip ?? []);
    const n = await page.locator(CLICKABLE).count();
    for (let i = 0; i < n; i++) {
      const info = await page.locator(CLICKABLE).nth(i).evaluate((el) => ({
        cls: (el.className || "").toString().split(" ").filter((c) => !c.startsWith("s-")).join("."),
        text: (el.textContent || "").replace(/\s+/g, " ").trim().slice(0, 18),
      })).catch(() => null);
      if (!info) continue;
      const key = `${info.cls}|${info.text}`;
      if (skip.has(info.cls)) continue;
      if (DESTRUCTIVE.test(info.text)) continue;
      if (EXPECTED_TO_MOVE.some((c) => info.cls.split(".").includes(c))) continue;

      const before = await page.locator(CLICKABLE).nth(i).evaluate(POS).catch(() => null);
      const nextBefore = await page.locator(CLICKABLE).nth(i + 1).evaluate(POS).catch(() => null);
      if (!before) continue;
      await page.locator(CLICKABLE).nth(i).dispatchEvent("click").catch(() => {});
      await wait(420);
      const after = await page.locator(CLICKABLE).nth(i).evaluate(POS).catch(() => null);
      const nextAfter = await page.locator(CLICKABLE).nth(i + 1).evaluate(POS).catch(() => null);
      if (after && (before[0] !== after[0] || before[1] !== after[1])) {
        findings.push(`[${where}] 押した要素が動いた .${info.cls} 「${info.text}」 ${JSON.stringify(before)} → ${JSON.stringify(after)}`);
      } else if (nextBefore && nextAfter && (nextBefore[0] !== nextAfter[0] || nextBefore[1] !== nextAfter[1])) {
        findings.push(`[${where}] 押したら次の要素が動いた .${info.cls} 「${info.text}」 ${JSON.stringify(nextBefore)} → ${JSON.stringify(nextAfter)}`);
      }
      // 押した状態を戻す(トグルなら 2 回目で戻る)
      await page.locator(CLICKABLE).nth(i).dispatchEvent("click").catch(() => {});
      await wait(320);
    }
    console.log(`  [${where}] ${n} 個を押した`);
  }

  const goTab = async (tab) => {
    await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
    await wait(1600);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  await goTab("ホーム");
  await sweep("ホーム");

  await goTab("ダメージ計算");
  await sweep("計算・攻撃");

  await goTab("キャラ");
  await wait(600);
  const panes = await page.locator(".src-name").allInnerTexts();
  for (let i = 0; i < panes.length; i++) {
    await page.locator(".src-line").nth(i).dispatchEvent("click");
    await wait(900);
    await sweep(`キャラ・${panes[i]}`, { skip: ["src-line", "src-name", "fav"] });
  }

  console.log("");
  if (findings.length === 0) console.log("§09 規則 1・3: 全経路 OK");
  else findings.forEach((f) => console.log("NG " + f));
  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
