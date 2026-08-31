// お知らせ(更新内容・これから・既知の不具合)のデータ。
//
// 中身は **配布先(R2)に置いた JSON が正**で、アプリを出し直さずに書き換えられる。
// 同じファイルを同梱もしているので、通信できないときは同梱ぶんを出す(お知らせが
// 白紙になるより、少し古い内容が見えるほうがよい)。
//
// 元ファイルは `src/data/news.json`。リリース CI がそのまま R2 へ上げるので、
// 直すのはこの 1 か所だけでよい(手で 2 か所を揃えない)。
// WebView の fetch ではなく Rust 側から取る。配信元(R2)に CORS 設定を要求しないため。
// 許可先は capabilities の `http:default`(dl.tw-context.dev だけ)で縛っている。
import { fetch } from "@tauri-apps/plugin-http";

import bundled from "./data/news.json";

export type ReleaseChangeKind = "added" | "changed" | "fixed";

export const CHANGE_LABELS: Record<ReleaseChangeKind, string> = {
  added: "追加",
  changed: "変更",
  fixed: "修正",
};

export interface ReleaseChange {
  kind: ReleaseChangeKind;
  /** 機能名。無い項目もある */
  title?: string;
  /** その版で何ができるようになったかを、使う人の言葉で 1 文 */
  text: string;
}

export interface ReleaseNote {
  /** 版番号 */
  version: string;
  /** 公開日(ISO8601 の YYYY-MM-DD) */
  date: string;
  /** 版の一言説明 */
  headline?: string;
  changes: ReleaseChange[];
}

/** 「これから実装するもの」と「既知の不具合」。どちらもまだ版に入っていないもの。 */
export interface BacklogItem {
  /** 機能名・不具合の出る場所。無い項目もある */
  title?: string;
  text: string;
}

export interface News {
  /** 新しい版が先頭 */
  releases: ReleaseNote[];
  planned: BacklogItem[];
  knownIssues: BacklogItem[];
}

/**
 * お知らせの配信元。
 *
 * **この URL は配布したアプリの capabilities に焼き込まれる。** 変えると、既に入っている版は
 * 同梱ぶんしか見られなくなる。変えるときは `src-tauri/capabilities/default.json` の
 * `http:default` の許可先も必ず同じ URL にする(問い合わせ先と同じ約束)。
 */
export const NEWS_ENDPOINT = "https://dl.tw-context.dev/news/news.json";

/** 同梱ぶん。取得できないときと、取得するまでのあいだに出す */
export const BUNDLED_NEWS = bundled as News;

const isBacklog = (v: unknown): v is BacklogItem =>
  typeof v === "object" && v !== null && typeof (v as BacklogItem).text === "string";

const isRelease = (v: unknown): v is ReleaseNote => {
  if (typeof v !== "object" || v === null) return false;
  const note = v as ReleaseNote;
  return typeof note.version === "string" && typeof note.date === "string"
    && Array.isArray(note.changes)
    && note.changes.every((c) => c !== null && typeof c === "object" && c.kind in CHANGE_LABELS && typeof c.text === "string");
};

/** 配信元の JSON は手で書き換えるので、形が違うものは捨てて同梱ぶんを使う */
export const parseNews = (value: unknown): News | null => {
  if (typeof value !== "object" || value === null) return null;
  const news = value as News;
  if (!Array.isArray(news.releases) || !news.releases.every(isRelease)) return null;
  if (!Array.isArray(news.planned) || !news.planned.every(isBacklog)) return null;
  if (!Array.isArray(news.knownIssues) || !news.knownIssues.every(isBacklog)) return null;
  return news;
};

/**
 * 配信元から取り直す。取れない・形が違うときは同梱ぶん。
 * お知らせが見えないこと自体は困らないので、失敗をエラー帯には出さない。
 */
export async function fetchNews(): Promise<News> {
  try {
    const response = await fetch(NEWS_ENDPOINT, { cache: "no-cache" });
    if (!response.ok) return BUNDLED_NEWS;
    return parseNews(await response.json()) ?? BUNDLED_NEWS;
  } catch {
    return BUNDLED_NEWS;
  }
}
