<script lang="ts" module>
  // 「目安火力が変わった」影響カードは、キャラごとにセッション中 1 回だけ判定する
  // (起動中に何度もホームへ戻っても再表示しない)。モジュールスコープなので
  // タブ切替でこのコンポーネントが作り直されても保持される。
  const checkedSnapshotIds = new Set<number>();
</script>

<script lang="ts">
  // ホーム: 選択中キャラの「キャラの窓」ヒーロー(現況・次の目標・おすすめ強化)+
  // 今日の期限・影響 + 反映(部位タイル)+ 到達一覧(畳み)+ うごみ、のブリーフィング型 1 カラム。
  // 判定はすべて Rust 側(evaluate_contents / preview_effective_stats / preview_defense / preview_damage)。
  // この画面は表示と選択のみ。
  import {
    errorMessage, getDamageSnapshot, listSkills, previewDamage, previewDefense, previewEffectiveStats,
    setDamageSnapshot,
  } from "../../api/commands";
  import type {
    Content, ContentEvaluation, DefenseProfile, EquipmentPart, NewCharacter, PartSlot, StatPreview,
  } from "../../api/types";
  import { candidatesFor, COST_COLORS, COST_LABELS, tryCandidates, type Candidate } from "../../candidates";
  import { equipmentEnchantTotal, equipmentIconId, sumValues } from "../../equipment";
  import { FEED_ITEMS } from "../../feed";
  import { fmtInt } from "../../format";
  import { STAT_KINDS, STAT_LABELS } from "../../labels";
  import {
    app, buffSelectionFor, evaluationFor, flatContents, focusCharacterSource, gameCharacterName, payloadOf,
    refreshEvaluation, selectedCharacter, totalContents,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Icon from "../../ui/Icon.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  import { badgeStyle, REACH_BADGES, STATE, triadStyle, type Badge } from "../../ui/states";

  const character = $derived(selectedCharacter());
  const totalCount = $derived(totalContents());

  const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];
  const today = new Date();
  const todayLabel = `${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}(${WEEKDAYS[today.getDay()]})`;
  const fmtMonthDay = (iso: string) => {
    const d = new Date(iso);
    return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };
  const daysAgo = (iso: string) => Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000));

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
   * 一覧で破線が増えて画面が壊れて見えるので、**行頭に 1 つだけ**宣言する。
   * 完全に分かっている行は `null` — 全行に並ぶとバッジが装飾になる。
   */
  function coverage(r: Row): string | null {
    if (!r.ev) return "判定中";
    if (r.content.enemy_id === null) return "敵データなし";
    if (!r.ev.damage) return "スキル未収録";
    return null;
  }
  // 言葉はこの画面のもの、色は 6 系統から選ぶ(design-system §03)。先頭 6 件は共通(ui/states.ts)
  const BADGE: Badge[] = [
    ...REACH_BADGES,
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

  /** どこまでいける?一覧の行を押すと、その対象を選んだ状態で計算タブへ移る(押した場所は動かさず、遷移で詳細を賄う)。 */
  function openInCalc(contentId: string) {
    app.calcTargetId = contentId;
    app.tab = "calc";
  }

  // フロンティア(最初の未クリア)
  const frontierId = $derived(rows.find((r) => r.ev && !r.ev.clear)?.content.id ?? null);
  /** ヒーローの「次の目標」= 到達一覧で上から最初の届かないコンテンツ */
  const heroGoal = $derived(rows.find((r) => r.content.id === frontierId) ?? null);

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

  // エリアの開閉(既定: 全部畳む。fold 自体も既定で畳む)
  let openAreas = $state<Record<string, boolean>>({});
  function areaRows(areaId: string): Row[] {
    return rows.filter((r) => r.areaId === areaId);
  }
  function isAreaOpen(areaId: string): boolean {
    return openAreas[areaId] ?? false;
  }
  function toggleArea(areaId: string) {
    openAreas[areaId] = !isAreaOpen(areaId);
  }

  /** 未収録(収録度バッジが出る)コンテンツ数。fold の畳み要約に使う。 */
  const uncoveredCount = $derived(rows.filter((r) => coverage(r) !== null).length);

  const areas = $derived(app.areas);

  // ===== ヒーロー(キャラの窓): 選択中キャラの現況 =============================

  // 7 ステ・装備基礎値(previewEffectiveStats)と防御側(preview_defense)
  let heroStats = $state<StatPreview | null>(null);
  let heroDefense = $state<DefenseProfile | null>(null);
  const heroStatsLatest = latest({ debounce: 120 });
  $effect(() => {
    const c = character;
    if (!c) {
      heroStatsLatest.cancel();
      heroStats = null;
      heroDefense = null;
      return;
    }
    const p = payloadOf(c);
    const buffs = buffSelectionFor(c);
    heroStatsLatest.run(async (isCurrent) => {
      try {
        const [stats, defense] = await Promise.all([
          previewEffectiveStats(
            p.base_stats, p.stat_sources, p.equipment, p.common_skills, p.awakening, p.main_skill_id, buffs,
          ),
          previewDefense(p, buffs),
        ]);
        if (isCurrent()) {
          heroStats = stats;
          heroDefense = defense;
        }
      } catch (e) {
        if (isCurrent()) reportError(errorMessage(e));
      }
    });
    return () => heroStatsLatest.cancel();
  });

  // 装備値の内訳(基礎+エンチャ=合計)。基礎(称号・アビリティ込み)は preview、エンチャ分は
  // クライアント側の Σpart.enchant(equipment.ts。実際の集計元は Rust 側 Equipment::enhanced_totals)。
  const heroEnchant = $derived(character ? equipmentEnchantTotal(character.equipment) : null);
  const heroThrustTotal = $derived(
    heroStats && heroEnchant ? heroStats.equipment_base_total.thrust + heroEnchant.thrust : null,
  );
  const heroSlashTotal = $derived(
    heroStats && heroEnchant ? heroStats.equipment_base_total.slash + heroEnchant.slash : null,
  );

  // 命中P(次の目標のスキルで判定。BestSkillDamage には無いので previewDamage を別途叩く)と、
  // おすすめ強化(candidatesFor → tryCandidates → perHit 降順の上位 3 件。既存候補システムを再利用)
  interface HeroAdvice { candidate: Candidate; perHit: number; deltaPct: number }
  let heroAccuracy = $state<number | null>(null);
  let heroAdvice = $state<HeroAdvice[]>([]);
  const heroAdviceLatest = latest({ debounce: 150 });
  $effect(() => {
    const c = character;
    const g = heroGoal;
    // 依存を明示的に読む(保存データの変化でも再計算する)
    void app.evaluations[c?.id ?? -1];
    // 敵データが無いコンテンツ(enemy_id なし)は火力を比較できないので候補を出さない
    if (!c || !g?.ev?.damage || !g.content.enemy_id) {
      heroAdviceLatest.cancel();
      heroAccuracy = null;
      heroAdvice = [];
      return;
    }
    const skillId = g.ev.damage.skill_id;
    const contentId = g.content.id;
    const baseDamage = g.ev.damage.per_hit_max;
    const buffs = buffSelectionFor(c);
    heroAdviceLatest.run(async (isCurrent) => {
      try {
        const current = await previewDamage(payloadOf(c), skillId, contentId, 0, null, null, buffs);
        const candidates = candidatesFor(payloadOf(c), app.equipmentCatalog);
        const results = await tryCandidates(
          candidates,
          () => payloadOf(c),
          (p) => previewDamage(p, skillId, contentId, 0, null, null, buffs),
          baseDamage,
        );
        if (isCurrent()) {
          heroAccuracy = current.accuracy_point;
          heroAdvice = results.slice(0, 3);
        }
      } catch (e) {
        if (isCurrent()) reportError(errorMessage(e));
      }
    });
    return () => heroAdviceLatest.cancel();
  });

  function tryHeroGoalInCalc() {
    if (!heroGoal) return;
    app.calcTargetId = heroGoal.content.id;
    app.tab = "calc";
  }

  function applyHeroAdvice(a: HeroAdvice) {
    if (!character || !heroGoal) return;
    // 既存の試し変更があればその上に候補を重ねる(無確認で破棄しない。PR レビュー指摘)
    const p = JSON.parse(JSON.stringify(app.sim ?? payloadOf(character))) as NewCharacter;
    a.candidate.apply(p);
    app.sim = p;
    app.calcTargetId = heroGoal.content.id;
    app.tab = "calc";
  }

  // ===== 影響カード: 前回起動からの目安火力の変化(セッション中キャラごと初回のみ) ======
  interface ImpactCard {
    characterId: number;
    skillId: string;
    contentId: string;
    perHit: number;
    prevPerHit: number;
  }
  let impactCard = $state<ImpactCard | null>(null);
  $effect(() => {
    const c = character;
    const g = heroGoal;
    if (!c || !g?.ev?.damage || checkedSnapshotIds.has(c.id)) return;
    checkedSnapshotIds.add(c.id);
    const skillId = g.ev.damage.skill_id;
    const contentId = g.content.id;
    const perHit = g.ev.damage.per_hit_max;
    (async () => {
      try {
        const prev = await getDamageSnapshot(c.id);
        if (prev && prev.per_hit !== perHit) {
          impactCard = { characterId: c.id, skillId, contentId, perHit, prevPerHit: prev.per_hit };
        }
        await setDamageSnapshot(c.id, skillId, contentId, perHit);
      } catch (e) {
        reportError(errorMessage(e));
      }
    })();
  });

  // ===== 反映ゾーン: 部位タイル(ADR 004 の部位列)。押すとキャラタブの装備ペインの該当部位へ ======
  const REACH_TILES: { slot: PartSlot; label: string }[] = [
    { slot: "helm", label: "兜" },
    { slot: "armor", label: "鎧" },
    { slot: "weapon", label: "武器" },
    { slot: "shield", label: "盾" },
    { slot: "shield_plus", label: "カフス(盾+)" },
    { slot: "head", label: "頭" },
    { slot: "hand", label: "手" },
    { slot: "leg", label: "足" },
    { slot: "relic_pendant", label: "レリック左" },
    { slot: "relic_bracelet", label: "レリック右" },
  ];
  const isRelicTile = (slot: PartSlot) => slot === "relic_pendant" || slot === "relic_bracelet";
  function partOf(slot: PartSlot): EquipmentPart | null {
    if (!character) return null;
    const list = character.equipment.parts[slot];
    return list.registered.find((p) => p.id === list.selected_id) ?? null;
  }
  /** タイルの現在値要約。未装備は「未設定」(§03 操作待ち・金)、レリックは Lv n、
   *  強化Lvを持つ部位(武器・鎧)はその段、それ以外はエンチャント合計。 */
  function partValue(slot: PartSlot, part: EquipmentPart | null): { text: string; unset: boolean } {
    if (isRelicTile(slot)) {
      const level = part?.item_id?.match(/-plus(\d+)$/)?.[1];
      return level ? { text: `Lv${level}`, unset: false } : { text: "未設定", unset: true };
    }
    if (!part || (part.item_id === null && part.custom_name === null)) return { text: "未設定", unset: true };
    if (slot === "weapon" || slot === "armor") {
      return { text: part.enhance_level > 0 ? `+${part.enhance_level}` : "強化なし", unset: false };
    }
    const total = sumValues(part.enchant);
    return { text: total > 0 ? `+${total}` : "±0", unset: false };
  }
  /** シエナのオーラは装着中の登録数(部位を跨いだ合計)。8 部位すべてに発現できるので独立管理。 */
  const auraCount = $derived(
    character ? Object.values(character.equipment.siena).filter((l) => l.selected_id !== null).length : 0,
  );
</script>

<div class="home">
  <div class="head-bar">
    <span class="title">今日の TW</span>
    <span class="note">{todayLabel}{character ? ` ・ ${character.name} の現況` : ""}</span>
  </div>
  <div class="scroll">
    {#if !character}
      <p class="empty dim">キャラを登録すると、ここに今日の状況が出ます。左のレールの「＋ キャラを登録」からどうぞ。</p>
    {:else}
      <!-- ===== キャラの窓ヒーロー: 言葉(左)と数値(右)を空間で分離する ===== -->
      <div class="hero">
        <div class="hero-top">
          <div class="hero-id">
            <Icon kind="character" id={character.game_character_id} size={64} label={character.name} />
            <span class="hero-id-name">{character.name}</span>
            <span class="hero-id-class">
              {gameCharacterName(character.game_character_id)} / 覚醒{character.awakening.stage} ・ エタ意志
              <span class="num strong">Lv {character.awakening.eternal_level}</span>
            </span>
          </div>
          <div class="hero-panels">
            <div class="hero-panel">
              <span class="hero-panel-title">ステータス</span>
              {#each STAT_KINDS as k, i (k)}
                <span class="hero-row" class:first={i === 0}>
                  <span class="hero-row-label">{STAT_LABELS[k]}</span>
                  <span class="num hero-row-value" use:bump={() => heroStats?.stats[k] ?? null}>
                    {heroStats ? fmtInt(heroStats.stats[k]) : "—"}
                  </span>
                </span>
              {/each}
            </div>
            <div class="hero-panel">
              <span class="hero-panel-title">装備・命中</span>
              <span class="hero-row first">
                <span class="hero-row-label">突き</span>
                <span class="hero-row-value-wrap">
                  <span class="num hero-sub">
                    {heroStats && heroEnchant ? `${fmtInt(heroStats.equipment_base_total.thrust)} +${fmtInt(heroEnchant.thrust)}` : ""}
                  </span>
                  <span class="num hero-row-value" use:bump={() => heroThrustTotal}>
                    {heroThrustTotal !== null ? fmtInt(heroThrustTotal) : "—"}
                  </span>
                </span>
              </span>
              <span class="hero-row">
                <span class="hero-row-label">斬り</span>
                <span class="hero-row-value-wrap">
                  <span class="num hero-sub">
                    {heroStats && heroEnchant ? `${fmtInt(heroStats.equipment_base_total.slash)} +${fmtInt(heroEnchant.slash)}` : ""}
                  </span>
                  <span class="num hero-row-value" use:bump={() => heroSlashTotal}>
                    {heroSlashTotal !== null ? fmtInt(heroSlashTotal) : "—"}
                  </span>
                </span>
              </span>
              <span class="hero-row">
                <span class="hero-row-label">命中P</span>
                <span class="num hero-row-value" use:bump={() => heroAccuracy}>
                  {heroAccuracy !== null ? fmtInt(heroAccuracy) : "—"}
                </span>
              </span>
              <span class="hero-row">
                <span class="hero-row-label">回避P</span>
                <span class="num hero-row-value" use:bump={() => heroDefense?.evasion_point.physical ?? null}>
                  {heroDefense ? fmtInt(heroDefense.evasion_point.physical) : "—"}
                </span>
              </span>
            </div>
          </div>
        </div>

        <!-- 次の目標スポットライト(全幅 — 右列に入れるとメーターが潰れる) -->
        <div class="hero-goal">
          <span class="tag">次の目標</span>
          {#if !app.evaluations[character.id]}
            <span class="dim">到達判定を取得できていません。</span>
          {:else if !heroGoal}
            <span class="hero-goal-name">なし — 全 {fmtInt(totalCount)} コンテンツ クリア可</span>
          {:else}
            <span class="hero-goal-name">{heroGoal.content.series?.name ?? heroGoal.content.name}</span>
            {#if heroGoal.content.need_per_hit === null || !heroGoal.ev?.damage}
              <span class="hero-goal-note dim">{noteOf(heroGoal).text}</span>
            {:else}
              <span class="hero-div"></span>
              <Icon
                kind="skill" id={heroGoal.ev.damage.skill_id} size={28}
                label={skillNames[heroGoal.ev.damage.skill_id] ?? heroGoal.ev.damage.skill_id}
              />
              <span class="hero-goal-skill">{skillNames[heroGoal.ev.damage.skill_id] ?? heroGoal.ev.damage.skill_id}</span>
              <span class="meter hero-meter">
                <span class="fill" style="width: {pctOf(heroGoal)}; background: {STATE[BADGE[rowState(heroGoal)].state].bar};"></span>
              </span>
              <span class="hero-spot-wrap">
                <span class="num hero-spot" use:bump={() => heroGoal?.ev?.damage?.per_hit_max ?? null}>
                  {fmtInt(heroGoal.ev.damage.per_hit_max)}
                </span>
                <span class="num dim"> / {fmtInt(heroGoal.content.need_per_hit)}</span>
              </span>
              {#key rowState(heroGoal)}
                <span class="badge" style={badgeStyle(BADGE[rowState(heroGoal)])} use:flash={() => String(rowState(heroGoal))}>
                  {BADGE[rowState(heroGoal)].label}
                </span>
              {/key}
            {/if}
            <button type="button" class="cta" onclick={tryHeroGoalInCalc}>計算タブで詰める ›</button>
          {/if}
        </div>

        {#if heroAdvice.length > 0}
          <div class="hero-advice">
            <span class="hero-advice-title">おすすめ強化 — 届かせるなら</span>
            <div class="hero-advice-list">
              {#each heroAdvice as a, i (a.candidate.id)}
                {@const need = heroGoal?.content.need_per_hit ?? 0}
                {@const reach = a.perHit >= need}
                <button type="button" class="hero-advice-row" onclick={() => applyHeroAdvice(a)}>
                  <span class="rank num">{i + 1}</span>
                  <span class="cost" style={triadStyle(COST_COLORS[a.candidate.cost])}>{COST_LABELS[a.candidate.cost]}</span>
                  <span class="hero-advice-label">{a.candidate.label}</span>
                  <span class="hero-advice-nums">
                    <span class="num" use:bump={() => a.perHit}>{fmtInt(a.perHit)}</span>
                    <span class="num" use:bump={() => a.deltaPct}>{a.deltaPct > 0 ? " +" : " "}{a.deltaPct}%</span>
                  </span>
                  {#if reach}
                    <span class="badge" style={badgeStyle({ label: "届く見込み", state: "temp" })}>届く見込み</span>
                  {/if}
                  <span class="chev dim">›</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </div>

      <!-- ===== 期限・影響(今日のカード)。0 件の日は列ごと出さない ===== -->
      {#if impactCard && impactCard.characterId === character.id}
        {@const card = impactCard}
        <div class="section">
          <div class="area-head">
            <span class="area-name">期限・影響</span>
            <span class="area-rule"></span>
          </div>
          <div class="brief-card" use:flash={() => String(card.perHit)}>
            <span class="tag">影響</span>
            <Icon
              kind="skill" id={card.skillId} size={28}
              label={skillNames[card.skillId] ?? card.skillId}
            />
            <span class="brief-copy">
              <span class="brief-title">
                目安火力が <span class="num" use:bump={() => card.perHit}>{fmtInt(card.perHit)}</span> に{card.perHit >= card.prevPerHit ? "上がりました" : "下がりました"}
                <span class="num" style="color: {card.perHit >= card.prevPerHit ? 'var(--good)' : 'var(--danger)'}">
                  {card.perHit >= card.prevPerHit ? "+" : ""}{fmtInt(card.perHit - card.prevPerHit)}
                </span>
              </span>
              <span class="brief-why">前回 <span class="num">{fmtInt(card.prevPerHit)}</span></span>
            </span>
            <button type="button" class="cta" onclick={() => openInCalc(card.contentId)}>なぜこの数字? ›</button>
          </div>
        </div>
      {/if}

      <!-- ===== 反映: 部位タイル。押すとキャラタブの装備ペインの該当部位へ ===== -->
      <div class="section">
        <div class="area-head">
          <span class="area-name">反映</span>
          <span class="area-rule"></span>
          {#if character.updated_at}
            <span class="last-reflect dim">
              最終反映 <span class="num">{fmtMonthDay(character.updated_at)}</span>
              ({daysAgo(character.updated_at) === 0 ? "今日" : `${daysAgo(character.updated_at)} 日前`})
            </span>
          {/if}
        </div>
        <div class="parts-grid">
          {#each REACH_TILES as t (t.slot)}
            {@const part = partOf(t.slot)}
            {@const val = partValue(t.slot, part)}
            <button type="button" class="part-tile" onclick={() => focusCharacterSource("equipment", t.slot)}>
              <Icon kind="equipment" id={part ? equipmentIconId(part.item_id, app.equipmentCatalog) : null} size={20} label={t.label} />
              <span class="part-tile-name">{t.label}</span>
              {#if val.unset}
                <span class="badge" style={badgeStyle({ label: "未設定", state: "edge" })}>未設定</span>
              {:else}
                <span class="num part-tile-val" use:flash={() => val.text}>{val.text}</span>
              {/if}
              <span class="chev dim">›</span>
            </button>
          {/each}
          <button type="button" class="part-tile" onclick={() => focusCharacterSource("siena")}>
            <Icon kind="equipment" id={null} size={20} label="オーラ" />
            <span class="part-tile-name">オーラ</span>
            <span class="num part-tile-val" use:flash={() => String(auraCount)}>{auraCount} 部位</span>
            <span class="chev dim">›</span>
          </button>
        </div>
        <button type="button" class="cta tile-more" onclick={() => (app.tab = "chars")}>そのほかの設定(ペット・ルーン・バフ) ›</button>
      </div>

      <!-- ===== どこまでいける?: 畳み既定。エリア 4 行 → 押すと直下に一覧が展開(§09 規則 1) ===== -->
      <details class="fold reach-fold">
        <summary>
          <span class="area-name">どこまでいける?</span>
          <span class="fold-count">未収録 <span class="num">{uncoveredCount}</span></span>
        </summary>
        <div class="fold-body">
          {#if !app.evaluations[character.id]}
            <div class="retry-row">
              <span class="dim">到達判定を取得できていません。</span>
              <button type="button" class="btn" onclick={() => character && refreshEvaluation(character)}>再判定</button>
            </div>
          {/if}
          <div class="areas">
            {#each areas as area (area.id)}
              {@const open = isAreaOpen(area.id)}
              {@const shown = areaDisplayRows(area.id)}
              {@const okCount = shown.filter((r) => r.ev?.clear).length}
              <div class="area">
                <button type="button" class="mini-row" onclick={() => toggleArea(area.id)}>
                  <span class="name">{area.name}</span>
                  <span class="meter">
                    <span
                      class="fill"
                      style="width: {shown.length ? (okCount / shown.length) * 100 : 0}%; background: var(--state-met-bar);"
                    ></span>
                  </span>
                  <span class="num count">{okCount} / {shown.length}</span>
                  <span class="chev dim">{open ? "▴" : "▾"}</span>
                </button>
                {#if open}
                  <div class="rows open-in">
                    {#each shown as r (r.content.series?.id ?? r.content.id)}
                      {@const st = rowState(r)}
                      {@const cov = coverage(r)}
                      {@const note = noteOf(r)}
                      <div
                        class="row"
                        role="button"
                        tabindex="0"
                        onclick={() => openInCalc(r.content.id)}
                        onkeydown={(e) => e.key === "Enter" && openInCalc(r.content.id)}
                      >
                        {#if r.content.id === frontierId}
                          <div class="frontier">次はここ</div>
                        {/if}
                        <div class="row-main">
                          <Icon kind="mob" id={r.content.enemy_id} size={28} label={r.content.name} />
                          <!-- 収録度は行頭に 1 つだけ(§14 決定 5)。完全な行には出さない -->
                          {#if cov !== null}<span class="coverage">{cov}</span>{/if}
                          {#if r.content.series}
                            {@const series = r.content.series}
                            {@const list = seriesRowsOf(series.id)}
                            {@const maxStep = list[list.length - 1]?.content.series?.step ?? series.step}
                            <span class="name">{series.name}</span>
                            <span class="series-stepper">
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
                          <span class="chev dim">›</span>
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
        </div>
      </details>

      <!-- ===== うごき: 上のカードに入らなかった新着(ツールの更新履歴。外部フィードは今回スコープ外) ===== -->
      <div class="section">
        <div class="area-head">
          <span class="area-name">うごき</span>
          <span class="area-rule"></span>
        </div>
        {#each FEED_ITEMS as item (item.date + item.title)}
          <div class="tl-row">
            <span class="tl-date num">{fmtMonthDay(item.date)}</span>
            <span class="tag">{item.source === "tool" ? "ツール" : item.source === "official" ? "TW公式" : "韓国"}</span>
            <span class="tl-text">{item.title}</span>
            {#if item.note}<span class="tl-note">{item.note}</span>{/if}
            {#if item.url}<span class="ext dim">↗</span>{/if}
          </div>
        {/each}
      </div>

      <p class="foot dim">
        入場条件は swiki「コンテンツ入場条件」由来。装備条件は使うスキルの依存(突き/斬り/魔攻/魔防/複合)で比較先が変わります。
        目安ダメージは wiki に無い値で、コミュニティ知識・実測が出典です(実測で更新)。
      </p>
    {/if}
  </div>
</div>

<style>
  .home { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 940px; }
  .empty { font-size: 12px; }

  .retry-row { display: flex; align-items: center; gap: 10px; margin-bottom: 10px; font-size: 11px; }

  /* 帯ラベル・行動チップ。「うごき」= ソース分類、影響カード = カード種別、と 1 部品 2 用途 */
  .tag {
    flex: none; width: 52px; text-align: center; padding: 1px 0; border-radius: var(--r-pill);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted); white-space: nowrap;
  }
  .cta {
    flex: none; display: inline-flex; align-items: center; gap: 5px; padding: 4px 12px; border-radius: var(--r-pill);
    background: #fff; border: 1px solid var(--border-soft); font-size: 9.5px; font-weight: 700; color: var(--accent); white-space: nowrap;
  }
  .cta:hover { border-color: var(--accent); }

  /* ===== キャラの窓ヒーロー ============================================= */
  .hero {
    display: flex; flex-direction: column; gap: 10px; padding: 13px 15px; border-radius: var(--r-window);
    background: linear-gradient(180deg, #fff, #E8F1FB 94%);
    border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 #fff, 0 1px 3px rgba(30, 44, 74, 0.12);
  }
  .hero-top { display: flex; align-items: stretch; gap: 14px; min-width: 0; }

  .hero-id {
    flex: none; width: 172px; display: flex; flex-direction: column; align-items: center; gap: 6px;
    padding: 10px 10px 9px; border-radius: var(--r-panel); background: var(--bg-panel); border: 1px solid var(--border-soft);
  }
  .hero-id-name { font-size: 14px; font-weight: 800; max-width: 150px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hero-id-class { font-size: 9.5px; color: var(--fg-muted); white-space: nowrap; text-align: center; }
  .hero-id-class .strong { font-weight: 700; color: var(--fg); }
  .hero-panels { min-width: 0; flex: 1; display: flex; align-items: stretch; gap: 6px; }
  .hero-panel {
    flex: 1; min-width: 0; display: flex; flex-direction: column; padding: 7px 0 8px;
    border-radius: var(--r-panel); background: var(--bg-panel); border: 1px solid var(--border-soft);
  }
  .hero-panel-title { padding: 0 11px 4px; font-size: 8.5px; font-weight: 700; letter-spacing: 0.1em; color: var(--fg-muted); }
  .hero-row { display: flex; align-items: baseline; gap: 8px; padding: 3px 11px; border-top: 1px dashed var(--border-soft); min-width: 0; }
  .hero-row.first { border-top: none; }
  .hero-row-label { font-size: 9px; font-weight: 700; letter-spacing: 0.06em; color: var(--fg-muted); white-space: nowrap; }
  .hero-row-value { margin-left: auto; font-size: 12.5px; font-weight: 700; color: var(--fg); white-space: nowrap; }
  .hero-row-value-wrap { margin-left: auto; display: flex; align-items: baseline; gap: 6px; }
  .hero-sub { font-size: 8.5px; color: var(--fg-dim); white-space: nowrap; }

  .hero-goal {
    display: flex; align-items: center; gap: 9px; padding: 8px 12px; border-radius: var(--r-panel);
    background: var(--bg-panel); border: 1px solid var(--border-strong); min-width: 0;
  }
  .hero-goal-name { min-width: 0; flex: none; max-width: 170px; font-size: 12px; font-weight: 800; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .hero-goal-note { min-width: 0; flex: 1; font-size: var(--t-label); }
  .hero-div { width: 1px; align-self: stretch; background: var(--border-soft); }
  .hero-goal-skill { min-width: 0; max-width: 100px; font-size: 10px; font-weight: 700; color: var(--fg-sub); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .hero-meter { flex: 1; height: 12px; }
  .hero-spot-wrap { flex-shrink: 0; white-space: nowrap; }
  .hero-spot { font-size: 27px; line-height: 1; font-weight: 700; color: #16223A; text-shadow: 0 1px 0 #fff; }

  .hero-advice { display: flex; flex-direction: column; gap: 5px; border-top: 1px dashed var(--border-soft); padding-top: 9px; }
  .hero-advice-title { font-size: 10px; font-weight: 700; letter-spacing: 0.1em; color: var(--fg-muted); }
  .hero-advice-list { display: flex; flex-direction: column; gap: 5px; }
  .hero-advice-row {
    display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft); min-width: 0; text-align: left;
  }
  .hero-advice-row:hover { border-color: var(--accent); }
  .hero-advice-row .rank {
    flex: none; width: 18px; height: 18px; display: grid; place-items: center; border-radius: 50%;
    background: var(--bg-rail); border: 1px solid var(--border-soft); font-size: 10px; font-weight: 700; color: var(--fg-sub);
  }
  .hero-advice-label { min-width: 0; flex: 1; font-size: 10.5px; font-weight: 700; color: var(--fg-sub); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .hero-advice-nums { flex-shrink: 0; white-space: nowrap; }
  .hero-advice-nums .num { font-size: 12px; font-weight: 700; color: var(--sim-fg); }
  .hero-advice-nums .num:last-child { font-size: 9px; }
  .hero-advice-row .chev { flex-shrink: 0; font-size: 9px; }
  .cost { flex-shrink: 0; padding: 1px 8px; border-radius: var(--r-pill); border: 1px solid; font-size: 9px; font-weight: 700; white-space: nowrap; }

  /* ===== 汎用セクション見出し(期限・影響 / 反映 / うごき) ===== */
  .section { display: flex; flex-direction: column; gap: 6px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: var(--r-inset); background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }
  .last-reflect { flex: none; font-size: 9px; white-space: nowrap; }

  /* ===== 期限・影響カード ===== */
  .brief-card {
    display: flex; align-items: center; gap: 12px; padding: 12px 15px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgba(30, 44, 74, 0.08);
  }
  .brief-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .brief-title { font-size: 14px; font-weight: 800; color: var(--fg-head); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .brief-why { font-size: 10px; color: var(--fg-muted); line-height: 1.4; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* ===== 反映: 部位タイル ===== */
  .parts-grid { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 6px; }
  .part-tile {
    display: flex; align-items: center; gap: 7px; padding: 6px 9px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left; min-width: 0;
  }
  .part-tile:hover { border-color: var(--accent); }
  .part-tile-name { min-width: 0; flex: 1; font-size: 10px; font-weight: 700; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .part-tile-val { flex-shrink: 0; font-size: 12px; font-weight: 700; white-space: nowrap; }
  .tile-more { align-self: flex-start; }

  /* ===== どこまでいける?(details.fold は app.css 側の畳み見た目を継承) ===== */
  .reach-fold summary { display: flex; align-items: center; gap: 9px; }
  .fold-count { font-size: 10.5px; font-weight: 700; color: var(--fg-sub); }

  .areas { margin-top: 4px; display: flex; flex-direction: column; gap: 8px; }
  .area { display: flex; flex-direction: column; gap: 6px; }
  .mini-row {
    display: flex; align-items: center; gap: 9px; padding: 7px 10px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .mini-row:hover { border-color: var(--accent); }
  .mini-row .name { min-width: 0; flex: 1; font-size: 10.5px; font-weight: 700; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mini-row .meter { width: 90px; flex: none; }
  .mini-row .count { flex: none; width: 46px; text-align: right; font-size: 10.5px; font-weight: 700; color: var(--fg-sub); }

  .rows { padding-left: 6px; display: flex; flex-direction: column; gap: 6px; }
  .row {
    padding: 9px 12px; border-radius: var(--r-window); cursor: pointer;
    background: var(--bg-field); border: 1px solid var(--border-soft);
  }
  .frontier {
    display: inline-flex; align-items: center; margin-bottom: 6px; padding: 2px 9px; border-radius: var(--r-pill);
    background: var(--sel); border: 1px solid var(--sel-bd);
    font-size: 9.5px; font-weight: 700; color: var(--sel-fg);
  }
  .row-main { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .row-main .name { flex: 1; min-width: 0; font-size: 12.5px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row-main .dmg { flex-shrink: 0; font-size: 15px; font-weight: 700; white-space: nowrap; }
  .row-main .chev { flex-shrink: 0; font-size: 9px; }

  .row-bar { margin-top: 6px; display: flex; align-items: center; gap: 9px; }
  .row-bar .meter { flex: 1 1 auto; min-width: 40px; }
  .row-bar .need { flex-shrink: 0; font-size: 10px; white-space: nowrap; }
  .row-bar .over { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: var(--accent-hover); }
  .row-bar .badge { margin-left: auto; }

  .row-note { margin-top: 5px; display: flex; align-items: center; gap: 7px; min-width: 0; }
  /* .coverage は app.css(§14 決定 5)。行内の歯抜けはこの 1 つが引き受ける */
  .entry-dot { width: 5px; height: 5px; flex-shrink: 0; border-radius: 50%; }
  .note-text { flex: 1; min-width: 0; font-size: var(--t-label); color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .note-text.unmet { color: var(--danger); font-weight: 700; }
  .team {
    flex-shrink: 0; padding: 1px 6px; border-radius: var(--r-pill);
    background: var(--state-temp-bg); border: 1px solid var(--sim);
    font-size: 8.5px; font-weight: 700; color: var(--sim-fg);
  }

  /* 難易度送り。app.css §07 の .stepper(StatInput 専用)と紛らわしいので別名にする */
  .series-stepper {
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

  /* ===== うごき ===== */
  .tl-row {
    display: flex; align-items: center; gap: 9px; padding: 7px 12px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-soft);
  }
  .tl-date { flex: none; width: 38px; font-size: 9.5px; color: var(--fg-dim); }
  .tl-text { min-width: 0; flex: 1; font-size: 11px; color: var(--fg-sub); line-height: 1.45; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tl-note { flex: none; font-size: 9.5px; color: var(--fg-dim); white-space: nowrap; }
  .ext { flex: none; font-size: 10px; }

  .foot { margin: 0; font-size: 10px; line-height: 1.7; }
</style>
