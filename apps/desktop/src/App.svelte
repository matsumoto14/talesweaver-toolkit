<script lang="ts">
  // 画面枠: 上部タブ(ホーム/ダメージ計算/キャラ)+ 左キャラレール + エラー帯。
  // 構成は docs/design の TW Toolkit Prototype v4 に合わせる。
  import { onMount } from "svelte";
  import { errorMessage } from "./api/commands";
  import CharacterRail from "./CharacterRail.svelte";
  import { loadStatLimits } from "./limits.svelte";
  import CalcPage from "./pages/calc/CalcPage.svelte";
  import CharsPage from "./pages/chars/CharsPage.svelte";
  import HomePage from "./pages/home/HomePage.svelte";
  import { app, loadAll, type Tab } from "./state.svelte";
  import { dismissError, reportError, toast } from "./toast.svelte";
  import { persisted } from "./ui/persistedState.svelte";
  import Splitter from "./ui/Splitter.svelte";

  const TABS: { id: Tab; label: string }[] = [
    { id: "home", label: "ホーム" },
    { id: "calc", label: "ダメージ計算" },
    { id: "chars", label: "キャラ" },
  ];

  const DEFAULT_RAIL_WIDTH = 280;
  const railWidth = persisted("tw-v4-rail", { width: DEFAULT_RAIL_WIDTH });
  const railCollapsed = persisted("tw-v4-rail-collapsed", false);
  const gridTemplateColumns = $derived(
    railCollapsed.value
      ? "64px 0px minmax(0, 1fr)"
      : `minmax(200px, ${railWidth.value.width ?? DEFAULT_RAIL_WIDTH}px) 6px minmax(0, 1fr)`,
  );

  onMount(() => {
    loadStatLimits().catch((e) => reportError(errorMessage(e)));
    void loadAll();
  });
</script>

<div class="shell">
  <header class="topbar">
    <div class="brand">
      <span class="mark"></span>
      <span class="name">TW TOOLKIT</span>
    </div>
    <nav class="tabs">
      {#each TABS as t (t.id)}
        <button type="button" class="tab" class:active={app.tab === t.id} onclick={() => (app.tab = t.id)}>
          {t.label}
        </button>
      {/each}
    </nav>
    {#if app.sim !== null}
      <div class="sim-note">
        <span class="dot"></span>
        <span>試し変更中 — 保存されていません</span>
      </div>
    {/if}
  </header>

  {#if toast.message}
    <div class="toast" role="alert">
      <span>{toast.message}</span>
      <button type="button" onclick={dismissError} aria-label="閉じる">×</button>
    </div>
  {/if}

  <div class="body" style="grid-template-columns: {gridTemplateColumns};">
    <CharacterRail collapsed={railCollapsed.value} onToggle={() => (railCollapsed.value = !railCollapsed.value)} />
    {#if !railCollapsed.value}
      <Splitter
        bind:value={railWidth.value.width}
        min={200}
        max={380}
        defaultValue={DEFAULT_RAIL_WIDTH}
        controls="prev"
        label="キャラレールとメインの境界"
      />
    {:else}
      <div></div>
    {/if}
    <main>
      {#if app.tab === "home"}
        <HomePage />
      {:else if app.tab === "calc"}
        <CalcPage />
      {:else}
        <CharsPage />
      {/if}
    </main>
  </div>
</div>

<style>
  .shell { height: 100%; display: flex; flex-direction: column; position: relative; }

  .topbar {
    height: 52px; flex-shrink: 0; padding: 0 20px 0 16px;
    background: linear-gradient(180deg, #DBE6F8, #AEC7F0);
    border-bottom: 1px solid #687287;
    display: flex; align-items: center; gap: 14px;
  }
  .brand { display: flex; align-items: center; gap: 9px; }
  .brand .mark {
    width: 22px; height: 22px; border-radius: var(--r-inset);
    background: linear-gradient(160deg, #fff, #8EB9FC);
    border: 1px solid #687287;
  }
  .brand .name { font-weight: 800; font-size: 12.5px; letter-spacing: 0.08em; white-space: nowrap; }

  .tabs { display: flex; align-items: flex-end; gap: 3px; margin-left: 8px; align-self: flex-end; }
  .tab {
    padding: 6px 14px; border-radius: var(--r-panel) 9px 0 0;
    background: linear-gradient(180deg, #A9CDFE, #8EB9FC);
    border: 1px solid var(--border-strong); border-bottom: 0;
    color: #2B3C57; font-size: 12px; font-weight: 500; white-space: nowrap;
  }
  .tab.active {
    padding: 7px 16px; position: relative; top: 1px;
    background: linear-gradient(180deg, #CCF7FF, #90D7FF);
    border-color: #687287; color: #123047; font-weight: 700;
  }

  .sim-note {
    margin-left: auto; display: flex; align-items: center; gap: 8px;
    padding: 5px 12px; border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.7); border: 1px solid var(--sim);
    font-size: var(--t-label); font-weight: var(--w-strong); color: var(--sim-fg); white-space: nowrap;
  }
  .sim-note .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--sim); }

  .toast {
    position: absolute; top: 58px; left: 16px; right: 16px; z-index: 50;
    display: flex; align-items: center; gap: 12px; padding: 9px 12px;
    border-radius: var(--r-panel);
    background: #FDF1EF; border: 1px solid var(--danger); border-left-width: 3px;
    color: var(--fg); font-size: 12px;
  }
  .toast button { margin-left: auto; color: var(--fg-muted); font-size: 14px; }

  .body { flex: 1; min-height: 0; display: grid; }
  main { min-width: 0; min-height: 0; overflow: hidden; display: flex; flex-direction: column; background: var(--bg-mid); }
</style>
