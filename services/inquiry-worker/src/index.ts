// アプリからの問い合わせを GitHub Issue にする中継。
//
// 匿名で投げられる代わりに、荒らしへの備えを 3 段持つ:
//   1. proof-of-work … Worker が署名した nonce を解かないと投稿できない(外部依存ゼロ)
//   2. レート制限   … IP のハッシュ単位で 1 日 N 件
//   3. サニタイズ   … 長さ上限 + メンション/課題リンク/コードブロック脱出の無効化
//
// GitHub へは GitHub App の installation token で書く。トークンは 1 時間で失効し、
// 権限は Issues: write だけ。漏れても issue を立てる以上のことはできない。

export interface Env {
  INQUIRY: KVNamespace;
  GITHUB_OWNER: string;
  GITHUB_REPO: string;
  ISSUE_LABELS: string;
  GITHUB_APP_ID: string;
  GITHUB_APP_PRIVATE_KEY: string;
  GITHUB_INSTALLATION_ID: string;
  NONCE_SECRET: string;
  /** 難易度の上書き(テスト用。未設定なら POW_DIFFICULTY_BITS)。 */
  POW_DIFFICULTY_BITS?: string;
}

/** PoW の難易度(先頭の 0 ビット数)。20 でおよそ 100 万回、実測で 1 秒前後。 */
const POW_DIFFICULTY_BITS = 20;

/** `wrangler secret put` / ダッシュボードで入れる 4 つ。1 つでも欠けると動かない。 */
function missingSecrets(env: Env): string[] {
  return (
    ["NONCE_SECRET", "GITHUB_APP_ID", "GITHUB_APP_PRIVATE_KEY", "GITHUB_INSTALLATION_ID"] as const
  ).filter((name) => !env[name]);
}

function difficultyOf(env: Env): number {
  const override = Number(env.POW_DIFFICULTY_BITS);
  return Number.isFinite(override) && override > 0 ? override : POW_DIFFICULTY_BITS;
}
/** nonce の有効時間。解くのに要る時間 + 人が確認画面を読む時間。 */
const NONCE_TTL_SECONDS = 600;
/** 同じ IP から 1 日に受け付ける件数。 */
const RATE_LIMIT_PER_DAY = 5;

const LIMITS = {
  title: 120,
  body: 4000,
  diagnostics: 4000,
} as const;

const KINDS = ["bug", "data", "feature"] as const;
type Kind = (typeof KINDS)[number];

const KIND_LABEL: Record<Kind, string> = {
  bug: "不具合",
  data: "データの誤り",
  feature: "要望",
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") return cors(new Response(null, { status: 204 }));

    // 設定漏れは「サーバー側で問題が起きました」に化けさせない。何が足りないかを返す。
    // (未設定の NONCE_SECRET は HMAC の鍵長 0 になり、原因の分からない例外になる)
    const missing = missingSecrets(env);
    if (missing.length > 0) {
      return cors(json({ error: `中継サーバーが未設定です: ${missing.join(", ")}` }, 503));
    }

    try {
      if (url.pathname === "/challenge" && request.method === "GET") {
        return cors(await issueChallenge(env));
      }
      if (url.pathname === "/inquiry" && request.method === "POST") {
        return cors(await createInquiry(request, env));
      }
    } catch (error) {
      console.error(error);
      return cors(json({ error: "サーバー側で問題が起きました。時間をおいて試してください。" }, 500));
    }

    return cors(json({ error: "not found" }, 404));
  },
};

// --- PoW ---------------------------------------------------------------------

/** `<発行時刻>.<乱数>.<署名>`。KV に持たずに「自分が出した nonce か」を検証できる。 */
async function issueChallenge(env: Env): Promise<Response> {
  const issuedAt = Math.floor(Date.now() / 1000);
  const random = crypto.randomUUID().replace(/-/g, "");
  const payload = `${issuedAt}.${random}`;
  const signature = await hmacHex(env.NONCE_SECRET, payload);

  return json({
    nonce: `${payload}.${signature}`,
    difficultyBits: difficultyOf(env),
    expiresInSeconds: NONCE_TTL_SECONDS,
  });
}

async function verifyNonce(env: Env, nonce: string): Promise<string | null> {
  const parts = nonce.split(".");
  if (parts.length !== 3) return "形式が不正です";

  const [issuedAtText, random, signature] = parts;
  const expected = await hmacHex(env.NONCE_SECRET, `${issuedAtText}.${random}`);
  if (!timingSafeEqual(signature, expected)) return "署名が一致しません";

  const issuedAt = Number(issuedAtText);
  if (!Number.isFinite(issuedAt)) return "形式が不正です";
  if (Math.floor(Date.now() / 1000) - issuedAt > NONCE_TTL_SECONDS) {
    return "時間切れです。もう一度送信してください";
  }

  // 同じ解答の使い回しを防ぐ(有効時間だけ覚えれば足りる)。
  const usedKey = `used:${random}`;
  if (await env.INQUIRY.get(usedKey)) return "この送信はすでに受け付けています";
  await env.INQUIRY.put(usedKey, "1", { expirationTtl: NONCE_TTL_SECONDS });

  return null;
}

/** sha256(nonce + ":" + solution) の先頭が規定ビット数だけ 0 か。 */
async function verifyProofOfWork(nonce: string, solution: string, bits: number): Promise<boolean> {
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

// --- 受け付け ----------------------------------------------------------------

async function createInquiry(request: Request, env: Env): Promise<Response> {
  const payload = (await request.json().catch(() => null)) as Record<string, unknown> | null;
  if (!payload) return json({ error: "本文を読み取れません" }, 400);

  const nonce = asString(payload.nonce);
  const solution = asString(payload.solution);
  if (!nonce || !solution) return json({ error: "送信の準備ができていません" }, 400);

  const nonceError = await verifyNonce(env, nonce);
  if (nonceError) return json({ error: nonceError }, 400);

  if (!(await verifyProofOfWork(nonce, solution, difficultyOf(env)))) {
    return json({ error: "送信の検証に失敗しました" }, 400);
  }

  const rateLimited = await consumeRateLimit(request, env);
  if (rateLimited) return json({ error: rateLimited }, 429);

  const kind = KINDS.includes(payload.kind as Kind) ? (payload.kind as Kind) : "feature";
  const title = clean(asString(payload.title), LIMITS.title);
  const body = clean(asString(payload.body), LIMITS.body);
  const diagnostics = clean(asString(payload.diagnostics), LIMITS.diagnostics);

  if (!title || !body) return json({ error: "件名と内容を入力してください" }, 400);

  const issue = await createIssue(env, {
    title: `[${KIND_LABEL[kind]}] ${title}`,
    body: renderIssueBody(body, diagnostics),
    labels: env.ISSUE_LABELS.split(",").map((l) => l.trim()).filter(Boolean),
  });

  return json({ url: issue.html_url, number: issue.number });
}

/** IP は保存せず、ハッシュだけをキーにする。 */
async function consumeRateLimit(request: Request, env: Env): Promise<string | null> {
  const ip = request.headers.get("cf-connecting-ip") ?? "unknown";
  const day = new Date().toISOString().slice(0, 10);
  const key = `rate:${day}:${await sha256Hex(`${env.NONCE_SECRET}:${ip}`)}`;

  const used = Number((await env.INQUIRY.get(key)) ?? "0");
  if (used >= RATE_LIMIT_PER_DAY) {
    return `1 日に送れるのは ${RATE_LIMIT_PER_DAY} 件までです。明日また送ってください。`;
  }
  // 日付が変われば別キーなので、TTL は 1 日 + 余裕で足りる。
  await env.INQUIRY.put(key, String(used + 1), { expirationTtl: 60 * 60 * 30 });
  return null;
}

// --- サニタイズ --------------------------------------------------------------

/**
 * 投稿者が書いた文字列を、issue 本文に置いても副作用が出ない形にする。
 *
 * - 制御文字を落とす(改行とタブは残す)
 * - `@名前` / `#123` はリンクさせない(通知の巻き込み・課題の相互リンクを防ぐ)
 * - ``` を潰す(こちらが用意したコードブロックから抜け出させない)
 * - 上限で切る
 */
function clean(value: string, limit: number): string {
  return value
    .replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, "")
    .replace(/@(?=[\w-])/g, "@\u200B")
    .replace(/#(?=\d)/g, "#\u200B")
    .replace(/`{3,}/g, "'''")
    .trim()
    .slice(0, limit);
}

function renderIssueBody(body: string, diagnostics: string): string {
  const parts = [
    "> アプリの問い合わせフォームから送られた、**投稿者を確認していない**内容です。",
    "",
    body,
  ];

  if (diagnostics) {
    parts.push(
      "",
      "<details><summary>アプリが自動で付けた情報</summary>",
      "",
      "```",
      diagnostics,
      "```",
      "",
      "</details>",
    );
  }
  return parts.join("\n");
}

// --- GitHub ------------------------------------------------------------------

interface CreatedIssue {
  html_url: string;
  number: number;
}

async function createIssue(
  env: Env,
  issue: { title: string; body: string; labels: string[] },
): Promise<CreatedIssue> {
  const token = await installationToken(env);
  const response = await fetch(
    `https://api.github.com/repos/${env.GITHUB_OWNER}/${env.GITHUB_REPO}/issues`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${token}`,
        accept: "application/vnd.github+json",
        "user-agent": "tw-toolkit-inquiry",
        "content-type": "application/json",
      },
      body: JSON.stringify(issue),
    },
  );

  if (!response.ok) {
    throw new Error(`issue 作成に失敗 (${response.status}): ${await response.text()}`);
  }
  return (await response.json()) as CreatedIssue;
}

/** installation token は 1 時間有効。KV に 55 分だけ持たせて使い回す。 */
async function installationToken(env: Env): Promise<string> {
  const cached = await env.INQUIRY.get("github:token");
  if (cached) return cached;

  const jwt = await appJwt(env);
  const response = await fetch(
    `https://api.github.com/app/installations/${env.GITHUB_INSTALLATION_ID}/access_tokens`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${jwt}`,
        accept: "application/vnd.github+json",
        "user-agent": "tw-toolkit-inquiry",
      },
    },
  );

  if (!response.ok) {
    throw new Error(`installation token の取得に失敗 (${response.status})`);
  }
  const { token } = (await response.json()) as { token: string };
  await env.INQUIRY.put("github:token", token, { expirationTtl: 55 * 60 });
  return token;
}

/** GitHub App の秘密鍵で RS256 の JWT を作る(有効 9 分)。 */
async function appJwt(env: Env): Promise<string> {
  const now = Math.floor(Date.now() / 1000);
  const header = base64url(JSON.stringify({ alg: "RS256", typ: "JWT" }));
  const claims = base64url(
    // iat を 60 秒戻すのは GitHub の推奨(時計のずれ対策)。
    JSON.stringify({ iat: now - 60, exp: now + 540, iss: env.GITHUB_APP_ID }),
  );
  const signingInput = `${header}.${claims}`;

  const key = await crypto.subtle.importKey(
    "pkcs8",
    pemToBinary(env.GITHUB_APP_PRIVATE_KEY),
    { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "RSASSA-PKCS1-v1_5",
    key,
    new TextEncoder().encode(signingInput),
  );

  return `${signingInput}.${base64urlBytes(new Uint8Array(signature))}`;
}

// --- 小物 --------------------------------------------------------------------

function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json; charset=utf-8" },
  });
}

/**
 * デスクトップアプリからの呼び出しなので origin は当てにしない。
 * 認証情報を載せない API なので `*` で開き、守りは PoW とレート制限に持たせる。
 */
function cors(response: Response): Response {
  const headers = new Headers(response.headers);
  headers.set("access-control-allow-origin", "*");
  headers.set("access-control-allow-methods", "GET, POST, OPTIONS");
  headers.set("access-control-allow-headers", "content-type");
  headers.set("access-control-max-age", "86400");
  return new Response(response.body, { status: response.status, headers });
}

async function hmacHex(secret: string, message: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(message));
  return toHex(new Uint8Array(signature));
}

async function sha256Hex(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return toHex(new Uint8Array(digest));
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** 長さの違いで中身を推測されないよう、全文字を突き合わせてから判定する。 */
function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

function base64url(value: string): string {
  return base64urlBytes(new TextEncoder().encode(value));
}

function base64urlBytes(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function pemToBinary(pem: string): ArrayBuffer {
  const body = pem
    .replace(/-----BEGIN [^-]+-----/, "")
    .replace(/-----END [^-]+-----/, "")
    .replace(/\s+/g, "");
  const binary = atob(body);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}
