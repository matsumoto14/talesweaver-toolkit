// 実機の画面を撮る。**撮るだけ** — 操作も測定もしない(判断は人がする)。
//
// 使い方: アプリを start-app.ps1 で起動(CDP 9222)してから
//   node apps/desktop/scripts/shoot.js <出力先> [タブ名...]
// タブ名を省略すると、いま開いている画面を 1 枚撮る。
const { chromium } = require("playwright-core");

const [, , outDir, ...tabs] = process.argv;
if (!outDir) {
  console.error("出力先を指定する: node shoot.js <出力先ディレクトリ> [タブ名...]");
  process.exit(1);
}
const out = outDir.replace(/[\/]*$/, "/");

(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
  const page = browser.contexts()[0].pages()[0];
  await page.setViewportSize({ width: 1280, height: 840 });

  const shot = async (name) => {
    const path = `${out}${name}.png`;
    await page.screenshot({ path, fullPage: true });
    console.log(path);
  };

  if (tabs.length === 0) {
    await shot("current");
  } else {
    for (const [i, label] of tabs.entries()) {
      // 上部タブ(App.svelte の <Choose class="tabs">)。ラベル文字で選ぶ
      await page.locator(".tabs label.chip", { hasText: label }).click();
      await page.waitForTimeout(400);
      await shot(`${String(i + 1).padStart(2, "0")}-${label}`);
    }
  }
  await browser.close();
})().catch((e) => { console.error(e.message); process.exit(1); });
