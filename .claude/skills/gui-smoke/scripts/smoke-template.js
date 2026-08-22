// スモークテストの雛形。スクラッチパッドにコピーして確認項目を書く。
// 実行: NODE_PATH=/c/github/private/twtoolkit/node_modules node smoke.js
const { chromium } = require("playwright-core");

const OUT = "C:/github/private/talesweaver-toolkit/docs/screenshots/";
const log = (...a) => console.log(...a);

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errors = [];
  page.on("pageerror", (e) => errors.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errors.push("CONSOLE.ERROR " + m.text()); });

  // ---- helpers
  const shot = (name) => page.screenshot({ path: OUT + name, fullPage: true });
  const nav = (label) => page.locator("nav button", { hasText: label }).click();
  const toastText = async () => (await page.locator(".toast").count()) ? await page.locator(".toast").innerText() : null;
  const byLabel = (scope, label) => scope.locator(".label", { hasText: new RegExp("^" + label + "$") });
  const field = (scope, label) => scope.locator("label.field", { has: byLabel(page, label) }).locator("input");
  const selectByLabel = (scope, label) => scope.locator("label.select", { has: byLabel(page, label) }).locator("select");
  const setNum = async (loc, v) => { await loc.fill(String(v)); await loc.dispatchEvent("blur"); };
  const setRange = (loc, v) => loc.evaluate((el, v) => {
    el.value = String(v);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }, v);
  const groupHead = (title) => page.locator(".group-head", { has: page.locator(".group-title", { hasText: title }) });

  // ---- 確認項目(例)
  await nav("キャラ管理");
  await page.waitForTimeout(300);
  log("title:", await page.title());
  await shot("99-smoke-example.png");

  // ---- 結果
  log(errors.length ? "ERRORS:\n" + errors.join("\n") : "no page/console errors");
  await browser.close(); // CDP 接続を切るだけ。アプリは終了しない
})().catch((e) => { console.error("FAILED", e); process.exit(1); });
