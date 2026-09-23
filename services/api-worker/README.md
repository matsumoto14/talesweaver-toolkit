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
npx wrangler d1 execute tw-wiki --remote --file migrations/001-ask-log.sql -y   # schema.sql 投入後に足した表(1 回だけ)
npx wrangler d1 execute tw-wiki --remote --file migrations/002-ask-log-qkey.sql -y  # 001 の後に 1 回だけ(--file が認証エラーなら中の 1 文を --command で)
curl https://api.tw-context.dev/health
```

deploy は main への push で `.github/workflows/workers.yml` が行う(型検査 → テスト → `wrangler deploy`)。
手で出すなら `npx wrangler deploy`。`units.sql` を再投入する前後で deploy し直す必要はない(表の中身だけ変わる)。

## 1 日の上限と記録

- **連打止め**: 1 分 10 問(IP のハッシュ、Rate Limiting バインディング `ASK_BURST`)
- **1 日の上限**: `RATE_LIMIT_PER_DAY`(既定 20 問、wrangler.toml の vars)。単位は**端末**(`x-client-id`、
  端末が localStorage に持つ UUID)のハッシュ。ヘッダが無ければ IP のハッシュ。費用の歯止めは正直な
  利用者向けで、ID を消せば別の端末として数え直される(PoW を解き直す手間だけ)。突破する人への
  歯止めは連打止めと Anthropic Console の月額上限
- **答えのキャッシュ**(`src/cache.ts`、KV `answer:<qkey のハッシュ>`): 「役に立った」が付いた答えを、同じ意味の質問
  (分かち書きした語の集合が同じ。語順・助詞・空白は無視)に LLM を呼ばず返す(`route: "cached"`)。キャラ状態は
  キーに入れず、答えが状態で行を絞っていた(steps の `filtered_by`)ときだけ同じ状態の人に限って返す。
  wiki の版(meta の synced_at / imported_at)が変わったら捨てる。続きの質問(prev あり)は貯めない・引かない。
  TTL 30 日。評価で外したいときは要求の `debug: { cache: false }`
- **プロンプトキャッシュ**(回す道だけ): 往復ごとに最後のメッセージへ `cache_control` を 1 つ付け、履歴を
  キャッシュから読ませる。Haiku 4.5 は前方の合計が 4096 トークン未満だと効かないので、1〜2 往復目は普通に課金。
  効き具合は `dropped` の `route: loop:<回>/<ms>/in<入力>+cached<キャッシュ読み>+written<キャッシュ書き>/out<出力>` と ask_log の body で見る
- **記録**(`ask_log`): 質問・キャラ状態(level / evolution だけ)・答えの JSON 全文・経路・所要時間・端末のハッシュ。
  IP と装備の中身は入らない。端末側の注記は「質問は外部の AI サービスに送ります(キャラや装備の中身は送りません)」だけで、
  記録することは書いていない(2026-09-23 ユーザー判断。戻すなら AskPage.svelte の注記に 1 文足す)。保持期間・削除の手段は未定。見るときは
  ```
  npx wrangler d1 execute tw-wiki --remote --command "SELECT at, substr(user,1,8) AS who, kind, route, ms, question, lead FROM ask_log ORDER BY id DESC LIMIT 50"
  npx wrangler d1 execute tw-wiki --remote --command "SELECT substr(user,1,8) AS who, count(*) AS n FROM ask_log WHERE at >= date('now') GROUP BY user ORDER BY n DESC"
  ```

`units.sql` は先頭で全表を `DELETE` してから入れ直すので、wiki を再同期(`sync.py`)したら
`units.py` → `--remote` の 2 手で更新できる。`unit_fts` も毎回作り直す。
生きている Worker インスタンスは別名の辞書をキャッシュしているので、投入直後は古い辞書で
動くことがある(次の deploy か isolate の入れ替わりで揃う)。

## 評価

```
npm run dev                                          # ローカル D1 に全件入れてから
node --experimental-strip-types eval/run.ts          # 段階 0: /search の再現率(LLM 不要)
node --experimental-strip-types eval/run.ts --ask               # /ask を叩く(ANTHROPIC_API_KEY が要る)。回す道あり
node --experimental-strip-types eval/run.ts --ask --no-understand  # 理解を飛ばして再現率を比べる
node --experimental-strip-types eval/run.ts --ask --no-loop        # 回す道を無効化(安い道だけの数字)
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
| `src/tools.ts` | `src/retrieve.ts` のラップ + 候補への札の採番(`assignSlots`)。安い道(選択 1 回)の下地 |
| `src/agent.ts` | 回す道(段階 2)。手動のツールループ(`runAgentLoop`)。ツールは `src/retrieve.ts` を直接呼ぶ |
| `src/wiki-url.ts` | `page.url`(EUC-JP percent-encoding 済み。units.py が作る)+ アンカーの連結 |
| `tools/segment-cli.ts` | `src/segment.ts` を読み込む CLI。units.py が子プロセスとして呼ぶ |
| `eval/run.ts` | 評価セットの実行(`--ask` / `--no-understand` / `--no-loop`) |

## 回す道(段階 2、`src/agent.ts`)

安い道(理解 1 回 + 選択 1 回)で解けなかった質問だけ、ツールを渡した手動のループ(`client.messages.create`
の `stop_reason: "tool_use"` の間ツールを実行して返す形。claude-api skill の Manual Agentic Loop)に
1 回だけ回す。振り分けは `runAsk`(`src/index.ts`):

1. 理解の `hops: "multi"` → 最初から回す道(`route: "loop"`)。安い道の検索・選択は呼ばない
2. 安い道の結果が `none`(`llm_none` / `verification_failed`)、または答えの `missing` が空でなければ、
   **1 回だけ**回す道でやり直す(`route: "cheap_then_loop"`)。回す道でも駄目なら安い道の結果をそのまま返す
   (「駄目」= ツール呼び出し無しで終わった・`answer` の JSON が壊れている・API エラー・`max_tokens` が
   2 回続いた。`agent.runAgentLoop` が `null` を返す)
3. 1 問につき回す道は 1 回まで(1 で試したら 2 では試さない)

上限はツール呼び出し 5 回・全体 12 秒(理解と安い道の時間を除く)・札 40・入力トークン概算 40,000。
上限に達したら次の 1 往復だけ `tool_choice: {type:"tool", name:"answer"}` で `answer` を強制する。

**リクエストごとのタイムアウトは 20 秒(全体予算の 12 秒より緩い)。** 最初の実装は「残り予算(12 秒 −
経過時間)」をそのままリクエストの `timeout` に渡していたが、実測で Haiku 4.5 の 1 往復が 12 秒を
超えることがあり、1 回目のリクエストからタイムアウトで打ち切ってしまっていた(`Error: Request timed
out.`)。全体 12 秒の歯止めは「次の往復から `answer` を強制する」ソフトな予算として持ち、リクエスト
自体は落とさないようにした。

### 実測(2026-09-22、ローカル D1 全件、Haiku 4.5、`RATE_LIMIT_DISABLED=1`)

`エタ解放までのクエストの流れわかる?`(hops: multi、`route: "loop"`)と
`喪失の島の侵入モブが強すぎる`(`route: "loop"`、`trouble: cant_win`)を数回ずつ実 API で通した:

- 所要時間はループだけで 8.5〜12.3 秒(ツール呼び出し 5 回で `answer` を強制した回が大半)。
  リクエスト全体(理解 + ループ)は 10〜16 秒
- 5 回のツール呼び出しでは、多段(ページをまたぐ)質問を最後まで辿りきれず `none` で終わることが
  あった(`find_pages` → `search_units` → `get_outline` → `get_rows` → `search_units` で 5 回使い切り、
  2 ページ目以降に届く前に強制 `answer`)
- **12 秒は妥当(そのままにする)。** ソフトな予算に変えたことで、超えても answer を強制するだけで
  失敗にはならない。ツール呼び出し 5 回のほうが先に効くことが多いので、辿りきれない質問が残るなら
  次に見るのは回数の上限(5)を上げるか、1 回の `search_units` の返す情報量を増やすこと

評価セット(`eval/run.ts --ask`、85 問、行一致・none 率は「期待あり」70 問)の比較(同日、1 回ずつ):

| | 安い道で解けなかった率(none または missing あり) | 期待ありの行一致 | route ごとの件数 |
|---|---|---|---|
| 安い道のみ(`--no-loop`) | 35/70(50%) | 58/70 | cheap=65 / (none)=20 |
| 回す道あり(既定) | 20/70(29%) | 48/70 | cheap=34 / cheap_then_loop=15 / loop=3 / (none)=33 |
| 段階 1 の実測(参考、当時 70 問) | 31/70(44%) | — | — |

回す道を足したことで「安い道で解けなかった率」は 50%→29%(同日の対照)に下がり、目安の 2 割には
まだ届かないが段階 1 からは大きく改善した。`cheap_then_loop` に回った 15 問は行一致 12/14・none 率
0/15 で、安い道が `none` や `missing` ありだった質問の大半を回す道が拾えている。一方で全体の行一致が
58→48 に下がって見えるのは、`missing` が非空なら（安い道の答えが部分的に合っていても）回す道の答えで
**まるごと置き換える**契約(§振り分け 2)の効果と、1 回ずつしか測っていない LLM の揺れの両方が乗っている
可能性があり、揺れを切り分けるには複数回の対照実験が要る(残作業)。
