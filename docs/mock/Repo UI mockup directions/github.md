repo: matsumoto14/talesweaver-toolkit
branch: main

## Last sync

date: 2026-08-23T08:45:00Z

### Updated in this project

- TW Toolkit Prototype v3.dc.html — スキル選択を追加。skills.rs のボリス5件（倍率D・段数・Cri倍率F・依存種別）と characters.rs の依存別装備係数を反映。ピッカーは選択中の相手に対する各スキルの合計ダメージで並ぶ
- 表示は「1段」を主・「合計 ×N段」「クリティカル ×F」を副に。damage.rs の total = per_hit × hit_count / 非クリ F=1.0 に準拠
- ダメージ計算タブを全面的に組み替え。「相手を選ぶ → この一発 → もし〜だったら → ダメージの通り道（3段の分解）」の縦フローに
- 通り道は damage-formula.md の式を①攻撃力をつくる ②防御力を抜く ③倍率で伸ばす の3段に要約。③は各倍率が「足したダメージ」の帯幅で寄与を可視化
- 右カラムを「試し変更の操作盤」から「計算の材料」（常用バフ・倍率の材料・入場条件）に変更。試し変更は入っているときだけ細い帯で出す
- 目標コンテンツは ◀▶ で1件ずつ送れるプレートに（一覧セレクトは併置）

## Sync history

- 2026-08-22T17:50:07Z — v2 でホーム・ダメージ計算・キャラ登録を全部入れの動く形に

### Updated then

- TW Toolkit Prototype v2.dc.html — ホーム（到達一覧）・ダメージ計算・キャラ登録＋装備のドリルダウン
- キャラ登録を「呼び名＋職のアイコン選択」に変更（gamedata のプレイヤブル19キャラ）


- 2026-08-22T16:10:49Z — 9a/9b/9c（装備グループの入力形式案）、Prototype の誤った装備表現を巻き戻し

### Updated before

- 9a/9b 装備グループの入力形式2案（一式テンプレ主役＋8値微調整 / ゲーム内ステータスの写し取り主役）。装備モデルは基本4値・強化4値＋PW/SW に統一
- 9c 到達一覧に装備は「合計値1行」だけ出す案（編集はキャラ設定へ送る）
- Prototype の7部位×4段階という誤った装備表現を撤回し、直前の状態に巻き戻し

## Screen map

| 画面 | 参照した repo ファイル |
|---|---|
| TW Toolkit Prototype v3.dc.html — ダメージ計算タブ（通り道／もし〜だったら／計算の材料） | docs/damage-formula.md §4 A・X / §5 倍率, apps/desktop/src/pages/damage/DamagePage.svelte, docs/screenshots/26-damage.png, docs/screenshots/37-damage-trace.png |
| TW Toolkit Prototype v3.dc.html — スキル選択 | crates/gamedata/src/skills.rs, crates/gamedata/src/characters.rs（equipment_coefficients / SkillDependency）, crates/domain/src/damage.rs（段数・Cri倍率・対モンスター下限）, docs/damage-formula.md §3・§5 |
| TW Toolkit Prototype v2.dc.html — キャラ登録の職アイコン | crates/gamedata/src/characters.rs（プレイアブル19キャラの id / 表示名） |
| TW Toolkit Prototype v2.dc.html — 装備・強化能力値の扱い | docs/claude/decisions.md（2026-08-22 装備攻撃力）, docs/damage-formula.md §4 A |
| TW Toolkit Redesign.dc.html — 装備グループ 9a/9b/9c | docs/claude/goals/2026-08-22-equipment-attack.md, docs/claude/decisions.md（2026-08-22 装備攻撃力 #1〜#9）, docs/damage-formula.md §4 A, docs/ux-guidelines.md |
| TW Toolkit Redesign.dc.html — ダメージ計算 1a/1b/1c | apps/desktop/src/pages/damage/DamagePage.svelte, apps/desktop/src/App.svelte, apps/desktop/src/app.css, apps/desktop/src/ui/Select.svelte, apps/desktop/src/ui/StatInput.svelte, docs/ux-guidelines.md, docs/screenshots/26-damage.png |
| TW Toolkit Redesign.dc.html — 規格シート 2a / v2 3a | apps/desktop/src/app.css, apps/desktop/src/ui/Select.svelte, apps/desktop/src/ui/StatInput.svelte |
| TW Toolkit Redesign.dc.html — キャラ設定 2b/2c | apps/desktop/src/pages/character/CharacterSettings.svelte, apps/desktop/src/ui/AdjustmentEditor.svelte, apps/desktop/src/labels.ts, docs/screenshots/20-workspace.png |
| TW Toolkit Redesign.dc.html — キャラ設定合体版 4a | apps/desktop/src/pages/character/CharacterSettings.svelte, docs/damage-formula.md（§4 A/X/L/P/O/N/H の上限） |
| TW Toolkit Redesign.dc.html — 5a/5b/5c | docs/damage-formula.md（§4 A・X / §6 防御力 / §7 回避）, crates/gamedata/src/buffs.rs（バフ実名）, crates/gamedata/src/skills.rs, crates/gamedata/src/enemies.rs |
| TW Toolkit Redesign.dc.html — 比較タブ 2d | apps/desktop/src/pages/damage/DamagePage.svelte, docs/status.md |
