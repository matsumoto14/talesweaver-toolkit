// 入力欄の棚卸し。全画面の入力を 1 件ずつ拾って、§07 / §08 の観点で並べる。
//
//   形態     — 自動 / 段階選択 / チップ / 値(表示既定) / 自由入力 / ドロップダウン
//   ラベル   — 何を入れる欄か言えているか
//   範囲     — 上限が値の隣にあるか
//   単位     — 値の意味(%・秒・Lv)が読めるか
//   初期値   — 実用値で埋まっているか(§07「入力欄は常に埋まっている」)
const { chromium } = require("playwright-core");

const CHECK = `(() => {
  const txt = (el) => (el?.textContent || "").replace(/\\s+/g, " ").trim();
  const rows = [];

  // 値の欄(表示が既定・編集は例外)
  document.querySelectorAll(".stat-input").forEach((el) => {
    const label = txt(el.querySelector(".label"));
    const box = el.querySelector(".value-box");
    const cap = txt(el.querySelector(".cap"));
    const hint = txt(el.querySelector(".hint"));
    rows.push({
      kind: "値",
      label: label || "(ラベルなし)",
      value: txt(box),
      cap: cap || "(範囲なし)",
      hint,
      // 行の見出し(表のセルなら左の列)を拾って、何の値か分かるかを見る
      near: txt(el.closest("tr")?.querySelector("td")) || txt(el.closest(".adj-stat")?.querySelector(".adj-stat-label")) || "",
    });
  });

  // 段階選択
  document.querySelectorAll(".step-select").forEach((el) => {
    const on = txt(el.querySelector(".step.on"));
    rows.push({
      kind: "段階",
      label: txt(el.querySelector(".label")) || "(ラベルなし)",
      value: on || "(未選択)",
      cap: el.querySelectorAll(".step").length + " 段",
      hint: "",
      near: "",
    });
  });

  // チップ
  document.querySelectorAll("label.check").forEach((el) => {
    rows.push({
      kind: "チップ",
      label: txt(el),
      value: el.querySelector("input")?.checked ? "オン" : "オフ",
      cap: "",
      hint: "",
      near: "",
    });
  });

  // ドロップダウン(5 形態に無い)と自由入力
  document.querySelectorAll("select").forEach((el) => {
    rows.push({
      kind: "ドロップダウン",
      label: txt(el.closest("label")?.querySelector(".label")) || el.getAttribute("aria-label") || "(ラベルなし)",
      value: el.options[el.selectedIndex]?.text || "",
      cap: el.options.length + " 択",
      hint: "",
      near: "",
    });
  });
  document.querySelectorAll("input[type=text], textarea").forEach((el) => {
    rows.push({
      kind: "自由入力",
      label: txt(el.closest("label")?.querySelector(".label")) || el.getAttribute("placeholder") || "(ラベルなし)",
      value: el.value || "(空)",
      cap: "",
      hint: "",
      near: "",
    });
  });
  return rows;
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 1000) => page.waitForTimeout(ms);
  const all = [];
  const collect = async (where) => {
    const rows = await page.evaluate(CHECK);
    rows.forEach((r) => all.push({ where, ...r }));
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  for (const tab of ["ホーム", "ダメージ計算"]) {
    await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
    await wait(1700);
    await collect(tab);
  }
  await page.locator("nav.tabs button", { hasText: "キャラ" }).click({ force: true });
  await wait(1500);
  const panes = await page.locator(".src-name").allInnerTexts();
  for (let i = 0; i < panes.length; i++) {
    await page.locator(".src-line").nth(i).dispatchEvent("click");
    await wait(800);
    await collect(panes[i]);
  }
  // 装備は部位詳細に入らないと入力が出ない
  const eq = panes.indexOf("装備");
  if (eq >= 0) {
    await page.locator(".src-line").nth(eq).dispatchEvent("click");
    await wait(900);
    await page.locator("button.part-row").first().dispatchEvent("click");
    await wait(1200);
    await collect("装備・部位詳細");
  }

  // --- 課題になりそうなものを拾う
  const issues = [];
  for (const r of all) {
    const where = `[${r.where}] ${r.kind} 「${r.label}」`;
    if (r.kind === "値") {
      if (r.label === "(ラベルなし)" && !r.near) issues.push(`${where} 何の値か言えていない(ラベルも行見出しも無い)`);
      if (r.cap === "(範囲なし)") issues.push(`${where} 上限が値の隣に無い(§07)`);
      if (/^(0|—|)$/.test(r.value)) issues.push(`${where} 初期値が 0 / 空(§07 入力欄は常に埋まっている)`);
    }
    if (r.kind === "段階" && r.value === "(未選択)") issues.push(`${where} どの段も選ばれていない(§07 初期値)`);
    if (r.kind === "ドロップダウン") issues.push(`${where} 5 形態のどれでもない(${r.cap})`);
    if (r.kind === "自由入力" && r.value === "(空)") issues.push(`${where} 空欄のまま(§07 placeholder に頼らない)`);
  }

  console.log(`=== 入力欄 ${all.length} 件 ===`);
  const byKind = {};
  all.forEach((r) => (byKind[r.kind] = (byKind[r.kind] || 0) + 1));
  console.log(JSON.stringify(byKind));
  if (process.argv.includes("--dump")) {
    console.log("\n=== 値の入力(ラベル / 現在値 / 上限 / 単位の注記) ===");
    const seen = new Set();
    all.filter((r) => r.kind === "値").forEach((r) => {
      const k = `${r.where}|${r.label}`;
      if (seen.has(k)) return;
      seen.add(k);
      console.log(`  [${r.where}] ${r.label || r.near || "(名前なし)"} = ${r.value} / ${r.cap}${r.hint ? " ・ " + r.hint : "  ← 単位の注記なし"}`);
    });
  }
  console.log(`\n=== 課題の候補 ${issues.length} 件 ===`);
  const uniq = [...new Set(issues)];
  uniq.forEach((s) => console.log("  " + s));
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
