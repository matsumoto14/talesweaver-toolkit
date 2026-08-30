// §10 規則 2「同時に 2 箇所が変わったら、2 箇所とも動かす」を網羅的に測る。
//
// 操作の前後で画面上の数値を**全部**記録し、
//   「値が変わったのに跳ねなかった」= 動かないほうが古い値に見える
// を列挙する。手で対象を挙げると漏れる(4 周目のサマリー行がそれ)。
const { chromium } = require("playwright-core");

/**
 * 数値要素に印を振って現在値を拾う。class 名で突き合わせると
 * 「同じ class の別要素」と混ざるので、要素そのものを追う。
 */
const SNAP = `(() => {
  const out = {};
  document.querySelectorAll(".num").forEach((el, i) => {
    if (el.querySelector(".num")) return;          // 入れ子は内側だけ見る
    if (el.closest("input")) return;
    const t = (el.textContent || "").trim();
    if (!/[0-9]/.test(t)) return;
    if (!el.dataset.auditId) el.dataset.auditId = "n" + i;
    out[el.dataset.auditId] = { text: t, cls: (el.className || "").toString().split(" ").filter((c) => !c.startsWith("s-")).join(".") };
  });
  return out;
})()`;

/** 跳ねた要素の印を記録する(子孫に付いたときは親の印を辿る) */
const WATCH = `(() => {
  window.__bumped = new Set();
  const mark = (el) => {
    let n = el;
    while (n && !n.dataset?.auditId) n = n.parentElement;
    if (n) window.__bumped.add(n.dataset.auditId);
  };
  new MutationObserver((ms) => {
    for (const m of ms) {
      const el = m.target;
      // 数値は跳ね(bump)、文字列の要約は弾み(badge-in)で見せる
      if (el.classList && (el.classList.contains("bump-up") || el.classList.contains("bump-down") || el.classList.contains("badge-in"))) mark(el);
    }
  }).observe(document.body, { subtree: true, attributes: true, attributeFilter: ["class"] });
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 1400) => page.waitForTimeout(ms);

  /** 操作して、変わった数値と跳ねた数値を突き合わせる */
  async function probe(where, act) {
    const before = await page.evaluate(SNAP);
    await page.evaluate(WATCH);
    await act();
    await wait(1800);
    const after = await page.evaluate(SNAP);
    const bumped = new Set(await page.evaluate(() => [...window.__bumped]));
    const changed = Object.entries(after).filter(([id, v]) => before[id] && before[id].text !== v.text);
    const missing = changed.filter(([id]) => !bumped.has(id));
    console.log(`  [${where}] 変わった ${changed.length} 件 / 跳ねた ${bumped.size} 件`);
    if (missing.length) {
      console.log(`      跳ねなかった: ${missing.map(([id, v]) => `.${v.cls || "?"}(${before[id].text}→${v.text})`).join(", ")}`);
    } else console.log("      変わったものはすべて跳ねた OK");
  }

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  // --- 計算タブ: 対象を切り替える(1 発・合計・1 秒あたりが同時に変わる)
  await page.locator("nav.tabs button", { hasText: "ダメージ計算" }).click({ force: true });
  await wait(1800);
  // 対象の切替は画面が入れ替わるので §10「変わった要素だけ動かす」の対象外。
  // **同じ対象のまま材料を変える**操作で測る。どちらも試し変更(sim)なので保存されない —
  // キャラタブの入力は自動保存で実データを壊すため、probe は計算タブの sim だけで組む
  await probe("計算・コンボ条件を切り替える", async () => {
    await page.locator(".combo .check input[type=checkbox]").first().dispatchEvent("click");
  });
  // 戻す
  await page.locator(".combo .check input[type=checkbox]").first().dispatchEvent("click").catch(() => {});
  await wait(700);

  // --- 極限スキルを入れ替える(1 発・合計・1 秒あたり・クリ率が同時に変わる)
  const ult = page.locator(".ultimate-chip:not([disabled])");
  if (await ult.count()) {
    await probe("計算・極限スキルを外す", async () => {
      await ult.first().dispatchEvent("click");
    });
    // 戻す
    await ult.first().dispatchEvent("click").catch(() => {});
    await wait(700);
  } else {
    console.log("  [計算・極限スキル] 押せるチップが無いので未実行");
  }

  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
