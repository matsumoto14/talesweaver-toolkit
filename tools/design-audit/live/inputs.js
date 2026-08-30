// §07「入力は 5 形態の上から順に試す」を全画面で数える。
//
//   1 自動(表示だけ) / 2 段階選択 / 3 チップ / 4 ステッパー / 5 自由入力
//
// 素の <select> は 5 形態のどれでもない。並べられる数の選択肢がドロップダウンに
// 入っていたら、それは段階選択に降ろせていないということ。
// 自由入力(text / number)は「ここまで降りたら理由を書く」なので、件数を見せる。
const { chromium } = require("playwright-core");

const CHECK = `(() => {
  const label = (el) => {
    const l = el.closest("label")?.querySelector(".label")?.textContent
      || el.getAttribute("aria-label")
      || el.getAttribute("placeholder")
      || el.previousElementSibling?.textContent
      || "";
    return l.trim().slice(0, 20) || "(名前なし)";
  };
  const selects = [...document.querySelectorAll("select")].map((s) => ({
    name: label(s),
    n: s.options.length,
  }));
  return {
    selects,
    steps: document.querySelectorAll(".seg").length,
    chips: document.querySelectorAll("label.check, .buff-chip").length,
    free: [...document.querySelectorAll("input[type=text], textarea")].map(label),
    // 編集は例外操作なので、ふだんは読み取り表示になっているか
    readonlyBoxes: document.querySelectorAll(".value-box.read").length,
    openInputs: document.querySelectorAll("input.value-box").length,
  };
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 1100) => page.waitForTimeout(ms);
  const seen = [];
  const check = async (where) => {
    const r = await page.evaluate(CHECK);
    // 並べられる数(<= 12)の選択肢がドロップダウンに入っていたら段階選択に降ろせる
    const shouldBeSteps = r.selects.filter((s) => s.n <= 12);
    if (shouldBeSteps.length) {
      seen.push(`[${where}] 段階選択に降ろせる <select>: ${shouldBeSteps.map((s) => `${s.name}(${s.n})`).join(", ")}`);
    }
    const many = r.selects.filter((s) => s.n > 12);
    console.log(
      `  [${where}] 段階選択 ${r.steps} / チップ ${r.chips} / 読取表示 ${r.readonlyBoxes}(編集中 ${r.openInputs})` +
        ` / <select> ${r.selects.length}件(うち多数 ${many.length}) / 自由入力 ${r.free.length}件`,
    );
    if (r.free.length) console.log(`      自由入力: ${r.free.join(", ")}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  for (const tab of ["ホーム", "ダメージ計算"]) {
    await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
    await wait(1700);
    await check(tab);
  }
  // バフタブ。目的タブごとに出る量が大きく変わるので 3 つとも測る
  // (ここを巡回していなかったせいで、チップの増分がはみ出していたのを 3 周見逃した)
  await page.locator("nav.tabs button", { hasText: "バフ" }).click({ force: true });
  await wait(1600);
  await check("バフ");
  for (const purpose of ["火力を上げたい", "耐久を上げたい"]) {
    const tab = page.locator(".category-tab", { hasText: purpose });
    if (await tab.count()) {
      await tab.first().dispatchEvent("click");
      await wait(900);
      await check(`バフ・${purpose}`);
    }
  }

  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1500);
  const panes = await page.locator(".src-name").allInnerTexts();
  for (let i = 0; i < panes.length; i++) {
    await page.locator(".src-line").nth(i).dispatchEvent("click");
    await wait(800);
    await check(`キャラ・${panes[i]}`);
  }

  console.log("");
  if (seen.length === 0) console.log("§07: 並べられる選択肢のドロップダウンは 0 件");
  else seen.forEach((s) => console.log("NG " + s));
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
