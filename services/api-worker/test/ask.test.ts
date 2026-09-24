// POST /ask の経路。claude.ts(understand/select)を差し替え、D1 は fake の最小スタブ。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../src/claude", () => ({
  understand: vi.fn(),
  select: vi.fn(),
}));
vi.mock("../src/agent", () => ({
  runAgentLoop: vi.fn(),
}));

import * as agent from "../src/agent";
import * as claude from "../src/claude";
import { issueSessionToken } from "../src/auth";
import type { AuthEnv } from "../src/auth";
import worker, { type Env } from "../src/index";
import { fakeCtx } from "./ctx";

const ctx = fakeCtx();
import { NONE_UNDERSTAND } from "../src/schema";
import type { Understand } from "../src/schema";
import type { CallInfo, UnderstandResult } from "../src/claude";

/** claude.ts の呼び出し記録の最小のダミー(個々の値はこのテストでは見ない)。 */
const FAKE_CALL: CallInfo = { model: "claude-haiku-4-5", ms: 1, inputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 0 };
function understandResult(understanding: Understand): UnderstandResult {
  return { understanding, call: FAKE_CALL };
}

interface FakeRow { [key: string]: unknown }

function fakeDb(data: {
  meta?: FakeRow[];
  alias?: FakeRow[];
  aliasExact?: boolean;
  findPages?: FakeRow[];
  units?: FakeRow[];
  tableRows?: FakeRow[];
  pages?: FakeRow[];
  columnNotes?: FakeRow[];
  corrections?: FakeRow[];
  tableInfo?: FakeRow | null;
  unitLinks?: FakeRow[];
}): D1Database {
  const meta = data.meta ?? [];
  const alias = data.alias ?? [];
  const findPages = data.findPages ?? [];
  const units = data.units ?? [];
  const tableRows = data.tableRows ?? [];
  const pages = data.pages ?? [];
  const columnNotes = data.columnNotes ?? [];
  const corrections = data.corrections ?? [];
  const tableInfo = data.tableInfo ?? null;
  const unitLinks = data.unitLinks ?? [];

  return {
    prepare(sql: string) {
      const statement = {
        bind: (..._args: unknown[]) => statement,
        all: async <T = FakeRow>() => {
          let results: FakeRow[] = [];
          if (sql.includes("SELECT 1 FROM alias")) results = [];
          else if (sql.includes("FROM alias") && sql.includes("LIMIT 10")) results = findPages;
          else if (sql.includes("FROM alias")) results = alias;
          else if (sql.includes("FROM unit_link")) results = unitLinks;
          else if (sql.includes("FROM unit_fts")) results = units;
          else if (sql.includes("FROM unit WHERE page")) results = tableRows;
          else if (sql.includes("FROM meta")) results = meta;
          else if (sql.includes("FROM column_note")) results = columnNotes;
          else if (sql.includes("FROM correction")) results = corrections;
          else if (sql.includes("FROM page") && sql.includes("IN")) results = pages;
          else if (sql.includes("FROM page")) results = pages.map((p) => ({ name: p.name }));
          return { results: results as unknown as T[], success: true, meta: {} } as D1Result<T>;
        },
        first: async <T = FakeRow>() => {
          if (sql.includes("SELECT 1 FROM alias")) return (data.aliasExact ? { 1: 1 } : null) as T;
          if (sql.includes("FROM wiki_table")) return (tableInfo as T) ?? null;
          return null;
        },
        run: async () => ({ success: true, meta: { last_row_id: 1 } }) as D1Result,
      };
      return statement as unknown as D1PreparedStatement;
    },
    batch: async (statements: D1PreparedStatement[]) => statements.map(() => ({ success: true, meta: {} }) as D1Result),
  } as unknown as D1Database;
}

function memoryKv(): KVNamespace & { store: Map<string, string> } {
  const store = new Map<string, string>();
  return {
    store,
    get: async (key: string) => store.get(key) ?? null,
    put: async (key: string, value: string) => { store.set(key, value); },
  } as unknown as KVNamespace & { store: Map<string, string> };
}

const HITS: FakeRow[] = [
  { id: "p:テシスコア/top/1", kind: "paragraph", page: "テシスコア", section: "概要", anchor: "top", ord: 1, table_idx: null, text: "テシスコアの説明", truncated: 0, score: 1 },
  { id: "r:テシスコア/h2_1/1/進0-強0", kind: "row", page: "テシスコア", section: "強化", anchor: "h2_1", ord: 2, table_idx: 1, text: "進化: 0 | 成功率: 30%", truncated: 0, score: 1 },
];
const ROW_UNIT: FakeRow = {
  id: "r:テシスコア/h2_1/1/進0-強0", kind: "row", page: "テシスコア", section: "強化", anchor: "h2_1", ord: 2,
  table_idx: 1, group_key: "テシスコア\u0000h2_1\u00001", row_key: "進0-強0", truncated: 0,
  text: "進化: 0 | 成功率: 30%", cells: JSON.stringify({ "進化": "0", "成功率": "30%" }), nums: JSON.stringify({ "進化": 0 }),
};

function baseEnv(overrides: Partial<Parameters<typeof fakeDb>[0]> = {}): Env {
  return {
    WIKI: fakeDb({
      meta: [{ key: "synced_at", value: "2026-09-20T00:00:00Z" }],
      alias: [{ name: "テシスコア", page: "テシスコア" }],
      findPages: [{ name: "テシスコア", page: "テシスコア" }],
      units: HITS,
      tableRows: [ROW_UNIT],
      pages: [{ name: "テシスコア", url: "https://talewiki.com/?%A5%C6" }],
      columnNotes: [{ name: "進化", note: "コアの進化段階", state_key: "evolution" }],
      tableInfo: { caption: "強化に必要なもの", row_count: 1 },
      ...overrides,
    }),
    API: memoryKv(),
    ANTHROPIC_API_KEY: "sk-ant-test",
    NONCE_SECRET: "test-secret",
    SELECT_MODEL: "claude-haiku-4-5",
    UNDERSTAND_MODEL: "claude-haiku-4-5",
    RATE_LIMIT_PER_DAY: "100",
  };
}

async function bearerFor(env: Env): Promise<string> {
  const { token } = await issueSessionToken(env as unknown as AuthEnv);
  return token;
}

function askRequest(body: unknown, token: string): Request {
  return new Request("https://worker.test/ask", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
}

beforeEach(() => {
  vi.mocked(claude.understand).mockReset();
  vi.mocked(claude.select).mockReset();
  // 既定では回す道は使わない(null = 経路の故障・不使用。呼び元は安い道の結果へ)。
  // 回す道そのものの振る舞いは agent.test.ts、振り分けは下の「回す道への振り分け」で見る。
  vi.mocked(agent.runAgentLoop).mockReset().mockResolvedValue(null);
});

describe("認証・秘密", () => {
  it("トークンが無ければ 401", async () => {
    const env = baseEnv();
    const res = await worker.fetch(
      new Request("https://worker.test/ask", { method: "POST", body: JSON.stringify({ question: "x" }) }),
      env, ctx,
    );
    expect(res.status).toBe(401);
    expect((await res.json()) as { error: string }).toMatchObject({ error: "セッションが切れました" });
  });

  it("ANTHROPIC_API_KEY が無ければ 503", async () => {
    const env = baseEnv();
    env.ANTHROPIC_API_KEY = "";
    const token = await bearerFor(env);
    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    expect(res.status).toBe(503);
    expect((await res.json()) as { error: string }).toMatchObject({ error: expect.stringContaining("ANTHROPIC_API_KEY") });
  });
});

describe("/ask の経路", () => {
  it("理解が smalltalk でも、質問の語で索引に当たりがあれば wiki として進める(聖水の実例 2026-09-23)", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, kind: "smalltalk" }));
    vi.mocked(claude.select).mockResolvedValue({ selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] }, slotToId: new Map(), call: FAKE_CALL });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアはどうやって稼ぐ?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; reason: string; dropped: { what: string; why: string }[] };

    expect(body.reason).toBe("llm_none"); // 雑談ではなく検索 → 選択まで進んだ
    expect(claude.select).toHaveBeenCalled();
    expect(body.dropped).toEqual(expect.arrayContaining([{ what: "kind", why: "smalltalk→wiki(索引に当たりあり)" }]));
  });

  it("理解が smalltalk なら検索も選択も呼ばず none を返す", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, kind: "smalltalk" }));
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "こんにちは" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; reason: string };

    expect(res.status).toBe(200);
    expect(body).toMatchObject({ kind: "none", reason: "smalltalk" });
    expect(claude.select).not.toHaveBeenCalled();
  });

  it("理解が落ちたらコード経路(wiki 扱い)で進む", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(null);
    vi.mocked(claude.select).mockResolvedValue({ selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] }, slotToId: new Map(), call: FAKE_CALL });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    expect(res.status).toBe(200);
    expect(claude.select).toHaveBeenCalled();
  });

  it("選択が none なら reason llm_none + 検索結果", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({ selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] }, slotToId: new Map(), call: FAKE_CALL });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; reason: string; search: unknown[] };

    expect(body.kind).toBe("none");
    expect(body.reason).toBe("llm_none");
    expect(body.search.length).toBeGreaterThan(0);
  });

  it("検証で手順が全部落ちたら verification_failed", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "none", basis: [], missing: [], lead: "", computed: false,
        steps: [{ units: ["存在しない札"], columns: [], key_check: [] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; reason: string };

    expect(body.kind).toBe("none");
    expect(body.reason).toBe("verification_failed");
  });

  it("通常の経路: 検証を通れば answer JSON の形で返す", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: [],
        lead: "進化 {{r:テシスコア/h2_1/1/進0-強0.進化}} のいまは成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} から始めるッピ。",
        computed: false,
        steps: [
          { units: ["p:テシスコア/top/1"], columns: [], key_check: [] },
          { units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] },
        ],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(
      askRequest({ question: "テシスコアの成功率は?", state: { level: 63, evolution: 0 } }, token),
      env, ctx,
    );
    const body = (await res.json()) as {
      kind: string; lead: unknown[]; steps: { units: unknown[] }[]; answer_id: string; route: string;
    };

    expect(res.status).toBe(200);
    expect(body.kind).toBe("answer");
    expect(body.lead).not.toBeNull();
    expect(body.steps).toHaveLength(2);
    expect(body.answer_id).toMatch(/^a_[0-9a-f]{16}$/);
    expect(body.route).toBe("cheap");
  });

  it("lead に LLM の計算が含まれるとき(selection.computed)は answer.computed が true になる", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "none", basis: [], missing: [],
        lead: "{{r:テシスコア/h2_1/1/進0-強0.成功率}} × {{r:テシスコア/h2_1/1/進0-強0.成功率}} ≒ 18%ッピ。",
        computed: true,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "成功率を 2 連続で引く確率は?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; lead: unknown[]; computed: boolean };

    expect(body.kind).toBe("answer");
    expect(body.lead).not.toBeNull();
    expect(body.computed).toBe(true);
  });

  it("trouble: cant_win なら answer に playbook: cant_win が載る", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, trouble: "cant_win" }));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: [],
        lead: "成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} ッピ。",
        computed: false,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアが強すぎて勝てない" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; playbook: string | null };

    expect(body.kind).toBe("answer");
    expect(body.playbook).toBe("cant_win");
  });

  it("trouble: cant_win で選択が none でも kind:none の応答に playbook が載る", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, trouble: "cant_win" }));
    vi.mocked(claude.select).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアが強すぎて勝てない" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; reason: string; playbook: string | null };

    expect(body.kind).toBe("none");
    expect(body.reason).toBe("llm_none");
    expect(body.playbook).toBe("cant_win");
  });

  it("trouble: none なら playbook は null", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { playbook: string | null };

    expect(body.playbook).toBeNull();
  });

  it("debug.understand: false なら理解を呼ばない", async () => {
    const env = baseEnv();
    vi.mocked(claude.select).mockResolvedValue({ selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] }, slotToId: new Map(), call: FAKE_CALL });
    const token = await bearerFor(env);

    await worker.fetch(askRequest({ question: "テシスコアの成功率は?", debug: { understand: false } }, token), env, ctx);

    expect(claude.understand).not.toHaveBeenCalled();
  });

  it("followup: true かつ直前のページが実在すれば boost し、応答に followup を載せる", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, followup: true }));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: [],
        lead: "成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} ッピ。",
        computed: false,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(
      askRequest({ question: "それの成功率は?", prev: { question: "テシスコアって何?", page: "テシスコア" } }, token),
      env, ctx,
    );
    const body = (await res.json()) as { followup: { page: string } | null };

    expect(body.followup).toEqual({ page: "テシスコア" });
  });

  it("followup: true でも直前のページが存在しなければ無効(followup は null)", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, followup: true }));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: [],
        lead: "成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} ッピ。",
        computed: false,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(
      askRequest({ question: "それの成功率は?", prev: { question: "x", page: "存在しないページ" } }, token),
      env, ctx,
    );
    const body = (await res.json()) as { followup: { page: string } | null };

    expect(body.followup).toBeNull();
  });

  it("hops: multi は dropped に route:hops:multi を記録するだけ(route は cheap のまま)", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, hops: "multi" }));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "none", basis: [], missing: [], lead: "", computed: false,
        steps: [{ units: ["p:テシスコア/top/1"], columns: [], key_check: [] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { route: string; dropped: { what: string; why: string }[] };

    expect(body.route).toBe("cheap");
    expect(body.dropped).toEqual(expect.arrayContaining([{ what: "route", why: "hops:multi" }]));
  });

  it("次の一手: 選ばれたユニットの unit_link から実在ページを最大 3 件、自ページ除外・重複除外", async () => {
    const env = baseEnv({
      pages: [
        { name: "テシスコア", url: "https://talewiki.com/?%A5%C6" },
        { name: "モンスター/モルペウス", url: "https://talewiki.com/?モルペウス" },
      ],
      unitLinks: [
        { unit_id: "p:テシスコア/top/1", page: "テシスコア", ord: 0 },
        { unit_id: "p:テシスコア/top/1", page: "モンスター/モルペウス", ord: 1 },
        { unit_id: "p:テシスコア/top/1", page: "モンスター/モルペウス", ord: 2 },
        { unit_id: "p:テシスコア/top/1", page: "存在しないページ", ord: 3 },
      ],
    });
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "none", basis: [], missing: [], lead: "", computed: false,
        steps: [{ units: ["p:テシスコア/top/1"], columns: [], key_check: [] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { next: { question: string; page: string }[] };

    expect(body.next).toEqual([{ question: "「モルペウス」はどこで手に入る?", page: "モンスター/モルペウス" }]);
  });

  it("unit_link が無ければ next は空配列", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "none", basis: [], missing: [], lead: "", computed: false,
        steps: [{ units: ["p:テシスコア/top/1"], columns: [], key_check: [] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { next: unknown[] };

    expect(body.next).toEqual([]);
  });
});

function sseAskRequest(body: unknown, token: string): Request {
  return new Request("https://worker.test/ask", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}`, accept: "text/event-stream" },
    body: JSON.stringify(body),
  });
}

function parseSse(text: string): { event: string; data: Record<string, unknown> }[] {
  return text
    .trim()
    .split("\n\n")
    .filter((chunk) => chunk.length > 0)
    .map((chunk) => {
      const [eventLine, dataLine] = chunk.split("\n");
      return {
        event: (eventLine ?? "").replace("event: ", ""),
        data: JSON.parse((dataLine ?? "").replace("data: ", "")) as Record<string, unknown>,
      };
    });
}

describe("/ask の SSE(Accept: text/event-stream)", () => {
  it("progress(understand → search → select)のあと result を 1 回流す", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(sseAskRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("text/event-stream");

    const events = parseSse(await res.text());
    const progressSteps = events.filter((e) => e.event === "progress").map((e) => e.data.step);

    expect(progressSteps).toEqual(["understand", "search", "select"]);
    expect(events.at(-1)!.event).toBe("result");
    expect((events.at(-1)!.data as { kind: string }).kind).toBe("none");
  });

  it("Claude 呼び出し後の 502 は event: error で返す", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue(null);
    const token = await bearerFor(env);

    const res = await worker.fetch(sseAskRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    expect(res.status).toBe(200); // SSE の枠自体は開始済み。エラーは event: error で伝える

    const events = parseSse(await res.text());
    const last = events.at(-1)!;

    expect(last.event).toBe("error");
    expect(last.data.status).toBe(502);
  });

  it("401(セッション切れ)は SSE を始める前に通常の HTTP ステータスで返す", async () => {
    const env = baseEnv();
    const res = await worker.fetch(
      new Request("https://worker.test/ask", {
        method: "POST",
        headers: { accept: "text/event-stream" },
        body: JSON.stringify({ question: "x" }),
      }),
      env, ctx,
    );

    expect(res.status).toBe(401);
    expect(res.headers.get("content-type")).not.toBe("text/event-stream");
  });

  it("ANTHROPIC_API_KEY が無ければ SSE を始める前に 503 を返す", async () => {
    const env = baseEnv();
    env.ANTHROPIC_API_KEY = "";
    const token = await bearerFor(env);

    const res = await worker.fetch(
      new Request("https://worker.test/ask", {
        method: "POST",
        headers: { authorization: `Bearer ${token}`, accept: "text/event-stream" },
        body: JSON.stringify({ question: "テシスコアの成功率は?" }),
      }),
      env, ctx,
    );

    expect(res.status).toBe(503);
    expect(res.headers.get("content-type")).not.toBe("text/event-stream");
  });
});

describe("回す道への振り分け(agent.ts は runAgentLoop をモックして見る)", () => {
  it("hops: multi なら最初から回す道(安い道の select は呼ばない)", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, hops: "multi" }));
    vi.mocked(agent.runAgentLoop).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: [],
        lead: "成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} ッピ。",
        computed: false,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      candidates: new Map([["r:テシスコア/h2_1/1/進0-強0", {
        id: "r:テシスコア/h2_1/1/進0-強0", kind: "row", page: "テシスコア", section: "強化", anchor: "h2_1",
        ord: 2, table_idx: 1, group_key: "テシスコア h2_1 1", row_key: "進0-強0", text: "",
        truncated: false, cells: { "成功率": "30%" }, nums: null,
      }]]),
      toolCalls: 2,
      calls: [],
      ms: 900,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "エタ解放までの流れは?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; route: string; dropped: { what: string; why: string }[] };

    expect(res.status).toBe(200);
    expect(body.kind).toBe("answer");
    expect(body.route).toBe("loop");
    expect(claude.select).not.toHaveBeenCalled();
    expect(body.dropped).toEqual(expect.arrayContaining([
      { what: "route", why: "hops:multi" },
      { what: "route", why: "loop:2回/900ms" },
    ]));
  });

  it("安い道が none なら 1 回だけ回す道でやり直す(route: cheap_then_loop)", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      slotToId: new Map(), call: FAKE_CALL,
    });
    vi.mocked(agent.runAgentLoop).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: [],
        lead: "成功率 {{r:テシスコア/h2_1/1/進0-強0.成功率}} ッピ。",
        computed: false,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      candidates: new Map([["r:テシスコア/h2_1/1/進0-強0", {
        id: "r:テシスコア/h2_1/1/進0-強0", kind: "row", page: "テシスコア", section: "強化", anchor: "h2_1",
        ord: 2, table_idx: 1, group_key: "テシスコア h2_1 1", row_key: "進0-強0", text: "",
        truncated: false, cells: { "成功率": "30%" }, nums: null,
      }]]),
      toolCalls: 3,
      calls: [],
      ms: 1200,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; route: string };

    expect(res.status).toBe(200);
    expect(body.kind).toBe("answer");
    expect(body.route).toBe("cheap_then_loop");
    expect(agent.runAgentLoop).toHaveBeenCalledTimes(1);
  });

  it("安い道が missing 付きの答えで、回す道が none なら、安い道の部分的な答えを残す", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: {
        none: false, verdict: "yes", basis: ["r:テシスコア/h2_1/1/進0-強0"], missing: ["where"],
        lead: "この段から始めるッピ。",
        computed: false,
        steps: [{ units: ["r:テシスコア/h2_1/1/進0-強0"], columns: ["成功率"], key_check: ["進0-強0"] }],
      },
      slotToId: new Map(), call: FAKE_CALL,
    });
    vi.mocked(agent.runAgentLoop).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      candidates: new Map(),
      toolCalls: 5,
      calls: [],
      ms: 9000,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; route: string; missing: string[]; steps: unknown[] };

    expect(res.status).toBe(200);
    expect(body.kind).toBe("answer");
    expect(body.route).toBe("cheap");
    expect(body.missing).toEqual(["where"]);
    expect(body.steps).toHaveLength(1);
    expect(agent.runAgentLoop).toHaveBeenCalledTimes(1);
  });

  it("安い道も回す道も駄目なら安い道の結果(none)をそのまま返す", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult(NONE_UNDERSTAND));
    vi.mocked(claude.select).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      slotToId: new Map(), call: FAKE_CALL,
    });
    vi.mocked(agent.runAgentLoop).mockResolvedValue(null);
    const token = await bearerFor(env);

    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, token), env, ctx);
    const body = (await res.json()) as { kind: string; reason: string };

    expect(res.status).toBe(200);
    expect(body.kind).toBe("none");
    expect(body.reason).toBe("llm_none");
    expect(agent.runAgentLoop).toHaveBeenCalledTimes(1);
  });

  it("debug.loop: false なら回す道を一切試さない", async () => {
    const env = baseEnv();
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ ...NONE_UNDERSTAND, hops: "multi" }));
    vi.mocked(claude.select).mockResolvedValue({
      selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] },
      slotToId: new Map(), call: FAKE_CALL,
    });
    const token = await bearerFor(env);

    const res = await worker.fetch(
      askRequest({ question: "テシスコアの成功率は?", debug: { loop: false } }, token),
      env, ctx,
    );
    const body = (await res.json()) as { kind: string };

    expect(res.status).toBe(200);
    expect(body.kind).toBe("none");
    expect(agent.runAgentLoop).not.toHaveBeenCalled();
  });
});

describe("上限の消費と記録(ask_log)", () => {
  it("空の質問は 400 で、1 日の上限を消費しない", async () => {
    const env = baseEnv();
    const token = await bearerFor(env);
    const res = await worker.fetch(askRequest({ question: "   " }, token), env, ctx);
    expect(res.status).toBe(400);
    const kv = env.API as unknown as { store: Map<string, string> };
    expect([...kv.store.keys()].some((k) => k.startsWith("rate:"))).toBe(false);
  });

  it("答えを返したら ask_log に質問と答えを書く(waitUntil)", async () => {
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ kind: "smalltalk", pages: [], terms: [], hops: "single", followup: false, mood: "ask", trouble: "none" }));
    const env = baseEnv();
    const inserted: { sql: string; args: unknown[] }[] = [];
    const db = env.WIKI;
    const originalPrepare = db.prepare.bind(db);
    db.prepare = ((sql: string) => {
      const stmt = originalPrepare(sql);
      if (sql.includes("INSERT INTO ask_log")) {
        const originalBind = stmt.bind.bind(stmt);
        stmt.bind = ((...args: unknown[]) => { inserted.push({ sql, args }); return originalBind(...args); }) as typeof stmt.bind;
      }
      return stmt;
    }) as typeof db.prepare;
    const waited: Promise<unknown>[] = [];
    const recordingCtx = { ...fakeCtx(), waitUntil: (p: Promise<unknown>) => { waited.push(p); } } as unknown as ExecutionContext;

    const token = await bearerFor(env);
    const res = await worker.fetch(askRequest({ question: "こんにちは" }, token), env, recordingCtx);
    expect(res.status).toBe(200);
    await Promise.all(waited);

    expect(inserted).toHaveLength(1);
    const args = inserted[0]!.args;
    expect(args[2]).toBe("こんにちは"); // question
    expect(typeof args[3]).toBe("string"); // qkey(prev 無しなので作る)
    expect(args[6]).toBe("none"); // kind
    expect(args[7]).toBe("smalltalk"); // reason
    expect(JSON.parse(String(args[11]))).toMatchObject({ kind: "none", reason: "smalltalk" }); // body
  });
});

describe("答えのキャッシュ(役に立った答えを同じ意味の質問に返す)", () => {
  const CACHED_BODY = {
    kind: "answer", lead: [{ t: "そうッピ。" }], steps: [], next: [], synced_at: "2026-09-20T00:00:00Z",
    model: "claude-haiku-4-5", route: "cheap", playbook: null, answer_id: "a_orig", corrections: [],
    missing: [], verdict: "yes", basis: [], dropped: [], followup: null,
  };

  async function seed(env: Env, question: string, body: unknown = CACHED_BODY, state: Record<string, number> = {}): Promise<void> {
    const { questionKey, putCachedAnswer, wikiVersion } = await import("../src/cache");
    const { getMeta } = await import("../src/retrieve");
    const { buildDict } = await import("../src/segment");
    // Worker が使う辞書と同じ組み方(alias の名前 + BASE_WORDS)
    const qkey = questionKey(question, buildDict(["テシスコア"]));
    await putCachedAnswer(env.API!, qkey, wikiVersion(await getMeta(env.WIKI)), body as never, state);
  }

  it("語順・空白が違うだけの質問に LLM を呼ばず返し、answer_id は新しく route は cached", async () => {
    const env = baseEnv();
    await seed(env, "テシスコアの成功率は?");
    const token = await bearerFor(env);
    const res = await worker.fetch(askRequest({ question: "成功率 テシスコア" }, token), env, ctx);
    expect(res.status).toBe(200);
    const body = (await res.json()) as { kind: string; route: string; answer_id: string; dropped: { why: string }[] };
    expect(body.kind).toBe("answer");
    expect(body.route).toBe("cached");
    expect(body.answer_id).not.toBe("a_orig");
    expect(body.dropped.map((d) => d.why)).toContain("cached");
    expect(claude.understand).not.toHaveBeenCalled();
  });

  it("SSE でも result 1 つで返す", async () => {
    const env = baseEnv();
    await seed(env, "テシスコアの成功率は?");
    const token = await bearerFor(env);
    const req = askRequest({ question: "テシスコアの成功率は?" }, token);
    req.headers.set("accept", "text/event-stream");
    const res = await worker.fetch(req, env, ctx);
    expect(res.headers.get("content-type")).toBe("text/event-stream");
    const text = await res.text();
    expect(text.startsWith("event: result\ndata: ")).toBe(true);
    expect(text.endsWith("\n\n")).toBe(true);
  });

  it("キャラ状態が違っても返す。答えが状態で行を絞っていたときだけ同じ状態に限る", async () => {
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ kind: "wiki", pages: [], terms: [], hops: "single", followup: false, mood: "ask", trouble: "none" }));
    vi.mocked(claude.select).mockResolvedValue({ selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] }, slotToId: new Map(), call: FAKE_CALL });
    const env = baseEnv();
    await seed(env, "テシスコアの成功率は?", CACHED_BODY, { evolution: 3 });
    const token = await bearerFor(env);
    const other = await worker.fetch(askRequest({ question: "テシスコアの成功率は?", state: { evolution: 5 } }, token), env, ctx);
    expect(((await other.json()) as { route: string }).route).toBe("cached");

    const env2 = baseEnv();
    const filtered = { ...CACHED_BODY, steps: [{ title: "強化", units: [], columns: [], sources: [], filtered_by: { "進化": "3" } }] };
    await seed(env2, "テシスコアの成功率は?", filtered, { evolution: 3 });
    const token2 = await bearerFor(env2);
    const same = await worker.fetch(askRequest({ question: "テシスコアの成功率は?", state: { evolution: 3 } }, token2), env2, ctx);
    expect(((await same.json()) as { route: string }).route).toBe("cached");
    const diff = await worker.fetch(askRequest({ question: "テシスコアの成功率は?", state: { evolution: 5 } }, token2), env2, ctx);
    expect(((await diff.json()) as { kind: string }).kind).toBe("none");
  });

  it("続きの質問(prev あり)と wiki の版が違うときは引かない", async () => {
    vi.mocked(claude.understand).mockResolvedValue(understandResult({ kind: "wiki", pages: [], terms: [], hops: "single", followup: false, mood: "ask", trouble: "none" }));
    vi.mocked(claude.select).mockResolvedValue({ selection: { none: true, verdict: "none", basis: [], missing: [], lead: "", computed: false, steps: [] }, slotToId: new Map(), call: FAKE_CALL });
    const env = baseEnv();
    await seed(env, "テシスコアの成功率は?");
    const token = await bearerFor(env);
    const withPrev = await worker.fetch(
      askRequest({ question: "テシスコアの成功率は?", prev: { question: "x", page: "テシスコア" } }, token), env, ctx,
    );
    expect(((await withPrev.json()) as { kind: string }).kind).toBe("none");
    expect(claude.understand).toHaveBeenCalledTimes(1);

    const stale = baseEnv({ meta: [{ key: "synced_at", value: "2026-09-21T00:00:00Z" }] });
    (stale as { API: KVNamespace }).API = env.API!;
    const res = await worker.fetch(askRequest({ question: "テシスコアの成功率は?" }, await bearerFor(stale)), stale, ctx);
    expect(((await res.json()) as { kind: string }).kind).toBe("none");
    expect(claude.understand).toHaveBeenCalledTimes(2);
  });
});
