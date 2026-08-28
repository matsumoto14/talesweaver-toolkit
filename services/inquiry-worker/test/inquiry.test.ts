// Worker の受け付け経路のテスト。GitHub API だけ差し替えて、実際の fetch ハンドラを叩く。
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import worker, { type Env } from "../src/index";

/**
 * JWT の署名経路も実際に通したいので、本物の RSA 鍵を作って PKCS#8 PEM にする
 * (GitHub が配る PKCS#1 は openssl で変換して使う。README 参照)。
 */
let privateKeyPem = "";

beforeAll(async () => {
  // workers-types の generateKey / exportKey は共用体を返すので、鍵種を絞る。
  const pair = (await crypto.subtle.generateKey(
    { name: "RSASSA-PKCS1-v1_5", modulusLength: 2048, publicExponent: new Uint8Array([1, 0, 1]), hash: "SHA-256" },
    true,
    ["sign", "verify"],
  )) as CryptoKeyPair;
  const pkcs8 = new Uint8Array(
    (await crypto.subtle.exportKey("pkcs8", pair.privateKey)) as ArrayBuffer,
  );
  const base64 = Buffer.from(pkcs8).toString("base64").replace(/(.{64})/g, "$1\n");
  privateKeyPem = `-----BEGIN PRIVATE KEY-----\n${base64}\n-----END PRIVATE KEY-----\n`;
});

/** テスト用の KV。TTL は見ない(有効時間そのものは nonce の署名で検証している)。 */
function memoryKv() {
  const store = new Map<string, string>();
  return {
    store,
    get: async (key: string) => store.get(key) ?? null,
    put: async (key: string, value: string) => {
      store.set(key, value);
    },
    delete: async (key: string) => {
      store.delete(key);
    },
  } as unknown as KVNamespace & { store: Map<string, string> };
}

function makeEnv(kv = memoryKv()): Env & { INQUIRY: ReturnType<typeof memoryKv> } {
  return {
    INQUIRY: kv,
    GITHUB_OWNER: "matsumoto14",
    GITHUB_REPO: "talesweaver-toolkit",
    ISSUE_LABELS: "from-app,unverified",
    GITHUB_APP_ID: "1",
    GITHUB_APP_PRIVATE_KEY: privateKeyPem,
    GITHUB_INSTALLATION_ID: "1",
    NONCE_SECRET: "test-secret",
    // 本番は 20 ビット。テストで待たされないよう下げる(検証しているのは経路であって難易度ではない)。
    POW_DIFFICULTY_BITS: "8",
  } as Env & { INQUIRY: ReturnType<typeof memoryKv> };
}

/** アプリ側と同じ解き方(sha256 の先頭 bits が 0 になる counter を探す)。 */
async function solve(nonce: string, bits: number): Promise<string> {
  const encoder = new TextEncoder();
  for (let counter = 0; ; counter += 1) {
    const digest = new Uint8Array(
      await crypto.subtle.digest("SHA-256", encoder.encode(`${nonce}:${counter}`)),
    );
    let remaining = bits;
    let ok = true;
    for (const byte of digest) {
      if (remaining >= 8) {
        if (byte !== 0) {
          ok = false;
          break;
        }
        remaining -= 8;
        continue;
      }
      ok = byte >>> (8 - remaining) === 0;
      break;
    }
    if (ok) return String(counter);
  }
}

const post = (body: unknown) =>
  new Request("https://worker.test/inquiry", {
    method: "POST",
    headers: { "content-type": "application/json", "cf-connecting-ip": "203.0.113.7" },
    body: JSON.stringify(body),
  });

const challenge = () => new Request("https://worker.test/challenge");

/** 送信 1 回ぶんの nonce と解答。 */
async function ticket(env: Env) {
  const response = await worker.fetch(challenge(), env);
  const { nonce, difficultyBits } = (await response.json()) as {
    nonce: string;
    difficultyBits: number;
  };
  return { nonce, solution: await solve(nonce, difficultyBits) };
}

let created: { title: string; body: string; labels: string[] } | null = null;

beforeEach(() => {
  created = null;
  vi.stubGlobal("fetch", async (input: RequestInfo, init?: RequestInit) => {
    const url = String(input);
    if (url.includes("/access_tokens")) {
      return new Response(JSON.stringify({ token: "ghs_test" }), { status: 201 });
    }
    if (url.endsWith("/issues")) {
      created = JSON.parse(String(init?.body));
      return new Response(
        JSON.stringify({ html_url: "https://github.com/o/r/issues/12", number: 12 }),
        { status: 201 },
      );
    }
    throw new Error(`想定外の外部呼び出し: ${url}`);
  });
});

afterEach(() => vi.unstubAllGlobals());

describe("設定漏れ", () => {
  it("シークレットが欠けていたら、何が足りないかを返す", async () => {
    const env = makeEnv();
    env.NONCE_SECRET = "";

    const response = await worker.fetch(challenge(), env);

    expect(response.status).toBe(503);
    expect((await response.json()) as { error: string }).toMatchObject({
      error: expect.stringContaining("NONCE_SECRET"),
    });
  });
});

describe("challenge", () => {
  it("nonce と難易度を返す", async () => {
    const response = await worker.fetch(challenge(), makeEnv());
    const payload = (await response.json()) as { nonce: string; difficultyBits: number };

    expect(response.status).toBe(200);
    expect(payload.nonce.split(".")).toHaveLength(3);
    expect(payload.difficultyBits).toBeGreaterThan(0);
    expect(response.headers.get("access-control-allow-origin")).toBe("*");
  });
});

describe("inquiry", () => {
  it("PoW を解いていれば issue になる", async () => {
    const env = makeEnv();
    const { nonce, solution } = await ticket(env);

    const response = await worker.fetch(
      post({ nonce, solution, kind: "bug", title: "落ちる", body: "極・連撃で落ちる" }),
      env,
    );

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ url: "https://github.com/o/r/issues/12", number: 12 });
    expect(created?.title).toBe("[不具合] 落ちる");
    expect(created?.labels).toEqual(["from-app", "unverified"]);
    expect(created?.body).toContain("投稿者を確認していない");
  });

  it("PoW の解答が違えば弾く", async () => {
    const env = makeEnv();
    const { nonce } = await ticket(env);

    const response = await worker.fetch(
      post({ nonce, solution: "0", title: "t", body: "b" }),
      env,
    );

    expect(response.status).toBe(400);
    expect(created).toBeNull();
  });

  it("Worker が出していない nonce は弾く", async () => {
    const env = makeEnv();

    const response = await worker.fetch(
      post({ nonce: "1700000000.abc.deadbeef", solution: "0", title: "t", body: "b" }),
      env,
    );

    expect(response.status).toBe(400);
    expect((await response.json()) as { error: string }).toMatchObject({
      error: expect.stringContaining("署名"),
    });
  });

  it("同じ nonce は 2 回使えない", async () => {
    const env = makeEnv();
    const { nonce, solution } = await ticket(env);

    await worker.fetch(post({ nonce, solution, title: "t", body: "b" }), env);
    const second = await worker.fetch(post({ nonce, solution, title: "t", body: "b" }), env);

    expect(second.status).toBe(400);
  });

  it("件名か内容が空なら弾く", async () => {
    const env = makeEnv();
    const { nonce, solution } = await ticket(env);

    const response = await worker.fetch(post({ nonce, solution, title: "", body: "b" }), env);

    expect(response.status).toBe(400);
    expect(created).toBeNull();
  });

  it("1 日 5 件でレート制限にかかる", async () => {
    const env = makeEnv();

    for (let i = 0; i < 5; i += 1) {
      const { nonce, solution } = await ticket(env);
      const ok = await worker.fetch(post({ nonce, solution, title: `t${i}`, body: "b" }), env);
      expect(ok.status).toBe(200);
    }

    const { nonce, solution } = await ticket(env);
    const blocked = await worker.fetch(post({ nonce, solution, title: "t5", body: "b" }), env);

    expect(blocked.status).toBe(429);
    expect((await blocked.json()) as { error: string }).toMatchObject({
      error: expect.stringContaining("5 件"),
    });
  });

  it("メンション・課題リンク・コードブロック脱出を無効化する", async () => {
    const env = makeEnv();
    const { nonce, solution } = await ticket(env);

    await worker.fetch(
      post({
        nonce,
        solution,
        title: "@matsumoto14 見て",
        body: "```\n脱出\n```\n#1 と @someone を巻き込みたい",
        diagnostics: "version 0.1.0",
      }),
      env,
    );

    // `@` `#` の直後にゼロ幅スペースが入り、リンクにならない。
    expect(created?.title).toBe("[要望] @​matsumoto14 見て");
    expect(created?.body).toContain("#​1");
    expect(created?.body).toContain("@​someone");
    // 本文中の ``` は潰れているので、自動で付ける情報のブロックから抜け出せない。
    expect(created?.body).not.toMatch(/^```$/m.source ? /\n```\n脱出/ : /x/);
    expect(created?.body).toContain("'''");
    expect(created?.body).toContain("<details><summary>アプリが自動で付けた情報</summary>");
  });

  it("知らない経路は 404", async () => {
    const response = await worker.fetch(new Request("https://worker.test/nope"), makeEnv());
    expect(response.status).toBe(404);
  });
});
