// verify() の落とし方(§5「検証」)を 1 ケースずつ。
import { describe, expect, it } from "vitest";

import type { Candidate } from "../src/retrieve";
import { NONE_SELECTION } from "../src/schema";
import type { Selection } from "../src/schema";
import { leadHasBareNumber, verify } from "../src/verify";
import type { Ctx } from "../src/verify";

function paragraph(id: string, overrides: Partial<Candidate> = {}): Candidate {
  return {
    id, kind: "paragraph", page: "テシスコア", section: "概要", anchor: "top", ord: 1,
    table_idx: null, group_key: null, row_key: null, text: "断片", truncated: false,
    cells: null, nums: null, ...overrides,
  };
}

function row(id: string, overrides: Partial<Candidate> = {}): Candidate {
  return {
    id, kind: "row", page: "テシスコア", section: "強化", anchor: "h2_1", ord: 2,
    table_idx: 1, group_key: "テシスコア\u0000h2_1\u00001", row_key: "進0-強0",
    text: "進化: 0 | 強化: 0 | 成功率: 100%", truncated: false,
    cells: { "進化": "0", "強化": "0", "成功率": "100%" },
    nums: { "進化": 0, "強化": 0 },
    ...overrides,
  };
}

function ctxOf(candidates: Candidate[], overrides: Partial<Ctx> = {}): Ctx {
  return {
    candidates: new Map(candidates.map((c) => [c.id, c])),
    state: {},
    columnDict: { "進化": "evolution" },
    pageNames: ["テシスコア", "マーキュリアルコア"],
    candidateText: candidates.map((c) => `${c.page} ${c.text} ${JSON.stringify(c.cells)}`).join(" "),
    ...overrides,
  };
}

function selectionOf(overrides: Partial<Selection> = {}): Selection {
  return {
    none: false, verdict: "none", basis: [], missing: [], lead: "", computed: false,
    steps: [{ units: ["p1"], columns: [], key_check: [] }],
    ...overrides,
  };
}

describe("verify", () => {
  it("none なら手順 0・lead null(llm_none)", () => {
    const result = verify(NONE_SELECTION, ctxOf([]));
    expect(result.steps).toEqual([]);
    expect(result.lead).toBeNull();
    expect(result.dropped).toEqual([{ what: "all", why: "llm_none" }]);
  });

  it("結論文が落ちて根拠も残らなければ答えなしに倒す(無関係な断片だけ出さない)", () => {
    // 実例(2026-09-24): 「ティチエルの最も DPS が高い攻撃コンビネーション」に、verdict: no・
    // basis が全部選択集合の外、という選択が返り、無関係な段落 4 本だけが残っていた
    const sel = selectionOf({
      verdict: "no", basis: ["p9"], lead: "答えッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([paragraph("p1")]));
    expect(result.steps).toEqual([]);
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "all", why: "no_basis" });
  });

  it("verdict が none(はい/いいえの質問でない)なら、結論文が落ちても手順は残す", () => {
    const sel = selectionOf({ verdict: "none", basis: [], lead: "", steps: [{ units: ["p1"], columns: [], key_check: [] }] });
    const result = verify(sel, ctxOf([paragraph("p1")]));
    expect(result.steps).toHaveLength(1);
    expect(result.lead).toBeNull();
  });

  it("未使用スロット(候補に無い ID)は落とす。手順が空になれば手順ごと落とす", () => {
    const sel = selectionOf({ steps: [{ units: ["p1"], columns: [], key_check: [] }] });
    const result = verify(sel, ctxOf([]));
    expect(result.steps).toEqual([]);
    expect(result.dropped).toContainEqual({ what: "unit", id: "p1", why: "unknown_id" });
    expect(result.dropped).toContainEqual({ what: "all", why: "no_steps" });
  });

  it("重複したユニットは 2 回目を落とす", () => {
    const p1 = paragraph("p1");
    const sel = selectionOf({ steps: [{ units: ["p1", "p1"], columns: [], key_check: [] }] });
    const result = verify(sel, ctxOf([p1]));
    expect(result.steps).toHaveLength(1);
    expect(result.steps[0]?.units).toHaveLength(1);
    expect(result.dropped).toContainEqual({ what: "unit", id: "p1", why: "duplicate" });
  });

  it("1 手順のユニット数が上限(4)を超えたら塊で数えて落とす", () => {
    const units = ["a", "b", "c", "d", "e"].map((s) => paragraph(s, { group_key: null }));
    const sel = selectionOf({
      steps: [{ units: units.map((u) => u.id), columns: [], key_check: units.map(() => "") }],
    });
    const result = verify(sel, ctxOf(units));
    expect(result.steps[0]?.units).toHaveLength(4);
    expect(result.dropped).toContainEqual({ what: "unit", id: "e", why: "over_limit" });
  });

  it("行がキャラ状態と矛盾したら落とす(進化 0 なのに進化 2 の行)", () => {
    const r = row("r1", { nums: { "進化": 2 } });
    const sel = selectionOf({ steps: [{ units: ["r1"], columns: [], key_check: ["進0-強0"] }] });
    const result = verify(sel, ctxOf([r], { state: { evolution: 0 } }));
    expect(result.steps).toEqual([]);
    expect(result.dropped).toContainEqual({ what: "unit", id: "r1", why: "state:進化" });
  });

  it("key_check が元行と食い違えば、同じ表にそのキーの行があれば差し替え、無ければ札のまま残す", () => {
    // 実例(2026-09-26 q51): キーの写し間違い 1 件で唯一の根拠が落ち「答えなし」になっていた
    const r1 = row("r1");
    const r2 = row("r2", { row_key: "進1-強9", cells: { "進化": "1", "強化": "9", "成功率": "50%" }, nums: { "進化": 1, "強化": 9 } });
    const swapped = verify(selectionOf({ steps: [{ units: ["r1"], columns: [], key_check: ["進1-強9"] }] }), ctxOf([r1, r2]));
    expect(swapped.steps[0]?.units.map((u) => u.id)).toEqual(["r2"]);
    expect(swapped.dropped).toContainEqual({ what: "unit", id: "r1", why: "key_check:swap:r2" });

    const kept = verify(selectionOf({ steps: [{ units: ["r1"], columns: [], key_check: ["存在しないキー"] }] }), ctxOf([r1]));
    expect(kept.steps[0]?.units.map((u) => u.id)).toEqual(["r1"]);
    expect(kept.dropped).toContainEqual({ what: "unit", id: "r1", why: "key_check:kept" });
  });

  it("実在しない列は抜き、キー列(先頭列)は常に出し、0 列ならその表の既定列で描く", () => {
    const r = row("r1");
    const sel = selectionOf({
      steps: [{ units: ["r1"], columns: ["成功率", "存在しない列"], key_check: ["進0-強0"] }],
    });
    const result = verify(sel, ctxOf([r]));
    expect(result.steps[0]?.columns).toEqual(["進化", "成功率"]);
    expect(result.dropped).toContainEqual({ what: "unit", why: "column:存在しない列" });

    const sel2 = selectionOf({ steps: [{ units: ["r1"], columns: [], key_check: ["進0-強0"] }] });
    const result2 = verify(sel2, ctxOf([r]));
    expect(result2.steps[0]?.columns).toEqual(["進化", "強化", "成功率"]);
  });

  it("地の文に数字があっても結論文は落とさない(計算した値。2026-09-24、ADR-020)", () => {
    const p1 = paragraph("p1");
    const sel = selectionOf({
      verdict: "yes", basis: ["p1"], lead: "成功率は100%ッピ。", computed: true,
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([p1]));
    expect(result.steps).toHaveLength(1);
    expect(result.lead).toEqual([{ t: "成功率は100%ッピ。" }]);
  });

  it("参照の直前・直後に同じ値を地の文でも書いた重複は落とす(「100% 100%」にしない)", () => {
    const r = row("r1");
    const sel = selectionOf({
      verdict: "yes", basis: ["r1"], lead: "成功率は{{r1.成功率}} 100%、前も100%{{r1.成功率}}ッピ。",
      steps: [{ units: ["r1"], columns: ["成功率"], key_check: ["進0-強0"] }],
    });
    const result = verify(sel, ctxOf([r]));
    expect(result.lead).toEqual([
      { t: "成功率は" }, { ref: "r1", col: "成功率" }, { t: "、前も" }, { ref: "r1", col: "成功率" }, { t: "ッピ。" },
    ]);
  });

  it("{{スロット.列}} の参照先が選択集合に無ければ結論文を捨てる", () => {
    const r = row("r1");
    const p2 = paragraph("p2");
    const sel = selectionOf({
      verdict: "yes", basis: ["r1"], lead: "成功率は {{p2.成功率}} ッピ。",
      steps: [{ units: ["r1"], columns: [], key_check: ["進0-強0"] }],
    });
    const result = verify(sel, ctxOf([r, p2]));
    expect(result.steps).toHaveLength(1);
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "lead", why: "ref_not_selected:p2" });
  });

  it("ID にピリオドが入る行も参照できる(最後のピリオドで区切る)", () => {
    // 実例(2026-09-26 q51): 「喪失の耐魔力 (LV.310)」の「.」で区切りを取り違え、結論文ごと落ちていた
    const r = row("r:P/a/0/喪失 (LV.310)");
    const sel = selectionOf({
      verdict: "yes", basis: [r.id], lead: `成功率は{{${r.id}.成功率}}ッピ。`,
      steps: [{ units: [r.id], columns: [], key_check: ["進0-強0"] }],
    });
    expect(verify(sel, ctxOf([r])).lead).toEqual([{ t: "成功率は" }, { ref: r.id, col: "成功率" }, { t: "ッピ。" }]);
  });

  it("短い段落への参照は本文に置き換え、形の崩れた参照は結論文ごと捨てる", () => {
    // 実例(2026-09-26 v01): 段落に「{{u02.所持Elso上限}}」と書き、ref_col で結論文が落ちていた
    const p1 = paragraph("p1", { text: "所持Elso上限: 1000万Elso" });
    const inline = verify(selectionOf({
      verdict: "yes", basis: ["p1"], lead: "{{p1.所持Elso上限}}ッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    }), ctxOf([p1]));
    expect(inline.lead?.map((seg) => ("t" in seg ? seg.t : "")).join("")).toBe("所持Elso上限: 1000万Elsoッピ。");

    const broken = verify(selectionOf({
      verdict: "yes", basis: ["p1"], lead: "合計は{{u32}}ッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    }), ctxOf([p1]));
    expect(broken.lead).toBeNull();
    expect(broken.dropped).toContainEqual({ what: "lead", why: "ref_malformed" });
  });

  it("参照先の列が実在しなければ結論文を捨てる", () => {
    const r = row("r1");
    const sel = selectionOf({
      verdict: "yes", basis: ["r1"], lead: "成功率は {{r1.存在しない列}} ッピ。",
      steps: [{ units: ["r1"], columns: [], key_check: ["進0-強0"] }],
    });
    const result = verify(sel, ctxOf([r]));
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "lead", why: "ref_col:存在しない列" });
  });

  it("lead に候補外の固有名詞(ページ名)があれば結論文を捨てる", () => {
    const p1 = paragraph("p1");
    const sel = selectionOf({
      verdict: "none", lead: "マーキュリアルコアもおすすめッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([p1]));
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "lead", why: "foreign:マーキュリアルコア" });
  });

  it("候補内に出るページ名は許す", () => {
    const p1 = paragraph("p1", { page: "マーキュリアルコア" });
    const sel = selectionOf({
      verdict: "none", lead: "マーキュリアルコアの話ッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([p1]));
    expect(result.lead).not.toBeNull();
  });

  it("verdict が yes/no/depends なのに basis が空なら lead を捨て、答えなしに倒す", () => {
    const p1 = paragraph("p1");
    const sel = selectionOf({
      verdict: "yes", basis: [], lead: "そうだッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([p1]));
    expect(result.steps).toEqual([]);
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "lead", why: "basis_empty" });
  });

  it("basis に挙げて手順に入れていない候補は手順として足す(手順を空にして根拠だけ書いた答えも残す)", () => {
    // 実例(2026-09-26 v01・s01): 手順が空で「答えなし」、根拠のユニットが答えに出ない、が起きていた
    const p1 = paragraph("p1");
    const p2 = paragraph("p2");
    const added = verify(selectionOf({
      verdict: "yes", basis: ["p2"], lead: "そうだッピ。",
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    }), ctxOf([p1, p2]));
    expect(added.steps.flatMap((st) => st.units.map((u) => u.id))).toEqual(["p1", "p2"]);
    expect(added.lead).not.toBeNull();

    const onlyBasis = verify(selectionOf({ verdict: "yes", basis: ["p1"], lead: "そうだッピ。", steps: [] }), ctxOf([p1]));
    expect(onlyBasis.steps.flatMap((st) => st.units.map((u) => u.id))).toEqual(["p1"]);
  });

  it("basis が 1 つでも選択集合に残っていれば、lead が別の理由で落ちても手順は残す", () => {
    const p1 = paragraph("p1");
    const sel = selectionOf({
      verdict: "yes", basis: ["p1"], lead: "あ".repeat(241),
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([p1]));
    expect(result.steps).toHaveLength(1);
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "lead", why: "too_long" });
  });

  it("lead が 240 字を超えたら捨てる", () => {
    const p1 = paragraph("p1");
    const sel = selectionOf({
      verdict: "none", lead: "あ".repeat(241),
      steps: [{ units: ["p1"], columns: [], key_check: [] }],
    });
    const result = verify(sel, ctxOf([p1]));
    expect(result.lead).toBeNull();
    expect(result.dropped).toContainEqual({ what: "lead", why: "too_long" });
  });

  it("手順は 4 つまで(超えた分は切る)", () => {
    const units = ["a", "b", "c", "d", "e"].map((s) => paragraph(s));
    const sel = selectionOf({
      steps: units.map((u) => ({ units: [u.id], columns: [], key_check: [] })),
    });
    const result = verify(sel, ctxOf(units));
    expect(result.steps).toHaveLength(4);
  });
});

describe("leadHasBareNumber", () => {
  it("参照の外の数字は LLM が書いた値として拾う(computed の申告漏れでも AI の計算の印を付ける)", () => {
    expect(leadHasBareNumber([{ t: "合わせて 3,000個ッピ。" }])).toBe(true);
    expect(leadHasBareNumber([{ t: "成功率は " }, { ref: "r:x", col: "成功率" }, { t: " の二乗で約１８％ッピ。" }])).toBe(true);
    expect(leadHasBareNumber([{ t: "五個ずつ要るッピ。" }])).toBe(true);
  });

  it("参照だけ・数字を含まない言い回しは拾わない", () => {
    expect(leadHasBareNumber([{ t: "必要数は " }, { ref: "r:x", col: "経験の聖水" }, { t: " ッピ。" }])).toBe(false);
    expect(leadHasBareNumber([{ t: "一番早いのは十分に強化してからッピ。" }])).toBe(false);
  });
});
