// renderCandidates(): 候補の KV 形式描画(§選択の契約 s4)。
import { describe, expect, it } from "vitest";

import { renderCandidates, renderPageDirectory, renderUnderstandPrompt } from "../src/prompt";
import type { Candidate } from "../src/retrieve";

describe("renderCandidates", () => {
  it("段落と行を KV 形式(slot / ページ / 断片 or 列: 値)で描く", () => {
    const paragraph: Candidate = {
      id: "p:マーキュリアルコア/top/1", kind: "paragraph", page: "マーキュリアルコア", section: "概要",
      anchor: "top", ord: 1, table_idx: null, group_key: null, row_key: null,
      text: "コアの説明。", truncated: true, cells: null, nums: null,
    };
    const row: Candidate = {
      id: "r:マーキュリアルコア/h2_1/1/進0-強0", kind: "row", page: "マーキュリアルコア", section: "強化",
      anchor: "h2_1", ord: 2, table_idx: 1, group_key: "マーキュリアルコア\u0000h2_1\u00001",
      row_key: "進0-強0", text: "進化: 0 | 成功率: 100%", truncated: false,
      cells: { "進化": "0", "成功率": "100%" }, nums: { "進化": 0 },
    };
    const slotOf = (id: string): string => (id === paragraph.id ? "u01" : "u02");

    const text = renderCandidates(
      "マーキュリアルコアをあげたい", { evolution: 0 }, [paragraph, row], slotOf,
      { "進化": "コアの進化段階" },
    );

    expect(text).toContain("【質問】マーキュリアルコアをあげたい");
    expect(text).toContain("【あなた】進化: 0");
    expect(text).toContain("- slot: u01");
    expect(text).toContain("断片: コアの説明。");
    expect(text).toContain("- slot: u02");
    expect(text).toContain("進化: 0 | 成功率: 100%");
    expect(text).toContain("進化=コアの進化段階");
  });

  it("訂正がある行は訂正後の値で描き、行の末尾に訂正を別記する", () => {
    const row: Candidate = {
      id: "r:x/h2_1/1/k", kind: "row", page: "x", section: "y", anchor: "h2_1", ord: 1,
      table_idx: 1, group_key: "x\u0000h2_1\u00001", row_key: "k",
      text: "効果: -5%", truncated: false, cells: { "効果": "-5%" }, nums: null,
      corrections: [{ col: "効果", value: "-10%", grade: "confirmed", source: { kind: "notice", title: "t", url: "u" } }],
    };
    const slotOf = (): string => "u01";

    const text = renderCandidates("質問", {}, [row], slotOf, {});

    expect(text).toContain("効果: -10%");
    expect(text).toContain("訂正: 効果 = -10% [公式お知らせ]");
  });
});

describe("renderUnderstandPrompt", () => {
  it("直前が無ければ (なし)、あれば質問とページを書く", () => {
    const withoutPrev = renderUnderstandPrompt("質問", null, ["テシスコア"]);
    expect(withoutPrev).toContain("【直前】(なし)");
    expect(withoutPrev).toContain("【語が一致したページ】テシスコア");

    const withPrev = renderUnderstandPrompt("続きの質問", { question: "前の質問", page: "テシスコア" }, []);
    expect(withPrev).toContain('質問「前の質問」/ ページ「テシスコア」');
  });
});

describe("renderPageDirectory", () => {
  it("固有名の系列・運営ページを外し、名前順で並べる(キャッシュが効くよう毎回同じ文字列)", () => {
    const dir = renderPageDirectory(["テシスコア", "Monster/アーカン", "Elso", "コメント/Elso", "MenuBar"]);
    expect(dir).toBe(["【ページ一覧】(Tale Wiki のページ名 — 冒頭の一文)", "Elso", "テシスコア"].join("\n"));
    // 冒頭の断片があれば 1 行説明として添える(名前だけでは中身が引けない)
    const withLeads = renderPageDirectory(["Elso"], new Map([["Elso", "一部要素でSEEDの代わりに消費することができる"]]));
    expect(withLeads).toContain("Elso — 一部要素でSEEDの代わりに消費することができる");
  });
});
