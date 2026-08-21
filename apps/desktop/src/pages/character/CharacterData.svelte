<script lang="ts">
  // 中央カラム(キャラデータ)。「答え」を出す列: 名前・キャラ種・覚醒、
  // 能力値表(ステ | 素 | 補正 | 最終)、補正の内訳(層名を出してよい唯一の場所)。
  import { STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
  import { fmtInt, formatLayerValue } from "../../format";
  import type { BuffDefinition, GameCharacter, StatPreview } from "../../api/types";
  import { limits } from "../../limits.svelte";
  import Select from "../../ui/Select.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import type { Draft } from "./draft";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    previewError: string | null;
    gameCharacters: GameCharacter[];
    catalog: BuffDefinition[];
    save: () => void;
    saving: boolean;
    dirty: boolean;
    canSubmit: boolean;
  }
  let { draft, preview, previewError, gameCharacters, save, saving, dirty, canSubmit }: Props = $props();

  const STAT_MIN = 1;

  const characterOptions = $derived(gameCharacters.map((c) => ({ value: c.id, label: c.name })));
  const stageOptions = Array.from({ length: 6 }, (_, i) => ({ value: String(i), label: `${i} 段階` }));
  const eternalOptions = Array.from({ length: 81 }, (_, i) => ({ value: String(i), label: `Lv ${i}` }));

  const traceFor = (k: (typeof STAT_KINDS)[number]) => preview?.traces.find((t) => t.kind === k) ?? null;

  const signed = (n: number) => `${n >= 0 ? "+" : ""}${fmtInt(n)}`;
</script>

<section class="data">
  <div class="panel-head">
    <span class="dot"></span><span class="title">CHARACTER DATA — キャラデータ</span>
    {#if dirty}<span class="badge">未保存</span>{/if}
    <div class="spacer"></div>
    <button type="button" class="btn primary" onclick={save} disabled={!canSubmit}>
      {saving ? "保存中…" : "保存"}
    </button>
  </div>

  <div class="scroll">
    <div class="block head">
      <label class="text">
        <span class="label">名前</span>
        <input type="text" bind:value={draft.name} maxlength="32" placeholder="表示名" />
      </label>
      <Select label="キャラ" bind:value={draft.gameCharacterId} options={characterOptions} />
      <div class="block two">
        <Select label="覚醒段階" bind:value={draft.stage} options={stageOptions} />
        <Select label="エタの意志 Lv" bind:value={draft.eternalLevel} options={eternalOptions} />
      </div>
    </div>

    <div class="section-label"><span>能力値</span><span class="rule"></span><span class="dim">設定を触ると即時更新</span></div>
    {#if previewError}<p class="preview-error">{previewError}</p>{/if}
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
      <summary>
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
        <span>補正の内訳</span>
        <span class="dim">{preview ? preview.contributions.length : 0} 件</span>
      </summary>
      <div class="tbl">
        {#if !preview || preview.contributions.length === 0}
          <p class="empty dim">補正源なし(素ステのみ)</p>
        {:else}
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
        {/if}
      </div>
    </details>
  </div>
</section>

<style>
  .data { background: var(--bg); display: flex; flex-direction: column; min-height: 0; }
  .scroll { overflow: auto; min-height: 0; }
  .panel-head .spacer { flex-grow: 1; }
  .badge {
    font-size: 10px; letter-spacing: 0.08em; color: var(--warm); border: 1px solid var(--warm);
    padding: 1px 6px;
  }

  .block { display: flex; flex-direction: column; gap: 10px; padding: 12px 14px; }
  .block.head { border-bottom: 1px solid var(--border); }
  .block.two { flex-direction: row; }
  .block.two > :global(*) { flex: 1; }
  .text { display: flex; flex-direction: column; gap: 6px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  input[type="text"] {
    padding: 8px 10px; background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); }

  .preview-error { padding: 4px 14px 0; font-size: 11px; color: var(--warm); }

  .tbl { overflow-x: auto; margin: 0 14px 12px; border: 1px solid var(--border-soft); }
  table.grid td.stat-cell { min-width: 180px; }
  .stat-cell :global(.stat-input) { justify-content: flex-end; flex-wrap: nowrap; }
  .final { white-space: nowrap; }
  .final .pin-badge { margin-left: 6px; vertical-align: middle; }
  .strong { font-weight: 700; }
  .pin-badge {
    font-size: 9px; letter-spacing: 0.05em; color: var(--accent); border: 1px solid var(--accent);
    padding: 1px 4px; cursor: default;
  }

  details.contrib { border-top: 1px solid var(--border); }
  details.contrib summary {
    display: flex; align-items: center; gap: 8px; padding: 11px 14px;
    font-size: 10px; letter-spacing: 0.14em; color: var(--fg-muted); cursor: pointer; list-style: none;
    user-select: none;
  }
  details.contrib summary::-webkit-details-marker { display: none; }
  details.contrib summary svg { transition: transform 0.15s; }
  details.contrib[open] summary svg { transform: rotate(90deg); }
  details.contrib summary:hover { color: var(--fg); }
  .empty { padding: 10px 12px; font-size: 11px; }
</style>
