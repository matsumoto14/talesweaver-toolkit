// /search と /health の入力検証。D1 は fake で prepare().bind().all() を返す最小のスタブ。
// ローカル D1 での通しは司令塔が手で行う(README 参照)。
import { describe, expect, it } from "vitest";

import worker, { type Env } from "../src/index";

interface FakeRow {
  [key: string]: unknown;
}

/** SQL の先頭のキーワードだけを見て、期待するテーブルの行を返す最小のスタブ。 */
function fakeDb(data: {
  meta?: FakeRow[];
  alias?: FakeRow[];
  units?: FakeRow[];
  pages?: FakeRow[];
}): D1Database {
  const meta = data.meta ?? [];
  const alias = data.alias ?? [];
  const units = data.units ?? [];
  const pages = data.pages ?? [];

  return {
    prepare(sql: string) {
      const statement = {
        bind: (..._args: unknown[]) => statement,
        all: async <T = FakeRow>() => {
          let results: FakeRow[] = [];
          if (sql.includes("FROM meta")) results = meta;
          else if (sql.includes("FROM alias")) results = alias;
          else if (sql.includes("FROM unit_fts")) results = units;
          else if (sql.includes("FROM page")) results = pages;
          return { results: results as unknown as T[], success: true, meta: {} } as D1Result<T>;
        },
      };
      return statement as unknown as D1PreparedStatement;
    },
  } as unknown as D1Database;
}

describe("GET /health", () => {
  it("meta が揃っていれば ok", async () => {
    const env: Env = {
      WIKI: fakeDb({
        meta: [
          { key: "synced_at", value: "2026-09-20T00:00:00Z" },
          { key: "unit_count", value: "186000" },
          { key: "schema_version", value: "1" },
        ],
      }),
    };
    const res = await worker.fetch(new Request("https://x/health"), env);
    expect(res.status).toBe(200);
    const body = (await res.json()) as Record<string, unknown>;
    expect(body).toMatchObject({ ok: true, unit_count: 186000, schema_version: "1" });
  });

  it("meta が無ければ 503", async () => {
    const env: Env = { WIKI: fakeDb({}) };
    const res = await worker.fetch(new Request("https://x/health"), env);
    expect(res.status).toBe(503);
    const body = (await res.json()) as Record<string, unknown>;
    expect(body.ok).toBe(false);
  });
});

describe("GET /search", () => {
  const baseEnv = (): Env => ({
    WIKI: fakeDb({
      meta: [{ key: "synced_at", value: "2026-09-20T00:00:00Z" }],
      alias: [{ name: "エタの意志", page: "エタの意志" }],
      units: [
        {
          id: "p:エタの意志/top/1",
          kind: "paragraph",
          page: "エタの意志",
          section: "",
          anchor: "top",
          ord: 1,
          table_idx: null,
          text: "断片",
          truncated: 1,
        },
      ],
      pages: [{ name: "エタの意志", url: "https://talewiki.com/?%A5%A8%A5%BF" }],
    }),
  });

  it("q が空なら no_terms", async () => {
    const res = await worker.fetch(new Request("https://x/search?q="), baseEnv());
    expect(res.status).toBe(200);
    const body = (await res.json()) as Record<string, unknown>;
    expect(body).toMatchObject({ kind: "none", reason: "no_terms", query: null, search: [] });
  });

  it("q が語を持てば no_llm で検索結果を返す(anchor top は url にアンカーを付けない)", async () => {
    const res = await worker.fetch(
      new Request(`https://x/search?${new URLSearchParams({ q: "エタ解放" })}`),
      baseEnv(),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as { kind: string; reason: string; search: { url: string }[] };
    expect(body.kind).toBe("none");
    expect(body.reason).toBe("no_llm");
    expect(body.search).toHaveLength(1);
    expect(body.search[0]?.url).toBe("https://talewiki.com/?%A5%A8%A5%BF");
  });

  it("q は 200 字で切る", async () => {
    const long = "あ".repeat(300);
    const res = await worker.fetch(
      new Request(`https://x/search?${new URLSearchParams({ q: long })}`),
      baseEnv(),
    );
    expect(res.status).toBe(200);
  });

  it("limit 省略と空文字は既定の 10(Number(null) が 0 になる罠)", async () => {
    const many = Array.from({ length: 15 }, (_, i) => ({
      id: `p:エタの意志/top/${i + 1}`, kind: "paragraph", page: "エタの意志", section: "", anchor: "top",
      ord: i + 1, table_idx: null, text: "断片", truncated: 0,
    }));
    const env: Env = {
      WIKI: fakeDb({
        meta: [{ key: "synced_at", value: "2026-09-20T00:00:00Z" }],
        alias: [{ name: "エタの意志", page: "エタの意志" }],
        units: many,
        pages: [{ name: "エタの意志", url: "https://talewiki.com/?%A5%A8%A5%BF" }],
      }),
    };
    for (const qs of ["q=エタ", "q=エタ&limit="]) {
      const res = await worker.fetch(new Request(`https://x/search?${qs}`), env);
      const body = (await res.json()) as { search: unknown[] };
      expect(body.search).toHaveLength(10);
    }
  });

  it("limit は 1..20 に丸める", async () => {
    const res = await worker.fetch(
      new Request(`https://x/search?${new URLSearchParams({ q: "エタ", limit: "999" })}`),
      baseEnv(),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as { search: unknown[] };
    expect(body.search.length).toBeLessThanOrEqual(20);
  });
});
