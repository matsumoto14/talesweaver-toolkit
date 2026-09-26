// 評価のハーネス: LLM を呼ばずに /ask の安い道を回す。理解・節選び・選択の 3 か所だけ差し替え、
// プロンプトを pending.jsonl に書き出して止まる。答え(answers.json)は外から入れる(Claude Code の
// サブエージェントが LLM の役をする)。答えが揃うまで同じコマンドを繰り返すと 1 段ずつ進む。
// 回す道(agent.ts)は往復が多く再現しにくいので止める(debug.loop = false)。
//
// 実行(npm test には混ざらない。HARNESS_DIR が無ければ skip):
//   HARNESS_DIR=<dir> HARNESS_DB=<ローカル D1 の .sqlite> [HARNESS_ONLY=q51,v01] [HARNESS_RUNS=3] \
//     npx vitest run eval/harness.test.ts
// 出力: <dir>/pending.jsonl(答えが要るプロンプト)・<dir>/results.json(採点)
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

import { describe, it, vi } from "vitest";
import { z } from "zod";

import type * as ClaudeModule from "../src/claude";

const DIR = process.env.HARNESS_DIR ?? "";
const ONLY = (process.env.HARNESS_ONLY ?? "").split(",").map((s) => s.trim()).filter(Boolean);
const RUNS = Number(process.env.HARNESS_RUNS ?? "1");

/** 答えの無いプロンプトに当たったら投げる。その問はこの回はここで止まる。 */
class Pending extends Error {}

/** system は問をまたいで同じ(ページ一覧は約 1 万トークン)なので、ファイルに 1 回だけ書いて名前で指す。 */
interface PendingItem { key: string; run: string; stage: string; system_file: string; user: string; schema: unknown }
const systems = new Map<string, string>();
const pending: PendingItem[] = [];
const answers: Record<string, unknown> = DIR && existsSync(join(DIR, "answers.json"))
  ? JSON.parse(readFileSync(join(DIR, "answers.json"), "utf8")) as Record<string, unknown>
  : {};
/** 同じ問を複数回まわすとき、回ごとに別の答えを取る(揺れを測る)。 */
let runTag = "1";

function oracle<T>(stage: string, system: string, user: string, schema: z.ZodType<T>): T | null {
  const key = createHash("sha256").update(`${runTag}\n${stage}\n${system}\n${user}`).digest("hex").slice(0, 16);
  if (!(key in answers)) {
    if (!pending.some((p) => p.key === key)) {
      const systemFile = `system-${createHash("sha256").update(system).digest("hex").slice(0, 8)}.txt`;
      systems.set(systemFile, system);
      pending.push({ key, run: runTag, stage, system_file: systemFile, user, schema: z.toJSONSchema(schema) });
    }
    throw new Pending(key);
  }
  const parsed = schema.safeParse(answers[key]);
  return parsed.success ? parsed.data : null; // 壊れた答えは API の parse 失敗と同じ扱い
}

const FAKE_CALL = { model: "harness", ms: 0, inputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, outputTokens: 0 };

vi.mock("../src/claude", async (importOriginal) => {
  const actual = await importOriginal<typeof ClaudeModule>();
  const prompt = await import("../src/prompt");
  const schema = await import("../src/schema");
  const tools = await import("../src/tools");
  return {
    ...actual,
    understand: async (_env: unknown, question: string, prev: never, pageMatches: string[], dir: string) => {
      const u = oracle("understand", `${actual.UNDERSTAND_SYSTEM}\n\n${dir}`,
        prompt.renderUnderstandPrompt(question, prev, pageMatches), schema.UnderstandSchema);
      return u ? { understanding: u, call: FAKE_CALL } : null;
    },
    pickSections: async (_env: unknown, question: string, outline: { page: string; anchor: string; section: string }[]) => {
      if (outline.length === 0) return null;
      const slotted = outline.map((o, i) => ({ ...o, slot: `s${String(i + 1).padStart(2, "0")}` }));
      const bySlot = new Map(slotted.map((o) => [o.slot, o]));
      const pick = oracle("section", actual.SECTION_SYSTEM, prompt.renderSectionPrompt(question, slotted), schema.SectionPickSchema);
      if (!pick) return null;
      const sections = pick.sections.map((s) => bySlot.get(s)).filter((o) => !!o).slice(0, 2)
        .map((o) => ({ page: o!.page, anchor: o!.anchor }));
      return { sections, call: FAKE_CALL };
    },
    select: async (_env: unknown, question: string, state: Record<string, number>, candidates: never[], columnNotes: Record<string, string>) => {
      const { slotToId, idToSlot } = tools.assignSlots(candidates);
      const user = prompt.renderCandidates(question, state, candidates, (id) => idToSlot.get(id) ?? id, columnNotes);
      const sel = oracle("select", prompt.SYSTEM_RULES, user, schema.SelectionSchema);
      return sel ? { selection: actual.resolveSlots(sel, slotToId), slotToId, call: FAKE_CALL } : null;
    },
  };
});

/** node:sqlite の上に D1 の読み取りだけを載せる(書き込み = ask_log などは読み取り専用で失敗させて捨てる)。 */
function d1(db: DatabaseSync): D1Database {
  const prepare = (sql: string) => {
    let args: unknown[] = [];
    const st = {
      bind: (...a: unknown[]) => { args = a; return st; },
      all: async () => ({ results: db.prepare(sql).all(...(args as never[])), success: true, meta: {} }),
      first: async (col?: string) => {
        const row = db.prepare(sql).get(...(args as never[])) as Record<string, unknown> | undefined;
        return row === undefined ? null : col ? row[col] : row;
      },
      run: async () => { throw new Error("harness: read-only"); },
    };
    return st;
  };
  return { prepare, batch: async () => { throw new Error("harness: read-only"); } } as unknown as D1Database;
}

function memoryKv(): KVNamespace {
  const m = new Map<string, string>();
  return {
    get: async (k: string) => m.get(k) ?? null,
    put: async (k: string, v: string) => { m.set(k, v); },
    delete: async (k: string) => { m.delete(k); },
  } as unknown as KVNamespace;
}

interface Expect { page: string; text_contains?: string }
interface Question { id: string; kind: string; question: string; need?: "all"; expect: Expect[]; state?: Record<string, number> }

describe.skipIf(!DIR)("harness", () => {
  it("runs", async () => {
    const worker = (await import("../src/index")).default;
    const { issueSessionToken } = await import("../src/auth");
    const env = {
      WIKI: d1(new DatabaseSync(process.env.HARNESS_DB ?? "", { readOnly: true })),
      API: memoryKv(),
      ANTHROPIC_API_KEY: "harness",
      NONCE_SECRET: "harness",
      SELECT_MODEL: "harness",
      UNDERSTAND_MODEL: "harness",
      RATE_LIMIT_DISABLED: "1",
    };
    const ctx = { waitUntil: (p: Promise<unknown>) => { void p.catch(() => {}); }, passThroughOnException: () => {}, props: {} } as unknown as ExecutionContext;
    const { token } = await issueSessionToken(env as never);
    const questions = (JSON.parse(readFileSync(new URL("./questions.json", import.meta.url), "utf8")) as { questions: Question[] })
      .questions.filter((q) => (ONLY.length === 0 || ONLY.includes(q.id)) && (q.expect.length > 0 || q.kind === "absent"));

    const results: { run: string; id: string; status: string; hit?: boolean; lead?: unknown; dropped?: unknown[]; units?: string[] }[] = [];
    for (let run = 1; run <= RUNS; run += 1) {
      runTag = String(run);
      for (const q of questions) {
        const req = new Request("https://harness.test/ask", {
          method: "POST",
          headers: { "content-type": "application/json", authorization: `Bearer ${token}`, "x-client-id": "harness" },
          body: JSON.stringify({ question: q.question, state: q.state ?? {}, prev: null, debug: { loop: false, cache: false } }),
        });
        const before = pending.length;
        const res = await worker.fetch(req, env as never, ctx);
        if (pending.length > before) { results.push({ run: runTag, id: q.id, status: "pending" }); continue; }
        const body = await res.json() as { kind?: string; reason?: string; lead?: unknown; dropped?: unknown[]; steps?: { units: { id: string; text?: string; cells?: Record<string, string> }[] }[] };
        const units = (body.steps ?? []).flatMap((s) => s.units);
        const textOf = (u: (typeof units)[number]): string => u.text ?? Object.values(u.cells ?? {}).join(" ");
        const hitOf = (e: Expect): boolean => units.some((u) => e.text_contains === undefined || textOf(u).includes(e.text_contains));
        // 答えが wiki に無い問(absent)は「答えなし」を正解に数える。答えた場合の良し悪しは lead を人が読む
        const hit = q.kind === "absent" ? body.kind === "none"
          : q.need === "all" ? q.expect.every(hitOf) : q.expect.some(hitOf);
        results.push({ run: runTag, id: q.id, status: `${res.status} ${body.kind ?? ""} ${body.reason ?? ""}`.trim(), hit, lead: body.lead, dropped: body.dropped, units: units.map((u) => u.id) });
      }
    }
    for (const [file, text] of systems) writeFileSync(join(DIR, file), text);
    writeFileSync(join(DIR, "pending.jsonl"), pending.map((p) => JSON.stringify(p)).join("\n"));
    writeFileSync(join(DIR, "results.json"), JSON.stringify(results, null, 2));
    const done = results.filter((r) => r.status !== "pending");
    console.log(`harness: pending ${pending.length} / done ${done.length} / hit ${done.filter((r) => r.hit).length}`);
  }, 600_000);
});
