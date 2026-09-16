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

## 却下した選択肢

- **Modal がヘッダ(題名 + 閉じるボタン)も持つ案** — 4 箇所のヘッダは見た目が揃っていない(`.panel-header` と sticky な `.part-detail-header`)。揃えるのは見た目の決定で、段階 1 の「挙動を標準に寄せる」と混ざる。段階 6(チップ・タブ)の design-review で扱う。
- **幅の段(`wide` / 既定)を props で持たせる案** — `<dialog>` を暗幕の位置に置けば、幅は中身側の既存 CSS(`.panel` 560px / `.part-detail` 980px)がそのまま決める。呼ぶ側に新しい決定を増やさずに済んだ。
- **Escape を `oncancel` で常に `preventDefault` して自前で閉じる案** — 既定の cancel → close → `onclose` に任せれば配線が 1 本で済む。`closeDisabled` のときだけ `preventDefault` する。
- **(段階 2)`Popover` が「閉じる」ボタンも持つ案** — 3 箇所は持っていて `Picker` は持っていない。「閉じる」は中身であって作法ではない(`Modal` と同じ線引き)ので、閉じる関数だけ中身のスニペットに渡した。
- **(段階 2)面を出す `anchor-name` を行(`.togrow`)に付けて、今までどおり行幅で開く案** — 「どの要素にぶら下げるか」を呼ぶ側の決定として戻すことになる。トリガを部品が持てば決める必要がなくなるので、幅のほうを譲った。

## 経緯

- 置き換え前、リポ全体で `.focus()` が 0 件だった。つまり**フォーカスの罠も復帰もどこにも無かった**。`showModal()` に寄せた時点で、書かずに両方入った。
- `<dialog>` に `display: flex` を当てると UA の `dialog:not([open]) { display: none }` を打ち消すので、`.modal-root:not([open]) { display: none }` を自分で書く必要がある(開く前の 1 フレームで出てしまう)。
- 実機で Tab を 40 回送ると、一周する折り返しで `document.activeElement` が 4 回 `body` を経由した。背面の操作要素には一度も止まらない(背面は `inert`)ので、罠としては効いている。
