/**
 * 読み取り専用のツール関数。名前・引数の型・返り値の型を持つ(段階 1 で LLM のツール呼び出しに
 * そのまま載せる形)。中身は src/retrieve.ts。札の採番(u01…)は段階 1 で足すので、今は unit.id を
 * そのまま安定 ID として返す。
 */
import type { AliasHit, Candidate, CorrectionRow, SearchHit, UnitRow } from "./retrieve";
import * as retrieve from "./retrieve";
import { SLOTS } from "./schema";

export interface FindPagesArgs {
  name: string;
}

/** ページ名を別名・部分一致で引く。最大 10 件。 */
export async function find_pages(db: D1Database, args: FindPagesArgs): Promise<AliasHit[]> {
  return retrieve.findPages(db, args.name);
}

export interface SearchUnitsArgs {
  terms: string;
  page?: string;
}

/** FTS の OR クエリで検索する。bm25 順、最大 40 件。page を渡すとそのページを先頭に寄せる。 */
export async function search_units(db: D1Database, args: SearchUnitsArgs): Promise<SearchHit[]> {
  return retrieve.searchUnits(db, args.terms, args.page);
}

export interface GetOutlineArgs {
  page: string;
}

/** ページの見出し(section, anchor)一覧。出現順。 */
export async function get_outline(
  db: D1Database,
  args: GetOutlineArgs,
): Promise<{ section: string; anchor: string }[]> {
  return retrieve.getOutline(db, args.page);
}

export interface GetRowsArgs {
  page: string;
  anchor: string;
  table_idx: number;
}

/** 表 1 つぶんの行を ord 順で返す。 */
export async function get_rows(db: D1Database, args: GetRowsArgs): Promise<UnitRow[]> {
  return retrieve.getRows(db, args.page, args.anchor, args.table_idx);
}

export interface GetAppDataArgs {
  name: string;
}

/** wiki の外から来た訂正(correction)を名前で引く。段階 0 では常に空配列(形だけ)。 */
export async function get_app_data(db: D1Database, args: GetAppDataArgs): Promise<CorrectionRow[]> {
  return retrieve.getAppData(db, args.name);
}

export interface SlotAssignment {
  /** u01… → 候補の安定 ID。LLM に渡した順(自然順)そのまま。 */
  slotToId: Map<string, string>;
  /** 候補の安定 ID → u01…。resolveSlots が逆引きに使う。 */
  idToSlot: Map<string, string>;
}

/**
 * 候補に u01〜u40 の札を振る(§実装 s12「札は 1 つの質問の中で通し番号」)。
 * 候補の並び順(ページ › ord の自然順。retrieve.collectCandidates が作る)をそのまま u01 から採番する。
 * 20 件を超える分(回す道で積んだ札)には割り当てない。
 */
export function assignSlots(candidates: Candidate[]): SlotAssignment {
  const slotToId = new Map<string, string>();
  const idToSlot = new Map<string, string>();
  candidates.forEach((c, i) => {
    const slot = SLOTS[i];
    if (!slot) return;
    slotToId.set(slot, c.id);
    idToSlot.set(c.id, slot);
  });
  return { slotToId, idToSlot };
}
