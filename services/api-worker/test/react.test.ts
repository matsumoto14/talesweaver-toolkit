// POST /react。answer_id につき 1 回、question は同意時だけ保存。
import { describe, expect, it } from "vitest";

import { issueSessionToken } from "../src/auth";
import type { AuthEnv } from "../src/auth";
import worker, { type Env } from "../src/index";

function memoryKv(): KVNamespace {
  const store = new Map<string, string>();
  return {
    get: async (key: string) => store.get(key) ?? null,
    put: async (key: string, value: string) => { store.set(key, value); },
  } as unknown as KVNamespace;
}

function fakeWiki(onInsert: (values: unknown[]) => void, onUpdate: (values: unknown[]) => void = () => {}): D1Database {
  return {
    prepare(sql: string) {
      const statement = {
        bind: (...args: unknown[]) => {
          if (sql.includes("INSERT INTO reaction")) onInsert(args);
          if (sql.includes("UPDATE reaction")) onUpdate(args);
          return statement;
        },
        run: async () => ({ success: true, meta: {} }) as D1Result,
      };
      return statement as unknown as D1PreparedStatement;
    },
  } as unknown as D1Database;
}

function makeEnv(onInsert: (values: unknown[]) => void, onUpdate?: (values: unknown[]) => void): Env {
  return {
    WIKI: fakeWiki(onInsert, onUpdate),
    API: memoryKv(),
    NONCE_SECRET: "test-secret",
    SELECT_MODEL: "m",
    UNDERSTAND_MODEL: "m",
  };
}

async function bearerFor(env: Env): Promise<string> {
  const { token } = await issueSessionToken(env as unknown as AuthEnv);
  return token;
}

function reactRequest(body: unknown, token: string): Request {
  return new Request("https://worker.test/react", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
}

describe("POST /react", () => {
  it("helpful を保存する", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    const res = await worker.fetch(
      reactRequest({ answer_id: "a_1", kind: "helpful", unit_ids: ["r:x/1"] }, token),
      env,
    );

    expect(res.status).toBe(200);
    expect(inserts).toHaveLength(1);
    // bind の並びは (day, answer_id, kind, reason, unit_ids, question)
    const [day, answerId, kind, reason, unitIds, question] = inserts[0]!;
    expect(day).toEqual(expect.any(String));
    expect(answerId).toBe("a_1");
    expect(kind).toBe("helpful");
    expect(reason).toBeNull();
    expect(unitIds).toBe(JSON.stringify(["r:x/1"]));
    expect(question).toBeNull();
  });

  it("同じ answer_id・同じ種類の 2 回目は理由と同意した質問文の追記(UPDATE)になる", async () => {
    const inserts: unknown[][] = [];
    const updates: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v), (v) => updates.push(v));
    const token = await bearerFor(env);

    await worker.fetch(reactRequest({ answer_id: "a_3", kind: "wrong", unit_ids: [] }, token), env);
    const second = await worker.fetch(
      reactRequest({ answer_id: "a_3", kind: "wrong", reason: "outdated", unit_ids: [], question: "エタ解放は?" }, token),
      env,
    );

    expect(second.status).toBe(200);
    expect(inserts).toHaveLength(1);
    expect(updates).toHaveLength(1);
    expect(updates[0]).toEqual(["a_3", "outdated", "エタ解放は?", "wrong"]);
  });

  it("同じ answer_id で種類が違う 2 回目は無視する(200 のまま)", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    await worker.fetch(reactRequest({ answer_id: "a_2", kind: "helpful", unit_ids: [] }, token), env);
    const second = await worker.fetch(reactRequest({ answer_id: "a_2", kind: "wrong", unit_ids: [] }, token), env);

    expect(second.status).toBe(200);
    expect(inserts).toHaveLength(1);
  });

  it("question は同意(question を送った)ときだけ保存される", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    await worker.fetch(
      reactRequest({ answer_id: "a_3", kind: "wrong", reason: "outdated", unit_ids: [], question: "この値は違うのでは" }, token),
      env,
    );

    const questionArg = inserts[0]?.[5];
    expect(questionArg).toBe("この値は違うのでは");
  });

  it("question を送らなければ null で保存する", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    await worker.fetch(reactRequest({ answer_id: "a_4", kind: "wrong", unit_ids: [] }, token), env);

    expect(inserts[0]?.[5]).toBeNull();
  });

  it("answer_id か kind が無ければ 400", async () => {
    const env = makeEnv(() => {});
    const token = await bearerFor(env);

    const res = await worker.fetch(reactRequest({ kind: "helpful" }, token), env);
    expect(res.status).toBe(400);
  });

  it("トークンが無ければ 401", async () => {
    const env = makeEnv(() => {});
    const res = await worker.fetch(
      new Request("https://worker.test/react", { method: "POST", body: JSON.stringify({ answer_id: "a", kind: "helpful", unit_ids: [] }) }),
      env,
    );
    expect(res.status).toBe(401);
  });
});

describe("POST /react value_wrong(段階 2)", () => {
  it("1 回目は unit_id/col だけの行を作る", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    const res = await worker.fetch(
      reactRequest({ answer_id: "a_5", kind: "value_wrong", unit_id: "r:x/1", col: "成功率", unit_ids: ["r:x/1"] }, token),
      env,
    );

    expect(res.status).toBe(200);
    expect(inserts).toHaveLength(1);
    // bind の並びは (day, answer_id, unit_ids, unit_id, col, claim, note, question)
    const [, answerId, unitIds, unitId, col, claim, note, question] = inserts[0]!;
    expect(answerId).toBe("a_5");
    expect(unitIds).toBe(JSON.stringify(["r:x/1"]));
    expect(unitId).toBe("r:x/1");
    expect(col).toBe("成功率");
    expect(claim).toBeNull();
    expect(note).toBeNull();
    expect(question).toBeNull();
  });

  it("同じ answer_id・同じ列の 2 回目は claim/note/question を追記(UPDATE)する", async () => {
    const inserts: unknown[][] = [];
    const updates: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v), (v) => updates.push(v));
    const token = await bearerFor(env);

    await worker.fetch(
      reactRequest({ answer_id: "a_6", kind: "value_wrong", unit_id: "r:x/1", col: "成功率", unit_ids: [] }, token),
      env,
    );
    const second = await worker.fetch(
      reactRequest(
        { answer_id: "a_6", kind: "value_wrong", unit_id: "r:x/1", col: "成功率", unit_ids: [], claim: "50%", note: "公式お知らせ" },
        token,
      ),
      env,
    );

    expect(second.status).toBe(200);
    expect(inserts).toHaveLength(1);
    expect(updates).toHaveLength(1);
    // bind の並びは (answer_id, unit_id, claim, note, question, col)
    expect(updates[0]).toEqual(["a_6", "r:x/1", "50%", "公式お知らせ", null, "成功率"]);
  });

  it("同じ answer_id でも列が違えば別に 1 回ずつ数える", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    await worker.fetch(
      reactRequest({ answer_id: "a_7", kind: "value_wrong", unit_id: "r:x/1", col: "成功率", unit_ids: [] }, token),
      env,
    );
    await worker.fetch(
      reactRequest({ answer_id: "a_7", kind: "value_wrong", unit_id: "r:x/1", col: "進化", unit_ids: [] }, token),
      env,
    );

    expect(inserts).toHaveLength(2);
  });

  it("helpful/wrong の 1 種類制限とは独立(value_wrong のあとも helpful を送れる)", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    await worker.fetch(
      reactRequest({ answer_id: "a_8", kind: "value_wrong", unit_id: "r:x/1", col: "成功率", unit_ids: [] }, token),
      env,
    );
    await worker.fetch(reactRequest({ answer_id: "a_8", kind: "helpful", unit_ids: [] }, token), env);

    expect(inserts).toHaveLength(2);
  });

  it("unit_id か col が無ければ 400", async () => {
    const env = makeEnv(() => {});
    const token = await bearerFor(env);

    const res = await worker.fetch(
      reactRequest({ answer_id: "a_9", kind: "value_wrong", unit_ids: [] }, token),
      env,
    );
    expect(res.status).toBe(400);
  });

  it("claim は 100 字・note は 300 字で切る", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);

    await worker.fetch(
      reactRequest(
        {
          answer_id: "a_10", kind: "value_wrong", unit_id: "r:x/1", col: "成功率", unit_ids: [],
          claim: "あ".repeat(150), note: "い".repeat(400),
        },
        token,
      ),
      env,
    );

    const [, , , , , claim, note] = inserts[0]!;
    expect((claim as string).length).toBe(100);
    expect((note as string).length).toBe(300);
  });

  it("value_wrong: unit_id や col に区切り文字が混ざっても別の列として数える(KV キーの衝突なし)", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);
    const base = { answer_id: "a_9", kind: "value_wrong", unit_ids: [] };
    await worker.fetch(reactRequest({ ...base, unit_id: "p:X", col: "Y:Z" }, token), env);
    await worker.fetch(reactRequest({ ...base, unit_id: "p:X:Y", col: "Z" }, token), env);
    expect(inserts).toHaveLength(2);
  });

  it("value_wrong: unit_id の形(p:/r:)と col の長さを検証する", async () => {
    const inserts: unknown[][] = [];
    const env = makeEnv((v) => inserts.push(v));
    const token = await bearerFor(env);
    const bad1 = await worker.fetch(reactRequest({ answer_id: "a_10", kind: "value_wrong", unit_ids: [], unit_id: "x:1", col: "a" }, token), env);
    const bad2 = await worker.fetch(reactRequest({ answer_id: "a_10", kind: "value_wrong", unit_ids: [], unit_id: "p:1", col: "a".repeat(81) }, token), env);
    expect(bad1.status).toBe(400);
    expect(bad2.status).toBe(400);
    expect(inserts).toHaveLength(0);
  });
});
