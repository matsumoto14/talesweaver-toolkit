// 管理ホスト(admin.tw-context.dev)。ホスト分岐は worker.fetch を通して見る(access.ts はモック)。
// /api/logs /api/logs/:id /api/costs は handleAdmin を直接呼び、D1 は fake の最小スタブ。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../src/access", () => ({ verifyAccess: vi.fn() }));

import * as access from "../src/access";
import worker, { type Env } from "../src/index";
import { handleAdmin } from "../src/admin";
import type { AdminEnv } from "../src/admin";
import { fakeCtx } from "./ctx";

const ctx = fakeCtx();

beforeEach(() => {
  vi.mocked(access.verifyAccess).mockReset();
});

describe("ホスト分岐", () => {
  it("公開ホストは管理ルートを持たない(/ は 404、Access は見ない)", async () => {
    const res = await worker.fetch(new Request("https://api.tw-context.dev/"), {} as Env, ctx);
    expect(res.status).toBe(404);
    expect(access.verifyAccess).not.toHaveBeenCalled();
  });

  it("管理ホストは公開ルートを持たない(/ask は 404)", async () => {
    vi.mocked(access.verifyAccess).mockResolvedValue(true);
    const res = await worker.fetch(
      new Request("https://admin.tw-context.dev/ask", { method: "POST", body: "{}" }),
      {} as Env, ctx,
    );
    expect(res.status).toBe(404);
  });

  it("Access の検証に失敗したら 403(HTML を出さない)", async () => {
    vi.mocked(access.verifyAccess).mockResolvedValue(false);
    const res = await worker.fetch(new Request("https://admin.tw-context.dev/"), {} as Env, ctx);
    expect(res.status).toBe(403);
  });

  it("Access の検証を通れば管理画面の HTML を返す", async () => {
    vi.mocked(access.verifyAccess).mockResolvedValue(true);
    const res = await worker.fetch(new Request("https://admin.tw-context.dev/"), {} as Env, ctx);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("text/html");
    expect(await res.text()).toContain("wiki に聞く");
  });
});

// --- /api/* のロジック(D1 は fake) ------------------------------------------------

interface FakeRow { [key: string]: unknown }

function fakeAdminDb(data: {
  logs?: FakeRow[];
  calls?: FakeRow[];
  reactionCounts?: FakeRow[];
  logDetail?: FakeRow | null;
  detailCalls?: FakeRow[];
  reactions?: FakeRow[];
  neighborsBefore?: FakeRow[];
  neighborsAfter?: FakeRow[];
  costRows?: FakeRow[];
}): D1Database {
  const logs = data.logs ?? [];
  const calls = data.calls ?? [];
  const reactionCounts = data.reactionCounts ?? [];
  const logDetail = data.logDetail ?? null;
  const detailCalls = data.detailCalls ?? [];
  const reactions = data.reactions ?? [];
  const neighborsBefore = data.neighborsBefore ?? [];
  const neighborsAfter = data.neighborsAfter ?? [];
  const costRows = data.costRows ?? [];

  return {
    prepare(sql: string) {
      const statement = {
        bind: (..._args: unknown[]) => statement,
        all: async <T = FakeRow>() => {
          let results: FakeRow[] = [];
          if (sql.includes("FROM ask_call ac JOIN ask_log al")) results = costRows;
          else if (sql.includes("FROM ask_call WHERE ask_log_id IN")) results = calls;
          else if (sql.includes("FROM ask_call WHERE ask_log_id = ?")) results = detailCalls;
          else if (sql.includes("FROM reaction") && sql.includes("GROUP BY")) results = reactionCounts;
          else if (sql.includes("FROM reaction WHERE answer_id = ?")) results = reactions;
          else if (sql.includes("id < ?")) results = neighborsBefore;
          else if (sql.includes("id > ?")) results = neighborsAfter;
          else if (sql.includes("FROM ask_log al")) results = logs;
          return { results: results as unknown as T[], success: true, meta: {} } as D1Result<T>;
        },
        first: async <T = FakeRow>() => (sql.includes("FROM ask_log WHERE id = ?") ? (logDetail as T) : null),
        run: async () => ({ success: true, meta: {} }) as D1Result,
      };
      return statement as unknown as D1PreparedStatement;
    },
  } as unknown as D1Database;
}

function adminEnv(db: D1Database): AdminEnv {
  return { WIKI: db, ACCESS_TEAM_DOMAIN: "team", ACCESS_AUD: "aud" };
}

beforeEach(() => {
  vi.mocked(access.verifyAccess).mockResolvedValue(true);
});

describe("GET /api/logs", () => {
  it("呼び出し・費用・反応を突き合わせて返す", async () => {
    const db = fakeAdminDb({
      logs: [{ id: 1, at: "2026-09-23T00:00:00Z", question: "q", kind: "answer", reason: null, route: "cheap", ms: 500, answer_id: "a1" }],
      calls: [{ ask_log_id: 1, model: "claude-haiku-4-5", input_tokens: 1_000_000, cache_read_tokens: 0, cache_creation_tokens: 0, output_tokens: 0 }],
      reactionCounts: [{ answer_id: "a1", kind: "helpful", n: 2 }],
    });
    const res = await handleAdmin(new Request("https://admin.tw-context.dev/api/logs"), adminEnv(db), new URL("https://admin.tw-context.dev/api/logs"));
    expect(res.status).toBe(200);
    const body = (await res.json()) as { logs: { id: number; calls: number; cost_usd: number; reactions: Record<string, number> }[] };
    expect(body.logs).toHaveLength(1);
    expect(body.logs[0]!.calls).toBe(1);
    expect(body.logs[0]!.cost_usd).toBeCloseTo(1.0, 6); // 100 万 input トークン = $1
    expect(body.logs[0]!.reactions).toEqual({ helpful: 2 });
  });

  it("Access 検証に失敗したら 403", async () => {
    vi.mocked(access.verifyAccess).mockResolvedValue(false);
    const db = fakeAdminDb({});
    const res = await handleAdmin(new Request("https://admin.tw-context.dev/api/logs"), adminEnv(db), new URL("https://admin.tw-context.dev/api/logs"));
    expect(res.status).toBe(403);
  });
});

describe("GET /api/logs/:id", () => {
  it("ログ 1 件・呼び出し・反応・前後のやりとりをまとめて返す", async () => {
    const db = fakeAdminDb({
      logDetail: {
        id: 5, at: "2026-09-23T00:00:00Z", user: "u1", question: "q", qkey: null, prev_page: null,
        state: "{}", kind: "answer", reason: null, route: "cheap", answer_id: "a1", lead: "そうッピ。",
        body: JSON.stringify({ kind: "answer" }), ms: 300,
        understanding: JSON.stringify({ kind: "wiki" }), candidates: JSON.stringify([{ id: "p:1", page: "P", section: "S" }]),
      },
      detailCalls: [{ ask_log_id: 5, seq: 0, kind: "select", model: "claude-haiku-4-5", input_tokens: 100, cache_read_tokens: 0, cache_creation_tokens: 0, output_tokens: 10, ms: 400 }],
      reactions: [{ id: 1, day: "2026-09-23", answer_id: "a1", kind: "helpful" }],
      neighborsBefore: [{ id: 4, at: "2026-09-22T23:00:00Z", question: "前の質問", kind: "answer" }],
      neighborsAfter: [{ id: 6, at: "2026-09-23T01:00:00Z", question: "次の質問", kind: "none" }],
    });
    const res = await handleAdmin(
      new Request("https://admin.tw-context.dev/api/logs/5"), adminEnv(db), new URL("https://admin.tw-context.dev/api/logs/5"),
    );
    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      log: { id: number; understanding: unknown; candidates: unknown[] };
      calls: { cost_usd: number | null }[];
      reactions: unknown[];
      neighbors: { before: unknown[]; after: unknown[] };
    };
    expect(body.log.id).toBe(5);
    expect(body.log.understanding).toEqual({ kind: "wiki" });
    expect(body.log.candidates).toEqual([{ id: "p:1", page: "P", section: "S" }]);
    expect(body.calls[0]!.cost_usd).not.toBeNull();
    expect(body.reactions).toHaveLength(1);
    expect(body.neighbors.before).toHaveLength(1);
    expect(body.neighbors.after).toHaveLength(1);
  });

  it("存在しない id は 404", async () => {
    const db = fakeAdminDb({ logDetail: null });
    const res = await handleAdmin(
      new Request("https://admin.tw-context.dev/api/logs/999"), adminEnv(db), new URL("https://admin.tw-context.dev/api/logs/999"),
    );
    expect(res.status).toBe(404);
  });
});

describe("GET /api/costs", () => {
  it("日別に質問数・呼び出し・トークン・費用を集計する(単価不明モデルは null)", async () => {
    const db = fakeAdminDb({
      costRows: [
        { ask_log_id: 1, at: "2026-09-23T00:00:00Z", user: "u1", model: "claude-haiku-4-5", input_tokens: 1_000_000, cache_read_tokens: 0, cache_creation_tokens: 0, output_tokens: 0 },
        { ask_log_id: 2, at: "2026-09-23T05:00:00Z", user: "u2", model: "claude-haiku-4-5", input_tokens: 500_000, cache_read_tokens: 0, cache_creation_tokens: 0, output_tokens: 0 },
      ],
    });
    const res = await handleAdmin(new Request("https://admin.tw-context.dev/api/costs?days=7"), adminEnv(db), new URL("https://admin.tw-context.dev/api/costs?days=7"));
    expect(res.status).toBe(200);
    const body = (await res.json()) as { days: { date: string; questions: number; cost_usd: number }[]; top_users: { user: string }[] };
    expect(body.days).toHaveLength(1);
    expect(body.days[0]!.questions).toBe(2);
    expect(body.days[0]!.cost_usd).toBeCloseTo(1.5, 6);
    expect(body.top_users.map((u) => u.user).sort()).toEqual(["u1", "u2"]);
  });

  it("単価不明のモデルが混じると、その日の合計は null", async () => {
    const db = fakeAdminDb({
      costRows: [
        { ask_log_id: 1, at: "2026-09-23T00:00:00Z", user: "u1", model: "unknown-model", input_tokens: 100, cache_read_tokens: 0, cache_creation_tokens: 0, output_tokens: 0 },
      ],
    });
    const res = await handleAdmin(new Request("https://admin.tw-context.dev/api/costs"), adminEnv(db), new URL("https://admin.tw-context.dev/api/costs"));
    const body = (await res.json()) as { days: { cost_usd: number | null }[] };
    expect(body.days[0]!.cost_usd).toBeNull();
  });
});
