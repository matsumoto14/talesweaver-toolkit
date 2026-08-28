<script lang="ts">
  // キャラ登録(v4): 呼び名 + 19 職のアイコン選択だけ。詳細は登録後にワークスペースで育てる
  // (docs/ux-guidelines.md 原則3)。「コピー」は選択中キャラの補正源・装備を引き継ぐ。
  import { createCharacter, errorMessage } from "../../api/commands";
  import { dropForeignSkills, mainSkillOptions as buildMainSkillOptions } from "../../characterSkills";
  import type { NewCharacter } from "../../api/types";
  import { DEFAULT_AWAKENING_STAGE, defaultCommonSkills, defaultEquipment, neutralStatSources } from "../../draft";
  import { STAT_KINDS } from "../../labels";
  import {
    app, loadSkills, payloadOf, selectCharacter, selectedCharacter, skillsByCharacter, upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Picker from "../../ui/Picker.svelte";

  let name = $state("");
  let gameCharacterId = $state("boris");
  let mainSkillId = $state("");
  let saving = $state(false);

  const source = $derived(selectedCharacter());
  const selectedGame = $derived(app.gameCharacters.find((c) => c.id === gameCharacterId) ?? null);

  // 主軸スキル(攻撃力の依存種別を決める)。スキル未収録のキャラがあるので未選択を許す。
  $effect(() => {
    void loadSkills(gameCharacterId);
  });
  const skills = $derived(skillsByCharacter[gameCharacterId] ?? []);
  /** 火力の高い順(StatusPane と同じ形) */
  const mainSkillOptions = $derived(buildMainSkillOptions(skills, "未選択(あとで選ぶ)"));
  /** キャラを選び直したら前キャラのスキル id を残さない */
  function pickGameCharacter(id: string) {
    if (id === gameCharacterId) return;
    gameCharacterId = id;
    mainSkillId = "";
  }

  async function register(copy: boolean) {
    if (!selectedGame || saving) return;
    saving = true;
    try {
      let payload: NewCharacter;
      if (copy && source) {
        payload = {
          ...payloadOf(source),
          name: name.trim() || selectedGame.name,
          game_character_id: gameCharacterId,
          main_skill_id: mainSkillId === "" ? null : mainSkillId,
        };
        if (source.game_character_id !== gameCharacterId) {
          // キャラ種が違うコピーでは、旧キャラ専用のキャラスキルを落とす(幽霊スキル対策)
          payload.stat_sources.character_skills.skill_ids = dropForeignSkills(
            payload.stat_sources.character_skills.skill_ids,
            app.characterSkills,
            gameCharacterId,
          );
        }
      } else {
        payload = {
          name: name.trim() || selectedGame.name,
          game_character_id: gameCharacterId,
          base_stats: Object.fromEntries(STAT_KINDS.map((k) => [k, 1])) as NewCharacter["base_stats"],
          // このツールのターゲット層は**覚醒 5**(遅くても 4)。既定を 0 にすると
          // ほぼ全員が毎回上書きすることになる(ux-guidelines「初期値は実用値」)
          awakening: { stage: DEFAULT_AWAKENING_STAGE, eternal_level: 0 },
          stat_sources: neutralStatSources(),
          equipment: defaultEquipment(),
          common_skills: defaultCommonSkills(),
          main_skill_id: mainSkillId === "" ? null : mainSkillId,
        };
      }
      const saved = await createCharacter(payload);
      upsertCharacter(saved);
      selectCharacter(saved.id);
      app.registerOpen = false;
      name = "";
      mainSkillId = "";
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
        <button type="button" class="pick" class:on onclick={() => pickGameCharacter(c.id)}>
          <Icon kind="character" id={c.id} size={40} label={c.name} />
          <span class="pick-name">{c.name}</span>
        </button>
      {/each}
    </div>
    <div class="row skill-row">
      <span class="label">主軸スキル</span>
      <span class="skill-select">
        <Picker options={mainSkillOptions} note="火力の高い順(倍率 × 段数)" placeholder="あとで選ぶ" bind:value={mainSkillId} />
      </span>
      <span class="hint dim">
        {skills.length === 0 ? "このキャラのスキルは未収録" : "攻撃力の依存種別を決めます。あとで変更できます"}
      </span>
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
  <p class="note dim">
    登録は名前だけでOK。装備やステータスは登録後にこの画面で育てます(空の項目は中立値で計算されます)。
    パワーウェポンとストロングウェポン Lv6(合計 +20%)は既定で入ります(装備ペインで変更できます)。
  </p>
</div>

<style>
  .pane { max-width: 720px; }
  .card { padding: 13px; border-radius: var(--r-window); }
  .card-title.big { font-size: 12px; color: var(--fg-head); }
  .row { margin-top: 9px; display: flex; align-items: center; gap: 8px; min-width: 0; }
  .label { width: 60px; flex-shrink: 0; font-size: var(--t-label); color: var(--fg-muted); }
  input[type="text"] {
    min-width: 0; flex: 1; padding: 6px 9px; border-radius: var(--r-panel);
    border: 1px solid var(--accent); background: var(--bg-panel); font-size: 12px; color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent-hover); }
  .picked { font-size: 11.5px; font-weight: 700; }
  .hint { margin-left: auto; font-size: 9.5px; }

  .grid { margin-top: 9px; display: flex; flex-wrap: wrap; gap: 6px; }
  .pick {
    width: 66px; display: flex; flex-direction: column; align-items: center; gap: 4px;
    padding: 7px 4px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft);
  }
  .pick.on { background: var(--sel-card); border-color: var(--accent); box-shadow: 0 0 0 3px rgba(66, 109, 214, 0.16); }
  .pick-name { max-width: 58px; font-size: 9.5px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pick.on .pick-name { color: var(--fg); font-weight: 700; }

  .skill-row { margin-top: 11px; }
  .skill-select { min-width: 0; flex: 1; }
  .actions { margin-top: 11px; display: flex; gap: 7px; }
  .actions .btn { flex: 1; }
  .actions .btn.cancel { flex: 0 0 auto; }
  .note { margin: 9px 0 0; font-size: 10px; line-height: 1.65; }
</style>
