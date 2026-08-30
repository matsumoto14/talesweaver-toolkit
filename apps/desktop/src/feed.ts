// ホーム「うごき」列のデータ型と、ツール自身の更新履歴(静的データ)。
// 外部フィード(TW公式・韓国の一次情報)は今回のスコープ外(docs/status.md 参照)。
// 型だけ将来の合流先として用意しておく — source: "official" | "kr" はまだ値を持たない。
export type FeedSource = "official" | "kr" | "tool";
export type FeedKind = "release" | "known_issue";

export interface FeedItem {
  /** ISO8601 の日付(YYYY-MM-DD)。表示は MM-DD に整形する */
  date: string;
  source: FeedSource;
  kind?: FeedKind;
  title: string;
  /** 期限つきイベントの締切(ISO8601)。期限カードの選定にのみ使う(今回は生成しない) */
  deadline?: string;
  note?: string;
}

/** ツールの更新履歴(docs/status.md の直近の実績から起こす)。既知のバグは今は無い。 */
export const FEED_ITEMS: FeedItem[] = [
  { date: "2026-08-29", source: "tool", kind: "release", title: "ソウルリンクのリンクステータスを実装" },
  { date: "2026-08-29", source: "tool", kind: "release", title: "バフセットを独立管理に変更(作成・複製・削除)" },
  { date: "2026-08-27", source: "tool", kind: "release", title: "装備登録を刷新(部位別の複数登録・強化等級・装着アビリティ)" },
  { date: "2026-08-27", source: "tool", kind: "release", title: "シエナのオーラを装備と別管理の独立登録に変更" },
  { date: "2026-08-27", source: "tool", kind: "release", title: "スキルアイコンを全プレイアブルキャラぶん収録" },
];
