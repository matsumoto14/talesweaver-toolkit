/** D1 からの読み取り専用の取得関数。src/tools.ts がツールの形にラップする。 */

export interface PageRow {
  name: string;
  url: string;
  mtime: string | null;
  fetched_at: string | null;
  checked_at: string | null;
  status: string;
  error: string | null;
}

export interface UnitRow {
  id: string;
  kind: string;
  page: string;
  section: string;
  anchor: string;
  ord: number;
  truncated: number;
  text: string;
  table_idx: number | null;
  group_key: string | null;
  row_key: string | null;
  cells: string | null;
  nums: string | null;
}

export interface SearchHit {
  id: string;
  kind: string;
  page: string;
  section: string;
  anchor: string;
  ord: number;
  table_idx: number | null;
  score: number;
  text: string;
  truncated: number;
}

export interface AliasHit {
  name: string;
  page: string;
}

/** alias の前方/部分一致 + ページ名 → 最大 10 件。 */
export async function findPages(db: D1Database, name: string): Promise<AliasHit[]> {
  const like = `%${name}%`;
  const result = await db
    .prepare(
      `SELECT name, page FROM alias
       WHERE name = ?1 OR name LIKE ?2
       ORDER BY (name = ?1) DESC, length(name) ASC
       LIMIT 10`,
    )
    .bind(name, like)
    .all<AliasHit>();
  return result.results ?? [];
}

/**
 * FTS `MATCH` の OR クエリで検索し bm25 順、LIMIT 40 で返す。
 * page を渡したときはそのページのユニットを先頭に寄せる(加点)。
 */
export async function searchUnits(
  db: D1Database,
  query: string,
  page?: string,
): Promise<SearchHit[]> {
  const statement = page
    ? db.prepare(
        `SELECT u.id, u.kind, u.page, u.section, u.anchor, u.ord, u.table_idx, u.text, u.truncated,
                bm25(unit_fts) - (CASE WHEN u.page = ?2 THEN 1000 ELSE 0 END) AS score
         FROM unit_fts JOIN unit u ON u.rowid = unit_fts.rowid
         WHERE unit_fts MATCH ?1
         ORDER BY score LIMIT 40`,
      ).bind(query, page)
    : db.prepare(
        `SELECT u.id, u.kind, u.page, u.section, u.anchor, u.ord, u.table_idx, u.text, u.truncated,
                bm25(unit_fts) AS score
         FROM unit_fts JOIN unit u ON u.rowid = unit_fts.rowid
         WHERE unit_fts MATCH ?1
         ORDER BY score LIMIT 40`,
      ).bind(query);

  const result = await statement.all<SearchHit>();
  return result.results ?? [];
}

/** 見出し(section, anchor)の一覧。unit から DISTINCT、出現順。 */
export async function getOutline(
  db: D1Database,
  page: string,
): Promise<{ section: string; anchor: string }[]> {
  const result = await db
    .prepare(
      `SELECT DISTINCT section, anchor, MIN(ord) AS first_ord
       FROM unit WHERE page = ?1 GROUP BY section, anchor ORDER BY first_ord`,
    )
    .bind(page)
    .all<{ section: string; anchor: string; first_ord: number }>();
  return (result.results ?? []).map((r) => ({ section: r.section, anchor: r.anchor }));
}

/** 表の行を ord 順。 */
export async function getRows(
  db: D1Database,
  page: string,
  anchor: string,
  tableIdx: number,
): Promise<UnitRow[]> {
  const result = await db
    .prepare(
      `SELECT * FROM unit WHERE page = ?1 AND anchor = ?2 AND table_idx = ?3 AND kind = 'row'
       ORDER BY ord`,
    )
    .bind(page, anchor, tableIdx)
    .all<UnitRow>();
  return result.results ?? [];
}

export interface CorrectionRow {
  id: string;
  subject: string;
  unit_id: string | null;
  col: string;
  value: string;
  grade: string;
  source_kind: string;
  source_title: string;
  source_url: string | null;
  /** `unit_id` が NULL の行(app_data)の疑似候補用の節名。それ以外は NULL(段階 3 spec B 7)。 */
  section: string | null;
}

/** correction を name(ページ名か subject)で引く。段階 0 では常に空(表に投入していない)。 */
export async function getAppData(db: D1Database, name: string): Promise<CorrectionRow[]> {
  const result = await db
    .prepare(
      `SELECT c.* FROM correction c
       LEFT JOIN unit u ON u.id = c.unit_id
       WHERE c.subject = ?1 OR u.page = ?1`,
    )
    .bind(name)
    .all<CorrectionRow>();
  return result.results ?? [];
}

/** 候補に重ねる訂正。対象ユニット ID の集合で引く(unit_id が NULL の訂正は含まない)。 */
export async function getCorrectionsForUnitIds(
  db: D1Database,
  unitIds: string[],
): Promise<CorrectionRow[]> {
  if (unitIds.length === 0) return [];
  const placeholders = unitIds.map((_, i) => `?${i + 1}`).join(",");
  const result = await db
    .prepare(`SELECT * FROM correction WHERE unit_id IN (${placeholders})`)
    .bind(...unitIds)
    .all<CorrectionRow>();
  return result.results ?? [];
}

export interface ColumnNote {
  note: string;
  state_key: string | null;
}

/** 列名辞書。名前 → { 意味, キャラ状態のキー }。 */
export async function getColumnNotes(db: D1Database): Promise<Record<string, ColumnNote>> {
  const result = await db
    .prepare("SELECT name, note, state_key FROM column_note")
    .all<{ name: string; note: string; state_key: string | null }>();
  const out: Record<string, ColumnNote> = {};
  for (const row of result.results ?? []) out[row.name] = { note: row.note, state_key: row.state_key };
  return out;
}

// --- 候補の集め方(段階 1。§3「検索の前に質問を理解・書き換える段を置く」) ------------------

/** 選択(2 回目)の LLM に渡す 1 候補。安定 ID・種類・セル(訂正の重ね書き前)を持つ。 */
export interface Candidate {
  id: string;
  kind: "paragraph" | "row";
  page: string;
  section: string;
  anchor: string;
  ord: number;
  table_idx: number | null;
  group_key: string | null;
  row_key: string | null;
  text: string;
  truncated: boolean;
  cells: Record<string, string> | null;
  nums: Record<string, number | [number, number]> | null;
  /** 重ねる訂正。answer.ts が corrections[] を組み、prompt.ts が訂正後の値で描く。 */
  corrections?: CandidateCorrection[];
}

export interface CandidateCorrection {
  col: string;
  value: string;
  grade: string;
  source: { kind: string; title: string; url: string | null };
}

/** candidates に corrections[] を重ねる(unit_id で引いた CorrectionRow から)。破壊的に更新する。 */
export function attachCorrections(candidates: Candidate[], rows: CorrectionRow[]): void {
  const byUnit = new Map<string, CorrectionRow[]>();
  for (const r of rows) {
    if (!r.unit_id) continue;
    const list = byUnit.get(r.unit_id) ?? [];
    list.push(r);
    byUnit.set(r.unit_id, list);
  }
  for (const c of candidates) {
    const rs = byUnit.get(c.id);
    if (!rs || rs.length === 0) continue;
    c.corrections = rs.map((r) => ({
      col: r.col,
      value: r.value,
      grade: r.grade,
      source: { kind: r.source_kind, title: r.source_title, url: r.source_url },
    }));
  }
}

/** 候補の列に訂正が重なっていれば、その値(訂正後)を返す。無ければ wiki のセルのまま。 */
export function correctedValue(c: Candidate, col: string): string | undefined {
  const hit = c.corrections?.find((cc) => cc.col === col);
  return hit ? hit.value : c.cells?.[col];
}

/** UnitRow → Candidate(cells/nums の JSON を復元)。回す道(agent.ts)の get_rows もこれを使う。 */
export function toCandidate(u: UnitRow): Candidate {
  return {
    id: u.id,
    kind: u.kind === "row" ? "row" : "paragraph",
    page: u.page,
    section: u.section,
    anchor: u.anchor,
    ord: u.ord,
    table_idx: u.table_idx,
    group_key: u.group_key,
    row_key: u.row_key,
    text: u.text,
    truncated: u.truncated === 1,
    cells: u.cells ? (JSON.parse(u.cells) as Record<string, string>) : null,
    nums: u.nums ? (JSON.parse(u.nums) as Record<string, number | [number, number]>) : null,
  };
}

/** 塊のキー。表の行は page/anchor/table_idx、箇条書きは group_key、単独の段落は自分の id。 */
function groupKeyOf(c: Candidate): string {
  return c.group_key ?? c.id;
}

/**
 * 行の列にキャラ状態と同じ意味の列があり、数値化できて、値が範囲外ならその列名を返す。
 * 数値化できない列(nums に無い)は矛盾と見なさない。
 */
export function conflictingColumn(
  c: Candidate,
  state: Record<string, number>,
  columnNotes: Record<string, ColumnNote>,
): string | null {
  if (!c.nums) return null;
  for (const [col, note] of Object.entries(columnNotes)) {
    if (!note.state_key) continue;
    const want = state[note.state_key];
    const have = c.nums[col];
    if (want === undefined || have === undefined) continue;
    const [lo, hi] = Array.isArray(have) ? have : [have, have];
    if (want < lo || want > hi) return col;
  }
  return null;
}

/** 表の行を状態で絞る。絞った結果が 0 行なら絞らない(条件が合わない表を消さない)。安い道と回す道で共用。 */
export function filterRowsByState<T extends Candidate>(
  rows: T[],
  state: Record<string, number>,
  columnNotes: Record<string, ColumnNote>,
): T[] {
  const filtered = rows.filter((c) => !conflictingColumn(c, state, columnNotes));
  return filtered.length > 0 ? filtered : rows;
}

export interface CollectCandidatesInput {
  /** 質問の分かち書き + 理解の terms を混ぜた OR クエリ(呼び元が segment.toQuery で作る)。 */
  query: string;
  /** 理解が選んだページ(0〜3 件)。先頭に寄せる加点。 */
  boostPages: string[];
  state: Record<string, number>;
  columnNotes: Record<string, ColumnNote>;
}

/**
 * §3「検索候補を集める」/ §5「絞り込みの規則」。
 * FTS(terms OR、加点)→ 行が当たったら同じ表の残りの行を足す(1 表 25 行) →
 * state で絞る(0 行になったら絞らない)→ ページ › ord の自然順 → 20 件
 * (溢れは検索スコア順に落とすが、同じ表の行は塊で残す)。
 */
export async function collectCandidates(
  db: D1Database,
  input: CollectCandidatesInput,
): Promise<Candidate[]> {
  const rawHits = await searchUnits(db, input.query);
  if (rawHits.length === 0) return [];

  // ページ加点: 理解が選んだページを先頭に寄せる(安定ソート)。
  const boost = new Set(input.boostPages);
  const ordered = boost.size > 0
    ? [...rawHits].sort((a, b) => Number(boost.has(b.page)) - Number(boost.has(a.page)))
    : rawHits;

  const byId = new Map<string, Candidate>();
  const order: string[] = []; // 発見順(スコア順)。溢れたときの切り方に使う
  const tablesSeen = new Set<string>();

  for (const hit of ordered) {
    if (byId.has(hit.id)) continue;
    if (hit.kind === "row" && hit.table_idx !== null) {
      const tableKey = `${hit.page}\u0000${hit.anchor}\u0000${hit.table_idx}`;
      if (!tablesSeen.has(tableKey)) {
        tablesSeen.add(tableKey);
        const rows = await getRows(db, hit.page, hit.anchor, hit.table_idx);
        // 当たった行は必ず残す: 当たった行を中心に前後を取り、1 表 25 行までにする
        const at = Math.max(0, rows.findIndex((r) => r.id === hit.id));
        const start = Math.max(0, Math.min(at - 12, rows.length - 25));
        for (const row of rows.slice(start, start + 25)) {
          if (byId.has(row.id)) continue;
          byId.set(row.id, toCandidate(row));
          order.push(row.id);
        }
        continue;
      }
    }
    byId.set(hit.id, toCandidate({
      id: hit.id, kind: hit.kind, page: hit.page, section: hit.section, anchor: hit.anchor,
      ord: hit.ord, truncated: hit.truncated, text: hit.text, table_idx: hit.table_idx,
      group_key: null, row_key: null, cells: null, nums: null,
    }));
    order.push(hit.id);
  }

  // state で絞る。表(group)単位で見て、絞った結果が 0 行になる表は絞らない。
  const byGroup = new Map<string, Candidate[]>();
  for (const id of order) {
    const c = byId.get(id);
    if (!c) continue;
    const key = groupKeyOf(c);
    const list = byGroup.get(key) ?? [];
    list.push(c);
    byGroup.set(key, list);
  }
  const kept = new Set<string>();
  for (const list of byGroup.values()) {
    const filtered = list.filter((c) => !conflictingColumn(c, input.state, input.columnNotes));
    const survivors = filtered.length > 0 ? filtered : list;
    for (const c of survivors) kept.add(c.id);
  }

  // ページ › ord の自然順に固定(検索スコアでは並べない)。
  const natural = order
    .filter((id) => kept.has(id))
    .map((id) => byId.get(id)!)
    .sort((a, b) => (a.page === b.page ? a.ord - b.ord : a.page.localeCompare(b.page)));

  // 20 件に切る。溢れたら検索スコア順(= order の出現順)で落とすが、同じ表(group)は塊で残す。
  if (natural.length <= MAX_CANDIDATES_CAP) return natural;
  const groupScoreRank = new Map<string, number>();
  order.forEach((id, i) => {
    const c = byId.get(id);
    if (!c || !kept.has(id)) return;
    const key = groupKeyOf(c);
    if (!groupScoreRank.has(key)) groupScoreRank.set(key, i);
  });
  const groups = [...byGroup.entries()]
    .filter(([key]) => groupScoreRank.has(key))
    .map(([key, list]) => ({
      key,
      rank: groupScoreRank.get(key)!,
      units: list.filter((c) => kept.has(c.id)),
    }))
    .sort((a, b) => a.rank - b.rank);

  const picked: Candidate[] = [];
  for (const g of groups) {
    if (picked.length + g.units.length > MAX_CANDIDATES_CAP) {
      if (picked.length === 0) { picked.push(...g.units.slice(0, MAX_CANDIDATES_CAP)); }
      continue;
    }
    picked.push(...g.units);
  }
  const pickedIds = new Set(picked.map((c) => c.id));
  return natural.filter((c) => pickedIds.has(c.id));
}

const MAX_CANDIDATES_CAP = 20;

/** app_data 由来の疑似候補の上限(段階 3 spec B 7)。 */
const MAX_APP_DATA_CANDIDATES = 3;

/** subject 1 件ぶんの correction 行(unit_id が NULL)を、1 つの疑似候補(row)にまとめる。
 * 回す道(agent.ts)の get_app_data も同じ形を使う(段階 3 spec B 7)。 */
export function toAppDataCandidate(subject: string, rows: CorrectionRow[]): Candidate {
  const cells: Record<string, string> = { 名前: subject };
  for (const r of rows) cells[r.col] = r.value;
  const section = rows[0]?.section ?? "";
  return {
    id: `c:app/${subject}`,
    kind: "row",
    page: "アプリのデータ",
    section,
    anchor: "app",
    ord: 0,
    table_idx: null,
    group_key: null,
    row_key: subject,
    text: "",
    truncated: false,
    cells,
    nums: null,
  };
}

/**
 * 質問の分かち書きの語 + 理解の terms が `correction.subject`(`unit_id` が NULL、= wiki の行と
 * 結び付かない静的データ)に**完全一致**したときだけ、その subject の全セルを 1 つの疑似候補に
 * まとめて返す(段階 3 spec B 7)。alias 表には入れない(ページ候補を汚さない)。
 */
export async function getAppDataCandidates(db: D1Database, subjects: string[]): Promise<Candidate[]> {
  // 呼び元が語 → correction.subject の原文に戻してから渡す(fold 済みの語では完全一致しない)。bind は 100 まで
  const words = [...new Set(subjects.filter((t) => t.length > 0))].slice(0, 90);
  if (words.length === 0) return [];
  const placeholders = words.map((_, i) => `?${i + 1}`).join(",");
  const result = await db
    .prepare(
      `SELECT * FROM correction WHERE unit_id IS NULL AND subject IN (${placeholders})`,
    )
    .bind(...words)
    .all<CorrectionRow>();
  const rows = result.results ?? [];

  const bySubject = new Map<string, CorrectionRow[]>();
  for (const r of rows) {
    const list = bySubject.get(r.subject) ?? [];
    list.push(r);
    bySubject.set(r.subject, list);
  }

  const out: Candidate[] = [];
  for (const word of words) {
    if (out.length >= MAX_APP_DATA_CANDIDATES) break;
    const subjectRows = bySubject.get(word);
    if (!subjectRows || subjectRows.length === 0) continue;
    out.push(toAppDataCandidate(subjectRows[0]!.subject, subjectRows));
  }
  return out;
}

/** ページ名 → page.url(出典 URL の組み立て用)。 */
export async function getPageUrls(db: D1Database, names: string[]): Promise<Map<string, string>> {
  const unique = [...new Set(names)];
  const out = new Map<string, string>();
  if (unique.length === 0) return out;
  const placeholders = unique.map((_, i) => `?${i + 1}`).join(",");
  const result = await db
    .prepare(`SELECT name, url FROM page WHERE name IN (${placeholders})`)
    .bind(...unique)
    .all<{ name: string; url: string }>();
  for (const row of result.results ?? []) out.set(row.name, row.url);
  return out;
}

export interface UnitLinkRow {
  unit_id: string;
  page: string;
  ord: number;
}

/** 選ばれたユニットが張っている [[リンク]] 先(出現順)。次の一手(answer.ts)が使う。 */
export async function getUnitLinks(db: D1Database, unitIds: string[]): Promise<UnitLinkRow[]> {
  if (unitIds.length === 0) return [];
  const placeholders = unitIds.map((_, i) => `?${i + 1}`).join(",");
  const result = await db
    .prepare(`SELECT unit_id, page, ord FROM unit_link WHERE unit_id IN (${placeholders})`)
    .bind(...unitIds)
    .all<UnitLinkRow>();
  return result.results ?? [];
}

export interface TableInfo {
  caption: string | null;
  row_count: number;
}

/** 表の行数・キャプション(応答 JSON の `table` 用)。 */
export async function getTableInfo(
  db: D1Database,
  page: string,
  anchor: string,
  tableIdx: number,
): Promise<TableInfo | null> {
  const row = await db
    .prepare(
      "SELECT caption, row_count FROM wiki_table WHERE page = ?1 AND anchor = ?2 AND table_idx = ?3",
    )
    .bind(page, anchor, tableIdx)
    .first<TableInfo>();
  return row ?? null;
}

/** 全ページ名の一覧(結論文の固有名詞照合用)。 */
export async function getAllPageNames(db: D1Database): Promise<string[]> {
  const result = await db.prepare("SELECT name FROM page").all<{ name: string }>();
  return (result.results ?? []).map((r) => r.name);
}

export async function getMeta(db: D1Database): Promise<Record<string, string>> {
  const result = await db.prepare("SELECT key, value FROM meta").all<{ key: string; value: string }>();
  const out: Record<string, string> = {};
  for (const row of result.results ?? []) out[row.key] = row.value;
  return out;
}
