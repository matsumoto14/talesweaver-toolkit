// §09 規則 4 — 桁が増えても隣が動かないか。実際に桁を変えて座標を測る。
const { chromium } = require("playwright-core");
(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  const wait = (ms = 900) => page.waitForTimeout(ms);
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1400);
  const names = await page.locator(".src-name").allInnerTexts();
  await page.locator(".src-line").nth(names.indexOf("調整")).click();
  await wait(1000);
  if (!(await page.locator(".sheet-body").count())) { await page.locator("button.sheet-trigger").click(); await wait(800); }
  const xs = async () => page.$$eval(".stat-cell", (els) => els.map((e) => Math.round(e.getBoundingClientRect().x)));
  const label = async () => (await page.locator(".stat-grid").innerText()).replace(/\n+/g, " ");
  const before = await xs();
  const l1 = await label();
  // §08 で「表示が既定・編集は例外」になったので、まず読み取り表示を押して編集に入る
  await page.locator(".adj-stat").nth(1).locator(".stat-input .read").first().click();
  await wait(600);
  const f = page.locator(".adj-stat").nth(1).locator(".num-field").first();
  await f.fill("9999"); await f.dispatchEvent("blur");
  await wait(1600);
  const after = await xs();
  const l2 = await label();
  console.log("before:", l1);
  console.log("after :", l2);
  console.log("stat-cell の x が不変か:", JSON.stringify(before) === JSON.stringify(after), JSON.stringify(before), JSON.stringify(after));
  await f.fill("0"); await f.dispatchEvent("blur");
  await wait(900);
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
