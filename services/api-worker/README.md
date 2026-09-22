# wiki に聞く — api Worker(段階 2)

「wiki に聞く」(Issue #36、docs/adr/013-wiki-import.md 段階 3)の Worker。段階 0 の検索
(`/health` `/search`)に加え、段階 1 で理解 + 選択(Claude Haiku 4.5)・認証(PoW → セッション
トークン)・訂正の重ね書き・リアクションを持つ(`/challenge` `/session` `/ask` `/react`)。
段階 2 で `/ask` に続きの判定(`prev` → `followup`)・進み具合の SSE・次の一手(`next`)を足し、
`/react` に値の指摘(`value_wrong`、列ごとに 1 回)を足した。回す道(`agent.ts`)はまだ無い
(評価の結果を見てから作るかどうか決める)。
inquiry-worker とは別 Worker・別ホスト(`api.tw-context.dev`)、別の秘密・別のレート制限。

```
npm install
npm run check   # tsc --noEmit
npm test        # vitest
npm run dev     # wrangler dev。ローカル D1 は http://127.0.0.1:8787
npm run deploy
```

## 秘密の入れ方

- 本番: `wrangler secret put ANTHROPIC_API_KEY` / `wrangler secret put NONCE_SECRET`
  (`ANTHROPIC_API_KEY` は Anthropic Console でこの Worker 専用に発行し、月額の費用上限を
  鍵側に付ける。コードの外に置く。`NONCE_SECRET` は inquiry-worker の値と別にする)
- ローカル: `services/api-worker/.dev.vars`(gitignore 済み。コミットしない)
  ```
  NONCE_SECRET=dev
  ANTHROPIC_API_KEY=sk-ant-...   # 無くても /health /search /challenge /session は動く。
                                  # /ask は 503(未設定)を返す。偽の鍵を入れると /ask は
                                  # 502(経路の故障として扱う。データの問題とは混ぜない)
  ```
- `ANTHROPIC_API_KEY` か `NONCE_SECRET` が欠けていると、それを要る経路だけ
  `503 {"error":"中継サーバーが未設定です: <欠けている名前>"}` を返す(`/health` `/search` は
  秘密が無くても動く)

## AI Gateway

`wrangler.toml` の `AI_GATEWAY_URL`(vars)に Cloudflare AI Gateway の URL を入れると、
Claude API 呼び出しがそこを経由する(空文字なら Anthropic SDK 既定 = 直接呼び出し)。
毎回 `cf-aig-collect-log: false` ヘッダーを付けて質問文をログに残さない。費用上限は
AI Gateway ではなく Anthropic Console の鍵側に付ける(README「秘密の入れ方」参照)。

## D1 の作成・投入

```
npx wrangler d1 create tw-wiki       # 出力の database_id を wrangler.toml に書く(作成済み。id はコミット済み)
python tools/gamedata/wiki/units.py  # リポジトリルートで。tools/gamedata/wiki/out/units.sql(約 100 MB)を作る
npx wrangler d1 execute tw-wiki --local  --file schema.sql -y     # 初回だけ(表の定義)
npx wrangler d1 execute tw-wiki --local  --file ../../tools/gamedata/wiki/out/units.sql -y
npx wrangler d1 execute tw-wiki --remote --file schema.sql -y     # 初回だけ
npx wrangler d1 execute tw-wiki --remote --file ../../tools/gamedata/wiki/out/units.sql -y
npx wrangler deploy                  # 独自ドメイン api.tw-context.dev は wrangler.toml の routes で付く
curl https://api.tw-context.dev/health
```

`units.sql` は先頭で全表を `DELETE` してから入れ直すので、wiki を再同期(`sync.py`)したら
`units.py` → `--remote` の 2 手で更新できる。`unit_fts` も毎回作り直す。
生きている Worker インスタンスは別名の辞書をキャッシュしているので、投入直後は古い辞書で
動くことがある(次の deploy か isolate の入れ替わりで揃う)。

## 評価

```
npm run dev                                          # ローカル D1 に全件入れてから
node --experimental-strip-types eval/run.ts          # 段階 0: /search の再現率(LLM 不要)
node --experimental-strip-types eval/run.ts --ask               # 段階 1: /ask を叩く(ANTHROPIC_API_KEY が要る)
node --experimental-strip-types eval/run.ts --ask --no-understand  # 理解を飛ばして再現率を比べる
```

段階 0(2026-09-22)の実測: ページ名あり 88%(ページ単位 96%)、言い換え 47%、続きの質問 20%。
数字と判断は docs/adr/020-wiki-ask.md。`--ask` は行の一致・none 率・lead の生存率と口調・
verdict の分布・安い道で解けなかった率・雑談/範囲外の kind 正誤を出す(実 API を叩くので、
`.dev.vars` に本物の鍵を入れて評価すること。費用は Anthropic Console の使用量で確認する)。

ローカル D1 はこのリポジトリの中(`services/api-worker`)で動かすこと。パスが長いと
workerd が internal error になる(scratchpad 等では動かない)。

## エンドポイント

| メソッド・パス | 認証 | 役目 |
|---|---|---|
| `GET /health` | なし | 取込日時とユニット数。索引が未投入なら 503 |
| `GET /search?q=&limit=` | なし | LLM なしの検索。`q` を分かち書きして D1 の FTS5 に投げるだけ |
| `GET /challenge` | なし | PoW の nonce と難易度(inquiry-worker と同じ形) |
| `POST /session` | PoW | 解いた nonce と引き換えに 1 時間有効のセッショントークン(HMAC 署名、KV に持たない) |
| `POST /ask` | Bearer トークン | 質問 + キャラ状態 + `prev`(続きの質問なら)→ 理解(1 回目)→ 候補収集 → 選択(2 回目)→ 検証 → 回答 JSON。`Accept: text/event-stream` なら進み具合を SSE で流す |
| `POST /react` | Bearer トークン | 答えへのリアクション(`helpful`/`wrong` は `answer_id` につき 1 回、`value_wrong` は列ごとに 1 回) |

1 質問で Claude を呼ぶのは最大 2 回(理解 + 選択)+ 選択の再試行 1 回まで。理解が落ちて(タイムアウト・
API エラー・`max_tokens` 切れ)もコード経路(別名の最長一致 + 分かち書き)で進み、止まらない。
選択が再試行しても失敗したら `502`(該当なしには化かさない。データの問題と経路の問題を混ぜない)。

### `/ask` の SSE(`Accept: text/event-stream`)

401/429/503(セッション切れ・レート制限・秘密未設定)は理解より前に判定できるので、SSE を
始める前に今までどおりの HTTP ステータスで返す。それより後に Claude 呼び出しが失敗した 502 は
SSE の中で `event: error` として流す(`content-type: text/event-stream`)。

```
event: progress
data: {"step":"understand"}

event: progress
data: {"step":"search"}

event: progress
data: {"step":"select"}

event: result
data: { ...今までの応答 JSON 全体... }
```

`Accept` ヘッダを付けなければ今までどおり素の JSON 1 個を返す(評価スクリプトと curl はこちら)。

## ファイル

| ファイル | 役目 |
|---|---|
| `src/index.ts` | ルーティング、cors、json、`/ask` の全体の流れ |
| `src/schema.ts` | 理解・選択の Zod スキーマ(`UnderstandSchema` / `SelectionSchema`)と固定 enum(`SLOTS` / `PAGE_SLOTS`) |
| `src/claude.ts` | `understand()` / `select()`(Anthropic SDK、`zodOutputFormat`)。テストではモジュールごと差し替える |
| `src/prompt.ts` | `SYSTEM_RULES`、候補の KV 形式描画(`renderCandidates`)、理解のプロンプト |
| `src/verify.ts` | LLM の生 JSON を検査し、残った手順・結論文・落とした記録を組む(`verify()`) |
| `src/answer.ts` | 検証済み選択 → 応答 JSON(`buildAnswer()`)。訂正を重ね、出典 URL を組み、`unit_link` から次の一手(`next`)を作る |
| `src/auth.ts` | PoW(inquiry-worker から複製)・セッショントークンの発行/検証・レート制限 |
| `src/react.ts` | `POST /react` の受け付け(重複防止・サニタイズ)。`helpful`/`wrong` は answer_id 単位、`value_wrong` は列単位で重複を防ぐ |
| `src/segment.ts` | 分かち書き。取込(`tools/segment-cli.ts` 経由)と検索クエリで同じ切れ目を使う |
| `src/retrieve.ts` | D1 の読み取り(alias 検索、FTS 検索、候補収集 `collectCandidates`、訂正、列辞書、outline、行、meta) |
| `src/tools.ts` | `src/retrieve.ts` のラップ + 候補への札の採番(`assignSlots`)。回す道(段階 2)のツール呼び出しの下地 |
| `src/wiki-url.ts` | `page.url`(EUC-JP percent-encoding 済み。units.py が作る)+ アンカーの連結 |
| `tools/segment-cli.ts` | `src/segment.ts` を読み込む CLI。units.py が子プロセスとして呼ぶ |
| `eval/run.ts` | 評価セットの実行(`--ask` / `--no-understand`) |
