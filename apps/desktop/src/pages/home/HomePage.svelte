<script lang="ts">
  // ホーム: 選択中キャラが「どこにどのくらい通るか」の到達一覧(v4 デザイン)。
  // 判定はすべて Rust 側(evaluate_contents)。この画面は表示と選択のみ。
  import { errorMessage, listSkills, previewDamage } from "../../api/commands";
  import type { Content, ContentEvaluation, NewCharacter } from "../../api/types";
  import { candidatesFor, COST_COLORS, type Candidate } from "../../candidates";
  import { fmtInt } from "../../format";
  import {
    app, evaluationFor, flatContents, payloadOf, refreshEvaluation, selectedCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Icon from "../../ui/Icon.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import { bump } from "../../ui/motion.svelte";
  import { badgeStyle, STATE, type Badge } from "../../ui/states";

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

  // 行の状態: 0余裕 1通る 2ぎりぎり 3届かない 4条件・火力とも未達 5条件だけ未達 6スキル未収録 7判定中
  //           8 入場OK(火力データなし) 9 入場条件が未達(火力データなし)
  // 「判定未着(!r.ev)」と「本当にスキル未収録(!r.ev.damage)」を混同しない(PR レビュー指摘)。
  // 敵データが無いコンテンツ(need_per_hit === null)は入場条件だけで状態を決める。
  function rowState(r: Row): number {
    if (!r.ev) return 6;
    if (r.content.need_per_hit === null) return r.ev.entry_ok ? 7 : 8;
    if (!r.ev.damage) return 6;
    const ratio = r.ev.damage.per_hit_max / r.content.need_per_hit;
    if (!r.ev.entry_ok) return r.ev.reaches_need ? 5 : 4;
    return ratio >= 1.3 ? 0 : ratio >= 1 ? 1 : ratio >= 0.8 ? 2 : 3;
  }

  /**
   * 収録度 — この敵についてどこまで分かっているか(design-system §14 決定 5)。
   *
   * 到達判定(バッジ)とは別の軸なので、同じ場所に混ぜない。行ごとに歯抜けを見せると
   * 30 行の一覧で破線が増えて画面が壊れて見えるので、**行頭に 1 つだけ**宣言する。
   * 完全に分かっている行は `null` — 全行に並ぶとバッジが装飾になる。
   */
  function coverage(r: Row): string | null {
    if (!r.ev) return "判定中";
    if (r.content.enemy_id === null) return "敵データなし";
    if (!r.ev.damage) return "スキル未収録";
    return null;
  }
  // 言葉はこの画面のもの、色は 6 系統から選ぶ(design-system §03)
  const BADGE: Badge[] = [
    { label: "余裕", state: "goal" },
    { label: "通る", state: "met" },
    { label: "ぎりぎり", state: "edge" },
    { label: "届かない", state: "short" },
    { label: "条件・火力とも未達", state: "unknown" },
    { label: "条件だけ未達", state: "temp" },
    // 火力の判定ができない行。理由は行頭の収録度バッジが言うので、ここでは繰り返さない
    { label: "火力は判定できません", state: "unknown" },
    { label: "入場OK", state: "met" },
    { label: "条件未達", state: "temp" },
  ];

  // 火力バーの比率。敵データが無いコンテンツは入場条件の充足度(満たした項目の割合)を出す。
  const ratioOf = (r: Row) => {
    if (r.content.need_per_hit === null) {
      if (!r.ev || r.ev.checks.length === 0) return r.ev?.entry_ok ? 1 : 0;
      return r.ev.checks.filter((c) => c.ok).length / r.ev.checks.length;
    }
    return r.ev?.damage ? r.ev.damage.per_hit_max / r.content.need_per_hit : 0;
  };
  const pctOf = (r: Row) => `${Math.min(100, ratioOf(r) * 100).toFixed(1)}%`;

  /** 未達条件の説明(「エタの意志 Lv あと 5」)。装備条件は比較先がスキル依存なので label をそのまま使う。 */
  const unmetText = (ev: ContentEvaluation) =>
    ev.checks.filter((c) => !c.ok).map((c) => `${c.label} あと ${fmtInt(c.required - c.current)}`).join(" ・ ");

  function noteOf(r: Row): { text: string; unmet: boolean } {
    if (!r.ev) return { text: "判定中…", unmet: false };
    // 敵データなし: 入場条件だけを説明する(火力の話をしない)
    if (r.content.need_per_hit === null) {
      if (r.ev.entry_ok) {
        const met = r.ev.checks.map((c) => `${c.label} ${fmtInt(c.required)}`).join(" / ");
        return { text: met ? `入場条件OK(${met})` : "入場条件なし", unmet: false };
      }
      return { text: `入場まで: ${unmetText(r.ev)}`, unmet: true };
    }
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
    return { text: `入場まで: ${unmetText(r.ev)}`, unmet: true };
  }

  // 選択とフロンティア(最初の未クリア)
  let selectedContentId = $state<string | null>(null);
  const frontierId = $derived(rows.find((r) => r.ev && !r.ev.clear)?.content.id ?? null);
  const selectedRow = $derived(
    rows.find((r) => r.content.id === selectedContentId) ?? rows.find((r) => r.content.id === frontierId) ?? rows[0] ?? null,
  );

  // --- 段数違いの系列(レリックの聖域 10〜19段)は 1 行 + 難易度ステッパーに畳む ---
  // 10 行並ぶと一覧のノイズになるだけで、実際に見たいのは「いまどの段まで行けるか」。
  let seriesStep = $state<Record<string, number>>({});
  const seriesRowsOf = (seriesId: string) =>
    rows
      .filter((r) => r.content.series?.id === seriesId)
      .sort((a, b) => (a.content.series?.step ?? 0) - (b.content.series?.step ?? 0));
  function currentSeriesRow(seriesId: string): Row | null {
    const list = seriesRowsOf(seriesId);
    const step = seriesStep[seriesId];
    return list.find((r) => r.content.series?.step === step) ?? list[0] ?? null;
  }
  function stepSeries(e: MouseEvent, seriesId: string, dir: number) {
    e.stopPropagation();
    const list = seriesRowsOf(seriesId);
    const current = currentSeriesRow(seriesId);
    const i = list.findIndex((r) => r.content.id === current?.content.id);
    const next = list[Math.min(list.length - 1, Math.max(0, i + dir))];
    if (next?.content.series) {
      seriesStep[seriesId] = next.content.series.step;
      selectedContentId = next.content.id;
    }
  }

  /** 一覧に出す行。系列は選択中の段だけを代表 1 行として出す */
  function areaDisplayRows(areaId: string): Row[] {
    const seen = new Set<string>();
    const out: Row[] = [];
    for (const r of areaRows(areaId)) {
      const series = r.content.series;
      if (!series) {
        out.push(r);
        continue;
      }
      if (seen.has(series.id)) continue;
      seen.add(series.id);
      const current = currentSeriesRow(series.id);
      if (current) out.push(current);
    }
    return out;
  }

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
    // 敵データが無いコンテンツ(enemy_id なし)は火力を比較できないので候補を出さない
    if (!c || !g?.ev?.damage || !g.content.enemy_id) {
      advice = [];
      return;
    }
    const skillId = g.ev.damage.skill_id;
    const contentId = g.content.id;
    const baseDamage = g.ev.damage.per_hit_max;
    const seq = ++adviceSeq;
    adviceHandle = setTimeout(async () => {
      try {
        const list = candidatesFor(payloadOf(c), app.equipmentCatalog);
        // 1 候補の失敗(装備検証エラー等)で他候補まで消さない(独立レビュー指摘)
        const settled = await Promise.allSettled(
          list.map(async (candidate) => {
            const p = payloadOf(c);
            candidate.apply(p);
            const r = await previewDamage(p, skillId, contentId, 0);
            return { candidate, perHit: r.per_hit.max, deltaPct: Math.round((r.per_hit.max / baseDamage - 1) * 100) };
          }),
        );
        const results = settled.flatMap((s) => (s.status === "fulfilled" ? [s.value] : []));
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
    // 既存の試し変更があればその上に候補を重ねる(無確認で破棄しない。PR レビュー指摘)
    const p = JSON.parse(JSON.stringify(app.sim ?? payloadOf(character))) as NewCharacter;
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
      <span class="note">目安＝実用的に周回できる1発量(コミュニティ知識)</span>
    </div>
    <div class="scroll">
      {#if !character}
        <p class="empty dim">キャラを登録すると、ここに到達一覧が出ます。左下の「＋ キャラを登録」からどうぞ。</p>
      {:else}
        {#if !app.evaluations[character.id]}
          <div class="retry-row">
            <span class="dim">到達判定を取得できていません。</span>
            <button type="button" class="btn" onclick={() => character && refreshEvaluation(character)}>再判定</button>
          </div>
        {/if}
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
            {@const shown = areaDisplayRows(area.id)}
            {@const okCount = shown.filter((r) => r.ev?.clear).length}
            <div class="area">
              <div class="area-head">
                <span class="area-name">{area.name}</span>
                {#if open}
                  <span class="area-rule"></span>
                {:else}
                  <button type="button" class="collapsed-note" onclick={() => toggleArea(area.id)}>
                    <span class="ok-dot"></span>
                    <!-- 系列は代表 1 行の名前だけ出す(10 段ぶん並べると畳んだ意味がない) -->
                    <span class="dim">全部クリア可 — {shown.map((r) => r.content.series?.name ?? r.content.name).join("・")}</span>
                  </button>
                {/if}
                <!-- 開閉で幅が変わると、押したボタン自身が動く(§09 規則 1・4)。件数は常に出す -->
                <button type="button" class="area-toggle" onclick={() => toggleArea(area.id)}>
                  <span class="num">{okCount}/{shown.length}</span>
                  <span class="chev">{open ? "▴" : "▾"}</span>
                </button>
              </div>
              {#if open}
                <div class="rows open-in">
                  {#each shown as r (r.content.series?.id ?? r.content.id)}
                    {@const st = rowState(r)}
                    {@const cov = coverage(r)}
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
                        <Icon kind="mob" id={r.content.enemy_id} size={28} label={r.content.name} />
                        <!-- 収録度は行頭に 1 つだけ(§14 決定 5)。完全な行には出さない -->
                        {#if cov !== null}<span class="coverage">{cov}</span>{/if}
                        {#if r.content.series}
                          {@const series = r.content.series}
                          {@const list = seriesRowsOf(series.id)}
                          {@const maxStep = list[list.length - 1]?.content.series?.step ?? series.step}
                          <span class="name">{series.name}</span>
                          <span class="stepper">
                            <button
                              type="button" class="st" aria-label="難易度を下げる"
                              disabled={series.step <= (list[0]?.content.series?.step ?? series.step)}
                              onclick={(e) => stepSeries(e, series.id, -1)}
                            >◀</button>
                            <span class="st-label num">難易度 {series.step} / {maxStep}</span>
                            <button
                              type="button" class="st" aria-label="難易度を上げる"
                              disabled={series.step >= maxStep}
                              onclick={(e) => stepSeries(e, series.id, 1)}
                            >▶</button>
                          </span>
                        {:else}
                          <span class="name">{r.content.name}</span>
                        {/if}
                        <span class="dmg num" use:bump={() => r.ev?.damage?.per_hit_max ?? null}>{r.ev?.damage ? fmtInt(r.ev.damage.per_hit_max) : "—"}</span>
                      </div>
                      <div class="row-bar">
                        <div class="meter"><div class="fill" style="width: {pctOf(r)}; background: {STATE[BADGE[st].state].bar};"></div></div>
                        {#if r.content.need_per_hit === null}
                          <span class="need num dim">入場条件のみ</span>
                        {:else}
                          <span class="need num dim">目安 {fmtInt(r.content.need_per_hit)}</span>
                          {#if ratioOf(r) >= 1.15}
                            <span class="over num">×{ratioOf(r).toFixed(1)}</span>
                          {/if}
                        {/if}
                        {#key st}<span class="badge badge-in" style={badgeStyle(BADGE[st])}>{BADGE[st].label}</span>{/key}
                      </div>
                      <div class="row-note">
                        <span class="entry-dot" style="background: {r.content.requirements.length === 0 ? STATE.unknown.bd : r.ev?.entry_ok ? STATE.met.bd : STATE.short.bd};"></span>
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
        <p class="foot dim">
          入場条件は swiki「コンテンツ入場条件」由来。装備条件は使うスキルの依存(突き/斬り/魔攻/魔防/複合)で比較先が変わります。
          目安ダメージは wiki に無い値で、コミュニティ知識・実測が出典です(実測で更新)。
        </p>
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
          {#if r.content.need_per_hit === null}
            <!-- 敵データが無いコンテンツ: 火力を出さず入場条件だけを示す -->
            <div class="sel-entry-only dim">敵データ未収録のため、入場条件のみ判定しています。</div>
          {:else}
            <div class="sel-dmg">
              <span class="num huge" use:bump={() => r.ev?.damage?.per_hit_max ?? null}>{r.ev?.damage ? fmtInt(r.ev.damage.per_hit_max) : "—"}</span>
              <span class="dim">1発(最大)</span>
            </div>
            <div class="sel-need num dim">目安 {fmtInt(r.content.need_per_hit)}</div>
            {#if r.ev?.damage}
              <div class="sel-skill dim">
                スキル: {skillNames[r.ev.damage.skill_id] ?? r.ev.damage.skill_id}(最大ダメージのスキルで判定)
              </div>
              <div class="sel-note" class:ok>
                {ok
                  ? "火力は目安を超えています(参考値)。"
                  : `目安まで あと ${Math.max(1, Math.round((1 - ratioOf(r)) * 100))}%。`}
              </div>
            {:else}
              <div class="sel-note">スキル未収録のため火力を判定できません。</div>
            {/if}
            <button type="button" class="try" onclick={tryInCalc}>計算シートで試す</button>
          {/if}
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
          {#if r.content.entry_note}
            <!-- ルーン Lv・共通スキル・コア等、キャラモデルに値が無く判定できない条件 -->
            <div class="sel-entry-note">{r.content.entry_note}</div>
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
                {@const need = goal.content.need_per_hit ?? 0}
                {@const reach = a.perHit >= need}
                <button type="button" class="advice" class:reach onclick={() => applyAdvice(a)}>
                  <span class="adv-row">
                    <span class="adv-label">{a.candidate.label}</span>
                    <span class="num adv-delta" class:up={a.deltaPct > 0}>
                      {a.deltaPct === 0 ? "±0%" : `${a.deltaPct > 0 ? "+" : ""}${a.deltaPct}%`}
                    </span>
                  </span>
                  <span class="adv-row sub">
                    <span class="num dim">{fmtInt(a.perHit)} / 目安 {fmtInt(need)}</span>
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

  .retry-row { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; font-size: 11px; }

  .summary {
    display: flex; align-items: center; gap: 10px; padding: 12px 15px; border-radius: var(--r-window);
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
    padding: 4px 11px; border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.75); border: 1px solid var(--border-soft);
    font-size: 9.5px; white-space: nowrap;
  }
  .entry-pill .strong { font-size: 12px; font-weight: 700; color: var(--accent-hover); }

  .areas { margin-top: 12px; display: flex; flex-direction: column; gap: 14px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: var(--r-inset); background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }
  .collapsed-note { flex: 1; min-width: 0; display: flex; align-items: center; gap: 7px; font-size: var(--t-label); text-align: left; overflow: hidden; }
  .collapsed-note .dim { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .stepper {
    flex-shrink: 0; display: inline-flex; align-items: center; gap: 5px;
    padding: 1px 4px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border-soft);
  }
  .st {
    width: 16px; height: 16px; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-pill); font-size: 8.5px; color: var(--accent);
  }
  .st:hover:not(:disabled) { background: var(--bg-active); }
  .st-label { font-size: 9px; color: var(--fg-muted); white-space: nowrap; }

  .ok-dot { width: 5px; height: 5px; flex-shrink: 0; border-radius: 50%; background: var(--good-soft); }
  .area-toggle {
    flex-shrink: 0; display: inline-flex; align-items: center; gap: 5px;
    padding: 2px 9px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    font-size: 9px; font-weight: 700; color: var(--accent); white-space: nowrap;
  }

  .rows { padding-top: 7px; display: flex; flex-direction: column; gap: 6px; }
  .row {
    padding: 9px 12px; border-radius: var(--r-window); cursor: pointer;
    background: var(--bg-field); border: 1px solid var(--border-soft);
  }
  .row.sel { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); box-shadow: 0 0 0 3px rgba(66, 109, 214, 0.18); }
  .frontier {
    display: inline-flex; align-items: center; margin-bottom: 6px; padding: 2px 9px; border-radius: var(--r-pill);
    background: linear-gradient(180deg, #CCF7FF, #90D7FF); border: 1px solid #687287;
    font-size: 9.5px; font-weight: 700; color: #123047;
  }
  .row-main { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .pin {
    width: 20px; height: 20px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); border: 1px solid var(--border-soft); font-size: 10px; color: var(--fg-dim);
  }
  .pin.on { background: var(--bg-active); border-color: var(--accent); color: var(--accent-hover); }

  .row-main .name { flex: 1; min-width: 0; font-size: 12.5px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row.sel .row-main .name { font-weight: 700; }
  .row-main .dmg { flex-shrink: 0; font-size: 15px; font-weight: 700; white-space: nowrap; }

  .row-bar { margin-top: 6px; display: flex; align-items: center; gap: 9px; }
  .row-bar .meter { flex: 1 1 auto; min-width: 40px; }
  .row-bar .need { flex-shrink: 0; font-size: 10px; white-space: nowrap; }
  .row-bar .over { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: var(--accent-hover); }
  .row-bar .badge { margin-left: auto; }

  .row-note { margin-top: 5px; display: flex; align-items: center; gap: 7px; min-width: 0; }
  /* 収録度(§14 決定 5)。破線 = 「まだ無い」の記号。行内の歯抜けはこの 1 つが引き受ける */
  .coverage {
    flex-shrink: 0; padding: 1px 7px; border-radius: var(--r-pill);
    border: 1px dashed var(--border); background: var(--bg-rail);
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted); white-space: nowrap;
  }
  .entry-dot { width: 5px; height: 5px; flex-shrink: 0; border-radius: 50%; }
  .note-text { flex: 1; min-width: 0; font-size: var(--t-label); color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .note-text.unmet { color: var(--danger); font-weight: 700; }
  .team {
    flex-shrink: 0; padding: 1px 6px; border-radius: var(--r-pill);
    background: var(--state-temp-bg); border: 1px solid var(--sim);
    font-size: 8.5px; font-weight: 700; color: var(--sim-fg);
  }

  .foot { margin: 14px 0 0; font-size: 10px; line-height: 1.7; }

  /* 右カラム */
  .scroll.pad { display: flex; flex-direction: column; gap: 11px; }
  .sel-card {
    padding: 14px; border-radius: var(--r-window);
    background: linear-gradient(180deg, var(--sim-bg), #F1F0FA);
    border: 1px solid var(--sim); box-shadow: inset 0 1px 0 #fff;
  }
  /* このカード群の親になる大見出し(規格の見出し段) */
  .sel-name { font-size: var(--t-heading); font-weight: var(--w-strong); letter-spacing: 0.02em; color: var(--fg); line-height: 1.25; }
  .sel-dmg { margin-top: 2px; display: flex; align-items: baseline; gap: 7px; }
  /* 右カラムの選択中は結果の数値の 2 段目(§05: 44 / 40 / 27) */
  .huge { font-size: 40px; line-height: 1.05; font-weight: 700; }
  .sel-need { margin-top: 4px; font-size: var(--t-label); }
  .sel-skill { margin-top: 4px; font-size: 9.5px; line-height: 1.5; }
  .sel-note {
    margin-top: 9px; padding: 7px 10px; border-radius: var(--r-panel);
    background: var(--state-edge-bg); border: 1px solid #E3CB93;
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--state-edge-fg);
  }
  .sel-note.ok { background: var(--state-met-bg); border-color: var(--good-soft); color: var(--good); }
  .try {
    margin-top: 8px; width: 100%; text-align: center; padding: 9px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, var(--sim), var(--sim-strong)); border: 1px solid #3C3A6B;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3);
    font-size: 11px; font-weight: 700; color: #fff;
  }
  .sel-entry {
    margin-top: 7px; padding: 7px 10px; border-radius: var(--r-panel);
    background: #F4F9FE; border: 1px solid var(--border-soft);
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--fg-sub);
  }
  .sel-entry.ng { background: var(--state-short-bg); border-color: var(--state-short-bd); color: var(--danger); }
  .sel-entry-only {
    margin-top: 7px; padding: 7px 10px; border-radius: var(--r-panel);
    background: #F7F8FB; border: 1px dashed var(--border-soft);
    font-size: var(--t-label); line-height: 1.6;
  }
  .sel-entry-note {
    margin-top: 7px; padding: 7px 10px; border-radius: var(--r-panel);
    background: #FDF9EE; border: 1px solid var(--gold);
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--state-edge-fg);
  }
  .sel-team {
    margin-top: 7px; padding: 7px 10px; border-radius: var(--r-panel);
    background: var(--state-temp-bg); border: 1px solid var(--sim);
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--sim-fg);
  }
  .reqs { margin-top: 7px; display: flex; flex-direction: column; gap: 5px; }
  .req {
    display: flex; align-items: center; gap: 8px; padding: 6px 9px; border-radius: var(--r-panel);
    background: #F4F9FE; border: 1px solid var(--border-soft);
  }
  .req.ng { background: var(--state-short-bg); border-color: var(--state-short-bd); }
  .req-label { min-width: 0; flex: 1; font-size: var(--t-label); font-weight: 500; color: var(--fg-sub); white-space: nowrap; }
  .req.ng .req-label { color: var(--danger); }
  .req .num { font-size: 10px; white-space: nowrap; }
  .req-tag { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: var(--fg-sub); white-space: nowrap; }
  .req.ng .req-tag { color: var(--danger); }

  .card-head { display: flex; align-items: center; gap: 8px; }
  .small { font-size: 9.5px; margin-left: auto; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fav-list { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .fav {
    display: flex; align-items: center; gap: 7px; padding: 8px 10px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .fav.act { background: #F1F6FF; border-color: var(--accent); }
  .fav .mark { flex-shrink: 0; font-size: 10px; color: var(--fg-dim); }
  .fav .mark.act { color: var(--accent-hover); }
  .fav-name { min-width: 0; flex: 1; font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fav.act .fav-name { font-weight: 700; }

  .advice-card { border-color: var(--accent); }
  .advice-list { margin-top: 9px; display: flex; flex-direction: column; gap: 7px; }
  .advice {
    display: flex; flex-direction: column; gap: 3px; padding: 9px 11px; border-radius: var(--r-panel); text-align: left;
    background: var(--bg-panel); border: 1px solid var(--border-soft); box-shadow: inset 0 1px 0 #fff;
  }
  .advice.reach { background: linear-gradient(180deg, #F3FBF6, #E4F4EB); border-color: var(--good-soft); }
  .advice:hover { border-color: var(--accent); }
  .adv-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .adv-row.sub { margin-top: 0; }
  .adv-label { min-width: 0; flex: 1; font-size: 11.5px; font-weight: 500; }
  .adv-delta { flex-shrink: 0; font-size: 11.5px; font-weight: 700; color: var(--fg-dim); }
  .adv-delta.up { color: var(--good); }
  .adv-row.sub .num { font-size: 10px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .cost { flex-shrink: 0; margin-left: auto; padding: 1px 8px; border-radius: var(--r-pill); border: 1px solid; font-size: 9px; font-weight: 700; white-space: nowrap; }
  .advice-foot { margin: 8px 0 0; font-size: 9.5px; line-height: 1.6; }
</style>
