---
name: release
description: TW Context の版を出す手順(bump → CHANGELOG → news.json → commit → タグ → CI 確認)と、版を出さずにお知らせ(news.json)だけ直す手順。「リリースして」「v0.x を出す」「版を上げる」「タグを打つ」「CHANGELOG を締める」「お知らせ / 既知の不具合を更新して」と頼まれたとき、リリース準備や配布 CI(release.yml / news.yml / R2 / 自動更新)を触るときは必ず使う。
---

# リリース

正は `.github/RELEASE.md`(初回設定・鍵・Secrets)と `.github/workflows/release.yml`。
ここは毎回の手順と、順番を間違えると壊れる点だけをまとめる。

## 全体像(なぜこの順か)

版は 3 ファイル(tauri.conf.json / package.json / Cargo.toml)に散っていて、
CI は **タグの版と tauri.conf.json の版が違うと止まる**。だから bump → commit → タグの順は崩せない。
タグを push した時点で R2 の `latest/latest.json` が書き換わり、**既存ユーザーの次回起動に更新のお知らせが出る**
(下書き Release を見てから、ではない)。戻せない操作はタグ push だけなので、そこまでを整えてから 1 回で押す。

## 手順

1. **束ねる時期か確認する**。goal ごとに出さず 2〜4 週で 1 本。前回の版からの差分を見る:
   ```sh
   git describe --tags --abbrev=0            # 前回のタグ
   git log <前回タグ>..HEAD --oneline
   ```
2. **DB スキーマが変わっていないか見る**。変わっていれば「新しい版で開いた DB は古い版で開けない」ので、
   ユーザーに一言添えて出す判断をしてもらう(`db-migration` skill の「リリース前」節):
   ```sh
   git diff <前回タグ>..HEAD --stat -- crates/storage apps/desktop/src/api/browserStore.ts apps/desktop/src/api/transfer.ts
   ```
3. **テストが通る状態か**。`cargo test --workspace`、`cd apps/desktop && npm run build && npx svelte-check`。
   CI でも回るが、タグを打ってから落ちると版だけ上がって成果物が無い状態になる。
4. **版を揃える**。MINOR は機能追加、PATCH は修正だけ:
   ```sh
   python tools/release/bump.py --show      # いまの版とズレの有無
   python tools/release/bump.py 0.2.0
   ```
5. **CHANGELOG.md** の `## [未リリース]` を `## [0.2.0] — YYYY-MM-DD` にし、空の `## [未リリース]` を上に残す。
   読者はプレイヤーなので、内部名(struct 名・ファイル名)を書かない。
6. **news.json**(`apps/desktop/src/data/news.json`)の `releases` 先頭に `version` / `date` / `changes` を足す。
   CHANGELOG と同じ内容を `kind`(added / changed / fixed)+ `title` + `text` に落とす。
   直した不具合は `knownIssues` から消して `fixed` に載せる。**同梱にも使うのでリリース commit に含める**
   (後から直すと、通信できないユーザーには古いお知らせが出続ける)。
   形は `news.yml` が検査する: `releases[].version/date/changes[].kind/text`、`planned[].text`、`knownIssues[].text`。
7. **commit とタグ**。commit・タグ・push はユーザーの指示があるときだけ。指示が無ければここまでを整えて
   下のコマンドを提示して止まる:
   ```sh
   git commit -am "release: v0.2.0"
   git tag v0.2.0
   git push origin main v0.2.0
   ```
8. **CI を見届ける**。`gh run watch` か Actions で release.yml の完了を確認し、下書き Release に
   `*-setup.exe` と `*.sig` があるか、R2 の `dl.tw-context.dev/v0.2.0/` と `latest/latest.json` が
   新しい版を指しているかを見る。**Release の公開はユーザーがする**(内容を見てから押す運用)。
9. `docs/status.md` に「vX.Y.Z を出した」を 1 行。

## お知らせだけ直す(版は出さない)

予定(`planned`)と既知の不具合(`knownIssues`)は版に紐づかない。`news.json` を直して main に push すれば
`news.yml` が R2 へ上げ、次の起動から全ユーザーに反映される。アプリを出し直さない。

## 落とし穴

- **手動実行(workflow_dispatch)は `dev/<実行番号>/` に置くだけで `latest/` を触らない**。
  経路や署名を試したいときはタグを打つ前にこちらを通す。
- 署名 Secrets(`AZURE_CLIENT_ID`)が空でもビルドは通る(未署名)。`TAURI_SIGNING_PRIVATE_KEY` が無いと
  `.sig` が出ず、CI は自動更新を配れないので**止まる**。鍵の扱いは RELEASE.md「初回だけ必要な設定」。
- 配布物は NSIS の `.exe` だけ。MSI は作らない(理由は RELEASE.md)。
- R2 へは `aws s3 cp`(S3 互換 API)。`wrangler r2 object put` はアカウント全体の権限を要求して 403 になる。
- 紹介ページ(`site/`)のダウンロードボタンは `latest/TW-Context-setup.exe` 固定。リリースで書き換えない。
