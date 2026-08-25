<script lang="ts">
  import { STAT_KINDS, STAT_LABELS } from "../labels";
  import type { Adjustments, StatKind } from "../api/types";
  import StatInput from "./StatInput.svelte";

  interface Props {
    adjustments: Adjustments;
    addMin: number;
    addMax: number;
    pinMin: number;
    pinMax: number;
    /** 固定(pin)を ON にしたときの初期値(「初期値は実用値」の原則) */
    pinDefault: (k: StatKind) => number;
  }
  let { adjustments, addMin, addMax, pinMin, pinMax, pinDefault }: Props = $props();

  function togglePin(k: StatKind, checked: boolean) {
    adjustments[k].pin = checked ? pinDefault(k) : null;
  }
</script>

<div class="adjustments">
  {#each STAT_KINDS as k (k)}
    <div class="adj-stat">
      <div class="adj-stat-label">{STAT_LABELS[k]}</div>
      <div class="adj-row">
        <span class="adj-desc dim">加算 — このステに +N する(検証・仮定用)</span>
        <div class="adj-control">
          <StatInput label="" min={addMin} max={addMax} bind:value={adjustments[k].add} />
        </div>
      </div>
      <div class="adj-row">
        <label class="toggle">
          <input
            type="checkbox"
            checked={adjustments[k].pin !== null}
            onchange={(e) => togglePin(k, e.currentTarget.checked)}
          />
          <span class="check" aria-hidden="true">
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M1.6 4.5l1.9 1.9L7.4 2.6"/></svg>
          </span>
          <span class="adj-desc">固定 — 最終能力値を N に固定する(実測値で計算したいとき)</span>
        </label>
        {#if adjustments[k].pin !== null}
          <div class="adj-control">
            <StatInput
              label="" min={pinMin} max={pinMax}
              bind:value={
                () => adjustments[k].pin ?? pinMin,
                (v) => (adjustments[k].pin = v)
              }
            />
          </div>
        {/if}
      </div>
    </div>
  {/each}
</div>

<style>
  .adjustments { display: flex; flex-direction: column; min-width: 0; }
  .adj-stat { padding: 8px 14px; border-bottom: 1px solid var(--border-soft); display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .adj-stat:last-child { border-bottom: 0; }
  .adj-stat-label { font-size: 11px; font-weight: 700; color: var(--fg-muted); }
  .adj-row { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; min-width: 0; }
  .adj-row .toggle { padding: 0; }
  .adj-desc { font-size: 11px; min-width: 0; }
  .adj-control { width: 100%; min-width: 0; }
  .adj-control :global(.stat-input) { width: 100%; }

  .toggle { display: flex; align-items: center; flex-wrap: wrap; gap: 9px; cursor: pointer; font-size: 12px; min-width: 0; }
  .toggle input { position: absolute; opacity: 0; width: 0; height: 0; }
  .check {
    width: 13px; height: 13px; flex-shrink: 0; border: 1px solid var(--border-strong);
    border-radius: var(--r-inset);
    display: flex; align-items: center; justify-content: center; color: transparent;
  }
  .toggle input:checked + .check { background: var(--accent); border-color: var(--accent); color: var(--bg); }
  .toggle input:focus-visible + .check { outline: 1px solid var(--accent); outline-offset: 2px; }
</style>
