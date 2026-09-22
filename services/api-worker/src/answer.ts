/**
 * 検証済み選択(verify() の結果)→ 応答 JSON(§実装 s12「/ask の要求と応答」)。
 * cells は候補の描画時点ではなく、ここでもう一度訂正を重ねて出す(LLM は値を書かないので、
 * 訂正は自動で結論文にも steps にも届く)。
 */
import { getPageUrls, getTableInfo, getUnitLinks } from "./retrieve";
import type { Candidate, CandidateCorrection } from "./retrieve";
import type { Dropped, LeadSeg, Step } from "./verify";
import type { Aspect } from "./schema";
import { wikiUrl } from "./wiki-url";

export interface AnswerUnit {
  id: string;
  kind: "paragraph" | "row";
  text?: string;
  truncated?: boolean;
  key?: string;
  cells?: Record<string, string>;
}

export interface AnswerStep {
  title: string;
  /** 静的データだけの項目(段階 3 spec B 7)は page="アプリのデータ"・url=null になる。 */
  source: { page: string; section: string; url: string | null };
  columns?: string[];
  filtered_by?: Record<string, string>;
  table?: { rows: number; url: string };
  units: AnswerUnit[];
}

export interface AnswerCorrection {
  unit: string;
  col: string;
  wiki: string;
  value: string;
  grade: string;
  source: { kind: string; title: string; url: string | null };
}

/** 次の一手。選ばれたユニットの [[リンク]] 先から、実在するページを最大 3 件(§Worker 3)。 */
export interface NextItem {
  question: string;
  page: string;
}

export interface AnswerResponse {
  kind: "answer";
  lead: LeadSeg[] | null;
  steps: AnswerStep[];
  next: NextItem[];
  synced_at: string | null;
  model: string;
  route: "cheap" | "loop" | "cheap_then_loop";
  playbook: "cant_win" | null;
  answer_id: string;
  corrections: AnswerCorrection[];
  /** 候補に答えが無かった観点。端末が「〜は wiki に見当たらなかったッピ」を出す */
  missing: Aspect[];
  /** はい/いいえの質問への答え(検算用)。none 以外なら basis に根拠の unit id が入る */
  verdict: "yes" | "no" | "depends" | "none";
  basis: string[];
  dropped: Dropped[];
  /** 直前の質問の続きとして答えたなら、そのページ。そうでなければ null(§Worker 1)。 */
  followup: { page: string } | null;
}

/** 乱数 16 桁 hex。質問文とは結び付けて保存しない。 */
export function newAnswerId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(8));
  return `a_${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

/** 節名(タイトル)。見出しの階層 " › " のうち最後の節を使う。 */
function titleOf(section: string): string {
  const parts = section.split(" › ").filter(Boolean);
  return parts.at(-1) ?? section;
}

function toAnswerUnit(c: Candidate): AnswerUnit {
  if (c.kind === "paragraph") {
    return { id: c.id, kind: "paragraph", text: c.text, truncated: c.truncated };
  }
  // cells は wiki の値のまま返す。訂正は corrections[] に両方(wiki / 訂正後)を載せ、画面が重ねて描く(ADR-021)
  return { id: c.id, kind: "row", key: c.row_key ?? undefined, cells: { ...(c.cells ?? {}) } };
}

/** state のうち、この手順の行に列として現れているものだけを残す(絞り込んだ条件の表示用)。 */
function filteredByOf(units: Candidate[], columnDict: Record<string, string>, state: Record<string, number>): Record<string, string> | undefined {
  const row = units.find((u) => u.kind === "row" && u.nums);
  if (!row?.nums) return undefined;
  const out: Record<string, string> = {};
  for (const [col, stateKey] of Object.entries(columnDict)) {
    const want = state[stateKey];
    const have = row.nums[col];
    if (want === undefined || have === undefined) continue;
    out[col] = String(want);
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

async function buildStep(
  db: D1Database,
  step: Step,
  columnDict: Record<string, string>,
  state: Record<string, number>,
  pageUrls: Map<string, string>,
): Promise<AnswerStep> {
  const head = step.units[0]!;
  const pageUrl = pageUrls.get(head.page);
  // 静的データだけの項目(page="アプリのデータ")は page 表に無いので url は null(段階 3 spec B 7)
  const url = pageUrl !== undefined ? wikiUrl(pageUrl, head.anchor) : null;
  const out: AnswerStep = {
    title: titleOf(head.section),
    source: { page: head.page, section: head.section, url },
    units: step.units.map(toAnswerUnit),
  };
  const rows = step.units.filter((u) => u.kind === "row");
  if (rows.length > 0) {
    out.columns = step.columns;
    const filteredBy = filteredByOf(rows, columnDict, state);
    if (filteredBy) out.filtered_by = filteredBy;
    const first = rows[0]!;
    if (first.table_idx !== null) {
      const info = await getTableInfo(db, first.page, first.anchor, first.table_idx);
      if (info) out.table = { rows: info.row_count, url: url ?? "" };
    }
  }
  return out;
}

function toAnswerCorrections(units: Candidate[]): AnswerCorrection[] {
  const out: AnswerCorrection[] = [];
  for (const u of units) {
    if (!u.corrections || !u.cells) continue;
    for (const cc of u.corrections as CandidateCorrection[]) {
      out.push({
        unit: u.id, col: cc.col, wiki: u.cells[cc.col] ?? "", value: cc.value,
        grade: cc.grade, source: cc.source,
      });
    }
  }
  return out;
}

/**
 * 次の一手(§Worker 3)。選ばれたユニットが張る [[リンク]] 先のうち、page 表に実在し、
 * 選ばれたユニット自身のページでないものを出現順に重複なく最大 3 件。unit_link が無ければ空。
 */
async function computeNext(db: D1Database, allUnits: Candidate[]): Promise<NextItem[]> {
  const unitIds = allUnits.map((u) => u.id);
  const links = await getUnitLinks(db, unitIds);
  if (links.length === 0) return [];

  const selfPages = new Set(allUnits.map((u) => u.page));
  const unitOrder = new Map(unitIds.map((id, i) => [id, i] as [string, number]));
  const sorted = [...links].sort((a, b) => {
    const ao = unitOrder.get(a.unit_id) ?? 0;
    const bo = unitOrder.get(b.unit_id) ?? 0;
    return ao !== bo ? ao - bo : a.ord - b.ord;
  });

  const candidatePages = [...new Set(sorted.map((l) => l.page).filter((p) => !selfPages.has(p)))];
  if (candidatePages.length === 0) return [];
  const existing = await getPageUrls(db, candidatePages); // 存在確認だけに使う(URL は使わない)

  const seen = new Set<string>();
  const out: NextItem[] = [];
  for (const link of sorted) {
    if (out.length >= 3) break;
    if (selfPages.has(link.page)) continue;
    if (!existing.has(link.page)) continue;
    if (seen.has(link.page)) continue;
    seen.add(link.page);
    const name = link.page.split("/").pop() || link.page;
    // page はフルパス(続きの加点で page 表と突き合わせる)。質問文に出すのは末尾の名前
    out.push({ question: `「${name}」はどこで手に入る?`, page: link.page });
  }
  return out;
}

export interface BuildAnswerInput {
  steps: Step[];
  lead: LeadSeg[] | null;
  dropped: Dropped[];
  columnDict: Record<string, string>;
  state: Record<string, number>;
  syncedAt: string | null;
  model: string;
  route: AnswerResponse["route"];
  playbook: "cant_win" | null;
  missing: Aspect[];
  verdict: AnswerResponse["verdict"];
  basis: string[];
  /** 直前の質問の続きとして答えたなら、そのページ(§Worker 1)。 */
  followup: { page: string } | null;
}

export async function buildAnswer(db: D1Database, input: BuildAnswerInput): Promise<AnswerResponse> {
  const allUnits = input.steps.flatMap((s) => s.units);
  const pageUrls = await getPageUrls(db, allUnits.map((u) => u.page));
  const steps = await Promise.all(
    input.steps.map((s) => buildStep(db, s, input.columnDict, input.state, pageUrls)),
  );
  const next = await computeNext(db, allUnits);
  return {
    kind: "answer",
    lead: input.lead,
    steps,
    next,
    synced_at: input.syncedAt,
    model: input.model,
    route: input.route,
    playbook: input.playbook,
    answer_id: newAnswerId(),
    corrections: toAnswerCorrections(allUnits),
    missing: input.missing,
    verdict: input.verdict,
    basis: input.basis,
    dropped: input.dropped,
    followup: input.followup,
  };
}
