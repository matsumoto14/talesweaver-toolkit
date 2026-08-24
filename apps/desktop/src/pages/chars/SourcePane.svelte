<script lang="ts" module>
  export type SourceId =
    | "status"
    | "equipment"
    | "pet"
    | "rune"
    | "crown"
    | "relic"
    | "skills"
    | "adjust";
</script>

<script lang="ts">
  // 選択した補正源の編集ペイン。draft($state プロキシ)のネストしたプロパティを直接書き換える。
  // 専門用語(層名など)は「補正の内訳」以外に出さない(既存決定を踏襲)。
  import type { PetSkillTier, StatKind, StatPreview } from "../../api/types";
  import { isAllySkill, isCharacterSkillFor, isFixedValue, toggleBuff } from "../../buffs";
  import type { Draft } from "../../draft";
  import { fmtInt, formatLayerValue } from "../../format";
  import {
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, PET_SKILL_TIER_LABELS, STAT_KINDS, STAT_LABELS,
    STAT_LAYER_LABELS,
  } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { app } from "../../state.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import Select from "../../ui/Select.svelte";
  import StatInput from "../../ui/StatInput.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    previewError: string | null;
    sourceId: SourceId;
  }
  let { draft, preview, previewError, sourceId }: Props = $props();

  const STAT_MIN = 1;
  const characterOptions = $derived(app.gameCharacters.map((c) => ({ value: c.id, label: c.name })));
  const stageOptions = Array.from({ length: 6 }, (_, i) => ({ value: String(i), label: `${i} 段階` }));
  const eternalOptions = Array.from({ length: 81 }, (_, i) => ({ value: String(i), label: `Lv ${i}` }));

  const PET_TIERS: PetSkillTier[] = ["basic", "true_lv1", "true_lv2", "true_lv3", "true_lv4"];
  const petSkillOptions = [
    { value: "", label: "なし" },
    ...PET_TIERS.map((t) => ({ value: t, label: PET_SKILL_TIER_LABELS[t] })),
  ];
  const petSkillValue = (k: StatKind) => draft.statSources.pet_skills[k] ?? "";
  const setPetSkillValue = (k: StatKind, v: string) => {
    draft.statSources.pet_skills[k] = (v === "" ? null : v) as PetSkillTier | null;
  };

  const strongWeaponOptions = $derived([
    { value: "0", label: "なし" },
    ...Array.from({ length: limits.strong_weapon_level_max }, (_, i) => {
      const lv = i + 1;
      return { value: String(lv), label: `Lv${lv}(+${lv * 3}%)` };
    }),
  ]);

  const ownSkillBuffs = $derived(app.catalog.filter((d) => isCharacterSkillFor(d, draft.gameCharacterId)));
  const allySkillBuffs = $derived(app.catalog.filter(isAllySkill));
  const buffChecked = (buffId: string) => draft.statSources.buffs.choices.some((c) => c.buff_id === buffId);

  const TITLES: Record<SourceId, { title: string; note: string }> = {
    status: { title: "キャラステータス", note: "素ステ・覚醒・エタの意志" },
    equipment: { title: "装備", note: "ゲーム内ステータス画面の表示値をそのまま" },
    pet: { title: "ペット S スキル", note: "ステごとに 1 段階" },
    rune: { title: "ルーンスキル", note: `0–${limits.rune_level_max}` },
    crown: { title: "クラウン", note: `0–${limits.crown_max}` },
    relic: { title: "神鳥の聖物", note: `0–${limits.sacred_relic_stage_max} 段階(実加算は段階×10)` },
    skills: { title: "キャラスキル", note: "自分のスキルと味方から受けるスキル" },
    adjust: { title: "調整", note: "検証・仮定用の例外操作" },
  };

  const traceFor = (k: StatKind) => preview?.traces.find((t) => t.kind === k) ?? null;
  const signed = (n: number) => `${n >= 0 ? "+" : ""}${fmtInt(n)}`;
</script>

<div class="pane">
  <div class="pane-head">
    <span class="pane-title">{TITLES[sourceId].title}</span>
    <span class="dim">{TITLES[sourceId].note}</span>
  </div>

  {#if previewError}<p class="preview-error">{previewError}</p>{/if}

  {#if sourceId === "status"}
    <div class="card">
      <div class="fields">
        <label class="text">
          <span class="label">名前</span>
          <input type="text" bind:value={draft.name} maxlength="32" placeholder="表示名" />
        </label>
        <Select label="キャラ" bind:value={draft.gameCharacterId} options={characterOptions} />
        <div class="two">
          <Select label="覚醒段階" bind:value={draft.stage} options={stageOptions} />
          <Select label="エタの意志 Lv" bind:value={draft.eternalLevel} options={eternalOptions} />
        </div>
      </div>
    </div>
    <div class="card">
      <div class="card-title">能力値 <span class="dim normal">設定を触ると即時更新</span></div>
      <div class="tbl">
        <table class="grid">
          <thead><tr><th>ステ</th><th class="n">素</th><th class="n">補正</th><th class="n">最終</th></tr></thead>
          <tbody>
            {#each STAT_KINDS as k (k)}
              {@const trace = traceFor(k)}
              {@const diff = preview ? preview.stats[k] - draft.baseStats[k] : null}
              <tr>
                <td>{STAT_LABELS[k]}</td>
                <td class="n stat-cell">
                  <StatInput label="" min={STAT_MIN} max={limits.base_stat_max} bind:value={draft.baseStats[k]} />
                </td>
                <td class="n muted">{diff === null ? "—" : signed(diff)}</td>
                <td class="n final">
                  <span class="strong">{preview ? fmtInt(preview.stats[k]) : "—"}</span>
                  {#if trace?.pinned_from !== null && trace?.pinned_from !== undefined}
                    <span class="pin-badge" title={`固定前: ${fmtInt(trace.pinned_from)}`}>固定</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <details class="contrib">
        <summary>補正の内訳 <span class="dim">{preview ? preview.contributions.length : 0} 件</span></summary>
        {#if !preview || preview.contributions.length === 0}
          <p class="empty dim">補正源なし(素ステのみ)</p>
        {:else}
          <div class="tbl">
            <table class="grid">
              <thead><tr><th>ステ</th><th>出典</th><th>層</th><th class="n">値</th></tr></thead>
              <tbody>
                {#each STAT_KINDS.flatMap((k) => preview!.contributions.filter((c) => c.kind === k)) as c, i (i)}
                  <tr>
                    <td>{STAT_LABELS[c.kind]}</td>
                    <td class="muted">{c.source}</td>
                    <td class="muted">{STAT_LAYER_LABELS[c.layer]}</td>
                    <td class="n">{formatLayerValue(c.layer, c.value)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </details>
    </div>
  {:else if sourceId === "equipment"}
    <div class="card">
      <div class="card-title">基本能力値</div>
      <p class="hint dim">ゲーム内ステータス画面の装備補正から、エンチャント分を除いた値(+アビリティ)</p>
      <div class="fields">
        {#each EQUIPMENT_STAT_KINDS as k (k)}
          <StatInput label={EQUIPMENT_STAT_LABELS[k]} min={0} max={limits.equipment_value_max} bind:value={draft.equipment.base[k]} />
        {/each}
      </div>
    </div>
    <div class="card">
      <div class="card-title">強化能力値</div>
      <p class="hint dim">エンチャント・テシスコア・シエナのオーラ等の期間制能力値</p>
      <div class="fields">
        {#each EQUIPMENT_STAT_KINDS as k (k)}
          <StatInput label={EQUIPMENT_STAT_LABELS[k]} min={0} max={limits.equipment_value_max} bind:value={draft.equipment.enhanced[k]} />
        {/each}
      </div>
    </div>
    <div class="card">
      <div class="card-title">装備攻撃力強化</div>
      <div class="fields">
        <label class="check">
          <input type="checkbox" bind:checked={draft.equipment.power_weapon} />
          <span>パワーウェポン(+2%)</span>
        </label>
        <Select
          label="ストロングウェポン"
          options={strongWeaponOptions}
          bind:value={
            () => String(draft.equipment.strong_weapon_level),
            (v) => (draft.equipment.strong_weapon_level = Number(v))
          }
        />
      </div>
    </div>
  {:else if sourceId === "pet"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <Select
            label={STAT_LABELS[k]}
            options={petSkillOptions}
            placeholder="なし"
            bind:value={() => petSkillValue(k), (v) => setPetSkillValue(k, v)}
          />
        {/each}
      </div>
    </div>
  {:else if sourceId === "rune"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput label={STAT_LABELS[k]} min={0} max={limits.rune_level_max} bind:value={draft.statSources.rune_levels[k]} />
        {/each}
      </div>
    </div>
  {:else if sourceId === "crown"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput label={STAT_LABELS[k]} min={0} max={limits.crown_max} bind:value={draft.statSources.crown[k]} />
        {/each}
      </div>
    </div>
  {:else if sourceId === "relic"}
    <div class="card">
      <div class="fields">
        {#each STAT_KINDS as k (k)}
          <StatInput
            label={STAT_LABELS[k]}
            min={0}
            max={limits.sacred_relic_stage_max}
            bind:value={draft.statSources.sacred_relic[k]}
            format={(v) => `${v} 段階 (+${v * 10})`}
          />
        {/each}
      </div>
    </div>
  {:else if sourceId === "skills"}
    <div class="card">
      <div class="card-title">このキャラのスキル</div>
      <div class="buff-list">
        {#if ownSkillBuffs.length === 0}
          <p class="empty dim">このキャラのスキルデータは未収録です。</p>
        {/if}
        {#each ownSkillBuffs as def (def.id)}
          {@const checked = buffChecked(def.id)}
          <label class="check">
            <input
              type="checkbox"
              {checked}
              onchange={(e) => (draft.statSources.buffs.choices = toggleBuff(draft.statSources.buffs.choices, def, e.currentTarget.checked))}
            />
            <span>{def.name}</span>
            {#if isFixedValue(def.value)}<span class="fixed-value dim">{formatLayerValue(def.layer, def.value.fixed)}</span>{/if}
            {#if def.note}<span class="dim note">{def.note}</span>{/if}
          </label>
        {/each}
      </div>
      <div class="card-title space">味方から受けるスキル</div>
      <div class="buff-list">
        {#if allySkillBuffs.length === 0}
          <p class="empty dim">味方から受けるスキルデータは未収録です。</p>
        {/if}
        {#each allySkillBuffs as def (def.id)}
          {@const checked = buffChecked(def.id)}
          <label class="check">
            <input
              type="checkbox"
              {checked}
              onchange={(e) => (draft.statSources.buffs.choices = toggleBuff(draft.statSources.buffs.choices, def, e.currentTarget.checked))}
            />
            <span>{def.name}</span>
            {#if isFixedValue(def.value)}<span class="fixed-value dim">{formatLayerValue(def.layer, def.value.fixed)}</span>{/if}
            {#if def.note}<span class="dim note">{def.note}</span>{/if}
          </label>
        {/each}
      </div>
    </div>
  {:else if sourceId === "adjust"}
    <div class="card">
      <p class="hint dim">「このステに +N」「最終能力値を N に固定」の 2 種。検証・未収録データ用の例外操作です。</p>
      <AdjustmentEditor
        adjustments={draft.statSources.adjustments}
        addMin={limits.adjustment_add_min}
        addMax={limits.adjustment_add_max}
        pinMin={limits.adjustment_pin_min}
        pinMax={limits.adjustment_pin_max}
        pinDefault={(k) => preview?.stats[k] ?? draft.baseStats[k]}
      />
    </div>
  {/if}
</div>

<style>
  .pane { display: flex; flex-direction: column; gap: 9px; padding-bottom: 10px; }
  .pane-head { display: flex; align-items: baseline; gap: 8px; padding: 0 2px; }
  .pane-title { font-size: 10.5px; font-weight: 800; letter-spacing: 0.08em; color: #26334A; }
  .pane-head .dim { margin-left: auto; font-size: 9px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .preview-error { margin: 0; padding: 0 2px; font-size: 11px; color: var(--warm); }

  .card-title .normal { font-weight: 400; font-size: 9px; }
  .card-title.space { margin-top: 12px; }
  .hint { margin: 6px 0 0; font-size: 9.5px; line-height: 1.5; }
  .fields { margin-top: 9px; display: flex; flex-direction: column; gap: 9px; }
  .two { display: flex; gap: 10px; }
  .two > :global(*) { flex: 1; }
  .text { display: flex; flex-direction: column; gap: 6px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  input[type="text"] {
    padding: 8px 10px; border-radius: 8px;
    background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); }

  .tbl { margin-top: 8px; overflow-x: auto; border: 1px solid var(--border-soft); border-radius: 8px; background: #fff; }
  table.grid td.stat-cell { min-width: 180px; }
  .stat-cell :global(.stat-input) { justify-content: flex-end; flex-wrap: nowrap; }
  .final { white-space: nowrap; }
  .strong { font-weight: 700; }
  .pin-badge {
    margin-left: 6px; vertical-align: middle;
    font-size: 9px; letter-spacing: 0.05em; color: var(--accent); border: 1px solid var(--accent);
    border-radius: 4px; padding: 1px 4px; cursor: default;
  }
  details.contrib { margin-top: 8px; }
  details.contrib summary { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-muted); cursor: pointer; user-select: none; }
  details.contrib summary:hover { color: var(--fg); }
  .empty { margin: 6px 0 0; padding: 4px 0; font-size: 11px; }

  .check { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; font-size: 12px; cursor: pointer; }
  .check input { accent-color: var(--accent); }
  .buff-list { margin-top: 8px; display: flex; flex-direction: column; gap: 8px; }
  .note { font-size: 10px; }
  .fixed-value { font-size: 11px; font-weight: 500; }
</style>
