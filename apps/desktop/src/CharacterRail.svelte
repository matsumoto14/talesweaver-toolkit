<script lang="ts">
  // 左のキャラレール。全タブ共通の「どのキャラの話をしているか」を担う。
  import { app, gameCharacterName, selectCharacter } from "./state.svelte";

  interface Props {
    collapsed: boolean;
    onToggle: () => void;
  }
  let { collapsed, onToggle }: Props = $props();

  const totalContents = $derived(app.areas.reduce((n, a) => n + a.contents.length, 0));
  const clearCount = (id: number) => (app.evaluations[id] ?? []).filter((e) => e.clear).length;

  function goRegister() {
    app.tab = "chars";
    app.registerOpen = true;
  }
</script>

<aside class:collapsed>
  <div class="head-bar">
    {#if !collapsed}
      <span class="title">キャラ</span>
      <span class="note">{app.characters.length} 人登録済み</span>
    {/if}
    <button
      type="button"
      class="rail-toggle"
      title={collapsed ? "レールを開く" : "レールを畳む"}
      aria-label={collapsed ? "レールを開く" : "レールを畳む"}
      onclick={onToggle}
    >{collapsed ? "›" : "‹"}</button>
  </div>
  <div class="list">
    {#if app.loading}
      <p class="note-text dim">読み込み中…</p>
    {/if}
    {#each app.characters as c (c.id)}
      {@const selected = c.id === app.selectedId}
      <button
        type="button"
        class="char"
        class:selected
        title="{c.name}({gameCharacterName(c.game_character_id)}) クリア可 {clearCount(c.id)} / {totalContents}"
        onclick={() => selectCharacter(c.id)}
      >
        <span class="icon">{c.name.slice(0, 1)}</span>
        {#if !collapsed}
          <span class="meta">
            <span class="name">{c.name}</span>
            <span class="cls">{gameCharacterName(c.game_character_id)} / 覚醒{c.awakening.stage}</span>
          </span>
          <span class="count">
            <span class="ok num">{clearCount(c.id)}<span class="total"> / {totalContents}</span></span>
            <span class="cap">クリア可</span>
          </span>
        {:else}
          <span class="mini num">{clearCount(c.id)}</span>
        {/if}
      </button>
    {/each}
    <button type="button" class="register" onclick={goRegister}>{collapsed ? "＋" : "＋ キャラを登録"}</button>
    {#if !collapsed}
      <p class="note-text dim">コンテンツの目安ダメージ・入場条件は仮値です。</p>
    {/if}
  </div>
</aside>

<style>
  aside {
    min-width: 0; min-height: 0; display: flex; flex-direction: column;
    background: var(--bg-rail); overflow: hidden;
  }
  .rail-toggle {
    flex-shrink: 0; margin-left: auto; width: 20px; height: 20px;
    display: flex; align-items: center; justify-content: center; border-radius: 6px;
    background: rgba(255, 255, 255, 0.22); border: 1px solid rgba(255, 255, 255, 0.55);
    font-size: 10px; font-weight: 700; color: #fff;
  }
  .list { flex: 1; min-height: 0; overflow: auto; padding: 12px 10px; display: flex; flex-direction: column; gap: 9px; }
  aside.collapsed .list { padding: 10px 8px; }

  .char {
    display: flex; align-items: center; gap: 10px; padding: 9px 10px 9px 12px;
    border-radius: 11px; text-align: left;
    background: linear-gradient(180deg, #fff, #F4F8FD);
    border: 1px solid #C8D6E6; border-left: 3px solid #DCE5F0;
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgba(30, 44, 74, 0.06);
  }
  .char:hover { border-color: #9FB4D0; }
  .char.selected {
    background: linear-gradient(180deg, #E4F1FF, #C6E2FF);
    border-color: var(--accent); border-left-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(66, 109, 214, 0.14), inset 0 1px 0 #fff;
  }
  aside.collapsed .char { flex-direction: column; gap: 2px; padding: 8px 4px 7px; }

  .icon {
    width: 30px; height: 30px; flex-shrink: 0;
    display: flex; align-items: center; justify-content: center; border-radius: 9px;
    background: repeating-linear-gradient(135deg, #E4EDF9 0 4px, #CFDFF2 4px 8px);
    border: 1px solid var(--border-strong);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.8);
    font-size: 13px; font-weight: 800; color: #3B4A63;
  }
  aside.collapsed .icon { width: 34px; height: 34px; }

  .meta { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .meta .name { font-size: 12.5px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta .cls { font-size: 9.5px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .char.selected .meta .name { font-weight: 700; }

  .count { margin-left: auto; flex-shrink: 0; text-align: right; display: flex; flex-direction: column; }
  .count .ok { font-size: 13px; font-weight: 700; white-space: nowrap; }
  .count .total { font-size: 9.5px; color: var(--fg-dim); font-weight: 400; }
  .count .cap { font-size: 8.5px; color: var(--fg-muted); white-space: nowrap; }
  .mini { font-size: 9.5px; font-weight: 700; color: #26334A; }

  .register {
    text-align: center; padding: 9px 6px; border-radius: 10px;
    background: linear-gradient(180deg, #fff, var(--bg-rail));
    border: 1px dashed #9FB4D0; box-shadow: inset 0 1px 0 #fff;
    font-size: 11px; font-weight: 700; color: #2B3C57; white-space: nowrap; overflow: hidden;
  }
  .register:hover { border-style: solid; }

  .note-text { margin: 0; font-size: 9px; line-height: 1.6; }
</style>
