// logAsk(): ask_log の INSERT が返す meta.last_row_id を使って ask_call を batch で書く(src/log.ts)。
import { describe, expect, it } from "vitest";

import { logAsk } from "../src/log";
import type { AskTrace } from "../src/log";
import type { RunAskResult } from "../src/index";

interface Recorded { sql: string; args: unknown[] }

function fakeDb(askLogId: number): { db: D1Database; inserted: Recorded[] } {
  const inserted: Recorded[] = [];
  const db = {
    prepare(sql: string) {
      const statement = {
        bind: (...args: unknown[]) => {
          inserted.push({ sql, args });
          return statement;
        },
        run: async () =>
          sql.includes("INSERT INTO ask_log")
            ? ({ success: true, meta: { last_row_id: askLogId } } as unknown as D1Result)
            : ({ success: true, meta: {} } as unknown as D1Result),
      };
      return statement as unknown as D1PreparedStatement;
    },
    batch: async (statements: D1PreparedStatement[]) => statements.map(() => ({ success: true, meta: {} }) as D1Result),
  } as unknown as D1Database;
  return { db, inserted };
}

const NONE_RESULT: RunAskResult = {
  status: 200,
  body: { kind: "none", reason: "llm_none", search: [], synced_at: null, dropped: [], playbook: null },
};

describe("logAsk", () => {
  it("trace があれば ask_call を ask_log の last_row_id・順番どおりの seq で書く", async () => {
    const { db, inserted } = fakeDb(42);
    const trace: AskTrace = {
      understanding: { kind: "wiki" },
      candidates: [{ id: "p:テシスコア/top/1", page: "テシスコア", section: "概要" }],
      calls: [
        { kind: "understand", model: "claude-haiku-4-5", ms: 5, inputTokens: 10, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 2 },
        { kind: "select", model: "claude-haiku-4-5", ms: 20, inputTokens: 50, cacheReadTokens: 5, cacheCreationTokens: 0, outputTokens: 8 },
      ],
    };

    await logAsk(db, { user: "u1", question: "q", qkey: null, prev: null, state: {}, result: NONE_RESULT, ms: 10, trace });

    const logInsert = inserted.find((r) => r.sql.includes("INSERT INTO ask_log"));
    expect(logInsert).toBeDefined();
    // 末尾 2 つ(understanding, candidates)は JSON 文字列で入る
    expect(JSON.parse(String(logInsert!.args.at(-2)))).toEqual({ kind: "wiki" });
    expect(JSON.parse(String(logInsert!.args.at(-1)))).toEqual(trace.candidates);

    const callInserts = inserted.filter((r) => r.sql.includes("INSERT INTO ask_call"));
    expect(callInserts).toHaveLength(2);
    expect(callInserts[0]!.args.slice(0, 3)).toEqual([42, 0, "understand"]);
    expect(callInserts[1]!.args.slice(0, 3)).toEqual([42, 1, "select"]);
    expect(callInserts[1]!.args).toEqual([42, 1, "select", "claude-haiku-4-5", 50, 5, 0, 8, 20]);
  });

  it("trace が無ければ ask_call は書かない", async () => {
    const { db, inserted } = fakeDb(1);
    await logAsk(db, { user: "u1", question: "q", qkey: null, prev: null, state: {}, result: NONE_RESULT, ms: 10 });
    expect(inserted.some((r) => r.sql.includes("INSERT INTO ask_call"))).toBe(false);
  });

  it("trace.calls が空なら ask_call は書かない", async () => {
    const { db, inserted } = fakeDb(1);
    const trace: AskTrace = { understanding: null, candidates: [], calls: [] };
    await logAsk(db, { user: "u1", question: "q", qkey: null, prev: null, state: {}, result: NONE_RESULT, ms: 10, trace });
    expect(inserted.some((r) => r.sql.includes("INSERT INTO ask_call"))).toBe(false);
  });

  it("ask_log の書き込みに失敗しても例外を投げない", async () => {
    const db = {
      prepare() {
        return { bind: () => ({ run: async () => { throw new Error("boom"); } }) } as unknown as D1PreparedStatement;
      },
      batch: async () => [],
    } as unknown as D1Database;
    await expect(
      logAsk(db, { user: "u1", question: "q", qkey: null, prev: null, state: {}, result: NONE_RESULT, ms: 10 }),
    ).resolves.toBeUndefined();
  });
});
