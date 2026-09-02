# 対人タブ「伸びしろ」を「次にできること」に作り直す(実装済み 2026-09-02)

2026-09-02 の UX レビューで出たユーザーの指摘と、その場で決めたこと。
**実装済み(2026-09-02)**。決定記録は [ADR 007 入力 UX](adr/007-input-ux.md) と
[ADR 002 ダメージ計算の出典](adr/002-damage-formula-sources.md) にある。
この文書は経緯(何が問題で、どう決めたか)の置き場として残す。

追記(2026-09-02): 開いたときの手が多すぎたので **3 段**(合計 → 区分 → 手)にした。
domain は区分ごとに `GrowthGroupRooms { group, gain, hit_rate_gain, provisional, rooms }` を返し、
画面は区分の行を押した区分だけ手を出す。決定は ADR 007。

## 何が問題か(ユーザーの言葉)

- 伸びしろを開いても「DEX を上限まで」と言われて**具体的に何ができるのか分からない**
- エンチャントは費用が高く**最終手段**。装備の命中補正(アビリティ → ランダム OP の順)や
  バフのほうが上げやすいのに、gain 降順だとエンチャントが上に来る
- 的中剣は伸びしろではなく**つけ外し**(なんなら既定 ON)
- 命中率の行の「A → B」は**どちらが殴る側か**を読者が補っている。文言全般も調整したい

## 決めたこと

1. **並びは費用の安い順で固定**(gain 降順をやめる)
   1. バフ(命中P増加・DEX 増加)。**具体名で**書く: 「射手のルーンを付ける 0 → 20」
   2. 装備の命中補正: 装備アビリティの追加「命中」→ ランダム OP の命中P の順
   3. DEX の固定上昇: エタの意志 / 覚醒の上限に達していないぶんだけ効く。
      源はクラウン・聖物・ペット S スキル・バフなど(ルーン・モンスターカードも候補)。
      各行「ペット S スキル(DEX)を 真4 に 40 → 60」のように**現在 → 上限**で
   4. エンチャント枠(最終手段。末尾に薄く、または畳んだ先)
2. **各行は「何をするか」**を単位にする。材料の名前ではなく行動(付ける・替える・上げる)
3. **的中剣(極・的中剣、マキシミン専用スキル)は ON / OFF のチップ**。覚えられるキャラだけに出す。
   既定 ON、Lv7 固定。伸びしろの行からは外す。対人タブ内の状態だけで済む
   (`preview_versus` はドラフト値を受けるので、`character_skills.skill_ids` を足し引きするだけ)
4. **矢印で方向を示さない**。命中率の行は助詞で主語・目的語を決める(案: 「A が B に当てる 必中」)。
   伸びしろの「いま → 上限」の矢印は値の遷移なのでそのまま
5. 文言は着手時にまとめて見直す(選択カードの題・注記、根拠の帯、表のラベル、内訳の一言、空状態)

## 決定(2026-09-02 ユーザー確認)

- 装備アビリティの命中: 空き枠に付けるだけでなく、**上位への差し替え**(「夜星に替える +5」)も出す。
  `GrowthRoom` の「装備の買い替えは含めない」の例外。
- ランダム OP の命中P(グローブ・遺物ブレス): 空き枠に入れる + **ランク上げ**(Special → S・真)も出す。
- 回避P側(AGI のバフ・固定上昇 → エンチャント末尾)も**同時に**同じ作りに直す。
- DEX / AGI の固定上昇源は**モデルが持つ 6 つ全部**(ペット S・ルーン・クラウン・モンスターカード・聖物・バフ)。
  上限に達していない源だけ行になる。

## 設計(2026-09-02。architecture-audit B21 の受け皿)

### domain: 源ごとの列挙 API(値だけ。文言は持たない)

- `stat_sources.rs`
  - `stat_fixed_rooms(sources, kind) -> Vec<StatFixedRoom { source: StatFixedSource, current: i64, max: i64 }>`
    源は ペット S(段階 → 真4 = +60)/ ルーン(Lv → 20)/ クラウン(値 → `max_value(kind)`)/
    モンスターカード(→ 70)/ 神鳥の聖物(値 → 400)。上限に達している源は返さない
  - `accuracy_buff_rooms(selection, catalog, boost) -> Vec<BuffRoom { buff_id, name, value }>`
    まだ選んでいない命中P増加バフ。排他枠(`blocked_buffs` と同じ規則)と的中剣の `exclusive_with` を見る
  - `stat_buff_rooms(base, sources, selection, equipment, common, catalogs, kind, cap) -> Vec<BuffRoom>`
    まだ選んでいないステ増加バフのうち `kind` に効くもの。効きは `buff_target_stat_gains` と同じ
    「外した状態 → 足した状態」の再計算(値は最終能力値の増分)
- `equipment.rs`
  - `ability_value_rooms(equipment, abilities, kind: EquipmentStatKind) -> Vec<AbilityRoom>`
    `AbilityRoom { slot, action: Attach { ability_id } | Replace { from_ability_id, ability_id }, current: i64, target: i64 }`
    空き枠(`ability_slots` − 装着数)にはその部位・系統で付けられる最大値のアビリティ、装着済みの同系統
    (`exclusive_group`)には上位(`ladder` / `grade` が上で値が大きい)への差し替え。武器の系統適合は既存の規則
- `random_option.rs`
  - `random_option_rooms(part, slot, catalog, effect) -> Vec<RandomOptionRoom>`
    `RandomOptionRoom { slot, action: Attach { option_id, rank } | RankUp { option_id, from_rank, rank }, current, target }`
    空き枠には Special で付ける、装着済みは S・真へ上げる(既定値 = レンジ上限)
- `defense.rs`
  - `GrowthRoom` を作り直す: `{ group: GrowthGroup, action: GrowthAction, current, target, gain, hit_rate_gain, provisional }`
    `GrowthGroup` = Buff / EquipmentAbility / RandomOption / StatFixed / Enchant / Siena(**費用の安い順で固定**。
    gain 降順はやめる)。`GrowthAction` は上の Room をそのまま包む enum(Buff / AbilityAttach / AbilityReplace /
    RandomOptionAttach / RandomOptionRankUp / StatFixed / StatBuff / Enchant / Siena)
  - 的中剣(`AccuracySkill`)は伸びしろから外す(画面の ON/OFF チップ)
  - `accuracy_growth` / `evasion_growth` は列挙 API を呼び、材料ごとに命中P / 回避Pを引き直して並べるだけ

### commands
- `preview_versus` の攻撃側・防御側に、アビリティ / ランダム OP のカタログを渡す(既存の `VersusAttacker` /
  `VersusDefender` に追加)。`learnable_accuracy_skill` は「チップを出すか」の判定に残す

### TS(VersusPage)
- 行の文言は `GrowthAction` から画面が組む(「射手のルーンを付ける 0 → 20」「グローブの空き枠に『夜星の命中』+16」
  「ペット S スキル(DEX)を 真4 に 40 → 60」「盾のエンチャント 命中率補正 12 → 30」)。値の遷移は矢印のまま
- 的中剣: 覚えられるキャラだけ ON / OFF チップ(既定 ON、Lv 上限)。`character_skills.skill_ids` の足し引きで
  `preview_versus` を叩き直す
- 命中率の行は「A が B に当てる」の助詞表現。文言の総見直しはこのとき
- `GROWTH_SOURCE_ORDER` は消し、Rust の並び(`group` 順)をそのまま出す

## いまのモデルにあるもの / 無いもの(2026-09-02 の棚卸し)

| 材料 | 状態 | 場所 |
|---|---|---|
| 命中P増加バフ(4 件) | ある。ただし `buff_accuracy_point_room` は排他枠を見ずに単純合計 | `stat_sources.rs` / `gamedata/buffs.rs` |
| DEX 増加バフ | 専用の関数が無い。`buff_target_stat_gains` はフル再計算で重い | `stat_sources.rs` |
| DEX の固定上昇(ペット S・ルーン・クラウン・カード・聖物) | 現在値と上限は `StatSources` にある。**源ごとの分解は無い**(`stat_cap` との差 1 本) | `stat_sources.rs` / `defense.rs accuracy_growth` |
| 装備アビリティの命中 | カタログはある(Hand は固定値 + 排他グループ、盾+・遺物ブレスは `value_option`)。**空き枠 / 上限未満の列挙は無い** | `equipment.rs` / `equipment_catalog/abilities.rs` |
| ランダム OP の命中P | Hand / RelicBracelet の 2 部位。装着済み枠は既にランク上限。**伸びしろの計算は無い** | `random_option.rs` / `gamedata/random_options.rs` |
| エンチャント枠 | ある(`enchant_room`) | `defense.rs` |
| 的中剣 | `AccuracyBoost::PrecisionSword`。既定 Lv7。名指し分岐は architecture-audit #3 で整備予定 | `defense.rs` / `character_skill.rs` / `commands/lib.rs` |

消費側: `GrowthRoom` / `GrowthSource`(`defense.rs`)→ `api/types.ts` → `VersusPage.svelte`(`GROWTH_SOURCE_ORDER` で固定順)。
