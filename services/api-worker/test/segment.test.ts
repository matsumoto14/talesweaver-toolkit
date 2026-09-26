import { describe, expect, it } from "vitest";

import { buildDict, fold, normalize, segment, toQuery } from "../src/segment";

describe("normalize", () => {
  it("NFKC と大小文字を揃える", () => {
    expect(normalize("ＡＢＣ")).toBe("abc");
  });

  it("語末の長音は normalize では落とさず、語単位の fold で落とす(語中は残す)", () => {
    expect(normalize("サーバー")).toBe("サーバー");
    expect(fold("サーバー")).toBe("サーバ");
    expect(fold("サーバ")).toBe("サーバ");
    expect(fold("ラー")).toBe("ラー");
    // 後ろにひらがなが続いても、切り出した語には掛かる(索引側と一致する)
    expect(segment("ドライバーってどこ", [])).toContain("ドライバ");
    expect(segment("万能ドライバー(好感度)", [])).toContain("ドライバ");
  });

  it("表記の揺れを片方に寄せる(開放→解放)", () => {
    expect(normalize("開放条件")).toBe("解放条件");
  });

  it("俗語を正式名にする(モブ→モンスター)", () => {
    expect(normalize("モブ狩り")).toBe("モンスター狩り");
  });
});

describe("segment", () => {
  it("辞書語(ページ名)の最長一致を Segmenter より先に確保する", () => {
    // 素の Intl.Segmenter はこの語を割る(仕様書の実測): マ/ー/キ/ュ/リアル/コア
    const tokens = segment("マーキュリアルコアが欲しい", ["マーキュリアルコア"]);
    expect(tokens).toContain("マーキュリアルコア");
    expect(tokens).not.toContain("マ");
  });

  it("ひらがなのみ・記号のみの語を捨てる", () => {
    const tokens = segment("それはとても良い", []);
    for (const t of tokens) {
      expect(/[一-鿿゠-ヿｦ-ﾟa-zA-Z0-9]/.test(t)).toBe(true);
    }
  });

  it("1 文字の語を捨てる(英数字 1 文字も)", () => {
    const tokens = segment("A あ 上", []);
    expect(tokens).not.toContain("A");
    expect(tokens).not.toContain("上");
  });

  it("漢字 1 字の接頭・接尾は隣の漢字語とつないだ複合語にする(部品も残す)", () => {
    // 素の Segmenter: 新|鉱物|の|白|魔法|って|何|?  → 1 字を捨てると「鉱物 / 魔法」しか残らない
    const tokens = segment("新鉱物の白魔法って何?", []);
    expect(tokens).toEqual(expect.arrayContaining(["新鉱物", "鉱物", "白魔法", "魔法"]));
    expect(tokens).not.toContain("何"); // 隣が漢字でない 1 字は従来どおり捨てる
    expect(segment("巡礼者の村", [])).toContain("巡礼者"); // 接尾は前とつなぐ
    expect(segment("アビを何個も付けたい", [])).not.toContain("何個"); // 疑問詞はつながない
  });

  it("続いたカタカナは 1 語につないだものも出す(部品も残す)", () => {
    // 素の Segmenter: エル|ソ|って|何 / ポー|ション|の|レシピ
    expect(segment("エルソって何?", [])).toContain("エルソ");
    expect(segment("ポーションのレシピ", [])).toEqual(expect.arrayContaining(["ポーション", "ポー", "ション", "レシピ"]));
    expect(segment("ルーンの庭園", [])).toContain("ルーン");
  });

  it("質問側(query)はつないだカタカナの破片を出さない(エル|ソ の「エル」がエルシリア等に当たるため)", () => {
    const q = segment("エルソはどこで稼げる?", [], { query: true });
    expect(q).toContain("エルソ");
    expect(q).not.toContain("エル");
    // 索引側は破片も残す(質問の語が常に索引の語の部分集合になる)
    expect(segment("エルソはどこで稼げる?", [])).toEqual(expect.arrayContaining(["エルソ", "エル"]));
  });

  it("質問例: エタ解放までのクエストの流れわかる?", () => {
    const tokens = segment("エタ解放までのクエストの流れわかる?", ["エタの意志"]);
    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens).toContain("解放");
  });

  it("質問例: 巡礼者の村の近くに何がある?", () => {
    const tokens = segment("巡礼者の村の近くに何がある?", ["巡礼者の村"]);
    expect(tokens).toContain("巡礼者の村");
  });

  it("質問例: エクリプスダンジョンの入場条件は?", () => {
    const tokens = segment("エクリプスダンジョンの入場条件は?", ["エクリプスダンジョン"]);
    expect(tokens).toContain("エクリプスダンジョン");
    expect(tokens).toContain("入場");
  });
});

describe("toQuery", () => {
  it("重複を落として \"…\" OR … で連結する", () => {
    expect(toQuery(["解放", "解放", "エタ"])).toBe('"解放" OR "エタ"');
  });

  it("0 語なら null", () => {
    expect(toQuery([])).toBeNull();
  });
});

describe("静的データの名前(correction.subject)を辞書に入れると 1 語で確保される", () => {
  it("迅速の秘薬 / 神鳥の塒 が割れずに残り、fold した語で原文に戻せる", () => {
    const subjects = ["迅速の秘薬", "神鳥の塒", "サーバー"];
    const dict = buildDict(subjects);
    const byToken = new Map(subjects.map((s) => [fold(normalize(s)), s]));
    for (const q of ["迅速の秘薬の効果は?", "神鳥の塒ってどこ?"]) {
      const hit = segment(q, dict).map((t) => byToken.get(t)).filter(Boolean);
      expect(hit.length).toBe(1);
    }
    expect(segment("迅速の秘薬の効果は?", [])).not.toContain("迅速の秘薬"); // 辞書が無いと割れる
  });
});
