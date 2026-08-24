<script lang="ts">
  // ホーム: 選択中キャラが「どこにどのくらい通るか」の到達一覧(v4 デザイン)。
  // 判定はすべて Rust 側(evaluate_contents)。この画面は表示と選択のみ。
  import { errorMessage, listSkills, previewDamage } from "../../api/commands";
  import type { Content, ContentEvaluation } from "../../api/types";
  import { candidatesFor, COST_COLORS, type Candidate } from "../../candidates";
  import { fmtInt } from "../../format";
  import {
    app, evaluationFor, flatContents, payloadOf, selectedCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Splitter from "../../ui/Splitter.svelte";

  const DEFAULT_RIGHT_WIDTH = 330;
  const layoutWidths = persisted("tw-v4-home", { right: DEFAULT_RIGHT_WIDTH });
  const gridTemplateColumns = $derived(
    `minmax(300px, 1fr) 6px minmax(240px, ${layoutWidths.value.right ?? DEFAULT_RIGHT_WIDTH}px)`,
  );

  const pins = persisted("tw-v4-pins", { ids: [] as string[] });

  const character = $derived(selectedCharacter());
  const totalCount = $derived(app.areas.reduce((n, a) => n + a.contents.length, 0));

  // 判定に使ったスキル名の表示用(evaluate_contents は「最大ダメージのスキル」で判定する)
  let skillNames = $state<Record<string, string>>({});
  $effect(() => {
    const gid = character?.game_character_id;
    if (!gid) {
      skillNames = {};
      return;
    }
    listSkills(gid)
      .then((list) => (skillNames = Object.fromEntries(list.map((s) => [s.id, s.name]))))
      .catch((e) => reportError(errorMessage(e)));
  });

  interface Row {
    areaId: string;
    areaName: string;
    content: Content;
    ev: ContentEvaluation | null;
  }
  const rows = $derived<Row[]>(
    flatContents().map(({ areaId, areaName, content }) => ({
      areaId,
      areaName,
      content,
      ev: character ? evaluationFor(character.id, content.id) : null,
    })),
  );
  const clearCount = $derived(rows.filter((r) => r.ev?.clear).length);
  const entryCount = $derived(rows.filter((r) => r.ev?.entry_ok).length);

  // 行の状態: 0余裕 1通る 2ぎりぎり 3届かない 4条件・火力とも未達 5条件だけ未達 6スキル未収録
  function rowState(r: Row): number {
    if (!r.ev || !r.ev.damage) return 6;
    const ratio = r.ev.damage.per_hit_max / r.content.need_per_hit;
    if (!r.ev.entry_ok) return r.ev.reaches_need ? 5 : 4;
    return ratio >= 1.3 ? 0 : ratio >= 1 ? 1 : ratio >= 0.8 ? 2 : 3;
  }
  const BADGE = ["余裕", "通る", "ぎりぎり", "届かない", "条件・火力とも未達", "条件だけ未達", "スキル未収録"];
  const BADGE_BG = ["#DCEBFF", "#DFF3E6", "#FDF3DE", "#F6E8E5", "#ECEEF2", "#EFEEF8", "#ECEEF2"];
  const BADGE_BD = ["#426DD6", "#6FA98A", "#C2A057", "#B08480", "#A9B4C4", "#6D6AA8", "#A9B4C4"];
  const BADGE_FG = ["#2B4FA8", "#2E6B4C", "#7A6420", "#8C4A42", "#5E6E88", "#4A4780", "#5E6E88"];
  const BAR_BG = [
    "linear-gradient(90deg,#90D7FF,#426DD6)",
    "linear-gradient(90deg,#9FD9BC,#3E8C63)",
    "linear-gradient(90deg,#F0D79A,#C2A057)",
    "linear-gradient(90deg,#E8B3A2,#B0574A)",
    "linear-gradient(90deg,#CBD3DE,#9AA6B6)",
    "linear-gradient(90deg,#C3C1E4,#6D6AA8)",
    "linear-gradient(90deg,#CBD3DE,#9AA6B6)",
  ];

  const ratioOf = (r: Row) => (r.ev?.damage ? r.ev.damage.per_hit_max / r.content.need_per_hit : 0);
  const pctOf = (r: Row) => `${Math.min(100, ratioOf(r) * 100).toFixed(1)}%`;

  function noteOf(r: Row): { text: string; unmet: boolean } {
    if (!r.ev) return { text: "判定中…", unmet: false };
    if (!r.ev.damage) return { text: "このキャラのスキルデータが未収録のため火力を判定できません", unmet: false };
    const lackPct = Math.max(1, Math.round((1 - ratioOf(r)) * 100));
    if (r.content.requirements.length === 0) {
      return r.ev.reaches_need
        ? { text: "入場条件なし", unmet: false }
        : { text: `入場条件なし ／ 火力が あと ${lackPct}%`, unmet: false };
    }
    if (r.ev.entry_ok) {
      return r.ev.reaches_need
        ? { text: `入場条件OK(${r.ev.checks.map((c) => `${c.label} ${fmtInt(c.required)}`).join(" / ")})`, unmet: false }
        : { text: `入場条件OK ／ 火力が あと ${lackPct}%`, unmet: false };
    }
    const unmet = r.ev.checks.filter((c) => !c.ok);
    return {
      text: `入場まで: ${unmet.map((c) => `${c.label} あと ${fmtInt(c.required - c.current)}`).join(" ・ ")}`,
      unmet: true,
    };
  }

  // 選択とフロンティア(最初の未クリア)
  let selectedContentId = $state<string | null>(null);
  const frontierId = $derived(rows.find((r) => r.ev && !r.ev.clear)?.content.id ?? null);
  const selectedRow = $derived(
    rows.find((r) => r.content.id === selectedContentId) ?? rows.find((r) => r.content.id === frontierId) ?? rows[0] ?? null,
  );

  // エリアの開閉(既定: 全クリアのエリアは畳む)
  let openAreas = $state<Record<string, boolean>>({});
  function areaRows(areaId: string): Row[] {
    return rows.filter((r) => r.areaId === areaId);
  }
  function isAreaOpen(areaId: string): boolean {
    const explicit = openAreas[areaId];
    if (explicit !== undefined) return explicit;
    return !areaRows(areaId).every((r) => r.ev?.clear);
  }
  function toggleArea(areaId: string) {
    openAreas[areaId] = !isAreaOpen(areaId);
  }

  // お気に入り(localStorage のみ。未クリアだけ表示)
  const pinned = (id: string) => pins.value.ids.includes(id);
  function togglePin(e: MouseEvent, id: string) {
    e.stopPropagation();
    pins.value.ids = pinned(id) ? pins.value.ids.filter((x) => x !== id) : [...pins.value.ids, id];
  }
  const favRows = $derived(
    pins.value.ids
      .map((id) => rows.find((r) => r.content.id === id))
      .filter((r): r is Row => !!r && !!r.ev && !r.ev.clear),
  );
  const favDone = $derived(pins.value.ids.length - favRows.length);

  // 次に変えるなら: 目標 = お気に入りの先頭 → 最初の未クリア → 選択中
  const goal = $derived(favRows[0] ?? rows.find((r) => r.ev && !r.ev.clear) ?? selectedRow);

  interface Advice {
    candidate: Candidate;
    perHit: number;
    deltaPct: number;
  }
  let advice = $state<Advice[]>([]);
  let adviceSeq = 0;
  let adviceHandle: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const c = character;
    const g = goal;
    // 依存を明示的に読む(保存データの変化でも再計算する)
    void app.evaluations[c?.id ?? -1];
    if (adviceHandle) clearTimeout(adviceHandle);
    if (!c || !g?.ev?.damage) {
      advice = [];
      return;
    }
    const skillId = g.ev.damage.skill_id;
    const enemyId = g.content.enemy_id;
    const baseDamage = g.ev.damage.per_hit_max;
    const seq = ++adviceSeq;
    adviceHandle = setTimeout(async () => {
      try {
        const list = candidatesFor(payloadOf(c));
        const results = await Promise.all(
          list.map(async (candidate) => {
            const p = payloadOf(c);
            candidate.apply(p);
            const r = await previewDamage(p, skillId, enemyId, 0);
            return { candidate, perHit: r.per_hit.max, deltaPct: Math.round((r.per_hit.max / baseDamage - 1) * 100) };
          }),
        );
        if (seq === adviceSeq) advice = results.sort((a, b) => b.perHit - a.perHit);
      } catch (e) {
        if (seq === adviceSeq) reportError(errorMessage(e));
      }
    }, 150);
    return () => {
      if (adviceHandle) clearTimeout(adviceHandle);
    };
  });

  function applyAdvice(a: Advice) {
    if (!character || !goal) return;
    const p = payloadOf(character);
    a.candidate.apply(p);
    app.sim = p;
    app.calcTargetId = goal.content.id;
    app.calcSkillId = goal.ev?.damage?.skill_id ?? null;
    app.tab = "calc";
  }

  function tryInCalc() {
    if (!selectedRow) return;
    app.calcTargetId = selectedRow.content.id;
    app.calcSkillId = selectedRow.ev?.damage?.skill_id ?? null;
    app.tab = "calc";
  }

  const areas = $derived(app.areas);
</script>

<div class="layout" style="grid-template-columns: {gridTemplateColumns};">
  <section class="mid">
    <div class="head-bar">
      <span class="title">どこにどのくらい通るか</span>
      <span class="note">目安＝実用的に周回できる1発量(仮値)</span>
    </div>
    <div class="scroll">
      {#if !character}
        <p class="empty dim">キャラを登録すると、ここに到達一覧が出ます。左下の「＋ キャラを登録」からどうぞ。</p>
      {:else}
        <div class="summary">
          <span class="cap">クリアできるのは</span>
          <span class="big-wrap"><span class="big num">{clearCount}</span><span class="of num">/ {totalCount}</span></span>
          <span class="entry-pill">
            <span class="dim">入場条件だけなら</span>
            <span class="num strong">{entryCount}</span>
            <span class="num dim">/ {totalCount}</span>
          </span>
        </div>

        <div class="areas">
          {#each areas as area (area.id)}
            {@const open = isAreaOpen(area.id)}
            {@const okCount = areaRows(area.id).filter((r) => r.ev?.clear).length}
            <div class="area">
              <div class="area-head">
                <span class="area-name">{area.name}</span>
                {#if open}
                  <span class="area-rule"></span>
                {:else}
                  <button type="button" class="collapsed-note" onclick={() => toggleArea(area.id)}>
                    <span class="ok-dot"></span>
                    <span class="dim">全部クリア可 — {areaRows(area.id).map((r) => r.content.name).join("・")}</span>
                  </button>
                {/if}
                <button type="button" class="area-toggle" onclick={() => toggleArea(area.id)}>
                  {open ? "▴" : `${okCount}/${areaRows(area.id).length} ▾`}
                </button>
              </div>
              {#if open}
                <div class="rows">
                  {#each areaRows(area.id) as r (r.content.id)}
                    {@const st = rowState(r)}
                    {@const note = noteOf(r)}
                    {@const sel = selectedRow?.content.id === r.content.id}
                    <div
                      class="row"
                      class:sel
                      role="button"
                      tabindex="0"
                      onclick={() => (selectedContentId = r.content.id)}
                      onkeydown={(e) => e.key === "Enter" && (selectedContentId = r.content.id)}
                    >
                      {#if r.content.id === frontierId}
                        <div class="frontier">次はここ</div>
                      {/if}
                      <div class="row-main">
                        <button
                          type="button"
                          class="pin"
                          class:on={pinned(r.content.id)}
                          title={pinned(r.content.id) ? "お気に入りから外す" : "お気に入りに追加"}
                          onclick={(e) => togglePin(e, r.content.id)}
                        >★</button>
                        <span class="thumb"></span>
                        <span class="name">{r.content.name}</span>
                        <span class="dmg num">{r.ev?.damage ? fmtInt(r.ev.damage.per_hit_max) : "—"}</span>
                      </div>
                      <div class="row-bar">
                        <div class="meter"><div class="fill" style="width: {pctOf(r)}; background: {BAR_BG[st]};"></div></div>
                        <span class="need num dim">目安 {fmtInt(r.content.need_per_hit)}</span>
                        {#if ratioOf(r) >= 1.15}
                          <span class="over num">×{ratioOf(r).toFixed(1)}</span>
                        {/if}
                        <span class="badge" style="background: {BADGE_BG[st]}; border-color: {BADGE_BD[st]}; color: {BADGE_FG[st]};">{BADGE[st]}</span>
                      </div>
                      <div class="row-note">
                        <span class="entry-dot" style="background: {r.content.requirements.length === 0 ? '#C1D3E6' : r.ev?.entry_ok ? '#6FA98A' : '#B0574A'};"></span>
                        <span class="note-text" class:unmet={note.unmet}>{note.text}</span>
                        {#if r.content.team_note}
                          <span class="team" title="チーム条件: {r.content.team_note}">チーム</span>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        </div>
        <p class="foot dim">目安ダメージ・入場条件は仮値です(wiki 狩り場情報の取込後に置き換わります)。</p>
      {/if}
    </div>
  </section>

  <Splitter
    bind:value={layoutWidths.value.right}
    min={240}
    defaultValue={DEFAULT_RIGHT_WIDTH}
    controls="next"
    label="一覧と選択中の境界"
  />

  <section class="right">
    <div class="head-bar">
      <span class="title">選択中</span>
      <span class="note">{selectedRow?.areaName ?? ""}</span>
    </div>
    <div class="scroll pad">
      {#if character && selectedRow}
        {@const r = selectedRow}
        {@const ok = !!r.ev?.reaches_need}
        <div class="sel-card">
          <div class="sel-name">{r.content.name}</div>
          <div class="sel-dmg">
            <span class="num huge">{r.ev?.damage ? fmtInt(r.ev.damage.per_hit_max) : "—"}</span>
            <span class="dim">1発(最大)</span>
          </div>
          <div class="sel-need num dim">目安 {fmtInt(r.content.need_per_hit)}</div>
          {#if r.ev?.damage}
            <div class="sel-skill dim">
              スキル: {skillNames[r.ev.damage.skill_id] ?? r.ev.damage.skill_id}(最大ダメージのスキルで判定)
            </div>
          {/if}
          {#if r.ev?.damage}
            <div class="sel-note" class:ok>
              {ok
                ? "火力は目安を超えています(参考値)。"
                : `目安まで あと ${Math.max(1, Math.round((1 - ratioOf(r)) * 100))}%。`}
            </div>
          {:else}
            <div class="sel-note">スキル未収録のため火力を判定できません。</div>
          {/if}
          <button type="button" class="try" onclick={tryInCalc}>計算シートで試す</button>
          <div class="sel-entry" class:ng={r.ev ? !r.ev.entry_ok && r.content.requirements.length > 0 : false}>
            {r.content.requirements.length === 0
              ? "入場条件はありません。"
              : r.ev?.entry_ok
                ? "入場条件をすべて満たしています。"
                : `入場条件が ${r.ev?.checks.filter((c) => !c.ok).length ?? 0} 項目 未達`}
          </div>
          {#if r.content.team_note}
            <div class="sel-team">チーム条件: {r.content.team_note}</div>
          {/if}
          {#if r.ev && r.ev.checks.length > 0}
            <div class="reqs">
              {#each r.ev.checks as c (c.label)}
                <div class="req" class:ng={!c.ok}>
                  <span class="req-label">{c.label}</span>
                  <span class="num dim">{fmtInt(c.current)} / {fmtInt(c.required)}</span>
                  <span class="req-tag">{c.ok ? "OK" : `あと ${fmtInt(c.required - c.current)}`}</span>
                </div>
              {/each}
            </div>
          {/if}
        </div>

        {#if favRows.length > 0}
          <div class="card">
            <div class="card-head">
              <span class="card-title">お気に入り</span>
              {#if favDone > 0}<span class="dim small">クリア済み {favDone} 件は非表示</span>{/if}
            </div>
            <div class="fav-list">
              {#each favRows as f (f.content.id)}
                {@const act = goal?.content.id === f.content.id}
                <button type="button" class="fav" class:act onclick={() => (selectedContentId = f.content.id)}>
                  <span class="mark" class:act>{act ? "★" : "☆"}</span>
                  <span class="fav-name">{f.content.name}</span>
                  <span class="num muted">{f.ev?.damage ? fmtInt(f.ev.damage.per_hit_max) : "—"}</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}

        {#if goal && advice.length > 0}
          <div class="card advice-card">
            <div class="card-head">
              <span class="card-title">次に変えるなら</span>
              <span class="dim small">目標: {goal.content.name}</span>
            </div>
            <div class="advice-list">
              {#each advice as a (a.candidate.id)}
                {@const reach = a.perHit >= goal.content.need_per_hit}
                <button type="button" class="advice" class:reach onclick={() => applyAdvice(a)}>
                  <span class="adv-row">
                    <span class="adv-label">{a.candidate.label}</span>
                    <span class="num adv-delta" class:up={a.deltaPct > 0}>
                      {a.deltaPct === 0 ? "±0%" : `${a.deltaPct > 0 ? "+" : ""}${a.deltaPct}%`}
                    </span>
                  </span>
                  <span class="adv-row sub">
                    <span class="num dim">{fmtInt(a.perHit)} / 目安 {fmtInt(goal.content.need_per_hit)}</span>
                    <span
                      class="cost"
                      style="background: {COST_COLORS[a.candidate.cost][0]}; border-color: {COST_COLORS[a.candidate.cost][1]}; color: {COST_COLORS[a.candidate.cost][2]};"
                    >{a.candidate.cost}</span>
                  </span>
                </button>
              {/each}
            </div>
            <p class="advice-foot dim">押すと計算シートに移動して、その変更を当てた状態から試せます。</p>
          </div>
        {/if}
      {:else}
        <p class="empty dim">キャラを選択してください。</p>
      {/if}
    </div>
  </section>
</div>

<style>
  .layout { flex: 1; min-height: 0; display: grid; }
  section { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  section.mid { background: var(--bg-mid); }
  section.right { background: var(--bg-rail); border-left: 1px solid var(--border-strong); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 13px 16px 18px; }
  .scroll.pad { padding: 12px; }
  .empty { font-size: 12px; }

  .summary {
    display: flex; align-items: center; gap: 10px; padding: 12px 15px; border-radius: 13px;
    background: linear-gradient(180deg, #fff, #E8F1FB 92%);
    border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgba(30, 44, 74, 0.1);
  }
  .summary .cap { font-size: 10px; font-weight: 700; letter-spacing: 0.12em; color: var(--fg-muted); white-space: nowrap; }
  .summary .big-wrap { display: flex; align-items: baseline; gap: 5px; }
  .summary .big { font-size: 27px; line-height: 1; font-weight: 700; color: #16223A; text-shadow: 0 1px 0 #fff; }
  .summary .of { font-size: 12px; color: #7E8EA6; white-space: nowrap; }
  .entry-pill {
    flex-shrink: 0; margin-left: auto; display: flex; align-items: baseline; gap: 6px;
    padding: 4px 11px; border-radius: 999px;
    background: rgba(255, 255, 255, 0.75); border: 1px solid var(--border-soft);
    font-size: 9.5px; white-space: nowrap;
  }
  .entry-pill .strong { font-size: 12px; font-weight: 700; color: #2B4FA8; }

  .areas { margin-top: 12px; display: flex; flex-direction: column; gap: 14px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: #26334A; text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: 2px; background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }
  .collapsed-note { flex: 1; min-width: 0; display: flex; align-items: center; gap: 7px; font-size: 10.5px; text-align: left; overflow: hidden; }
  .collapsed-note .dim { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ok-dot { width: 5px; height: 5px; flex-shrink: 0; border-radius: 50%; background: #6FA98A; }
  .area-toggle {
    flex-shrink: 0; padding: 2px 9px; border-radius: 999px;
    background: #fff; border: 1px solid var(--border-soft);
    font-size: 9px; font-weight: 700; color: var(--accent); white-space: nowrap;
  }

  .rows { padding-top: 7px; display: flex; flex-direction: column; gap: 6px; }
  .row {
    padding: 9px 12px; border-radius: 11px; cursor: pointer;
    background: #fff; border: 1px solid var(--border-soft);
  }
  .row.sel { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); box-shadow: 0 0 0 3px rgba(66, 109, 214, 0.18); }
  .frontier {
    display: inline-flex; align-items: center; margin-bottom: 6px; padding: 2px 9px; border-radius: 999px;
    background: linear-gradient(180deg, #CCF7FF, #90D7FF); border: 1px solid #687287;
    font-size: 9.5px; font-weight: 700; color: #123047;
  }
  .row-main { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .pin {
    width: 20px; height: 20px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    border-radius: 7px; border: 1px solid var(--border-soft); font-size: 10px; color: var(--fg-dim);
  }
  .pin.on { background: #DCEBFF; border-color: var(--accent); color: #2B4FA8; }
  .thumb {
    width: 24px; height: 24px; flex-shrink: 0; border-radius: 8px;
    background: repeating-linear-gradient(135deg, #F3E7E4 0 4px, #E6D3CD 4px 8px);
    border: 1px solid #A98B86; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.7);
  }
  .row-main .name { flex: 1; min-width: 0; font-size: 12.5px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row.sel .row-main .name { font-weight: 700; }
  .row-main .dmg { flex-shrink: 0; font-size: 15px; font-weight: 700; white-space: nowrap; }

  .row-bar { margin-top: 6px; display: flex; align-items: center; gap: 9px; }
  .row-bar .meter { flex: 1 1 auto; min-width: 40px; }
  .row-bar .need { flex-shrink: 0; font-size: 10px; white-space: nowrap; }
  .row-bar .over { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: #2B4FA8; }
  .row-bar .badge { margin-left: auto; }

  .row-note { margin-top: 5px; display: flex; align-items: center; gap: 7px; min-width: 0; }
  .entry-dot { width: 5px; height: 5px; flex-shrink: 0; border-radius: 50%; }
  .note-text { flex: 1; min-width: 0; font-size: 10.5px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .note-text.unmet { color: var(--danger); font-weight: 700; }
  .team {
    flex-shrink: 0; padding: 1px 6px; border-radius: 999px;
    background: #EFEEF8; border: 1px solid #6D6AA8;
    font-size: 8.5px; font-weight: 700; color: #4A4780;
  }

  .foot { margin: 14px 0 0; font-size: 10px; line-height: 1.7; }

  /* 右カラム */
  .scroll.pad { display: flex; flex-direction: column; gap: 11px; }
  .sel-card {
    padding: 14px; border-radius: 12px;
    background: linear-gradient(180deg, #FBFAFE, #F1F0FA);
    border: 1px solid #6D6AA8; box-shadow: inset 0 1px 0 #fff;
  }
  .sel-name { font-size: 11px; font-weight: 700; letter-spacing: 0.04em; color: #4A4780; }
  .sel-dmg { margin-top: 2px; display: flex; align-items: baseline; gap: 7px; }
  .huge { font-size: 34px; line-height: 1.05; font-weight: 700; }
  .sel-need { margin-top: 4px; font-size: 10.5px; }
  .sel-skill { margin-top: 4px; font-size: 9.5px; line-height: 1.5; }
  .sel-note {
    margin-top: 9px; padding: 7px 10px; border-radius: 9px;
    background: #FDF3DE; border: 1px solid #E3CB93;
    font-size: 10.5px; font-weight: 500; line-height: 1.6; color: #7A6420;
  }
  .sel-note.ok { background: #DFF3E6; border-color: #6FA98A; color: var(--good); }
  .try {
    margin-top: 8px; width: 100%; text-align: center; padding: 9px; border-radius: 9px;
    background: linear-gradient(180deg, #6D6AA8, #565394); border: 1px solid #3C3A6B;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3);
    font-size: 11px; font-weight: 700; color: #fff;
  }
  .sel-entry {
    margin-top: 7px; padding: 7px 10px; border-radius: 9px;
    background: #F4F9FE; border: 1px solid var(--border-soft);
    font-size: 10.5px; font-weight: 500; line-height: 1.6; color: #3B4A63;
  }
  .sel-entry.ng { background: #F6E8E5; border-color: #B08480; color: var(--danger); }
  .sel-team {
    margin-top: 7px; padding: 7px 10px; border-radius: 9px;
    background: #EFEEF8; border: 1px solid #6D6AA8;
    font-size: 10.5px; font-weight: 500; line-height: 1.6; color: #4A4780;
  }
  .reqs { margin-top: 7px; display: flex; flex-direction: column; gap: 5px; }
  .req {
    display: flex; align-items: center; gap: 8px; padding: 6px 9px; border-radius: 8px;
    background: #F4F9FE; border: 1px solid var(--border-soft);
  }
  .req.ng { background: #F6E8E5; border-color: #B08480; }
  .req-label { min-width: 0; flex: 1; font-size: 10.5px; font-weight: 500; color: #3B4A63; white-space: nowrap; }
  .req.ng .req-label { color: var(--danger); }
  .req .num { font-size: 10px; white-space: nowrap; }
  .req-tag { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: #3B4A63; white-space: nowrap; }
  .req.ng .req-tag { color: var(--danger); }

  .card-head { display: flex; align-items: center; gap: 8px; }
  .small { font-size: 9.5px; margin-left: auto; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fav-list { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .fav {
    display: flex; align-items: center; gap: 7px; padding: 8px 10px; border-radius: 10px;
    background: #fff; border: 1px solid var(--border-soft); text-align: left;
  }
  .fav.act { background: #F1F6FF; border-color: var(--accent); }
  .fav .mark { flex-shrink: 0; font-size: 10px; color: var(--fg-dim); }
  .fav .mark.act { color: #2B4FA8; }
  .fav-name { min-width: 0; flex: 1; font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fav.act .fav-name { font-weight: 700; }

  .advice-card { border-color: var(--accent); }
  .advice-list { margin-top: 9px; display: flex; flex-direction: column; gap: 7px; }
  .advice {
    display: flex; flex-direction: column; gap: 3px; padding: 9px 11px; border-radius: 10px; text-align: left;
    background: #F6FBFF; border: 1px solid var(--border-soft); box-shadow: inset 0 1px 0 #fff;
  }
  .advice.reach { background: linear-gradient(180deg, #F3FBF6, #E4F4EB); border-color: #6FA98A; }
  .advice:hover { border-color: var(--accent); }
  .adv-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .adv-row.sub { margin-top: 0; }
  .adv-label { min-width: 0; flex: 1; font-size: 11.5px; font-weight: 500; }
  .adv-delta { flex-shrink: 0; font-size: 11.5px; font-weight: 700; color: var(--fg-dim); }
  .adv-delta.up { color: var(--good); }
  .adv-row.sub .num { font-size: 10px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .cost { flex-shrink: 0; margin-left: auto; padding: 1px 8px; border-radius: 999px; border: 1px solid; font-size: 9px; font-weight: 700; white-space: nowrap; }
  .advice-foot { margin: 8px 0 0; font-size: 9.5px; line-height: 1.6; }
</style>
