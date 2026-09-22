/**
 * 認証(PoW → セッショントークン)とレート制限。services/inquiry-worker の
 * issueChallenge / verifyNonce / verifyProofOfWork / hmac / timingSafeEqual を複製し、
 * この Worker の KV(`API` binding)と `NONCE_SECRET` に向けたもの。
 *
 * セッショントークンは KV に持たない(`base64url(issuedAt).signature`、HMAC 署名、
 * 1 時間有効、失効不可を許容)。nonce の使用済み記録だけ KV に書く。
 */

export interface AuthEnv {
  API: KVNamespace;
  NONCE_SECRET: string;
  /** 難易度の上書き(テスト用)。未設定なら POW_DIFFICULTY_BITS。 */
  POW_DIFFICULTY_BITS?: string;
  RATE_LIMIT_PER_DAY?: string;
  /** 評価スクリプト用(ローカルの .dev.vars にだけ置く)。本番の vars には無い */
  RATE_LIMIT_DISABLED?: string;
  /** 連打止め(Workers Rate Limiting バインディング)。ローカル/テストでは無くてもよい。 */
  ASK_BURST?: { limit: (opts: { key: string }) => Promise<{ success: boolean }> };
}

const POW_DIFFICULTY_BITS = 20;
const NONCE_TTL_SECONDS = 600;
const SESSION_TTL_SECONDS = 60 * 60; // 1 時間

function difficultyOf(env: AuthEnv): number {
  const override = Number(env.POW_DIFFICULTY_BITS);
  return Number.isFinite(override) && override > 0 ? override : POW_DIFFICULTY_BITS;
}

// --- PoW ---------------------------------------------------------------------

/** `<発行時刻>.<乱数>.<署名>`。KV に持たずに「自分が出した nonce か」を検証できる。 */
export async function issueChallenge(env: AuthEnv): Promise<{
  nonce: string;
  difficultyBits: number;
  expiresInSeconds: number;
}> {
  const issuedAt = Math.floor(Date.now() / 1000);
  const random = crypto.randomUUID().replace(/-/g, "");
  const payload = `${issuedAt}.${random}`;
  const signature = await hmacHex(env.NONCE_SECRET, payload);
  return { nonce: `${payload}.${signature}`, difficultyBits: difficultyOf(env), expiresInSeconds: NONCE_TTL_SECONDS };
}

export async function verifyNonce(env: AuthEnv, nonce: string): Promise<string | null> {
  const parts = nonce.split(".");
  if (parts.length !== 3) return "形式が不正です";

  const [issuedAtText, random, signature] = parts as [string, string, string];
  const expected = await hmacHex(env.NONCE_SECRET, `${issuedAtText}.${random}`);
  if (!timingSafeEqual(signature, expected)) return "署名が一致しません";

  const issuedAt = Number(issuedAtText);
  if (!Number.isFinite(issuedAt)) return "形式が不正です";
  if (Math.floor(Date.now() / 1000) - issuedAt > NONCE_TTL_SECONDS) {
    return "時間切れです。もう一度送信してください";
  }

  const usedKey = `used:${random}`;
  if (await env.API.get(usedKey)) return "この送信はすでに受け付けています";
  await env.API.put(usedKey, "1", { expirationTtl: NONCE_TTL_SECONDS });
  return null;
}

/** sha256(nonce + ":" + solution) の先頭が規定ビット数だけ 0 か。 */
export async function verifyProofOfWork(nonce: string, solution: string, bits: number): Promise<boolean> {
  const digest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(`${nonce}:${solution}`)),
  );
  let remaining = bits;
  for (const byte of digest) {
    if (remaining >= 8) {
      if (byte !== 0) return false;
      remaining -= 8;
      continue;
    }
    return byte >>> (8 - remaining) === 0;
  }
  return true;
}

export { difficultyOf };

// --- セッショントークン --------------------------------------------------------

/** PoW を解いた nonce と引き換えに発行する。KV には持たない。 */
export async function issueSessionToken(env: AuthEnv): Promise<{ token: string; expiresInSeconds: number }> {
  const issuedAt = Math.floor(Date.now() / 1000);
  const payload = String(issuedAt);
  const signature = await hmacHex(env.NONCE_SECRET, payload);
  return { token: `${base64url(payload)}.${signature}`, expiresInSeconds: SESSION_TTL_SECONDS };
}

/** トークンが有効(署名一致・期限内)か。 */
export async function verifySessionToken(env: AuthEnv, token: string | null): Promise<boolean> {
  if (!token) return false;
  const parts = token.split(".");
  if (parts.length !== 2) return false;
  const [payloadB64, signature] = parts as [string, string];
  const payload = base64urlDecode(payloadB64);
  if (payload === null) return false;
  const expected = await hmacHex(env.NONCE_SECRET, payload);
  if (!timingSafeEqual(signature, expected)) return false;
  const issuedAt = Number(payload);
  if (!Number.isFinite(issuedAt)) return false;
  return Math.floor(Date.now() / 1000) - issuedAt <= SESSION_TTL_SECONDS;
}

/** `Authorization: Bearer <token>` を検証する。無効なら 401 の理由を返す。 */
export async function requireSession(request: Request, env: AuthEnv): Promise<string | null> {
  const header = request.headers.get("authorization") ?? "";
  const token = header.startsWith("Bearer ") ? header.slice("Bearer ".length) : null;
  if (!(await verifySessionToken(env, token))) return "セッションが切れました";
  return null;
}

// --- レート制限 ----------------------------------------------------------------

const CLIENT_ID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const DEFAULT_RATE_LIMIT_PER_DAY = 20;

/** IP は保存せず、ハッシュだけをキーにする。連打止め(ASK_BURST)のキー。 */
export async function ipHash(request: Request, env: AuthEnv): Promise<string> {
  const ip = request.headers.get("cf-connecting-ip") ?? "unknown";
  return sha256Hex(`${env.NONCE_SECRET}:${ip}`);
}

/**
 * 「ユーザーあたり」の単位。端末が localStorage に持つ UUID(`x-client-id`)のハッシュ。
 * 端末を跨いで同じ人かは分からないし、消せば新しい人になる(PoW を解き直す手間だけ)。
 * 費用の上限は正直な利用者向けで、突破する人の歯止めは IP の連打止めと Anthropic 側の月額上限。
 * ヘッダが無い(古い端末)なら IP のハッシュで代用する。1 日の上限と ask_log の両方がこれを使う。
 */
export async function userHash(request: Request, env: AuthEnv): Promise<string> {
  const clientId = request.headers.get("x-client-id") ?? "";
  if (CLIENT_ID_PATTERN.test(clientId)) return sha256Hex(`${env.NONCE_SECRET}:client:${clientId}`);
  return ipHash(request, env);
}

/** 連打(1 分 10 問、IP、Rate Limiting バインディング)+ 1 日の上限(ユーザー、KV カウンタ)。 */
export async function consumeRateLimit(request: Request, env: AuthEnv): Promise<string | null> {
  if (env.RATE_LIMIT_DISABLED === "1") return null; // 評価(eval/run.ts)が 90 問を続けて投げるため。ローカル専用

  if (env.ASK_BURST) {
    const burst = await env.ASK_BURST.limit({ key: await ipHash(request, env) });
    if (!burst.success) return "続けて聞きすぎです。少し待ってからもう一度どうぞ";
  }

  const perDay = Number(env.RATE_LIMIT_PER_DAY) || DEFAULT_RATE_LIMIT_PER_DAY;
  const day = new Date().toISOString().slice(0, 10);
  const key = `rate:${await userHash(request, env)}:${day}`;
  const used = Number((await env.API.get(key)) ?? "0");
  if (used >= perDay) return `1 日に聞けるのは ${perDay} 問までです。また明日どうぞ`;
  await env.API.put(key, String(used + 1), { expirationTtl: 60 * 60 * 48 });
  return null;
}

// --- 小物(inquiry-worker と同じ実装) ------------------------------------------

export async function hmacHex(secret: string, message: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"],
  );
  const signature = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(message));
  return toHex(new Uint8Array(signature));
}

export async function sha256Hex(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return toHex(new Uint8Array(digest));
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** 長さの違いで中身を推測されないよう、全文字を突き合わせてから判定する。 */
export function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

export function base64url(value: string): string {
  let binary = "";
  for (const byte of new TextEncoder().encode(value)) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64urlDecode(value: string): string | null {
  try {
    const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=");
    return atob(padded);
  } catch {
    return null;
  }
}
