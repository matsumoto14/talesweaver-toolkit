// 静的データだけの項目(段階 3 spec B 7)。getAppDataCandidates() の完全一致・上限・alias 非汚染。
import { describe, expect, it } from "vitest";
import { getAppDataCandidates } from "../src/retrieve";

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
