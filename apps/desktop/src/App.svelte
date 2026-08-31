<script lang="ts">
  // 画面枠: 上部タブ(ホーム/ダメージ計算/バフ/キャラ/実測/お知らせ)+ 左キャラレール + エラー帯。
  // 構成は デザインモック TW Toolkit Prototype v4 に合わせる(規格は docs/design-system.html)。
  import { onMount } from "svelte";
  import brandLogo from "./assets/brand/tw-context-logo.png";
  import AboutPanel from "./AboutPanel.svelte";
  import InquiryPanel from "./InquiryPanel.svelte";
  import { errorMessage, getStartupNotice } from "./api/commands";
  import CharacterRail from "./CharacterRail.svelte";
  import CalcPage from "./pages/calc/CalcPage.svelte";
  import BuffsPage from "./pages/buffs/BuffsPage.svelte";
  import CharsPage from "./pages/chars/CharsPage.svelte";
  import HomePage from "./pages/home/HomePage.svelte";
  import MeasurePage from "./pages/measure/MeasurePage.svelte";
  import NewsPage from "./pages/news/NewsPage.svelte";
  import { app, focusErrorTarget, loadAll, simIsDirty, type Tab } from "./state.svelte";
  import { dismissError, reportError, reportNotice, runUndo, toast } from "./toast.svelte";
  import { checkForUpdate, updater } from "./update.svelte";
  import { persisted } from "./ui/persistedState.svelte";
  import Splitter from "./ui/Splitter.svelte";

  const TABS: { id: Tab; label: string }[] = [
    { id: "home", label: "ホーム" },
    { id: "calc", label: "ダメージ計算" },
    { id: "buffs", label: "バフ" },
    { id: "chars", label: "キャラ" },
    { id: "measure", label: "実測" },
    { id: "news", label: "お知らせ" },
  ];

  const DEFAULT_RAIL_WIDTH = 280;
  const railWidth = persisted("tw-v4-rail", { width: DEFAULT_RAIL_WIDTH });
  const railCollapsed = persisted("tw-v4-rail-collapsed", false);
  const expandedRailWidth = $derived(
    Math.max(200, Math.min(380, railWidth.value.width ?? DEFAULT_RAIL_WIDTH)),
  );
  const gridTemplateColumns = $derived(
    railCollapsed.value
      ? "64px 0px minmax(0, 1fr)"
      : `${expandedRailWidth}px 6px minmax(0, 1fr)`,
  );

  // お知らせタブに出す「まだ当てていない更新がある」印。押して当てたら消える。
  const updateWaiting = $derived(updater.status === "available" || updater.status === "ready");

  let aboutOpen = $state(false);
  let inquiryOpen = $state(false);

  onMount(() => {
    void loadAll();
    // 新しい版があるかだけ見に行く。当てるのはお知らせタブで押されたときだけ。
    void checkForUpdate();
    // バックアップからの復元など、読み飛ばされては困る事実は自動で消さない帯に出す。
    getStartupNotice()
      .then((notice) => {
        if (notice) reportNotice(notice.message);
      })
      .catch((e) => reportError(errorMessage(e)));
  });
</script>

<div class="shell">
  <header class="topbar">
    <div class="brand">
      <img src={brandLogo} alt="TW Context" />
    </div>
    <nav class="tabs">
      {#each TABS as t (t.id)}
        <button type="button" class="tab" class:on={app.tab === t.id} onclick={() => (app.tab = t.id)}>
          {t.label}
          {#if t.id === "news" && updateWaiting}<span class="tab-dot" aria-label="新しい版があります"></span>{/if}
        </button>
      {/each}
    </nav>
    {#if simIsDirty()}
      <div class="sim-note">
        <span class="dot"></span>
        <span>試し変更中 — 保存されていません</span>
      </div>
    {/if}
    <div class="utility-actions" class:pushed={!simIsDirty()}>
      <button
        type="button"
        class="utility-open"
        onclick={() => (aboutOpen = true)}
        aria-label="インフォメーション"
        title="このアプリについて"
      ><span aria-hidden="true">i</span>インフォメーション</button>
      <button
        type="button"
        class="utility-open"
        onclick={() => (inquiryOpen = true)}
      ><span aria-hidden="true">?</span>問い合わせ</button>
    </div>
  </header>

  {#if toast.message}
    <div class="toast" class:notice={toast.kind === "notice"} class:undo={toast.kind === "undo"} role="alert">
      <span>{toast.message}</span>
      <!-- どこの話か分かるエラーは、読ませるだけで終わらせない。押せばその場所が開く(§00 ⑤) -->
      {#if toast.target}
        {@const target = toast.target}
        <button
          type="button"
          class="toast-goto"
          onclick={() => { focusErrorTarget(target); dismissError(); }}
        >ここを開く ›</button>
      {/if}
      <!-- 消したものを戻す。押せるのはこの帯が出ているあいだだけ -->
      {#if toast.undoable}
        <button type="button" class="toast-goto" onclick={runUndo}>元に戻す</button>
      {/if}
      <button type="button" onclick={dismissError} aria-label="閉じる">×</button>
    </div>
  {/if}

  {#if aboutOpen}
    <AboutPanel onClose={() => (aboutOpen = false)} />
  {/if}
  <!-- 実測の送信(計算タブ)も同じパネルを使う。送信前に全文を見せる作法を 1 か所に保つ -->
  {#if inquiryOpen || app.inquiryPrefill}
    <InquiryPanel
      prefill={app.inquiryPrefill}
      onClose={() => {
        inquiryOpen = false;
        app.inquiryPrefill = null;
      }}
    />
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
      <!-- タブは面ごと入れ替わる。入ってくる面を短く動かして「切り替わった」を見せる(§10 型 3b) -->
      {#key app.tab}
        <div class="tabbody swap-in">
          {#if app.tab === "home"}
            <HomePage />
          {:else if app.tab === "calc"}
            <CalcPage />
          {:else if app.tab === "buffs"}
            <BuffsPage />
          {:else if app.tab === "measure"}
            <MeasurePage />
          {:else if app.tab === "news"}
            <NewsPage />
          {:else}
            <CharsPage />
          {/if}
        </div>
      {/key}
    </main>
  </div>
</div>

<style>
  .shell { height: 100%; display: flex; flex-direction: column; position: relative; }

  .topbar {
    height: 52px; flex-shrink: 0; padding: 0 20px 0 16px;
    background: linear-gradient(180deg, #DBE6F8, #AEC7F0);
    border-bottom: 1px solid var(--sel-bd);
    display: flex; align-items: center; gap: 14px;
  }
  .brand { display: flex; align-items: center; flex-shrink: 0; }
  .brand img { width: auto; height: 48px; display: block; object-fit: contain; }

  /* 見た目は app.css の `.tabs` / `.tab`(§08)。ここには置き場所だけ */
  .tabs { margin-left: 8px; align-self: flex-end; }
  /* 更新が待っている印。タブの幅を動かさないよう、文字の右に 6px だけ足す(§00 03) */
  .tab-dot {
    display: inline-block; margin-left: 6px; width: 6px; height: 6px; border-radius: 50%;
    background: var(--gold); box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.7);
  }

  .sim-note {
    margin-left: auto; display: flex; align-items: center; gap: 8px;
    padding: 5px 12px; border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.7); border: 1px solid var(--sim);
    font-size: var(--t-label); font-weight: var(--w-strong); color: var(--sim-fg); white-space: nowrap;
  }
  .sim-note .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--sim); }

  /* 補助導線。名前を併記し、情報と問い合わせを押す前から区別できるようにする。 */
  .utility-actions { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
  .utility-actions.pushed { margin-left: auto; }
  .utility-open {
    height: 26px; padding: 0 9px 0 6px;
    display: flex; align-items: center; gap: 5px;
    border-radius: var(--r-pill); border: 1px solid var(--sel-bd);
    background: rgba(255, 255, 255, 0.7); color: var(--fg-muted);
    font-size: var(--t-label); font-weight: var(--w-strong); line-height: 1; white-space: nowrap;
  }
  .utility-open span {
    width: 16px; height: 16px; display: grid; place-items: center;
    border-radius: 50%; background: var(--bg-panel); border: 1px solid var(--border-soft);
    font-family: var(--font-num); font-size: 9px; font-weight: var(--w-strong);
    font-variant-numeric: tabular-nums;
  }
  .utility-open:hover { background: var(--bg-field); color: var(--accent); }

  .toast {
    position: absolute; top: 58px; left: 16px; right: 16px; z-index: 50;
    display: flex; align-items: center; gap: 12px; padding: 9px 12px;
    border-radius: var(--r-panel);
    background: #FDF1EF; border: 1px solid var(--danger); border-left-width: 3px;
    color: var(--fg); font-size: 12px;
  }
  /* 起動時の復元など、失敗ではないが読み飛ばされては困る事実(自動では消えない) */
  .toast.notice {
    background: var(--state-edge-bg); border-color: var(--state-edge-bd);
  }
  /* 取り消せる操作の直後。失敗ではないので危険色にしない — 押せるのは「元に戻す」だけ */
  .toast.undo {
    background: var(--state-edge-bg); border-color: var(--state-edge-bd);
  }
  .toast.undo .toast-goto {
    border-color: var(--accent); color: var(--accent);
  }
  .toast.undo .toast-goto:hover { background: var(--accent); color: #fff; }
  .toast span { margin-right: auto; }
  .toast button { color: var(--fg-muted); font-size: 14px; }
  /* 「読むだけ」の帯の中で、押せるものだけ形を持たせる(§00 ⑤ 考えさせない) */
  .toast-goto {
    flex-shrink: 0; padding: 2px 9px; border-radius: var(--r-pill);
    border: 1px solid var(--danger); background: var(--bg-field);
    color: var(--danger); font-size: 11px; font-weight: 600;
  }
  .toast-goto:hover { background: var(--danger); color: #fff; }

  .body {
    flex: 1; min-height: 0; display: grid;
    transition: grid-template-columns 260ms cubic-bezier(0.4, 0, 0.2, 1);
  }
  main { min-width: 0; min-height: 0; overflow: hidden; display: flex; flex-direction: column; background: var(--bg-mid); }
  .tabbody { flex: 1; min-height: 0; min-width: 0; display: flex; flex-direction: column; }
</style>
