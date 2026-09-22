// PoW・セッショントークンの検証(§実装 s12「認証」)。
import { describe, expect, it, vi } from "vitest";

import {
  consumeRateLimit, issueChallenge, issueSessionToken, userHash, verifyNonce, verifyProofOfWork, verifySessionToken,
} from "../src/auth";
import type { AuthEnv } from "../src/auth";

function memoryKv() {
  const store = new Map<string, string>();
  return {
    get: async (key: string) => store.get(key) ?? null,
    put: async (key: string, value: string) => { store.set(key, value); },
  } as unknown as KVNamespace;
}

function makeEnv(overrides: Partial<AuthEnv> = {}): AuthEnv {
  return { API: memoryKv(), NONCE_SECRET: "test-secret", POW_DIFFICULTY_BITS: "8", ...overrides };
}

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
        if (byte !== 0) { ok = false; break; }
        remaining -= 8;
        continue;
      }
      ok = byte >>> (8 - remaining) === 0;
      break;
    }
    if (ok) return String(counter);
  }
}

describe("PoW", () => {
  it("解いた nonce は使い回せない", async () => {
    const env = makeEnv();
    const { nonce, difficultyBits } = await issueChallenge(env);
    const solution = await solve(nonce, difficultyBits);

    expect(await verifyNonce(env, nonce)).toBeNull();
    expect(await verifyProofOfWork(nonce, solution, difficultyBits)).toBe(true);

    const second = await verifyNonce(env, nonce);
    expect(second).toContain("すでに受け付けています");
  });

  it("署名が違う nonce は弾く", async () => {
    const env = makeEnv();
    const error = await verifyNonce(env, "1700000000.abc.deadbeef");
    expect(error).toContain("署名");
  });

  it("時間切れの nonce は弾く", async () => {
    const env = makeEnv();
    vi.useFakeTimers();
    const { nonce } = await issueChallenge(env);
    vi.advanceTimersByTime(700_000); // NONCE_TTL_SECONDS(600)を超える
    const error = await verifyNonce(env, nonce);
    vi.useRealTimers();
    expect(error).toContain("時間切れ");
  });

  it("解答が違えば PoW を通さない", async () => {
    const env = makeEnv();
    const { nonce } = await issueChallenge(env);
    expect(await verifyProofOfWork(nonce, "0", 32)).toBe(false);
  });
});

describe("セッショントークン", () => {
  it("発行直後は有効", async () => {
    const env = makeEnv();
    const { token } = await issueSessionToken(env);
    expect(await verifySessionToken(env, token)).toBe(true);
  });

  it("署名が一致しないトークンは無効", async () => {
    const env = makeEnv();
    const { token } = await issueSessionToken(env);
    const tampered = `${token.split(".")[0]}.deadbeef`;
    expect(await verifySessionToken(env, tampered)).toBe(false);
  });

  it("1 時間を超えたトークンは無効", async () => {
    const env = makeEnv();
    vi.useFakeTimers();
    const { token } = await issueSessionToken(env);
    vi.advanceTimersByTime(60 * 60 * 1000 + 1000);
    const valid = await verifySessionToken(env, token);
    vi.useRealTimers();
    expect(valid).toBe(false);
  });

  it("null・形式違反は無効", async () => {
    const env = makeEnv();
    expect(await verifySessionToken(env, null)).toBe(false);
    expect(await verifySessionToken(env, "not-a-token")).toBe(false);
  });
});

describe("1 日の上限(ユーザーあたり)", () => {
  const CLIENT_A = "11111111-2222-4333-8444-555555555555";
  const CLIENT_B = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
  function req(clientId: string | null, ip = "203.0.113.1"): Request {
    const headers: Record<string, string> = { "cf-connecting-ip": ip };
    if (clientId) headers["x-client-id"] = clientId;
    return new Request("https://x/ask", { method: "POST", headers });
  }

  it("端末 ID ごとに数え、上限を超えると回数入りの文で断る", async () => {
    const env = makeEnv({ RATE_LIMIT_PER_DAY: "2" });
    expect(await consumeRateLimit(req(CLIENT_A), env)).toBeNull();
    expect(await consumeRateLimit(req(CLIENT_A), env)).toBeNull();
    expect(await consumeRateLimit(req(CLIENT_A), env)).toBe("1 日に聞けるのは 2 問までです。また明日どうぞ");
    // 同じ IP でも別の端末 ID なら別枠
    expect(await consumeRateLimit(req(CLIENT_B), env)).toBeNull();
  });

  it("端末 ID が無い・形式違反なら IP で数える", async () => {
    const env = makeEnv({ RATE_LIMIT_PER_DAY: "1" });
    expect(await consumeRateLimit(req(null), env)).toBeNull();
    expect(await consumeRateLimit(req("not-a-uuid"), env)).not.toBeNull();
    expect(await consumeRateLimit(req(null, "198.51.100.9"), env)).toBeNull();
  });

  it("userHash は端末 ID の生の値を含まず、同じ ID なら同じ値", async () => {
    const env = makeEnv();
    const a1 = await userHash(req(CLIENT_A), env);
    const a2 = await userHash(req(CLIENT_A, "198.51.100.9"), env);
    expect(a1).toBe(a2);
    expect(a1).not.toContain(CLIENT_A);
    expect(await userHash(req(CLIENT_B), env)).not.toBe(a1);
  });
});
