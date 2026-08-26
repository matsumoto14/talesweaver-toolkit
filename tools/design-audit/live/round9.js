// 9 周目。寸法の規格(§02 帯の高さ / §05 実寸のスケール / §01 面の 3 層)を実測する。
const { chromium } = require("playwright-core");

const SCALE = [44, 40, 27, 19, 15, 14, 13, 12.5, 12, 11.5, 11, 10.5, 10, 9.5, 9, 8.5];
// アイコン内の記号は §06 の管轄(font-size は icon-size の比率)なので段の外でよい
const ICON_GLYPH = [16.8, 16, 11.76, 8.4];

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 1000) => page.waitForTimeout(ms);
  await page.reload({ waitUntil: "load" });
  await wait(2600);

  // §02 帯の高さ — アプリ帯 52px / カラム見出し 30px
  const bands = await page.evaluate(() => ({
    topbar: Math.round(document.querySelector(".topbar")?.getBoundingClientRect().height ?? 0),
    headBars: [...new Set([...document.querySelectorAll(".head-bar")].map((e) => Math.round(e.getBoundingClientRect().height)))],
  }));
  console.log("§02 アプリ帯:", bands.topbar, "px(規格 52) / カラム見出し:", JSON.stringify(bands.headBars), "(規格 30)");

  // §05 実寸のスケール — 使われている font-size が規格の段に収まっているか
  for (const tab of ["ホーム", "ダメージ計算", "キャラ"]) {
    await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
    await wait(1600);
    const sizes = await page.evaluate(() =>
      [...new Set([...document.querySelectorAll("*")]
        .filter((e) => e.children.length === 0 && (e.textContent || "").trim().length > 0)
        .map((e) => parseFloat(getComputedStyle(e).fontSize)))].sort((a, b) => b - a),
    );
    const off = sizes.filter((s) => !SCALE.includes(s) && !ICON_GLYPH.includes(s));
    console.log(`[${tab}] font-size 段の外: ${off.length === 0 ? "なし" : JSON.stringify(off)}  (使用中: ${JSON.stringify(sizes)})`);
  }

  // §01 面の 3 層 — 地 / 窓 / カード / インセット の色が規格どおりか
  const surfaces = await page.evaluate(() => {
    const get = (sel) => { const e = document.querySelector(sel); return e ? getComputedStyle(e).backgroundColor : null; };
    return { body: getComputedStyle(document.body).backgroundColor, card: get(".card"), inset: get(".inset, .stat-grid") };
  });
  console.log("§01 面:", JSON.stringify(surfaces), "(地 rgb(201,216,238) / カード rgb(255,255,255) / インセット rgb(193,211,230))");

  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
