<script lang="ts">
  // 右カラム(設定)。アコーディオン、1つずつ開く。専門用語(固定値/最終固定値/倍率A/B)は
  // ここに出さない(内訳は CharacterData の折りたたみのみ)。
  import { PET_SKILL_TIER_LABELS, STAT_KINDS, STAT_LABELS } from "../../labels";
  import { fmtInt, formatLayerValue } from "../../format";
  import type {
    BuffChoice, BuffDefinition, BuffTarget, BuffValue, PetSkillTier, StatKind, StatLayer, StatPreview,
  } from "../../api/types";
  import { limits } from "../../limits.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import Select from "../../ui/Select.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import type { Draft } from "./draft";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    catalog: BuffDefinition[];
  }
  let { draft, preview, catalog }: Props = $props();

  let openGroup = $state<"permanent" | "buffs" | "skills" | "adjustments" | null>(null);
  function toggleGroup(g: "permanent" | "buffs" | "skills" | "adjustments") {
    openGroup = openGroup === g ? null : g;
  }

  const PET_TIERS: PetSkillTier[] = ["basic", "true_lv1", "true_lv2", "true_lv3", "true_lv4"];
  const petSkillOptions = [
    { value: "", label: "なし" },
    ...PET_TIERS.map((t) => ({ value: t, label: PET_SKILL_TIER_LABELS[t] })),
  ];
  // Select は value:string 必須。stat_sources.pet_skills は PetSkillTier | null なので橋渡しする。
  const petSkillValue = (k: StatKind) => draft.statSources.pet_skills[k] ?? "";
  const setPetSkillValue = (k: StatKind, v: string) => {
    draft.statSources.pet_skills[k] = (v === "" ? null : v) as PetSkillTier | null;
  };

  const statOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));

  // --- バフ共通ロジック(旧 CharacterDetail.svelte を踏襲) -----------------------

  const isChoiceValue = (v: BuffValue): v is { choice: number[] } =>
    typeof v === "object" && v !== null && "choice" in v;
  const userInputRange = (v: BuffValue): { min: number; max: number } | null =>
    typeof v === "object" && v !== null && "user_input" in v ? v.user_input : null;
  const isUserSelectedTarget = (t: BuffTarget): boolean => t === "user_selected";
  const isPercentLayer = (layer: StatLayer): boolean => layer === "percent_of_base" || layer === "multiplier_b";

  const isConsumable = (d: BuffDefinition): boolean => d.group === "consumable";
  const isCharacterSkillFor = (d: BuffDefinition, gameCharacterId: string): boolean =>
    typeof d.group === "object" && "character_skill" in d.group
    && d.group.character_skill.game_character_id === gameCharacterId;
  const isAllySkill = (d: BuffDefinition): boolean => d.group === "ally_skill";
  const isFixedValue = (v: BuffValue): v is { fixed: number } =>
    typeof v === "object" && v !== null && "fixed" in v;

  const consumableBuffs = $derived(catalog.filter(isConsumable));
  // 「このキャラのスキル」(自身のみ)と「味方から受けるスキル」(誰でもON可)を分けて表示する。
  const ownSkillBuffs = $derived(catalog.filter((d) => isCharacterSkillFor(d, draft.gameCharacterId)));
  const allySkillBuffs = $derived(catalog.filter(isAllySkill));
  const skillBuffs = $derived([...ownSkillBuffs, ...allySkillBuffs]);

  const buffChoiceFor = (buffId: string) => draft.statSources.buffs.choices.find((c) => c.buff_id === buffId);

  function usedExclusiveSlots(excludingBuffId: string): Set<string> {
    const slots = new Set<string>();
    for (const c of draft.statSources.buffs.choices) {
      if (c.buff_id === excludingBuffId) continue;
      const d = catalog.find((x) => x.id === c.buff_id);
      if (d) for (const s of d.exclusive_slots) slots.add(s);
    }
    return slots;
  }
  function isBlocked(def: BuffDefinition): boolean {
    if (def.exclusive_slots.length === 0) return false;
    const used = usedExclusiveSlots(def.id);
    return def.exclusive_slots.some((s) => used.has(s));
  }

  function toggleBuff(def: BuffDefinition, checked: boolean) {
    if (checked) {
      const choice: BuffChoice = { buff_id: def.id, stat: null, choice_index: null, value: null };
      if (isUserSelectedTarget(def.target)) choice.stat = STAT_KINDS[0];
      if (isChoiceValue(def.value)) choice.choice_index = 0;
      if (userInputRange(def.value)) choice.value = def.default_value ?? 0;
      draft.statSources.buffs.choices = [...draft.statSources.buffs.choices, choice];
    } else {
      draft.statSources.buffs.choices = draft.statSources.buffs.choices.filter((c) => c.buff_id !== def.id);
    }
  }

  // --- 要約行 ---------------------------------------------------------------

  const permanentSummary = $derived.by(() => {
    const parts: string[] = [];
    const petCount = STAT_KINDS.filter((k) => draft.statSources.pet_skills[k] !== null).length;
    if (petCount > 0) parts.push(`ペット ${petCount} 種`);
    const runeTotal = STAT_KINDS.reduce((sum, k) => sum + draft.statSources.rune_levels[k], 0);
    if (runeTotal > 0) parts.push(`ルーン 合計+${runeTotal}`);
    const crownTotal = STAT_KINDS.reduce((sum, k) => sum + draft.statSources.crown[k], 0);
    if (crownTotal > 0) parts.push(`クラウン 合計+${crownTotal}`);
    const relicTotal = STAT_KINDS.reduce((sum, k) => sum + draft.statSources.sacred_relic[k] * 10, 0);
    if (relicTotal > 0) parts.push(`聖物 合計+${relicTotal}`);
    return parts.length === 0 ? "未設定(中立値で計算)" : parts.join(" ・ ");
  });

  const buffsSummary = $derived.by(() => {
    const choices = draft.statSources.buffs.choices.filter((c) => consumableBuffs.some((d) => d.id === c.buff_id));
    if (choices.length === 0) return "未設定(中立値で計算)";
    const names = choices.map((c) => catalog.find((d) => d.id === c.buff_id)?.name ?? c.buff_id);
    return `${choices.length}件選択: ${names.join("、")}`;
  });

  const skillsSummary = $derived.by(() => {
    const choices = draft.statSources.buffs.choices.filter((c) => skillBuffs.some((d) => d.id === c.buff_id));
    if (choices.length === 0) return "未設定(中立値で計算)";
    const names = choices.map((c) => catalog.find((d) => d.id === c.buff_id)?.name ?? c.buff_id);
    return `${choices.length}件選択: ${names.join("、")}`;
  });

  const adjustmentsSummary = $derived.by(() => {
    const parts = STAT_KINDS.filter(
      (k) => draft.statSources.adjustments[k].add !== 0 || draft.statSources.adjustments[k].pin !== null,
    ).map((k) => {
      const a = draft.statSources.adjustments[k];
      const bits: string[] = [];
      if (a.add !== 0) bits.push(`加算${a.add >= 0 ? "+" : ""}${a.add}`);
      if (a.pin !== null) bits.push(`固定=${fmtInt(a.pin)}`);
      return `${STAT_LABELS[k]} ${bits.join("/")}`;
    });
    return parts.length === 0 ? "未設定(中立値で計算)" : parts.join("、");
  });
</script>

<section class="settings">
  <div class="panel-head"><span class="dot"></span><span class="title">SETTINGS — 設定</span></div>
  <div class="scroll">
    <div class="group">
      <button type="button" class="group-head" onclick={() => toggleGroup("permanent")}>
        <svg class="chevron" class:open={openGroup === "permanent"} width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
        <span class="group-title">恒常補正</span>
        <span class="group-summary dim">{permanentSummary}</span>
      </button>
      {#if openGroup === "permanent"}
        <div class="group-body">
          <div class="section-label"><span>ペット S スキル</span><span class="rule"></span></div>
          <div class="block stats">
            {#each STAT_KINDS as k (k)}
              <Select
                label={STAT_LABELS[k]}
                options={petSkillOptions}
                placeholder="なし"
                bind:value={() => petSkillValue(k), (v) => setPetSkillValue(k, v)}
              />
            {/each}
          </div>
          <div class="section-label"><span>ルーンスキル</span><span class="rule"></span><span class="dim">0–{limits.rune_level_max}</span></div>
          <div class="block stats">
            {#each STAT_KINDS as k (k)}
              <StatInput label={STAT_LABELS[k]} min={0} max={limits.rune_level_max} bind:value={draft.statSources.rune_levels[k]} />
            {/each}
          </div>
          <div class="section-label"><span>クラウン</span><span class="rule"></span><span class="dim">0–{limits.crown_max}</span></div>
          <div class="block stats">
            {#each STAT_KINDS as k (k)}
              <StatInput label={STAT_LABELS[k]} min={0} max={limits.crown_max} bind:value={draft.statSources.crown[k]} />
            {/each}
          </div>
          <div class="section-label"><span>神鳥の聖物</span><span class="rule"></span><span class="dim">0–{limits.sacred_relic_stage_max} 段階</span></div>
          <div class="block stats">
            {#each STAT_KINDS as k (k)}
              <StatInput
                label={STAT_LABELS[k]} min={0} max={limits.sacred_relic_stage_max}
                bind:value={draft.statSources.sacred_relic[k]}
                format={(v) => `${v} 段階 (+${v * 10})`}
              />
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <div class="group">
      <button type="button" class="group-head" onclick={() => toggleGroup("buffs")}>
        <svg class="chevron" class:open={openGroup === "buffs"} width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
        <span class="group-title">常用バフ</span>
        <span class="group-summary dim">{buffsSummary}</span>
      </button>
      {#if openGroup === "buffs"}
        <div class="group-body">
          <div class="block buffs">
            {#each consumableBuffs as def (def.id)}
              {@const choice = buffChoiceFor(def.id)}
              {@const checked = !!choice}
              {@const blocked = !checked && isBlocked(def)}
              <div class="buff-row">
                <label class="buff-check" class:disabled={blocked} title={blocked ? "同枠の他バフと排他です" : undefined}>
                  <input
                    type="checkbox"
                    {checked}
                    disabled={blocked}
                    onchange={(e) => toggleBuff(def, e.currentTarget.checked)}
                  />
                  <span>{def.name}</span>
                  {#if def.note}<span class="dim note">{def.note}</span>{/if}
                </label>
                {#if choice}
                  <div class="buff-detail">
                    {#if isUserSelectedTarget(def.target)}
                      <Select
                        label="対象ステ"
                        options={statOptions}
                        bind:value={() => choice.stat ?? STAT_KINDS[0], (v) => (choice.stat = v as StatKind)}
                      />
                    {/if}
                    {#if isChoiceValue(def.value)}
                      {@const options = def.value.choice.map((v, i) => ({ value: String(i), label: formatLayerValue(def.layer, v) }))}
                      <Select
                        label="値"
                        {options}
                        bind:value={() => String(choice.choice_index ?? 0), (v) => (choice.choice_index = Number(v))}
                      />
                    {/if}
                    {#if userInputRange(def.value)}
                      {@const range = userInputRange(def.value)!}
                      {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                      <StatInput
                        label={isPercentLayer(def.layer) ? "値 (%)" : "値"}
                        min={range.min * scale}
                        max={range.max * scale}
                        bind:value={
                          () => (choice.value ?? 0) * scale,
                          (v) => (choice.value = v / scale)
                        }
                      />
                    {/if}
                    {#if isFixedValue(def.value)}
                      <span class="fixed-value dim">値: {formatLayerValue(def.layer, def.value.fixed)}</span>
                    {/if}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <div class="group">
      <button type="button" class="group-head" onclick={() => toggleGroup("skills")}>
        <svg class="chevron" class:open={openGroup === "skills"} width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
        <span class="group-title">キャラスキル</span>
        <span class="group-summary dim">{skillsSummary}</span>
      </button>
      {#if openGroup === "skills"}
        <div class="group-body">
          <div class="section-label"><span>このキャラのスキル</span><span class="rule"></span></div>
          <div class="block buffs">
            {#if ownSkillBuffs.length === 0}
              <p class="empty dim">このキャラのスキルデータは未収録です。</p>
            {/if}
            {#each ownSkillBuffs as def (def.id)}
              {@const checked = !!buffChoiceFor(def.id)}
              <div class="buff-row">
                <label class="buff-check">
                  <input type="checkbox" {checked} onchange={(e) => toggleBuff(def, e.currentTarget.checked)} />
                  <span>{def.name}</span>
                  {#if isFixedValue(def.value)}<span class="fixed-value dim">{formatLayerValue(def.layer, def.value.fixed)}</span>{/if}
                  {#if def.note}<span class="dim note">{def.note}</span>{/if}
                </label>
              </div>
            {/each}
          </div>
          <div class="section-label"><span>味方から受けるスキル</span><span class="rule"></span></div>
          <div class="block buffs">
            {#if allySkillBuffs.length === 0}
              <p class="empty dim">味方から受けるスキルデータは未収録です。</p>
            {/if}
            {#each allySkillBuffs as def (def.id)}
              {@const checked = !!buffChoiceFor(def.id)}
              <div class="buff-row">
                <label class="buff-check">
                  <input type="checkbox" {checked} onchange={(e) => toggleBuff(def, e.currentTarget.checked)} />
                  <span>{def.name}</span>
                  {#if isFixedValue(def.value)}<span class="fixed-value dim">{formatLayerValue(def.layer, def.value.fixed)}</span>{/if}
                  {#if def.note}<span class="dim note">{def.note}</span>{/if}
                </label>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <div class="group">
      <button type="button" class="group-head" onclick={() => toggleGroup("adjustments")}>
        <svg class="chevron" class:open={openGroup === "adjustments"} width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
        <span class="group-title">調整</span>
        <span class="group-summary dim">{adjustmentsSummary}</span>
      </button>
      {#if openGroup === "adjustments"}
        <div class="group-body">
          <AdjustmentEditor
            adjustments={draft.statSources.adjustments}
            addMin={limits.adjustment_add_min} addMax={limits.adjustment_add_max}
            pinMin={limits.adjustment_pin_min} pinMax={limits.adjustment_pin_max}
            pinDefault={(k) => preview?.stats[k] ?? draft.baseStats[k]}
          />
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .settings { background: var(--bg); display: flex; flex-direction: column; min-height: 0; }
  .scroll { overflow: auto; min-height: 0; }

  .group { border-bottom: 1px solid var(--border); }
  .group-head {
    display: flex; align-items: center; gap: 10px; width: 100%; padding: 12px 14px;
    background: none; border: 0; cursor: pointer; text-align: left; color: var(--fg);
  }
  .group-head:hover { background: var(--bg-raised); }
  .chevron { flex-shrink: 0; transition: transform 0.15s; color: var(--fg-dim); }
  .chevron.open { transform: rotate(90deg); }
  .group-title { font-size: 12px; font-weight: 500; flex-shrink: 0; }
  .group-summary { font-size: 11px; min-width: 0; white-space: normal; overflow-wrap: break-word; }
  .group-body { padding-bottom: 10px; }

  .block { display: flex; flex-direction: column; gap: 10px; padding: 10px 14px 4px; }
  .block.stats { gap: 8px; }

  .block.buffs { gap: 0; padding: 4px 14px 6px; }
  .buff-row { padding: 7px 0; border-bottom: 1px solid var(--border-soft); }
  .buff-row:last-child { border-bottom: 0; }
  .buff-check { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; font-size: 12px; cursor: pointer; }
  .buff-check.disabled { opacity: 0.45; cursor: not-allowed; }
  .buff-check input { accent-color: var(--accent); }
  .note { font-size: 10px; }
  .fixed-value { font-size: 11px; font-weight: 500; }
  .buff-detail { display: flex; flex-wrap: wrap; gap: 8px; padding: 8px 0 2px 22px; }
  .buff-detail > :global(*) { min-width: 140px; flex: 1; }
  .empty { padding: 8px 0; font-size: 11px; }
</style>
