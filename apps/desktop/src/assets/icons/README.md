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
| 称号 | `titles/` | `title.png` | 称号は個別の絵を持たないので「名誉の証」1 枚を全称号で使う。枠は装備 |
| 補正源 | `sources/` | `<SourceId>.png` | `pages/chars/sourceId.ts` の `SourceId`(例 `thesis.png`)。その補正源を象徴するアイテムの絵。対応表は `tools/gamedata/import_source_icons.py` だけが持つ。枠は装備 |

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

2026-09-15に装着アビリティ 185 件のうち 119 件の絵をクライアント展開データ `tw_assets/item_icons` から
**名前で**引いて `equipment/` に同梱した(装着アビリティはゲーム内ではアイテムで、名前がそのままアイテム名)。
既存の夜星・喪失系 28 件はスクショ由来のまま据え置き。クライアントに同名アイテムが無い 38 件
(旧武器アビリティ「(下)尖った刃」「疾風の刃」、N/R/L-耐魔法・失われた魂、深淵の魔法耐性 など)は `?` のまま。
再取込は `tools/gamedata/import_client_ability_icons.py`。

2026-09-16にレリック 40 件(神鳥・ルナリアのペンダント / ブレスレット +1〜+10)をクライアント展開データと
名前で照合し、全件を `equipment/` に取り込んだ。ライジングホリックカフス(ゲーム内名は †ライゾシンボリックカフス)も
同じ手順で取り込み、上の「`?` のまま」から外れた。AF 32 件(ディフェンシオ含む)も同じスクリプトで
クライアントの絵に揃え、wiki に添付が無かった 7 件(エクリプス物理 / 魔斬・エーテリアルチューブ 5 種)が埋まった。
再取込は `tools/gamedata/import_manual_item_icons.py` を使う。
同日、補正源 18 種の代表アイコンを `sources/` に同梱した(`tools/gamedata/import_source_icons.py`)。
同日、wiki に絵が無いバフ 5 件(略奪パン・茹でミミック・古代レリックの聖域ミニゲームバフ・遊び用チンキ剤・
ハードウエポン(エアル))をクライアント item の絵と同梱済みのスキル絵で埋めた(`tools/gamedata/import_client_buff_icons.py`)。
テイルズウィーバーのエネルギーは wiki Skill/共通 の添付(`import_buff_icons.py`)。クラブ効果・射手のルーンは
本物の絵が無く、別の物で代用せず `?` のまま(ユーザー判断)。
同日、共通スキル 11 件(`skills/common_<名前>.png`)と極限スキル 3 件(`skills/<UltimateSkill id>.png`)を
wiki Skill/共通・Skill/ゲージスキル から取り込んだ(`tools/gamedata/import_common_skill_icons.py`)。

2026-09-16にモンスター立ち絵 18 件を差し替えた(ユーザー判断)。`enemies.rs` の `client_image` を直して
`import_enemy_icons.py` を流し直すだけで、対応表は増やしていない。
- アビス(ヘル) = 深淵の第2使徒 / アビスEX = 深淵の第1使徒(どちらもアーカンだった)
- アークロン地下要塞 = アンデッド兵士(デュラハンだった)
- 最後の決戦 1 = 切り込み隊長、ロカゴス / 2 = 杖の司祭・ゴイティア / 3 = 召喚の石像(1 つずつずれていた)
- レリックの聖域 10〜19段 = デスポイナ(20段 = キシニクと同じ代役に揃えた。遺跡ガーディアンだった)
- キマイラ(参加型レイド) = メルカルト・キマイラ(旧マップの通常キマイラだった)

キシニク・オーディン・トゥタトールは 3D モデル(d2a が `t17` で 3DObject を指す)なので 2D の立ち絵が
無く、wiki にも添付が無い。同じコンテンツに出る絵を代役に当てている。

同日、機械的に解決できない絵を `tools/gamedata/import_manual_icons.py` にまとめた。
- アビス(ノーマル)/(ハード)は `enemies.rs` に敵が無く `mobs/` のフォールバックも効かないので
  `?` のままだった。ゲーム内一覧のメダルも撮れないため、そのコンテンツに出るモンスターの立ち絵を
  当てている(ノーマル = 深淵の第4使徒 / ハード = 深淵の第5使徒。第3使徒は 2D の絵が無い)
- キシニクは 3D モデルで 2D 立ち絵が無いが、**アフェティリア(ハード / EX)のメダルが本人の絵**
  なので、キシニクが出る面すべて(アフェティリア 2 段・レリックの聖域 10〜20段)でこれを流用する。
  `enemies.rs` 側は `client_image: None` にしてあるので `import_enemy_icons.py` とはぶつからない
- トゥタトゥールは 3D モデルで絵が無い。プレタを代役に当てている(ユーザー判断)
- 神鳥の塒(ノーマル / ハード)= 神鳥、オルリー防衛戦(ヘル)= バンダレックス・ダ・アノマラド。
  どちらも敵データを持たない行(ユーザー判断)

同日、ホームに絵を足した。
- 「次の目標」のピッカーは一覧の行と同じ `content` → `mob` の解決で、自動の行も**行き先の絵**を出す
  (`Picker` の `PickerOption` に `iconFallback` を足した)
- 「今日の強化」の 5 タイル: 神鳥の聖物 = `sources/relic`、シエナのオーラ = `sources/siena`、
  カフス = `equipment/rising-holic-cuffs`、レリック = `equipment/godbird-pendant-plus1`
  (+1〜+10 は全部同じ絵)、エンチャント = `sources/enchant`。
  `enchant` は SourceId に無い**固定名**(キャラタブでは装備の中の操作なので補正源のペインが無い)。
  絵はアイテム「エンチャント強化呪文書」で、`import_source_icons.py` が取り込む。
