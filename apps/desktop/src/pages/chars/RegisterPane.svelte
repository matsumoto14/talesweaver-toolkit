<script lang="ts">
  // キャラ登録(v4): 呼び名 + 19 職のアイコン選択だけ。詳細は登録後にワークスペースで育てる
  // (docs/ux-guidelines.md 原則3)。「コピー」は選択中キャラの補正源・装備を引き継ぐ。
  import { createCharacter, errorMessage } from "../../api/commands";
  import type { NewCharacter } from "../../api/types";
  import { neutralEquipment, neutralStatSources } from "../../draft";
  import { STAT_KINDS } from "../../labels";
  import {
    app, payloadOf, selectCharacter, selectedCharacter, upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";

  let name = $state("");
  let gameCharacterId = $state("boris");
  let saving = $state(false);

  const source = $derived(selectedCharacter());
  const selectedGame = $derived(app.gameCharacters.find((c) => c.id === gameCharacterId) ?? null);

  async function register(copy: boolean) {
    if (!selectedGame || saving) return;
    saving = true;
    try {
      let payload: NewCharacter;
      if (copy && source) {
        payload = { ...payloadOf(source), name: name.trim() || selectedGame.name, game_character_id: gameCharacterId };
        if (source.game_character_id !== gameCharacterId) {
          // キャラ種が違うコピーでは、旧キャラ専用のキャラスキルバフを落とす(幽霊バフ対策)
          payload.stat_sources.buffs.choices = payload.stat_sources.buffs.choices.filter((ch) => {
            const def = app.catalog.find((d) => d.id === ch.buff_id);
            return !(def && typeof def.group === "object" && "character_skill" in def.group);
          });
        }
      } else {
        payload = {
          name: name.trim() || selectedGame.name,
          game_character_id: gameCharacterId,
          base_stats: Object.fromEntries(STAT_KINDS.map((k) => [k, 1])) as NewCharacter["base_stats"],
          awakening: { stage: 0, eternal_level: 0 },
          stat_sources: neutralStatSources(),
          equipment: neutralEquipment(),
        };
      }
      const saved = await createCharacter(payload);
      upsertCharacter(saved);
      selectCharacter(saved.id);
      app.registerOpen = false;
      name = "";
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      saving = false;
    }
  }
</script>

<div class="pane">
  <div class="card">
    <div class="card-title big">キャラを登録</div>
    <div class="row">
      <span class="label">名前</span>
      <input type="text" bind:value={name} maxlength="32" placeholder="呼び名(空ならキャラの名前)" />
    </div>
    <div class="row">
      <span class="label">キャラ</span>
      <span class="picked">{selectedGame?.name ?? ""}</span>
      <span class="hint dim">素ステ・覚醒はあとで「キャラステータス」で</span>
    </div>
    <div class="grid">
      {#each app.gameCharacters as c (c.id)}
        {@const on = c.id === gameCharacterId}
        <button type="button" class="pick" class:on onclick={() => (gameCharacterId = c.id)}>
          <span class="icon" class:on></span>
          <span class="pick-name">{c.name}</span>
        </button>
      {/each}
    </div>
    <div class="actions">
      <button type="button" class="btn primary" disabled={saving} onclick={() => register(false)}>
        {saving ? "登録中…" : "未装備で登録"}
      </button>
      {#if source}
        <button type="button" class="btn" disabled={saving} onclick={() => register(true)}>
          {source.name} をコピー
        </button>
      {/if}
      {#if app.characters.length > 0}
        <button type="button" class="btn cancel" onclick={() => (app.registerOpen = false)}>閉じる</button>
      {/if}
    </div>
  </div>
  <p class="note dim">登録は名前だけでOK。装備やステータスは登録後にこの画面で育てます(空の項目は中立値で計算されます)。</p>
</div>

<style>
  .pane { max-width: 720px; }
  .card { padding: 13px; border-radius: 12px; }
  .card-title.big { font-size: 12px; color: #26334A; }
  .row { margin-top: 9px; display: flex; align-items: center; gap: 8px; min-width: 0; }
  .label { width: 60px; flex-shrink: 0; font-size: 10.5px; color: var(--fg-muted); }
  input[type="text"] {
    min-width: 0; flex: 1; padding: 6px 9px; border-radius: 8px;
    border: 1px solid var(--accent); background: var(--bg-panel); font-size: 12px; color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent-hover); }
  .picked { font-size: 11.5px; font-weight: 700; }
  .hint { margin-left: auto; font-size: 9.5px; }

  .grid { margin-top: 9px; display: flex; flex-wrap: wrap; gap: 6px; }
  .pick {
    width: 66px; display: flex; flex-direction: column; align-items: center; gap: 4px;
    padding: 7px 4px; border-radius: 10px;
    background: #fff; border: 1px solid var(--border-soft);
  }
  .pick.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); box-shadow: 0 0 0 3px rgba(66, 109, 214, 0.16); }
  .icon {
    width: 30px; height: 30px; border-radius: 9px;
    background: repeating-linear-gradient(135deg, #E4EDF9 0 4px, #CFDFF2 4px 8px);
    border: 1px solid var(--border-strong); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.8);
  }
  .icon.on { border-color: var(--accent); }
  .pick-name { max-width: 58px; font-size: 9.5px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pick.on .pick-name { color: var(--fg); font-weight: 700; }

  .actions { margin-top: 11px; display: flex; gap: 7px; }
  .actions .btn { flex: 1; }
  .actions .btn.cancel { flex: 0 0 auto; }
  .note { margin: 9px 0 0; font-size: 10px; line-height: 1.65; }
</style>
