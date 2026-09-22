/**
 * 分かち書き。Worker(検索クエリ)と取込(索引の terms)が同じ切れ目を使うための共有モジュール。
 * `tools/segment-cli.ts` が取込側からこれを呼ぶ(units.py が子プロセスとして起動する)。
 *
 * segment() の辞書語の最長一致が要る理由: `Intl.Segmenter("ja",{granularity:"word"})` は
 * カタカナ複合語を割る(「マーキュリアルコア」→ マ/ー/キ/ュ/リアル/コア、実測)。
 * ページ名・別名(alias.name 全件)を先に確保してから残りを Segmenter に渡す。
 */

/** 表記の揺れ。片方に寄せる(両方向は持たない)。normalize 後の文字列に適用する。 */
const VARIANTS: [RegExp, string][] = [[/開放/g, "解放"]];

/** 俗語 → 正式名。normalize 後の文字列に適用する。 */
const SLANG: [RegExp, string][] = [[/モブ/g, "モンスター"]];

/** 検索・索引に使う語として意味を持つか(ひらがなのみ・記号のみは捨てる、1 文字も捨てる)。 */
const MEANINGFUL = /[一-鿿゠-ヿｦ-ﾟa-zA-Z0-9]/;

/**
 * ゲームの一般語のうち `Intl.Segmenter` が割ってしまうもの(実測: ダンジョン → ダン/ジョン、
 * 好感度 → 好/感度)。ページ名・別名の辞書と一緒に最長一致で確保する。索引と質問の両方に効く。
 * 固有名詞(ページ名)はここに書かず alias に任せる。
 */
export const BASE_WORDS: readonly string[] = [
  "ダンジョン", "インクリ", "ルーン", "レリック", "マスタリー", "テレポート", "セイクリッド",
  "好感度", "経験値", "攻撃力", "防御力", "魔法力", "ハンドベル", "スペルブック", "チームロック",
  "エンチャント", "アビリティ", "クリティカル", "アーティファクト", "モンスター", "クエスト",
];

/** NFKC → 英字小文字 → 表記の揺れ → 俗語。文字列全体に掛ける(辞書語の照合はこの形で行う)。 */
export function normalize(s: string): string {
  let out = s.normalize("NFKC").toLowerCase();
  for (const [pattern, to] of VARIANTS) out = out.replace(pattern, to);
  for (const [pattern, to] of SLANG) out = out.replace(pattern, to);
  return out;
}

/**
 * 語単位の折り畳み。語末の長音「ー」を落とす(「サーバー」と「サーバ」を揃える。語中は残す)。
 * 文字列全体ではなく切り出した語に掛ける。文字列全体に掛けると「ドライバーって」のように後ろに
 * ひらがなが続く語で落ちず、索引側(「ドライバー(」→ 落ちる)と食い違う(実測)。
 */
export function fold(token: string): string {
  return token.length >= 3 ? token.replace(/ー+$/u, "") : token;
}

/** 辞書語の走査順(長い順)。alias の語に BASE_WORDS を足して正規化する。 */
export function buildDict(words: Iterable<string>): string[] {
  const set = new Set<string>();
  for (const w of words) {
    const n = normalize(w);
    if (n.length >= 2 && MEANINGFUL.test(n)) set.add(n);
  }
  for (const w of BASE_WORDS) set.add(normalize(w));
  return [...set].sort((a, b) => b.length - a.length);
}

/**
 * text を分かち書きして語の配列にする(各語は fold 済み)。
 * dict は正規化済みの辞書語(長い順に並んでいる前提。buildDict が用意する)。
 */
export function segment(text: string, dict: readonly string[]): string[] {
  const normalized = normalize(text);
  const tokens: string[] = [];
  const consumed = new Uint8Array(normalized.length);

  // 辞書語の最長一致を先に確保する。1 文字・記号だけの語は辞書に混ざっていても使わない
  // (雑音になるうえ、正しい語の最長一致を食ってしまう)。
  for (const word of dict) {
    if (word.length <= 1 || !MEANINGFUL.test(word)) continue;
    let from = 0;
    for (;;) {
      const at = normalized.indexOf(word, from);
      if (at === -1) break;
      const end = at + word.length;
      let free = true;
      for (let i = at; i < end; i += 1) {
        if (consumed[i]) { free = false; break; }
      }
      if (free) {
        tokens.push(fold(word));
        for (let i = at; i < end; i += 1) consumed[i] = 1;
      }
      from = end;
    }
  }

  // 残り(辞書語に使われていない範囲)を Intl.Segmenter に渡す。
  const segmenter = new Intl.Segmenter("ja", { granularity: "word" });
  let rest = "";
  const flushRest = (): void => {
    if (!rest) return;
    for (const { segment: word } of segmenter.segment(rest)) {
      const trimmed = word.trim();
      if (trimmed.length <= 1) continue;
      if (!MEANINGFUL.test(trimmed)) continue;
      tokens.push(fold(trimmed));
    }
    rest = "";
  };
  for (let i = 0; i < normalized.length; i += 1) {
    if (consumed[i]) {
      flushRest();
    } else {
      rest += normalized[i];
    }
  }
  flushRest();

  return tokens;
}

/** 重複を落とし、各語を `"…"` で囲んで ` OR ` で連結する。0 語なら null。 */
export function toQuery(tokens: readonly string[]): string | null {
  const unique = [...new Set(tokens)];
  if (unique.length === 0) return null;
  return unique.map((t) => `"${t.replace(/"/g, '""')}"`).join(" OR ");
}
