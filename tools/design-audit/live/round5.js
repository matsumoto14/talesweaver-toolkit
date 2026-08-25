// 5 周目。まだ実測していない観点を見る。
//  §00 未収録 / 未実装 / 未設定 を 0 や空白で埋めていないか
//  §06 アイコンが単独表示(名前なし)になっていないか
//  §03 斜線が「効いていない・持っていかれた」以外に使われていないか
//  §05 数値が M PLUS 1 Code + tabular-nums で出ているか
const { chromium } = require("playwright-core");

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 1000) => page.waitForTimeout(ms);

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  for (const tab of ["ホーム", "ダメージ計算", "キャラ"]) {
    await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
    await wait(1800);

    // §06 アイコン単独表示 — .icon の隣(同じ親の中)にテキストがあるか
    const lonely = await page.$$eval(".icon", (els) =>
      els
        .filter((e) => {
          const p = e.parentElement;
          if (!p) return false;
          const text = (p.textContent || "").replace(/\s/g, "");
          return text.length === 0;
        })
        .map((e) => e.className),
    );

    // §05 数値の書体 — .num が等幅 + tabular-nums になっているか
    const badNum = await page.$$eval(".num", (els) =>
      els
        .filter((e) => {
          const cs = getComputedStyle(e);
          return !/M PLUS 1 Code|monospace|ui-monospace/.test(cs.fontFamily) || !/tabular-nums/.test(cs.fontVariantNumeric);
        })
        .slice(0, 5)
        .map((e) => e.className + " :: " + getComputedStyle(e).fontFamily.slice(0, 30)),
    );

    // §03 斜線 — repeating-linear-gradient を使っている要素の用途
    const hatch = await page.$$eval("*", (els) =>
      [
        ...new Set(
          els
            .filter((e) => /repeating-linear-gradient/.test(getComputedStyle(e).backgroundImage))
            .map((e) => (e.className || "").toString().split(" ")[0] || e.tagName),
        ),
      ],
    );

    // §00 0 で埋めていないか — 「0」だけのセルがどれくらいあるか(表の実データは 0 でよい)
    const zeros = await page.$$eval("span, td, div", (els) =>
      els.filter((e) => e.children.length === 0 && e.textContent.trim() === "0").length,
    );
    const dashes = await page.$$eval("span, td, div", (els) =>
      els.filter((e) => e.children.length === 0 && /^(—|―|--)$/.test(e.textContent.trim())).length,
    );

    console.log(`[${tab}] アイコン単独: ${lonely.length} / 数値書体NG: ${badNum.length} ${JSON.stringify(badNum)}`);
    console.log(`        斜線の使用: ${JSON.stringify(hatch)} / 「0」表示 ${zeros} 件・「—」表示 ${dashes} 件`);
  }

  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
