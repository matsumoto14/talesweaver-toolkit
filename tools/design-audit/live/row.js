const { chromium } = require("playwright-core");
(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  const wait = (ms = 900) => page.waitForTimeout(ms);
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1400);
  const box = async (loc) => { const b = await loc.boundingBox(); return b ? [Math.round(b.x), Math.round(b.y), Math.round(b.height)] : null; };
  const listBox = await page.locator(".src-list").boundingBox();
  for (const i of [0, 1, 2]) {
    const row = page.locator(".src-line").nth(i);
    const b = await box(row);
    const fully = b[1] >= listBox.y && b[1] + b[2] <= listBox.y + listBox.height;
    const before = b;
    await row.click();
    await wait(900);
    const after = await box(row);
    console.log(`行 ${i} (完全に見えている: ${fully}):`, JSON.stringify(before), "→", JSON.stringify(after),
      before[0] === after[0] && before[1] === after[1] ? "OK" : "NG");
  }
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
