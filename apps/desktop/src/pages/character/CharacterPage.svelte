<script lang="ts">
  // マスター・ディテール構成(docs/claude/goals/2026-08-21-ux-guidelines-character-screen.md)。
  // 一覧は名前・キャラ種のみの簡素な行にし、詳細は CharacterWorkspace(キャラデータ+設定の2カラム)が十分な幅で担う。
  import { deleteCharacter, errorMessage, listCharacters, listGameCharacters } from "../../api/commands";
  import type { GameCharacter, RegisteredCharacter } from "../../api/types";
  import { reportError } from "../../toast.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import CharacterRegisterForm from "./CharacterRegisterForm.svelte";
  import CharacterWorkspace from "./CharacterWorkspace.svelte";

  const DEFAULT_LIST_WIDTH = 280;
  const layoutWidths = persisted("tw-layout-character-list", { list: DEFAULT_LIST_WIDTH });
  const gridTemplateColumns = $derived(`minmax(160px, ${layoutWidths.value.list ?? DEFAULT_LIST_WIDTH}px) 6px minmax(466px, 1fr)`);

  let characters = $state<RegisteredCharacter[]>([]);
  let gameCharacters = $state<GameCharacter[]>([]);
  let loading = $state(true);
  let selectedId = $state<number | null>(null);

  const gameName = (id: string) => gameCharacters.find((g) => g.id === id)?.name ?? id;
  const selectedCharacter = $derived(characters.find((c) => c.id === selectedId) ?? null);

  function handleCreated(c: RegisteredCharacter) {
    characters = [...characters, c];
    selectedId = c.id;
  }

  function handleSaved(c: RegisteredCharacter) {
    characters = characters.map((x) => (x.id === c.id ? c : x));
  }

  $effect(() => {
    Promise.all([listCharacters(), listGameCharacters()])
      .then(([cs, gs]) => { characters = cs; gameCharacters = gs; })
      .catch((e) => reportError(errorMessage(e)))
      .finally(() => (loading = false));
  });

  async function remove(e: MouseEvent, c: RegisteredCharacter) {
    e.stopPropagation();
    try {
      await deleteCharacter(c.id);
      characters = characters.filter((x) => x.id !== c.id);
      if (selectedId === c.id) selectedId = null;
    } catch (err) {
      reportError(errorMessage(err));
    }
  }
</script>

<div class="layout" style="grid-template-columns: {gridTemplateColumns};">
  <section class="list">
    <div class="panel-head"><span class="dot"></span><span class="title">CHARACTERS — 登録一覧</span><span class="count dim">{characters.length} 件</span></div>
    {#if loading}
      <p class="empty dim">読み込み中…</p>
    {:else if characters.length === 0}
      <p class="empty dim">登録キャラはまだありません。下のフォームから登録してください。</p>
    {:else}
      <div class="scroll">
        <table class="grid">
          <tbody>
            {#each characters as c (c.id)}
              <tr class:selected={c.id === selectedId} onclick={() => (selectedId = c.id)}>
                <td class="name">
                  <span>{c.name}</span>
                  <span class="muted">{gameName(c.game_character_id)}</span>
                </td>
                <td class="n actions-cell">
                  <button class="btn danger small" onclick={(e) => remove(e, c)}>削除</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    <div class="panel-head sub"><span class="dot warm"></span><span class="title">REGISTER — 新規登録</span></div>
    <CharacterRegisterForm {gameCharacters} onCreated={handleCreated} />
  </section>

  <Splitter
    bind:value={layoutWidths.value.list}
    min={160}
    defaultValue={DEFAULT_LIST_WIDTH}
    controls="prev"
    label="一覧とキャラデータの境界"
  />

  <section class="detail">
    {#if selectedCharacter}
      {#key selectedCharacter.id}
        <CharacterWorkspace character={selectedCharacter} {gameCharacters} onSaved={handleSaved} />
      {/key}
    {:else}
      <p class="empty dim">キャラを選択するか、左下のフォームから新規登録してください。</p>
    {/if}
  </section>
</div>

<style>
  .layout {
    height: 100%; display: grid;
    background: var(--border); overflow-x: auto;
  }
  section { background: var(--bg); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  section.detail { background: var(--bg-raised); overflow: auto; }
  .count { margin-left: auto; font-size: 10px; }
  .scroll { overflow: auto; min-height: 0; }
  .empty { padding: 20px 14px; font-size: 12px; }
  .panel-head.sub { border-top: 1px solid var(--border); }

  table.grid { width: 100%; }
  tbody tr { cursor: pointer; }
  tbody tr:hover td { background: var(--bg-raised); }
  tbody tr.selected td { background: var(--bg-active); }
  td.name { display: flex; flex-direction: column; gap: 2px; font-weight: 500; }
  td.name .muted { font-weight: 400; font-size: 11px; }
  .btn.small { padding: 3px 8px; font-size: 11px; }
  .actions-cell { display: flex; justify-content: flex-end; }
</style>
