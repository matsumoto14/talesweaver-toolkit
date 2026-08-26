// §09 規則 1 を「押すと状態が変わる要素」で総当たりする。
// 12 周目の見落とし — round4.js は「押すと**開く**もの」しか測っていなかった。
// ここでは押せる要素を片っ端から押して、押した要素自身の座標が動かないかを見る。
const { chromium } = require("playwright-core");

const TARGETS = {
  ダメージ計算: [
    ["攻撃 / 防御タブ", "button.side-tab"],
    ["対象の ◀▶", "button.step"],
    ["段階選択(ストロングW)", ".seg .step"],
    ["バフチップ", ".buff-chip:not([disabled])"],
    ["候補(足りない分)", "button.whatif"],
    ["パワーウェポン", "label.pw"],
  ],
  ホーム: [
    ["お気に入りの ★", "button.pin"],
    ["エリアの折りたたみ", "button.area-toggle"],
    ["到達一覧の行", ".row"],
  ],
  キャラ: [
    ["補正源の行", ".src-line"],
    ["段階選択", ".seg .step", "キャラステータス"],
    ["数値の編集(行の位置)", ".stepper", "キャラステータス"],
    ["部位の行", "button.part-row", "装備"],
    ["チップ(オン/オフ)", "label.check", "クリティカル率"],
  ],
};

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 1100) => page.waitForTimeout(ms);
  // ビューポート座標だと、押した拍子のスクロール(Playwright の click も要素を視界に入れる)で
  // 動いて見える。**レイアウト上の位置**(offsetParent からの距離)で測る
  const box = async (loc) =>
    loc.evaluate((e) => (e.offsetParent === null ? null : [e.offsetLeft, e.offsetTop])).catch(() => null);
  const resetSim = async () => {
    const r = page.locator("button.btn", { hasText: "ぜんぶ戻す" });
    if ((await r.count()) && (await r.first().isEnabled())) { await r.first().click(); await wait(1300); }
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  let ng = 0;
  for (const [tab, list] of Object.entries(TARGETS)) {
    for (const [name, sel, pane] of list) {
      // 前の測定で開いたペイン・スクロール位置が残っていると座標が動いて見える。
      // 1 件ごとにリロードして同じ初期状態から測る
      await page.reload({ waitUntil: "load" });
      await wait(2400);
      await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
      await wait(1700);
      await resetSim();
      if (pane) {
        // 補正源ペインを指定して開く(そのペインにしか無い部品を測るため)
        const names = await page.locator(".src-name").allInnerTexts();
        const i = names.indexOf(pane);
        if (i >= 0) {
          const row = page.locator(".src-line").nth(i);
          await row.scrollIntoViewIfNeeded().catch(() => {});
          await row.click({ force: true });
          await wait(1200);
        }
      }
      const all = page.locator(sel);
      const n = await all.count();
      if (n === 0) { console.log(`  [${tab}] ${name}: 見つからず`); continue; }
      // 完全に見えている要素を選ぶ(見切れているとフォーカスで視界に入れるためにスクロールする)
      let target = null;
      for (let i = 0; i < Math.min(n, 8); i++) {
        const c = all.nth(i);
        const b = await c.boundingBox();
        if (b && b.y > 90 && b.y + b.height < 800 && (await c.isEnabled().catch(() => true))) { target = c; break; }
      }
      if (!target) { console.log(`  [${tab}] ${name}: 完全に見えているものが無い`); continue; }
      const before = await box(target);
      await target.dispatchEvent("click").catch(() => {});
      await wait(1300);
      const after = await box(target);
      if (!after) { console.log(`  [${tab}] ${name}: 押したあと消えた(要確認)`); ng++; continue; }
      const ok = before[0] === after[0] && before[1] === after[1];
      if (!ok) ng++;
      console.log(`  [${tab}] ${name}: ${JSON.stringify(before)} → ${JSON.stringify(after)} ${ok ? "OK" : `NG(${after[0] - before[0]},${after[1] - before[1]})`}`);
    }
  }
  await resetSim();
  console.log(ng === 0 ? "§09 規則 1: 全経路 OK" : `§09 規則 1: ${ng} 件 NG`);
  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
