<script lang="ts" module>
  // 「目安火力が変わった」影響カードは、キャラごとにセッション中 1 回だけ判定する
  // (起動中に何度もホームへ戻っても再表示しない)。モジュールスコープなので
  // タブ切替でこのコンポーネントが作り直されても保持される。
  const checkedSnapshotIds = new Set<number>();
</script>

<script lang="ts">
  // ホーム: 選択中キャラの「キャラの窓」ヒーロー(現況・次の目標・おすすめ強化)+
  // 今日の期限・影響 + 今日の強化(5 項目タイル。押すとグリッド全体の下に展開)+ 到達一覧(畳み)のブリーフィング型 1 カラム(更新内容は「お知らせ」タブ)。
  // 判定はすべて Rust 側(evaluate_contents / preview_effective_stats / preview_defense / preview_damage)。
  // この画面は表示と選択のみ。
  import {
    errorMessage, getDamageSnapshot, listSkills, listUpgradeCandidates, previewDamage, previewDefense,
    previewEffectiveStats, setDamageSnapshot,
  } from "../../api/commands";
  import type {
    Content, ContentEvaluation, DefenseProfile, EquipmentItem, EquipmentPart, PartSlot,
    RegisteredCharacter, SkillDependency, StatKind, StatPreview, UpgradeCandidate,
  } from "../../api/types";
  import { COST_COLORS, COST_LABELS } from "../../candidates";
  import {
    enchantCap as enchantCapShared, enchantDepKeysFor, enchantRows as enchantRowsShared, ENCHANT_SLOT_LABELS,
    type EnchantDepKey,
  } from "../../enchant";
  import {
    applyCatalogItem, equipmentIconId, sacredRelicStageFromValue, sacredRelicValue, selectedSienaAura, sienaStage,
    valuesSummary,
  } from "../../equipment";
  import { fmtInt, fmtMonthDay } from "../../format";
  import {
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT, SIENA_ALLOWED_SLOTS, STAT_KINDS, STAT_LABELS,
  } from "../../labels";
  import type { EquipmentStatKind } from "../../labels";
  import { limits } from "../../limits.svelte";
  import {
    app, buffSelectionFor, enqueueCharacterSave, evaluationFor, flatContents, focusCharacterSource, gameCharacterName,
    payloadOf, refreshEvaluation, selectedCharacter, totalContents, upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { critChanceStage } from "../../ui/critChance";
  import Icon from "../../ui/Icon.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { bump, flash, swap } from "../../ui/motion.svelte";
  import Picker, { type PickerOption } from "../../ui/Picker.svelte";
  import { badgeStyle, REACH_BADGES, STATE, triadStyle, type Badge } from "../../ui/states";
  import StatInput from "../../ui/StatInput.svelte";

  const character = $derived(selectedCharacter());
  const totalCount = $derived(totalContents());

  const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];
  const today = new Date();
  const todayLabel = `${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}(${WEEKDAYS[today.getDay()]})`;
  const daysAgo = (iso: string) => Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000));

  // 判定に使ったスキル名の表示用(evaluate_contents は「最大ダメージのスキル」で判定する)。
  // 依存種別は「装備・命中」パネルの表示行の切り替えに使う
  let skillNames = $state<Record<string, string>>({});
  let skillDeps = $state<Record<string, SkillDependency>>({});
  $effect(() => {
    const gid = character?.game_character_id;
    if (!gid) {
      skillNames = {};
      skillDeps = {};
      return;
    }
    listSkills(gid)
      .then((list) => {
        skillNames = Object.fromEntries(list.map((s) => [s.id, s.name]));
        skillDeps = Object.fromEntries(list.map((s) => [s.id, s.dependency]));
      })
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
    const ratio = r.ev.damage.per_hit_primary / r.content.need_per_hit;
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
    return r.ev?.damage ? r.ev.damage.per_hit_primary / r.content.need_per_hit : 0;
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
  /**
   * 自動で選ぶ「次の目標」= 火力目標(必要 /hit)のある最初の未クリア。
   * 入場条件だけのコンテンツ(敵データなし)が先にあっても飛ばす — スポットライトの答えは
   * 「どのスキルでどのくらい出るか」であり、それが出せない目標を主役に据えない。
   * 火力目標のある未クリアが 1 つも無ければ frontier(最初の未クリア)へ落とす。
   */
  const autoGoal = $derived(
    rows.find((r) => r.ev && !r.ev.clear && r.content.need_per_hit !== null && r.ev.damage) ??
      rows.find((r) => r.content.id === frontierId) ??
      null,
  );
  /**
   * ユーザーが自分で据えた目標(character.goal_content_id)。「クリアできる」と
   * 「周回したい・詰めたい」は別なので、自動判定を上書きできるようにしてある。
   * 未設定なら null で、自動判定がそのまま主役になる(自動はやめない)。
   * 敵データを持たないコンテンツは火力も命中Pも出せないので、選択肢にも据え先にもしない。
   */
  const manualGoal = $derived.by(() => {
    const id = character?.goal_content_id ?? null;
    if (id === null) return null;
    return rows.find((r) => r.content.id === id && r.content.enemy_id !== null) ?? null;
  });
  /** 保存された目標が今のデータに無い(消えた・敵データが落ちた)。その旨を出して自動へ落とす */
  const goalStale = $derived(rows.length > 0 && character?.goal_content_id != null && manualGoal === null);
  const heroGoal = $derived(manualGoal ?? autoGoal);
  /**
   * 目標の候補。先頭は「自動」に戻す行で、自動なら**どこが目標になるか**まで出す
   * (§00 05: 戻した先を頭の中で当てさせない)。以降は敵データのあるコンテンツだけ
   * (計算タブの対象ピッカーと同じ絞り込み。選べない行を一覧に残さない)。
   */
  const goalOptions = $derived<PickerOption[]>([
    {
      value: "",
      name: `自動: ${autoGoal ? (autoGoal.content.series?.name ?? autoGoal.content.name) : "目標なし"}`,
    },
    ...rows
      .filter((r) => r.content.enemy_id !== null)
      .map((r) => ({ value: r.content.id, name: r.content.name, meta: r.areaName })),
  ]);
  /**
   * 「次の目標」を保存する。null = 自動に戻す。保存経路は「今日の強化」と同じ直更新
   * (キャラ全体の上書き保存をキャラ単位のキューに通す)。
   */
  function commitGoal(c: RegisteredCharacter, contentId: string | null) {
    commitFieldUpdate(
      c, `${c.id}:goal_content_id`,
      (cc) => cc.goal_content_id,
      (cc, v) => { cc.goal_content_id = v; },
      contentId,
    );
  }

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
  /** 見出しの主役はクリア済み数(現況)。未収録数は詳細情報として脇に残す(§00 02)。 */
  const clearedCount = $derived(rows.filter((r) => r.ev?.clear).length);

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

  // 装備値の内訳(基本能力値 + 強化能力値 = 合計)。どちらも preview(Rust 側の計算済み値)。
  const heroEnhanced = $derived(heroStats?.equipment_enhanced_total ?? null);
  // 装備値の表示行はスポットライトのスキルの依存で切り替える(HI 依存に突きを見せない)。
  // 依存種別 → 見るステ 2 本はドメイン(装備攻撃力係数)から起動時に引いた静的テーブルを使う
  // (enchant.ts の enchantDepKeysFor。ルール表をフロントに持たない)。
  const EQUIP_ROW_LABELS = { thrust: "突き", slash: "斬り", magic_attack: "魔攻", magic_defense: "魔防" } as const;
  const heroEquipRows = $derived.by(() => {
    const stats = heroStats;
    const enhanced = heroEnhanced;
    if (!stats || !enhanced) return [];
    const skillId = heroSpot?.skillId ?? character?.main_skill_id ?? null;
    const dep = (skillId ? skillDeps[skillId] : null) ?? "stab_hack";
    return enchantDepKeysFor(dep).map((key) => ({
      key,
      label: EQUIP_ROW_LABELS[key],
      base: stats.equipment_base_total[key],
      enhanced: enhanced[key],
      total: stats.equipment_base_total[key] + enhanced[key],
    }));
  });

  // 命中P(次の目標のスキルで判定。BestSkillDamage には無いので previewDamage を別途叩く)と、
  // おすすめ強化(list_upgrade_candidates。列挙・並び順は Rust 側。上位 3 件を表示)
  let heroAccuracy = $state<number | null>(null);
  let heroAdvice = $state<UpgradeCandidate[]>([]);
  /** 伸び率の表示。表記ダメージと合計ダメージの 2 本を同じ書き方で並べる(計算タブと同じ) */
  const deltaText = (pct: number) => (pct === 0 ? "±0%" : `${pct > 0 ? "+" : ""}${pct}%`);
  /** スポットライトの /hit(previewDamage の結果)。主軸スキル設定済みならそのスキルの値 */
  let heroDamage = $state<{
    skillId: string;
    perHit: number;
    /** クリティカル率(0..1)。critRate が null(wiki 未記載)なら確定扱いの 1.0 */
    critChance: number;
    /** wiki スキル性能一覧の Cri値。null = 未記載 */
    critRate: number | null;
  } | null>(null);
  const heroAdviceLatest = latest({ debounce: 150 });

  /**
   * スポットライトに出すスキルと /hit。キャラが主軸スキルを選んでいればそれ(自分のビルドの答え)、
   * 未選択・previewDamage 反映前は到達判定と同じ最大ダメージスキルで埋める。
   */
  const heroSpot = $derived(
    heroGoal?.ev?.damage
      ? (heroDamage ?? { skillId: heroGoal.ev.damage.skill_id, perHit: heroGoal.ev.damage.per_hit_primary })
      : null,
  );
  /** スポットライトの到達状態(rowState と同じ段。判定値は heroSpot の /hit) */
  const heroSpotState = $derived.by(() => {
    const g = heroGoal;
    const s = heroSpot;
    if (!g?.ev || g.content.need_per_hit === null || !s) return 6;
    const ratio = s.perHit / g.content.need_per_hit;
    if (!g.ev.entry_ok) return ratio >= 1 ? 5 : 4;
    return ratio >= 1.3 ? 0 : ratio >= 1 ? 1 : ratio >= 0.8 ? 2 : 3;
  });
  const heroSpotPct = $derived(
    heroGoal?.content.need_per_hit && heroSpot
      ? `${Math.min(100, (heroSpot.perHit / heroGoal.content.need_per_hit) * 100).toFixed(1)}%`
      : "0%",
  );
  /** 次の目標の火力が必要値未満か(おすすめ強化を出す条件) */
  const heroPowerShort = $derived(
    heroGoal?.content.need_per_hit != null && heroSpot != null && heroSpot.perHit < heroGoal.content.need_per_hit,
  );
  /** 次の目標の入場条件が未達か(条件の行を出す条件) */
  const heroEntryUnmet = $derived(heroGoal?.ev != null && !heroGoal.ev.entry_ok);
  /** 命中Pが出せない理由(design-system: 未収録は空白や「—」ではなく理由を読める形にする)。
   *  算出できているときは null。 */
  const heroAccuracyReason = $derived.by(() => {
    if (heroAccuracy !== null) return null;
    const g = heroGoal;
    if (!g) return "対象コンテンツなし";
    if (!g.ev) return "判定中";
    if (g.content.enemy_id === null) return "敵データ未収録";
    if (!g.ev.damage) return "スキル未収録";
    return "算出中";
  });
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
      heroDamage = null;
      return;
    }
    // 主軸スキル設定済みならスポットライト・おすすめもそのスキルで計算(未設定は判定スキル = 最大ダメージ)
    const skillId = c.main_skill_id ?? g.ev.damage.skill_id;
    const contentId = g.content.id;
    const buffs = buffSelectionFor(c);
    heroDamage = null;
    heroAdviceLatest.run(async (isCurrent) => {
      try {
        const current = await previewDamage(payloadOf(c), skillId, contentId, 0, null, null, buffs);
        const results = await listUpgradeCandidates(payloadOf(c), skillId, contentId, 0, null, null, buffs);
        if (isCurrent()) {
          heroAccuracy = current.accuracy_point;
          heroDamage = {
            skillId,
            perHit: current.per_hit_primary,
            critChance: current.critical_chance,
            critRate: current.critical_rate?.value ?? null,
          };
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

  async function applyHeroAdvice(a: UpgradeCandidate) {
    const g = heroGoal;
    if (!character || !g?.ev?.damage) return;
    if (app.sim === null) {
      app.sim = a.applied;
      app.calcTargetId = g.content.id;
      app.tab = "calc";
      return;
    }
    // 既存の試し変更(app.sim)がある場合は無確認で捨てず、その上に重ねる。
    // 同じ候補 id が sim 基点でも成立するなら、その applied を採用する。
    const skillId = character.main_skill_id ?? g.ev.damage.skill_id;
    const buffs = buffSelectionFor(character);
    try {
      const results = await listUpgradeCandidates(app.sim, skillId, g.content.id, 0, null, null, buffs);
      const same = results.find((r) => r.id === a.id);
      if (same) app.sim = same.applied;
      // 同 id が見つからない(sim で適用済み等)場合は sim を変えず計算タブへ遷移だけする。
    } catch (e) {
      reportError(errorMessage(e));
    }
    app.calcTargetId = g.content.id;
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
    const perHit = g.ev.damage.per_hit_primary;
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

  // ===== 今日の強化: 5 つの項目タイル。押すとグリッド全体の下に展開する ==========================
  // ユーザーからの訂正: 装備強化(武器・鎧の強化Lv)は低頻度。実際のデイリーはレリック・カフス・
  // 神鳥の聖物・エンチャント・シエナのオーラなので、直更新の対象はこの 5 つにする。
  // 武器・鎧の強化Lvは「そのほかの設定」からキャラタブへ回す。
  type TodayTile = "sacredRelic" | "cuffs" | "enchant" | "equipRelic" | "siena";
  let openTile = $state<TodayTile | null>(null);
  function toggleTile(t: TodayTile) {
    openTile = openTile === t ? null : t;
  }

  // エンチャントは間違って盛ってしまっても直せるように、値を押すとその場でテキスト編集にする
  // (StatInput の read↔editing パターンと同じ考え方。増分チップはやり直しの手段を持たないため)。
  let editingEnchant = $state<string | null>(null);
  function commitEnchantText(slot: PartSlot, k: EnchantDepKey, cap: number, raw: string) {
    const n = Math.round(Number(raw));
    const clamped = Number.isFinite(n) ? Math.max(0, Math.min(cap, n)) : 0;
    commitEnchant(character!, slot, k, clamped);
    editingEnchant = null;
  }

  function partOf(slot: PartSlot): EquipmentPart | null {
    if (!character) return null;
    const list = character.equipment.parts[slot];
    return list.registered.find((p) => p.id === list.selected_id) ?? null;
  }
  const isUnequipped = (part: EquipmentPart | null) =>
    !part || (part.item_id === null && part.custom_name === null);
  const itemOf = (part: EquipmentPart | null): EquipmentItem | null =>
    part?.item_id ? (app.equipmentCatalog.find((it) => it.id === part.item_id) ?? null) : null;
  /**
   * get/set が触れる対象の最小形。RegisteredCharacter・app.sim(NewCharacter)・保存前ペイロードの
   * いずれも equipment/stat_sources を持つので、この形だけを要求すれば 3 者に共通で使える
   * (試し変更 app.sim もここを通じて最新に保つ。バグB対応)。
   */
  type FieldSaveTarget = {
    equipment: RegisteredCharacter["equipment"];
    stat_sources: RegisteredCharacter["stat_sources"];
    /** ホームの「次の目標」。装備・補正源と同じ直更新の経路で保存する */
    goal_content_id: RegisteredCharacter["goal_content_id"];
  };

  interface FieldSaveState<T> { timer: ReturnType<typeof setTimeout> | null; baseline: T }
  const fieldSaveState: Record<string, FieldSaveState<unknown>> = {};

  /**
   * 直更新の保存を汎用化したもの(部位・stat_sources 共通。Home の「今日の強化」の全項目が
   * ここを通る — 項目ごとに保存の仕組みを複製しない)。get/set で「キャラのどこを読み書きするか」
   * だけを渡す。実際の送信は enqueueCharacterSave(キャラ単位で直列化)へ通す — update_character は
   * 毎回キャラ全体を送る full-overwrite 方式なので、ここでの保存とキャラタブの保存
   * (Workspace.svelte の自動保存)がほぼ同時に走ると、後から解決した方が先に解決した方の変更を
   * 巻き戻す事故になる。共有キューを通せば、待っている間の 2 件目は 1 件目の結果を必ず拾ってから
   * 送信されるので、その事故が起きない。
   *
   * 連打・通信中の追加編集を失わない設計(独立レビュー指摘: 送信中の押下が黙って消える事故):
   * - ステッパー連打は「+N したい」という単一の意図なので、key(キャラ id + 項目)ごとに debounce
   *   して最新値だけを 1 回送る。
   * - 送信中(enqueueCharacterSave の応答待ち)にさらに押されても state は消さない。押した分は
   *   その場で楽観反映され、新しい debounce タイマーが張られて次のバーストとして続けて送られる。
   * - 応答が返ると upsertCharacter がキャラをオブジェクトごと差し替えるため、応答待ちの間に
   *   進んだ「まだ送っていない最新値」がその差し替えで消える窓ができる。ここは応答が来た瞬間に
   *   「差し替え直前の実際の値」を読み、差し替え後のオブジェクトへ同じ値を re-apply することで
   *   埋める(flushFieldUpdate 内)。
   * - state を消してよいのは、消す時点で新しい debounce タイマーが張られていない(＝これ以上
   *   送るべき保留編集が無い)ときだけ。
   *
   * 試し変更(app.sim)もサイレントに失わない(独立レビュー指摘: ステッパー1押しで sim が消える):
   * app.sim は equipment/stat_sources を持つ NewCharacter なので、楽観更新と同じ set() をそのまま
   * 適用して常に最新に保つ。upsertCharacter 側には preserveSim を渡し、ここで既に同期済みの
   * sim を無条件リセットさせない(元の全消し判断は sim と無関係な保存を前提にしていたため)。
   */
  function commitFieldUpdate<T>(
    c: RegisteredCharacter,
    key: string,
    get: (c: FieldSaveTarget) => T,
    set: (c: FieldSaveTarget, value: T) => void,
    nextValue: T,
  ) {
    let state = fieldSaveState[key] as FieldSaveState<T> | undefined;
    if (!state) {
      state = { timer: null, baseline: get(c) };
      fieldSaveState[key] = state as FieldSaveState<unknown>;
    }
    set(c, nextValue); // 楽観更新: 押した瞬間に数字が動く(§00 考えさせない)
    if (app.sim && app.selectedId === c.id) set(app.sim, nextValue); // 試し変更も同じ変更で最新に保つ
    if (state.timer) clearTimeout(state.timer);
    const characterId = c.id;
    const st = state;
    st.timer = setTimeout(() => {
      st.timer = null;
      void flushFieldUpdate(characterId, key, get, set);
    }, 350);
  }
  async function flushFieldUpdate<T>(
    characterId: number,
    key: string,
    get: (c: FieldSaveTarget) => T,
    set: (c: FieldSaveTarget, value: T) => void,
  ): Promise<void> {
    const state = fieldSaveState[key] as FieldSaveState<T> | undefined;
    if (!state) return;
    try {
      // buildPayload はキューで自分の番が来た瞬間に呼ばれる。待っている間に別の保存(キャラタブ
      // など)が先に確定していたら、その結果を含む最新の app.characters から組み立てる。
      const saved = await enqueueCharacterSave(characterId, () => {
        const c = app.characters.find((x) => x.id === characterId);
        if (!c) throw new Error("character not found");
        return payloadOf(c);
      });
      // 差し替え直前の「いま画面にある値」を控える。通信中にさらに押されていれば、これは
      // 今回送った値より先へ進んでいる(取りこぼし対策)。
      const liveBefore = app.characters.find((x) => x.id === characterId);
      const latestValue = liveBefore ? get(liveBefore) : state.baseline;
      const confirmedValue = get(saved);
      upsertCharacter(saved, { preserveSim: true }); // ここでオブジェクトごと差し替わる
      if (JSON.stringify(latestValue) !== JSON.stringify(confirmedValue)) {
        // 差し替え後の新しいオブジェクトへ、通信中に進んだ分を re-apply する(押した分を消さない)
        const liveAfter = app.characters.find((x) => x.id === characterId);
        if (liveAfter) set(liveAfter, latestValue);
      }
      state.baseline = confirmedValue;
      // まだ新しい debounce タイマーが張られていなければ(=これ以上保留の編集が無い)完了
      if (!state.timer) delete fieldSaveState[key];
    } catch (e) {
      delete fieldSaveState[key];
      const c = app.characters.find((x) => x.id === characterId);
      if (c) {
        set(c, state.baseline); // 失敗: 見た目を保存済みの値へ巻き戻す
        if (app.sim && app.selectedId === characterId) set(app.sim, state.baseline); // 試し変更も揃えて戻す
      }
      reportError(errorMessage(e));
    }
  }

  // --- 1. 神鳥の聖物(stat_sources.sacred_relic)。表示は SourcePane.svelte の relic セクション
  //    (254-266行)と完全に揃える: 実値 = 段階 × value_per_stage、1 押し = 1 段階。 ---
  const SACRED_RELIC_MAX_VALUE = limits.sacred_relic_stage_max * limits.sacred_relic_value_per_stage;
  function sacredRelicValueOf(c: RegisteredCharacter, k: StatKind): number {
    return sacredRelicValue(c.stat_sources.sacred_relic[k] ?? 0, limits.sacred_relic_value_per_stage);
  }
  function commitSacredRelic(c: RegisteredCharacter, k: StatKind, value: number) {
    const stage = sacredRelicStageFromValue(
      value,
      limits.sacred_relic_stage_max,
      limits.sacred_relic_value_per_stage,
    );
    commitFieldUpdate(
      c, `${c.id}:sacred_relic:${k}`,
      (cc) => cc.stat_sources.sacred_relic[k],
      (cc, v) => { cc.stat_sources.sacred_relic[k] = v; },
      stage,
    );
  }
  /** 何か盛ってあるかだけを見る(閉じたタイルの「未設定」バッジ判定)。内訳は開いて見せる —
   *  閉じた面に略字を並べるとノイズになるため(ユーザー判断)。 */
  const sacredRelicSet = $derived.by(() => {
    const c = character;
    return c ? STAT_KINDS.some((k) => sacredRelicValueOf(c, k) > 0) : false;
  });
  /** まだ伸ばせる余地(全ステ合計、上限まで)。「合計 +N」ではなく「あと伸ばせる分」だけを見せる
   *  (ユーザー判断: 意味のない達成量の合計は出さない)。 */
  const sacredRelicRemaining = $derived.by(() => {
    const c = character;
    return c ? STAT_KINDS.reduce((sum, k) => sum + Math.max(0, SACRED_RELIC_MAX_VALUE - sacredRelicValueOf(c, k)), 0) : 0;
  });

  // --- 2. カフス(shield_plus の成長値)。編集規則は EquipmentPane.svelte の
  //    growth-equipment-card(874-895行)と完全に同じにする: 対象ステは growth_caps[k] > 0 の
  //    ものだけ、範囲は values_min[k]..growth_caps[k](スカラーの growth_cap ではない)。
  //    growth_caps を持たない装備(通常のカフス)・未装備はステッパーを出さない。 ---
  function cuffsPart(c: FieldSaveTarget): EquipmentPart | null {
    const list = c.equipment.parts.shield_plus;
    return list.registered.find((p) => p.id === list.selected_id) ?? null;
  }
  const cuffsItem = $derived(character ? itemOf(cuffsPart(character)) : null);
  const cuffsGrowthKeys = $derived(
    cuffsItem?.growth_caps ? EQUIPMENT_STAT_KINDS.filter((k) => cuffsItem!.growth_caps![k] > 0) : [],
  );
  /** ステッパーを出せない理由。未装備は「未設定」、装備済みだが成長枠が無い(通常のカフス)は
   *  区別できる文言にする。ステッパーを出せるときは null。 */
  const cuffsUnsetLabel = $derived.by(() => {
    const c = character;
    if (!c || isUnequipped(cuffsPart(c))) return "未設定";
    if (!cuffsItem?.growth_caps) return "成長装備ではありません";
    return null;
  });
  function commitCuffsBase(c: RegisteredCharacter, k: EquipmentStatKind, value: number) {
    commitFieldUpdate(
      c, `${c.id}:shield_plus:base:${k}`,
      (cc) => cuffsPart(cc)?.base[k] ?? 0,
      (cc, v) => { const p = cuffsPart(cc); if (p) p.base[k] = v; },
      value,
    );
  }
  const cuffsSummary = $derived.by(() => {
    const c = character;
    const part = c ? cuffsPart(c) : null;
    const keys = cuffsGrowthKeys;
    const item = cuffsItem;
    if (!c || !part || !item?.growth_caps || keys.length === 0) return null;
    const growthCaps = item.growth_caps;
    return {
      value: keys.reduce((n, k) => n + part.base[k], 0),
      max: keys.reduce((n, k) => n + growthCaps[k], 0),
    };
  });
  /** この段階の上限までの残り(段が上がれば別の上限に変わる。「いまの段」だけを見る)。 */
  const cuffsRemaining = $derived(cuffsSummary ? cuffsSummary.max - cuffsSummary.value : null);

  // --- 3. エンチャント(各部位の part.enchant)。軸を主軸スキルの依存ステ(1〜2 本)に絞る
  //    (heroEquipRows と同じ enchantDepKeysFor を使う)。行 = そのステのエンチャント枠を持つ
  //    装備済み部位だけ(枠 0 の部位・未装備・カタログ外の品は出さない)。
  //    ルール表・上限のフォールバックは enchant.ts(ドメイン経由の 1 本)に寄せている。 ---
  /** キャラタブと同じ 4 種 + MAX(ユーザー要望: 増分の種類を絞りすぎないでほしい)。 */
  const ENCHANT_INCREMENTS = [12, 14, 17, 20] as const;
  const enchantDepKeys = $derived.by(() => {
    const skillId = character?.main_skill_id ?? null;
    const dep = (skillId ? skillDeps[skillId] : null) ?? "stab_hack";
    return enchantDepKeysFor(dep);
  });
  /** 上限が分からなければ `null`(ADR 001: フォールバックを持たない)。呼び出し側は「?」扱いで見せる。 */
  function enchantCap(part: EquipmentPart, k: EnchantDepKey): number | null {
    return enchantCapShared(part, k, app.equipmentCatalog);
  }
  /** 上限が 1 本も分からない(カタログ外で実測上限も未入力)部位は落とさず「未収録」行として出す
   *  (enchant.ts 共通。CalcPage と同じ経路)。 */
  const enchantRows = $derived.by(() => {
    if (!character) return [];
    return enchantRowsShared(character.equipment, enchantDepKeys, app.equipmentCatalog);
  });
  function commitEnchant(c: RegisteredCharacter, slot: PartSlot, k: EnchantDepKey, value: number) {
    commitFieldUpdate(
      c, `${c.id}:${slot}:enchant:${k}`,
      (cc) => {
        const l = cc.equipment.parts[slot];
        return l.registered.find((p) => p.id === l.selected_id)?.enchant[k] ?? 0;
      },
      (cc, v) => {
        const l = cc.equipment.parts[slot];
        const p = l.registered.find((x) => x.id === l.selected_id);
        if (p) p.enchant[k] = v;
      },
      value,
    );
  }
  const enchantSummary = $derived.by(() => {
    const rows = enchantRows;
    const keys = enchantDepKeys;
    if (rows.length === 0) return null;
    let value = 0;
    let max = 0;
    for (const { part } of rows) {
      for (const k of keys) {
        const cap = enchantCap(part, k);
        if (cap === null || cap <= 0) continue;
        value += part.enchant[k];
        max += cap;
      }
    }
    return { value, max };
  });
  /** まだ上限に届いていない部位の数と、その残り量の合計(主軸スキルの依存ステのみ)。
   *  上限が未収録の部位(capUnknown)は数に入れない — 別途「未収録」行として案内する。 */
  const enchantRemaining = $derived.by(() => {
    const rows = enchantRows;
    const keys = enchantDepKeys;
    let remain = 0;
    const shortParts = new Set<PartSlot>();
    for (const { slot, part } of rows) {
      for (const k of keys) {
        const cap = enchantCap(part, k);
        if (cap === null || cap <= 0) continue;
        const short = cap - part.enchant[k];
        if (short > 0) {
          remain += short;
          shortParts.add(slot);
        }
      }
    }
    return { remain, parts: shortParts.size };
  });

  // --- 4. レリック左右。カタログの次段(+レベル)へ差し替える(EquipmentPane.pickRelicLevel と
  //    同じ結果になるよう equipment.ts の applyCatalogItem を共有する)。神鳥は 1→10、ルナリアは
  //    別系列で 1→10(kind の外へは踏み出さない。相手 kind への切り替えはキャラタブで行う)。
  //    各段には補正値の範囲があり(gamedata: relic_item は growth_caps = values_max)、実際の値は
  //    直前段階のMAXから始まり表示段階のMAXまで育つ(wiki)。+レベルで次の段へ進めるのは、
  //    いまの段の補正値を上限まで上げてから、というゲーム内の順序をそのまま表現する。
  const RELIC_ROWS = [
    { slot: "relic_pendant", side: "左" },
    { slot: "relic_bracelet", side: "右" },
  ] as const;
  const RELIC_KIND_LABELS = { godbird: "神鳥", lunaria: "ルナリア" } as const;
  const relicPartName = (slot: "relic_pendant" | "relic_bracelet") =>
    slot === "relic_pendant" ? "pendant" : "bracelet";
  const relicKindOf = (itemId: string): "godbird" | "lunaria" | null =>
    itemId.startsWith("godbird-") ? "godbird" : itemId.startsWith("lunaria-") ? "lunaria" : null;
  const relicLevelOf = (itemId: string): number | null => {
    const m = itemId.match(/-plus(\d+)$/);
    return m ? Number(m[1]) : null;
  };
  const relicItemFor = (slot: "relic_pendant" | "relic_bracelet", kind: string, level: number): EquipmentItem | null =>
    app.equipmentCatalog.find((it) => it.id === `${kind}-${relicPartName(slot)}-plus${level}`) ?? null;
  /** 同 kind(神鳥/ルナリア)でカタログに存在する最大 Lv。この kind の系列の外へは踏み出さない。 */
  const relicMaxLevel = (slot: "relic_pendant" | "relic_bracelet", kind: string): number => {
    const prefix = `${kind}-${relicPartName(slot)}-plus`;
    return app.equipmentCatalog.reduce((max, it) => {
      if (!it.id.startsWith(prefix)) return max;
      const lv = Number(it.id.slice(prefix.length));
      return Number.isFinite(lv) ? Math.max(max, lv) : max;
    }, 0);
  };
  /** いまの段で補正値を上げられるステ(カタログの growth_caps がそのまま今の段の上限)。 */
  function relicGrowthKeys(item: EquipmentItem | null): EquipmentStatKind[] {
    return item?.growth_caps ? EQUIPMENT_STAT_KINDS.filter((k) => item.growth_caps![k] > 0) : [];
  }
  /** 補正値がいまの段の上限まで埋まっているか。埋まっていなければ次の段には進めない
   *  (ゲーム内の育成順序: 補正値が育ち切ってから+レベル)。育成対象ステが無ければ常に true。 */
  function relicGrowthComplete(part: EquipmentPart, item: EquipmentItem | null): boolean {
    const keys = relicGrowthKeys(item);
    return keys.length === 0 || keys.every((k) => part.base[k] >= item!.growth_caps![k]);
  }
  interface RelicState {
    value: number; max: number; kind: "godbird" | "lunaria";
    canDown: boolean; canUp: boolean; growthDone: boolean;
  }
  function relicState(slot: "relic_pendant" | "relic_bracelet", part: EquipmentPart | null): RelicState | null {
    if (!part || part.item_id === null) return null;
    const kind = relicKindOf(part.item_id);
    const level = relicLevelOf(part.item_id);
    if (!kind || level === null) return null;
    const max = relicMaxLevel(slot, kind);
    const growthDone = relicGrowthComplete(part, itemOf(part));
    return { value: level, max, kind, canDown: level > 1, canUp: level < max && growthDone, growthDone };
  }
  function commitRelicLevel(c: RegisteredCharacter, slot: "relic_pendant" | "relic_bracelet", nextPart: EquipmentPart) {
    commitFieldUpdate(
      c, `${c.id}:${slot}`,
      (cc) => {
        const l = cc.equipment.parts[slot];
        return l.registered.find((p) => p.id === l.selected_id) ?? nextPart;
      },
      (cc, part) => {
        const l = cc.equipment.parts[slot];
        const i = l.registered.findIndex((p) => p.id === l.selected_id);
        if (i >= 0) l.registered[i] = part;
      },
      nextPart,
    );
  }
  /** 段の補正値(part.base)の直更新。カフスの成長値編集(commitCuffsBase)と同じ形。 */
  function commitRelicBase(c: RegisteredCharacter, slot: "relic_pendant" | "relic_bracelet", k: EquipmentStatKind, value: number) {
    commitFieldUpdate(
      c, `${c.id}:${slot}:base:${k}`,
      (cc) => {
        const l = cc.equipment.parts[slot];
        return l.registered.find((p) => p.id === l.selected_id)?.base[k] ?? 0;
      },
      (cc, v) => {
        const l = cc.equipment.parts[slot];
        const p = l.registered.find((x) => x.id === l.selected_id);
        if (p) p.base[k] = v;
      },
      value,
    );
  }
  function stepRelicLevel(slot: "relic_pendant" | "relic_bracelet", dir: number) {
    const c = character;
    const part = partOf(slot);
    if (!c || !part || part.item_id === null) return;
    const kind = relicKindOf(part.item_id);
    const level = relicLevelOf(part.item_id);
    if (!kind || level === null) return;
    if (dir > 0 && !relicGrowthComplete(part, itemOf(part))) return; // 補正値が上限に届くまで進めない
    const item = relicItemFor(slot, kind, level + dir);
    if (!item) return;
    const next = applyCatalogItem(part, item);
    // レリックは直前段階のMAXから育って表示段階のMAXに到達する(wiki 注記)。applyCatalogItem は
    // 通常装備と同じく base を values_max(=この段の完成値)にするが、レリックを「段を上げた」
    // 直後は正しくは「まだ育っていない」状態(補正値 = この段の下限 = 直前段階の完成値)。
    // 段を下げる(dir<0)方向は「その段は育成済みだった」扱いのまま(values_max)でよい。
    const base = dir > 0 ? { ...item.values_min } : next.base;
    commitRelicLevel(c, slot, { ...next, base });
  }
  /** 片側の残り(補正値がまだ上限に届いていなければそちら優先。届いていれば次の段への Lv 差)。
   *  null = その側は未装備。 */
  function relicSideRemaining(slot: "relic_pendant" | "relic_bracelet"): { text: string; done: boolean } | null {
    const part = partOf(slot);
    const rs = relicState(slot, part);
    if (!rs) return null;
    const item = itemOf(part);
    const growthKeys = relicGrowthKeys(item);
    if (growthKeys.length > 0 && !rs.growthDone) {
      const remain = growthKeys.reduce((sum, k) => sum + Math.max(0, item!.growth_caps![k] - part!.base[k]), 0);
      return { text: `補正値あと${fmtInt(remain)}`, done: false };
    }
    if (rs.canUp) return { text: `Lvあと${fmtInt(rs.max - rs.value)}`, done: false };
    return { text: "上限", done: true };
  }
  const relicSides = $derived(RELIC_ROWS.map((r) => ({ side: r.side, info: relicSideRemaining(r.slot) })));
  const relicEquippedSides = $derived(relicSides.filter((s) => s.info !== null));

  // --- 5. シエナのオーラ: 部位ごとの段階(増幅段階 = 足したスロット数)を合算する。
  //    ここはタイル自体がキャラタブへ遷移するだけなので展開は持たない。 ---
  const sienaEquippedStages = $derived.by(() => {
    const c = character;
    if (!c) return [];
    return SIENA_ALLOWED_SLOTS
      .map((slot) => selectedSienaAura(c.equipment.siena[slot]))
      .filter((a): a is NonNullable<typeof a> => a !== null)
      .map((a) => sienaStage(a));
  });
  const sienaSummary = $derived.by(() => {
    const stages = sienaEquippedStages;
    if (stages.length === 0) return null;
    return { value: stages.reduce((s, v) => s + v, 0), max: stages.length * app.siena.stage_max };
  });
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
            <Icon kind="character" id={character.game_character_id} size={64} label={character.name} source={app.characterIcons[character.id] ?? null} />
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
            <!-- 目標を選び直すとスポットライトのスキルが変わり、見る装備値の 2 本(突き/斬り/魔攻…)も
                 入れ替わる。中身が入れ替わった面は短く動かす(§10 型 3b。数値の跳ねでは表せない) -->
            <div class="hero-panel" use:swap={() => heroEquipRows.map((r) => r.key).join(",")}>
              <span class="hero-panel-title">装備・命中</span>
              {#each heroEquipRows as row, i (row.key)}
                <span class="hero-row" class:first={i === 0}>
                  <span class="hero-row-label">{row.label}</span>
                  <span class="hero-row-value-wrap">
                    <span class="num hero-sub">{fmtInt(row.base)} +{fmtInt(row.enhanced)}</span>
                    <span class="num hero-row-value" use:bump={() => row.total}>{fmtInt(row.total)}</span>
                  </span>
                </span>
              {/each}
              {#if heroEquipRows.length === 0}
                <span class="hero-row first">
                  <span class="hero-row-label">装備</span>
                  <span class="num hero-row-value">—</span>
                </span>
              {/if}
              <span class="hero-row">
                <span class="hero-row-label">命中P</span>
                {#if heroAccuracy !== null}
                  <span class="num hero-row-value" use:bump={() => heroAccuracy}>{fmtInt(heroAccuracy)}</span>
                {:else}
                  <span class="hero-row-value-wrap">
                    <span class="coverage" title="命中Pを算出できません: {heroAccuracyReason}">{heroAccuracyReason}</span>
                  </span>
                {/if}
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
          {:else}
            <!-- 目標はふだん自動で決まる。ただし「クリアできる」と「周回したい」は別なので、
                 ここは自動値を上書きする例外操作(ux-guidelines 原則 4)。候補は重なって出るので
                 押した場所も右の数値も動かない(§00 03) -->
            <div
              class="hero-goal-pick" class:manual={manualGoal !== null}
              title={manualGoal
                ? "自動判定ではなく、自分で選んだ目標です(保存されます)。先頭の「自動: …」を選ぶと自動に戻ります"
                : "自動で選ばれている目標です。押すと自分の目標に差し替えられます"}
              use:flash={() => heroGoal?.content.id ?? ""}
            >
              <Picker
                options={goalOptions}
                note="自動で選ばれる目標を、自分の目標に差し替える"
                bind:value={
                  () => character?.goal_content_id ?? "",
                  (v) => { if (character) commitGoal(character, v === "" ? null : v); }
                }
              />
            </div>
            {#if goalStale}
              <span class="coverage" title="選んでいた目標が今のデータにありません。自動で選んだ目標を出しています">選んだ目標が見つかりません</span>
            {/if}
            {#if !heroGoal}
              <span class="hero-goal-note dim">全 {fmtInt(totalCount)} コンテンツ クリア可 — 目標を選ぶとここで詰められます</span>
            {:else if heroGoal.content.need_per_hit === null || !heroSpot}
              <span class="hero-goal-note dim">{noteOf(heroGoal).text}</span>
            {:else}
              <span class="hero-div"></span>
              <Icon
                kind="skill" id={heroSpot.skillId} size={28}
                label={skillNames[heroSpot.skillId] ?? heroSpot.skillId}
              />
              <span class="hero-goal-skill">{skillNames[heroSpot.skillId] ?? heroSpot.skillId}</span>
              {#if heroDamage}
                {@const stage = critChanceStage(heroDamage.critChance * 100)}
                {#key stage.label}
                  <span class="badge" style={badgeStyle({ label: "", state: heroDamage.critRate === null ? "unknown" : stage.state })} use:flash={() => stage.label}>
                    {heroDamage.critRate === null ? "クリ 確定扱い" : `クリ${stage.label} ${heroDamage.critRate.toFixed(1)}%`}
                  </span>
                {/key}
              {/if}
              <span class="meter hero-meter">
                <span class="fill" style="width: {heroSpotPct}; background: {STATE[BADGE[heroSpotState].state].bar};"></span>
              </span>
              <span class="hero-spot-wrap">
                <span class="num hero-spot" use:bump={() => heroSpot?.perHit ?? null} title="表記ダメージ(スキル分のみ。武器強化の追加固定ダメージは含まない)">
                  {fmtInt(heroSpot.perHit)}
                </span>
                <!-- 目標を選び直すと必要値も変わる。変わったものは全部動かす(§00 04) -->
                <span class="num dim" use:bump={() => heroGoal?.content.need_per_hit ?? null}> / {fmtInt(heroGoal.content.need_per_hit)}</span>
              </span>
              {#key heroSpotState}
                <span class="badge" style={badgeStyle(BADGE[heroSpotState])} use:flash={() => String(heroSpotState)}>
                  {BADGE[heroSpotState].label}
                </span>
              {/key}
            {/if}
            {#if heroGoal}
              <!-- この行は目標名 → 火力 → 到達バッジの 1 本の視線で読ませる。文字の CTA を末尾に置くと
                   その幅ぶん量バーとスキル名が痩せるので、掘り下げの入口は一覧行と同じ「›」に寄せる -->
              <button
                type="button" class="cta chev-only" title="計算タブで詰める"
                aria-label="計算タブで詰める" onclick={tryHeroGoalInCalc}
              >›</button>
            {/if}
          {/if}
        </div>

        <!-- 足りないものに合わせて出す: 入場条件未達なら条件の行、火力未達ならおすすめ強化。
             火力が既に届いているのに「届かせるなら +0%」を並べない(§00 考えさせない) -->
        {#if heroEntryUnmet}
          <div class="hero-advice">
            <span class="hero-advice-title">あとは入場条件 — 満たすなら</span>
            <button type="button" class="hero-advice-row" onclick={() => (app.tab = "chars")}>
              <span class="hero-advice-label">入場まで: {heroGoal?.ev ? unmetText(heroGoal.ev) : ""}</span>
              <span class="chev dim">›</span>
            </button>
          </div>
        {/if}
        {#if heroAdvice.length > 0 && heroPowerShort}
          <div class="hero-advice">
            <span class="hero-advice-title">おすすめ強化 — 届かせるなら</span>
            <div class="hero-advice-list">
              {#each heroAdvice as a, i (a.id)}
                <button type="button" class="hero-advice-row" onclick={() => applyHeroAdvice(a)}>
                  <span class="rank num">{i + 1}</span>
                  <span class="cost" style={triadStyle(COST_COLORS[a.cost])}>{COST_LABELS[a.cost]}</span>
                  <span class="hero-advice-label">{a.label}</span>
                  <!-- 伸び率は表記ダメージと合計ダメージの 2 本。シャープネスビジョンのように
                       表記が動かず合計だけ伸びる候補があるので、片方だけだと「効いていない」と
                       読めてしまう(ユーザー判断 2026-09-01) -->
                  <span class="hero-advice-nums">
                    <span class="num" use:bump={() => a.per_hit_primary} title="表記ダメージ(スキル分のみ)">{fmtInt(a.per_hit_primary)}</span>
                    <span class="num advice-delta" use:bump={() => a.delta_pct} title="表記ダメージの伸び率">{deltaText(a.delta_pct)}</span>
                    <span class="num advice-total dim" use:bump={() => a.delta_total_pct} title="実際に敵へ入る合計ダメージの伸び率(武器強化の追加固定・割合追加を含む)">合計 {deltaText(a.delta_total_pct)}</span>
                  </span>
                  {#if a.reaches}
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

      <!-- ===== 今日の強化: 5 項目タイル。押すと**グリッド全体の下**に展開する(押した場所は動かない・
           同時に開くのは 1 つだけ)。武器・鎧の強化Lvは低頻度なので「そのほかの設定」からキャラタブへ ===== -->
      <div class="section">
        <div class="area-head">
          <span class="area-name">今日の強化</span>
          <span class="area-rule"></span>
          {#if character.updated_at}
            <span class="last-enhance dim">
              最後の強化 <span class="num">{fmtMonthDay(character.updated_at)}</span>
              ({daysAgo(character.updated_at) === 0 ? "今日" : `${daysAgo(character.updated_at)} 日前`})
            </span>
          {/if}
        </div>
        <div class="today-grid">
          <button type="button" class="today-tile" class:open={openTile === "sacredRelic"} onclick={() => toggleTile("sacredRelic")}>
            <div class="today-tile-head">
              <span class="today-tile-name">神鳥の聖物</span>
              {#if !sacredRelicSet}
                <span class="badge" style={badgeStyle({ label: "未設定", state: "edge" })}>未設定</span>
              {:else if sacredRelicRemaining <= 0}
                <span class="badge" style={badgeStyle({ label: "上限まで到達", state: "met" })}>上限まで到達</span>
              {/if}
            </div>
            {#if sacredRelicSet && sacredRelicRemaining > 0}
              <span class="today-tile-note num" use:flash={() => String(sacredRelicRemaining)}>残り {fmtInt(sacredRelicRemaining)}</span>
            {/if}
          </button>
          <button type="button" class="today-tile" class:open={openTile === "cuffs"} onclick={() => toggleTile("cuffs")}>
            <div class="today-tile-head">
              <span class="today-tile-name">カフス</span>
              {#if !cuffsSummary}
                <span class="badge" style={badgeStyle({ label: cuffsUnsetLabel ?? "未設定", state: "edge" })}>{cuffsUnsetLabel ?? "未設定"}</span>
              {:else if cuffsRemaining !== null && cuffsRemaining <= 0}
                <span class="badge" style={badgeStyle({ label: "上限まで到達", state: "met" })}>上限まで到達</span>
              {/if}
            </div>
            {#if cuffsRemaining !== null && cuffsRemaining > 0}
              <span class="today-tile-note num" use:flash={() => String(cuffsRemaining)}>この段階 残り {fmtInt(cuffsRemaining)}</span>
            {/if}
          </button>
          <button type="button" class="today-tile" class:open={openTile === "enchant"} onclick={() => toggleTile("enchant")}>
            <div class="today-tile-head">
              <span class="today-tile-name">エンチャント</span>
              {#if !enchantSummary}
                <span class="badge" style={badgeStyle({ label: "対象なし", state: "edge" })}>対象なし</span>
              {:else if enchantRemaining.remain <= 0}
                <span class="badge" style={badgeStyle({ label: "上限まで到達", state: "met" })}>上限まで到達</span>
              {/if}
            </div>
            {#if enchantSummary && enchantRemaining.remain > 0}
              <span class="today-tile-note num" use:flash={() => String(enchantRemaining.remain)}>
                {fmtInt(enchantRemaining.parts)}部位 残り {fmtInt(enchantRemaining.remain)}
              </span>
            {/if}
          </button>
          <button type="button" class="today-tile" class:open={openTile === "equipRelic"} onclick={() => toggleTile("equipRelic")}>
            <div class="today-tile-head">
              <span class="today-tile-name">レリック</span>
              {#if relicEquippedSides.length === 0}
                <span class="badge" style={badgeStyle({ label: "未設定", state: "edge" })}>未設定</span>
              {:else if relicEquippedSides.every((s) => s.info!.done)}
                <span class="badge" style={badgeStyle({ label: "上限まで到達", state: "met" })}>上限まで到達</span>
              {/if}
            </div>
            {#if relicEquippedSides.length > 0 && !relicEquippedSides.every((s) => s.info!.done)}
              <span class="today-tile-note" use:flash={() => relicEquippedSides.map((s) => s.info!.text).join()}>
                {relicEquippedSides.map((s) => `${s.side} ${s.info!.done ? "上限" : s.info!.text}`).join(" ・ ")}
              </span>
            {/if}
          </button>
          <button
            type="button" class="today-tile today-tile-nav" onclick={() => focusCharacterSource("siena")}
            title="キャラタブへ移動して編集します"
          >
            <div class="today-tile-head">
              <span class="today-tile-name">シエナのオーラ</span>
              {#if !sienaSummary}
                <span class="badge" style={badgeStyle({ label: "未設定", state: "edge" })}>未設定</span>
              {:else if sienaSummary.max - sienaSummary.value <= 0}
                <span class="badge" style={badgeStyle({ label: "上限まで到達", state: "met" })}>上限まで到達</span>
              {/if}
              <span class="chev dim" aria-hidden="true">↗</span>
            </div>
            {#if sienaSummary && sienaSummary.max - sienaSummary.value > 0}
              {@const remain = sienaSummary.max - sienaSummary.value}
              <span class="today-tile-note num" use:flash={() => String(remain)}>増幅 残り {fmtInt(remain)} 段</span>
            {/if}
          </button>
        </div>

        {#if openTile}
          <div class="today-expand">
            {#if openTile === "sacredRelic"}
              <div class="expand-head">
                <span class="expand-title">神鳥の聖物</span>
                <span class="dim expand-note">ステごとの加算({limits.sacred_relic_value_per_stage} きざみ・0–{SACRED_RELIC_MAX_VALUE})</span>
              </div>
              <div class="expand-rows two-col">
                {#each STAT_KINDS as k (k)}
                  <div class="expand-row" use:flash={() => String(sacredRelicValueOf(character, k))}>
                    <span class="expand-row-label">{STAT_LABELS[k]}</span>
                    <StatInput
                      label="{STAT_LABELS[k]}の聖物" hideLabel
                      min={0} max={SACRED_RELIC_MAX_VALUE} step={limits.sacred_relic_value_per_stage} stepper
                      bind:value={
                        () => sacredRelicValueOf(character, k),
                        (v) => commitSacredRelic(character, k, v)
                      }
                    />
                  </div>
                {/each}
              </div>
            {:else if openTile === "cuffs"}
              <div class="expand-head">
                <span class="expand-title">カフス</span>
                <span class="dim expand-note">この段階の実値。下限は直前段階の完成値、上限はこの段階のMAX</span>
              </div>
              {#if cuffsGrowthKeys.length === 0}
                <button type="button" class="expand-nav" onclick={() => focusCharacterSource("equipment", "shield_plus")}>
                  <span class="badge" style={badgeStyle({ label: cuffsUnsetLabel ?? "未設定", state: "edge" })}>{cuffsUnsetLabel ?? "未設定"}</span>
                  <span class="expand-nav-text">キャラタブで装備を選ぶ</span>
                  <span class="chev dim">›</span>
                </button>
              {:else}
                {@const item = cuffsItem}
                <div class="expand-rows">
                  {#each cuffsGrowthKeys as k (k)}
                    <div class="expand-row" use:flash={() => String(cuffsPart(character)?.base[k] ?? 0)}>
                      <span class="expand-row-label">{EQUIPMENT_STAT_LABELS[k]}</span>
                      <StatInput
                        label="{EQUIPMENT_STAT_LABELS[k]}の装備補正" hideLabel
                        min={item!.values_min[k]} max={item!.growth_caps![k]} strictMax stepper
                        bind:value={
                          () => cuffsPart(character)?.base[k] ?? 0,
                          (v) => commitCuffsBase(character, k, v)
                        }
                      />
                    </div>
                  {/each}
                </div>
              {/if}
            {:else if openTile === "enchant"}
              <div class="expand-head">
                <span class="expand-title">エンチャント</span>
                <span class="dim expand-note">主軸: {enchantDepKeys.map((k) => EQUIP_ROW_LABELS[k]).join("・")}</span>
                <button type="button" class="cta expand-more" onclick={() => (app.tab = "chars")}>ほかのステはキャラタブへ ›</button>
              </div>
              {#if enchantRows.length === 0}
                <p class="dim expand-empty">主軸スキルの依存ステを盛れる部位が装備されていません。</p>
              {:else}
                <div class="expand-rows">
                  {#each enchantRows as row (row.slot)}
                    <div class="expand-row enchant-row">
                      <span class="expand-row-label">{ENCHANT_SLOT_LABELS[row.slot]}</span>
                      {#if row.capUnknown}
                        <button type="button" class="expand-nav" onclick={() => focusCharacterSource("equipment", row.slot)}>
                          <span class="coverage" title="カタログ外(カスタム名)装備でエンチャント上限が未入力です">上限未入力</span>
                          <span class="chev dim">›</span>
                        </button>
                      {:else}
                      <div class="enchant-row-cols">
                        {#each enchantDepKeys as k (k)}
                          {@const cap = enchantCap(row.part, k)}
                          {#if cap !== null && cap > 0}
                            {@const cur = partOf(row.slot)?.enchant[k] ?? 0}
                            <div class="enchant-stat">
                              <span class="enchant-stat-label">{EQUIPMENT_STAT_SHORT[k]}</span>
                              <span class="enchant-stat-val">
                                {#if editingEnchant === `${row.slot}:${k}`}
                                  <input
                                    class="enchant-stat-input num" type="number" min="0" max={cap} value={cur}
                                    onblur={(e) => commitEnchantText(row.slot, k, cap, e.currentTarget.value)}
                                    onkeydown={(e) => {
                                      if (e.key === "Enter") e.currentTarget.blur();
                                      if (e.key === "Escape") editingEnchant = null;
                                    }}
                                    {@attach (node) => { node.focus(); node.select(); }}
                                  />
                                {:else}
                                  <button
                                    type="button" class="num enchant-stat-num" use:bump={() => cur}
                                    aria-label="{EQUIPMENT_STAT_SHORT[k]}のエンチャントを編集"
                                    onclick={() => (editingEnchant = `${row.slot}:${k}`)}
                                  >{fmtInt(cur)}</button>
                                {/if}
                                <span class="num dim">/{fmtInt(cap)}</span>
                              </span>
                              <span class="enchant-stat-chips">
                                {#each ENCHANT_INCREMENTS as amt (amt)}
                                  <button
                                    type="button" class="chip-add" disabled={cur >= cap}
                                    onclick={() => commitEnchant(character, row.slot, k, Math.min(cap, cur + amt))}
                                  >+{amt}</button>
                                {/each}
                                <button
                                  type="button" class="chip-add chip-max" disabled={cur >= cap}
                                  onclick={() => commitEnchant(character, row.slot, k, cap)}
                                >MAX</button>
                                <button
                                  type="button" class="chip-add chip-zero" disabled={cur <= 0}
                                  onclick={() => commitEnchant(character, row.slot, k, 0)}
                                >0</button>
                              </span>
                            </div>
                          {/if}
                        {/each}
                      </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            {:else if openTile === "equipRelic"}
              <div class="expand-head"><span class="expand-title">レリック</span></div>
              <div class="expand-rows">
                {#each RELIC_ROWS as r (r.slot)}
                  {@const part = partOf(r.slot)}
                  {@const rs = relicState(r.slot, part)}
                  {@const item = itemOf(part)}
                  {@const growthKeys = relicGrowthKeys(item)}
                  <div class="expand-row relic-row">
                    <span class="expand-row-label">レリック{r.side}</span>
                    {#if rs}
                      <div class="relic-row-body">
                        <div class="relic-row-head">
                          <span class="badge" style={badgeStyle({ label: RELIC_KIND_LABELS[rs.kind], state: "unknown" })}>{RELIC_KIND_LABELS[rs.kind]}</span>
                          <div class="today-stepper">
                            <button type="button" class="dst" aria-label="レリック{r.side}を下げる" disabled={!rs.canDown} onclick={() => stepRelicLevel(r.slot, -1)}>−</button>
                            <span class="today-stepper-val">
                              <span class="num" use:bump={() => rs!.value}>Lv{rs.value}</span>
                              <span class="num dim">/ {rs.max}</span>
                            </span>
                            <button type="button" class="dst" aria-label="レリック{r.side}を上げる" disabled={!rs.canUp} onclick={() => stepRelicLevel(r.slot, 1)}>+</button>
                          </div>
                        </div>
                        {#if growthKeys.length > 0}
                          <div class="relic-growth-rows">
                            {#each growthKeys as k (k)}
                              <div class="enchant-stat" use:flash={() => String(partOf(r.slot)?.base[k] ?? 0)}>
                                <span class="enchant-stat-label">{EQUIPMENT_STAT_SHORT[k]}</span>
                                <StatInput
                                  label="{EQUIPMENT_STAT_SHORT[k]}の補正値" hideLabel
                                  min={item!.values_min[k]} max={item!.growth_caps![k]} strictMax stepper
                                  bind:value={
                                    () => partOf(r.slot)?.base[k] ?? 0,
                                    (v) => commitRelicBase(character, r.slot, k, v)
                                  }
                                />
                              </div>
                            {/each}
                          </div>
                          {#if !rs.growthDone}
                            <p class="relic-hint dim">補正値が上限まで届くと次の段へ進めます</p>
                          {/if}
                        {:else}
                          <span class="expand-row-vals num dim">{valuesSummary(part!.base)}</span>
                        {/if}
                      </div>
                    {:else}
                      <button type="button" class="expand-nav" onclick={() => focusCharacterSource("equipment", r.slot)}>
                        <span class="badge" style={badgeStyle({ label: "未設定", state: "edge" })}>未設定</span>
                        <span class="chev dim">›</span>
                      </button>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}

        <button type="button" class="cta tile-more" onclick={() => (app.tab = "chars")}>そのほかの設定(武器・鎧の強化・ペット・ルーン・バフ) ›</button>
      </div>

      <!-- ===== どこまでいける?: 畳み既定。エリア 4 行 → 押すと直下に一覧が展開(§09 規則 1) ===== -->
      <details class="fold reach-fold">
        <summary>
          <span class="area-name">どこまでいける?</span>
          <span class="fold-count">
            クリア済み <span class="num" use:bump={() => clearedCount}>{fmtInt(clearedCount)}</span>
            <span class="dim">/ {fmtInt(totalCount)}</span>
          </span>
          {#if uncoveredCount > 0}
            <span class="fold-note dim">未収録 <span class="num">{fmtInt(uncoveredCount)}</span></span>
          {/if}
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
                          <!-- コンテンツの絵(ゲーム内のコンテンツ一覧と同じ絵)。行頭の枠は
                               サイズ固定なので、未収録のコンテンツが混ざっても行の高さは動かない -->
                          <Icon kind="content" id={r.content.id} size={28} label={r.content.name} />
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
                          <span class="dmg num" use:bump={() => r.ev?.damage?.per_hit_primary ?? null} title="表記ダメージ(スキル分のみ)">{r.ev?.damage ? fmtInt(r.ev.damage.per_hit_primary) : "—"}</span>
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

  /* 帯ラベル・行動チップ。影響カードのカード種別に使う */
  .tag {
    flex: none; width: 52px; text-align: center; padding: 1px 0; border-radius: var(--r-pill);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted); white-space: nowrap;
  }
  .cta {
    flex: none; display: inline-flex; align-items: center; gap: 5px; padding: 4px 12px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border-soft); font-size: 9.5px; font-weight: 700; color: var(--accent); white-space: nowrap;
  }
  .cta:hover { border-color: var(--accent); }
  .cta.chev-only { padding: 4px 9px; font-size: 11px; }

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
  /* 目標の選択。自動値を上書きする例外操作なので、白い面(編集できる面)で出す。
     幅は固定 — 目標名が長短しても右のメーター・数値の位置が動かない(§00 03) */
  /* 目標名が長いときはピッカーのほうを縮めて、量バーを残す(バーは一目で分かる唯一の要素) */
  .hero-goal-pick { flex: 0 1 190px; min-width: 118px; }
  /* 候補はコンテンツ名が長いのでトリガより広く出す。重なるので周りの行は押さない */
  .hero-goal-pick :global(.picker-pop) { right: auto; min-width: 340px; }
  /* エリア名は候補の中でだけ出す。トリガに置くと目標名を押しのけ、右のメーターまで痩せる
     (この行は目標名 → 火力 → 到達バッジの 1 本の視線で読ませたい。§00 01) */
  .hero-goal-pick :global(.picker-trigger .picker-meta) { display: none; }
  /* 自動ではなく自分で選んでいる印。ピッカーの文字自体が「自動: …」かコンテンツ名かで
     どちらかを言っているので、隣に「自分で選択中」の札は足さない(§00 02。札のぶん量バーが痩せる)。
     **保存される**値であることは選択中の面を水色(--accent)で縁取って示す */
  .hero-goal-pick.manual :global(.picker-trigger) {
    border-color: var(--accent); box-shadow: 0 0 0 2px rgba(66, 109, 214, 0.16);
  }
  .hero-goal-note { min-width: 0; flex: 1; font-size: var(--t-label); }
  .hero-div { width: 1px; align-self: stretch; background: var(--border-soft); }
  /* スキル名は中身ぶんだけ(長い名前は 100px で省略)。ここを縮ませると
     アイコンだけが残って何のスキルか読めなくなる(§06 アイコン単独表示は禁止) */
  .hero-goal-skill { flex: none; max-width: 100px; font-size: 10px; font-weight: 700; color: var(--fg-sub); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* 量バーは「目安に対してどれだけ出ているか」を一目で見る唯一の要素。行の幅が足りなくなったら
     先に縮むのは目標名・スキル名のほうで、バーは縮ませない(shrink 0 + 基準幅) */
  .hero-meter { flex: 1 0 96px; height: 12px; }
  /* 桁が増えても右のバッジ・バーの位置が動かないよう、数値の場所は先に確保する(§00 03) */
  .hero-spot-wrap { flex: none; min-width: 132px; text-align: right; white-space: nowrap; }
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
  /* 伸び率は本体の数値より小さく。合計側はさらに控えめに置く(主役は表記ダメージ) */
  .hero-advice-nums .advice-delta, .hero-advice-nums .advice-total { font-size: 9px; margin-left: 5px; }
  .hero-advice-nums .advice-total { color: var(--fg-dim); }
  .hero-advice-row .chev { flex-shrink: 0; font-size: 9px; }
  .cost { flex-shrink: 0; padding: 1px 8px; border-radius: var(--r-pill); border: 1px solid; font-size: 9px; font-weight: 700; white-space: nowrap; }

  /* ===== 汎用セクション見出し(期限・影響 / 今日の強化 / どこまでいける?) ===== */
  .section { display: flex; flex-direction: column; gap: 6px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: var(--r-inset); background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }
  .last-enhance { flex: none; font-size: 9px; white-space: nowrap; }

  /* ===== 期限・影響カード ===== */
  .brief-card {
    display: flex; align-items: center; gap: 12px; padding: 12px 15px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 #fff, 0 1px 2px rgba(30, 44, 74, 0.08);
  }
  .brief-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .brief-title { font-size: 14px; font-weight: 800; color: var(--fg-head); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .brief-why { font-size: 10px; color: var(--fg-muted); line-height: 1.4; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* ===== 今日の強化: 5 項目タイル。閉じているときは要約(合計/上限)+ 内訳、押すと下に展開する ===== */
  .today-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
  .today-tile {
    display: flex; flex-direction: column; align-items: stretch; gap: 3px; min-width: 0; padding: 9px 12px;
    border-radius: var(--r-window); background: var(--bg-field); border: 1px solid var(--border-soft);
    box-shadow: inset 0 1px 0 #fff; text-align: left;
  }
  .today-tile:hover { border-color: var(--accent); }
  /* 開いているタイルだけ枠を強める(押した場所が分かる。§00 押した場所は動かない) */
  .today-tile.open { border-color: var(--accent); background: var(--bg-panel); }
  .today-tile-head { display: flex; align-items: center; gap: 7px; min-width: 0; }
  .today-tile-name { min-width: 0; flex: 1; font-size: 10.5px; font-weight: 700; color: var(--fg-sub); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* まだ伸ばせる余地(§00 04: 数値が変われば use:flash で気づかせる) */
  .today-tile-note { min-width: 0; font-size: 9.5px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .today-tile-head .chev { flex-shrink: 0; font-size: 9px; }
  /* シエナのオーラだけ展開ではなくキャラタブへ飛ぶ。外部遷移を示す ↗ を常設し、押す前に
     区別できるようにする(押した場所は動かない §00 に対する例外なので明示する) */
  .today-tile-nav:hover { border-color: var(--sim); }
  /* ===== 今日の強化: 展開(押したタイルの上には差し込まず、グリッド全体の下に出す) ===== */
  .today-expand {
    display: flex; flex-direction: column; gap: 8px; padding: 11px 13px; border-radius: var(--r-window);
    background: var(--bg-panel); border: 1px solid var(--border-soft);
  }
  .expand-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .expand-title { font-size: 10.5px; font-weight: 800; color: var(--fg-head); white-space: nowrap; }
  .expand-note { min-width: 0; flex: 1; font-size: 9.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .expand-more { flex: none; }
  .expand-empty { margin: 0; font-size: 10.5px; }
  .expand-rows { display: flex; flex-direction: column; gap: 5px; }
  .expand-rows.two-col { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px 12px; }
  .expand-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .expand-row-label { flex: none; width: 56px; font-size: 10px; font-weight: 700; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .enchant-row { align-items: flex-start; }
  .enchant-row-cols { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  /* エンチャント行: 短縮ステ名 + 値/上限 + 増分チップ2種(押した場所は動かない・幅は詰める) */
  .enchant-stat { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .enchant-stat-label { flex: none; width: 26px; font-size: 9px; font-weight: 700; color: var(--fg-muted); white-space: nowrap; }
  .enchant-stat-val { flex: none; min-width: 62px; display: flex; align-items: baseline; gap: 2px; }
  .enchant-stat-val .num { font-size: 11.5px; font-weight: 800; font-variant-numeric: tabular-nums; color: var(--fg-head); }
  .enchant-stat-val .num.dim { font-size: 9px; font-weight: 700; }
  /* 押すとテキスト編集にできる(戻す手段が増分チップだけだと足りないため。§07 形態5の例外運用) */
  .enchant-stat-num { padding: 0; background: none; border: none; cursor: text; }
  .enchant-stat-num:hover { text-decoration: underline dotted; }
  .enchant-stat-input {
    width: 44px; padding: 1px 3px; border-radius: var(--r-inset); background: var(--bg-field);
    border: 1px solid var(--accent); font-size: 11.5px; font-weight: 800; font-variant-numeric: tabular-nums;
  }
  .enchant-stat-chips { flex: none; display: flex; flex-wrap: wrap; gap: 3px; max-width: 190px; }
  .chip-add {
    flex: none; padding: 2px 7px; border-radius: var(--r-inset); background: var(--state-goal-bg);
    border: 1px solid var(--cell-bd); color: var(--accent-hover); font-size: 8.5px; font-weight: 700; font-variant-numeric: tabular-nums;
  }
  .chip-add:hover:not(:disabled) { border-color: var(--accent); }
  .chip-add:disabled { color: var(--border); background: none; }
  .chip-max { border-style: dashed; }
  .chip-zero { color: var(--fg-muted); } /* 一発で0へ戻す取り消しチップ */
  .expand-row-vals { flex: none; max-width: 130px; font-size: 9.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .expand-nav { display: flex; align-items: center; gap: 8px; padding: 2px 0; text-align: left; }
  .expand-nav-text { flex: 1; min-width: 0; font-size: 10px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .relic-row { align-items: flex-start; }
  .relic-row-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 5px; }
  .relic-row-head { display: flex; align-items: center; gap: 8px; }
  .relic-growth-rows { display: flex; flex-direction: column; gap: 4px; }
  .relic-hint { margin: 0; font-size: 9px; color: var(--fg-muted); }

  /* レリック左右の −/値/+(既存の直更新タイルと同じ見た目を踏襲) */
  .today-stepper { flex: none; display: flex; align-items: center; gap: 4px; } /* 押しやすさ優先で −/値/+ を近接させる(離れすぎ対策) */
  .dst {
    width: 24px; height: 24px; flex: none; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); background: var(--state-goal-bg); border: 1px solid var(--cell-bd);
    color: var(--accent-hover); font-size: 12px; font-weight: 700;
  }
  .dst:hover:not(:disabled) { border-color: var(--accent); }
  .dst:disabled { color: var(--border); background: none; } /* 上限・下限(§00 考えさせない: 押せないボタンで示す) */
  /* min-width で桁が増減しても幅を変えない(§00 押した場所は動かない) */
  .today-stepper-val { flex: none; min-width: 46px; display: flex; align-items: baseline; justify-content: center; gap: 2px; }

  .tile-more { align-self: flex-start; margin-top: 2px; }
  .tile-more { align-self: flex-start; margin-top: 2px; }

  /* ===== どこまでいける?(details.fold は app.css 側の畳み見た目を継承) ===== */
  .reach-fold summary { display: flex; align-items: center; gap: 9px; }
  .fold-count { font-size: 10.5px; font-weight: 700; color: var(--fg-sub); }
  /* 未収録数は見出しの主役(クリア済み)より控えめに(§00 02: いま要らないものは弱める) */
  .fold-note { margin-left: auto; font-size: 9.5px; }

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

  .foot { margin: 0; font-size: 10px; line-height: 1.7; }
</style>
