# アイコン画像

`ui/Icon.svelte` が **gamedata の id から機械的に解決**する。手動のマッピング表は作らない。

| 系統 | ディレクトリ | ファイル名 | id の出どころ |
|---|---|---|---|
| キャラ | `characters/` | `<id>.png` | `gamedata::characters()` の `GameCharacter::id`(例 `boris.png`) |
| Mob | `mobs/` | `<id>.png` | `gamedata::enemies()` の `Enemy::id` |
| スキル | `skills/` | `<id>.png` | `gamedata::skills_for()` の `Skill::id`(例 `boris_continuous.png`) |
| バフ | `buffs/` | `<id>.png` | `gamedata::buff_catalog()` の `BuffDefinition::id` |
| マスタリー | `masteries/` | `<id>.png` | `gamedata::mastery_catalog()` の `MasteryDef::id`(例 `boris_m1_issen.png`)。枠はスキルと同じ |
| 装備 | `equipment/` | `<id>.png` | `gamedata::equipment_catalog()` の `EquipmentItem::id` |

置くだけで反映される(Vite の glob import)。**無い id は破線 + `?`** で表示され、
console に 1 行だけ warn が出る。サイズは `Icon.svelte` 側で固定なのでレイアウトは崩れない。

正方形推奨。表示サイズは 20 / 28 / 40 / 64 の 4 段なので、128px 程度で用意すれば足りる。

wiki の画像は `?plugin=ref&page=<ページ名(EUC-JP の URL エンコード)>&src=<ファイル名>` で取れる
(例: `Skill/ボリス` の `Mastary2_1.png`)。**ページごとの添付**なので、キャラが違えば同じ
ファイル名でも別の絵になる。

2026-08-27の上位装備カタログでは、生成594件のうち装備行に画像参照がある309件を同梱した。
参照のない285件（ライジングホリックカフスを含む）は `?` 表示のままにする。

2026-08-27に通常スキル303件とキャラスキル71件を Tale Wiki 各 `Skill/<キャラ名>` ページの
表示名と機械照合し、全件のアイコンを同梱した。3件は両カタログで同じidを共有するため、
`skills/` は19キャラ合計371枚。再取込は `tools/gamedata/import_skill_icons.py` を使う。
