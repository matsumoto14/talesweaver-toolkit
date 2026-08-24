# Goal: コンテンツカタログ拡充(到達一覧を実データ化)

状態: **完了(2026-08-24)**

## 方針(ユーザー決定 2026-08-24)

wiki 取り込みをベースにする。ただし Mob データが wiki に無い可能性があるため、**旧リポ(C:\github\private\twtoolkit)のモンスターデータをシードにする**。

## データソース

- 旧リポ `packages/damagecalc/src/native/rules/v0_1/generated/monsters.json`: 44 体(実データ 28)。hp / defense(C)/ threshold(I 閾値)/ af63(M)/ af64(V1)— 現行 gamedata/enemies.rs と同じ写像で取り込める
- 旧リポ `.agents/skills/damage-calc-knowledge/references/monster-params.md`: 28 体の整形済みパラメータ表
- wiki「狩り場情報一覧」(未取込、docs/damage-formula.md §9): 敵の防御力・カット率・必中命中P など。取れた値は wiki を正として上書き
- 目安ダメージ(need_per_hit)と入場条件はコミュニティ知識・実測。wiki に無ければ `[仮]` のまま実測で更新

## やること(案)

1. 旧リポ monsters.json 28 体を gamedata/enemies.rs へ転記(出典: 旧リポ、`[仮]`)
2. wiki 狩り場情報一覧を取込み、重複分を wiki 値で検証・上書き
3. contents.rs をエリア別に拡充(敵→コンテンツの対応、目安・入場条件)
4. モンスターアイコン(旧リポ public/monsters/)の同梱は任意(UI 側は無くても動く)

## 実施結果(2026-08-24)

- enemies.rs: 旧リポ 28 体を転記+ユーザー提供の実測表「モンスター能力値リスト」から 14 体(聖域10〜19・ゆがんだ村4)を追加(計 42 体)。wiki にしか無い 5 体(被害減少が不明)は収録しない(ユーザー決定)。3 ソースの重複分は一致し、被害減少の実値(2925/3250/4550/5850)も実測表で確定(`[仮]` 解消)。差分は最後の決戦2 のカット率のみ(実測表 70% を採用)
- contents.rs: swiki「コンテンツ入場条件」(<https://erumisutoburvip.swiki.jp/>、取得 2026-08-24)を正として 4 エリア 59 件に再構成。入場条件(覚醒段階・エタ Lv・装備補正)を実データ化。目安ダメージは `[仮]` のまま
- 装備条件は `EquipmentBySkill { single, mr, composite }` の 1 件で持ち、判定時に使うスキルの依存種別で比較先を選ぶ(Stab/Hack/Int → single、Mr → mr、StabHack/HackInt → composite)
- 敵データが無いコンテンツは `enemy_id`/`need_per_hit` が None で入場条件のみ判定。判定できない条件(ルーン Lv・共通スキルコンプ・コア・カフス上限・前提クリア)は `entry_note` に表示専用で保持
- モンスターアイコン同梱は見送り(任意項目)
- 詳細は docs/claude/decisions.md「2026-08-24 コンテンツカタログ拡充」
