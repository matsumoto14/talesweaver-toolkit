# アイコン画像

`ui/Icon.svelte` が **gamedata の id から機械的に解決**する。手動のマッピング表は作らない。

| 系統 | ディレクトリ | ファイル名 | id の出どころ |
|---|---|---|---|
| キャラ | `characters/` | `<id>.png` | `gamedata::characters()` の `GameCharacter::id`(例 `boris.png`) |
| Mob | `mobs/` | `<id>.png` | `gamedata::enemies()` の `Enemy::id`。コンテンツの絵(`contents/`)が無いとき `Icon` の `fallback` でそのコンテンツの `enemy_id` を引く |
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

2026-09-01に新装着アビリティの夜星系・喪失系(各14件)を `equipment/` に同梱した。出典は
wiki ではなく**ゲーム内インベントリのスクリーンショット**(`Item/合成/装着アビリティシステム/
新装着アビリティ` は表だけで添付画像が無い)。絵は**系列ごとに 1 種類**なので、1 枚を系列の
全 id に複製している(id 解決は機械的にやる方針なのでマッピング表は作らない)。インベントリの
個数表示「1」は装備の絵ではないので消してある。切り出しは
`tools/gamedata/import_ability_icons.py`、元のスクショは `tools/gamedata/screenshots/`。
古代精霊系・深淵系はスクショが無いので `?` のまま。

2026-09-03に装備 1,171 件(`equipment_catalog/client.rs` の全行。テネブリス・セイクリッドを含む。テネブリスの数値は 2026-09-14 に client.rs から外して R2 取得制にしたが、アイコンは同梱のまま据え置く)のアイコンをクライアント展開データ
`tw_assets/item_icons/<ItemId>_<名前>.png` から同梱した。wiki・韓国資料由来の 313 件は同じ id で上書き(絵は同じゲーム内画像。
ピクセル差があったのは 5 件で、クライアント側を正とする)。client に無い wiki 独自行(宝玉付与の名付け・旧装備)と
レリックは引き続き `?`。再取込は `tools/gamedata/import_client_icons.py`(`import_client_db.py` の後に実行)。

2026-09-03に敵カタログ全 42 件の立ち絵をクライアント展開データから `mobs/` に同梱した(d2a を読んで
正面向きで最も大きいコマを `tw_assets/sprites/` から取り、透明で正方形に詰める。`item_icons/monsters/` の
PNG は最初の方向の最初のコマで、後ろ向きや出現エフェクトになるので使わない)。敵カタログは
「コンテンツ名 + 難易度」で持っているため名前照合では取れず、`enemies.rs` の `client_image` に手で書いた。
名前で対応が取れる 14 件以外(コンテンツ名だけの敵、2D スプライトが無いオーディン・キシニク)は、同じ
コンテンツに出る雰囲気の近いモンスターをユーザー判断で当てている(2026-09-04)。再取込は
`tools/gamedata/import_enemy_icons.py`(tw_tool_v2 の d2a パーサを借りる)。

2026-09-15に計算タブの「地力」カード 3 つと称号カードの絵を同梱した。これらは gamedata に catalog が無いので
**id は固定名**(呼び出し側の `Icon` に直書き)。
- `skills/awakening.png`(id `awakening`): 覚醒・エタの意志。絵は覚醒アイテム「エオニス・ラピス」
  (`tw_assets/item_icons/1044428_エオニス・ラピス.png`、20×22 を透明で 22×22 に詰めた)。
- `skills/sharpness_vision.png`(id `sharpness_vision`): シャープネスビジョン(Lv10 の赤い絵)。クライアントの
  スキル表 0006(Id 3006281)が指すアニメ 29500 の anim 21 = テクスチャ 10505 frame 0
  (`tw_assets/sprites/0105__10505_f000.png`)。frame 1 の水色は別段階の絵(ユーザー確認 2026-09-15)。
- `skills/soul_link.png`(id `soul_link`): **未収録**(`?` 表示)。ゲーム内の「お化け」の絵はクライアント DB から
  名前で引けない(表 0007 の「ソウルリンク」は称号カテゴリの絵で別物、アイテム表にも無い)。
- `titles/title.png`(id `title`、系統 `title`): 称号。称号そのものに絵は無く(表 0264 にアニメ列が無い)、
  称号を授けるアイテム「名誉の証(◯◯)」の絵を使う。アイテム 268 件を照合したが**全件同じ 1 枚**だったので
  1 ファイルだけ置き、称号ごとには出さない(出典 `item_icons/1009851_名誉の証(イフリート).png`)。
