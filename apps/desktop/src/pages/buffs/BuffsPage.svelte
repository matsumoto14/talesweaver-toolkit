<script lang="ts">
  import {
    createBuffSet, deleteBuffSet, duplicateBuffSet, errorMessage, previewDefense,
    previewEffectiveStats, summarizeBuffSelection, updateBuffSet,
  } from "../../api/commands";
  import type {
    BuffChoice, BuffDefinition, BuffOrigin, BuffPurpose, BuffSet, BuffTarget, DamageCategory,
    CategoryTrace, DefenseProfile, EffectiveStats, StatKind,
  } from "../../api/types";
  import {
    isBlocked, isChoiceValue, isFixedValue, isPercentLayer, isRecordOnly, isUserSelectedTarget,
    toggleBuff, userInputRange,
  } from "../../buffs";
  import { formatLayerValue } from "../../format";
  import { singleEffectLabel } from "../../characterSkills";
  import { STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
  import { app, focusCharacterSource, payloadOf, refreshEvaluation, syncCalcBuffs, selectedCharacter } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import Icon from "../../ui/Icon.svelte";

  const PURPOSES: { id: BuffPurpose; label: string; description: string }[] = [
    { id: "stats", label: "ステータスを上げたい", description: "能力値が伸びる効果" },
    { id: "damage", label: "火力を上げたい", description: "攻撃ダメージ効果を持つバフ" },
    { id: "durability", label: "耐久を上げたい", description: "受けるダメージや生存力に関わる効果" },
  ];
  const ORIGIN_LABELS: Record<BuffOrigin, string> = {
    item: "アイテム", event: "イベント", club: "クラブ", skill: "スキル",
    rune: "ルーン", soul_link: "ソウルリンク", battle_state: "戦闘中", minigame: "ミニゲーム",
  };
  type DamageGroup = "general" | "isabel" | "japan" | "other";
  const DAMAGE_GROUPS: { id: DamageGroup; label: string }[] = [
    { id: "general", label: "一般" },
    { id: "isabel", label: "イザベル" },
    { id: "japan", label: "日本独自" },
    { id: "other", label: "その他" },
  ];

  let selectedId = $state<number | null>(null);
  let newName = $state("");
  let saving = $state(false);
  let persisting = false;
  let pendingPersist: BuffSet | null = null;
  let confirmDeleteId = $state<number | null>(null);
  let confirmDeleteTimer: ReturnType<typeof setTimeout> | null = null;
  let activePurpose = $state<BuffPurpose>("stats");
  let activeDamageGroup = $state<DamageGroup>("general");
  let damageSummary = $state<CategoryTrace[]>([]);
  let statBefore = $state<EffectiveStats | null>(null);
  let statAfter = $state<EffectiveStats | null>(null);
  let defenseBefore = $state<DefenseProfile | null>(null);
  let defenseAfter = $state<DefenseProfile | null>(null);
  let summaryLoading = $state(false);
  let editingBuffId = $state<string | null>(null);
  let draftChoice = $state<BuffChoice | null>(null);
  let summarySeq = 0;
  const selected = $derived(app.buffSets.find((set) => set.id === selectedId) ?? app.buffSets[0] ?? null);
  const activePurposeMeta = $derived(PURPOSES.find((purpose) => purpose.id === activePurpose) ?? PURPOSES[0]);
  const matchesPurpose = (def: BuffDefinition, purpose: BuffPurpose) =>
    purpose === "damage" ? def.damage_effects.length > 0 : def.purposes.includes(purpose);
  const damageCategories = (def: BuffDefinition): DamageCategory[] =>
    def.damage_effects.flatMap((effect) => effect !== "record_only" && "damage" in effect ? [effect.damage.category] : []);
  const matchesDamageGroup = (def: BuffDefinition, group: DamageGroup) => {
    const categories = damageCategories(def);
    if (group === "general") return categories.includes("attack_damage_general");
    if (group === "isabel") return categories.includes("attack_damage_isabel");
    if (group === "japan") return categories.includes("attack_damage_japan");
    return categories.some((category) => ![
      "attack_damage_general", "attack_damage_isabel", "attack_damage_japan",
    ].includes(category));
  };
  const activeDefinitions = $derived(app.catalog.filter((def) =>
    matchesPurpose(def, activePurpose) && (activePurpose !== "damage" || matchesDamageGroup(def, activeDamageGroup))
  ));

  $effect(() => {
    if (selectedId === null && app.buffSets.length > 0) selectedId = app.buffSets[0].id;
  });

  $effect(() => {
    const choices = selected ? JSON.parse(JSON.stringify(selected.choices)) : null;
    const character = selectedCharacter();
    const signature = `${selected?.id ?? "none"}:${JSON.stringify(choices)}:${character?.id ?? "none"}`;
    void signature;
    const seq = ++summarySeq;
    summaryLoading = true;
    void (async () => {
      if (!choices) {
        damageSummary = []; statBefore = null; statAfter = null; defenseBefore = null; defenseAfter = null;
        summaryLoading = false;
        return;
      }
      try {
        const damage = await summarizeBuffSelection(choices);
        let beforeStats: EffectiveStats | null = null;
        let afterStats: EffectiveStats | null = null;
        let beforeDefense: DefenseProfile | null = null;
        let afterDefense: DefenseProfile | null = null;
        if (character) {
          const draft = payloadOf(character);
          const [basePreview, buffPreview, baseDefense, buffDefense] = await Promise.all([
            previewEffectiveStats(draft.base_stats, draft.stat_sources, draft.equipment, draft.common_skills, draft.awakening, draft.main_skill_id),
            previewEffectiveStats(draft.base_stats, draft.stat_sources, draft.equipment, draft.common_skills, draft.awakening, draft.main_skill_id, choices),
            previewDefense(draft), previewDefense(draft, choices),
          ]);
          beforeStats = basePreview.stats; afterStats = buffPreview.stats;
          beforeDefense = baseDefense; afterDefense = buffDefense;
        }
        if (seq !== summarySeq) return;
        damageSummary = damage; statBefore = beforeStats; statAfter = afterStats;
        defenseBefore = beforeDefense; defenseAfter = afterDefense;
      } catch (e) {
        if (seq === summarySeq) reportError(errorMessage(e));
      } finally {
        if (seq === summarySeq) summaryLoading = false;
      }
    })();
  });

  function replaceSet(set: BuffSet) {
    const index = app.buffSets.findIndex((item) => item.id === set.id);
    if (index >= 0) app.buffSets[index] = set;
    else app.buffSets.push(set);
    selectedId = set.id;
  }

  async function create() {
    if (!newName.trim() || saving) return;
    saving = true;
    try {
      replaceSet(await createBuffSet(newName, { choices: [] }));
      newName = "";
    } catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  async function persist(set: BuffSet) {
    // 数値欄は入力のたびに値が届く。通信中の入力を捨てず、最新のセット全体だけを
    // 続けて保存する。応答が古い内容で画面を巻き戻さないよう、先にローカルへ反映する。
    if (saving && !persisting) return;
    replaceSet(set);
    pendingPersist = JSON.parse(JSON.stringify(set));
    if (persisting) return;
    persisting = true;
    saving = true;
    try {
      while (pendingPersist) {
        const next = pendingPersist;
        pendingPersist = null;
        const saved = await updateBuffSet(next.id, next.name, next.choices);
        if (pendingPersist === null) replaceSet(saved);
        const affected = app.characters.filter((character) => character.default_buff_set_id === saved.id);
        await Promise.all(affected.map(refreshEvaluation));
      }
    }
    catch (e) { pendingPersist = null; reportError(errorMessage(e)); }
    finally { persisting = false; saving = false; }
  }

  async function toggle(def: BuffDefinition) {
    if (!selected || saving) return;
    const next: BuffSet = JSON.parse(JSON.stringify(selected));
    next.choices.choices = toggleBuff(next.choices.choices, def, !next.choices.choices.some((c) => c.buff_id === def.id));
    await persist(next);
  }

  async function duplicate() {
    if (!selected || saving) return;
    saving = true;
    try { replaceSet(await duplicateBuffSet(selected.id)); }
    catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  function requestRemove() {
    if (!selected || saving) return;
    confirmDeleteId = selected.id;
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = setTimeout(() => (confirmDeleteId = null), 4000);
  }

  async function remove() {
    if (!selected || saving || confirmDeleteId !== selected.id) return;
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = null;
    confirmDeleteId = null;
    saving = true;
    try {
      const deletedId = selected.id;
      await deleteBuffSet(deletedId);
      app.buffSets = app.buffSets.filter((set) => set.id !== deletedId);
      const affectedIds = new Set(
        app.characters.filter((character) => character.default_buff_set_id === deletedId).map((character) => character.id),
      );
      app.characters = app.characters.map((character) =>
        affectedIds.has(character.id) ? { ...character, default_buff_set_id: null } : character,
      );
      selectedId = app.buffSets[0]?.id ?? null;
      if (app.calcBuffSetId === deletedId) syncCalcBuffs(selectedCharacter());
      await Promise.all(app.characters.filter((character) => affectedIds.has(character.id)).map(refreshEvaluation));
    } catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  const on = (def: BuffDefinition) => selected?.choices.choices.some((choice) => choice.buff_id === def.id) ?? false;
  const needsInput = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null;
  const exclusive = (def: BuffDefinition) => def.exclusive_slots.length > 0 ? def.exclusive_slots.join(" / ") : "独立";
  const statOptions = STAT_KINDS.map((kind) => ({ value: kind, label: STAT_LABELS[kind] }));

  function targetLabel(target: BuffTarget): string {
    if (target === "all_stats") return "全ステータス";
    if (target === "user_selected") return "選択したステータス";
    if ("stat" in target) return STAT_LABELS[target.stat];
    return target.stats.map((stat) => STAT_LABELS[stat]).join(" / ");
  }

  function effectSummary(def: BuffDefinition): string {
    if (isFixedValue(def.value)) return `${STAT_LAYER_LABELS[def.layer]} ${formatLayerValue(def.layer, def.value.fixed)}`;
    if (isChoiceValue(def.value)) return def.value.choice.map((value) => formatLayerValue(def.layer, value)).join(" / ");
    const range = userInputRange(def.value);
    if (range) return `${STAT_LAYER_LABELS[def.layer]} ${formatLayerValue(def.layer, range.min)}〜${formatLayerValue(def.layer, range.max)}`;
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    return damage.join(" ・ ") || "効果を記録";
  }

  function buffTooltip(def: BuffDefinition, blocked: boolean): string {
    const purposes = def.purposes.map((purpose) => PURPOSES.find((item) => item.id === purpose)?.label ?? purpose).join(" / ");
    const lines = [def.name, `目的: ${purposes}`, `種類: ${ORIGIN_LABELS[def.origin]}`, `主効果: ${effectSummary(def)}`, `対象: ${targetLabel(def.target)}`];
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    if (damage.length > 0 && !isRecordOnly(def.value)) lines.push(`追加効果: ${damage.join(" ・ ")}`);
    lines.push(`重複: ${exclusive(def)}`);
    if (def.note) lines.push(`補足: ${def.note}`);
    if (blocked) lines.unshift("選択不可: 同じ重複枠のバフが選ばれています", "");
    return lines.join("\n");
  }

  const purposeSelectedCount = (purpose: BuffPurpose) =>
    app.catalog.filter((def) => matchesPurpose(def, purpose) && on(def)).length;
  const damageGroupSelectedCount = (group: DamageGroup) =>
    app.catalog.filter((def) => matchesPurpose(def, "damage") && matchesDamageGroup(def, group) && on(def)).length;

  function choosePurpose(purpose: BuffPurpose) {
    cancelInput();
    activePurpose = purpose;
  }

  function chooseDamageGroup(group: DamageGroup) {
    cancelInput();
    activeDamageGroup = group;
  }

  const selectedChoice = (buffId: string) => selected?.choices.choices.find((choice) => choice.buff_id === buffId) ?? null;
  const formatDelta = (value: number, digits = 0) => `${value >= 0 ? "+" : ""}${value.toFixed(digits)}`;

  function cardEffectText(def: BuffDefinition): string {
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    const primary = activePurpose === "damage" && damage.length > 0 ? damage.join(" ・ ") : effectSummary(def);
    if (!needsInput(def)) return primary;
    const choice = selectedChoice(def.id);
    if (!choice) return `${primary} ・ クリックして設定`;
    if (activePurpose === "damage") return `${primary} ・ クリックで解除`;
    const parts: string[] = [];
    if (isUserSelectedTarget(def.target) && choice.stat) parts.push(STAT_LABELS[choice.stat]);
    if (isChoiceValue(def.value)) {
      const value = def.value.choice[choice.choice_index ?? 0];
      if (value !== undefined) parts.push(formatLayerValue(def.layer, value));
    } else if (userInputRange(def.value)) {
      parts.push(formatLayerValue(def.layer, choice.value ?? def.default_value ?? userInputRange(def.value)!.min));
    }
    return `${parts.join(" ") || primary} ・ クリックで解除`;
  }

  async function activate(def: BuffDefinition) {
    if (!needsInput(def)) {
      await toggle(def);
      return;
    }
    if (on(def)) {
      await toggle(def);
      return;
    }
    const initial = toggleBuff([], def, true).find((choice) => choice.buff_id === def.id) ?? null;
    editingBuffId = initial ? def.id : null;
    draftChoice = initial ? JSON.parse(JSON.stringify(initial)) : null;
  }

  function editDraft(edit: (choice: BuffChoice) => void) {
    if (!draftChoice) return;
    const next: BuffChoice = JSON.parse(JSON.stringify(draftChoice));
    edit(next);
    draftChoice = next;
  }

  function cancelInput() {
    editingBuffId = null;
    draftChoice = null;
  }

  async function confirmInput(def: BuffDefinition) {
    if (!selected || !draftChoice || editingBuffId !== def.id || saving) return;
    const next: BuffSet = JSON.parse(JSON.stringify(selected));
    next.choices.choices = toggleBuff(next.choices.choices, def, true);
    const index = next.choices.choices.findIndex((choice) => choice.buff_id === def.id);
    if (index < 0) return;
    next.choices.choices[index] = JSON.parse(JSON.stringify(draftChoice));
    cancelInput();
    await persist(next);
  }

</script>

<div class="buff-page">
  <aside class="sets">
    <div class="bar">バフセット <span use:bump={() => app.buffSets.length}>{app.buffSets.length}</span></div>
    <div class="create-row">
      <input bind:value={newName} disabled={saving} placeholder="セット名" aria-label="新しいバフセット名" onkeydown={(e) => e.key === "Enter" && create()} />
      <button class="btn primary" disabled={!newName.trim() || saving} onclick={create}>作成</button>
    </div>
    <div class="set-list">
      {#each app.buffSets as set (set.id)}
        <button class:on={selected?.id === set.id} disabled={saving} onclick={() => (selectedId = set.id)}>
          <span>{set.name}</span><small class="num">{set.choices.choices.length}</small>
        </button>
      {/each}
      {#if app.buffSets.length === 0}<p>セットを作ると、キャラや計算で使えます。</p>{/if}
    </div>
  </aside>

  <section class="catalog">
    <div class="bar">セットに入れるバフ</div>
    {#if selected}
      <div class="set-tools">
        <input value={selected.name} disabled={saving} aria-label="バフセット名" onchange={(e) => persist({ ...selected, name: e.currentTarget.value })} />
        <button class="btn" onclick={duplicate}>複製</button>
        <button class="btn danger delete-set" disabled={saving} onclick={requestRemove}>削除</button>
        {#if confirmDeleteId === selected.id}
          <div class="delete-confirm" role="alert">
            <span>このセットを削除します</span>
            <button class="btn danger" disabled={saving} onclick={remove}>削除する</button>
            <button class="btn" disabled={saving} onclick={() => (confirmDeleteId = null)}>やめる</button>
          </div>
        {/if}
      </div>
      <div class="groups">
        <div class="category-switch" role="tablist" aria-label="伸ばしたい効果">
          {#each PURPOSES as purpose (purpose.id)}
            {@const definitions = app.catalog.filter((def) => matchesPurpose(def, purpose.id))}
            {@const picked = purposeSelectedCount(purpose.id)}
            <button
              class="chip category-tab"
              class:on={activePurpose === purpose.id}
              role="tab"
              aria-selected={activePurpose === purpose.id}
              onclick={() => choosePurpose(purpose.id)}
            >
              <span>{purpose.label}</span>
              <span class="group-count num" use:bump={() => picked}>{picked}/{definitions.length}</span>
            </button>
          {/each}
        </div>
        <section class="buff-group" use:flash={() => `${activePurpose}:${activeDamageGroup}`}>
          <div class="group-summary">
            <span class="group-copy"><strong>{activePurposeMeta.label}</strong><small>{activePurposeMeta.description}</small></span>
            {#if activePurpose === "stats"}
              <button class="guide-link" type="button" onclick={() => focusCharacterSource("commonSkill")}>
                アンリーシュはキャラ設定 <span aria-hidden="true">›</span>
              </button>
            {/if}
          </div>
          {#if activePurpose === "damage"}
            <div class="damage-switch" role="tablist" aria-label="攻撃ダメージの種類">
              {#each DAMAGE_GROUPS as group (group.id)}
                {@const definitions = app.catalog.filter((def) => matchesPurpose(def, "damage") && matchesDamageGroup(def, group.id))}
                {@const picked = damageGroupSelectedCount(group.id)}
                <button
                  class="chip damage-tab"
                  class:on={activeDamageGroup === group.id}
                  role="tab"
                  aria-selected={activeDamageGroup === group.id}
                  onclick={() => chooseDamageGroup(group.id)}
                >
                  <span>{group.label}</span>
                  <span class="group-count num" use:bump={() => picked}>{picked}/{definitions.length}</span>
                </button>
              {/each}
            </div>
          {/if}
          <div class="chips">
            {#each activeDefinitions as def (def.id)}
              {@const blocked = !on(def) && isBlocked(selected.choices.choices, app.catalog, def)}
              {@const editing = editingBuffId === def.id && draftChoice !== null}
              <div class="buff-option" class:on={on(def)} class:editing use:flash={() => (on(def) ? "on" : "off")}>
                <span class="buff-icon"><Icon kind="buff" id={def.id} size={28} label={def.name} /></span>
                {#if editing && draftChoice}
                  <div class="inline-editor">
                    <div class="edit-copy"><strong>{def.name}</strong><small>値を決めて確定</small></div>
                    <div class="choice-editor">
                      {#if isUserSelectedTarget(def.target)}
                        <StepSelect label="対象ステ" options={statOptions} cols={4} bind:value={() => draftChoice!.stat ?? STAT_KINDS[0], (value) => editDraft((item) => (item.stat = value as StatKind))} />
                      {/if}
                      {#if isChoiceValue(def.value)}
                        {@const options = def.value.choice.map((value, index) => ({ value: String(index), label: formatLayerValue(def.layer, value) }))}
                        <StepSelect label="段階" {options} bind:value={() => String(draftChoice!.choice_index ?? 0), (value) => editDraft((item) => (item.choice_index = Number(value)))} />
                      {/if}
                      {#if userInputRange(def.value)}
                        {@const range = userInputRange(def.value)!}
                        {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                        <StatInput label={def.id === "club_effect" ? "クラブ効果" : (isPercentLayer(def.layer) ? "値 (%)" : "値")} min={range.min * scale} max={range.max * scale} bind:value={() => (draftChoice!.value ?? def.default_value ?? range.min) * scale, (value) => editDraft((item) => (item.value = value / scale))} />
                      {/if}
                    </div>
                    <div class="edit-actions">
                      <button type="button" class="btn" onclick={cancelInput}>キャンセル</button>
                      <button type="button" class="btn primary" onclick={() => confirmInput(def)}>確定</button>
                    </div>
                  </div>
                {:else}
                  <button
                    class="buff-toggle"
                    disabled={blocked || saving}
                    onclick={() => activate(def)}
                    title={buffTooltip(def, blocked)}
                    aria-label={`${def.name}。${cardEffectText(def)}`}
                  >
                    <span class="chip-copy">
                      <strong>{def.name}</strong><small class:input-hint={needsInput(def) && !on(def)}>{cardEffectText(def)}</small>
                      <span class="origin-badge">{ORIGIN_LABELS[def.origin]}</span>
                    </span>
                    <span class="info" aria-hidden="true">ⓘ</span>
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        </section>
      </div>
    {:else}
      <div class="empty">左でバフセットを作ってください。</div>
    {/if}
  </section>

  <aside class="summary">
    <div class="bar">現在の効果</div>
    {#if selected}
      <div class="count num" use:bump={() => selected.choices.choices.length}>{selected.choices.choices.length}<small>件 ON</small></div>
      {#if summaryLoading}<p class="summary-note">集計しています…</p>{/if}
      <div class="summary-block" use:flash={() => JSON.stringify(statAfter)}>
        <strong>ステータス</strong>
        {#if statBefore && statAfter}
          <div class="summary-grid stats-grid">
            {#each STAT_KINDS as kind}
              {@const delta = statAfter[kind] - statBefore[kind]}
              <span>{STAT_LABELS[kind]}</span><span class:positive={delta > 0} class="num" use:bump={() => delta}>{formatDelta(delta)}</span>
            {/each}
          </div>
        {:else}<p>キャラを選ぶと、実際に何点伸びるか表示します。</p>{/if}
      </div>
      <div class="summary-block" use:flash={() => JSON.stringify(damageSummary)}>
        <strong>攻撃ダメージ</strong>
        {#if damageSummary.length > 0}
          <div class="damage-list">
            {#each damageSummary as row (row.category)}
              <div><span>{row.label}</span><span class="num positive" use:bump={() => row.value}>+{(row.value * 100).toFixed(0)}%</span></div>
              {#if row.raw > row.value}<small>上限で {((row.raw - row.value) * 100).toFixed(0)}% は未反映</small>{/if}
            {/each}
          </div>
        {:else}<p>選択中のバフによる攻撃ダメージ増加はありません。</p>{/if}
      </div>
      <div class="summary-block" use:flash={() => JSON.stringify(defenseAfter)}>
        <strong>耐久</strong>
        {#if defenseBefore && defenseAfter}
          <div class="summary-grid">
            <span>物理防御力</span><span class="num">{formatDelta(defenseAfter.physical_defense - defenseBefore.physical_defense)}</span>
            <span>魔法防御力</span><span class="num">{formatDelta(defenseAfter.magic_defense - defenseBefore.magic_defense)}</span>
            <span>複合防御力</span><span class="num">{formatDelta(defenseAfter.composite_defense - defenseBefore.composite_defense)}</span>
            <span>コンボ回避</span><span class="num">{formatDelta((defenseAfter.combo_evasion - defenseBefore.combo_evasion) * 100, 1)}%</span>
          </div>
        {:else}<p>キャラを選ぶと、防御力などの変化を表示します。</p>{/if}
        {#if selected.choices.choices.some((choice) => choice.buff_id === "boiled_mimic")}
          <div class="unmodeled">被ダメージ -30%（記録のみ・耐久計算には未反映）</div>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<svelte:window onkeydown={(event) => { if (event.key === "Escape") cancelInput(); }} />

<style>
  .buff-page { flex: 1; min-height: 0; display: grid; grid-template-columns: 250px minmax(360px, 1fr) 290px; gap: 10px; padding: 12px; overflow: auto; }
  .sets, .catalog, .summary { min-height: 0; background: var(--bg-panel); border: 1px solid var(--border); border-radius: var(--r-panel); overflow: hidden; box-shadow: inset 0 1px #fff; }
  .catalog { display: flex; flex-direction: column; }
  .bar { height: 32px; padding: 0 12px; display: flex; align-items: center; justify-content: space-between; background: var(--head-bar); color: #fff; font-size: 11px; font-weight: 700; letter-spacing: .08em; }
  .create-row, .set-tools { display: flex; gap: 7px; padding: 10px; border-bottom: 1px solid var(--border-soft); }
  .set-tools { position: relative; }
  input { min-width: 0; height: 30px; flex: 1; border: 1px solid var(--border); border-radius: var(--r-inset); padding: 0 9px; background: var(--bg-field); color: var(--fg); }
  .set-list { padding: 8px; display: flex; flex-direction: column; gap: 4px; }
  .set-list > button { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px 9px; border: 1px solid transparent; border-radius: var(--r-inset); text-align: left; }
  .set-list > button.on { background: var(--sel-card); border-color: var(--sel-bd); }
  .set-list small { margin-left: auto; min-width: 2ch; text-align: right; }
  .set-list p, .empty { margin: 12px; color: var(--fg-muted); font-size: 11px; }
  .groups { flex: 1; min-height: 0; padding: 8px; display: flex; flex-direction: column; gap: 7px; overflow: hidden; }
  .category-switch { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px; }
  .category-tab { min-width: 0; width: 100%; justify-content: flex-start; border-radius: var(--r-inset); }
  .category-tab > span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .buff-group { flex: 1; min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--border-soft); border-radius: var(--r-panel); background: var(--surface-inset); box-shadow: inset 0 1px #fff; overflow: hidden; }
  .group-summary { min-height: 41px; padding: 6px 9px; display: flex; align-items: center; gap: 10px; }
  .group-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .group-summary small { color: var(--fg-muted); font-size: 9px; }
  .group-count { margin-left: auto; min-width: 5ch; color: inherit; text-align: right; font-size: 9px; }
  .damage-switch { padding: 0 7px 7px; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 5px; }
  .damage-tab { min-width: 0; width: 100%; justify-content: flex-start; border-radius: var(--r-inset); }
  .guide-link { flex: none; padding: 2px 4px; color: var(--fg-muted); font-size: 9px; text-decoration: underline; text-underline-offset: 2px; }
  .guide-link:hover { color: var(--accent-hover); }
  .chips { flex: 1; min-height: 0; padding: 7px; display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); grid-auto-rows: max-content; gap: 6px; border-top: 1px solid var(--border-soft); align-content: start; align-items: start; overflow-y: auto; scrollbar-gutter: stable; }
  .buff-option { position: relative; min-width: 0; border: 1px solid var(--border); border-radius: var(--r-panel); background: var(--bg-field); overflow: hidden; }
  .buff-option.on { border-color: var(--sel-bd); background: var(--sel-card); box-shadow: inset 0 0 0 1px var(--sel-bd); }
  .buff-option.editing { border-color: var(--sim); background: var(--state-temp-bg); box-shadow: inset 0 0 0 1px var(--sim); }
  .buff-icon { position: absolute; z-index: 1; top: 16px; left: 7px; width: 28px; height: 28px; transition: top .2s ease; }
  .buff-option.editing .buff-icon { top: 9px; }
  .buff-toggle { width: 100%; min-height: 60px; padding: 6px 7px 6px 48px; display: flex; align-items: center; gap: 7px; border: 0; background: transparent; color: var(--fg); text-align: left; }
  .buff-toggle:disabled { opacity: .45; }
  .inline-editor { min-height: 60px; padding: 7px; animation: buff-edit-in .2s ease-out; }
  .edit-copy { min-height: 35px; margin-left: 41px; display: flex; flex-direction: column; justify-content: center; }
  .edit-copy strong { font-size: 10px; }
  .edit-copy small { color: var(--sim-fg); font-size: 9px; }
  .choice-editor { margin-top: 5px; padding: 7px; display: flex; flex-direction: column; gap: 7px; border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--bg-field); box-shadow: inset 0 1px #fff; }
  .edit-actions { margin-top: 7px; display: flex; justify-content: flex-end; gap: 5px; }
  .input-hint { color: var(--sim-fg) !important; }
  @keyframes buff-edit-in { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: translateY(0); } }
  @media (prefers-reduced-motion: reduce) { .inline-editor { animation: none; } .buff-icon { transition: none; } }
  .chip-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .chip-copy strong, .chip-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip-copy strong { font-size: 10px; }
  .chips small { color: var(--fg-muted); font-size: 9px; }
  .origin-badge { align-self: flex-start; margin-top: 3px; padding: 1px 6px; border: 1px solid var(--border-soft); border-radius: var(--r-pill); background: var(--surface-inset); color: var(--fg-muted); font-size: 8.5px; line-height: 1.4; white-space: nowrap; }
  .info { width: 14px; flex: none; color: var(--fg-muted); font-size: 11px; text-align: center; }
  .summary { background: var(--bg-raised); }
  .count { margin: 12px; padding: 13px; border: 1px solid var(--border); border-radius: var(--r-inset); background: var(--surface-inset); box-shadow: inset 0 1px #fff; font-size: 27px; font-weight: 700; }
  .count small { margin-left: 6px; font-family: var(--font); font-size: 10px; font-weight: 500; }
  .summary-note { margin: 0 12px 8px; color: var(--fg-muted); font-size: 9px; }
  .summary-block { margin: 0 12px 8px; padding: 9px; border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--surface-inset); box-shadow: inset 0 1px #fff; }
  .summary-block > strong { display: block; margin-bottom: 6px; font-size: 10px; }
  .summary-block p { margin: 0; color: var(--fg-muted); font-size: 9px; }
  .summary-grid { display: grid; grid-template-columns: minmax(0, 1fr) 64px; gap: 3px 8px; font-size: 9px; }
  .summary-grid > span:nth-child(even) { min-width: 64px; text-align: right; font-variant-numeric: tabular-nums; }
  .positive { color: var(--good); font-weight: 700; }
  .damage-list { display: flex; flex-direction: column; gap: 4px; }
  .damage-list div { display: flex; align-items: baseline; gap: 8px; font-size: 9px; }
  .damage-list div > span:last-child { margin-left: auto; min-width: 54px; text-align: right; }
  .damage-list small { color: var(--danger); font-size: 8.5px; text-align: right; }
  .unmodeled { margin-top: 7px; padding: 5px 6px; border: 1px dashed var(--border); border-radius: var(--r-inset); color: var(--fg-muted); font-size: 8.5px; }
  .danger { color: var(--danger); }
  .delete-set { width: 58px; flex: none; }
  .delete-confirm {
    position: absolute; z-index: 5; top: 43px; right: 10px;
    display: flex; align-items: center; gap: 7px; padding: 8px;
    border: 1px solid var(--danger); border-radius: var(--r-panel);
    background: var(--bg-field); box-shadow: var(--shadow-pop);
    color: var(--fg); font-size: 10px; white-space: nowrap;
  }
  @media (max-width: 1100px) { .category-switch { grid-template-columns: 1fr; } }
  @media (max-width: 950px) { .buff-page { grid-template-columns: 220px minmax(320px, 1fr); } .summary { grid-column: 1 / -1; } }
</style>
