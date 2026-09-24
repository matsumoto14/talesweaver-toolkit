<script lang="ts">
  // 「randomOption」補正源のペイン。装備と同じ部位ドリルダウン(§09 規則 2)。
  import type {
    EquipmentPart, PartSlot, RandomOptionCandidate, RandomOptionDef, RandomOptionRank, StatPreview,
  } from "../../../api/types";
  import { listRandomOptionCandidates } from "../../../api/commands";
  import { fmtSigned, fmtSignedPct } from "../../../format";
  import type { Draft } from "../../../draft";
  import {
    neutralEquipmentPart,
    randomOptionEffectLabel, randomOptionIsApplied,
    randomOptionValue, randomOptionValueLabel,
  } from "../../../equipment";
  import {
    PART_SLOT_LABELS, RANDOM_OPTION_ALLOWED_SLOTS, RANDOM_OPTION_RANKS, RANDOM_OPTION_RANK_LABELS,
    SKILL_DEPENDENCIES, SKILL_DEPENDENCY_LABELS,
  } from "../../../labels";
  import { latest } from "../../../ui/latest.svelte";
  import { limits } from "../../../limits.svelte";
  import { tables } from "../../../tables.svelte";
  import { app, equipmentFocus } from "../../../state.svelte";
  import Picker, { type PickerOption } from "../../../ui/Picker.svelte";
  import Chip from "../../../ui/Chip.svelte";
  import Drill from "../../../ui/Drill.svelte";
  import NumberField from "../../../ui/NumberField.svelte";
  import Choose from "../../../ui/Choose.svelte";
  import type { SourceId } from "../sourceId";
  import { changed, reveal } from "../../../ui/motion.svelte";
  import { tick, untrack } from "svelte";
  import Value from "../../../ui/Value.svelte";
  import { randomOptionRecordOnlyCount } from "../summaries";
  import { t } from "../../../i18n";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, onOpenSource }: Props = $props();

  /** ランダムOP のうち記録するだけの枠数。行サブタイトルとも共有(summaries.ts) */
  const roRecordOnly = $derived(randomOptionRecordOnlyCount(preview));
  /**
   * ランダムOP の効き先ごとの合計(結果の置き場所)。同系統は足して 1 行にする。
   * 集計は Rust 側(preview.random_option_totals)。ここは効き先の日本語ラベルへの対応づけだけ
   */
  const roTotals = $derived.by<{ label: string; value: string }[]>(() => {
    const totals = preview?.random_option_totals;
    if (!totals) return [];
    const rows: { label: string; value: string }[] = [];
    const addPercent = (label: string, v: number) => {
      if (v === 0) return;
      rows.push({ label, value: fmtSignedPct(v, { max: 2 }) });
    };
    const addPoint = (label: string, v: number) => {
      if (v === 0) return;
      rows.push({ label, value: fmtSigned(v, { max: 3 }) });
    };
    for (const dep of SKILL_DEPENDENCIES) {
      addPercent(t("与ダメージ増加({dep})", { dep: SKILL_DEPENDENCY_LABELS[dep] }), totals.dependency_damage_rate[dep]);
    }
    addPercent(t("攻撃ダメージ増加"), totals.attack_damage_rate);
    addPercent(t("割合追加ダメージ"), totals.added_damage_rate);
    addPercent(t("割合追加ダメージ(物理依存)"), totals.physical_added_damage_rate);
    addPercent(t("割合追加ダメージ(魔法依存)"), totals.magic_added_damage_rate);
    addPercent(t("ダメージ増幅(物理依存)"), totals.physical_damage_amplify);
    addPercent(t("ダメージ増幅(魔法依存)"), totals.magic_damage_amplify);
    addPoint(t("命中P"), totals.accuracy_point);
    addPoint(t("回避P"), totals.evasion_point);
    if (totals.actual_delay_reduction !== 0) {
      rows.push({ label: t("中ディレイ"), value: fmtSignedPct(-totals.actual_delay_reduction, { max: 2 }) });
    }
    if (totals.min_evasion_rate !== 0) {
      rows.push({ label: t("最小回避率補正"), value: fmtSigned(totals.min_evasion_rate, { max: 2 }, "%") });
    }
    return rows;
  });

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
  const partSlotRule = (slot: PartSlot) => tables.part_slot_rules.find((r) => r.slot === slot) ?? null;
  const equippedItem = (slot: PartSlot) => {
    const itemId = selectedPartOrNull(slot)?.item_id;
    return itemId ? (app.equipmentCatalog.find((i) => i.id === itemId) ?? null) : null;
  };
  /** その部位に付けられる枠の数(domain: PartSlot::random_option_slots)。武器だけ 3 枠 */
  const randomOptionSlots = (slot: PartSlot) =>
    equippedItem(slot)?.random_option_slots ?? (selectedPartOrNull(slot)?.item_id ? 0 : (partSlotRule(slot)?.random_option_slots ?? 0));

  // --- ランダムオプション -------------------------------------------------
  // 効果値の上限は wiki の一覧表のレンジそのもの。足せる OP(同じカテゴリーは 1 部位に 1 つまで)と
  // 主軸スキルで発動するかの判定は Rust(list_random_option_candidates)。ここは並べるだけ。
  const randomOptionDef = (id: string): RandomOptionDef | undefined =>
    app.randomOptions.find((d) => d.id === id);

  /** 開いている部位。中身はその部位の行の直下に開く */
  let openRandomPart = $state<PartSlot | null>(null);

  // --- エラー帯からの「ここを開く」 -------------------------------------
  // 帯が指した部位を開き、該当 OP 行を光らせて見える位置まで送る(§00 ④)。
  let detailEl = $state<HTMLElement | null>(null);
  let focusedOptionId = $state<string | null>(null);
  let focusSeq = $state(0);
  const focusToken = (optionId: string) => (focusedOptionId === optionId ? String(focusSeq) : "");
  async function revealFocused(optionId: string) {
    await tick();
    reveal(detailEl?.querySelector(`[data-option-id="${CSS.escape(optionId)}"]`), "center");
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
  /**
   * 開いている部位に足せる OP。並び(主軸スキルの依存に合う「◯◯攻撃力が増加」が先頭)も、
   * チップで先に出すか奥に置くか(`common_choice`)も Rust が決める(ユーザー確認 2026-08-26)。
   */
  let candidates = $state<RandomOptionCandidate[]>([]);
  const candidatesLatest = latest();
  $effect(() => {
    const slot = openRandomPart;
    if (slot === null) {
      candidates = [];
      return;
    }
    // 候補を決めるのは付いている OP だけ。ここだけを読んで、ほかの編集で問い合わせ直さない
    const current = selectedPartOrNull(slot);
    const payload: EquipmentPart = {
      ...neutralEquipmentPart(),
      random_options: (current?.random_options ?? []).map((o) => ({ ...o })),
    };
    const mainSkillId = draft.mainSkillId;
    candidatesLatest.run((isCurrent) =>
      listRandomOptionCandidates(payload, slot, mainSkillId)
        .then((rows) => { if (isCurrent()) candidates = rows; })
        .catch(() => { if (isCurrent()) candidates = []; }),
    );
    return () => candidatesLatest.cancel();
  });
  const commonAddable = $derived(candidates.filter((c) => c.common_choice));
  // 主軸に合わない「よく使う OP」は消さず、ほかの OP から到達可能にする。
  const otherAddable = $derived(candidates.filter((c) => !c.common_choice));
  const otherPickerOptions = $derived<PickerOption[]>(
    otherAddable.map((d) => ({
      value: d.id,
      name: t(d.name),
      meta: t("カテゴリー{category} ・ {effect}", { category: d.category, effect: t(randomOptionEffectLabel(d.effect)) }),
    })),
  );
  function addRandomOption(slot: PartSlot, id: string) {
    if (id === "") return;
    const def = randomOptionDef(id);
    // 既定ランクは Rust が決める(一覧のいちばん上位。手持ちがそれ未満なら下げてもらう)
    if (!def || def.default_rank === null) return;
    selectedPart(slot).random_options = [
      ...selectedPart(slot).random_options,
      { option_id: id, rank: def.default_rank, value: null },
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
      <span>{t("先にこの部位の装備を登録してください。")}</span>
      <Chip onclick={() => onOpenSource("equipment")}>{t("装備へ ›")}</Chip>
    </div>
  {:else}
  {#each part.random_options as option, index (option.option_id)}
    {@const def = randomOptionDef(option.option_id)}
    {#if def}
      {@const tier = tierOf(def, option.rank)}
      <!-- 1 OP 1 行。名前 / ランク / 効果値 / 外す を列でそろえる(§00 01) -->
      <div
        class="ro-row"
        class:record-only={!randomOptionIsApplied(def.effect)}
        data-option-id={option.option_id}
        use:changed={() => focusToken(option.option_id)}
      >
        <span class="ro-name" title={t(def.name)}>{t(def.name)}</span>
        <button type="button" class="clear" onclick={() => removeRandomOption(slot, index)}>{t("外す")}</button>
        <!-- ランクは言葉なので幅は中身なり。ふだんは Special / S・真 だけ -->
        <span class="ro-rank">
          <Choose
            label={t("{name}のランク", { name: t(def.name) })}
            options={rankOptionsNow(def, option.rank)}
            bind:value={
              () => option.rank,
              (v) => setRandomOptionRank(slot, index, v as RandomOptionRank)
            }
          />
          {#if hasLowerRanks(def) && MAIN_RANKS.includes(option.rank)}
            <Chip class="quiet"
 on={rankAllOpen}
 onToggle={() => (rankAllOpen = !rankAllOpen)}
            >{rankAllOpen ? t("上位だけ") : t("下位も")}</Chip>
          {/if}
        </span>
        <NumberField
          label={t("{name}の値", { name: t(def.name) })}
          min={tier ? tier.min : 0}
          max={tier ? tier.max : limits.random_option_value_max}
          step={tier && Number.isInteger(tier.min) && Number.isInteger(tier.max) ? 1 : 0.5}
          format={tier ? () => t("wiki {min}–{max}", { min: tier.min, max: tier.max }) : undefined}
          bind:value={() => randomOptionValue(option, def), (v) => (option.value = v)}
        />
      </div>
      {#if def.note}<p class="hint dim ro-note">{t(def.note)}</p>{/if}
    {/if}
  {/each}
  {/if}
{/snippet}

<!-- 効いている量(結果)。ペイン自体が既に「ランダムOP」の名前を出しているので見出しは持たない -->
{#if roTotals.length > 0 || roRecordOnly > 0}
  <div class="eq-summary num inset">
    {#each roTotals as row (row.label)}
      <span><span class="dim">{row.label}</span> <Value value={row.value} /></span>
    {:else}
      <span class="dim">{t("計算に入る OP はまだありません")}</span>
    {/each}
  </div>
  {#if roRecordOnly > 0}
    <p class="dim tiny">{t("記録するだけの枠が {n} 件あります(発動条件付き・被ダメージ側)。", { n: roRecordOnly })}</p>
  {/if}
{/if}
<div class="card">
  <p class="hint dim">
    {t("wiki「ランダムオプション」。装備補正 9 値には乗らず、与ダメージ式のカテゴリ(依存別の与ダメージ増加・ 攻撃ダメージ増加)や命中P・回避P に直接効きます。")}<b>{t("同じカテゴリーの OP は 1 部位に 1 つだけ")}</b>{t("です(wiki: 転移)。 効果値は触らなければレンジ上限で計算します(オプション変化石で振り直せるため)。")}
    <b>{t("収録しているのは火力・命中・回避に関係する OP だけ")}</b>{t("で、HP・移動速度・経験値などは入れていません。 グレーの枠は")}<b>{t("記録するだけ")}</b>{t("(発動条件付き・未実装の概念)で計算には入りません。")}
  </p>
</div>
<!-- 部位を押すと**その行のすぐ下**に中身が開く(アコーディオン)。中身は枠 2 つ分の短い編集なので
     装備のような重ね窓にせず、押した行はその場に残す(§09 規則 3)。閉じるのは行をもう一度押す
     (閉じるボタンは置かない — ユーザー判断 2026-09-16) -->
<div class="part-split">
  <div class="part-list">
    {#each RANDOM_OPTION_ALLOWED_SLOTS as slot (slot)}
      {#if app.randomOptions.some((d) => d.slot === slot) && randomOptionSlots(slot) > 0}
        {@const count = selectedPartOrNull(slot)?.random_options.length ?? 0}
        <Drill
          open={openRandomPart === slot}
          onOpen={() => (openRandomPart = openRandomPart === slot ? null : slot)}
          detailClass="part-detail-body ro-inline"
        >
          {#snippet line()}
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
                  title={t("{name}({effect} {value})", { name: t(def.name), effect: t(randomOptionEffectLabel(def.effect)), value: randomOptionValueLabel(o, def) })}
                >{t(def.short)}</span>
              {/if}
            {/each}
            {#if count === 0}<span class="dim">{t("なし")}</span>{/if}
          </span>
          {/snippet}
          {#snippet detail()}
          <!-- 見出しは持たない。すぐ上の行が部位名を出している(§00 ②) -->
          <div class="card" bind:this={detailEl}>
            {@render randomOptionEditor(slot)}
            <!-- 枠は 1 装備 2 つ。**1 つ目を決めたら 2 つ目の候補を出す** —
                 候補を 2 枠ぶん並べても、実際に選べるのは順番に 1 つずつ(§00 02) -->
            {#if selectedPartOrNull(slot) !== null && (selectedPartOrNull(slot)?.random_options.length ?? 0) < randomOptionSlots(slot)}
              <div class="ro-next swap-in">
                <span class="ro-next-label">
                  {t("枠 {n}", { n: (selectedPartOrNull(slot)?.random_options.length ?? 0) + 1 })}
                  <span class="dim">/ {randomOptionSlots(slot)}</span>
                </span>
                {#if commonAddable.length > 0}
                  <div class="ro-common">
                    {#each commonAddable as o (o.id)}
                      <Chip class="add" onclick={() => addRandomOption(slot, o.id)}>
                        {t("＋ {name}", { name: t(o.name) })}
                      </Chip>
                    {/each}
                  </div>
                {/if}
                {#if otherAddable.length > 0}
                  <div class="ro-add">
                    <Picker
                      label={t("足すランダム OP")}
                      options={otherPickerOptions}
                      note={t("ほかの OP(同じカテゴリーは 1 つまで)")}
                      menu
                      bind:value={() => "", (v) => { if (v !== "") addRandomOption(slot, v); }}
                    />
                  </div>
                {/if}
              </div>
            {:else}
              <p class="hint dim">{t("枠は {n} つまで。変えるときは外してから足します。", { n: randomOptionSlots(slot) })}</p>
            {/if}
          </div>
          {/snippet}
        </Drill>
      {/if}
    {/each}
  </div>
</div>
