<script lang="ts">
  import CharacterPage from "./pages/character/CharacterPage.svelte";
  import DamagePage from "./pages/damage/DamagePage.svelte";
  import { dismissError, toast } from "./toast.svelte";
  import { persisted } from "./ui/persistedState.svelte";

  type PageId = "damage" | "characters" | "enhance" | "roadmap" | "index";
  interface NavItem { id: PageId; label: string; enabled: boolean }
  const NAV: NavItem[] = [
    { id: "damage", label: "ダメージ計算", enabled: true },
    { id: "characters", label: "キャラ管理", enabled: true },
    { id: "enhance", label: "強化提案", enabled: false },
    { id: "roadmap", label: "ロードマップ", enabled: false },
    { id: "index", label: "やりたいこと索引", enabled: false },
  ];

  let page = $state<PageId>("characters");
  const title = $derived(NAV.find((n) => n.id === page)?.label ?? "");

  let recalculate: (() => void) | null = null;

  const sidebarCollapsed = persisted("tw-sidebar-collapsed", false);
</script>

<div class="shell">
  <aside class:collapsed={sidebarCollapsed.value}>
    <div class="brand">
      <button
        type="button"
        class="collapse-btn"
        title={sidebarCollapsed.value ? "サイドバーを開く" : "サイドバーを閉じる"}
        aria-label={sidebarCollapsed.value ? "サイドバーを開く" : "サイドバーを閉じる"}
        onclick={() => (sidebarCollapsed.value = !sidebarCollapsed.value)}
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" class:flip={sidebarCollapsed.value}><path d="M10 3.2L5.4 8l4.6 4.8"/></svg>
      </button>
      <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="var(--accent)" stroke-width="1.4" stroke-linejoin="round"><path d="M9 1.5l6.5 3.75v7.5L9 16.5 2.5 12.75v-7.5z"/><path d="M9 6.2l3.2 1.85v3.7L9 13.6l-3.2-1.85v-3.7z"/></svg>
      {#if !sidebarCollapsed.value}<span>TW TOOLKIT</span>{/if}
    </div>
    <nav>
      {#each NAV as item (item.id)}
        <button type="button" class:active={page === item.id} disabled={!item.enabled} title={sidebarCollapsed.value ? item.label : undefined} onclick={() => (page = item.id)}>
          {#if item.id === "damage"}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="2" width="10" height="12" rx="1"/><path d="M5.5 5h5M5.5 8.2h1.4M7.8 8.2h1.4M10.1 8.2h1.4M5.5 11h1.4M7.8 11h1.4M10.1 11h1.4"/></svg>
          {:else if item.id === "characters"}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="5.6" r="2.6"/><path d="M2.9 13.6c0-2.5 2.3-4.1 5.1-4.1s5.1 1.6 5.1 4.1"/></svg>
          {:else if item.id === "enhance"}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M8 12.5V3.2M8 3.2L4.6 6.6M8 3.2l3.4 3.4"/><path d="M3.4 14.2h9.2"/></svg>
          {:else if item.id === "roadmap"}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M2.4 4.6l3.9-1.5 3.4 1.5 3.9-1.5v8.8l-3.9 1.5-3.4-1.5-3.9 1.5z"/><path d="M6.3 3.1v9.8M9.7 4.6v9.8"/></svg>
          {:else}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M2.6 3.4h3.6c1 0 1.8.8 1.8 1.8v8.4c0-1-.8-1.8-1.8-1.8H2.6z"/><path d="M13.4 3.4H9.8c-1 0-1.8.8-1.8 1.8v8.4c0-1 .8-1.8 1.8-1.8h3.6z"/></svg>
          {/if}
          {#if !sidebarCollapsed.value}
            <span>{item.label}</span>
            {#if !item.enabled}<span class="soon">未実装</span>{/if}
          {/if}
        </button>
      {/each}
    </nav>
    <div class="spacer"></div>
    {#if !sidebarCollapsed.value}<div class="foot dim">DATA seed / 2026-08-21</div>{/if}
  </aside>

  <main>
    <header>
      <span class="h-title">{title}</span>
      {#if page === "damage"}
        <span class="dim">/ 単発スキル</span>
        <div class="spacer"></div>
        <button class="btn primary" onclick={() => recalculate?.()}>
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.4l3 3 6-6.8"/></svg>
          <span>計算</span>
        </button>
      {/if}
    </header>

    {#if toast.message}
      <div class="toast" role="alert">
        <span>{toast.message}</span>
        <button type="button" onclick={dismissError} aria-label="閉じる">×</button>
      </div>
    {/if}

    <div class="content">
      {#if page === "damage"}
        <DamagePage registerRecalculate={(fn) => (recalculate = fn)} />
      {:else if page === "characters"}
        <CharacterPage />
      {/if}
    </div>
  </main>
</div>

<style>
  .shell { height: 100%; display: flex; }
  aside {
    width: 208px; flex-shrink: 0; display: flex; flex-direction: column;
    background: var(--bg-panel); border-right: 1px solid var(--border);
    transition: width 0.15s;
  }
  aside.collapsed { width: 56px; }
  .brand {
    height: 52px; display: flex; align-items: center; gap: 9px; padding: 0 14px;
    border-bottom: 1px solid var(--border);
    font-size: 11px; font-weight: 700; letter-spacing: 0.14em;
  }
  aside.collapsed .brand { padding: 0 10px; gap: 6px; }
  .collapse-btn {
    display: flex; align-items: center; justify-content: center; flex-shrink: 0;
    width: 18px; height: 18px; padding: 0; background: none; border: 0; color: var(--fg-muted);
  }
  .collapse-btn:hover { color: var(--fg); }
  .collapse-btn svg { transition: transform 0.15s; }
  .collapse-btn svg.flip { transform: rotate(180deg); }
  nav { display: flex; flex-direction: column; gap: 2px; padding: 10px 8px; }
  nav button {
    display: flex; align-items: center; gap: 10px; padding: 8px 10px;
    background: none; border: 0; border-left: 2px solid transparent;
    color: var(--fg-muted); font-size: 12px; text-align: left;
  }
  aside.collapsed nav button { justify-content: center; padding: 8px; }
  nav button:hover:not(:disabled) { color: var(--fg); }
  nav button.active { color: var(--accent); background: var(--bg-active); border-left-color: var(--accent); }
  nav button:disabled { opacity: 0.45; }
  .soon { margin-left: auto; font-size: 9px; letter-spacing: 0.08em; color: var(--fg-dim); }
  .spacer { flex-grow: 1; }
  .foot { padding: 12px 14px; border-top: 1px solid var(--border); font-size: 10px; }

  main { flex-grow: 1; display: flex; flex-direction: column; min-width: 0; position: relative; }
  header {
    height: 52px; flex-shrink: 0; display: flex; align-items: center; gap: 12px; padding: 0 16px;
    background: var(--bg-panel); border-bottom: 1px solid var(--border);
  }
  .h-title { font-weight: 500; }
  header .dim { font-size: 11px; }
  .content { flex-grow: 1; min-height: 0; }

  .toast {
    position: absolute; top: 60px; left: 16px; right: 16px; z-index: 10;
    display: flex; align-items: center; gap: 12px; padding: 9px 12px;
    background: oklch(0.26 0.04 25); border: 1px solid var(--danger); border-left-width: 3px;
    color: var(--fg); font-size: 12px;
  }
  .toast button { margin-left: auto; background: none; border: 0; color: var(--fg-muted); font-size: 14px; }
</style>
