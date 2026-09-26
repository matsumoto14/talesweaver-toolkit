// 静的データだけの項目(段階 3 spec B 7)。getAppDataCandidates() の完全一致・上限・alias 非汚染。
// + collectCandidates() の節ごと足す・リンク先の節をたどる(節をたどるゴール、2026-09-26)。
import { describe, expect, it } from "vitest";
import { collectCandidates, getAppDataCandidates } from "../src/retrieve";

interface FakeRow { [key: string]: unknown }

/** correction テーブルだけを持つ最小の D1。alias / unit_fts など他テーブルは空(呼ばれたら失敗させる)。 */
function fakeDb(corrections: FakeRow[]): D1Database {
  return {
    prepare(sql: string) {
      if (!sql.includes("FROM correction")) {
        throw new Error(`想定外のクエリ(alias 表などに触れていないか): ${sql}`);
      }
      const statement = {
        bind: (..._args: unknown[]) => statement,
        all: async <T = FakeRow>() => ({ results: corrections as unknown as T[], success: true, meta: {} }) as D1Result<T>,
        first: async () => null,
        run: async () => ({ success: true, meta: {} }) as D1Result,
      };
      return statement as unknown as D1PreparedStatement;
    },
  } as unknown as D1Database;
}

const ITEM_ROW = (subject: string, col: string, value: string): FakeRow => ({
  id: `c:app/${subject}/${col}`, subject, unit_id: null, col, value, grade: "confirmed",
  source_kind: "client_db", source_title: "アプリのデータ(クライアント DB 由来)",
  source_url: null, section: "装備(武器)",
});

describe("getAppDataCandidates", () => {
  it("質問の語が subject に完全一致したときだけ疑似候補にする", async () => {
    const db = fakeDb([
      ITEM_ROW("†アーミングソード", "突き", "120"),
      ITEM_ROW("†アーミングソード", "斬り", "0"),
    ]);
    const out = await getAppDataCandidates(db, ["テシスコア", "†アーミングソード"]);

    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({
      id: "c:app/†アーミングソード",
      kind: "row",
      page: "アプリのデータ",
      section: "装備(武器)",
      anchor: "app",
      row_key: "†アーミングソード",
    });
    expect(out[0]!.cells).toMatchObject({ 名前: "†アーミングソード", 突き: "120", 斬り: "0" });
  });

  it("部分一致では候補にしない", async () => {
    const db = fakeDb([ITEM_ROW("†アーミングソード", "突き", "120")]);
    const out = await getAppDataCandidates(db, ["アーミング"]);
    expect(out).toEqual([]);
  });

  it("一致する subject が複数あっても上限は 3 件", async () => {
    const rows = ["剣A", "剣B", "剣C", "剣D"].flatMap((s) => [ITEM_ROW(s, "突き", "10")]);
    const db = fakeDb(rows);
    const out = await getAppDataCandidates(db, ["剣A", "剣B", "剣C", "剣D"]);
    expect(out).toHaveLength(3);
  });

  it("トークンが無ければ D1 に問い合わせない(alias 表を汚さない=何も検索しない)", async () => {
    const out = await getAppDataCandidates(fakeDb([]), []);
    expect(out).toEqual([]);
  });
});

// --- collectCandidates: 節ごと足す・リンク先の節をたどる ---------------------------------------

const unitRow = (over: Partial<FakeRow> & { id: string }): FakeRow => ({
  kind: "paragraph", page: "P", section: "節", anchor: "a1", ord: 1, truncated: 0,
  text: "本文", table_idx: null, group_key: null, row_key: null, cells: null, nums: null,
  ...over,
});

/** collectCandidates が触るクエリだけを配れる最小の D1。 */
function fakeWikiDb(opts: {
  ftsHits?: FakeRow[];
  sections?: Record<string, FakeRow[]>;
  tables?: Record<string, FakeRow[]>;
  links?: Record<string, FakeRow[]>;
  sectionSearch?: Record<string, FakeRow[]>;
}): D1Database {
  return {
    prepare(sql: string) {
      let bound: unknown[] = [];
      const statement = {
        bind: (...args: unknown[]) => { bound = args; return statement; },
        all: async <T = FakeRow>() => {
          let results: FakeRow[] = [];
          if (sql.includes("FROM unit_fts") && sql.includes("u.anchor = ?3")) {
            const [, page, anchor] = bound as [string, string, string];
            results = opts.sectionSearch?.[`${page}\u0000${anchor}`] ?? [];
          } else if (sql.includes("FROM unit_fts")) {
            results = opts.ftsHits ?? [];
          } else if (sql.includes("FROM unit WHERE page = ?1 AND anchor = ?2 ORDER BY ord")) {
            const [page, anchor] = bound as [string, string];
            results = opts.sections?.[`${page}\u0000${anchor}`] ?? [];
          } else if (sql.includes("FROM unit WHERE page = ?1 AND anchor = ?2 AND table_idx")) {
            const [page, anchor, tableIdx] = bound as [string, string, number];
            results = opts.tables?.[`${page}\u0000${anchor}\u0000${tableIdx}`] ?? [];
          } else if (sql.includes("FROM unit_link")) {
            const ids = bound as string[];
            results = ids.flatMap((id) => opts.links?.[id] ?? []);
          } else if (sql.includes("FROM correction")) {
            results = [];
          }
          return { results: results as unknown as T[], success: true, meta: {} } as D1Result<T>;
        },
        first: async () => null,
        run: async () => ({ success: true, meta: {} }) as D1Result,
      };
      return statement as unknown as D1PreparedStatement;
    },
  } as unknown as D1Database;
}

const BASE_INPUT = { query: "q", state: {}, columnNotes: {} };

describe("collectCandidates: 節ごと足す(boostPages)", () => {
  it("boostPages の節はヒットが段落でも節を丸ごと足し、当たったユニットを必ず残す", async () => {
    const hit = unitRow({ id: "p:P/a1/2", ord: 2, score: 1 });
    const sectionRows = [
      unitRow({ id: "p:P/a1/1", ord: 1 }),
      unitRow({ id: "p:P/a1/2", ord: 2 }),
      unitRow({ id: "r:P/a1/0/x", kind: "row", ord: 3, table_idx: 0 }),
    ];
    const db = fakeWikiDb({
      ftsHits: [hit],
      sections: { "P\u0000a1": sectionRows },
    });
    const out = await collectCandidates(db, { ...BASE_INPUT, boostPages: ["P"] });
    expect(out.map((c) => c.id).sort()).toEqual(["p:P/a1/1", "p:P/a1/2", "r:P/a1/0/x"]);
  });

  it("boostPages でないページの行ヒットは今まで通り同じ表の残りだけを足す", async () => {
    const hit = unitRow({ id: "r:Q/a1/0/x", kind: "row", page: "Q", ord: 1, table_idx: 0, score: 1 });
    const tableRows = [
      unitRow({ id: "r:Q/a1/0/x", kind: "row", page: "Q", ord: 1, table_idx: 0 }),
      unitRow({ id: "r:Q/a1/0/y", kind: "row", page: "Q", ord: 2, table_idx: 0 }),
    ];
    const db = fakeWikiDb({
      ftsHits: [hit],
      tables: { "Q\u0000a1\u00000": tableRows },
    });
    const out = await collectCandidates(db, { ...BASE_INPUT, boostPages: [] });
    expect(out.map((c) => c.id).sort()).toEqual(["r:Q/a1/0/x", "r:Q/a1/0/y"]);
  });
});

describe("collectCandidates: リンク先の節をたどる", () => {
  it("最上位のヒットが boostPages の節なら、アンカー付きリンク先の節から同じ語のヒットを足す", async () => {
    const hit = unitRow({ id: "p:エルソ/ce291035/1", page: "エルソ", anchor: "ce291035", ord: 1, score: 1 });
    const sectionRows = [hit];
    const db = fakeWikiDb({
      ftsHits: [hit],
      sections: { "エルソ\u0000ce291035": sectionRows },
      links: {
        "p:エルソ/ce291035/1": [
          { unit_id: "p:エルソ/ce291035/1", page: "ミニゲーム/ルミナの回廊", anchor: "CorridorBuff", ord: 0 },
        ],
      },
      sectionSearch: {
        "ミニゲーム/ルミナの回廊\u0000CorridorBuff": [
          unitRow({
            id: "r:ミニゲーム/ルミナの回廊/CorridorBuff/0/x", kind: "row",
            page: "ミニゲーム/ルミナの回廊", anchor: "CorridorBuff", ord: 1, table_idx: 0, score: 1,
          }),
        ],
      },
    });
    const out = await collectCandidates(db, { ...BASE_INPUT, boostPages: ["エルソ"] });
    expect(out.map((c) => c.id)).toContain("r:ミニゲーム/ルミナの回廊/CorridorBuff/0/x");
  });

  it("アンカー無しのリンクはたどらない", async () => {
    const hit = unitRow({ id: "p:P/a1/1", ord: 1, score: 1 });
    const db = fakeWikiDb({
      ftsHits: [hit],
      sections: { "P\u0000a1": [hit] },
      links: { "p:P/a1/1": [{ unit_id: "p:P/a1/1", page: "Q", anchor: "", ord: 0 }] },
      sectionSearch: { "Q\u0000": [unitRow({ id: "p:Q/x/1", page: "Q" })] },
    });
    const out = await collectCandidates(db, { ...BASE_INPUT, boostPages: ["P"] });
    expect(out.map((c) => c.id)).not.toContain("p:Q/x/1");
  });

  it("上限は 40 件(= MAX_SLOTS。20 件を超えても切られない)", async () => {
    // 別々の節(a1・a2)を 15 ユニットずつヒットさせる(合計 30 件、旧上限 20 を超える)
    const sectionA = Array.from({ length: 15 }, (_, i) => unitRow({ id: `p:P/a1/${i + 1}`, anchor: "a1", ord: i + 1 }));
    const sectionB = Array.from({ length: 15 }, (_, i) => unitRow({ id: `p:P/a2/${i + 1}`, anchor: "a2", ord: i + 1 }));
    const db = fakeWikiDb({
      ftsHits: [sectionA[0]!, sectionB[0]!],
      sections: { "P\u0000a1": sectionA, "P\u0000a2": sectionB },
    });
    const out = await collectCandidates(db, { ...BASE_INPUT, boostPages: ["P"] });
    expect(out.length).toBe(30);
  });

  it("大きな節でもリンクを引く ID は当たったユニットの前後 25 件だけ(D1 の bind 上限 100 を超えない)", async () => {
    // 実例: アーティファクトのページの節が 100 ユニットを超え、500 になった(2026-09-26)
    const rows = Array.from({ length: 150 }, (_, i) => unitRow({ id: `p:P/a1/${i + 1}`, ord: i + 1 }));
    const linkLookups: number[] = [];
    const base = fakeWikiDb({ ftsHits: [{ ...rows[100]!, score: 1 }], sections: { "P\u0000a1": rows } });
    const db = {
      prepare(sql: string) {
        const statement = base.prepare(sql);
        if (!sql.includes("FROM unit_link")) return statement;
        return { ...statement, bind: (...args: unknown[]) => { linkLookups.push(args.length); return statement.bind(...args); } };
      },
    } as unknown as D1Database;
    await collectCandidates(db, { ...BASE_INPUT, boostPages: ["P"] });
    expect(linkLookups).toEqual([25]);
  });

  it("最上位のヒットが boostPages でなければリンクをたどらない", async () => {
    const hit = unitRow({ id: "p:P/a1/1", ord: 1, score: 1 });
    const db = fakeWikiDb({
      ftsHits: [hit],
      tables: {},
      links: { "p:P/a1/1": [{ unit_id: "p:P/a1/1", page: "Q", anchor: "x", ord: 0 }] },
      sectionSearch: { "Q\u0000x": [unitRow({ id: "p:Q/x/1", page: "Q" })] },
    });
    const out = await collectCandidates(db, { ...BASE_INPUT, boostPages: [] });
    expect(out.map((c) => c.id)).not.toContain("p:Q/x/1");
  });
});
