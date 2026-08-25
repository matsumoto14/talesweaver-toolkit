// §10 規則 2 — 同時に変わるものが全部動くか。キャラタブで HACK を変えて、
// サマリー行と最終能力値カードの両方が跳ねるかを見る。
const { chromium } = require("playwright-core");
(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  const wait = (ms = 900) => page.waitForTimeout(ms);
  await page.reload({ waitUntil: "load" });
  await wait(2600);
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1400);
  const names = await page.locator(".src-name").allInnerTexts();
  await page.locator(".src-line").nth(names.indexOf("調整")).click();
  await wait(1000);
  if (!(await page.locator(".sheet-body").count())) { await page.locator("button.sheet-trigger").click(); await wait(800); }
  await page.evaluate(() => {
    window.__b = [];
    new MutationObserver((ms) => ms.forEach((m) => {
      const el = m.target;
      if (el.classList && (el.classList.contains("bump-up") || el.classList.contains("bump-down"))) {
        const p = el.closest(".sheet-summary") ? "sheet-summary" : el.closest(".stat-grid") ? "stat-grid" : el.className;
        window.__b.push(p);
      }
    })).observe(document.body, { subtree: true, attributes: true, attributeFilter: ["class"] });
  });
  // §08 で「表示が既定・編集は例外」になったので、まず読み取り表示を押して編集に入る
  await page.locator(".adj-stat").nth(1).locator(".stat-input .read").first().click();
  await wait(600);
  const f = page.locator(".adj-stat").nth(1).locator(".num-field").first();
  await f.fill("777"); await f.dispatchEvent("blur");
  await wait(1800);
  console.log("同時に跳ねた場所:", JSON.stringify([...new Set(await page.evaluate(() => window.__b))]));
  await f.fill("0"); await f.dispatchEvent("blur");
  await wait(900);
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
