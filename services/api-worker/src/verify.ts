/**
 * 検証 — JSON が正しいことと、内容が正しいことは別。スキーマで捕まらない失敗
 * (妥当な enum だが文脈上は誤り、空を返さない、常に同じ既定値)をコードが見る。
 * 何が落ちても例外にせず、残ったもので答えを組む。手順が 0 になったときだけ該当なし経路。
 */
import type { Aspect } from "./schema";
import type { Selection } from "./schema";
import type { Candidate } from "./retrieve";

export type Dropped = { what: "unit" | "step" | "lead" | "all" | "route"; id?: string; why: string };
export type LeadSeg = { t: string } | { ref: string; col: string };

export interface Step {
  units: Candidate[];
  columns: string[];
}

const MAX_STEPS = 4;
const MAX_UNITS_PER_STEP = 4;
const MAX_LEAD_LENGTH = 80;

/** 半角・全角の数字と、単位が続く漢数字。「一番」「一度」「十分」のような語は通す。{{…}} は除外して見る。 */
const DIGITS = /[0-9０-９]|[〇一二三四五六七八九十百千万億]+(?=[%％個枚段倍割点]|パーセント|レベル)/;
const PLACEHOLDER = /\{\{([^.}]+)\.([^}]+)\}\}/g;

export interface Ctx {
  /** 渡した候補(ID → ユニット)。 */
  candidates: Map<string, Candidate>;
  state: Record<string, number>;
  /** 列名 → state のキー("進化" → "evolution")。 */
  columnDict: Record<string, string>;
  /** 全ページ名(固有名詞の照合用)。 */
  pageNames: string[];
  /** 候補を描いた全文(候補内に出る名前は許す)。 */
  candidateText: string;
}

export interface VerifyResult {
  steps: Step[];
  lead: LeadSeg[] | null;
  /** 候補に答えが無かった観点(LLM の申告。重複を落とす)。端末が定型文で「見当たらなかった」と言う */
  missing: Aspect[];
  dropped: Dropped[];
}

export function verify(raw: Selection, ctx: Ctx): VerifyResult {
  const dropped: Dropped[] = [];
  if (raw.none) return { steps: [], lead: null, missing: [], dropped: [{ what: "all", why: "llm_none" }] };

  const seen = new Set<string>();
  const steps: Step[] = [];

  for (const s of raw.steps.slice(0, MAX_STEPS)) {
    const units: Candidate[] = [];
    s.units.forEach((id, i) => {
      const u = ctx.candidates.get(id);
      if (!u) { dropped.push({ what: "unit", id, why: "unknown_id" }); return; }
      if (seen.has(id)) { dropped.push({ what: "unit", id, why: "duplicate" }); return; }
      if (groupCount(units, u) > MAX_UNITS_PER_STEP) {
        dropped.push({ what: "unit", id, why: "over_limit" });
        return;
      }
      if (u.kind === "row") {
        const col = conflictsWithState(u, ctx);
        if (col) { dropped.push({ what: "unit", id, why: `state:${col}` }); return; }
        const key = s.key_check[i];
        if (key === undefined || key !== u.row_key) {
          dropped.push({ what: "unit", id, why: "key_check" });
          return;
        }
      }
      seen.add(id);
      units.push(u);
    });
    if (units.length === 0) { dropped.push({ what: "step", why: "empty" }); continue; }
    steps.push({ units, columns: pickColumns(s.columns, units, dropped) });
  }

  const missing = [...new Set(raw.missing)];
  if (steps.length === 0) return { steps, lead: null, missing, dropped: [...dropped, { what: "all", why: "no_steps" }] };

  const lead = renderLead(raw, seen, ctx, dropped);
  return { steps, lead, missing, dropped };
}

/** u を足したときの塊の数。同じ表・同じ箇条書きに属するユニットは 1 塊。画面は 1 塊を最大 8 行まで描く。 */
function groupCount(units: Candidate[], u: Candidate): number {
  const groups = new Set([...units, u].map((x) => x.group_key ?? x.id));
  return groups.size;
}

/** 行の列にキャラ状態と同じ意味の列があり、数値化できて、値が違う(範囲の列なら範囲外)ならその列名。 */
function conflictsWithState(u: Candidate, ctx: Ctx): string | null {
  if (!u.nums) return null;
  for (const [col, stateKey] of Object.entries(ctx.columnDict)) {
    const want = ctx.state[stateKey];
    const have = u.nums[col];
    if (want === undefined || have === undefined) continue;
    const [lo, hi] = Array.isArray(have) ? have : [have, have];
    if (want < lo || want > hi) return col;
  }
  return null;
}

/** 実在する列だけ残す。0 列ならその表の全列(最大 4)。 */
function pickColumns(requested: string[], units: Candidate[], dropped: Dropped[]): string[] {
  const rows = units.filter((u) => u.kind === "row" && u.cells);
  if (rows.length === 0) return [];
  const known = new Set(rows.flatMap((r) => Object.keys(r.cells ?? {})));
  const kept = requested.filter((c) => known.has(c));
  for (const c of requested) if (!known.has(c)) dropped.push({ what: "unit", why: `column:${c}` });
  const first = rows[0];
  const all = first ? Object.keys(first.cells ?? {}) : [];
  if (kept.length === 0) return all.slice(0, 4);
  // キー列(表の先頭列。row_key はここから作られる)は常に出す(§画面の規則)
  const key = all[0];
  return key !== undefined && !kept.includes(key) ? [key, ...kept] : kept;
}

/**
 * 結論文。参照は選択集合の行の実在する列だけ、地の文に数字と候補外の固有名詞があれば文ごと捨てる。
 * verdict が yes / no / depends なのに basis が空、または basis が選択集合の外にあれば根拠なしとして捨てる。
 */
function renderLead(raw: Selection, selected: Set<string>, ctx: Ctx, dropped: Dropped[]): LeadSeg[] | null {
  const fail = (why: string): null => {
    dropped.push({ what: "lead", why });
    return null;
  };

  if (raw.verdict !== "none") {
    if (raw.basis.length === 0) return fail("basis_empty");
    // 選択集合の外の basis は捨てる(その札が上限や重複で落ちた場合が多い)。根拠が 1 つも残らないときだけ結論文を捨てる
    const kept = raw.basis.filter((id) => selected.has(id));
    for (const id of raw.basis) if (!selected.has(id)) dropped.push({ what: "lead", why: `basis_not_selected:${id}` });
    if (kept.length === 0) return fail("basis_empty");
  }

  const text = raw.lead;
  const segs: LeadSeg[] = [];
  let last = 0;
  for (const m of text.matchAll(PLACEHOLDER)) {
    const [whole, id, col] = m;
    const u = ctx.candidates.get(id ?? "");
    if (!id || !selected.has(id) || !u) return fail(`ref_not_selected:${id}`);
    if (u.kind !== "row" || !u.cells || !(col! in u.cells)) return fail(`ref_col:${col}`);
    segs.push({ t: text.slice(last, m.index) }, { ref: id, col: col! });
    last = m.index! + whole.length;
  }
  segs.push({ t: text.slice(last) });

  const plain = segs.map((s) => ("t" in s ? s.t : "")).join("");
  if (plain.length === 0) return fail("empty");
  if (DIGITS.test(plain)) return fail("digit");
  if (plain.length > MAX_LEAD_LENGTH) return fail("too_long");
  const foreign = ctx.pageNames.find(
    (n) => n.length >= 3 && plain.includes(n) && !ctx.candidateText.includes(n),
  );
  if (foreign) return fail(`foreign:${foreign}`);
  return segs;
}
