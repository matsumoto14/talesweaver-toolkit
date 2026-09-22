// 「wiki に聞く」(段階 1)。回答サーバー(services/api-worker)への問い合わせ。
//
// 段階 1 で LLM の結論文(lead)・手順(steps)・訂正(corrections)が加わる。応答は素の JSON
// (SSE は段階 2 で足す。stage1-spec.md 決定 1)。認証は PoW → セッショントークン(1 時間)で、
// PoW を解く部分は `pow.ts`(inquiry.ts と共通)。
//
// `request` の作りは inquiry.ts に倣う(ネットワーク失敗を日本語の Error にする)。

import { solveChallenge } from "./pow";

/**
 * 回答サーバー。
 *
 * **この URL は配布したアプリの CSP に焼き込まれる。** 変えると、既に入っている版からは
 * 二度と問い合わせできなくなる(その版の `connect-src` が古い URL のままのため)。
 * 変えるときは `src-tauri/tauri.conf.json` の `connect-src` も必ず同じ URL にする。
 */
export const API_ENDPOINT = "https://api.tw-context.dev";

export interface HealthResponse {
  ok: true;
  synced_at: string;
  unit_count: number;
  schema_version: string;
}

export interface HealthError {
  ok: false;
  error: string;
}

/** ユニットの種類。paragraph = 段落・箇条書き 1 項目、row = 表の 1 行 */
export type UnitKind = "paragraph" | "row";

export interface SearchHit {
  id: string;
  kind: UnitKind;
  page: string;
  section: string;
  anchor: string;
  /** unit.text(段落は冒頭の断片、行は「列名: 値 | …」) */
  snippet: string;
  /** true なら段落の続きがある(表の全行は元々出さない) */
  truncated: boolean;
  url: string;
}

export type SearchNoneReason = "no_llm" | "no_terms";

export interface SearchResponse {
  kind: "none";
  reason: SearchNoneReason;
  query: string | null;
  search: SearchHit[];
  synced_at: string;
  dropped: unknown[];
}

/** GET /health。索引が未投入なら 503 とともに `ok:false` が返る(HTTP エラーとしては扱わない) */
export async function health(): Promise<HealthResponse | HealthError> {
  return requestPlain<HealthResponse | HealthError>("/health", undefined, true);
}

/** GET /search?q=…。段階 0 の検索結果一覧(kind:"none" の該当なし経路と同じ形) */
export async function search(q: string, limit = 10): Promise<SearchResponse> {
  const params = new URLSearchParams({ q, limit: String(limit) });
  return requestPlain<SearchResponse>(`/search?${params.toString()}`);
}

// ---- /ask の型(応答 JSON。s12「エンドポイント」節のとおり。段階 3 まで変えない) ----

/** 結論文(lead)のセグメント。`ref` はユニット ID、`col` はそのユニットの列名 */
export type LeadSeg = { t: string } | { ref: string; col: string };

export interface ParagraphUnit {
  id: string;
  kind: "paragraph";
  /** 段落の冒頭の断片(最大 3 文・200 字) */
  text: string;
  /** true なら続きがある(「続きを wiki で読む」) */
  truncated: boolean;
}

export interface RowUnit {
  id: string;
  kind: "row";
  /** 行のキー列の値の組(例 "進0-強0") */
  key: string;
  /** セル文字列そのまま(wiki の値。訂正がある列は `corrections` を引いて画面側が重ねる) */
  cells: Record<string, string>;
}

export type Unit = ParagraphUnit | RowUnit;

export interface StepSource {
  page: string;
  section: string;
  /** null = wiki に無い項目(アプリのデータ)。出典はリンクにしない */
  url: string | null;
}

export interface Step {
  title: string;
  source: StepSource;
  units: Unit[];
  /** row のユニットで選ばれた列(キー列を含む)。行が無い手順(段落だけ)では省略される */
  columns?: string[];
  /** 状態で絞り込んだ条件(例 { "進化": "0" }) */
  filtered_by?: Record<string, string>;
  /** 元の表の全行数と URL(全行はここに出さず wiki で見る) */
  table?: { rows: number; url: string };
}

export interface NextQuestion {
  question: string;
  page: string;
}

/** 続きの判定(段階 2)。直前のページの続きとして答えたときだけ非 null */
export interface Followup {
  page: string;
}

export type CorrectionGrade = "confirmed" | "apparent";

export interface Correction {
  /** 対象ユニットの ID */
  unit: string;
  col: string;
  /** wiki のセル文字列(取り消し線で残す) */
  wiki: string;
  /** 訂正後の値 */
  value: string;
  grade: CorrectionGrade;
  source: { kind: string; title: string; url: string | null };
}

export interface Dropped {
  what: string;
  id?: string;
  why: string;
}

export type AskPlaybook = "cant_win";

/** 候補に答えが無かった観点(SELECTION_SCHEMA の `missing`)。s12 の応答例には出ないが、
 * stage1-spec.md 決定 11・16 が「missing の定型文を出す」と定めているので受け取れる形にしておく。
 * Worker がこのキーで返さない場合は常に空扱いになる(実機確認で要突き合わせ。報告に記載)。 */
export type Aspect = "what" | "how" | "materials" | "where" | "condition" | "numbers";

export interface AskAnswer {
  kind: "answer";
  /** 結論文セグメント。検証で落ちていれば空配列(手順だけを出す) */
  lead: LeadSeg[];
  steps: Step[];
  /** 次の一手(段階 3 まで常に空) */
  next: NextQuestion[];
  synced_at: string;
  model: string;
  /** cheap / loop / cheap_then_loop。評価・費用の見張り用(画面には出さない) */
  route: string;
  /** "cant_win" なら端末の domain が打ち手(Playbook)を描く。困りごとが無ければ null */
  playbook: AskPlaybook | null;
  /** リアクション用。乱数。質問文とは結び付けて保存されない */
  answer_id: string;
  corrections: Correction[];
  /** 検証で落としたものの記録。画面には出さない(評価・不具合調査用) */
  dropped: Dropped[];
  missing?: Aspect[];
  /** はい/いいえの質問への答え(検算用)。画面には出さない */
  verdict?: "yes" | "no" | "depends" | "none";
  /** 根拠になったユニット ID(none 以外なら非空)。行/段落に「根拠」バッジを付ける */
  basis?: string[];
  /** 直前のページの続きとして答えたときだけ非 null(段階 2)。旧い応答・未対応の Worker では省略される */
  followup?: Followup | null;
}

export type AskNoneReason = "llm_none" | "verification_failed" | "smalltalk" | "other" | "no_terms";

export interface AskNone {
  kind: "none";
  reason: AskNoneReason;
  search: SearchHit[];
  synced_at: string;
  dropped: Dropped[];
  /** "cant_win" なら wiki に答えが無くても端末の domain が打ち手(Playbook)を描く */
  playbook: AskPlaybook | null;
}

export type AskResponse = AskAnswer | AskNone;

/** 選択中キャラの状態。段階 1 は level だけを送る(stage1-spec.md 決定 2)。未選択なら `{}` */
export interface AskState {
  level?: number;
}

/** 対話の続き用。直前の質問文とページ名だけを渡す(答えの本文・会話全体は渡さない) */
export interface PrevTurn {
  question: string;
  page: string;
}

export type ReactKind = "helpful" | "wrong";
export type WrongReason = "off_topic" | "outdated" | "unclear";

export interface ReactPayload {
  answer_id: string;
  kind: ReactKind;
  reason?: WrongReason;
  /** 答えに出ていたユニットの ID */
  unit_ids: string[];
  /** 「質問文も一緒に送る」に同意したときだけ */
  question?: string;
}

/** 「この値は違う」(段階 2〜3)。列ごとに 1 回、Worker が KV `react:<answer_id>:<unit_id>:<col>` で抑える */
export interface ValueWrongPayload {
  answer_id: string;
  kind: "value_wrong";
  /** 値が違うと指摘された行のユニット ID */
  unit_id: string;
  col: string;
  /** 正しいと思う値(任意) */
  claim?: string;
  /** 根拠(任意) */
  note?: string;
  /** 答えに出ていたユニットの ID(helpful/wrong と同じ欄) */
  unit_ids: string[];
  /** 「質問文も一緒に送る」に同意したときだけ */
  question?: string;
}

/** SSE の `progress` イベントの `step`(stage2-spec.md desktop 8・9)。"outline"・"rows" は
 * 段階 2(回す道、agent.ts)の get_outline・get_rows ツール呼び出し中に流れる。 */
export type ProgressStep = "understand" | "search" | "select" | "app_data" | "outline" | "rows";

/** HTTP エラー(401 以外)。画面が状態(429 の上限・502/503 のつながらない)で表情を選べるよう status を持つ */
export class AskHttpError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = "AskHttpError";
    this.status = status;
  }
}

/** 401(セッション切れ)を、通常のエラーと区別するための型 */
export class SessionExpiredError extends Error {
  constructor() {
    super("セッションが切れました");
    this.name = "SessionExpiredError";
  }
}

/**
 * POST /ask(SSE)。401 なら呼び元が 1 回だけ再送する(request 内ではリトライしない)。
 * `onProgress` は `event: progress` のたびに呼ばれる(stage2-spec.md desktop 8・9)。
 * デスクトップ(WebView2)・ブラウザ版のどちらも `fetch` + `ReadableStream` の同じ経路を使う
 * (分岐は作らない。使えない環境があれば実機で分かる)。
 */
export async function ask(
  question: string,
  state: AskState,
  prev: PrevTurn | null = null,
  onProgress?: (step: ProgressStep) => void,
): Promise<AskResponse> {
  return withSession((token) => requestAskStream(question, state, prev, token, onProgress));
}

export async function react(payload: ReactPayload | ValueWrongPayload): Promise<void> {
  await withSession((token) => requestAuthed<unknown>("/react", payload, token));
}

/** トークンを使って呼び、401 なら 1 回だけトークンを取り直して再送する */
async function withSession<T>(call: (token: string) => Promise<T>): Promise<T> {
  const token = await sessionToken();
  try {
    return await call(token);
  } catch (e) {
    if (!(e instanceof SessionExpiredError)) throw e;
    clearSession();
    const retryToken = await sessionToken();
    return await call(retryToken);
  }
}

// ---- セッショントークン(PoW → 1 時間有効) ----

const SESSION_KEY = "ask.session";
const SESSION_TTL_MS = 60 * 60 * 1000;

interface StoredSession {
  token: string;
  issuedAt: number;
}

interface Challenge {
  nonce: string;
  difficultyBits: number;
}

function loadSession(): StoredSession | null {
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<StoredSession>;
    if (typeof parsed.token !== "string" || typeof parsed.issuedAt !== "number") return null;
    return { token: parsed.token, issuedAt: parsed.issuedAt };
  } catch {
    return null;
  }
}

function saveSession(session: StoredSession): void {
  try {
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } catch {
    // 保存できなくても致命的ではない(毎回 PoW を解き直すだけ)
  }
}

function clearSession(): void {
  try {
    localStorage.removeItem(SESSION_KEY);
  } catch {
    // 無視
  }
}

/** 期限内のトークンを再利用するか、PoW を解いて発行し直す */
export async function sessionToken(): Promise<string> {
  const existing = loadSession();
  if (existing && Date.now() - existing.issuedAt < SESSION_TTL_MS) return existing.token;

  const challenge = await requestPlain<Challenge>("/challenge");
  const solution = await solveChallenge(challenge.nonce, challenge.difficultyBits);
  const { token } = await requestPlain<{ token: string }>("/session", {
    nonce: challenge.nonce,
    solution,
  });
  saveSession({ token, issuedAt: Date.now() });
  return token;
}

// ---- 通信 ----

async function requestPlain<T>(path: string, body?: unknown, acceptError = false): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_ENDPOINT}${path}`, body === undefined ? undefined : {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
  } catch {
    throw new Error("回答サーバーに接続できませんでした。ネットワークを確認してください。");
  }

  const payload = (await response.json().catch(() => null)) as (T & { error?: string }) | null;
  if (!response.ok && !acceptError) {
    throw new Error(payload?.error ?? `問い合わせに失敗しました(${response.status})`);
  }
  if (payload === null) {
    throw new Error("回答サーバーの応答を読み取れませんでした。");
  }
  return payload;
}

/**
 * POST /ask を SSE で読む。`Accept: text/event-stream` を送り、`event: progress` のたびに
 * `onProgress` を呼び、`event: result` の中身を返す。理解より前に判定できるエラー(401/429/502/503)
 * は SSE を始める前に通常の HTTP ステータスで返る(Worker 側の作り。stage2-spec.md Worker 2)。
 * SSE が始まったあとの失敗は `event: error` で `AskHttpError` にする。
 */
async function requestAskStream(
  question: string,
  state: AskState,
  prev: PrevTurn | null,
  token: string,
  onProgress?: (step: ProgressStep) => void,
): Promise<AskResponse> {
  let response: Response;
  try {
    response = await fetch(`${API_ENDPOINT}/ask`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${token}`,
        accept: "text/event-stream",
      },
      body: JSON.stringify({ question, state, prev }),
    });
  } catch {
    throw new Error("回答サーバーに接続できませんでした。ネットワークを確認してください。");
  }

  if (response.status === 401) throw new SessionExpiredError();

  if (!response.ok) {
    const payload = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new AskHttpError(response.status, payload?.error ?? `問い合わせに失敗しました(${response.status})`);
  }

  const contentType = response.headers.get("content-type") ?? "";
  if (!response.body || !contentType.includes("text/event-stream")) {
    // 保険: SSE でなければ素の JSON として読む(評価スクリプト・curl と同じ経路。実測で必要なら報告)
    const payload = (await response.json().catch(() => null)) as AskResponse | null;
    if (payload === null) throw new Error("回答サーバーの応答を読み取れませんでした。");
    return payload;
  }

  return readAskStream(response.body, onProgress);
}

/** SSE フレーム(`event:` / `data:` 行、空行区切り)を 1 つずつ読む */
async function readAskStream(
  body: ReadableStream<Uint8Array>,
  onProgress?: (step: ProgressStep) => void,
): Promise<AskResponse> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let result: AskResponse | null = null;
  let sseError: { status: number; error: string } | null = null;

  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let sep: number;
    while ((sep = buffer.indexOf("\n\n")) !== -1) {
      const raw = buffer.slice(0, sep);
      buffer = buffer.slice(sep + 2);
      const frame = parseSseFrame(raw);
      if (!frame) continue;
      if (frame.event === "progress") {
        const step = (frame.data as { step?: ProgressStep } | null)?.step;
        if (step) onProgress?.(step);
      } else if (frame.event === "result") {
        result = frame.data as AskResponse;
      } else if (frame.event === "error") {
        const data = frame.data as { status?: number; error?: string } | null;
        sseError = { status: data?.status ?? 502, error: data?.error ?? "回答サーバーで問題が起きました" };
      }
    }
  }

  if (sseError) throw new AskHttpError(sseError.status, sseError.error);
  if (result === null) throw new Error("回答サーバーの応答を読み取れませんでした。");
  return result;
}

function parseSseFrame(raw: string): { event: string; data: unknown } | null {
  let event = "message";
  const dataLines: string[] = [];
  for (const line of raw.split("\n")) {
    if (line.startsWith("event:")) event = line.slice(6).trim();
    else if (line.startsWith("data:")) dataLines.push(line.slice(5).trim());
  }
  if (dataLines.length === 0) return null;
  try {
    return { event, data: JSON.parse(dataLines.join("\n")) };
  } catch {
    return null;
  }
}

/** Authorization: Bearer 付きの POST。401 は `SessionExpiredError` にする(トークンを使い回す `withSession` が拾う) */
async function requestAuthed<T>(path: string, body: unknown, token: string): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_ENDPOINT}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
      body: JSON.stringify(body),
    });
  } catch {
    throw new Error("回答サーバーに接続できませんでした。ネットワークを確認してください。");
  }

  if (response.status === 401) throw new SessionExpiredError();

  const payload = (await response.json().catch(() => null)) as (T & { error?: string }) | null;
  if (!response.ok) {
    // 429/502/503 は本文の error を日本語のまま伝える(stage1-spec.md 決定 5〜7)
    throw new AskHttpError(response.status, payload?.error ?? `問い合わせに失敗しました(${response.status})`);
  }
  if (payload === null) {
    throw new Error("回答サーバーの応答を読み取れませんでした。");
  }
  return payload;
}
