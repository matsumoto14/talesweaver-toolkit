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

## 却下した選択肢

- **(段階 3)`aria-expanded` を `<summary>` にも書いて揃える案** — `<summary>` は暗黙のロールで開閉状態を持っているので、書くと二重になる。標準が持っているものを手で足さない。
- **(段階 3)開閉の動きを `svelte/transition` で持つ案** — `::details-content` が flex 列の子でも開き・閉じの両方を動かすことを実機で確かめた時点で、JS の transition を挟む理由が消えた。`prefers-reduced-motion` の扱いも CSS 側の 1 か所に寄る。
- **(段階 3)トリガと面が離れた 3 箇所のために、`Disclosure` に「面だけ」モードを足す案** — props が 1 つ増えるうえ、「どっちのモードか」を呼ぶ側に決めさせることになる。数が 3 つなので、部品に載らないものは部品を使わず素で書くほうを選んだ。載るものが増えたら考え直す。
- **(段階 3)ドリルダウンの行(装備・アバター・ランダムOP の部位行)も `<Disclosure group>` に寄せる案** — 見た目(`›` の進行方向の印)と挙動(押すと右ではなく直下に開く)が 3 画面で揃っているので、まとめて別段階で扱う。段階 3 に混ぜると、開閉の話と一覧の作法の話が同じ commit に入る。

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
