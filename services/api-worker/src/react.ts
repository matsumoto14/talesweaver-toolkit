/**
 * POST /react — 答えへのリアクション。
 * helpful / wrong は答えの ID につき 1 種類(KV `react:<answer_id>` に種類を持つ)。
 * 同じ種類の 2 回目は理由・同意した質問文の追記、違う種類は無視する。`question` は「質問文も一緒に送る」に同意したときだけ送られてくる。
 *
 * value_wrong は同じ answer_id でも列ごとに 1 回(KV `react:<answer_id>:<unit_id>:<col>`。段階 2)。
 * 1 回目は unit_id/col だけ、2 回目(claim/note/question が付く)は同じ行を追記(COALESCE)する。
 * helpful/wrong の 1 種類制限とは独立(重複防止の単位が違う)。
 */

const REASONS = ["off_topic", "outdated", "unclear"] as const;
const KINDS = ["helpful", "wrong", "value_wrong"] as const;
const QUESTION_LIMIT = 200;
const CLAIM_LIMIT = 100;
const NOTE_LIMIT = 300;
const MAX_UNIT_IDS = 20;
const DEDUPE_TTL_SECONDS = 60 * 60 * 24 * 30; // 30 日

export interface ReactEnv {
  WIKI: D1Database;
  API: KVNamespace;
}

interface ReactPayload {
  answer_id: string;
  kind: (typeof KINDS)[number];
  reason: (typeof REASONS)[number] | null;
  unit_ids: string[];
  question: string | null;
  /** value_wrong のみ。 */
  unit_id: string | null;
  col: string | null;
  claim: string | null;
  note: string | null;
}

/** inquiry-worker の sanitize と同じ: 制御文字を落とし、メンション・課題リンクを無効化する。 */
function sanitize(value: string): string {
  return value
    .replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, "")
    .replace(/@(?=[\w-])/g, "@​")
    .replace(/#(?=\d)/g, "#​")
    .trim();
}

function parsePayload(raw: unknown): ReactPayload | null {
  if (!raw || typeof raw !== "object") return null;
  const body = raw as Record<string, unknown>;
  const answerId = typeof body.answer_id === "string" ? body.answer_id : "";
  const kind = KINDS.includes(body.kind as (typeof KINDS)[number]) ? (body.kind as ReactPayload["kind"]) : null;
  if (!answerId || !kind) return null;

  // unit_id は取込の形(p:… / r:…)だけ、col は列名(80 字まで)。長さ無制限の文字列を表に入れない
  const unitId = typeof body.unit_id === "string" && /^[pr]:.{1,300}$/.test(body.unit_id) ? body.unit_id : null;
  const col = typeof body.col === "string" && body.col.length >= 1 && body.col.length <= 80 ? body.col : null;
  if (kind === "value_wrong" && (!unitId || !col)) return null;

  const reason = REASONS.includes(body.reason as (typeof REASONS)[number])
    ? (body.reason as ReactPayload["reason"])
    : null;
  const unitIds = Array.isArray(body.unit_ids)
    ? body.unit_ids.filter((x): x is string => typeof x === "string").slice(0, MAX_UNIT_IDS)
    : [];
  const question = typeof body.question === "string" ? sanitize(body.question).slice(0, QUESTION_LIMIT) : null;
  const claim = typeof body.claim === "string" ? sanitize(body.claim).slice(0, CLAIM_LIMIT) : null;
  const note = typeof body.note === "string" ? sanitize(body.note).slice(0, NOTE_LIMIT) : null;

  return {
    answer_id: answerId, kind, reason, unit_ids: unitIds, question: question || null,
    unit_id: unitId, col, claim: claim || null, note: note || null,
  };
}

export async function handleReact(request: Request, env: ReactEnv): Promise<Response> {
  const raw = await request.json().catch(() => null);
  const payload = parsePayload(raw);
  if (!payload) return new Response(JSON.stringify({ error: "本文を読み取れません" }), {
    status: 400,
    headers: { "content-type": "application/json; charset=utf-8" },
  });

  if (payload.kind === "value_wrong") await handleValueWrong(env, payload);
  else await handleHelpfulOrWrong(env, payload);

  // どちらも 200。
  return new Response(JSON.stringify({ ok: true }), {
    status: 200,
    headers: { "content-type": "application/json; charset=utf-8" },
  });
}

async function handleHelpfulOrWrong(env: ReactEnv, payload: ReactPayload): Promise<void> {
  const dedupeKey = `react:${payload.answer_id}`;
  const already = await env.API.get(dedupeKey);
  if (!already) {
    await env.API.put(dedupeKey, payload.kind, { expirationTtl: DEDUPE_TTL_SECONDS });
    const day = new Date().toISOString().slice(0, 10);
    await env.WIKI.prepare(
      `INSERT INTO reaction (day, answer_id, kind, reason, unit_ids, question)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)`,
    )
      .bind(day, payload.answer_id, payload.kind, payload.reason, JSON.stringify(payload.unit_ids), payload.question)
      .run();
  } else if (already === payload.kind && (payload.reason || payload.question)) {
    // 同じ答え・同じ種類の 2 回目は「理由のチップ」「同意した質問文」の追記(端末は押した瞬間に 1 回目を
    // 送り、理由と同意はあとから同じ場所で選ぶ)。種類の違う 2 回目は無視する
    await env.WIKI.prepare(
      `UPDATE reaction SET reason = COALESCE(?2, reason), question = COALESCE(?3, question)
       WHERE answer_id = ?1 AND kind = ?4`,
    )
      .bind(payload.answer_id, payload.reason, payload.question, payload.kind)
      .run();
  }
}

/** value_wrong は列ごとに 1 回。1 回目は行を作るだけ、2 回目(claim/note/question 付き)は同じ行に追記する。 */
async function handleValueWrong(env: ReactEnv, payload: ReactPayload): Promise<void> {
  // unit_id も col も `:` を含みうるので、区切りで潰れないよう JSON のタプルをキーにする
  const dedupeKey = `react:${JSON.stringify([payload.answer_id, payload.unit_id, payload.col])}`;
  const already = await env.API.get(dedupeKey);
  if (!already) {
    await env.API.put(dedupeKey, "1", { expirationTtl: DEDUPE_TTL_SECONDS });
    const day = new Date().toISOString().slice(0, 10);
    await env.WIKI.prepare(
      `INSERT INTO reaction (day, answer_id, kind, unit_ids, unit_id, col, claim, note, question)
       VALUES (?1, ?2, 'value_wrong', ?3, ?4, ?5, ?6, ?7, ?8)`,
    )
      .bind(
        day, payload.answer_id, JSON.stringify(payload.unit_ids), payload.unit_id, payload.col,
        payload.claim, payload.note, payload.question,
      )
      .run();
  } else if (payload.claim || payload.note || payload.question) {
    await env.WIKI.prepare(
      `UPDATE reaction SET claim = COALESCE(?3, claim), note = COALESCE(?4, note), question = COALESCE(?5, question)
       WHERE answer_id = ?1 AND kind = 'value_wrong' AND unit_id = ?2 AND col = ?6`,
    )
      .bind(payload.answer_id, payload.unit_id, payload.claim, payload.note, payload.question, payload.col)
      .run();
  }
}
