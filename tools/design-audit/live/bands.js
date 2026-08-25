// §02「意味のある帯(金・ラベンダー)は 1 画面に同時に 2 種まで」を全画面で測る。
// 3 種目が欲しくなったら、それは階層を作り直すべきサイン。
//
// あわせて §03「予約色を用途外に使っていないか」を、面の色から機械的に見る:
//   金 = 操作待ち・上限・結果 / ラベンダー = 保存されない・一時的なもの
const { chromium } = require("playwright-core");

const CHECK = `(() => {
  const GOLD = /194, 160, 87|242, 227, 189|240, 215, 154|253, 249, 238|224, 201, 138|227, 203, 147/;
  const LAV = /109, 106, 168|86, 83, 148|239, 238, 248|195, 193, 228|247, 246, 252/;
  // 帯 = 横に広く、面の色を持つブロック。ボタンやチップは帯ではない
  const bands = [...document.querySelectorAll("*")].filter((e) => {
    const r = e.getBoundingClientRect();
    if (r.width < 200 || r.height < 14 || r.height > 90) return false;
    if (e.matches("button, [role=button], label, input")) return false;
    const cs = getComputedStyle(e);
    return (cs.backgroundImage && cs.backgroundImage !== "none") || cs.backgroundColor !== "rgba(0, 0, 0, 0)";
  });
  const label = (e) => (e.className || "").toString().split(" ").filter((c) => !c.startsWith("s-")).join(".") || e.tagName;
  const gold = [], lav = [];
  for (const e of bands) {
    const cs = getComputedStyle(e);
    const paint = cs.backgroundImage + " " + cs.backgroundColor + " " + cs.borderColor;
    if (GOLD.test(paint)) gold.push(label(e));
    else if (LAV.test(paint)) lav.push(label(e));
  }
  return { gold: [...new Set(gold)], lav: [...new Set(lav)] };
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 1100) => page.waitForTimeout(ms);
  const check = async (where) => {
    const r = await page.evaluate(CHECK);
    const kinds = (r.gold.length ? 1 : 0) + (r.lav.length ? 1 : 0);
    const ng = kinds > 2;
    console.log(`  [${where}] 金 ${r.gold.length} / ラベンダー ${r.lav.length} → 意味のある帯 ${kinds} 種 ${ng ? "NG" : "OK"}`);
    if (r.gold.length) console.log(`      金: ${r.gold.join(", ")}`);
    if (r.lav.length) console.log(`      ラベンダー: ${r.lav.join(", ")}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  for (const tab of ["ホーム", "ダメージ計算", "キャラ"]) {
    await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
    await wait(1800);
    await check(tab);
  }

  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
