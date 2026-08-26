// §00「いつでも意識する 5 つ」のうち、**01 視線を動かさない** と **02 要らないものを見せない**
// を全画面で測る。
//
// 規格(色・角丸・書体)に合っていても、この 2 つは崩れる。2026-08-26 の覚醒段階が実例で、
// 機械監査は 10 ルールすべて 0 件のまま「0〜5 を幅いっぱいに並べた、めったに触らない欄」が
// 通っていた。だからここは規格ではなく**目的**の側から測る。
//
//   A1 選択群が折り返している        段の並びが 1 回の視線で読めない(01)
//   A2 入力欄が 1 行に 2 つ以上並ぶ  決める順に上から読めず、目が Z 字に飛ぶ(01)
//   A3 常時見えている選択肢の数      道具が人に投げている質問の数(02)
//   A4 内容が枠からはみ出す / 切れる  読むのに目を動かす・スクロールする(01)
//   A5 並ぶ欄の高さが揃っていない    片側だけ空く。視線の帰り先がぶれる(01)
//
// **どれも候補であって違反ではない。**「その選択肢は本当に要るか」は人が決める
// (design-review skill の ② 分類)。数だけ見て一括で減らすと、今度は情報が足りなくなる。
const { chromium } = require("playwright-core");

const CHECK = `(() => {
  const name = (el) => {
    const c = (el.className || "").toString().split(" ").filter((x) => x && !x.startsWith("s-")).join(".");
    const t = (el.innerText || "").trim().split("\\n").join(" / ").slice(0, 30);
    return (c || el.tagName.toLowerCase()) + (t ? " «" + t + "»" : "");
  };
  const rows = (el) => new Set([...el.children].map((c) => c.offsetTop)).size;
  // 畳んである <details> の中は「いま見えていない」。開いたときに測る
  const hidden = (el) => el.closest("details:not([open])") !== null || el.offsetParent === null;
  const out = { A1: [], A2: [], A3: [], A4: [], A5: [] };

  // A1 段階選択・チップ群が折り返している
  document.querySelectorAll(".seg, .chips, .buff-chips, .add-row").forEach((el) => {
    if (hidden(el)) return;
    if (el.children.length > 1 && rows(el) > 1) out.A1.push(name(el) + " " + rows(el) + " 行");
  });

  // A2 入力欄が横に並んでいる(1 行に 2 つ以上)
  document.querySelectorAll(".fields").forEach((box) => {
    if (hidden(box)) return;
    const byRow = new Map();
    [...box.children].forEach((c) => {
      const k = c.offsetTop;
      if (!byRow.has(k)) byRow.set(k, []);
      byRow.get(k).push(c);
    });
    byRow.forEach((cells) => {
      if (cells.length > 1) out.A2.push(cells.map((c) => (c.querySelector(".label")?.textContent || name(c)).trim()).join(" ↔ "));
    });
  });

  // A3 常時見えている選択肢(段・チップ・開いていない <select> の中身は数えない)
  const segs = [...document.querySelectorAll(".seg")].filter((s) => !hidden(s)).map((s) => ({
    n: s.children.length,
    label: (s.closest(".step-select")?.querySelector(".label")?.textContent || "").trim() || name(s).slice(0, 24),
  }));
  const chips = [...document.querySelectorAll(".chip, .buff-chip")].filter((c) => !hidden(c)).length;
  out.A3.push({ segTotal: segs.reduce((a, s) => a + s.n, 0), chips, top: segs.sort((a, b) => b.n - a.n).slice(0, 5) });

  // A4 はみ出し・省略
  document.querySelectorAll("div, section, span, td, th, p, button").forEach((el) => {
    const cs = getComputedStyle(el);
    if (cs.overflowX !== "visible" && cs.overflowX !== "hidden") return;
    if (el.clientWidth > 0 && el.scrollWidth - el.clientWidth > 1) {
      out.A4.push((cs.textOverflow === "ellipsis" ? "切れ " : "はみ出し ") + name(el));
    }
  });

  // A5 同じ行に並ぶ欄の高さのずれ
  document.querySelectorAll(".fields").forEach((box) => {
    const byRow = new Map();
    [...box.children].forEach((c) => {
      const k = c.offsetTop;
      if (!byRow.has(k)) byRow.set(k, []);
      byRow.get(k).push(c);
    });
    byRow.forEach((cells) => {
      if (cells.length < 2) return;
      const hs = cells.map((c) => c.getBoundingClientRect().height);
      const gap = Math.round(Math.max(...hs) - Math.min(...hs));
      if (gap > 40) out.A5.push(cells.map((c) => (c.querySelector(".label")?.textContent || "").trim()).join(" ↔ ") + " 差 " + gap + "px");
    });
  });
  return out;
})()`;

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 1100) => page.waitForTimeout(ms);
  const fold = (list, max = 5) => {
    const seen = new Map();
    list.forEach((s) => seen.set(s, (seen.get(s) || 0) + 1));
    return [...seen].slice(0, max).map(([s, c]) => (c > 1 ? `${s} ×${c}` : s)).join(" | ") + (seen.size > max ? ` …他 ${seen.size - max} 種` : "");
  };
  const check = async (where) => {
    const r = await page.evaluate(CHECK);
    const a3 = r.A3[0];
    const head = `  [${where}] 見えている選択肢 ${a3.segTotal + a3.chips}(段 ${a3.segTotal} / チップ ${a3.chips})`;
    console.log(head + (a3.top.length ? ` — 多い順 ${a3.top.map((s) => `${s.label}:${s.n}`).join(", ")}` : ""));
    for (const k of ["A1", "A2", "A4", "A5"]) {
      if (r[k].length) console.log(`    ${k} ${r[k].length} 件: ${fold(r[k])}`);
    }
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
