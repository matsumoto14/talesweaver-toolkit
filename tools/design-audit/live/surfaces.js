// §01「面と階層」を全画面で測る。
//
//  白 = 編集できる面 / インセット = 読み取り専用。
//  「読み取り専用の数値が白い箱に入っていないか」を、実際の背景色で見る。
//
// 判定: **編集できる部品を含まない**枠(表・トレース・値の並び)の面が白なら候補。
// 「押すと編集に入る」読み取り表示(.value-box.read)はインセットなので白ではない。
const { chromium } = require("playwright-core");

const CHECK = `(() => {
  const bg = (el) => {
    let n = el;
    while (n) { const c = getComputedStyle(n).backgroundColor; if (c && c !== "rgba(0, 0, 0, 0)") return c; n = n.parentElement; }
    return "?";
  };
  const WHITE = "rgb(255, 255, 255)";
  // 押せる・打てるもの。これを含む枠は「編集できる面」なので白で正しい
  const EDITABLE = "input, select, textarea, button, [role=\\"button\\"], label";
  const out = {};
  document.querySelectorAll("table.grid tbody td, .num").forEach((el) => {
    const text = (el.textContent || "").trim();
    if (!/[0-9]/.test(text)) return;
    // §01 の「箱」= 枠を持つ面。裸のテキストは箱ではない
    // 点線は区切り線であって箱ではない。カード(.card / .sheet-card)は
    // 「内容の器」で §01 の白なので、その中の裸のテキストは対象外
    let box = el;
    while (box) {
      const cs = getComputedStyle(box);
      if (cs.borderTopWidth !== "0px" && cs.borderTopStyle === "solid") break;
      box = box.parentElement;
    }
    if (!box || box === document.body) return;
    if (box.classList.contains("card") || box.classList.contains("sheet-card")) return;
    if (box.matches(EDITABLE) || box.querySelector(EDITABLE)) return;  // 編集できる面
    if (bg(box) !== WHITE) return;
    const k = (box.className || "").toString().split(" ").filter((c) => !c.startsWith("s-")).join(".") || box.tagName;
    out[k] = (out[k] || 0) + 1;
  });
  return out;
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 1100) => page.waitForTimeout(ms);
  const check = async (where) => {
    const r = await page.evaluate(CHECK);
    const n = Object.values(r).reduce((a, b) => a + b, 0);
    console.log(`  [${where}] 白い面の読み取り数値 ${n} 件 ${n ? JSON.stringify(r) : ""}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  await page.locator("nav.tabs button", { hasText: "ホーム" }).click({ force: true });
  await wait(1600);
  await check("ホーム");

  await page.locator("nav.tabs button", { hasText: "ダメージ計算" }).click({ force: true });
  await wait(1800);
  await check("計算・攻撃");
  const open = page.locator("button, summary").filter({ hasText: "内訳をひらく" });
  if (await open.count()) { await open.first().dispatchEvent("click"); await wait(1000); await check("計算・なぜこの数字"); }
  const def = page.locator("button", { hasText: /^防御$/ });
  if (await def.count()) { await def.first().dispatchEvent("click"); await wait(1300); await check("計算・防御"); }

  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1500);
  const panes = await page.locator(".src-name").allInnerTexts();
  for (let i = 0; i < panes.length; i++) {
    await page.locator(".src-line").nth(i).dispatchEvent("click");
    await wait(800);
    await check(`キャラ・${panes[i]}`);
  }

  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
