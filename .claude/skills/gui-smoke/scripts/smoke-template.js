// スモークテストの雛形。スクラッチパッドにコピーして確認項目を書く。
// 実行: NODE_PATH=/c/github/private/twtoolkit/node_modules node smoke.js
const { chromium } = require("playwright-core");

const OUT = "C:/github/private/talesweaver-toolkit/docs/screenshots/";
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
  // サイドバー(nav)。ボタンはラベル文字で選ぶ(title 属性は折りたたみ時しか付かない)
  const nav = async (label) => { await page.locator("nav button", { hasText: label }).click(); await wait(); };
  // ui/Select.svelte: <label class="select"><span class="label">…</span><select>
  const selectByLabel = (scope, label) => scope.locator("label.select", { has: byLabel(scope, label) }).locator("select");
  // ui/StatInput.svelte: <div class="stat-input"><span class="label">…</span><input class="num-field">
  const statInput = (scope, label) => scope.locator(".stat-input", { has: byLabel(scope, label) }).locator(".num-field");
  const setNum = async (loc, v) => { await loc.fill(String(v)); await loc.dispatchEvent("blur"); };
  const setRange = (loc, v) => loc.evaluate((el, v) => {
    el.value = String(v);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }, v);
  // チェックボックス(buff-check): <label class="buff-check">名称 <input type=checkbox>
  const checkbox = (scope, text) => scope.locator(".buff-check", { hasText: text }).locator("input[type=checkbox]");

  // ---- キャラ管理画面
  // 一覧からキャラを選ぶ(表示名の完全一致)
  const openCharacter = async (name) => { await page.locator(".list td.name > span", { hasText: new RegExp("^" + name + "$") }).first().click(); await wait(400); };
  // 登録フォーム(名前 + キャラ種のみ)
  const registerCharacter = async (name, gameCharacterLabel) => {
    await page.locator("input[placeholder='表示名']").fill(name);
    await selectByLabel(page, "キャラ").selectOption({ label: gameCharacterLabel });
    await page.locator("button.btn.primary", { hasText: "登録" }).click();
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
  // 一覧は <section class="list"><table class="grid"><tr><td class="name"><span>表示名</span>…
  const deleteCharacter = async (name) => {
    const row = page.locator(".list table.grid tr", { has: page.locator("td.name > span", { hasText: new RegExp("^" + name + "$") }) });
    await row.locator("button.btn.danger", { hasText: "削除" }).click();
    await wait(400);
  };

  // ---- ダメージ計算画面
  // キャラ option のラベルは「表示名 (キャラ種)」
  const calculate = async ({ character, skill, enemy }) => {
    await selectByLabel(page, "キャラ").selectOption({ label: character }); await wait();
    await selectByLabel(page, "スキル").selectOption({ label: skill }); await wait();
    await selectByLabel(page, "対象").selectOption({ label: enemy }); await wait(800);
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
  await nav("キャラ管理");
  await openCharacter("検証ボリス");
  const eq = await openGroup("装備");
  await setNum(statInput(statsBlock(eq, 0), "突き攻撃力"), 400);
  await setNum(statInput(statsBlock(eq, 1), "突き攻撃力"), 200);
  await checkbox(eq, "パワーウェポン").check();
  await selectByLabel(eq, "ストロングウェポン").selectOption({ label: "Lv6(+18%)" });
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
