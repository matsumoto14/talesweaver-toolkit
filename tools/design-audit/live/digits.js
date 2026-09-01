// §09 規則 4 — 桁が増減しても隣が動かないか。実際に桁を変えて座標を測る。
//
// probe は**計算タブの試し変更(sim)だけ**で組む。キャラタブの入力は自動保存なので、
// 監査に使うとユーザーの実データを書き換えてしまう(motion.js と同じ方針)。
const { chromium } = require("playwright-core");

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const wait = (ms = 900) => page.waitForTimeout(ms);
  const head = (title) =>
    page.locator(".card-head", { has: page.locator(".card-title", { hasText: new RegExp("^" + title + "$") }) });
  const card = (title) => page.locator(".card", { has: head(title) });
  const openCard = async (title) => {
    if ((await card(title).locator(".basics-rows, .enchant-rows").count()) === 0) {
      await head(title).click();
      await wait(700);
    }
  };
  const resetSim = async () => {
    if (await page.locator(".sim-bar.active").count()) {
      await page.locator("button.btn", { hasText: "ぜんぶ戻す" }).click();
      await wait(1200);
    }
  };
  /** 対象群の x 座標(丸め)。「動かない」= この配列が変わらないこと */
  const xs = (selector) =>
    page.$$eval(selector, (els) => els.map((e) => Math.round(e.getBoundingClientRect().x)));
  const report = (what, before, after, texts) => {
    const ok = JSON.stringify(before) === JSON.stringify(after);
    console.log(`  [${what}] x が不変か: ${ok}`);
    console.log(`      ${texts}`);
    if (!ok) console.log(`      before ${JSON.stringify(before)}\n      after  ${JSON.stringify(after)}`);
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);
  await page.locator("nav.tabs button", { hasText: "ダメージ計算" }).click({ force: true });
  await wait(1800);
  await resetSim();

  // --- 鎖(1 発 → 合計 → 1 秒あたり)。覚醒段階を落とすとダメージ上限で桁が大きく減る。
  // **試し変更中どうしで比べる** — 登録どおり ⇄ 試し変更中では「キャラ登録どおりなら…」の
  // 一文が出入りして、桁とは関係なく幅が変わる(測りたいのは桁だけ)
  const chainText = async () => (await page.locator(".chain").innerText()).replace(/\n+/g, " ").slice(0, 80);
  await openCard("覚醒・エタの意志");
  const stageRow = card("覚醒・エタの意志").locator(".basics-row", { hasText: "覚醒段階" });
  await stageRow.locator("button.chip", { hasText: "それ以外" }).click();
  await wait(400);
  const setStage = async (n) => {
    await stageRow.locator(".seg .step", { hasText: new RegExp("^" + n + "$") }).click();
    await wait(2000);
  };
  await setStage(4);
  const beforeChain = await xs(".chain .node");
  const t1 = await chainText();
  await setStage(0);
  report("鎖 1 発 → 合計 → 1 秒あたり", beforeChain, await xs(".chain .node"), t1 + "\n      → " + (await chainText()));
  await resetSim();

  // --- エンチャントの伸びしろ。**押す場所**(値のセル・MAX)が動かないかを 2 通りで測る。
  // 右端の「MAX で +x%」は文言ごと変わる欄なので、その x を測っても意味がない
  await openCard("エンチャントの伸びしろ");
  const field = card("エンチャントの伸びしろ").locator(".enchant-stat").first();
  if (await field.count()) {
    const pressables = () => xs(".enchant-stat .cell, .enchant-stat .max");
    /** 編集を閉じる。focusout を実際に起こす必要があるので本当にフォーカスを移す */
    const leaveEdit = async () => {
      await page.locator(".chain .node.gate").click();
      await wait(700);
    };

    // (1) 読み取り ⇄ 編集。number 入力は UA 既定の固有幅を持つので、button と寸法が
    //     ずれると「押して編集に入っただけ」で隣が動く(§09 規則 1)
    const readState = await pressables();
    await field.locator(".cell .read").first().click();
    await wait(600);
    report("読み取り → 編集(値は変えない)", readState, await pressables(), "押して編集に入っただけ");

    // (2) 桁を変える。編集を閉じてから測り、(1) と混ざらないようにする
    const original = await field.locator("input").first().inputValue();
    await field.locator("input").first().fill("0");
    await field.locator("input").first().dispatchEvent("blur");
    await leaveEdit();
    report("桁が減ったあと", readState, await pressables(), `入力 ${original} → 0`);
  } else {
    console.log("  [エンチャント欄] 伸びしろのある部位が無いので未実行");
  }
  await resetSim();

  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
