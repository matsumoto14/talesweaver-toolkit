# 旧リポジトリ (C:\github\private\twtoolkit) の棚卸し

棚卸し日: 2026-08-21(旧リポ最終コミット 2026-07-24)。

旧リポは **コミュニティ製 Excel「ダメージ計算器【v4.00】.xlsx」を唯一のオラクルとして TypeScript(Next.js)に移植した計算エンジン**。
語彙は Excel セル名(AF54, M54 等)だが、式の骨格は docs/damage-formula.md のカテゴリモデル(A〜Y)とほぼ同一だった。

**扱いの原則: 構造とデータの器は旧リポから、数値の正解は現 wiki から。Excel は実測代わりの「第2の参照値」として保管。**

## ⭐ そのまま流用できる資産(パスは旧リポ基準)

### 静的ゲームデータ(JSON)

| データ | パス | 内容 |
|---|---|---|
| モンスター | `packages/damagecalc/src/native/rules/v0_1/generated/monsters.json` | 44体(実データ28)。hp / defense(=C) / threshold(=I の閾値) / af63(=M) / af64(=V1) |
| スキル係数 | 同 `coefficients.json` | 6タイプ別のステ→攻撃力係数(STAB: 23.75/3.75 等)。wiki の Skill#formula 相当 |
| 素ステ係数 | 同 `rawStatCoefficients.json` | 6タイプ |
| 覚醒倍率(=N) | 同 `awakening.json` | stage0-5 + エタLv 0-80 で 1.00〜2.49 |
| ダメージ上限 | 同 `damageCap.json` | エタLv 連動で 300万〜1800万 |
| キャラバフ表 | 同 `attackCharacterBuffs.json` | 22キャラ × ダメバフ/デバフ/追撃/ステデバフ |
| 防御キャップ等 | `.../defense/rules/v0_1/generated/*.json` | buffCaps / PA・CA テーブル / エタ防御上限 |
| **スキル 373件** | `.../native/skills/generated/<キャラ>.json` ×19 | name / skillType / multiplier(=D) / stages(=段数) / critMultiplier(=F) / delay / icon |
| バフカタログ | `apps/next-web/features/calculator/types.ts` | 約60バフの実名 + デフォルト% + カテゴリ上限。バフ静的データのシード |
| 画像 | `apps/next-web/public/icons/skills/**`, `public/monsters/**` | スキル・モンスターアイコン |

### 仕様ドキュメント

- `packages/damagecalc/src/native/SPEC_CELL_MAP.md`(567行) — Excel セル ↔ 式の対応表。Rust 移植時の最良の仕様書
- `.agents/skills/damage-calc-knowledge/SKILL.md` + `references/{buff-system,monster-params}.md` — ドメイン要約。28体パラメータ整形済み表

### 手法

- `scripts/scrapeSkills.js` / `download-monster-icons.mjs` — talewiki の EUC-JP URL エンコードを含むスクレイピング実装(Playwright)。手法だけ移植価値あり

## 🟡 参考程度(翻訳して使う)

- `core/displayDamage.ts` の式の順序・丸め位置 — カテゴリ体系に翻訳し直す。旧実装には `{}`(小数2位切捨)と `MAX(…,K)` が**無い**ので追加必須
- `attackBuffs.ts` の集計パターン(上限付き加算 / 無制限加算 / 乗算 `(1+a)(1+b)−1` / キャラ別参照)— Rust の enum で再設計する土台
- 防御側実装(`fParameter.ts` の `r = a/(a+80)` は wiki と完全一致 → 高信頼)
- UI の画面構成・入力項目(3カラム: キャラ / スキル・バフ / 結果。スキル選択で倍率・段数を自動投入 — 入力最小化方針と同方向)。URL 状態化(`lib/urlState.ts`)とプリセット保存の設計
- 運用ルール: トレースファースト(中間値まで突き合わせる)、ルールスナップショットの凍結

## 🔴 使わない

- `dist/`(stale なビルド跡)、`docs/CALC_LOGIC_ROADMAP.md` の進捗表(実装と乖離。ただし §7.2 未移行ロジック棚卸しは読む価値あり)
- `tests/vectors/*.json` — Excel 前提かつダメージキャップ飽和ケース多数で検証力が低い
- Next.js/React 実装コード全般 — 画面設計だけ引き継ぐ
- `damage-calc-knowledge-workspace/` — 未完了のスキル評価実験

## 現 wiki との食い違い(旧リポの数値を信じない箇所)

| 項目 | 旧(Excel v4.00) | 現 wiki (2026-07) |
|---|---|---|
| X1 イザベル上限 | 40% | **50%** |
| L 最終ダメージ上限 | 20% | **45%** |
| P 特定依存上限 | 上限なし | **+73%** |
| B 攻撃力乱数 | `INT((ステ攻+DEX*3)/18)` | `{(ステ攻+DEX*3)/18}+1`(2位切捨+1) |

ゲーム側のアップデートで変わった可能性が高い。データ取り込み時は wiki 側の値で上書き検証する。

## 旧リポの教訓(新リポで必ずやること)

1. **実測ダメージとの突き合わせ機能を作る。** 旧リポ最大の反省点: Excel 再現率100%だが、ゲーム内実測との比較は一件も無かった。wiki も「計算順序未確定」と自認しており、実測が最終の正解
2. **丸めを仕様として先に固定する。** `[]`=floor、`{}`=小数2位切捨、割合バフごと floor。旧リポはここが未着手のまま終わった
3. **トレースファースト。** 最終値だけの一致は偶然一致を見逃す(旧リポの「AF65事件」: 属性倍率の式が間違っていたが最終値が偶然一致)。中間値を全段トレースして比較する
4. **能力値計算(素ステ→最終能力値)を実装する。** 旧リポは丸ごと欠落し、画面表示値を手入力させていた。入力最小化方針に反するので新リポでは §2 を最初から実装
5. **配線漏れに注意。** 旧リポにはコンボ補正(H)を集計だけして式に繋げ忘れたバグがある。カテゴリ網羅テストで防ぐ
