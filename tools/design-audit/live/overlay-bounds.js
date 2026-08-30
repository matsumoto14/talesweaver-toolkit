// §09 規則 3 の見逃し対策: round4.js が見るのは「枠に対して内容がはみ出しているか」
// だけで、重なるもの(ポップオーバー・候補リスト・ポップアップ)が**ビューポートの外**に
// 開いているケースを捕まえられない。DOM 上は display:flex; visibility:visible; opacity:1
// でも、ビューポート下端より下に開いていれば実質誰にも見えない(バフタブ「ほか n」の実例)。
// ここでは「押して開く重なりもの」を実際に開き、getBoundingClientRect() が
// ビューポート内に収まっているかを測る。
const { chromium } = require("playwright-core");

// タブ名・開閉トリガの selector・重なりもの本体の selector の組。
// 画面を足したらここにも足す(README の注意と同じ理由: 巡回に無い画面は永久に測られない)。
const CASES = [
  {
    tab: "バフ",
    label: "バフ『ほか n』ポップオーバー",
    // 一番下寄りの ON チップで開くほど画面外に出やすいので末尾から探す
    open: async (page) => {
      const links = page.locator(".rest-link");
      const n = await links.count();
      if (n === 0) return false;
      await links.nth(n - 1).scrollIntoViewIfNeeded();
      await links.nth(n - 1).click();
      return true;
    },
    overlay: ".rest-popover",
  },
  {
    tab: "ダメージ計算",
    label: "計算『対象』ポップオーバー",
    // 開くだけ。行は選ばない(選ぶと計算対象が変わる)
    open: async (page) => {
      const t = page.locator(".target-trigger");
      if (await t.count() === 0) return false;
      await t.first().click();
      return true;
    },
    overlay: ".pop",
  },
  {
    tab: "ダメージ計算",
    label: "計算『スキル』ポップオーバー",
    open: async (page) => {
      const t = page.locator(".skill-trigger");
      if (await t.count() === 0) return false;
      await t.first().click();
      return true;
    },
    overlay: ".pop.gold",
  },
];

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 900) => page.waitForTimeout(ms);

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  const results = [];
  for (const c of CASES) {
    await page.locator("nav.tabs button", { hasText: c.tab }).click({ force: true });
    await wait(1200);
    const opened = await c.open(page);
    if (!opened) { results.push([c.label, "SKIP(対象なし)"]); continue; }
    await wait(500);
    const rect = await page.locator(c.overlay).first().evaluate((el) => {
      const r = el.getBoundingClientRect();
      return { top: r.top, bottom: r.bottom, left: r.left, right: r.right };
    }).catch(() => null);
    if (!rect) { results.push([c.label, "SKIP(要素なし)"]); continue; }
    const vw = await page.evaluate(() => window.innerWidth);
    const vh = await page.evaluate(() => window.innerHeight);
    const overflowBottom = Math.max(0, rect.bottom - vh);
    const overflowTop = Math.max(0, -rect.top);
    const overflowRight = Math.max(0, rect.right - vw);
    const overflowLeft = Math.max(0, -rect.left);
    const ok = overflowBottom === 0 && overflowTop === 0 && overflowRight === 0 && overflowLeft === 0;
    results.push([
      c.label,
      ok ? "OK 画面内" : `NG 画面外 top=${rect.top.toFixed(0)} bottom=${rect.bottom.toFixed(0)} vh=${vh}` +
        (overflowBottom ? ` 下に${overflowBottom.toFixed(0)}px超過` : "") +
        (overflowTop ? ` 上に${overflowTop.toFixed(0)}px超過` : "") +
        (overflowRight ? ` 右に${overflowRight.toFixed(0)}px超過` : "") +
        (overflowLeft ? ` 左に${overflowLeft.toFixed(0)}px超過` : ""),
    ]);
  }

  console.log("§09-3 重なるものはビューポート内に収まっているか:");
  for (const [label, verdict] of results) console.log(`  ${label}: ${verdict}`);

  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
