// 6 周目。まだ通していない画面を全部開いて、監査チェックリストの項目を機械的に測る。
const { chromium } = require("playwright-core");
const OUT = "C:/github/private/talesweaver-toolkit/docs/screenshots/";

const CHECK = `(() => {
  const out = { icon單: 0, numNG: [], radius: [], hatch: [], white: [] };
  // §06 アイコン単独表示(名前の併記なし)。title があれば例外(レール折りたたみ)
  out.icon単 = [...document.querySelectorAll(".icon")].filter((e) => {
    const p = e.parentElement;
    const text = (p?.textContent || "").replace(/\\s/g, "");
    return text.length === 0 && !e.getAttribute("title");
  }).length;
  // §05 数値の書体
  out.numNG = [...document.querySelectorAll(".num")].filter((e) => {
    const cs = getComputedStyle(e);
    return !/M PLUS 1 Code|monospace/.test(cs.fontFamily) || !/tabular-nums/.test(cs.fontVariantNumeric);
  }).slice(0, 4).map((e) => (e.className || "").toString().split(" ")[0]);
  // §04 角丸が 4 段の外(50% と 0 は除く)
  const OK = new Set(["12px", "9px", "6px", "999px", "0px", "50%"]);
  out.radius = [...new Set([...document.querySelectorAll("*")]
    .map((e) => getComputedStyle(e).borderRadius)
    .filter((r) => r && r !== "0px" && !r.split(" ").every((v) => OK.has(v))))].slice(0, 6);
  // §10 0.5s を超える動き
  out.slow = [...new Set([...document.querySelectorAll("*")].flatMap((e) => {
    const cs = getComputedStyle(e);
    return [cs.transitionDuration, cs.animationDuration].flatMap((d) => d.split(", "))
      .filter((d) => parseFloat(d) > 0.5);
  }))].slice(0, 4);
  return out;
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 900) => page.waitForTimeout(ms);
  const check = async (where) => {
    const r = await page.evaluate(CHECK);
    const bad = r.icon単 > 0 || r.numNG.length > 0 || r.radius.length > 0 || (r.slow || []).length > 0;
    console.log(`[${where}] ${bad ? "NG " + JSON.stringify(r) : "OK"}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  // ---- 計算タブ: 攻撃 / 防御 / トレース
  await page.locator("nav.tabs button", { hasText: "ダメージ計算" }).click({ force: true });
  await wait(1800);
  await check("計算・攻撃");
  const open = page.locator("button, summary").filter({ hasText: "内訳をひらく" });
  if (await open.count()) { await open.first().click(); await wait(900); }
  await check("計算・なぜこの数字");
  const tr = page.locator("details.trace, summary").filter({ hasText: "詳細トレース" });
  if (await tr.count()) { await tr.first().click(); await wait(900); await check("計算・トレース"); }
  await page.locator("button", { hasText: /^防御$/ }).first().click();
  await wait(1200);
  await check("計算・防御");

  // ---- キャラタブ: 補正源を全部開く
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1500);
  const n = await page.locator(".src-line").count();
  let ng = 0;
  for (let i = 0; i < n; i++) {
    const row = page.locator(".src-line").nth(i);
    await row.scrollIntoViewIfNeeded().catch(() => {});
    await row.click({ force: true });
    await wait(500);
    const r = await page.evaluate(CHECK);
    if (r.icon単 > 0 || r.numNG.length > 0 || r.radius.length > 0 || (r.slow || []).length > 0) {
      ng++;
      console.log(`  [補正源 ${i}] NG ${JSON.stringify(r)}`);
    }
  }
  console.log(`[キャラ・補正源 ${n} ペイン] ${ng === 0 ? "OK" : ng + " 件 NG"}`);

  // ---- キャラレールを畳む(§06 の例外: title が付いているか)
  await page.evaluate(() => {
    const b = [...document.querySelectorAll("button")].find((e) => /^[›‹»«]$/.test(e.textContent.trim()));
    b?.click();
  });
  await wait(900);
  await check("レール折りたたみ");

  // ---- ホーム
  await page.locator("nav.tabs button", { hasText: "ホーム" }).click({ force: true });
  await wait(1800);
  await check("ホーム");

  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
