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
| コンテンツ | `contents/` | `<id>.png` | `gamedata::content_areas()` の `Content::id`(例 `clamor.png`) |

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

2026-08-28に AF カタログ45件を Tale Wiki の
`Item/アクセサリー用装備/アーティファクト` と照合し、取得できた同名添付35件を
`equipment/` に取り込んだ。Wiki上で添付が無い、または参照だけが残って実体が無い10件は
別装備の画像を流用せず `?` のままにする。再取込は
`tools/gamedata/import_artifact_icons.py` を使う。

2026-08-29にバフ34件を Tale Wiki の `ステータス` 表と照合し、行に直接アイコン参照がある
28件を `buffs/` に取り込んだ。専用画像を確認できない6件は、似た効果の画像を流用せず
`?` のままにする。再取込は `tools/gamedata/import_buff_icons.py` を使う。

2026-08-31に極限スキル3件(`scope_eye` / `full_throttle` / `wide_focus`)を Tale Wiki の
`Skill/ゲージスキル` ページの添付から同梱した(`?plugin=ref` 経由)。極限は `Skill/極限` ページに
表があるが、画像の実体は `Skill/ゲージスキル` に添付されている。

2026-09-01にコンテンツ画像19枚を `contents/` に同梱した。出典は wiki ではなく**ゲーム内
「Content information → コンテンツクリア状況」のスクリーンショット**で、あの一覧は
1 行 = 1 コンテンツ・行頭に専用の絵が付く(wiki の「ミニゲーム/*」にはマップとドロップ品しか
無く、コンテンツ単位の絵が無い)。行の名前とツールのコンテンツを 1 対 1 で言い切れない行は
入れず、残り40件は `?` のままにしてある。切り出しは
`tools/gamedata/import_content_images.py`、元のスクショは `tools/gamedata/screenshots/`。
