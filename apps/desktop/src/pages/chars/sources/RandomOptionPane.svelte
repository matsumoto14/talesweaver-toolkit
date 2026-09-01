<script lang="ts">
  // 「randomOption」補正源のペイン。装備と同じ部位ドリルダウン(§09 規則 2)。
  import type { PartSlot, RandomOptionDef, RandomOptionRank, Skill, StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import {
    neutralEquipmentPart,
    randomOptionEffectLabel, randomOptionIsApplied, randomOptionMatchesDependency,
    randomOptionValue, randomOptionValueLabel,
  } from "../../../equipment";
  import {
    PART_SLOT_LABELS, RANDOM_OPTION_ALLOWED_SLOTS, RANDOM_OPTION_RANKS, RANDOM_OPTION_RANK_LABELS,
    SKILL_DEPENDENCIES, SKILL_DEPENDENCY_LABELS,
  } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { app, equipmentFocus } from "../../../state.svelte";
  import Picker, { type PickerOption } from "../../../ui/Picker.svelte";
  import StatInput from "../../../ui/StatInput.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";
  import type { SourceId } from "../sourceId";
  import { flash } from "../../../ui/motion.svelte";
  import { tick, untrack } from "svelte";
  import { randomOptionRecordOnlyCount } from "../summaries";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, skills, onOpenSource }: Props = $props();

  /** ランダムOP のうち記録するだけの枠数。行サブタイトルとも共有(summaries.ts) */
  const roRecordOnly = $derived(randomOptionRecordOnlyCount(preview));
  const pct = (v: number) => Number((v * 100).toFixed(2));
  /**
   * ランダムOP の効き先ごとの合計(結果の置き場所)。同系統は足して 1 行にする。
   * 集計は Rust 側(preview.random_option_totals)。ここは効き先の日本語ラベルへの対応づけだけ
   */
  const roTotals = $derived.by<{ label: string; value: string }[]>(() => {
    const t = preview?.random_option_totals;
    if (!t) return [];
    const rows: { label: string; value: string }[] = [];
    const addPercent = (label: string, v: number) => {
      if (v === 0) return;
      const n = pct(v);
      rows.push({ label, value: `${n > 0 ? "+" : ""}${n}%` });
    };
    const addPoint = (label: string, v: number) => {
      if (v === 0) return;
      rows.push({ label, value: `${v > 0 ? "+" : ""}${v}` });
    };
    for (const dep of SKILL_DEPENDENCIES) {
      addPercent(`与ダメージ増加(${SKILL_DEPENDENCY_LABELS[dep]})`, t.dependency_damage_rate[dep]);
    }
    addPercent("攻撃ダメージ増加", t.attack_damage_rate);
    addPercent("割合追加ダメージ", t.added_damage_rate);
    addPercent("割合追加ダメージ(物理依存)", t.physical_added_damage_rate);
    addPercent("割合追加ダメージ(魔法依存)", t.magic_added_damage_rate);
    addPercent("ダメージ増幅(物理依存)", t.physical_damage_amplify);
    addPercent("ダメージ増幅(魔法依存)", t.magic_damage_amplify);
    addPoint("命中P", t.accuracy_point);
    addPoint("回避P", t.evasion_point);
    if (t.actual_delay_reduction !== 0) {
      rows.push({ label: "中ディレイ", value: `−${pct(t.actual_delay_reduction)}%` });
    }
    if (t.min_evasion_rate !== 0) {
      rows.push({ label: "最小回避率補正", value: `+${t.min_evasion_rate}%` });
    }
    return rows;
  });

  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);

  const selectedPartOrNull = (slot: PartSlot) => {
    const list = draft.equipment.parts[slot];
    return list.registered.find((p) => p.id === list.selected_id) ?? null;
  };
  const selectedPart = (slot: PartSlot) => {
    const list = draft.equipment.parts[slot];
    let part = list.registered.find((p) => p.id === list.selected_id);
    if (!part) {
      part = neutralEquipmentPart();
      part.id = Math.max(0, ...list.registered.map((p) => p.id)) + 1;
      list.registered.push(part);
      list.selected_id = part.id;
    }
    return part;
  };
  const partSlotRule = (slot: PartSlot) => limits.part_slot_rules.find((r) => r.slot === slot) ?? null;
  const equippedItem = (slot: PartSlot) => {
    const itemId = selectedPartOrNull(slot)?.item_id;
    return itemId ? (app.equipmentCatalog.find((i) => i.id === itemId) ?? null) : null;
  };
  /** その部位に付けられる枠の数(domain: PartSlot::random_option_slots)。武器だけ 3 枠 */
  const randomOptionSlots = (slot: PartSlot) =>
    equippedItem(slot)?.random_option_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.random_option_slots ?? 0));

  // --- ランダムオプション -------------------------------------------------
  // 効果値の上限は wiki の一覧表のレンジそのもの。枠数は wiki に記載が無く、代わりに
  // 「同じカテゴリーは 1 部位に 1 つまで(カテゴリー 0 は除く)」で縛る(転移の説明)。
  const randomOptionDef = (id: string): RandomOptionDef | undefined =>
    app.randomOptions.find((d) => d.id === id);

  /** その部位に足せる OP(未選択 かつ カテゴリーが空いているもの) */
  function addableRandomOptions(slot: PartSlot) {
    const part = selectedPart(slot);
    const takenIds = new Set(part.random_options.map((o) => o.option_id));
    const takenCategories = new Set(
      part.random_options.map((o) => randomOptionDef(o.option_id)?.category).filter((c) => c !== undefined && c !== 0),
    );
    // 段階選択に並べるので、プレースホルダは入れない。押せない選択肢を混ぜると
    // 「選ばれているのに何も起きない」項目になる(§00 意味のないものを置かない)
    return [
      ...app.randomOptions
        .filter((d) => d.slot === slot && !takenIds.has(d.id) && !takenCategories.has(d.category))
        .map((d) => ({ value: d.id, label: d.name })),
    ];
  }

  /** ランダムOP のドリルダウン。装備と同じく、押した部位は残して右にペインを足す(§09 規則 2) */
  let openRandomPart = $state<PartSlot | null>(null);

  // --- エラー帯からの「ここを開く」 -------------------------------------
  // 帯が指した部位を開き、該当 OP 行を光らせて見える位置まで送る(§00 ④)。
  let detailEl = $state<HTMLElement | null>(null);
  let focusedOptionId = $state<string | null>(null);
  let focusSeq = $state(0);
  const focusToken = (optionId: string) => (focusedOptionId === optionId ? String(focusSeq) : "");
  async function revealFocused(optionId: string) {
    await tick();
    detailEl?.querySelector(`[data-option-id="${CSS.escape(optionId)}"]`)
      ?.scrollIntoView({ block: "center", behavior: "smooth" });
  }
  $effect(() => {
    const request = equipmentFocus.request;
    if (!request || request.randomOptionId === null) return;
    const optionId = request.randomOptionId;
    untrack(() => {
      const list = draft.equipment.parts[request.slot];
      if (list.registered.some((p) => p.id === request.partId)) list.selected_id = request.partId;
      openRandomPart = request.slot;
      focusedOptionId = optionId;
      focusSeq = request.seq;
      equipmentFocus.request = null;
      void revealFocused(optionId);
    });
  });
  const NEUTRAL_RO = "なし";
  /** その部位に足せる OP のうち、実際によく付けるもの(gamedata の common) */
  const addableDefs = (slot: PartSlot): RandomOptionDef[] =>
    addableRandomOptions(slot)
      .map((o) => randomOptionDef(o.value))
      .filter((d): d is RandomOptionDef => d !== undefined);
  /**
   * よく使う OP。**主軸スキルの依存に合う「◯◯攻撃力が増加」を先頭**に出す —
   * ここで実際に選ばれるのはほぼそれで、攻撃ダメージ増加はその次(ユーザー確認 2026-08-26)
   */
  const commonAddable = (slot: PartSlot) => {
    const dependency = mainSkill?.dependency ?? null;
    const rank = (d: RandomOptionDef) => {
      const effect = d.effect;
      if (typeof effect === "object") {
        return effect.dependency_damage_rate === dependency ? 0 : 1;
      }
      return 2;
    };
    return addableDefs(slot)
      .filter((d) => d.common && randomOptionMatchesDependency(d.effect, dependency))
      .sort((a, b) => rank(a) - rank(b));
  };
  // 主軸に合わない「よく使う OP」は消さず、ほかの OP から到達可能にする。
  const otherAddable = (slot: PartSlot) => {
    const dependency = mainSkill?.dependency ?? null;
    return addableDefs(slot).filter(
      (d) => !d.common || !randomOptionMatchesDependency(d.effect, dependency),
    );
  };
  const otherPickerOptions = (slot: PartSlot): PickerOption[] =>
    otherAddable(slot).map((d) => ({
      value: d.id,
      name: d.name,
      meta: `カテゴリー${d.category} ・ ${randomOptionEffectLabel(d.effect)}`,
      iconId: undefined,
    }));
  function addRandomOption(slot: PartSlot, id: string) {
    if (id === "") return;
    const def = randomOptionDef(id);
    if (!def || def.tiers.length === 0) return;
    // 既定は一覧のいちばん上位のランク(手持ちがそれ未満なら下げてもらう)
    const rank = def.tiers[def.tiers.length - 1].rank;
    selectedPart(slot).random_options = [
      ...selectedPart(slot).random_options,
      { option_id: id, rank, value: null },
    ];
  }
  function removeRandomOption(slot: PartSlot, index: number) {
    const part = selectedPart(slot);
    part.random_options = part.random_options.filter((_, i) => i !== index);
  }
  const rankOptions = (def: RandomOptionDef) =>
    RANDOM_OPTION_RANKS.filter((r) => def.tiers.some((t) => t.rank === r)).map((r) => ({
      value: r,
      label: RANDOM_OPTION_RANK_LABELS[r],
    }));
  // **実際に使うのは Special と S・真だけ**(Normal / Valuable / Rare はほぼ付けない。
  // ユーザー確認 2026-08-26)。下位は開いたときだけ出す
  const MAIN_RANKS: RandomOptionRank[] = ["special", "s_true"];
  let rankAllOpen = $state(false);
  const rankOptionsNow = (def: RandomOptionDef, rank: RandomOptionRank) => {
    const all = rankOptions(def);
    if (rankAllOpen || !MAIN_RANKS.includes(rank)) return all;
    const main = all.filter((o) => MAIN_RANKS.includes(o.value));
    return main.length > 0 ? main : all;
  };
  const hasLowerRanks = (def: RandomOptionDef) =>
    rankOptions(def).some((o) => !MAIN_RANKS.includes(o.value));
  /** ランクを変えるとレンジが変わるので、実測の上書きは外して既定(レンジ上限)へ戻す */
  function setRandomOptionRank(slot: PartSlot, index: number, rank: RandomOptionRank) {
    const option = selectedPart(slot).random_options[index];
    option.rank = rank;
    option.value = null;
  }
  const tierOf = (def: RandomOptionDef, rank: RandomOptionRank) =>
    def.tiers.find((t) => t.rank === rank);
</script>

<!-- ランダムOP の編集(部位詳細で共有する 1 部位ぶんのフォーム) -->
{#snippet randomOptionEditor(slot: PartSlot)}
  {@const part = selectedPartOrNull(slot)}
  {#if part === null}
    <div class="empty-note">
      <span>先にこの部位の装備を登録してください。</span>
      <button type="button" class="chip" onclick={() => onOpenSource("equipment")}>装備へ ›</button>
    </div>
  {:else}
  {#each part.random_options as option, index (option.option_id)}
    {@const def = randomOptionDef(option.option_id)}
    {#if def}
      {@const t = tierOf(def, option.rank)}
      <!-- 1 OP 1 行。名前 / ランク / 効果値 / 外す を列でそろえる(§00 01) -->
      <div
        class="ro-row"
        class:record-only={!randomOptionIsApplied(def.effect)}
        data-option-id={option.option_id}
        use:flash={() => focusToken(option.option_id)}
      >
        <span class="ro-name" title={def.name}>{def.name}</span>
        <button type="button" class="clear" onclick={() => removeRandomOption(slot, index)}>外す</button>
        <!-- ランクは言葉なので幅は中身なり。ふだんは Special / S・真 だけ -->
        <span class="ro-rank">
          <StepSelect
            label=""
            options={rankOptionsNow(def, option.rank)}
            bind:value={
              () => option.rank,
              (v) => setRandomOptionRank(slot, index, v as RandomOptionRank)
            }
          />
          {#if hasLowerRanks(def) && MAIN_RANKS.includes(option.rank)}
            <button
              type="button"
              class="chip quiet"
              class:on={rankAllOpen}
              onclick={() => (rankAllOpen = !rankAllOpen)}
            >{rankAllOpen ? "上位だけ" : "下位も"}</button>
          {/if}
        </span>
        <StatInput
          label=""
          min={t ? t.min : 0}
          max={t ? t.max : limits.random_option_value_max}
          step={t && Number.isInteger(t.min) && Number.isInteger(t.max) ? 1 : 0.5}
          format={t ? () => `wiki ${t.min}–${t.max}` : undefined}
          bind:value={() => randomOptionValue(option, def), (v) => (option.value = v)}
          stepper
        />
      </div>
      {#if def.note}<p class="hint dim ro-note">{def.note}</p>{/if}
    {/if}
  {/each}
  {/if}
{/snippet}

<!-- 効いている量(結果)。ペイン自体が既に「ランダムOP」の名前を出しているので見出しは持たない -->
{#if roTotals.length > 0 || roRecordOnly > 0}
  <div class="eq-summary num inset">
    {#each roTotals as t (t.label)}
      <span><span class="dim">{t.label}</span> <span use:flash={() => t.value}>{t.value}</span></span>
    {:else}
      <span class="dim">計算に入る OP はまだありません</span>
    {/each}
  </div>
  {#if roRecordOnly > 0}
    <p class="dim tiny">記録するだけの枠が {roRecordOnly} 件あります(発動条件付き・被ダメージ側)。</p>
  {/if}
{/if}
<div class="card">
  <p class="hint dim">
    wiki「ランダムオプション」。装備補正 9 値には乗らず、与ダメージ式のカテゴリ(依存別の与ダメージ増加・
    攻撃ダメージ増加)や命中P・回避P に直接効きます。<b>同じカテゴリーの OP は 1 部位に 1 つだけ</b>です(wiki: 転移)。
    効果値は触らなければレンジ上限で計算します(オプション変化石で振り直せるため)。
    <b>収録しているのは火力・命中・回避に関係する OP だけ</b>で、HP・移動速度・経験値などは入れていません。
    グレーの枠は<b>記録するだけ</b>(発動条件付き・未実装の概念)で計算には入りません。
  </p>
</div>
<!-- 装備と同じドリルダウン。押した部位はその場に残り、右にペインが増える(§09 規則 2) -->
<div class="part-split" class:open={openRandomPart !== null}>
  <div class="part-list">
    {#each RANDOM_OPTION_ALLOWED_SLOTS as slot (slot)}
      {#if app.randomOptions.some((d) => d.slot === slot) && randomOptionSlots(slot) > 0}
        {@const count = selectedPartOrNull(slot)?.random_options.length ?? 0}
        <button
          type="button"
          class="part-row"
          class:on={openRandomPart === slot}
          onclick={() => (openRandomPart = slot)}
        >
          <span class="part-main">
            <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
          </span>
          <!-- 付いている OP を短い名前のバッジで並べる。名前をそのまま出すと 1 行に入らない -->
          <span class="ro-badges">
            {#each selectedPartOrNull(slot)?.random_options ?? [] as o (o.option_id)}
              {@const def = randomOptionDef(o.option_id)}
              {#if def}
                <!-- バッジは「何が付いているか」だけ。いくら効いているかは行の要約で出す -->
                <span
                  class="ro-badge"
                  class:record-only={!randomOptionIsApplied(def.effect)}
                  title="{def.name}({randomOptionEffectLabel(def.effect)} {randomOptionValueLabel(o, def)})"
                >{def.short}</span>
              {/if}
            {/each}
            {#if count === 0}<span class="dim">なし</span>{/if}
          </span>
          <span class="chev dim">›</span>
        </button>
      {/if}
    {/each}
  </div>
  {#if openRandomPart !== null}
    {@const slot = openRandomPart}
    <div class="part-detail pane-in" bind:this={detailEl}>
      <button type="button" class="close-detail" onclick={() => (openRandomPart = null)}>✕ この部位を閉じる</button>
      <div class="card">
        <div class="card-title">{PART_SLOT_LABELS[slot]}</div>
        {@render randomOptionEditor(slot)}
        <!-- 枠は 1 装備 2 つ。**1 つ目を決めたら 2 つ目の候補を出す** —
             候補を 2 枠ぶん並べても、実際に選べるのは順番に 1 つずつ(§00 02) -->
        {#if selectedPartOrNull(slot) !== null && (selectedPartOrNull(slot)?.random_options.length ?? 0) < randomOptionSlots(slot)}
          <div class="ro-next swap-in">
            <span class="ro-next-label">
              枠 {(selectedPartOrNull(slot)?.random_options.length ?? 0) + 1}
              <span class="dim">/ {randomOptionSlots(slot)}</span>
            </span>
            {#if commonAddable(slot).length > 0}
              <div class="ro-common">
                {#each commonAddable(slot) as o (o.id)}
                  <button type="button" class="chip add" onclick={() => addRandomOption(slot, o.id)}>
                    ＋ {o.name}
                  </button>
                {/each}
              </div>
            {/if}
            {#if otherAddable(slot).length > 0}
              <div class="ro-add">
                <Picker
                  options={otherPickerOptions(slot)}
                  note="ほかの OP(同じカテゴリーは 1 つまで)"
                  placeholder="ほかの OP から選ぶ"
                  bind:value={() => "", (v) => { if (v !== "") addRandomOption(slot, v); }}
                />
              </div>
            {/if}
          </div>
        {:else}
          <p class="hint dim">枠は {randomOptionSlots(slot)} つまで。変えるときは外してから足します。</p>
        {/if}
      </div>
    </div>
  {/if}
</div>
