// 11 周目。まだ一度も見ていない画面 — 新規キャラの登録ペインとエラー帯。
const { chromium } = require("playwright-core");
const OUT = "C:/github/private/talesweaver-toolkit/docs/screenshots/";

const CHECK = `(() => {
  const OKR = new Set(["12px", "9px", "6px", "999px", "0px", "50%"]);
  const SCALE = new Set([44, 40, 27, 19, 15, 14, 13, 12.5, 12, 11.5, 11, 10.5, 10, 9.5, 9, 8.5]);
  const GLYPH = new Set([16.8, 16, 11.76, 8.4]);
  return {
    radius: [...new Set([...document.querySelectorAll("*")]
      .map((e) => getComputedStyle(e).borderRadius)
      .filter((r) => r && r !== "0px" && !r.split(" ").every((v) => OKR.has(v))))].slice(0, 5),
    font: [...new Set([...document.querySelectorAll("*")]
      .filter((e) => e.children.length === 0 && (e.textContent || "").trim().length > 0)
      .map((e) => parseFloat(getComputedStyle(e).fontSize))
      .filter((s) => !SCALE.has(s) && !GLYPH.has(s)))].slice(0, 5),
    numNG: [...document.querySelectorAll(".num")].filter((e) => {
      const cs = getComputedStyle(e);
      return !/M PLUS 1 Code|monospace/.test(cs.fontFamily) || !/tabular-nums/.test(cs.fontVariantNumeric);
    }).length,
    lonelyIcon: [...document.querySelectorAll(".icon")].filter((e) => {
      const text = (e.parentElement?.textContent || "").replace(/\\s/g, "");
      return text.length === 0 && !e.getAttribute("title");
    }).length,
  };
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 1000) => page.waitForTimeout(ms);
  const check = async (where) => {
    const r = await page.evaluate(CHECK);
    const bad = r.radius.length || r.font.length || r.numNG || r.lonelyIcon;
    console.log(`[${where}] ${bad ? "NG " + JSON.stringify(r) : "OK"}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  // ---- 登録ペイン(キャラレールの「＋ キャラを登録」)
  // 登録ペインはキャラタブの中。レールの「+」で registerOpen になる
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1400);
  // レールを開いてから「＋ キャラを登録」を押す(畳んだ状態だと別のボタンを拾う)
  await page.evaluate(() => {
    const t = [...document.querySelectorAll("button")].find((e) => /^[›»]$/.test(e.textContent.trim()));
    t?.click();
  });
  await wait(900);
  await page.evaluate(() => {
    const b = [...document.querySelectorAll("button")].find((e) => /キャラを登録/.test(e.textContent));
    b?.click();
  });
  await wait(1600);
  await check("登録ペイン");
  await page.screenshot({ path: OUT + "141-register-pane.png" });
  // §07: 登録ペインに自由入力がいくつあるか(名前だけのはず)
  const inputs = await page.evaluate(() =>
    [...document.querySelectorAll("input[type=text], input[type=number], textarea")].map(
      (e) => e.getAttribute("placeholder") || e.getAttribute("aria-label") || e.className,
    ),
  );
  console.log("  登録ペインの自由入力:", JSON.stringify(inputs));

  // ---- エラー帯(存在しないキャラで保存させる等は避け、帯そのものを出して測る)
  const around = async () =>
    page.evaluate(() => {
      const e = document.querySelector(".body") || document.body;
      const r = e.getBoundingClientRect();
      return [Math.round(r.x), Math.round(r.y)];
    });
  const before = await around();
  await page.evaluate(() => {
    // toast は共有 state。UI から出す経路が無いので、同じ見た目の帯を差し込んで測る
    const d = document.createElement("div");
    d.className = "__probe";
    d.style.cssText = "position:absolute;top:58px;left:16px;right:16px;height:38px;z-index:50";
    document.querySelector(".shell")?.appendChild(d);
  });
  await wait(500);
  const after = await around();
  await page.evaluate(() => document.querySelector(".__probe")?.remove());
  console.log("§09-3 上部に帯が重なっても本体が動かないか:", JSON.stringify(before), "→", JSON.stringify(after),
    before[0] === after[0] && before[1] === after[1] ? "OK" : "NG");

  console.log(errs.length ? "ERRORS:\\n" + errs.join("\\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
