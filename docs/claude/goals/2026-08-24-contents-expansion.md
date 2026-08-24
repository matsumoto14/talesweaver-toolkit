# Goal: コンテンツカタログ拡充(到達一覧を実データ化)

状態: **未着手(2026-08-24 方針決定)**

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
