// 試し変更に入る全経路で、押した要素がその場に残るか(§09 規則 1)
const { chromium } = require("playwright-core");
const OUT = "C:/github/private/talesweaver-toolkit/docs/screenshots/";
(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 1000) => page.waitForTimeout(ms);
  const box = async (loc) => { const b = await loc.boundingBox(); return b ? [Math.round(b.x), Math.round(b.y)] : null; };
  const reset = async () => {
    const r = page.locator("button.btn", { hasText: "ぜんぶ戻す" });
    if (await r.count() && await r.first().isEnabled()) { await r.first().click(); await wait(1300); }
  };
  await page.reload({ waitUntil: "load" });
  await wait(2600);
  // レールが開いていると右カラムが狭くなるので畳んでから測る
  await page.evaluate(() => {
    const b = [...document.querySelectorAll("button")].find((e) => /^[‹«]$/.test(e.textContent.trim()));
    b?.click();
  });
  await wait(800);
  await page.locator("nav.tabs button", { hasText: "ダメージ計算" }).click({ force: true });
  await wait(1800);

  for (const [name, sel] of [
    ["バフチップ", ".buff-chip:not([disabled])"],
    ["パワーウェポン", "label.pw"],
  ]) {
    await reset();
    const loc = page.locator(sel).nth(name === "バフチップ" ? 3 : 0);
    if (!(await loc.count())) { console.log(`[${name}] 見つからず`); continue; }
    await loc.scrollIntoViewIfNeeded();
    const before = await box(loc);
    await loc.click({ force: true });
    await wait(1400);
    const after = await box(loc);
    const ok = before[0] === after[0] && before[1] === after[1];
    console.log(`[${name}] ${JSON.stringify(before)} → ${JSON.stringify(after)} ${ok ? "OK" : `NG(${after[0] - before[0]},${after[1] - before[1]})`}`);
  }
  await page.screenshot({ path: OUT + "142-sim-no-shift.png" });
  await reset();
  console.log(errs.length ? errs.join("\n") : "no errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
