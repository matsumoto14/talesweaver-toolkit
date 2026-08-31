// ホーム「リリースノート」のデータ型と、ツール自身の更新内容(静的データ)。
// 正は CHANGELOG.md。ここはユーザーに見せるぶんだけを写す(開発の進捗は docs/status.md)。
// 出す順は 公開済みの版 → これから実装するもの → 既知の不具合。
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

/** 新しい版が先頭。先頭だけホームに開いて出し、残りは畳む。 */
export const RELEASE_NOTES: ReleaseNote[] = [
  {
    // 公開日は仮置き。実際に配布した日に差し替える(CHANGELOG.md の見出しも同じ日付にする)
    version: "0.1.0",
    date: "2026-08-31",
    headline: "最初の公開版",
    changes: [
      {
        kind: "added", title: "キャラクター管理",
        text: "名前とキャラ種だけで登録でき、素ステータス・装備・スキル・バフは後から埋めるほど精度が上がります",
      },
      {
        kind: "added", title: "ダメージ計算",
        text: "キャラ・スキル・コンテンツを選ぶだけで、1 発 / 合計 / クリティカルと 1 秒あたりの火力を表示します",
      },
      {
        kind: "added", title: "なぜこの数字?",
        text: "攻撃力の内訳・相手の防御をどれだけ抜けているか・どの倍率が効いているかを、全 30 カテゴリの計算過程まで追えます",
      },
      {
        kind: "added", title: "もし〜だったら",
        text: "装備やステータスを仮に変えたときのダメージ差を、保存せずに試せます",
      },
      {
        kind: "added", title: "ホーム",
        text: "どのコンテンツに入れるか・何が足りないかを、キャラごとに一覧できます",
      },
    ],
  },
];

/** 「これから実装するもの」と「既知の不具合」。どちらもまだ版に入っていないもの。 */
export interface BacklogItem {
  /** 機能名・不具合の出る場所。無い項目もある */
  title?: string;
  text: string;
}

/** 実装したいもの(まだ入っていない)。※いまは表示確認用のテストデータ */
export const PLANNED: BacklogItem[] = [
  { title: "テスト予定1", text: "これから実装したいものが、どう並ぶかを確かめるためのダミーです" },
  { title: "テスト予定2", text: "予定は版に入っていないので、公開済みの版とは分けて出しています" },
  { text: "機能名を持たない予定のダミーです" },
];

/** 既知の不具合(直っていない)。直したら該当の版の「修正」へ移す。※いまは表示確認用のテストデータ */
export const KNOWN_ISSUES: BacklogItem[] = [
  { title: "テスト不具合1", text: "既知の不具合がどう並ぶかを確かめるためのダミーです" },
  { title: "テスト不具合2", text: "直したらこの一覧から消し、その版の「修正」に載せ替えます" },
];
