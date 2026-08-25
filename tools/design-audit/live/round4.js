// 4 周目の実機監査。押した場所が動かないか(§09 規則 1)、重なるものがレイアウトを
// 押さないか(規則 3)、同時に変わるものが全部動くか(§10 規則 2)を実測する。
const { chromium } = require("playwright-core");

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 900) => page.waitForTimeout(ms);
  const box = async (loc) => { const b = await loc.boundingBox(); return b ? [Math.round(b.x), Math.round(b.y)] : null; };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  // ---- §10 規則 2: 同時に変わるものが全部動くか(1 発 / 合計 / 1 秒あたり)
  await page.locator("nav.tabs button", { hasText: "ダメージ計算" }).click({ force: true });
  await wait(1800);
  await page.evaluate(() => {
    window.__b = [];
    const NAMES = ["bump-up", "bump-down"];
    const note = (el) => {
      if (!el || !el.classList) return;
      for (const c of NAMES) if (el.classList.contains(c)) window.__b.push(el.className.split(" ").slice(0, 3).join("."));
    };
    new MutationObserver((ms) => ms.forEach((m) => note(m.target))).observe(document.body, {
      subtree: true, attributes: true, attributeFilter: ["class"],
    });
  });
  // 対象を切り替えると 1 発・合計・1 秒あたりが同時に変わる
  await page.locator("button.step").nth(1).click();
  await wait(2200);
  const bumped = await page.evaluate(() => [...new Set(window.__b)]);
  console.log("§10 同時に跳ねた数値:", JSON.stringify(bumped));

  // ---- §09 規則 1 / 規則 3: ポップオーバーを開いて、押したトリガと周囲が動かないか
  const trigger = page.locator("button.target-trigger").first();
  const skillRow = page.locator(".skill-row").first();
  const t0 = await box(trigger);
  const s0 = await box(skillRow);
  await trigger.click();
  await wait(700);
  const t1 = await box(trigger);
  const s1 = await box(skillRow);
  console.log("§09-1 押したトリガの位置:", JSON.stringify(t0), "→", JSON.stringify(t1), t0[0] === t1[0] && t0[1] === t1[1] ? "OK" : "NG");
  console.log("§09-3 直下の要素の位置:", JSON.stringify(s0), "→", JSON.stringify(s1), s0[0] === s1[0] && s0[1] === s1[1] ? "OK" : "NG");
  await page.evaluate(() => document.querySelector("button.overlay")?.click());
  await wait(700);

  // ---- §09 規則 1: 「なぜこの数字?」を開いて、押したヘッダが動かないか
  const head = page.locator("button.panel-head.blue").first();
  const h0 = await box(head);
  await head.click();
  await wait(900);
  const h1 = await box(head);
  console.log("§09-1 開閉ヘッダの位置:", JSON.stringify(h0), "→", JSON.stringify(h1), h0[1] === h1[1] ? "OK" : "NG");

  // ---- キャラタブ: 補正源を押したとき、押した行が動かないか
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1500);
  // 見切れている行を押すとブラウザがフォーカスを視界に入れてスクロールする。
  // それは §09 規則 1 の違反ではないので、**完全に見えている行**で測る
  const row = page.locator(".src-line").nth(1);
  const r0 = await box(row);
  await row.click();
  await wait(900);
  const r1 = await box(row);
  console.log("§09-1 押した補正源の位置:", JSON.stringify(r0), "→", JSON.stringify(r1), r0[0] === r1[0] && r0[1] === r1[1] ? "OK" : "NG");

  // ---- §07: 「適用」ボタンを挟んでいないか
  const applyish = await page.evaluate(() =>
    [...document.querySelectorAll("button")].map((b) => b.textContent.trim()).filter((s) => /^(適用|反映|更新|OK)$/.test(s)),
  );
  console.log("§07 適用ボタン:", applyish.length === 0 ? "なし OK" : JSON.stringify(applyish));

  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
