# ADR-008: storage と migration 方針

- ステータス: 採用
- 期間: 2026-08-21〜2026-08-28
- 元文書: goals/ 各所・decisions.md(git 履歴参照)

## 背景

`crates/storage` はキャラ登録データの永続化を担う。単一の `characters` テーブルに素ステ・補正源・装備等を
JSON 列として持たせる構成で始まり、機能追加のたびに列追加・JSON 形式変更が発生してきた。後方互換を保たない
方針(AGENTS.md)のもと、DB のマイグレーション手順と失敗時の扱いを決める必要があった。

## 決定

### v9: バフセットをキャラから独立させる(2026-08-29)

- `buff_sets(id, name, choices, created_at)` を追加し、`characters.default_buff_set_id` は nullable FK (`ON DELETE SET NULL`) とする
- 旧 `stat_sources.buffs` は、非空のキャラごとに「{name}の常用バフ」へ抽出する。同一 choices でも統合しない
- テーブル作成・列追加・抽出・旧JSONからの削除は単一transactionで行い、再open時は既定IDが付いた行を再移行しない
- `StatSources` はキャラ固有の恒常補正源だけを持ち、バフは計算要求ごとの `BuffSelection` として明示する

- **rusqlite(features = ["bundled"])を採用**。SQLite を同梱でき実行環境に依存しない
  (2026-08-21 最小 E2E)。
- **DB ファイルは Tauri 標準の `app_data_dir` に置く**。パスは `%APPDATA%\dev.twcontext.app\tw-context.sqlite`
  (アプリ識別子変更に伴い旧名 `talesweaver-toolkit.sqlite` から改称。公開後は変えない前提)。
- **スキーマバージョンは `PRAGMA user_version` で管理し、起動時に自動マイグレーションする**。
  版を上げる判定は `user_version` の値だけでなく `PRAGMA table_info` で列の実在を直接確認してから
  `ALTER TABLE`/データ変換を行う(列はあるが `user_version` が未設定という実運用状態が起こり得るため)。
  マイグレーション適用後に必ず `PRAGMA user_version = SCHEMA_VERSION` を設定する。
- **後方互換は保たない(AGENTS.md 原則どおり)**。旧形式から情報を再構成できない場合は該当データを
  破棄し、中立値で読めるようにするだけ(互換フィールド・旧パスを残さない)。
- **DB ファイルはマイグレーション適用前にバージョンごとバックアップする**(一般公開の準備、決定済み)。
  - `crates/storage` の責務(desktop は呼ぶだけ)
  - 開く前に `tw-context.sqlite.bak.<app_version>` へコピー。同名があれば上書きしない。直近 3 世代のみ保持
  - ファイル破損(`PRAGMA quick_check` 不通過)時は壊れたファイルを `*.broken.<timestamp>` へ退避し、
    直近バックアップから復元して再試行。復元も失敗すればインメモリ DB で起動し、ユーザーファイルには触れない
    (**起動不能にしない**)
  - 復元・退避の発生は起動時通知として画面に出す

### v1 → v8 の変遷

1. **v1**: `characters` 1 テーブル(id, name, game_character_id, 7 ステ, awakening_stage, eta_level)。
   バージョン管理なし(dev DB 削除で再生成する運用)
2. **v2**: `stat_sources TEXT`(JSON)列を追加。ここで `PRAGMA user_version` による自動マイグレーションへ移行
3. **v3**: `equipment TEXT NOT NULL DEFAULT '{}'` 列を追加(装備攻撃力、合計 8 値)
4. **v4**: 装備を部位別モデルへ刷新。旧 `equipment`(合計 8 値)は再構成不能なため破棄し
   `power_weapon`/`strong_weapon_level` のみ引き継ぐ
5. **v5**: `main_skill_id TEXT`(NULL 可)列を追加(主軸スキル)
6. **v6**: パワーウェポン/ストロングウェポンを `equipment` から新設の `common_skills` 列へ移す
7. **v7**: 装備部位の「レリック」を「ペンダント」「ブレスレット」の 2 部位に分割。旧値はペンダントへ
   引き継ぎ、ブレスレットは中立値で始める
8. **v8**: 装備を部位ごとの単一値から「登録一覧 + 選択中 ID」へ刷新。旧 JSON は各部位 1 件の登録として
   一度だけ移行する

(上記と並行して、カタログから消えたバフ/中ディレイ減少スキル id を落とす `migrate_removed_buffs`、
バフ扱いだったキャラ固有スキルを `character_skills` へ寄せる `migrate_character_skills` も
バージョン非依存の起動時移行として実施している。)

## 却下した選択肢

- **「dev DB を削除すれば再生成される」運用**: 開発機以外(将来のユーザー環境)では成立しないため、
  v1→v2 移行時に `PRAGMA user_version` ベースの自動マイグレーションへ切り替えた。
- **`PRAGMA user_version` の値のみでの列有無判定**: v1 時点の実スキーマは `stat_sources` 列を
  `CREATE TABLE` に直接持っており、一度でも起動した DB は「列はあるが `user_version` は未設定(0)」
  になり得た。値だけで判定すると `ALTER TABLE` が `duplicate column name` で失敗し起動不能になる
  実運用バグが独立レビューで発覚したため、`PRAGMA table_info` による列の実在確認に切り替えた。
- **専用マイグレーションライブラリの導入**: `PRAGMA user_version` + 手書き `ALTER TABLE`/JSON 変換で
  要件を満たせており、追加の依存を要らないと判断(CLAUDE.md「投機的な抽象化をしない」)。
- **JSON 列の形式変更を互換フィールドで吸収**: 装備 v8(旧実測固定ダメージ)・シエナのオーラ導入時など、
  再構成不能な旧値は互換フィールドとして残さず、最も近い値へ変換するか中立値扱いにする方針を一貫して採用。

## 経緯

- 2026-08-21 最小 E2E: rusqlite(bundled)採用、`characters` 1 テーブルで開始(dev DB 削除運用)。
- 2026-08-21 キャラステータス補正源 / 2026-08-22 PR レビュー: `stat_sources` 列追加を機に
  `PRAGMA user_version` 自動マイグレーションへ切替。列実在確認の欠如による起動不能バグを独立レビューで
  発見・修正(v2)。
- 2026-08-22 装備攻撃力: `equipment` 列追加(v3)。
- 2026-08-24 部位別装備モデル: 旧 8 値を破棄し部位別へ(v4)。移行前に実 DB をバックアップ済み。
- 2026-08-25 デザインモック整合 段階1: 主軸スキル列追加(v5)。
- 2026-08-25 共通スキル導入: `common_skills` 列新設、装備からパワーウェポン/ストロングウェポンを移設(v6)。
- 2026-08-27 シエナのオーラ独立登録・装備登録刷新: レリックのペンダント/ブレスレット分割(v7)、
  装備を登録一覧+選択中 ID モデルへ(v8)。
- 2026-08-28 一般公開の準備: 後方互換を持たない方針の帰結として、配布開始前にバックアップ/復元の
  仕組みを導入する決定(`crates/storage/src/backup.rs` として実装済み)。同時にアプリ識別子変更に伴い
  DB ファイル名を `tw-context.sqlite` へ改称。
