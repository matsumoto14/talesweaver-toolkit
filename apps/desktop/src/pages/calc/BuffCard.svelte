<script lang="ts">
  /**
   * バフ(計算の材料)。**この計算だけの選択**で、キャラにもバフセットにも保存しない
   * (保存は帯の「キャラに保存」)。35 個を全部並べるとペインの大半を占めるので、
   * 目的ごとに畳んで見出しの n/m だけを常に見せる(§09 規則 2)。
   * 排他枠の衝突・実際の寄与はどちらも Rust の応答を読むだけで、数字を写経しない。
   */
  import { listBlockedBuffs } from "../../api/commands";
  import type { BlockedBuff, BuffChoice, BuffDefinition, BuffSelection, DamageResult, StatKind } from "../../api/types";
  import {
    BUFF_PURPOSES, buffSetOptions, isBuffOn, isChoiceValue, isMultiTarget, isPercentLayer,
    isUserSelectedTarget, matchesPurpose, pickedStats, toggleBuff, toggleBuffStat, userInputRange,
  } from "../../buffs";
  import { fmtSigned, fmtSignedPct, formatLayerValue, topRowsText } from "../../format";
  import { t } from "../../i18n";
  import { STAT_KINDS, STAT_LABELS } from "../../labels";
  import { app } from "../../state.svelte";
  import Choose from "../../ui/Choose.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Icon from "../../ui/Icon.svelte";
  import NumberField from "../../ui/NumberField.svelte";
  import Picker from "../../ui/Picker.svelte";
  import Popover from "../../ui/Popover.svelte";
  import ToggleRow from "../../ui/ToggleRow.svelte";
  import Value from "../../ui/Value.svelte";
  import { latest } from "../../ui/latest.svelte";

  interface Props {
    /** 1 件ごとの寄与はこの計算結果のトレースから引く */
    result: DamageResult | null;
  }
  let { result }: Props = $props();

  // バフカタログは常用バフ専用(キャラスキルは補正源のキャラスキル欄)
  const consumableBuffs = $derived(app.catalog);
  const buffOn = (def: BuffDefinition) => isBuffOn(app.calcBuffs.choices, def.id);
  /**
   * バフの 3 状態(v4)。保存済みかどうかで「常時(マイセット)」と「追加枠」を分ける。
   * - `always`: キャラに保存済み = 毎回のっている常用セット
   * - `extra`: この計算だけの追加(試し変更。保存されない)
   * - `off`: 使わない(保存済みバフを一時的に外した場合も含む)
   * 常時への昇格は保存操作(「試し変更を保存」)で行う。チップのクリックで DB を書かない。
   */
  const buffState = (def: BuffDefinition): "always" | "extra" | "off" => {
    const saved = (app.buffSets.find((set) => set.id === app.calcBuffSetId)?.choices.choices ?? [])
      .some((c) => c.buff_id === def.id);
    if (!buffOn(def)) return "off";
    return saved ? "always" : "extra";
  };
  const alwaysBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "always").length);
  const extraBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "extra").length);
  function chooseCalcBuffSet(value: string) {
    const id = value === "" ? null : Number(value);
    app.calcBuffSetId = id;
    const set = app.buffSets.find((item) => item.id === id);
    app.calcBuffs = JSON.parse(JSON.stringify(set?.choices ?? { choices: [] }));
  }
  /* 「計算の材料」のカードもバフの目的グループも、排他にしない(2026-09-17)。
     1 つしか開かない形だと、押した見出しの上にあるまとまりが閉じて押した場所が上へ飛ぶ
     (§00 03「押した場所は動かない」に反する)。全部開くと縦に長くなるが、
     開くのはユーザーが選んだぶんだけなので、閉じ忘れは自分で畳める */
  function toggleBuffChip(def: BuffDefinition) {
    app.calcBuffs = { choices: toggleBuff(app.calcBuffs.choices, def, !buffOn(def)) };
  }
  // ON のバフのうち、対象ステ・効果量の選択肢・手入力を持つものの詳細編集(試し変更として反映)
  const statOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));
  const buffChoiceOf = (buffId: string) =>
    app.calcBuffs.choices.find((c) => c.buff_id === buffId) ?? null;
  const buffChoiceOfStat = (buffId: string, stat: StatKind) =>
    app.calcBuffs.choices.find((c) => c.buff_id === buffId && c.stat === stat) ?? null;
  /** この画面で**触れる**ものがあるか。固定値のバフは読むだけなので含めない —
   *  値はチップ本体の寄与表示で既に見えていて、開いても「値: +30%」と出るだけだった */
  const hasDetail = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null;
  function editBuffChoice(buffId: string, fn: (c: BuffChoice) => void, stat?: StatKind) {
    const choices = app.calcBuffs.choices.map((choice) => ({ ...choice }));
    const choice = choices.find(
      (item) => item.buff_id === buffId && (stat === undefined || item.stat === stat),
    );
    if (choice) fn(choice);
    app.calcBuffs = { choices };
  }
  /** 複数ステ対象バフ(クラブ効果)の、1 ステぶんの ON/OFF。試し変更なので保存はしない */
  function toggleBuffStatChip(def: BuffDefinition, stat: StatKind, next: boolean) {
    app.calcBuffs = { choices: toggleBuffStat(app.calcBuffs.choices, def, stat, next) };
  }
  /** 排他枠の衝突で選べないバフ。判定は Rust(`blocked_buffs`)、ここは応答を持つだけ */
  let calcBlockedBuffs = $state<BlockedBuff[]>([]);
  const blockedLatest = latest({ debounce: 0 });
  $effect(() => {
    const buffs = JSON.parse(JSON.stringify(app.calcBuffs)) as BuffSelection;
    blockedLatest.run(async (isCurrent) => {
      const blocked = await listBlockedBuffs(buffs);
      if (isCurrent()) calcBlockedBuffs = blocked;
    });
    return () => blockedLatest.cancel();
  });
  // --- バフ 1 件ごとの寄与(ON にしたチップに併記)。実計算(previewDamage)のトレースが
  //     供給源名で内訳を持っている(catContributions と同じデータ)ので、そこから該当バフの
  //     行だけ拾う。写経で数字を作らない。ステ側は stat_contributions ではなく
  //     stat_source_effects を使う(cap 込みで倍率A/B の増幅も一貫して割り振られる帰属)。
  //     全ステに乗るバフは 7 件そのまま並べるとチップ幅から欠けるので、ステ側は効きの大きい
  //     上位 2 件 + ほか n に絞る(バフタブの statDeltaText と同じ topRowsText を使い、
  //     二重管理にしない)。 ---
  function buffContributionText(def: BuffDefinition): string {
    const statRows = (result?.trace.stat_source_effects ?? [])
      .filter((c) => c.source === def.name && c.effect !== 0)
      .map((c) => ({ label: `${STAT_LABELS[c.kind]} ${fmtSigned(c.effect, { max: 3 })}`, value: c.effect }));
    const parts: string[] = [];
    if (statRows.length > 0) parts.push(topRowsText(statRows));
    for (const c of result?.trace.category_contributions ?? []) {
      if (c.source === def.name && c.value !== 0) {
        const cat = result?.trace.categories.find((x) => x.category === c.category);
        const label = cat ? `${cat.symbol}` : c.category;
        parts.push(`${label} ${fmtSignedPct(c.value, { max: 4 })}`);
      }
    }
    return parts.join(" ・ ");
  }
</script>

  {#snippet buffChip(def: BuffDefinition)}
                {@const state = buffState(def)}
                {@const blocked = state === "off" && calcBlockedBuffs.some((b) => b.buff_id === def.id)}
                {@const detail = state !== "off" && hasDetail(def)}
                <!-- ON にしたチップの実際の寄与(供給源ごとの実数。写経しない)。
                     値の調整(設定・ポップオーバー)は押せる面の外(extra)に置く —
                     押した名前の上に段を差し込まない(§00 03) -->
                <ToggleRow
                  name={t(def.name)}
                  value={state !== "off" ? buffContributionText(def) : undefined}
                  on={state !== "off"}
                  tone={state === "extra" ? "temp" : "saved"}
                  disabled={blocked}
                  title={blocked ? t("同枠の他バフと排他です") : (def.note ? t(def.note) : undefined)}
                  onToggle={() => { if (!blocked) toggleBuffChip(def); }}
                >
                  {#snippet icon()}
                    <!-- 未収録の id は破線 + ? になり、その場でも幅は変わらない -->
                    <Icon kind="buff" id={def.id} size={20} label={t(def.name)} />
          {/snippet}
                  {#snippet extra()}
                    {#if detail}
                      {@const choice = buffChoiceOf(def.id)}
                      {#if choice}
                        <Popover
                          label={t("{name} の設定", { name: t(def.name) })}
                          triggerLabel={t("{name} の設定", { name: t(def.name) })}
                          triggerClass="chip-config"
                          panelClass="buff-editor"
                        >
                          {#snippet trigger()}{t("設定")}{/snippet}
                          {#snippet children(close)}
                        {#if isMultiTarget(def.target)}
                        <!-- クラブエフェクトはステごとに 1 つずつ併用できる。ここでは対象ステの
                             出し入れだけを試せるようにし、値はバフタブ側の設定を引き継ぐ -->
                        <div class="field">
                          <span class="field-head">
                            <span class="field-label">{t("対象ステ")}</span>
                            <Value
                              class="field-count"
                              motion={() => pickedStats(app.calcBuffs.choices, def).length}
                              value={`${pickedStats(app.calcBuffs.choices, def).length}/${STAT_KINDS.length}`}
                            />
                          </span>
                          <Choose
                            label={t("対象ステ")}
                            options={statOptions}
                            max={STAT_KINDS.length}
                            values={pickedStats(app.calcBuffs.choices, def)}
                            onToggle={(v, next) => toggleBuffStatChip(def, v as StatKind, next)}
                          />
                        </div>
                      {:else if isUserSelectedTarget(def.target)}
                        <div class="field">
                          <span class="field-label">{t("対象ステ")}</span>
                          <Choose
                            label={t("対象ステ")}
                            options={statOptions}
                            bind:value={
                              () => choice.stat ?? STAT_KINDS[0],
                              (v) => editBuffChoice(def.id, (c) => (c.stat = v as StatKind))
                            }
                          />
                        </div>
                      {/if}
                      {#if isChoiceValue(def.value)}
                        {@const options = def.value.choice.map((v, i) => ({ value: String(i), label: formatLayerValue(def.layer, v) }))}
                        <!-- 値の候補は小さい順に並ぶ(順序あり)ので段(§07「1 つ選ぶ」) -->
                        <div class="field">
                          <span class="field-label">{t("値")}</span>
                          <Choose
                            label={t("値")}
                            {options}
                            full
                            bind:value={
                              () => String(choice.choice_index ?? 0),
                              (v) => editBuffChoice(def.id, (c) => (c.choice_index = Number(v)))
                            }
                          />
                        </div>
                      {/if}
                      {#if userInputRange(def.value)}
                        {@const range = userInputRange(def.value)!}
                        {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                        {#if isMultiTarget(def.target)}
                          {#each pickedStats(app.calcBuffs.choices, def) as stat (stat)}
                            <div class="stat-value-row">
                              <span class="stat-value-label">{STAT_LABELS[stat]}</span>
                              <NumberField
                                label={t("{stat}の値", { stat: STAT_LABELS[stat] })}
                                min={range.min * scale}
                                max={range.max * scale}
                                bind:value={
                                  () => (buffChoiceOfStat(def.id, stat)?.value ?? def.default_value ?? range.min) * scale,
                                  (v) => editBuffChoice(def.id, (c) => (c.value = v / scale), stat)
                                }
                              />
                            </div>
                          {/each}
                        {:else}
                          <div class="stat-value-row">
                            <span class="stat-value-label">{isPercentLayer(def.layer) ? t("値 (%)") : t("値")}</span>
                            <NumberField
                              label={isPercentLayer(def.layer) ? t("値 (%)") : t("値")}
                              min={range.min * scale}
                              max={range.max * scale}
                              bind:value={
                                () => (choice.value ?? 0) * scale,
                                (v) => editBuffChoice(def.id, (c) => (c.value = v / scale))
                              }
                            />
                          </div>
                        {/if}
                      {/if}
                            <button type="button" class="popover-close" onclick={close}>{t("閉じる")}</button>
                          {/snippet}
                        </Popover>
                      {/if}
                    {/if}
          {/snippet}
                </ToggleRow>
  {/snippet}

        <!-- バフ -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <Icon kind="buff" id="illumination_drink" size={20} label={t("バフ")} />
            <span class="card-title">{t("バフ")}</span>
            <Value class="dim small" motion={() => alwaysBuffCount + extraBuffCount} value={t("{n} 件", { n: alwaysBuffCount + extraBuffCount })} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <div class="calc-buff-set">
            <span>{t("使うセット")}</span>
            <Picker
              label={t("使うバフセット")}
              options={buffSetOptions(app.buffSets, t("追加だけで計算"))}
              bind:value={() => (app.calcBuffSetId === null ? "" : String(app.calcBuffSetId)), chooseCalcBuffSet}
            />
          </div>
          <p class="buff-legend dim">
            <span class="lg always">{t("常")}</span> {t("セット内({n} 件)", { n: alwaysBuffCount })}
            ／ <span class="lg extra">{t("追")}</span> {t("追加 = この計算だけ({n} 件・保存されません)", { n: extraBuffCount })}
            ／ {t("無印 使わない。")}
          </p>
          <!-- 目的ごとに畳む。35 個を全部並べると 31 行(1177px)になり、ペインの大半を
               バフが占める。見出しは常に同じ場所にあり、開いても**その下に生えるだけ**で
               上は動かない(§09 規則 2)。どこに何件入れているかは見出しの n/m で分かるので、
               閉じたままでも「使い忘れ」に気づける -->
          {#each BUFF_PURPOSES as purpose (purpose.id)}
            {@const defs = consumableBuffs.filter((d) => matchesPurpose(d, purpose.id))}
            {@const picked = defs.filter((d) => buffState(d) !== "off").length}
            {#if defs.length > 0}
              <Disclosure summaryClass="buff-group-head inset">
                {#snippet summary()}
                  <span class="bg-label">{purpose.label}</span>
                  <Value class="bg-count" motion={() => picked} value={`${picked}/${defs.length}`} />
                {/snippet}
                <div class="buff-chips">
                  {#each defs as def (def.id)}{@render buffChip(def)}{/each}
                </div>
              </Disclosure>
            {/if}
          {/each}
          <p class="buff-note dim">{t("変更はこの計算だけに反映され、バフセットやキャラには保存されません。")}</p>
          {/if}
          {/snippet}
        </Disclosure>

<style>
  /* 35 個を pill で流すと幅がまちまちで、251px の器では 31 行中 26 行が 1 個だけだった
     (実測)。横に並ぶ利点が出ないうえ、名前の頭が縦に揃わず探しにくい。**1 列の行**にして、
     頭を揃える。丸(--r-pill)は「小さな状態の印」に使う形なので、行にはインセットの角丸 */
  .buff-chips {
    margin-top: 5px; margin-bottom: 3px; display: flex; flex-direction: column; gap: 3px;
  }
  /* 目的グループの見出し。押しても見出し自身は動かず、中身がその下に生えるだけ */
  :global(summary.buff-group-head) {
    width: 100%; margin-top: 5px; padding: 5px 8px;
    display: flex; align-items: center; gap: 7px; color: var(--fg-sub);
    font-size: 10px; font-weight: 700; text-align: left; cursor: pointer;
  }
  :global(summary.buff-group-head:hover) { border-color: var(--accent); }
  :global(details[open] > summary.buff-group-head) { background: var(--sel-card); border-color: var(--sel-bd); color: var(--sel-fg); }
  .bg-label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* .bg-count は ui/Value.svelte が描く子要素。summary.buff-group-head も ui/Disclosure が描くので両方 :global */
  :global(summary.buff-group-head .bg-count) { flex: none; min-width: 5ch; text-align: right; font-size: 9px; font-weight: 500; }
  .calc-buff-set { margin-top: 8px; display: flex; align-items: flex-start; gap: 8px; font-size: 10px; color: var(--fg-muted); }
  .calc-buff-set > span { flex-shrink: 0; padding-top: 6px; }
  .calc-buff-set :global(.picker) { min-width: 0; flex: 1; }
  .buff-legend { margin: 7px 0 0; font-size: 9px; line-height: 1.7; }
  .buff-legend .lg {
    display: inline-block; padding: 0 5px; border-radius: var(--r-pill);
    font-size: 8.5px; font-weight: 700; border: 1px solid;
  }
  .buff-legend .lg.always { background: #CCF7FF; border-color: var(--sel-bd); color: var(--sel-fg); }
  .buff-legend .lg.extra { background: var(--state-temp-bg); border-color: var(--sim); color: var(--sim-fg); }
  .buff-note { margin: 8px 0 0; font-size: 9px; line-height: 1.6; }
  /* 値の調整。チップに重ねて出すので、ON にした数だけペインが伸びることがない。
     置き場所は app.css の .popover(「設定」の右端に揃えて下に開く) */
  :global(.buff-editor) { min-width: 210px; gap: 7px; }
  /* 値の行は「名前 + 欄」。名前は行が持つ(部品は読み上げ名だけを持つ・§07) */
  .stat-value-row { display: flex; align-items: center; gap: 3px; min-width: 0; }
  .stat-value-label {
    flex: none; min-width: 46px; margin-right: 5px; white-space: nowrap;
    font-size: 10.5px; font-weight: 700; color: var(--fg-muted);
  }
  /* 行の押せる面(.face)とは別の的。extra に置くので行そのものは押しても動かない */
  :global(.chip-config) {
    flex: none; margin: 0 2px 0 0; padding: 0 0 0 6px;
    border: 0; border-left: 1px solid var(--border-soft); background: none;
    color: var(--fg-muted); font: inherit; font-size: 8.5px; text-decoration: underline;
    text-underline-offset: 2px; cursor: pointer; opacity: .8;
  }
  :global(.chip-config:hover) { opacity: 1; color: var(--accent); }
</style>
