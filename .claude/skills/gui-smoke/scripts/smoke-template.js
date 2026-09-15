// スモークテストの雛形。スクラッチパッドにコピーして確認項目を書く。
// 実行: NODE_PATH=/c/github/private/twtoolkit/node_modules node smoke.js
const { chromium } = require("playwright-core");

const OUT = process.env.SMOKE_OUT || "C:/Users/takea/AppData/Local/Temp/claude-smoke/";
const log = (...a) => console.log(...a);

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errors = [];
  page.on("pageerror", (e) => errors.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errors.push("CONSOLE.ERROR " + m.text()); });

  // ---- helpers(実機で動作確認済みのセレクタ。UI を変えたらここも更新する)
  const shot = (name) => page.screenshot({ path: OUT + name, fullPage: true });
  const wait = (ms = 300) => page.waitForTimeout(ms);
  const toastText = async () => (await page.locator(".toast").count()) ? await page.locator(".toast").innerText() : null;
  const byLabel = (scope, label) => scope.locator(".label", { hasText: new RegExp("^" + label + "$") });
  // 上部タブ(App.svelte <nav class="tabs">)。ラベルは ホーム / ダメージ計算 / バフ / キャラ / 対人 / 実測 / お知らせ
  // (対人はロック解除後だけ出る)。折りたたみ時の title 属性は無いのでラベル文字で選ぶ
  const nav = async (label) => { await page.locator("nav.tabs button", { hasText: label }).click(); await wait(); };
  // ui/StepSelect.svelte(順序のある 1 つ選ぶ): <div class="step-select"><span class="label">…</span><div class="seg"><button class="step">
  const stepByLabel = async (scope, label, text) => {
    await scope.locator(".step-select", { has: byLabel(scope, label) }).locator(".seg .step", { hasText: text }).click(); await wait();
  };
  // ui/Picker.svelte(1 つ選ぶ): <div class="picker"><span class="label">…</span><div class="picker-line">
  //   固定チップ <button class="picker-chip">名前 値</button> … 候補面の口 <button class="picker-trigger">
  // 候補面は <button class="picker-row">名前 値</button>。pick(scope, label, name) で名前の候補を選ぶ
  const picker = (scope, label) => scope.locator(".picker", { has: byLabel(scope, label) });
  const pick = async (scope, label, name) => {
    const p = picker(scope, label);
    const chip = p.locator(".picker-chip", { hasText: name });
    if (await chip.count()) { await chip.first().click(); await wait(); return; }
    await p.locator(".picker-trigger").click(); await wait(200);
    await page.locator(".picker-row", { hasText: name }).first().click(); await wait();
  };
  // ui/StatInput.svelte: <div class="stat-input"><span class="label">…</span><input class="num-field">
  const statInput = (scope, label) => scope.locator(".stat-input", { has: byLabel(scope, label) }).locator(".num-field");
  const setNum = async (loc, v) => { await loc.fill(String(v)); await loc.dispatchEvent("blur"); };
  const setRange = (loc, v) => loc.evaluate((el, v) => {
    el.value = String(v);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }, v);
  // 行チップ(ui/ToggleRow): <div class="togrow [on]"><button class="face">名称 … 値</button></div>
  // toggle(scope, text) で押す。オンかどうかは .togrow.on で見る
  const toggleRow = (scope, text) => scope.locator(".togrow", { hasText: text });
  const toggle = (scope, text) => toggleRow(scope, text).locator(".face").click();

  // ---- キャラ画面
  // 左レール(CharacterRail.svelte)からキャラを選ぶ(表示名の完全一致)。
  // 行は <div class="list"><button class="char" title="名前(キャラ種) …"><span class="name">名前</span>
  const openCharacter = async (name) => {
    await page.locator(".list button.char", { has: page.locator(".name", { hasText: new RegExp("^" + name + "$") }) }).first().click();
    await wait(400);
  };
  // レールに並んでいる表示名の一覧(検証に使うキャラを決めるとき)
  const characterNames = () => page.locator(".list button.char .name").allInnerTexts();
  // 登録フォーム(呼び名 + キャラのアイコン選択)。呼び名は ui/TextField の読み取り面
  // (<button aria-label="呼び名 を編集">)が既定で、押すと <input aria-label="呼び名"> になる
  const registerCharacter = async (name, gameCharacterLabel) => {
    await page.locator("button[aria-label='呼び名 を編集']").click();
    await page.locator("input[aria-label='呼び名']").fill(name);
    await page.locator("button.pick", { hasText: gameCharacterLabel }).click();
    await page.locator("button.btn.primary", { hasText: "未装備で登録" }).click();
    await wait(500);
  };
  // 設定列のアコーディオン(恒常補正 / 装備 / 常用バフ / キャラスキル / 調整)。開いた .group を返す
  const groupHead = (title) => page.locator(".group-head", { has: page.locator(".group-title", { hasText: title }) });
  const openGroup = async (title) => {
    const group = page.locator(".group", { has: groupHead(title) });
    if ((await group.locator(".group-body").count()) === 0) { await groupHead(title).click(); await wait(); }
    return group;
  };
  // アコーディオン内の n 番目の .block.stats(例: 装備 = 0 基本能力値 / 1 強化能力値)
  const statsBlock = (group, n) => group.locator(".block.stats").nth(n);
  // 保存(未保存変更があるときだけ有効)。戻り値は保存後の「未保存」バッジ数(0 が正常)
  const saveCharacter = async () => {
    await page.locator("button.btn.primary", { hasText: "保存" }).click();
    await wait(500);
    return page.locator(".badge", { hasText: "未保存" }).count();
  };
  // 削除はキャラ画面(Workspace)の詳細側にある。選んでから押す
  const deleteCharacter = async (name) => {
    await openCharacter(name);
    await page.locator("button.btn.danger", { hasText: "削除" }).click();
    await wait(400);
  };

  // ---- ダメージ計算画面
  // キャラ option のラベルは「表示名 (キャラ種)」
  const calculate = async ({ character, skill, enemy }) => {
    await pick(page, "キャラ", character);
    await pick(page, "スキル", skill);
    await pick(page, "対象", enemy); await wait(800);
  };
  // トレース(<details class="trace">)を開く
  const openTrace = async () => {
    const d = page.locator("details.trace");
    if (!(await d.evaluate((el) => el.open))) { await d.locator("summary").click(); await wait(400); }
  };
  // body テキストから見出し以降を抜く(例: "RESULT", "(c) 式の各段")
  const textAfter = async (heading, len = 800) => {
    const t = await page.locator("body").innerText();
    const i = t.indexOf(heading);
    return i < 0 ? null : t.slice(i, i + len);
  };

  // ---- 確認項目(例: 装備を入れて保存 → ダメージ計算)
  await nav("キャラ");
  await openCharacter("検証ボリス");
  const eq = await openGroup("装備");
  await setNum(statInput(statsBlock(eq, 0), "突き攻撃力"), 400);
  await setNum(statInput(statsBlock(eq, 1), "突き攻撃力"), 200);
  await toggle(eq, "パワーウェポン");
  await stepByLabel(eq, "ストロングウェポン", "Lv6(+18%)");
  log("未保存 badge after save:", await saveCharacter());
  await shot("99-smoke-example.png");

  await nav("ダメージ計算");
  await calculate({ character: "検証ボリス (ボリス)", skill: "極・横斬り", enemy: "兄弟の鍛冶場" });
  await openTrace();
  log(await textAfter("RESULT", 400));
  log(await textAfter("(c) 式の各段", 1200));

  // ---- 結果
  log(errors.length ? "ERRORS:\n" + errors.join("\n") : "no page/console errors");
  await browser.close(); // CDP 接続を切るだけ。アプリは終了しない
})().catch((e) => { console.error("FAILED", e); process.exit(1); });
