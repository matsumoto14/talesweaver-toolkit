// §07「入力は 5 形態の上から順に試す」を全画面で数える。
//
//   1 自動(表示だけ) / 2 段階選択 / 3 チップ / 4 ステッパー / 5 自由入力
//
// 素の <select> は 5 形態のどれでもない(段階 3 で 0 件にした)。1 つ残っていても NG。
// 「1 つ選ぶ」は Picker(固定チップ + 候補面)。チップに値(meta)が無ければ、それは
// 素の select と同じ画面なので NG。
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
    chips: document.querySelectorAll(".togrow").length,
    pickers: document.querySelectorAll(".picker").length,
    pickerChips: document.querySelectorAll(".picker-chip").length,
    // 値の無い候補チップ(§07「1 つ選ぶ」: 候補には選ぶのに要る値を必ず出す)
    pickerChipsNoMeta: [...document.querySelectorAll(".picker-chip")]
      .filter((c) => !(c.querySelector(".picker-chip-meta")?.textContent || "").trim())
      .map((c) => (c.textContent || "").trim().slice(0, 16)),
    free: [...document.querySelectorAll("input[type=text], textarea")].map(label),
    // 数値の自由入力(StatInput の形態 5)。理由チップ(.chip.why)が付いているものだけ許す。
    // 生 input[type=number] は段階 4 で 0 件にしたので、1 件でも残っていれば NG
    freeNum: [...document.querySelectorAll(".stepper.free")].map((s) => ({
      name: (s.querySelector(".val")?.getAttribute("aria-label") || "").replace(/ を編集$/, "").slice(0, 20) || "(名前なし)",
      why: (s.querySelector(".chip.why")?.textContent || "").trim(),
    })),
    rawNumber: [...document.querySelectorAll("input[type=number]:not(.stepper .val)")].map(label),
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
    // <select> は 1 件でも NG(順序があれば段、なければ Picker)
    if (r.selects.length) {
      seen.push(`[${where}] <select> が残っている: ${r.selects.map((s) => `${s.name}(${s.n})`).join(", ")}`);
    }
    if (r.pickerChipsNoMeta.length) {
      seen.push(`[${where}] 値の無い Picker チップ: ${r.pickerChipsNoMeta.join(", ")}`);
    }
    console.log(
      `  [${where}] 段階選択 ${r.steps} / 行チップ ${r.chips} / Picker ${r.pickers}(固定チップ ${r.pickerChips})` +
        ` / 読取表示 ${r.readonlyBoxes}(編集中 ${r.openInputs}) / <select> ${r.selects.length}件 / 自由入力 ${r.free.length}件`,
    );
    if (r.free.length) console.log(`      自由入力: ${r.free.join(", ")}`);
    if (r.freeNum.length) {
      console.log(`      数値の自由入力: ${r.freeNum.map((x) => `${x.name}[${x.why || "理由なし"}]`).join(", ")}`);
    }
    const noWhy = r.freeNum.filter((x) => !x.why);
    if (noWhy.length) seen.push(`[${where}] 理由チップの無い数値自由入力: ${noWhy.map((x) => x.name).join(", ")}`);
    if (r.rawNumber.length) seen.push(`[${where}] 生 input[type=number] が残っている: ${r.rawNumber.join(", ")}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  for (const tab of ["ホーム", "ダメージ計算", "実測"]) {
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

  // 対人タブ。伸びしろを開いた状態は材料名の折り返しが出やすいので、閉じた状態と両方測る
  // (この画面は 2026-09-02 まで一度も巡回されておらず、白い結果面・跳ねない数値・3 行に折れた
  // 見出しを人のレビューで初めて拾った)
  await page.locator("nav.tabs button", { hasText: "対人" }).click({ force: true });
  await wait(1800);
  await check("対人");
  const growth = page.locator("button.growth-total-row.openable");
  if (await growth.count()) {
    for (let i = 0; i < (await growth.count()); i++) await growth.nth(i).dispatchEvent("click");
    await wait(900);
    await check("対人・伸びしろを開く");
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
  if (seen.length === 0) console.log("§07: <select> 0 件・値の無い Picker チップ 0 件・理由なしの数値自由入力 0 件・生 number 0 件");
  else seen.forEach((s) => console.log("NG " + s));
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
