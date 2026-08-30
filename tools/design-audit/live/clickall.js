// §09 規則 1 を「押すと状態が変わる要素」で総当たりする。
// 12 周目の見落とし — round4.js は「押すと**開く**もの」しか測っていなかった。
// ここでは押せる要素を片っ端から押して、押した要素自身の座標が動かないかを見る。
const { chromium } = require("playwright-core");

const TARGETS = {
  ダメージ計算: [
    ["攻撃 / 防御タブ", "button.side-tab"],
    ["対象の ◀▶", "button.step"],
    ["極限スキルのチップ", "button.ultimate-chip:not([disabled])"],
    ["バフチップ", ".buff-chip:not([disabled])"],
    ["候補(足りない分)", "button.fill-btn"],
    // エンチャントの伸びしろ(試し変更。保存を伴わない = ダメージ計算タブの他項目と同じ扱い)。
    // MAX を押すとその行が一覧から消え、繰り上がった別の行を誤操作させた実害があった箇所
    // (修正済み)。同じ index の要素を押す前後で位置を比べる既存の仕組みで、行が消えて
    // 繰り上がれば座標がずれ、最後の行が消えれば after が null になるので拾える
    ["エンチャント MAX", ".enchant-rows button.max"],
  ],
  ホーム: [
    ["今日の強化タイル", "button.today-tile"],
    ["エリアの折りたたみ", "button.mini-row"],
    // 到達一覧の行はエリアを開かないと DOM に無い。openSel で最初のエリア行を先に押してから測る
    ["到達一覧の行", ".row", null, "button.mini-row"],
  ],
  キャラ: [
    ["補正源の行", ".src-line"],
    ["段階選択", ".seg .step", "キャラステータス"],
    ["数値の編集(行の位置)", ".stepper", "キャラステータス"],
    ["部位の行", "button.part-row", "装備"],
    ["チップ(オン/オフ)", "label.check", "クリティカル率"],
  ],
};

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });
  const errs = [];
  page.on("pageerror", (e) => errs.push("PAGEERROR " + e.message));
  page.on("console", (m) => { if (m.type() === "error") errs.push("CONSOLE " + m.text()); });
  const wait = (ms = 1100) => page.waitForTimeout(ms);
  // ビューポート座標だと、押した拍子のスクロール(Playwright の click も要素を視界に入れる)で
  // 動いて見える。**レイアウト上の位置**(offsetParent からの距離)で測る
  const box = async (loc) =>
    loc.evaluate((e) => (e.offsetParent === null ? null : [e.offsetLeft, e.offsetTop])).catch(() => null);
  const resetSim = async () => {
    const r = page.locator("button.btn", { hasText: "ぜんぶ戻す" });
    // 登録どおりのあいだ、このボタンは枠だけ残して visibility:hidden + inert で隠してある。
    // isEnabled() は隠れていても true を返すので、見えているかも見ないと click で固まる
    if ((await r.count()) && (await r.first().isVisible()) && (await r.first().isEnabled())) {
      await r.first().click();
      await wait(1300);
    }
  };

  await page.reload({ waitUntil: "load" });
  await wait(2600);

  let ng = 0;
  let missing = 0;
  for (const [tab, list] of Object.entries(TARGETS)) {
    for (const [name, sel, pane, openSel] of list) {
      // 前の測定で開いたペイン・スクロール位置が残っていると座標が動いて見える。
      // 1 件ごとにリロードして同じ初期状態から測る
      await page.reload({ waitUntil: "load" });
      await wait(2400);
      await page.locator("nav.tabs button", { hasText: tab }).click({ force: true });
      await wait(1700);
      await resetSim();
      if (pane) {
        // 補正源ペインを指定して開く(そのペインにしか無い部品を測るため)
        const names = await page.locator(".src-name").allInnerTexts();
        const i = names.indexOf(pane);
        if (i >= 0) {
          const row = page.locator(".src-line").nth(i);
          await row.scrollIntoViewIfNeeded().catch(() => {});
          await row.click({ force: true });
          await wait(1200);
        }
      }
      if (openSel) {
        // 対象がデフォルトで畳まれている等、先に別の要素を押さないと DOM に現れないもの
        // (例: 到達一覧の行はエリアを開くまで存在しない)
        const opener = page.locator(openSel).first();
        if (await opener.count()) {
          await opener.scrollIntoViewIfNeeded().catch(() => {});
          await opener.click({ force: true }).catch(() => {});
          await wait(900);
        }
      }
      const all = page.locator(sel);
      const n = await all.count();
      // セレクタが古くなって 1 件も見つからないと、この項目は静かに測られないまま素通りする
      // (実際に何周も気づけなかった)。ここは NG と同じ扱いで目立たせる
      if (n === 0) { console.log(`  [${tab}] ${name}: ⚠ セレクタが見つからず測定できていません(${sel})`); missing++; continue; }
      // 完全に見えている要素を選ぶ(見切れているとフォーカスで視界に入れるためにスクロールする)
      let target = null;
      for (let i = 0; i < Math.min(n, 8); i++) {
        const c = all.nth(i);
        const b = await c.boundingBox();
        if (b && b.y > 90 && b.y + b.height < 800 && (await c.isEnabled().catch(() => true))) { target = c; break; }
      }
      if (!target) { console.log(`  [${tab}] ${name}: 完全に見えているものが無い`); continue; }
      const before = await box(target);
      await target.dispatchEvent("click").catch(() => {});
      await wait(60);
      const movedCandidate = name === "候補(足りない分)" ? {
        from: await target.evaluate((e) => getComputedStyle(e).animationName).catch(() => "removed"),
        to: await page.locator(".chip-diff").last().evaluate((e) => getComputedStyle(e).animationName).catch(() => "missing"),
      } : null;
      await wait(1240);
      if (movedCandidate) {
        const ok = movedCandidate.from.includes("tw-candidate-out") && movedCandidate.to.includes("tw-badge-in");
        if (!ok) ng++;
        console.log(`  [${tab}] ${name}: 退出 ${movedCandidate.from} → 移動先 ${movedCandidate.to} ${ok ? "OK" : "NG"}`);
        continue;
      }
      const after = await box(target);
      if (!after) { console.log(`  [${tab}] ${name}: 押したあと消えた(要確認)`); ng++; continue; }
      const ok = before[0] === after[0] && before[1] === after[1];
      if (!ok) ng++;
      console.log(`  [${tab}] ${name}: ${JSON.stringify(before)} → ${JSON.stringify(after)} ${ok ? "OK" : `NG(${after[0] - before[0]},${after[1] - before[1]})`}`);
    }
  }
  await resetSim();
  console.log(ng === 0 ? "§09 規則 1: 全経路 OK" : `§09 規則 1: ${ng} 件 NG`);
  if (missing > 0) console.log(`⚠ ${missing} 件はセレクタが見つからず未測定(TARGETS を現行 DOM に合わせて直すこと)`);
  console.log(errs.length ? "ERRORS:\n" + errs.join("\n") : "no page/console errors");
  await browser.close();
})().catch((e) => { console.error("FAILED", e.message); process.exit(1); });
