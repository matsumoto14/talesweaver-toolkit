// 表示言語(日本語 / 韓国語)。決定の理由は docs/adr/022-i18n.md。
//
// **鍵は日本語の文言そのもの**(gettext 方式)。画面は `t("閉じる")` と書き、韓国語の辞書
// (`i18n/ko/*.json`、日本語 → 韓国語)を引く。日本語が正で、辞書に無い文言は日本語のまま出る
// — 漏れは `tools/i18n/check.py` が数える。Rust が返す表示名(トレースの出どころ・部位名など)も
// 同じ `t()` に通す。Rust が `format!` で組み立てた文(「武器 エンチャント」)は、辞書に文面を
// `{0} エンチャント` の形で載せておき、完全一致が無いときに照合で当てる(穴に入っていた部分も訳す)。
// ゲームデータ名の辞書(`ko/names.json`)も同じ引き方で、ファイルを分けるのは
// 出典が違うから(UI 文言は翻訳、データ名は韓国側の資料)。
//
// 言語はページを読み込むたびに 1 回だけ決まり、切り替えるときは読み込み直す。labels.ts のように
// モジュール評価時に文言を作る所がそのまま効き、画面側に言語の変化を追う仕掛けが要らない。

export type Locale = "ja" | "ko";

/** 選ぶ段。言語名はその言語で書く(読めない言語で「韓国語」と書いても探せない) */
export const LOCALE_OPTIONS: { value: Locale; label: string }[] = [
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
];

const STORAGE_KEY = "tw-locale";

function initialLocale(): Locale {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "ja" || stored === "ko") return stored;
  } catch {
    // private モード等。下の既定へ
  }
  // 選んだことがなければ、ブラウザ・OS の言語が韓国語のときだけ韓国語で開く
  return navigator.language.toLowerCase().startsWith("ko") ? "ko" : "ja";
}

/** この読み込みでの表示言語。途中では変わらない */
export const locale: Locale = initialLocale();

/** 数値・日付の書式に渡す BCP 47 の言語タグ */
export const LOCALE_TAG = locale === "ko" ? "ko-KR" : "ja-JP";

/** 言語を変えて読み込み直す */
export function setLocale(next: Locale): void {
  if (next === locale) return;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // 書けなければ切り替えられない(読み込み直すと元に戻る)ので、何もしない
    return;
  }
  location.reload();
}

let dictionary: Record<string, string> = {};

/**
 * 辞書を読む。App を読み込む前に 1 回だけ呼ぶ(main.ts)。日本語のときは何も読まない
 * (韓国語の辞書とフォントを日本語の利用者に配らない)。
 */
export async function loadLocale(): Promise<void> {
  document.documentElement.lang = locale;
  if (locale === "ja") return;
  const files = import.meta.glob<Record<string, string>>("./i18n/ko/*.json", { import: "default" });
  const [parts] = await Promise.all([
    Promise.all(Object.values(files).map((load) => load())),
    // ハングルの字形。日本語のフォントの後ろに並べる(app.css の --font)
    import("@fontsource/noto-sans-kr/400.css"),
    import("@fontsource/noto-sans-kr/500.css"),
    import("@fontsource/noto-sans-kr/700.css"),
    import("@fontsource/noto-sans-kr/800.css"),
  ]);
  dictionary = Object.assign({}, ...parts);
  patterns = Object.keys(dictionary)
    .filter((key) => HOLE.test(key))
    // 文面の長いものから試す(「{0} エンチャント」より「{0} のエンチャント上限」を先に)
    .sort((a, b) => b.replace(HOLES, "").length - a.replace(HOLES, "").length)
    .map((key) => ({
      match: new RegExp(`^${key.split(HOLE).map(escapeRegExp).join("(.+?)")}$`, "s"),
      holes: [...key.matchAll(HOLES)].map((m) => m[1]),
      key,
    }));
}

/** Rust の `format!` の穴(`{0}` `{1}`…)。鍵の形は tools/i18n/check.py が作るものと同じ */
const HOLE = /\{\d+\}/;
const HOLES = /\{(\d+)\}/g;
const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** 穴のある訳。完全一致が無いときだけ上から試す */
let patterns: { match: RegExp; holes: string[]; key: string }[] = [];
const matched = new Map<string, string>();

/** Rust が組み立てた文を照合で訳す。当たらなければ null */
function translateComposed(text: string): string | null {
  const cached = matched.get(text);
  if (cached !== undefined) return cached;
  for (const { match, holes, key } of patterns) {
    const found = match.exec(text);
    if (!found) continue;
    const args = Object.fromEntries(holes.map((hole, i) => [hole, t(found[i + 1])]));
    const result = dictionary[key].replace(HOLES, (whole, hole: string) => args[hole] ?? whole);
    matched.set(text, result);
    return result;
  }
  return null;
}

/**
 * 同じ日本語が場面で訳し分かれるとき(「敏捷」= 能力値の略称 / アビリティ名)。辞書の鍵は
 * `文脈::文言` で、無ければ文脈なしの訳 → 日本語の順に引く。
 */
export function tc(context: string, text: string, params?: Record<string, string | number>): string {
  const key = `${context}::${text}`;
  return key in dictionary ? t(key, params) : t(text, params);
}

/**
 * 文言を今の言語で。`{name}` は `params` で埋める(語順は言語ごとに辞書側が決める)。
 * 日本語の文言を鍵にするので、鍵は**画面に出す日本語そのまま**を書く。
 */
export function t(text: string, params?: Record<string, string | number>): string {
  const translated = dictionary[text] ?? (params || locale === "ja" ? null : translateComposed(text)) ?? text;
  if (!params) return translated;
  return translated.replace(/\{(\w+)\}/g, (whole, key: string) =>
    key in params ? String(params[key]) : whole,
  );
}
