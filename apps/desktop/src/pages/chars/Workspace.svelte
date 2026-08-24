<script lang="ts">
  // 選択キャラのワークスペース(v4): 左に補正源リスト、右に選択した補正源の編集ペイン、
  // 下に「いまの実力」シート。draft(編集状態)を 1 つの $state に持ち、保存はキャラ単位で 1 ボタン。
  // 親 CharsPage が {#key character.id} で作り直す前提($effect による再同期は書かない)。
  import { untrack } from "svelte";
  import { errorMessage, previewEffectiveStats, updateCharacter } from "../../api/commands";
  import type { Equipment, RegisteredCharacter, StatPreview, StatSources } from "../../api/types";
  import { deleteCharacter } from "../../api/commands";
  import { buildDraft, draftToPayload, type Draft } from "../../draft";
  import {
    equipmentBaseTotal, equipmentEnchantTotal, sienaAttackRatePercent, sienaPartCount,
    sienaStatTotal, thesisCoresBestTotal,
  } from "../../equipment";
  import { fmtInt, fmtNum } from "../../format";
  import { EQUIPMENT_STAT_KINDS, STAT_KINDS, STAT_LABELS } from "../../labels";
  import { app, loadSkills, removeCharacter, skillsByCharacter, upsertCharacter } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import SourcePane, { type SourceId } from "./SourcePane.svelte";

  interface Props {
    character: RegisteredCharacter;
  }
  let { character }: Props = $props();

  // 表のヘッダは狭いので短縮名にする(EQUIPMENT_STAT_LABELS は「突き攻撃力」等で長い)
  const EQUIPMENT_STAT_SHORT = { thrust: "突き", slash: "斬り", magic_attack: "魔攻", magic_defense: "魔防" };

  const DEFAULT_LIST_WIDTH = 280;
  const layoutWidths = persisted("tw-v4-chars", { list: DEFAULT_LIST_WIDTH });
  const gridTemplateColumns = $derived(
    `minmax(220px, ${layoutWidths.value.list ?? DEFAULT_LIST_WIDTH}px) 6px minmax(300px, 1fr)`,
  );
  const sheetOpen = persisted("tw-v4-strength", { open: false });

  // 親が {#key} でこのコンポーネントを作り直す前提なので初期値だけ untrack で取る。
  const initial = untrack(() => character);
  let initialSnapshot = $state(JSON.stringify(buildDraft(initial)));
  let draft = $state<Draft>(buildDraft(initial));
  const dirty = $derived(JSON.stringify(draft) !== initialSnapshot);

  let saving = $state(false);
  const canSubmit = $derived(draft.name.trim().length > 0 && draft.gameCharacterId !== "" && !saving && dirty);

  // キャラ種を切り替えたら旧キャラ専用のキャラスキルバフを落とす(幽霊バフ対策、既存決定を踏襲)。
  let lastGameCharacterId = draft.gameCharacterId;
  $effect(() => {
    const currentId = draft.gameCharacterId;
    if (currentId === lastGameCharacterId || app.catalog.length === 0) return;
    lastGameCharacterId = currentId;
    draft.statSources.buffs.choices = draft.statSources.buffs.choices.filter((choice) => {
      const def = app.catalog.find((d) => d.id === choice.buff_id);
      if (!def || typeof def.group !== "object" || !("character_skill" in def.group)) return true;
      return def.group.character_skill.game_character_id === currentId;
    });
  });

  // 主軸スキル(攻撃力の依存種別を決める)。選択肢はキャラ種のスキル一覧。
  $effect(() => {
    void loadSkills(draft.gameCharacterId);
  });
  const skills = $derived(skillsByCharacter[draft.gameCharacterId] ?? []);
  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);

  // 即時プレビュー(100ms debounce)。エラーはペイン内に控えめに表示(トーストは出さない)。
  let preview = $state<StatPreview | null>(null);
  let previewError = $state<string | null>(null);
  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  let previewSeq = 0;
  $effect(() => {
    const baseStats = { ...draft.baseStats };
    const statSources = JSON.parse(JSON.stringify(draft.statSources)) as StatSources;
    // シエナのオーラのステ加算が最終能力値に乗るので、装備もプレビューの入力に含める
    const equipment = JSON.parse(JSON.stringify(draft.equipment)) as Equipment;
    const mainSkillId = draft.mainSkillId === "" ? null : draft.mainSkillId;
    const gameCharacterId = draft.gameCharacterId;
    if (debounceHandle) clearTimeout(debounceHandle);
    const seq = ++previewSeq;
    debounceHandle = setTimeout(() => {
      previewEffectiveStats(baseStats, statSources, equipment, gameCharacterId, mainSkillId)
        .then((p) => {
          if (seq === previewSeq) {
            preview = p;
            previewError = null;
          }
        })
        .catch((e) => {
          if (seq === previewSeq) previewError = errorMessage(e);
        });
    }, 100);
    return () => {
      if (debounceHandle) clearTimeout(debounceHandle);
    };
  });

  async function save() {
    if (!canSubmit) return;
    saving = true;
    try {
      const saved = await updateCharacter(character.id, draftToPayload(draft));
      initialSnapshot = JSON.stringify(draft);
      upsertCharacter(saved);
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      saving = false;
    }
  }

  let confirmDelete = $state(false);
  async function removeThis() {
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 4000);
      return;
    }
    try {
      await deleteCharacter(character.id);
      removeCharacter(character.id);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }

  // --- 補正源リスト -------------------------------------------------------
  let openSource = $state<SourceId>("status");

  const petCount = $derived(STAT_KINDS.filter((k) => draft.statSources.pet_skills[k] !== null).length);
  const runeTotal = $derived(STAT_KINDS.reduce((s, k) => s + draft.statSources.rune_levels[k], 0));
  const crownTotal = $derived(STAT_KINDS.reduce((s, k) => s + draft.statSources.crown[k], 0));
  const relicTotal = $derived(STAT_KINDS.reduce((s, k) => s + draft.statSources.sacred_relic[k] * 10, 0));
  const skillCount = $derived(
    draft.statSources.buffs.choices.filter((c) => {
      const def = app.catalog.find((d) => d.id === c.buff_id);
      return def && (def.group === "ally_skill" || (typeof def.group === "object" && "character_skill" in def.group));
    }).length,
  );
  const adjustCount = $derived(
    STAT_KINDS.filter((k) => draft.statSources.adjustments[k].add !== 0 || draft.statSources.adjustments[k].pin !== null).length,
  );
  const enhanceRatePercent = $derived(
    (draft.equipment.power_weapon ? 2 : 0) + draft.equipment.strong_weapon_level * 3,
  );
  const eqBaseTotal = $derived(equipmentBaseTotal(draft.equipment));
  const eqEnchantTotal = $derived(equipmentEnchantTotal(draft.equipment));
  const sienaParts = $derived(sienaPartCount(draft.equipment));
  const sienaRate = $derived(sienaAttackRatePercent(draft.equipment));
  const sienaStats = $derived(sienaStatTotal(draft.equipment));
  const coreBestTotal = $derived(thesisCoresBestTotal(draft.equipment.thesis_cores));

  const NEUTRAL = "未設定(中立値で計算)";
  const sources = $derived<{ id: SourceId; name: string; sub: string }[]>([
    { id: "status", name: "キャラステータス", sub: `覚醒 ${draft.stage} 段階 ・ エタの意志 Lv${draft.eternalLevel}` },
    {
      id: "equipment",
      name: "装備",
      sub: `基本合計 突${fmtInt(eqBaseTotal.thrust)} / 斬${fmtInt(eqBaseTotal.slash)}${enhanceRatePercent > 0 ? ` ・ +${enhanceRatePercent}%` : ""}`,
    },
    {
      id: "siena",
      name: "シエナのオーラ",
      sub:
        sienaParts > 0
          ? `${sienaParts} 部位 ・ 攻撃力 +${sienaRate}%${sienaStats > 0 ? ` ・ ステ +${fmtInt(sienaStats)}` : ""}`
          : NEUTRAL,
    },
    {
      id: "thesis",
      name: "テシスコア",
      sub: coreBestTotal > 0 ? `最大 合計 ${fmtInt(coreBestTotal)}` : NEUTRAL,
    },
    { id: "relic", name: "神鳥の聖物", sub: relicTotal > 0 ? `合計 +${fmtInt(relicTotal)}` : NEUTRAL },
    { id: "crown", name: "クラウン", sub: crownTotal > 0 ? `合計 +${fmtInt(crownTotal)}` : NEUTRAL },
    { id: "skills", name: "キャラスキル", sub: skillCount > 0 ? `${skillCount} 件選択` : NEUTRAL },
    { id: "pet", name: "ペット S スキル", sub: petCount > 0 ? `${petCount} 種` : NEUTRAL },
    { id: "rune", name: "ルーンスキル", sub: runeTotal > 0 ? `合計 +${fmtInt(runeTotal)}` : NEUTRAL },
    { id: "adjust", name: "調整", sub: adjustCount > 0 ? `${adjustCount} ステに適用` : NEUTRAL },
  ]);
  // 並びは 12a の指定順(キャラステータス / 装備 / シエナ / テシスコア / 聖物 / クラウン /
  // スキル / モンスターカード / ペット)。12a に無いルーン・調整はその後ろに置く。
  const PLANNED = ["モンスターカード", "称号"];
  const neutralCount = $derived(sources.filter((s) => s.sub === NEUTRAL).length);

  // --- いまの実力 ---------------------------------------------------------
  const totalContents = $derived(app.areas.reduce((n, a) => n + a.contents.length, 0));
  const savedClearCount = $derived((app.evaluations[character.id] ?? []).filter((e) => e.clear).length);
</script>

<div class="workspace">
  <div class="toolbar">
    <span class="char-name">{draft.name || "(名前未設定)"}</span>
    {#if dirty}<span class="unsaved">未保存</span>{/if}
    <span class="spacer"></span>
    <button type="button" class="btn primary" disabled={!canSubmit} onclick={save}>
      {saving ? "保存中…" : "保存"}
    </button>
  </div>

  <div class="cols" style="grid-template-columns: {gridTemplateColumns};">
    <section class="sources">
      <div class="src-head">
        <span class="src-title">補正源</span>
        {#if neutralCount > 0}<span class="src-unset">未設定 {neutralCount} 件</span>{/if}
        <span class="dim">押して中身を変える</span>
      </div>
      <div class="src-list">
        {#each sources as s (s.id)}
          <button type="button" class="src" class:on={openSource === s.id} onclick={() => (openSource = s.id)}>
            <span class="src-main">
              <span class="src-name">{s.name}</span>
              <span class="src-sub num">{s.sub}</span>
            </span>
            <span class="chev dim">›</span>
          </button>
        {/each}
        {#each PLANNED as name (name)}
          <div class="src planned">
            <span class="src-main">
              <span class="src-name">{name}</span>
              <span class="src-sub">これから</span>
            </span>
          </div>
        {/each}
      </div>
      <div class="attack-foot" class:empty={!preview?.attack}>
        <div class="attack-head">
          <span class="attack-label">いまの攻撃力</span>
          <span class="attack-skill dim">{mainSkill ? mainSkill.name : "主軸スキル未選択"}</span>
        </div>
        {#if preview?.attack}
          <div class="attack-value num">{fmtInt(preview.attack.breakdown.value)}</div>
          <div class="attack-parts num dim">
            ステ {fmtNum(Math.floor(preview.attack.breakdown.stat_attack))}
            ・ 装備基本 {fmtNum(Math.floor(preview.attack.breakdown.equipment_base_attack))}
            ・ 装備強化 {fmtNum(Math.floor(preview.attack.breakdown.equipment_enhanced_attack))}
            ・ 強化倍率 +{Math.round(preview.attack.breakdown.enhance_rate * 100)}%
          </div>
          <p class="attack-note dim">テシスコアの能力値は地域ごとなので、この値には入っていません(ダメージ計算タブでは対象の地域で入ります)。</p>
        {:else}
          <p class="attack-note dim">「キャラステータス」で<b>主軸スキル</b>を選ぶと攻撃力が出ます。</p>
        {/if}
      </div>
      <p class="src-note dim">常用バフは<b>ダメージ計算</b>タブの「計算の材料」で選べます。グレーの補正源はこれから。</p>
    </section>

    <Splitter
      bind:value={layoutWidths.value.list}
      min={220}
      defaultValue={DEFAULT_LIST_WIDTH}
      controls="prev"
      label="補正源リストと編集ペインの境界"
    />

    <section class="detail">
      <SourcePane {draft} {preview} {previewError} {skills} sourceId={openSource} />
    </section>
  </div>

  <div class="sheet">
    <button type="button" class="sheet-trigger" onclick={() => (sheetOpen.value.open = !sheetOpen.value.open)}>
      <span class="sheet-title">いまの実力</span>
      <span class="sheet-summary num dim">
        {preview
          ? STAT_KINDS.map((k) => `${STAT_LABELS[k]} ${fmtInt(preview!.stats[k])}`).join(" ・ ")
          : "計算中…"}
      </span>
      <span class="sheet-chev">{sheetOpen.value.open ? "▴" : "▾"}</span>
    </button>
    {#if sheetOpen.value.open}
      <div class="sheet-body">
        <div class="sheet-card">
          <div class="card-title">最終能力値</div>
          <div class="stat-grid">
            {#each STAT_KINDS as k (k)}
              <span class="stat-cell">
                <span class="dim">{STAT_LABELS[k]}</span>
                <span class="num strong">{preview ? fmtInt(preview.stats[k]) : "—"}</span>
              </span>
            {/each}
          </div>
        </div>
        <div class="sheet-card">
          <div class="card-title">攻撃力(A){mainSkill ? ` — ${mainSkill.name}` : ""}</div>
          {#if preview?.attack}
            <div class="clear num"><span class="strong">{fmtInt(preview.attack.breakdown.value)}</span></div>
            <div class="eq-summary num">
              <span><span class="dim">ステ攻撃力</span> {fmtNum(preview.attack.breakdown.stat_attack)}</span>
              <span><span class="dim">装備基本</span> {fmtNum(preview.attack.breakdown.equipment_base_attack)}</span>
              <span><span class="dim">装備強化</span> {fmtNum(preview.attack.breakdown.equipment_enhanced_attack)}</span>
              <span><span class="dim">強化倍率</span> +{Math.round(preview.attack.breakdown.enhance_rate * 100)}%</span>
            </div>
            <p class="dim tiny">テシスコアの能力値は地域ごとのため未加算(地域なしの値)。</p>
          {:else}
            <p class="dim tiny">「キャラステータス」で<b>主軸スキル</b>を選ぶと攻撃力が出ます。</p>
          {/if}
        </div>
        <div class="sheet-card">
          <div class="card-title">装備値(全部位の合計)</div>
          <table class="eq-table num">
            <thead>
              <tr>
                <th></th>
                {#each EQUIPMENT_STAT_KINDS as k (k)}<th class="n">{EQUIPMENT_STAT_SHORT[k]}</th>{/each}
              </tr>
            </thead>
            <tbody>
              <tr>
                <th class="rh">基本</th>
                {#each EQUIPMENT_STAT_KINDS as k (k)}<td class="n">{fmtInt(eqBaseTotal[k])}</td>{/each}
              </tr>
              <tr>
                <th class="rh">強化</th>
                {#each EQUIPMENT_STAT_KINDS as k (k)}<td class="n">{fmtInt(eqEnchantTotal[k])}</td>{/each}
              </tr>
            </tbody>
          </table>
          <p class="dim tiny">
            強化倍率 +{enhanceRatePercent}%。強化のうちテシスコア・シエナのオーラの分はこの表に入りません
            (それぞれの補正源で入力した分が計算時に強化能力値へ合流します)。
          </p>
        </div>
        <div class="sheet-card">
          <div class="card-title">このキャラで通るのは</div>
          <div class="clear num"><span class="strong">{savedClearCount}</span><span class="dim"> / {totalContents}</span></div>
          <p class="dim tiny">保存済みデータでの判定。一覧は<b>ホーム</b>で。</p>
        </div>
        <button type="button" class="delete" class:confirm={confirmDelete} onclick={removeThis}>
          {confirmDelete ? "もう一度押すと削除します" : "このキャラを削除"}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .workspace { flex: 1; min-height: 0; display: flex; flex-direction: column; }

  .toolbar {
    flex-shrink: 0; display: flex; align-items: center; gap: 10px;
    padding: 10px 16px 0;
  }
  .char-name { font-size: 13px; font-weight: 800; }
  .unsaved {
    font-size: 9.5px; font-weight: 700; letter-spacing: 0.08em; color: var(--warm);
    border: 1px solid var(--warm); border-radius: 999px; padding: 1px 8px;
  }
  .spacer { flex: 1; }

  .cols { flex: 1; min-height: 0; display: grid; padding: 10px 16px 8px; column-gap: 0; }
  section { min-width: 0; min-height: 0; display: flex; flex-direction: column; }

  .src-head { display: flex; align-items: baseline; gap: 8px; padding: 0 2px 7px; }
  .src-title { font-size: 10.5px; font-weight: 800; letter-spacing: 0.08em; color: #26334A; }
  .src-head .dim { margin-left: auto; font-size: 9px; }
  .src-unset {
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted);
    border: 1px solid var(--border); border-radius: 999px; padding: 0 6px;
  }
  .src-list { flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
  .src {
    display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 10px;
    background: #fff; border: 1px solid var(--border-soft); text-align: left;
  }
  .src:hover:not(.planned) { border-color: var(--accent); }
  .src.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); }
  .src.planned { background: #F0F3F7; border-style: dashed; cursor: default; }
  .src-main { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .src-name { font-size: 11px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .src.planned .src-name, .src.planned .src-sub { color: #A9B4C4; }
  .src-sub { font-size: 9px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { flex-shrink: 0; font-size: 11px; }
  .attack-foot {
    flex-shrink: 0; margin-top: 8px; padding: 9px 11px; border-radius: 10px;
    background: linear-gradient(180deg, #fff, #EFF5FD); border: 1px solid #9FB4D0;
  }
  .attack-foot.empty { background: var(--bg-rail); border-style: dashed; border-color: var(--border); }
  .attack-head { display: flex; align-items: baseline; gap: 8px; }
  .attack-label { font-size: 10px; font-weight: 800; letter-spacing: 0.08em; color: #26334A; }
  .attack-skill { margin-left: auto; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .attack-value { margin-top: 2px; font-size: 22px; font-weight: 700; line-height: 1.1; }
  .attack-parts { margin-top: 3px; font-size: 9px; line-height: 1.6; }
  .attack-note { margin: 4px 0 0; font-size: 9px; line-height: 1.55; }

  .src-note {
    flex-shrink: 0; margin: 10px 0 0; padding: 9px 11px; border-radius: 10px;
    background: var(--bg-rail); border: 1px dashed var(--border);
    font-size: 9.5px; line-height: 1.5; color: var(--fg-muted);
  }

  .detail { overflow: auto; padding-left: 6px; }

  .sheet { flex-shrink: 0; border-top: 1px solid var(--border-strong); background: var(--bg-mid); padding: 8px 16px 10px; }
  .sheet-trigger {
    width: 100%; display: flex; align-items: center; gap: 9px; padding: 8px 11px; border-radius: 10px;
    background: linear-gradient(180deg, #fff, #F1F6FC); border: 1px solid #9FB4D0;
    box-shadow: inset 0 1px 0 #fff; text-align: left;
  }
  .sheet-trigger:hover { border-color: #6382AD; }
  .sheet-title { flex-shrink: 0; font-size: 10.5px; font-weight: 700; letter-spacing: 0.06em; color: #26334A; white-space: nowrap; }
  .sheet-summary { min-width: 0; flex: 1; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sheet-chev {
    flex-shrink: 0; width: 20px; height: 20px; display: flex; align-items: center; justify-content: center;
    border-radius: 6px; background: #fff; border: 1px solid var(--border);
    font-size: 9px; font-weight: 700; color: var(--accent);
  }
  .sheet-body { margin-top: 8px; max-height: 220px; overflow: auto; display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-start; }
  .sheet-card {
    flex: 1 1 240px; min-width: 0; padding: 11px 12px; border-radius: 11px;
    background: #fff; border: 1px solid var(--border-strong);
  }
  .stat-grid { margin-top: 6px; display: flex; flex-wrap: wrap; gap: 5px 12px; }
  .stat-cell { display: flex; align-items: baseline; gap: 5px; font-size: 10px; }
  .stat-cell .strong { font-size: 12px; font-weight: 700; }
  .eq-summary { margin-top: 6px; display: flex; flex-wrap: wrap; gap: 5px 14px; font-size: 11px; }
  .eq-table { margin-top: 6px; width: 100%; border-collapse: collapse; font-size: 11px; }
  .eq-table th, .eq-table td { padding: 3px 6px; border-bottom: 1px solid var(--border-soft); }
  .eq-table thead th { font-size: 9px; font-weight: 700; color: var(--fg-muted); }
  .eq-table .rh { text-align: left; font-size: 10px; color: var(--fg-muted); font-weight: 700; }
  .eq-table .n { text-align: right; }
  .clear { margin-top: 4px; }
  .clear .strong { font-size: 20px; font-weight: 700; }
  .tiny { margin: 4px 0 0; font-size: 9.5px; line-height: 1.6; }
  .delete {
    flex-shrink: 0; align-self: stretch; padding: 8px 14px; border-radius: 9px;
    background: #fff; border: 1px solid #B08480; font-size: 10.5px; color: var(--danger);
  }
  .delete.confirm { background: #F6E8E5; font-weight: 700; }
</style>
