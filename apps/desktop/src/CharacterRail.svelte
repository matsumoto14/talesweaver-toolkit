<script lang="ts">
  // 左のキャラレール。全タブ共通の「どのキャラの話をしているか」を担う。
  import { app, gameCharacterName, selectCharacter, totalContents } from "./state.svelte";
  import Icon from "./ui/Icon.svelte";
  import { bump } from "./ui/motion.svelte";
  import { persisted } from "./ui/persistedState.svelte";
  import { dropHalfIndex, moveItem } from "./ui/reorder.svelte";

  interface Props {
    collapsed: boolean;
    onToggle: () => void;
  }
  let { collapsed, onToggle }: Props = $props();

  const total = $derived(totalContents());
  const clearCount = (id: number) => (app.evaluations[id] ?? []).filter((e) => e.clear).length;
  const characterOrder = persisted<number[]>("tw-character-order", []);
  const orderedCharacters = $derived.by(() => {
    const byId = new Map(app.characters.map((character) => [character.id, character]));
    return [
      ...characterOrder.value.map((id) => byId.get(id)).filter((character) => character !== undefined),
      ...app.characters.filter((character) => !characterOrder.value.includes(character.id)),
    ];
  });
  let draggedCharacterId = $state<number | null>(null);
  let characterDropAt = $state<number | null>(null);

  function startCharacterDrag(event: DragEvent, id: number) {
    draggedCharacterId = id;
    event.dataTransfer?.setData("text/plain", String(id));
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }
  function dragCharacterOver(event: DragEvent, index: number) {
    if (draggedCharacterId === null) return;
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    characterDropAt = dropHalfIndex(rect, event.clientY, index, "y");
  }
  function dropCharacter(event: DragEvent) {
    event.preventDefault();
    if (draggedCharacterId === null || characterDropAt === null) return;
    const ids = orderedCharacters.map((character) => character.id);
    const from = ids.indexOf(draggedCharacterId);
    if (from === -1) return;
    characterOrder.value = moveItem(ids, from, characterDropAt);
    draggedCharacterId = null;
    characterDropAt = null;
  }

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
    {#each orderedCharacters as c, index (c.id)}
      {@const selected = c.id === app.selectedId}
      <button
        type="button"
        class="char"
        class:selected
        class:dragging={draggedCharacterId === c.id}
        class:drop-before={characterDropAt === index}
        class:drop-after={characterDropAt === index + 1 && index === orderedCharacters.length - 1}
        draggable="true"
        title="{c.name}({gameCharacterName(c.game_character_id)}) クリア可 {clearCount(c.id)} / {total}"
        onclick={() => selectCharacter(c.id)}
        ondragstart={(event) => startCharacterDrag(event, c.id)}
        ondragover={(event) => dragCharacterOver(event, index)}
        ondrop={dropCharacter}
        ondragend={() => { draggedCharacterId = null; characterDropAt = null; }}
      >
        <span class="grip" aria-hidden="true">⠿</span>
        <!-- 畳んだときはアイコン単独になるが、ボタンに title があるので規格の例外に当たる -->
        <Icon
          kind="character"
          id={c.game_character_id}
          size={collapsed ? 40 : 28}
          label="{c.name}({gameCharacterName(c.game_character_id)})"
        />
        {#if !collapsed}
          <span class="meta">
            <span class="name">{c.name}</span>
            <span class="cls">{gameCharacterName(c.game_character_id)} / 覚醒{c.awakening.stage}</span>
          </span>
          <span class="count">
            <span class="ok num" use:bump={() => clearCount(c.id)}>{clearCount(c.id)}<span class="total"> / {total}</span></span>
            <span class="cap">クリア可</span>
          </span>
        {:else}
          <span class="mini num" use:bump={() => clearCount(c.id)}>{clearCount(c.id)}</span>
        {/if}
      </button>
    {/each}
    <button type="button" class="register" onclick={goRegister}>{collapsed ? "＋" : "＋ キャラを登録"}</button>
    {#if !collapsed}
      <p class="note-text dim">目安ダメージは wiki に無い値です(コミュニティ知識・実測)。</p>
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
    display: flex; align-items: center; justify-content: center; border-radius: var(--r-inset);
    background: rgba(255, 255, 255, 0.22); border: 1px solid rgba(255, 255, 255, 0.55);
    font-size: 10px; font-weight: 700; color: #fff;
  }
  .list { flex: 1; min-height: 0; overflow: auto; padding: 12px 10px; display: flex; flex-direction: column; gap: 9px; }
  aside.collapsed .list { padding: 10px 8px; }

  .char {
    position: relative; cursor: grab; user-select: none; -webkit-user-drag: element;
    display: flex; align-items: center; gap: 10px; padding: 9px 10px 9px 12px;
    border-radius: var(--r-window); text-align: left;
    background: linear-gradient(180deg, #fff, #F4F8FD);
    border: 1px solid #C8D6E6; border-left: 3px solid #DCE5F0;
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgba(30, 44, 74, 0.06);
  }
  .char:active { cursor: grabbing; }
  .char.dragging { opacity: .45; }
  .char.drop-before::before, .char.drop-after::after {
    content: ""; position: absolute; left: 0; right: 0; height: 2px;
    background: var(--accent); border-radius: var(--r-pill);
  }
  .char.drop-before::before { top: -5px; }
  .char.drop-after::after { bottom: -5px; }
  .grip { flex: none; width: 8px; margin-left: -5px; color: var(--fg-off); font-size: 11px; line-height: 1; }
  .char:hover .grip, .char.dragging .grip { color: var(--accent); }
  .char:hover { border-color: #9FB4D0; }
  .char.selected {
    background: linear-gradient(180deg, #E4F1FF, #C6E2FF);
    border-color: var(--accent); border-left-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(66, 109, 214, 0.14), inset 0 1px 0 #fff;
  }
  aside.collapsed .char { flex-direction: column; gap: 2px; padding: 8px 4px 7px; }
  aside.collapsed .grip { width: auto; height: 6px; margin: -3px 0 0; }

  .meta { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .meta .name { font-size: 12.5px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta .cls { font-size: 9.5px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .char.selected .meta .name { font-weight: 700; }

  .count { margin-left: auto; flex-shrink: 0; text-align: right; display: flex; flex-direction: column; }
  .count .ok { font-size: 13px; font-weight: 700; white-space: nowrap; }
  .count .total { font-size: 9.5px; color: var(--fg-dim); font-weight: 400; }
  .count .cap { font-size: 8.5px; color: var(--fg-muted); white-space: nowrap; }
  .mini { font-size: 9.5px; font-weight: 700; color: var(--fg-head); }

  .register {
    text-align: center; padding: 9px 6px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, var(--bg-rail));
    border: 1px dashed #9FB4D0; box-shadow: inset 0 1px 0 #fff;
    font-size: 11px; font-weight: 700; color: #2B3C57; white-space: nowrap; overflow: hidden;
  }
  .register:hover { border-style: solid; }

  .note-text { margin: 0; font-size: 9px; line-height: 1.6; }
</style>
