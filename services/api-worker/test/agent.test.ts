// 回す道(段階 2、src/agent.ts)。モックした Anthropic client(messages.create が tool_use → answer
// の順に返す偽物)で、ループの歯止め(5 回で強制・重複拒否・札 40・resolveSlots・refusal・no-answer)を見る。
// createClient だけ差し替え、resolveSlots は実物を使う(vi.importActual)。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../src/claude", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/claude")>();
  return { ...actual, createClient: vi.fn() };
});

import { createClient } from "../src/claude";
import { runAgentLoop } from "../src/agent";
import { NONE_SELECTION } from "../src/schema";
import type { Selection } from "../src/schema";

interface FakeRow { [key: string]: unknown }

/** find_pages / search_units / get_rows / get_app_data が触るテーブルだけ持つ最小の D1。 */
function fakeDb(opts: { aliasHits?: FakeRow[]; unitBatches?: FakeRow[][] } = {}): D1Database {
  const aliasHits = opts.aliasHits ?? [];
  const unitBatches = opts.unitBatches ?? [];
  let unitCall = 0;

  return {
    prepare(sql: string) {
      const statement = {
        bind: (..._args: unknown[]) => statement,
        all: async <T = FakeRow>() => {
          let results: FakeRow[] = [];
          if (sql.includes("FROM alias") && sql.includes("LIMIT 10")) results = aliasHits;
          else if (sql.includes("FROM unit_fts")) {
            results = unitBatches[Math.min(unitCall, unitBatches.length - 1)] ?? [];
            unitCall += 1;
          } else if (sql.includes("FROM correction")) results = [];
          else if (sql.includes("FROM unit WHERE page")) results = [];
          return { results: results as unknown as T[], success: true, meta: {} } as D1Result<T>;
        },
        first: async () => null,
        run: async () => ({ success: true, meta: {} }) as D1Result,
      };
      return statement as unknown as D1PreparedStatement;
    },
  } as unknown as D1Database;
}

/** テキストブロック無しの tool_use だけの応答。 */
function toolUseResponse(name: string, input: unknown, inputTokens = 100): unknown {
  return {
    stop_reason: "tool_use",
    usage: { input_tokens: inputTokens },
    content: [{ type: "tool_use", id: `t_${Math.random().toString(36).slice(2)}`, name, input }],
  };
}

function refusalResponse(): unknown {
  return { stop_reason: "refusal", usage: { input_tokens: 50 }, content: [] };
}

function endTurnNoToolResponse(): unknown {
  return { stop_reason: "end_turn", usage: { input_tokens: 50 }, content: [{ type: "text", text: "…" }] };
}

const EMPTY_ANSWER: Selection = { none: true, verdict: "none", basis: [], missing: [], lead: "", steps: [] };

/**
 * `messages` は agent.ts がループ中ずっと同じ配列を push で書き換える(参照が同じ)ので、
 * 呼ばれた時点のスナップショット(構造化複製)を別に持たないと、あとから mock.calls を見ても
 * 最終状態しか見えない。`calls` に呼び出し時点の複製を積む。
 */
function fakeClient(responses: unknown[]): {
  create: ReturnType<typeof vi.fn>;
  calls: { messages: { role: string; content: unknown }[]; tool_choice?: unknown }[];
} {
  let i = 0;
  const calls: { messages: { role: string; content: unknown }[]; tool_choice?: unknown }[] = [];
  const create = vi.fn(async (params: { messages: unknown; tool_choice?: unknown }) => {
    calls.push(structuredClone({ messages: params.messages, tool_choice: params.tool_choice }) as never);
    const r = responses[Math.min(i, responses.length - 1)];
    i += 1;
    return r;
  });
  return { create, calls };
}

const BASE_INPUT = {
  question: "テシスコアの成功率は?",
  state: {},
  columnNotes: {},
  dict: [] as string[],
};

beforeEach(() => {
  vi.mocked(createClient).mockReset();
});

describe("runAgentLoop", () => {
  it("5 回ツールを呼んだら 6 回目は answer を強制する(tool_choice)", async () => {
    const messagesMock = fakeClient([
      toolUseResponse("find_pages", { name: "A" }),
      toolUseResponse("find_pages", { name: "B" }),
      toolUseResponse("find_pages", { name: "C" }),
      toolUseResponse("find_pages", { name: "D" }),
      toolUseResponse("find_pages", { name: "E" }),
      toolUseResponse("answer", EMPTY_ANSWER),
    ]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db: fakeDb(), env: {} as never, ...BASE_INPUT });

    expect(result).not.toBeNull();
    expect(result!.toolCalls).toBe(5);
    expect(messagesMock.create).toHaveBeenCalledTimes(6);
    expect(messagesMock.calls[5]!.tool_choice).toEqual({ type: "tool", name: "answer" });
  });

  it("同じツールを同じ引数で 2 回呼んだら is_error の tool_result を返し、実行し直さない", async () => {
    const messagesMock = fakeClient([
      toolUseResponse("find_pages", { name: "テシスコア" }),
      toolUseResponse("find_pages", { name: "テシスコア" }), // 同じ引数
      toolUseResponse("answer", EMPTY_ANSWER),
    ]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db: fakeDb(), env: {} as never, ...BASE_INPUT });

    expect(result).not.toBeNull();
    // 実際に実行された find_pages は 1 回だけ(2 回目は重複拒否)
    expect(result!.toolCalls).toBe(1);
    const lastUserMessage = messagesMock.calls[2]!.messages.at(-1)!;
    const toolResults = lastUserMessage.content as { is_error?: boolean; content: string }[];
    expect(toolResults[0]!.is_error).toBe(true);
    expect(toolResults[0]!.content).toBe("同じ引数で呼び直せない。answer を返せ");
  });

  it("札は 40 を超えない(超えた分は units に含まれない)", async () => {
    // 20 件ずつ返す 3 バッチ(id は全て別)。1 回目 20 札・2 回目 20 札(合計 40)・3 回目は 0 札。
    const batch = (offset: number): FakeRow[] =>
      Array.from({ length: 20 }, (_, i) => ({
        id: `p:テスト${offset + i}/top/1`, kind: "paragraph", page: `テスト${offset + i}`,
        section: "概要", anchor: "top", ord: 1, table_idx: null, text: `説明${offset + i}`, truncated: 0, score: 1,
      }));
    const db = fakeDb({ unitBatches: [batch(0), batch(20), batch(40)] });

    const messagesMock = fakeClient([
      toolUseResponse("search_units", { terms: ["テスト1"], page: null }),
      toolUseResponse("search_units", { terms: ["テスト2"], page: null }),
      toolUseResponse("search_units", { terms: ["テスト3"], page: null }),
      toolUseResponse("answer", EMPTY_ANSWER),
      toolUseResponse("answer", EMPTY_ANSWER),
    ]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db, env: {} as never, ...BASE_INPUT });

    expect(result).not.toBeNull();
    expect(result!.candidates.size).toBe(40);

    // 3 回目の search_units の tool_result には新しい札が 0(既に 40 埋まっている)
    const thirdToolResultCallIndex = 3; // 0:検索1 → 応答, 1:検索2 → 応答, 2:検索3 → 応答, 3:次の要求
    const toolResults = messagesMock.calls[thirdToolResultCallIndex]!.messages.at(-1)!.content as { content: string }[];
    // tool_result は「[wiki のデータ…]」の前置き 1 行 + JSON
    const body = String(toolResults[0]!.content);
    const parsed = JSON.parse(body.slice(body.indexOf("{"))) as { units: unknown[] };
    expect(parsed.units).toHaveLength(0);
  });

  it("answer の札は安定 ID に resolve される", async () => {
    const db = fakeDb({
      unitBatches: [[
        { id: "p:テシスコア/top/1", kind: "paragraph", page: "テシスコア", section: "概要", anchor: "top",
          ord: 1, table_idx: null, text: "テシスコアの説明", truncated: 0, score: 1 },
      ]],
    });
    const messagesMock = fakeClient([
      toolUseResponse("search_units", { terms: ["テシスコア"], page: null }),
      toolUseResponse("answer", {
        none: false, verdict: "yes", basis: ["u01"], missing: [],
        lead: "そうッピ。",
        steps: [{ units: ["u01"], columns: [], key_check: [""] }],
      }),
    ]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db, env: {} as never, ...BASE_INPUT });

    expect(result).not.toBeNull();
    expect(result!.selection.basis).toEqual(["p:テシスコア/top/1"]);
    expect(result!.selection.steps[0]!.units).toEqual(["p:テシスコア/top/1"]);
    expect(result!.candidates.has("p:テシスコア/top/1")).toBe(true);
  });

  it("毎回、最後のメッセージの最後のブロックにだけ cache_control を付ける", async () => {
    const messagesMock = fakeClient([
      toolUseResponse("find_pages", { name: "A" }),
      toolUseResponse("find_pages", { name: "B" }),
      toolUseResponse("answer", EMPTY_ANSWER),
    ]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    await runAgentLoop({ db: fakeDb(), env: {} as never, ...BASE_INPUT });

    type Block = { type: string; cache_control?: { type: string } };
    for (const call of messagesMock.calls) {
      const marked: string[] = [];
      call.messages.forEach((m, i) => {
        const blocks = Array.isArray(m.content) ? (m.content as Block[]) : [];
        blocks.forEach((b, j) => { if (b.cache_control) marked.push(`${i}:${j}`); });
      });
      const last = call.messages.at(-1)!;
      const lastBlocks = last.content as Block[];
      expect(marked).toEqual([`${call.messages.length - 1}:${lastBlocks.length - 1}`]);
      expect(lastBlocks.at(-1)!.cache_control).toEqual({ type: "ephemeral" });
    }
    // 1 回目は文字列だった user メッセージが text ブロックに変わっている
    expect((messagesMock.calls[0]!.messages[0]!.content as Block[])[0]!.type).toBe("text");
  });

  it("stop_reason: refusal は none(NONE_SELECTION)を返す", async () => {
    const messagesMock = fakeClient([refusalResponse()]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db: fakeDb(), env: {} as never, ...BASE_INPUT });

    expect(result).not.toBeNull();
    expect(result!.selection).toEqual(NONE_SELECTION);
  });

  it("ツールも answer も呼ばずに終わったら null(呼び元が安い道へ)", async () => {
    const messagesMock = fakeClient([endTurnNoToolResponse()]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db: fakeDb(), env: {} as never, ...BASE_INPUT });

    expect(result).toBeNull();
  });

  it("max_tokens が 2 回続いたら null", async () => {
    const maxTokensResponse = { stop_reason: "max_tokens", usage: { input_tokens: 50 }, content: [] };
    const messagesMock = fakeClient([maxTokensResponse, maxTokensResponse]);
    vi.mocked(createClient).mockReturnValue({ messages: messagesMock } as never);

    const result = await runAgentLoop({ db: fakeDb(), env: {} as never, ...BASE_INPUT });

    expect(result).toBeNull();
    expect(messagesMock.create).toHaveBeenCalledTimes(2);
  });
});
