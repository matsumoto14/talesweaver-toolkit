# ADR-015: UI の部品化(呼ぶ側が決める数を 0 にする)

- ステータス: 採用
- 期間: 2026-09-17〜

## 背景

- 規格(docs/design-system.html)と機械監査(`tools/design-audit/run.py` R1〜R20)は、画面側が**決められる**ことを前提に「決め方」を縛る形で増えてきた。R16 は 4 回、R20 は 2 回書き直している。規則が要るのは、まだ画面側に決めさせているからで、規則ではなく部品側を直すべき — これは ADR-014 で確認済み。
- 部品の良し悪しは行数でも依存数でもなく **呼ぶ側が決める数**で測る。0 なら規約も監査も要らない(選べないものは違反しようがない)。
- 中身の調達順は **標準(ブラウザ)→ 借りる(ヘッドレス)→ 自前**。標準が持っている挙動を手組みしない。

## 決定

- 段階を踏んで既存の手組みを部品へ畳む。1 段階 = 1 goal = 1 commit。フォールバックを併置せず、置き換えたら古い経路は消す。
- **機械監査に R21 以降を足さない。**規則が要る状態は、まだ決めさせている信号として扱う。

### 段階 1: モーダルを `<dialog>.showModal()` にする(2026-09-17)

- `ui/Modal.svelte` を新設し、**暗幕そのもの**を `<dialog>` にする。面(`.modal-surface`)は中身側に残す。
- 呼ぶ側が決めるのは「中に何を出すか」だけ。**Escape で閉じる / フォーカスが外へ出ない / 閉じたら開いたボタンに戻る / 背面が押せない(`inert`)/ 重ね順(トップレイヤー)**はブラウザが持つ。
- 置き換え先は 4 箇所: `EquipmentPane` / `SienaPane` / `AboutPanel` / `InquiryPanel`。4 箇所のオーバーレイ CSS は**完全に同一だった**(`z-index: 90; padding: 3vh max(14px, 6vw);` + 中央寄せ)ので Modal が丸ごと持ち、`.modal-overlay` / `.about-overlay` / `.inquiry-overlay` / `.equipment-overlay` を削除した。
- 背景クリックでは閉じない(`<dialog>` の既定)。閉じる操作は面内の明示ボタンと Escape だけ — §00 ③「押した場所は動かない」を据え置く。
- 呼ぶ側に残した props は `label`(面の名前)と `closeDisabled`(送信中など、閉じさせない間だけ)の 2 つ。どちらも**そのモーダル固有の事実**であって、作法の選択肢ではない。

### 段階 2: 重なるものを `popover` 属性 + CSS Anchor Positioning にする(2026-09-17)

- `ui/Popover.svelte` を新設し、**トリガと面の両方**を持たせる(`popovertarget` で結ぶので、呼ぶ側に id を配らせない)。置き換え先は 4 箇所: `BuffsPage` の「ほか n」と「設定」/ `CalcPage` の `buff-editor` / `ui/Picker.svelte` の候補面。
- 呼ぶ側が決めるのは「中に何を出すか」だけ。**外を押すと閉じる(light dismiss)/ Escape / 重ね順(トップレイヤー)/ 祖先の `overflow` に切られない / 下に入らなければ上に開く / 中身が伸びたら置き直す**はブラウザが持つ。
- 消えたもの: `<svelte:window onclick/onkeydown>` 2 本、外側クリックの除外 selector(`closest(".rest-popover, .rest-link")` / `closest(".popover, .chip-config")`)、透明オーバーレイ `.picker-overlay`、開いている id を覚える `$state` 3 本(`openInfoId` / `openEditorId` / `buffEditorId`)、自前の位置決め `ui/popover.ts`(84 行)。
- 呼ぶ側に残した props は `label` / `triggerLabel`(読み上げ名)と `triggerClass` / `panelClass`(その画面での見た目)、`disabled` の 5 つ。どれも**その画面固有の事実**で、作法の選択肢ではない。
- 「ほか n」は行幅いっぱい(`left: 10px; right: 9px`)から**押した的の右端に揃える**に変わった。トリガと面が離れないほうが §00 ①(視線を動かさない)に合う。口そのものが読む起点になる `Picker` だけは左端に揃える(`span-inline-end`)。

#### 実機で確かめたこと(WebView2 = Chromium 153、2026-09-17)

- `HTMLElement.prototype.showPopover` / `anchor-name` / `position-anchor` / `position-try-fallbacks` / `position-area` / `anchor()` / `position-visibility` はすべて**使える**。段階 2 の時点で自前の位置決めは 1 行も要らなかった。
- **`position-try-fallbacks` の名前付き候補(`@position-try`)は一度も適用されなかった。**`flip-block` のようなキーワード候補は確実に効く。`--popover-clamp` を単独で指定しても、`max-height` を `100%` から `300px` に変えても、`position-area` をまったく別の面に変えても、はみ出した面の位置・高さは変わらなかった。バグか未実装かは追っていない。
- そのため「上にも下にも入り切らない」ときの高さの詰めは `.popover` の `max-height: calc(100dvh - 16px)` に置いた。実測: 200 行に伸ばした面が高さ 824px・上端 12px・中身だけスクロールで画面内に収まる。`flip-block` は別に効くので、収まる側があるときは今までどおり上に開く(実測: トリガ y=563 の面が y=226〜559 に置き直された)。
- 過去の実害の再現確認: ①バフタブの**スクロールする一覧の最下段**で開いても中身が全部見える(トップレイヤーなので `.chips` の `overflow-y: auto` に切られない)②クラブ効果の対象ステを 4 つ足して**背が 130 → 266px に伸びても**画面内に留まる ③開いても閉じてもトリガの矩形は 1px も動かない(§09 規則 1・3)。
- `prefers-reduced-motion: reduce` で `.pop-in` の `animation-duration` が 0.17s → 1e-05s になることも実機で確認した。

### 段階 3: 開閉を `<Disclosure>` に畳む(2026-09-17)

- `ui/Disclosure.svelte` を新設し、土台を `<details>` にする。**トリガ(`<summary>`)・面・キャレットの 3 つとも部品が持つ。**置き換え先は 36 箇所(既存の `<details>` 12 / `use:disclosurePane` 7 / `transition:collapse` 2 / 手書きの `{#if}` + `aria-expanded` トグル 15)。
- 呼ぶ側が決めるのは「トリガに何を出すか」「中に何を出すか」だけ。**キーボード操作 / ページ内検索での自動展開 / 開閉の状態 / 排他 / 読み上げの開閉状態**はブラウザが持つ。
- **`aria-expanded` を画面側から消した(29 → 5)。**`<summary>` の暗黙のロールが既に持っていて、手で書くと二重になる。残り 4 は下の「例外」。
- **キャレットの流儀を 1 つにした。**`.t-chev` / `.bg-caret` / `.stat-caret` / `.picker-chev` / `details.fold > summary::before` / TracePanel の 90° 回る SVG / RandomOptionPane の `{open ? "▾" : "›"}` の 7 通りを消し、字寸・幅・回す向きを `app.css` の `.caret` 1 つに集約した。置くのは `ui/Disclosure.svelte`(開閉する面)と `ui/Popover.svelte` のトリガ(重なるもの)だけ。`.chev`(`›`)はドリルダウンの進行方向の印で別物なので残す。
- **排他は `<details name>`。**「計算の材料」9 枚と、その中のバフの目的グループが 1 つずつしか開かないことは、いままで `openMaterial` / `openBuffPurpose` の 2 つの `$state` と `toggleMaterial` の副作用(バフを閉じたら目的も閉じる)で作っていた。`group` を渡すだけになり、状態も副作用も消えた。
- 呼ぶ側に残した props は 4 つ。`class`(面の見た目)/ `summaryClass`(トリガの見た目)/ `group`(排他のまとまり)/ `open`(初期値・外から開かせる。`bind:open` 可)。**当初の見込みは 1 つだったので、増やした理由を記録する**:
    - `class` / `summaryClass` — Svelte のスコープ付き CSS は**子コンポーネントの中の要素には届かない**(実測: `.reach-fold summary` が「未使用セレクタ」になった)。`<details>` と `<summary>` が部品の中に入った以上、その画面での見た目は class を渡して `:global()` か `app.css` で当てるしかない。段階 2 の `triggerClass` / `panelClass` と同じ線引きで、作法の選択肢ではない。
    - `group` — 排他は「その画面の事実」で、渡すと逆に `$state` が 2 本消える。足して減るほうを採った。
    - `open` — 外から開かせる必要がある面が実際にある(お知らせの最新版だけ開く / テシスコアの補助タイプが使用中なら開いたままにする / 計算タブの `followChange` が変わった内訳を辿って開く)。
- **当初「1 つだけ残す」と見込んでいた「閉じても DOM に残して前回値を覚える」props は要らなかった。**`<details>` は閉じても中身を DOM から外さない(`::details-content` が隠すだけ)ので、既定でそうなる。計算タブの内訳はそのまま前回値を覚えている。
- 消えたもの: `ui/motion.svelte.ts` の `disclosurePane` / `disclosureCaret` / `collapse`(59 行)、`cubicOut` の import、`openMaterial` / `openBuffPurpose` / `openAreas` / `openLowerGrades` の 4 つの `$state` と付随する関数、機械監査 **R15・R17・R20**(20 → 17 本)。
- **機械監査を 3 本減らした。**3 本とも「開閉を画面側でどう書くか」を縛る規則で、部品が開閉を丸ごと持った時点で縛る対象が消えた。R15(生の時間の直書き)は開閉以外にも効く規則だったので、捨てたことで `app.css` の `--dur-*` と `DUR` の突き合わせが機械では見張られなくなる — その 1 点は docs 側(§10「時間の段」)に注記して人の目に戻した。

#### `<details>` に載らなかった 3 箇所(例外)

`<details>` は「`<summary>` の直後に面が来る」構造しか表せない。次の 3 つはトリガと面が別の入れ物にいるので、面に `class="… open-in"` + `hidden={!open}` を直接書く形にした(§10 型 6 のクラスそのもの。`hidden` で消えた面は次に出すとき CSS アニメーションが最初から走る)。

- **計算タブの鎖 → 内訳**(`CalcPage`)。トリガは横並びの 3 つの節、面はその下の全幅 1 枚。
- **「なぜこの数字?」の帯 → 本体**(`CalcPage`)。帯と本体のあいだに、閉じていても見える 1 行と助言のチップが入る。
- **テシスコアの「補助タイプも出す」**(`ThesisCorePane`)。トリガ 1 つが 6 行ぶんの段を同時に出す(1 トリガ 1 面ではない)。

この 3 箇所に `aria-expanded` が 5 つ残る(鎖の節 3 + 帯 1 + テシスコア 1)。**規則で縛らない** — 縛りたくなったらそれは部品側を直す信号。

#### 実機で確かめたこと(WebView2 = Chromium 153、2026-09-17)

- `interpolate-size: allow-keywords` / `::details-content` / `block-size: auto` / `<details name>`(排他)はすべて**使える**。段階 2 の `@position-try` のような不発は 1 つも無かった。
- **`flex` 列の子でも開閉どちらも動く。**旧 `use:collapse` の存在理由(「`slide` は height を動かすので `flex: 1` の子では効かない」)は `::details-content` には当てはまらない。実測: 開き 18→218px / 閉じ 218→18px をそれぞれ 220ms。対人タブの実物でも 311→36px / 221→358px で動いた。
- `prefers-reduced-motion: reduce` で `::details-content` の `transition-duration` が `1e-05s`、キャレットも `1e-05s` になり、**1 フレームで開き切る**(46→212px)。
- `<details>` に `display: contents` を当てても開閉は動く。トリガを既存の行の中に置いたまま、面だけを親の `flex` / `grid` に落とす形(キャラの顔選び・装備の 5 補正・ほかの等級・内訳の子行・構成行・「他 n 件」・「次の候補」)で使った。
- **`display: contents` にしても `::details-content` の箱は 1 枚残る**(`display: block`)。親の `flex` / `grid` の子になるのはその箱なので、全幅に落とす `flex-basis: 100%` / `grid-column: 1 / -1` は**中身ではなく箱に書く**。中身側に書いて 2 回とも壊した(「他 n 件」の一覧が行の右に並ぶ / 「ほかの等級」が 3 列グリッドの 1 セルに縦積みになる)。箱が段を切るので、中身を親と同じ並べ方にしたいときは箱に `display: grid` ごと書き直す。
- `transition:collapse` は `use:collapse` では grep に出ないので、「使用箇所 0」に見えていた。実際は `VersusPage` の 2 箇所で生きていた。

### 段階 3 で拾わずに次へ送ったもの

- `CalcPage` の `targetOpen` / `skillOpen` は、段階 2 で置き換えそこねた**手書きポップオーバー**(透明な `<button class="overlay">` + `.pop pop-in`)。開閉ではないので段階 3 の対象外。段階 2 の積み残しとして扱う。
- `apps/desktop/scripts/shoot.js` が `require` のまま。`apps/desktop/package.json` が `"type": "module"` なので **`node` から直接実行できない**(`.cjs` に置き換えるだけで直る)。
- ドリルダウンの行(装備・アバター・ランダムOP の部位行)は `›` のまま。押すと直下に面が開く点では開閉だが、排他で 1 つだけ開く「行 → 面」の形が 3 画面で揃っていて、`<Disclosure>` にすると見た目が変わる。まとめて別段階で扱う。

### 段階 4: 値を出す部品 `<Num>`(2026-09-17)

- `ui/Num.svelte` を新設し、**値に関する作法をこれ 1 つに集める**。置き換え先は 180 箇所(`use:bump` 99 / `use:flash` のうち値のもの 41 / `use:delta` 18、`class="num"` の手書き込み)。画面側の `<Num>` は 143 箇所。
- 呼ぶ側が決めるのは「何を出すか」だけ。**数値書体と `tabular-nums`(`.num`)/ 未収録の破線 `?` / 値が変わったら跳ねる / いくつ変わったかの差分枠**は部品が持つ。
- **「跳ねさせるか」「跳ねか要約か差分か」を選ばせない。**`motion`(値の元になる数)が渡っていれば増減が言えるので `use:bump`、渡せない値なら書式済みの文字が変わったときに `use:flash` —— 選択肢ではなく、**数があるかどうか**で決まる。段階 2 の `Popover` が「トリガと面の両方を持つ」ことで id を配らせなくなったのと同じ形。
- **`ReadRow` を `<Num>` の上に載せ直した。**段階 4 の勝負どころは「行ならこっち、行でなければこっち」を毎回決めさせないこと。`ReadRow` は**行の形**(ラベル・値の位置・右端の固定幅)だけを持ち、値そのものは `<Num>` に渡す。跳ね・光り・差分枠・未収録の `?` を 2 か所に持たない。
- 呼ぶ側に残した props は 8 つ。`value` / `motion` / `delta` / `tone` / `class` / `deltaClass` / `onDelta` / `title`。**段階 1〜3 より多い。増やした理由を記録する**:
    - `value` / `motion` — これが「何を出すか」そのもの。`value` は書式済みの文字列(`format.ts` を通した結果)で、`null` は未収録。
    - `delta` / `deltaClass` / `onDelta` — 差分枠は「値の隣に置くか」ではなく「**その値に前回との差を出す意味があるか**」で決まる画面の事実(計算タブの鎖では差分から変わった内訳へ飛べる)。
    - `tone` — 緑 / 赤 / ラベンダーは §03 の状態色で、`motion` の符号からは決まらない(ダメージ 1,234 を緑にはしない)。
    - `class` / `title` — 段階 2 の `triggerClass` / 段階 3 の `class` と同じ線引き。その画面での見た目。
- **`value` を省くと値を出さず差分枠だけを置く。**計算タブの鎖は、44px の数値の横に差分を置くと枠に入らないので数値の下の行に出す(2026-09-15 の実機判断)。最初は空文字の値を `display: none` で消す形にしていたが、それは呼ぶ側に `class="delta-only"` という決定を戻すことなので、部品側で「値がここに無い」を表せるようにした。
- 消えたもの: 画面側の `use:bump` / `use:delta` が **0 件**、値に対する `use:flash` が 0 件、機械監査 **R16**(17 → **16 本**)と、R16 のためだけにあったコメントのマスク処理(`_mask` / `HTML_COMMENT` / `BLOCK_COMMENT` / `LINE_COMMENT`、95 行)。

#### `<Num>` に入れないもの

- **数字を含まない文・名前。**`<Num>` は数値書体を必ず付けるので、文(対人の「余裕があります」)・装備名を入れると読みにくくなる。判断は §05 の既存の線引き(数値書体で読めるか)で、新しい決定ではない。**元のコードに `num` クラスが付いていたかを見れば決まる。**
- **値ではなく面・行が変わったことを見せるもの**(26 箇所)。`<div class="brief-card">` ・一覧行が `focusToken` で光る・`<section class="buff-group">` の入れ替え・行の着地の印。Svelte では既存の要素を後から包めないので、**面の印は本質的に component ではなく action** — `use:flash` / `use:swap` / `use:pulse` のままにした。ここを部品にするなら「面が変わった」を 1 つの action に畳む話で、値の話とは別。
- **`num` という CSS クラスそのもの**(106 行)。数値書体を**文字送りのため**に当てている場所(`→` の矢印・式・チップのラベル・`.formula`)は値ではない。`.num` は `.dim` と同じ書体ユーティリティとして残し、値は `<Num>` が出す、という線で分けた。

#### なぜ R16 を消したか

R16 は「動きのクラス名を画面側が書いていないか」を見る規則だった。段階 1〜4 で動きが `ui/` の部品に入った結果、**規則が指す先が部品自身になった** — 2026-09-17 時点の候補 2 件のうち 1 件は `ui/Popover.svelte` の `pop-in`(部品が自分の入場を持っている、正しい姿)で、規則が部品化と衝突していた。ADR-015 冒頭の方針(規則が要る = まだ画面側に決めさせている信号)に従い、規則ではなく規則のほうを捨てた。

#### 実機で確かめたこと(WebView2 = Chromium 153、2026-09-17)

- **主役数字の 3 段は従来どおりの場所に出る。**計算タブの「表記ダメージ(1 発)」`.hero-num` が 44px(`--t-result`)、ホーム「次の目標」`.hero-spot` と対人の命中率 `.rate-num` が 27px(`--t-result-inline`)、計算タブの見出し数字が 19px(`--t-heading`)。書体は `M PLUS 1 Code` + `tabular-nums`。
- **跳ねは 0.3s で戻る。**敵を切り替えて `.hero-num` を 30ms 刻みで見ると、`tw-bump-down` が 154ms〜約 300ms のあいだ乗り、その後クラスが外れる。
- **差分枠は出たまま残り、隣を押さない。**`↓19,731` が出て次に変わるまで消えない。出た瞬間、右の節(`.node.mid` / `.node.rate`)の矩形は 1px も動かない。
- **未収録は `?` のまま。**`value={null}` を一時的に流して確認: 親は `.v`、`border-style: dashed`、8.5px。`0` や空白にならない。
- **動きを消す設定で本当に消える。**`prefers-reduced-motion: reduce` で `tw-bump-up` の `animation-duration` が `0.3s` → `1e-05s` になり、跳ねは 1 フレームで終わる。値そのものは変わって見える。
- **Svelte のスコープ付き CSS が届かない問題は段階 3 と同じ。**`<Num class="hero-num">` に移した時点で `.hero-num { … }` が「未使用セレクタ」になる。祖先を 1 つ前置して `:global()` で包む(`.hero :global(.hero-num)`)。`pages/chars/sources/pane-shared.css` のように**素の CSS として import しているファイルは元からグローバル**なので、この対応が要らない。

### 段階 4 で拾わずに次へ送ったもの

- **計算タブの鎖で、差分枠が出た瞬間に主役数字が 11px 上がる。**`.chain .nsub` は差分が空のとき高さ 0 で、出ると 11px になる。鎖が縦中央ぞろえなので、その分だけ 44px の数値が持ち上がる。コード上のコメントは「空でも行を取り、出た瞬間に下が動かない」と言っているが、実機では**上が動いている**(§00 ③)。段階 4 の前から同じ構造なので今回は触らない。
- `class="num"` の手書き 106 行(値ではなく書体ユーティリティとしての使用)。値かどうかの線引きは上に書いたが、`.num` という名前が「値」と紛らわしいので、書体ユーティリティ側の名前を見直すかは別段階で決める。

### 段階 5: 選ぶ部品 `<Choose>` + `<Chip>` と、Picker の中身(2026-09-17)

#### 借りるか — **借りない。標準がもう持っていたから**

ADR-015 の調達順(標準 → 借りる → 自前)で「借りる」を実際に判断したのはこの段階が初めて。**実機(WebView2 = Chromium 153)で標準の到達点を測ってから決めた**:

| 機能 | 実測 |
|---|---|
| `appearance: base-select`(口と `::picker(select)` の両方) | 使える |
| `<selectedcontent>`(`HTMLSelectedContentElement`) | 使える。**選んだ行の `<img>`(アイコン)ごと複製する** |
| `<select>` 直下の author `<button>` | 使える |
| `<option>` に `display: flex` | 効く(アイコン + 名前 + 値の行が組める) |
| `<select>` 直下の非 `<option>` 要素(`note` の 1 行) | 候補面の中に描かれる |
| `hidden` + `disabled` な `<option>` を `select.value` にする | **できる**。候補面には並ばず(高さ 0)、口の文字だけを持つ |
| `appearance: base`(checkbox / radio) | **使えない** → input は隠して `<label>` に着せる従来手 |

借りる候補は 3 つとも実データを取った(npm、2026-09-17):

| 候補 | 版 | 実行時依存 | unpacked |
|---|---|---|---|
| `bits-ui` | 2.19.2 | runed / esm-env / tabbable / svelte-toolbelt / **@floating-ui/dom + core** | 2.02 MB |
| `melt`(Melt UI next) | 0.44.0 | **@floating-ui/dom** / dequal / focus-trap / **jest-axe** / runed | 387 KB |
| `@zag-js/combobox` + `@zag-js/svelte` | 1.44.0 | core / types / utils / **popper** / anatomy / dom-query / collection / dismissable / live-region / focus-visible | 186 KB |

- 消える自前コードは **Picker のキーボード ~150 行だけ**。`appearance: base-select` を使えばその 150 行も**最初から書かずに済む**ので、借りて消える行は実質 0。
- 増えるものは大きい。**3 つとも自前の位置決め(floating-ui / zag の popper)と自前の light-dismiss を抱えている** — 段階 2 でちょうどそれ(`ui/popover.ts` 84 行)を捨てて `popover` 属性 + CSS Anchor Positioning に置き換えたばかりなので、借りると**捨てた 84 行を依存として買い戻す**ことになる。`melt` は実行時依存に `jest-axe` が入っていて 1.0 前。
- **指標の上でも動かない。**借りても呼ぶ側が決める数は変わらない(どちらの場合も決めるのは `ui/` の中)。借りる / 借りないは「自前が軽いか」ではなく、**標準より下に降りる理由があるか**で決まる。今回は無かった。
- ただし `@melt-ui/svelte` / `bits-ui` 相当が要る場面はいずれ来る(日付選択・仮想化された長い一覧など)。そのときは**この表をもう一度取り直す**。「一度借りないと決めた」を理由に自前で組まない。

#### `<Chip>` と `<Choose>`

- `ui/Chip.svelte` を新設。**アプリ中の小さな的はこれ 1 つ**(置き換え 35 箇所 / 11 ファイル)。何になるかは `on` が渡っているかで決まる —— 渡さなければ `<button>`(押すと何かが起きる)、渡せば `<input type=checkbox>` + `<label>`(なっている)、`name` も渡れば `<input type=radio>`(並んだ中の 1 つ。`Choose` だけが渡す)。**見た目の選択肢ではなく、状態があるかどうかで決まる** — 段階 4 の `<Num motion>` と同じ線。
- `ui/Choose.svelte` を新設。**段(`.seg`)・粒(`.chiprow`)・タブ(`.tabs`)はこれ 1 つ**。中身は `Chip`(`ReadRow` を `Num` の上に載せ直したのと同じ形)。`StepSelect`(29 箇所)/ `StepToggle`(2 箇所)を吸収して**両方削除**し、手書きタブ(App.svelte / BuffsPage ×2 / CalcPage)も載せた。
- **チップとタブを別の部品にしない。**BuffsPage の「タブ」は置き換え前から `class="chip category-tab"` と書かれていた — 実装がすでに「同じもの」だと言っていた。分けると呼ぶ側に「チップならこっち、タブならこっち」を毎回決めさせる。
- **1 つ選ぶか、いくつか選ぶかも決めさせない。**`value`(1 つ)を渡せば radio 群、`values`(いくつか)を渡せば checkbox 群。持っているものが決める。
- **キーボードは 1 行も書いていない。**同じ `name` の radio が、矢印キーでの移動・Tab ストップが群れで 1 つになること・読み上げの「n 個中 m 個目」を既定で持つ。置き換え前は `role="radio"` / `aria-pressed` の自作ボタンで、**矢印キーはどこにも無かった**。
- 呼ぶ側に残した props は `Chip` が 8(`on` / `name` / `value` / `class` / `disabled` / `title` / `onToggle` / `onclick`)、`Choose` が 13。**段階 1〜4 より多い。増えたのは 2 つの部品(`StepSelect` + `StepToggle`)を畳んだからで、`full` / `cols` / `cell` / `tone` / `disabledValues` / `titleFor` / `max` は畳む前からある「その画面の事実」をそのまま持ってきたもの**。作法の選択肢は 1 つも増えていない。`item`(段の中身を自分で描く)だけが新規で、件数の `<Num>` を添えるバフタブのために要った。

#### Picker の中身を customizable `<select>` にした

- 口(`<button><selectedcontent></selectedcontent></button>`)も候補面(`::picker(select)`)も `<select>` の中。**矢印キー・タイプアヘッド・Enter・Escape とフォーカスの戻り・トップレイヤー・口への位置合わせ**をブラウザが持つ。
- **チップ側を選んでいるときの「ほか n 件」は `hidden` + `disabled` な `<option>` 1 枚で表した。**候補面には並ばず(高さ 0)、`select.value` にはできるので `<selectedcontent>` がその文字を出す。置き換え前の口の出し分けがそのまま残り、見た目は変わっていない。
- **UA が足す `▼` は使わない。**`.picker-select::picker-icon { display: none }`(`::picker-icon` は `<button>` ではなく `<select>` に付く。実機で 1 度間違えて、口の枠の外に ▼ が出た)。キャレットは `app.css` の `.caret` 1 つのまま(§10)。
- 消えたもの: `ui/Popover.svelte` への依存(Picker だけ外れた。他 3 箇所は据え置き)、`.picker-overlay` 相当の出し分け、`.picker-trigger` の `aria-expanded` セレクタ。

#### 実機で確かめたこと(WebView2 = Chromium 153、2026-09-17)

**キーボードだけで一周した。**マウスは最初にタブを 1 回押すだけ。

- **上部タブ**: `Tab` 1 回で群れに入る → `ArrowRight` で ホーム → ダメージ計算 → バフ、`ArrowLeft` で戻る。**押すたびに表示中の面も変わる**(radio の既定どおり自動で選ばれる)。
- **段(`.seg`)**: `role="radiogroup"`、`ArrowRight` / `ArrowLeft` で「一覧から選ぶ」↔「一覧に無い敵」を往復。**`Tab` を 1 回押すと群れの外へ出る**(Tab ストップが群れで 1 つ)。群れの矩形は 86,144 676x29 のまま 1px も動かない(§09 規則 1)。
- **Picker**: `ArrowDown` で候補面が開きフォーカスが `<option>` に入る → `Enter` で決まり(`ringo` → `abyss_hell`、口の文字も変わる)フォーカスが `<select>` に戻る → 再度 `ArrowDown` で開き `Escape` で閉じてフォーカスが戻る。**口の矩形は開いても閉じても 86,202 662x34 のまま**。候補面は口の左端に揃って下に開き(x=87 / y=241、口は y=202 高さ 34)、幅は `anchor-size(width)` で口に揃う。
- **単独のチップ**: `Space` で ON / OFF が入れ替わる(`4 / 5 だけ` ↔ `それ以外`)。
- **動きを消す設定**: `prefers-reduced-motion: reduce` で `::picker(select)` の `animation-duration` が `0.17s` → `1e-05s`。**最初は消えていなかった** — `::picker(select)` は `*::before, *::after` では拾えない名前付き擬似要素で、段階 3 の `::details-content` とまったく同じ穴だった。`app.css` の reduce ブロックに明示して足した。§10 の「新しい動きを足したら実機で本当に消えるか確かめる」がそのまま効いた 2 例目。

#### 数字

- 画面側の手書き `<button class="chip">` **35 → 0**。`role="tab"` の直書き **7 → 0**。画面側の `role="radio"` / `role="radiogroup"` の直書きも **0**(素の radio になったので書く必要が無い)。
- 部品 **6 → 6**(`StepSelect` / `StepToggle` を削除し、`Choose` / `Chip` を新設)。
- 機械監査 **16 本のまま**(増やさない。減らせる規則は今回は無かった)。候補は 1 件で、段階 4 から変わらず `ui/Spinner.svelte` の 0.7s。
- `aria-expanded` の直書き **5 → 4**。テシスコアの「補助タイプも出す」が checkbox になり、`checked` が同じことを言うようになったので外した。

#### 段階 5 で拾わずに次へ送ったもの

- `apps/desktop/scripts/shoot.js` の `require`。タブの selector(`nav.tabs button` → `.tabs label.chip`)だけは今回の変更の後始末として直したが、**`.cjs` への置き換えは段階 6 のまま**(このセッションでは一時コピーを作って撮った)。
- `ui/Disclosure.svelte` に `summaryClass="chip quiet"` を渡している箇所。トリガが `<summary>` なので `Chip` には載らない(段階 3 の「部品に載らないものは素で書く」と同じ)。
- `.chip-diff` / `.chip-x`(計算タブの差分チップ)。名前は chip だが押して選ぶものではなく、印 + 取り消しボタン。
- `ui/StatInput.svelte` の「理由チップ」は押せないので `.chip` を外して `.why` にした。**残りの `class="num"` 106 行**(段階 4 から)と同じく、名前が実体とずれているものの棚卸しは別段階。

## 却下した選択肢

- **(段階 3)`aria-expanded` を `<summary>` にも書いて揃える案** — `<summary>` は暗黙のロールで開閉状態を持っているので、書くと二重になる。標準が持っているものを手で足さない。
- **(段階 3)開閉の動きを `svelte/transition` で持つ案** — `::details-content` が flex 列の子でも開き・閉じの両方を動かすことを実機で確かめた時点で、JS の transition を挟む理由が消えた。`prefers-reduced-motion` の扱いも CSS 側の 1 か所に寄る。
- **(段階 3)トリガと面が離れた 3 箇所のために、`Disclosure` に「面だけ」モードを足す案** — props が 1 つ増えるうえ、「どっちのモードか」を呼ぶ側に決めさせることになる。数が 3 つなので、部品に載らないものは部品を使わず素で書くほうを選んだ。載るものが増えたら考え直す。
- **(段階 3)ドリルダウンの行(装備・アバター・ランダムOP の部位行)も `<Disclosure group>` に寄せる案** — 見た目(`›` の進行方向の印)と挙動(押すと右ではなく直下に開く)が 3 画面で揃っているので、まとめて別段階で扱う。段階 3 に混ぜると、開閉の話と一覧の作法の話が同じ commit に入る。

### 却下(段階 4)

- **`<Num>` に「数値書体を付けるか」の props を足す案** — 文と数のどちらも受けられるようにするには 1 つ増える。数が渡っているか(`motion`)で決める案も試したが、`use:flash` だけの数値(倍率・カテゴリ値)で書体が落ちるので使えない。**`<Num>` は数の器と割り切り、文は入れない**ほうを採った。呼ぶ側の決定は増えない(「数か文か」は出すものそのものの性質)。
- **面・行が変わったことを見せる `use:flash` も部品に畳む案** — Svelte では既存の要素を後から包めないので、部品にすると DOM が 1 枚増えるか、呼ぶ側に「行ならこれ、面ならこれ」を毎回決めさせることになる。段階 4 の勝負どころ(部品を 2 つにしない)を自分から崩す。面の印は action のまま残し、畳むなら「面が変わった」を 1 つの action にする別の話として扱う。
- **`class="num"` の手書きを全部 `<Num>` にする案** — 106 行のうち多くは値ではなく、数値書体を文字送りのために当てている場所(矢印・式・チップのラベル)。値でないものを `<Num>` に入れると、変わってもいない文字が光る。

### 却下(段階 5)

- **ヘッドレス部品を借りる案(bits-ui / melt / zag)** — 上の表のとおり。3 つとも floating-ui / popper を抱えていて、段階 2 で捨てた自前の位置決めを依存として買い戻すことになる。消える自前コードは `appearance: base-select` を使えば**そもそも書かない** 150 行だけ。
- **`Choose` に `look: "seg" | "chip" | "tab"` を持たせる案** — 3 つから選ばせた時点で「決める数 0」が崩れる。見た目は `class` で渡す(段階 2 の `triggerClass` / 段階 3 の `class` / 段階 4 の `class` と同じ線)。app.css が `.seg` / `.chiprow` / `.tabs` を既に持っているので、呼ぶ側が新しい語彙を作ることもない。
- **チップ(選ぶ)とチップ(する)を 1 つの props で区別しない案** — 「押すと何かが起きる的」と「なっている的」は DOM からして別物(`<button>` と `<input>`)で、どちらかは**出すものの性質**で決まる。`on` を渡すかどうかで分けたのは、段階 4 で `<Num>` が `motion` の有無で跳ねと光りを分けたのと同じ。
- **タブを `role="tablist"` / `role="tab"` / `aria-controls` で組み直す案** — 揃えるには矢印キーと `aria-selected` を自分で書くことになる。素の radio 群は同じ操作感を**書かずに**持つので、標準に寄せるほうを採った。面との結び付き(`aria-controls`)は失うが、置き換え前も 7 箇所中どこにも無かった。
- **`Choose` の `label` を「読み上げ名だけ」にもできるようにする案** — props が 1 つ増える。`label` は今までどおり見える見出し兼読み上げ名にし、見出しを出したくない 2 箇所(バフタブの 2 つの切り替え)は `aria-label` を持たないままにした。置き換え前の `role="tablist" aria-label` は失うが、段は 1 つずつが本物の radio なので中身は読み上げられる。
- **`.chiprow` を `.chips` という名前にする案** — `BuffsPage` と `CalcPage` が**別の意味の `.chips`**(行チップ = `ToggleRow` の一覧 / 差分チップの帯)をスコープ付きで既に持っている。グローバルに同じ名前を足すと、後から片方を `:global` にした瞬間に壊れる。

### 却下(段階 1・2)


- **Modal がヘッダ(題名 + 閉じるボタン)も持つ案** — 4 箇所のヘッダは見た目が揃っていない(`.panel-header` と sticky な `.part-detail-header`)。揃えるのは見た目の決定で、段階 1 の「挙動を標準に寄せる」と混ざる。段階 6(チップ・タブ)の design-review で扱う。
- **幅の段(`wide` / 既定)を props で持たせる案** — `<dialog>` を暗幕の位置に置けば、幅は中身側の既存 CSS(`.panel` 560px / `.part-detail` 980px)がそのまま決める。呼ぶ側に新しい決定を増やさずに済んだ。
- **Escape を `oncancel` で常に `preventDefault` して自前で閉じる案** — 既定の cancel → close → `onclose` に任せれば配線が 1 本で済む。`closeDisabled` のときだけ `preventDefault` する。
- **(段階 2)`Popover` が「閉じる」ボタンも持つ案** — 3 箇所は持っていて `Picker` は持っていない。「閉じる」は中身であって作法ではない(`Modal` と同じ線引き)ので、閉じる関数だけ中身のスニペットに渡した。
- **(段階 2)面を出す `anchor-name` を行(`.togrow`)に付けて、今までどおり行幅で開く案** — 「どの要素にぶら下げるか」を呼ぶ側の決定として戻すことになる。トリガを部品が持てば決める必要がなくなるので、幅のほうを譲った。

## 経緯

- 置き換え前、リポ全体で `.focus()` が 0 件だった。つまり**フォーカスの罠も復帰もどこにも無かった**。`showModal()` に寄せた時点で、書かずに両方入った。
- `<dialog>` に `display: flex` を当てると UA の `dialog:not([open]) { display: none }` を打ち消すので、`.modal-root:not([open]) { display: none }` を自分で書く必要がある(開く前の 1 フレームで出てしまう)。
- 実機で Tab を 40 回送ると、一周する折り返しで `document.activeElement` が 4 回 `body` を経由した。背面の操作要素には一度も止まらない(背面は `inert`)ので、罠としては効いている。
