---
name: db-migration
description: 保存データの形を変えるときの手順。`characters` への列追加・テーブル追加・JSON 列の形の変更・`SCHEMA_VERSION` を上げる・`migrate_*` を足す・`RegisteredCharacter` / `BuffSet` に保存したい項目を足す、IndexedDB(browserStore.ts)や書き出し / 読み込み(transfer.ts)の形を変える、いずれかに触る変更では必ず使う。「キャラに〜を保存したい」「新しいテーブル」「マイグレーション」と言われたときも使う。
---

# DB マイグレーション

規約の正は `docs/adr/008-storage.md`。実装の型は `crates/storage/src/character_repository.rs` の
`from_connection` と `migrate_*`。ここは「毎回同じことを調べ直さない」ためのチェックリスト。

AGENTS.md では **Complex** 扱い(データ整合性)。researcher に出すときはこの skill を読ませ、
下の 3 か所と規約を前提に調査させる。

## 保存は 3 か所ある(片方だけ変えると壊れる)

| 場所 | 版 | 変える箇所 |
|---|---|---|
| SQLite(デスクトップ) | `SCHEMA_VERSION`(character_repository.rs) | `from_connection` の列追加・`migrate_*`、`SELECT_COLUMNS`、行 → 構造体の変換、INSERT / UPDATE |
| IndexedDB(ブラウザ版) | `SCHEMA_VERSION`(api/browserStore.ts) | `onupgradeneeded` にストア追加 / 既存行の埋め直し(`oldVersion` で分岐) |
| 書き出し / 読み込み JSON | `FORMAT_VERSION`(api/transfer.ts) | 形が変わったら上げる。旧形式は読ませず理由付きで拒否(後方互換なし) |

デスクトップとブラウザで**同じ移行を同じ意味で**入れる(IndexedDB 側は「既存行に `null` を足す」が
SQLite の `ALTER TABLE ... DEFAULT` に相当)。ブラウザ版はサイトデータ削除で消えるので、
書き出し JSON が実質のバックアップ。形を変えたら書き出し → 読み込みが往復することを確かめる。

## 規約(ADR-008)

- **`PRAGMA user_version` だけで「列が無い」と判定しない。** `PRAGMA table_info` で列の実在を見てから
  `ALTER TABLE`。列はあるのに `user_version` が 0 の DB が実在し、値だけ見ると `duplicate column name` で
  起動不能になった(独立レビューで発覚した実バグ)。
- `migrate_*` は**起動のたびに全部走る**。だから冪等にする(2 回開いても再移行しない・既定 ID が付いた行は
  触らない)。「この版だけ」の分岐に頼らない。
- データ変換(JSON の詰め替え・別テーブルへの抽出)は `unchecked_transaction` で **1 transaction**。
  途中で落ちたとき半分だけ移った DB を残さない。
- `user_version` は `from_connection` の最後に 1 回だけ `SCHEMA_VERSION` を書く。途中で書かない。
- **後方互換は持たない。** 旧値から再構成できないものは捨てて中立値にする。互換フィールド・旧パス・
  `#[serde(alias)]` で読み続ける形を残さない。新しい版で開いた DB は古い版で開けなくてよい。
- 順番に意味がある(例: v9 のバフセット抽出はキャラスキル分離の**後**)。`from_connection` の
  呼び出し順に足す場所を選び、理由をコメントに書く。
- バックアップ(`backup.rs`)は `open_with_backup` が**マイグレーション前**に `.bak.<版>` を取る。
  storage の責務なので desktop 側で二重に取らない。起動不能にしないことが最優先(復元 → インメモリ)。

## テスト(既存の型に合わせる)

`character_repository.rs` の `mod tests` に、旧スキーマを手で作って `from_connection` を通す形で書く:

1. **旧版から上がる**: `open_in_memory` → 旧状態を作る(`DROP TABLE` / `PRAGMA user_version = N` / 旧 JSON を INSERT)
   → `from_connection` → `user_version == SCHEMA_VERSION`、既存行がそのまま読める、新しい列が既定値
2. **2 回開いても再移行しない**(抽出系の migrate)
3. **列はあるが `user_version` 未設定**でも開ける(既存テスト `列は既にあるがuser_version未設定のdbも開ける` を壊さない)
4. 再構成できず捨てる値があるなら、**捨てた後の中立値**を明示的に assert する

テスト名は既存にならって日本語。ブラウザ側は svelte-check が通ることに加え、実機(`npm run build:web` の出力を配信して)で
旧版の IndexedDB を持ったまま開いて `onupgradeneeded` が走ることを見る。

## 記録

- `SCHEMA_VERSION` の doc コメントに vN で何が変わったかを 1 行足す
- `docs/adr/008-storage.md`: 「決定」に vN の節(何を・なぜ・旧値の扱い)、「v1 → v8 の変遷」の続きに 1 行
- 実 DB で確かめる: `gui-smoke` の `db-guard.ps1 -Action save` で退避してから新ビルドで開き、
  `%APPDATA%\dev.twcontext.app\` に `.bak.<版>` ができて既存キャラが読めることを見る。終わったら restore

## リリース前

スキーマを変えた版は**ダウングレードできない**。`release` skill の手順 2 で検出されるので、
お知らせ(news.json)の `changes` に「保存データの形が変わる。古い版に戻すときは書き出した JSON から」
の一言を入れるかユーザーに判断してもらう。
