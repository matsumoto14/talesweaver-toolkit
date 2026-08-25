<script lang="ts">
  // 選択キャラのワークスペース(v4): 左に補正源リスト、右に選択した補正源の編集ペイン、
  // 下に「いまの実力」シート。draft(編集状態)を 1 つの $state に持ち、保存はキャラ単位で 1 ボタン。
  // 親 CharsPage が {#key character.id} で作り直す前提($effect による再同期は書かない)。
  import { untrack } from "svelte";
  import { errorMessage, previewEffectiveStats, updateCharacter } from "../../api/commands";
  import type { CommonSkills, Equipment, RegisteredCharacter, StatPreview, StatSources } from "../../api/types";
  import { deleteCharacter } from "../../api/commands";
  import { buildDraft, draftToPayload, type Draft } from "../../draft";
  import {
    equipmentBaseTotal, equipmentElementValues, equipmentEnchantTotal, randomOptionCount,
    randomOptionRecordOnlyCount, sienaAttackRatePercent,
    sienaPartCount, sienaStatTotal, thesisCoresBestTotal,
  } from "../../equipment";
  import { fmtInt, fmtNum } from "../../format";
  import {
    ELEMENT_LABELS, ELEMENTS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT, STAT_KINDS, STAT_LABELS,
    ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import { app, loadSkills, removeCharacter, skillsByCharacter, upsertCharacter } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import SourcePane, { type SourceId } from "./SourcePane.svelte";
  import { bump } from "../../ui/motion.svelte";

  interface Props {
    character: RegisteredCharacter;
  }
  let { character }: Props = $props();


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
    // 浅いコピーだとネストした値(アンリーシュの枠・極限スキル)の変更を $effect が追跡できず、
    // 触っても再計算が走らない。装備と同じく deep copy で全プロパティを読む
    const commonSkills = JSON.parse(JSON.stringify(draft.commonSkills)) as CommonSkills;
    // 最終能力値の上限は覚醒段階 + エタの意志 Lv で決まるので、覚醒もプレビューの入力に含める
    const awakening = { stage: Number(draft.stage), eternal_level: Number(draft.eternalLevel) };
    const mainSkillId = draft.mainSkillId === "" ? null : draft.mainSkillId;
    const gameCharacterId = draft.gameCharacterId;
    if (debounceHandle) clearTimeout(debounceHandle);
    const seq = ++previewSeq;
    debounceHandle = setTimeout(() => {
      previewEffectiveStats(baseStats, statSources, equipment, commonSkills, awakening, gameCharacterId, mainSkillId)
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
  const monsterCardTotal = $derived(
    STAT_KINDS.reduce((s, k) => s + draft.statSources.monster_cards[k], 0),
  );
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
    (draft.commonSkills.power_weapon ? 2 : 0) + draft.commonSkills.strong_weapon_level * 3,
  );
  const eqBaseTotal = $derived(equipmentBaseTotal(draft.equipment, app.equipmentAbilities, app.titles));
  const eqEnchantTotal = $derived(equipmentEnchantTotal(draft.equipment));
  const sienaParts = $derived(sienaPartCount(draft.equipment));
  const sienaRate = $derived(sienaAttackRatePercent(draft.equipment));
  const sienaStats = $derived(sienaStatTotal(draft.equipment));
  const coreBestTotal = $derived(thesisCoresBestTotal(draft.equipment.thesis_cores));
  const roCount = $derived(randomOptionCount(draft.equipment));
  const roRecordOnly = $derived(randomOptionRecordOnlyCount(draft.equipment, app.randomOptions));
  const NEUTRAL = "未設定(中立値で計算)";

  // 中ディレイ減少(wiki: ステータス「中ディレイ倍率B」)。ここはキャラ固有のパッシブだけ。
  // 共通の供給源(フルスロットル / カフスの RO / シエナのオーラ)はそれぞれの補正源で設定する。
  const delaySummary = $derived.by(() => {
    const ids = draft.statSources.actual_delay_skills.skill_ids;
    if (ids.length === 0) return NEUTRAL;
    const percent = ids.reduce(
      (n, id) => n + (app.actualDelaySkills.find((d) => d.id === id)?.percent ?? 0),
      0,
    );
    return `${ids.length} 件 ・ 合計 −${percent}%`;
  });

  // 共通スキル(wiki: Skill/共通)。効き先ごとに 1 行でまとめる
  const commonSkillSummary = $derived.by(() => {
    const c = draft.commonSkills;
    const parts: string[] = [];
    if (enhanceRatePercent > 0) parts.push(`装備攻撃力 +${enhanceRatePercent}%`);
    const pa = c.protect_armor_level;
    const defense =
      (c.coat_armor ? 18 : 0) + (pa > 0 ? [36, 45, 54, 63, 72, 81][pa - 1] : 0) + c.kai_protect_armor_level * 9;
    if (defense > 0) parts.push(`装備防御力 物+${defense}%`);
    if (c.sharpness_vision_level > 0) {
      parts.push(`追加ダメージ +${[5, 10, 15, 20, 25, 28, 31, 34, 37, 40][c.sharpness_vision_level - 1]}%`);
    }
    const ultimate = c.ultimate.slots.filter((u) => u !== null);
    if (ultimate.length > 0) {
      parts.push(`極限 ${ultimate.map((u) => ULTIMATE_SKILL_LABELS[u]).join(" / ")}`);
    }
    // アンリーシュ(能力解放)。効き先は能力値倍率B
    const unleash = (c.unleash ?? []).filter((u) => u.stat !== null && u.level > 0);
    if (unleash.length > 0) {
      const rates = [1, 2, 3, 4, 5, 8, 11, 14, 17, 20];
      parts.push(
        `解放 ${unleash.map((u) => `${STAT_LABELS[u.stat!]} +${rates[u.level - 1]}%`).join(" / ")}`,
      );
    }
    return parts.length === 0 ? NEUTRAL : parts.join(" ・ ");
  });

  // 称号は 1 枠。表示中の 1 件だけが効く(wiki: 称号システム)
  const titleSummary = $derived.by(() => {
    const t = app.titles.find((x) => x.id === draft.equipment.title);
    if (!t) return NEUTRAL;
    const total = EQUIPMENT_STAT_KINDS.reduce((n, k) => n + t.values[k], 0);
    return `${t.name}(合計 +${fmtInt(total)})`;
  });

  // 装備の属性強化 + 装備以外の供給源。0 の属性は出さない(全部 0 なら未設定扱い)
  const elementSummary = $derived.by(() => {
    const values = equipmentElementValues(draft.equipment);
    for (const def of app.elementSources) {
      const element = draft.statSources.elements[def.id];
      if (element) values[element] += def.value;
    }
    const parts = ELEMENTS.filter((e) => values[e] > 0).map((e) => `${ELEMENT_LABELS[e]}${values[e]}`);
    return parts.length === 0 ? NEUTRAL : parts.join(" / ");
  });

  // クリティカル率(wiki: 計算式まとめ #CriticalChance)。ここはペット会心と増加だけ。
  // 装備クリティカル補正・AGI・スキルの Cri値は登録済みのデータから自動で入る。
  const criticalRateSummary = $derived.by(() => {
    const c = draft.statSources.critical_rate;
    const parts: string[] = [];
    if (c.pet) parts.push("ペット会心 ×1.1");
    const bonus = (c.ultimate_rune ? 20 : 0) + (c.architect_lab ? 30 : 0) + (c.deadly_blow ? 100 : 0);
    if (bonus > 0) parts.push(`増加 +${Math.min(100, bonus)}%`);
    return parts.length === 0 ? NEUTRAL : parts.join(" ・ ");
  });

  const sources = $derived<{ id: SourceId; name: string; sub: string }[]>([
    { id: "status", name: "キャラステータス", sub: `覚醒 ${draft.stage} 段階 ・ エタの意志 Lv${draft.eternalLevel}` },
    {
      id: "commonSkill",
      name: "共通スキル",
      sub: commonSkillSummary,
    },
    {
      id: "equipment",
      name: "装備",
      sub: `基本合計 突${fmtInt(eqBaseTotal.thrust)} / 斬${fmtInt(eqBaseTotal.slash)}`,
    },
    {
      id: "element",
      name: "属性",
      sub: elementSummary,
    },
    {
      id: "title",
      name: "称号",
      sub: titleSummary,
    },
    {
      id: "randomOption",
      name: "ランダムOP",
      sub:
        roCount > 0
          ? `${roCount} 枠${roRecordOnly > 0 ? ` ・ うち ${roRecordOnly} 枠は記録のみ` : ""}`
          : NEUTRAL,
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
    {
      id: "monsterCard",
      name: "モンスターカード",
      sub: monsterCardTotal > 0 ? `合計 +${fmtInt(monsterCardTotal)}` : NEUTRAL,
    },
    { id: "skills", name: "キャラスキル", sub: skillCount > 0 ? `${skillCount} 件選択` : NEUTRAL },
    { id: "actualDelay", name: "中ディレイ減少", sub: delaySummary },
    { id: "criticalRate", name: "クリティカル率", sub: criticalRateSummary },
    { id: "pet", name: "ペット S スキル", sub: petCount > 0 ? `${petCount} 種` : NEUTRAL },
    { id: "rune", name: "ルーンスキル", sub: runeTotal > 0 ? `合計 +${fmtInt(runeTotal)}` : NEUTRAL },
    { id: "adjust", name: "調整", sub: adjustCount > 0 ? `${adjustCount} ステに適用` : NEUTRAL },
  ]);
  // 補正源の並びはプレイヤーが決める(design-system §14 決定 3)。
  //
  // 全部を同じ行の重さで並べるのは、設計をしていないのと同じ。ただし「どれをよく触るか」は
  // 人によって違うので、こちらで頻度を決め打ちしない。**お気に入り(★)で重さを分け、
  // 並びはドラッグで動かす**。§09 規則 5「並びは、ユーザーが頼まない限り変わらない」の
  // 裏返しで、頼まれたら変わってよい。設定画面は作らない — リストの上で直接動かす。
  //
  // ★ はホームタブのコンテンツと同じ操作なので、覚えることが増えない。
  const DEFAULT_ORDER: SourceId[] = [
    "status", "skills", "equipment", "commonSkill", "thesis", "siena", "relic",
    "crown", "monsterCard", "pet", "rune", "actualDelay", "criticalRate",
    "element", "title", "randomOption", "adjust",
  ];
  interface SourceLayout {
    /** お気に入り。上に置いて常に開く */
    fav: string[];
    /** そのほか */
    rest: string[];
  }
  const layout = persisted("chars.sourceLayout", { fav: [], rest: [...DEFAULT_ORDER] } as SourceLayout);
  /** 保存済みの並びに無い補正源(あとから増えたもの)は「そのほか」の既定位置に戻す */
  const ordered = $derived.by<SourceLayout>(() => {
    const known = new Set<string>(sources.map((s) => s.id));
    const fav = (layout.value.fav ?? []).filter((id) => known.has(id));
    const seen = new Set(fav);
    const rest = (layout.value.rest ?? []).filter((id) => known.has(id) && !seen.has(id));
    rest.forEach((id) => seen.add(id));
    for (const id of DEFAULT_ORDER) if (known.has(id) && !seen.has(id)) rest.push(id);
    return { fav, rest };
  });
  const itemsOf = (ids: string[]) => ids.map((id) => sources.find((s) => s.id === id)).filter((s) => s !== undefined);

  /** ★ の付け外し。付けたら お気に入りの末尾、外したら そのほかの末尾へ */
  function toggleFavorite(id: string) {
    const { fav, rest } = ordered;
    layout.value = fav.includes(id)
      ? { fav: fav.filter((x) => x !== id), rest: [...rest, id] }
      : { fav: [...fav, id], rest: rest.filter((x) => x !== id) };
    follow(id);
  }
  /** 動かした行を追いかける。「操作したら対象が消えた」は最悪の体験(§09 規則 5) */
  function follow(id: string) {
    requestAnimationFrame(() => {
      document.querySelector(`[data-source-id="${id}"]`)?.scrollIntoView({ block: "nearest" });
    });
  }

  // --- ドラッグで並べ替え ---------------------------------------------------
  let dragId = $state<string | null>(null);
  /** いま落ちる位置。行の上半分なら手前、下半分なら後ろ */
  let dropAt = $state<{ list: "fav" | "rest"; index: number } | null>(null);

  function onDragStart(e: DragEvent, id: string) {
    dragId = id;
    e.dataTransfer?.setData("text/plain", id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
  function onDragOverRow(e: DragEvent, list: "fav" | "rest", index: number) {
    if (dragId === null) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropAt = { list, index: e.clientY < r.top + r.height / 2 ? index : index + 1 };
  }
  function onDragOverList(e: DragEvent, list: "fav" | "rest", count: number) {
    if (dragId === null) return;
    e.preventDefault();
    if (dropAt === null || dropAt.list !== list) dropAt = { list, index: count };
  }
  function onDrop(e: DragEvent) {
    e.preventDefault();
    const id = dragId;
    const at = dropAt;
    dragId = null;
    dropAt = null;
    if (id === null || at === null) return;
    const next: SourceLayout = {
      fav: ordered.fav.filter((x) => x !== id),
      rest: ordered.rest.filter((x) => x !== id),
    };
    const arr = next[at.list];
    const from = ordered[at.list].indexOf(id);
    const index = from !== -1 && from < at.index ? at.index - 1 : at.index;
    arr.splice(Math.max(0, Math.min(arr.length, index)), 0, id);
    layout.value = next;
    follow(id);
  }
  /** ドラッグできない環境(キーボード)向け。表には出さない */
  function onRowKey(e: KeyboardEvent, list: "fav" | "rest", index: number, id: string) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    dragId = id;
    dropAt = { list, index: index + (e.key === "ArrowUp" ? -1 : 2) };
    onDrop(new DragEvent("drop"));
  }

  const PLANNED: string[] = [];
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
        <span class="dim">押して中身を変える ・ つかんで並べ替え</span>
      </div>
      <div class="src-list">
        <!-- お気に入りと そのほか の 2 段。重さの差はプレイヤーが ★ で決める(§14 決定 3) -->
        {#each [{ key: "fav" as const, title: "お気に入り", ids: ordered.fav }, { key: "rest" as const, title: "そのほか", ids: ordered.rest }] as list (list.key)}
          <div
            class="src-group"
            role="list"
            ondragover={(e) => onDragOverList(e, list.key, list.ids.length)}
            ondrop={onDrop}
          >
            <div class="group-head">
              <span class="group-title">{list.title}</span>
              <span class="group-note dim">{list.ids.length} 件</span>
            </div>
            {#if list.ids.length === 0}
              <p class="group-empty dim">★ を押すか、行をここへ運ぶと上がります。</p>
            {/if}
            {#each itemsOf(list.ids) as s, i (s.id)}
              <!-- 行そのものが面。★ はその中のボタンで、面を 2 枚に割らない(§01) -->
              <div
                class="src src-line"
                class:on={openSource === s.id}
                class:dragging={dragId === s.id}
                class:drop-before={dropAt?.list === list.key && dropAt.index === i}
                class:drop-after={dropAt?.list === list.key && dropAt.index === i + 1 && i === list.ids.length - 1}
                data-source-id={s.id}
                role="button"
                tabindex="0"
                draggable="true"
                onclick={() => (openSource = s.id)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    openSource = s.id;
                  } else {
                    onRowKey(e, list.key, i, s.id);
                  }
                }}
                ondragstart={(e) => onDragStart(e, s.id)}
                ondragend={() => { dragId = null; dropAt = null; }}
                ondragover={(e) => onDragOverRow(e, list.key, i)}
                ondrop={onDrop}
              >
                <button
                  type="button"
                  class="fav"
                  class:on={list.key === "fav"}
                  aria-label="{s.name} を{list.key === 'fav' ? 'お気に入りから外す' : 'お気に入りに入れる'}"
                  title={list.key === "fav" ? "お気に入りから外す" : "お気に入りに入れる"}
                  onclick={(e) => { e.stopPropagation(); toggleFavorite(s.id); }}
                >★</button>
                <span class="src-main">
                  <span class="src-name">{s.name}</span>
                  <span class="src-sub num">{s.sub}</span>
                </span>
                <span class="chev dim">›</span>
              </div>
            {/each}
          </div>
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
      <div class="sheet-body open-in">
        <div class="sheet-card">
          <div class="card-title">最終能力値</div>
          <div class="stat-grid inset">
            {#each STAT_KINDS as k (k)}
              <span class="stat-cell">
                <span class="dim">{STAT_LABELS[k]}</span>
                <span class="num strong" use:bump={() => preview?.stats[k] ?? null}>{preview ? fmtInt(preview.stats[k]) : "—"}</span>
              </span>
            {/each}
          </div>
        </div>
        <div class="sheet-card">
          <div class="card-title">攻撃力(A){mainSkill ? ` — ${mainSkill.name}` : ""}</div>
          {#if preview?.attack}
            <div class="clear num"><span class="strong" use:bump={() => preview?.attack?.breakdown.value ?? null}>{fmtInt(preview.attack.breakdown.value)}</span></div>
            <div class="eq-summary num inset">
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
          <table class="eq-table num inset">
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
            強化倍率 +{enhanceRatePercent}%(共通スキル)。基本には武器アビリティと称号の分も入っています。
            強化のうちテシスコア・シエナのオーラの分はこの表に入りません
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
    border: 1px solid var(--warm); border-radius: var(--r-pill); padding: 1px 8px;
  }
  .spacer { flex: 1; }

  .cols { flex: 1; min-height: 0; display: grid; padding: 10px 16px 8px; column-gap: 0; }
  section { min-width: 0; min-height: 0; display: flex; flex-direction: column; }

  .src-head { display: flex; align-items: baseline; gap: 8px; padding: 0 2px 7px; }
  .src-title { font-size: var(--t-label); font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); }
  .src-head .dim { margin-left: auto; font-size: 9px; }
  .src-unset {
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted);
    border: 1px solid var(--border); border-radius: var(--r-pill); padding: 0 6px;
  }
  /* お気に入りと そのほか の 2 段(§14 決定 3)。重さの差はプレイヤーが決める */
  .src-group { min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .group-head {
    min-width: 0; display: flex; align-items: center; gap: 8px; padding: 2px 2px 0;
    font-size: 9.5px; letter-spacing: 0.08em;
  }
  .group-title { flex-shrink: 0; font-weight: 800; color: var(--fg-muted); }
  .group-note { min-width: 0; flex: 1; letter-spacing: 0; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .group-empty { margin: 0 0 2px; padding: 0 2px; font-size: 9.5px; }

  /* つかんで運ぶ。落ちる位置は行の縁に線で出す — 隙間を差し込むと下が全部ずれる(§09 規則 1) */
  .src-line { position: relative; cursor: grab; }
  .src-line.dragging { opacity: 0.45; }
  .src-line.drop-before::before, .src-line.drop-after::after {
    content: ""; position: absolute; left: 0; right: 0; height: 2px;
    background: var(--accent); border-radius: var(--r-pill);
  }
  .src-line.drop-before::before { top: -4px; }
  .src-line.drop-after::after { bottom: -4px; }
  /* ★ はホームタブのコンテンツと同じ操作・同じ寸法。金 = あなたの操作待ち(§03 予約色) */
  .src .fav {
    width: 20px; height: 20px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); border: 1px solid var(--border-soft);
    font-size: 10px; color: var(--fg-off);
  }
  .src .fav:hover { border-color: var(--gold); color: var(--gold); }
  .src .fav.on { background: #FDF9EE; border-color: var(--gold); color: var(--gold); }

  .src-list { flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
  .src {
    display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .src:hover:not(.planned) { border-color: var(--accent); }
  .src.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); }
  .src.planned { background: #F0F3F7; border-style: dashed; cursor: default; }
  .src-main { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .src-name { font-size: 11px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .src.planned .src-name, .src.planned .src-sub { color: var(--fg-off); }
  .src-sub { font-size: 9px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { flex-shrink: 0; font-size: 11px; }
  .attack-foot {
    flex-shrink: 0; margin-top: 8px; padding: 9px 11px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #EFF5FD); border: 1px solid #9FB4D0;
  }
  .attack-foot.empty { background: var(--bg-rail); border-style: dashed; border-color: var(--border); }
  .attack-head { display: flex; align-items: baseline; gap: 8px; }
  .attack-label { font-size: 10px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); }
  .attack-skill { margin-left: auto; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .attack-value { margin-top: 2px; font-size: 22px; font-weight: 700; line-height: 1.1; }
  .attack-parts { margin-top: 3px; font-size: 9px; line-height: 1.6; }
  .attack-note { margin: 4px 0 0; font-size: 9px; line-height: 1.55; }

  .src-note {
    flex-shrink: 0; margin: 10px 0 0; padding: 9px 11px; border-radius: var(--r-panel);
    background: var(--bg-rail); border: 1px dashed var(--border);
    font-size: 9.5px; line-height: 1.5; color: var(--fg-muted);
  }

  .detail { overflow: auto; padding-left: 6px; }

  .sheet { flex-shrink: 0; border-top: 1px solid var(--border-strong); background: var(--bg-mid); padding: 8px 16px 10px; }
  .sheet-trigger {
    width: 100%; display: flex; align-items: center; gap: 9px; padding: 8px 11px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #F1F6FC); border: 1px solid #9FB4D0;
    box-shadow: inset 0 1px 0 #fff; text-align: left;
  }
  .sheet-trigger:hover { border-color: var(--head-bar); }
  .sheet-title { flex-shrink: 0; font-size: var(--t-label); font-weight: 700; letter-spacing: 0.06em; color: var(--fg-head); white-space: nowrap; }
  .sheet-summary { min-width: 0; flex: 1; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sheet-chev {
    flex-shrink: 0; width: 20px; height: 20px; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); background: var(--bg-field); border: 1px solid var(--border);
    font-size: 9px; font-weight: 700; color: var(--accent);
  }
  .sheet-body { margin-top: 8px; max-height: 220px; overflow: auto; display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-start; }
  /* インセット面(表・最終能力値)の内側余白が増えた分、5 枚(4 カード + 削除)が
     1 行に収まるよう basis を詰める */
  .sheet-card {
    flex: 1 1 210px; min-width: 0; padding: 11px 12px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-strong);
  }
  .stat-grid { margin-top: 6px; padding: 7px 9px; display: flex; flex-wrap: wrap; gap: 5px 12px; }
  .stat-cell { display: flex; align-items: baseline; gap: 5px; font-size: 10px; }
  .stat-cell .strong { font-size: 12px; font-weight: 700; }
  .eq-summary { margin-top: 6px; padding: 7px 9px; display: flex; flex-wrap: wrap; gap: 5px 14px; font-size: 11px; }
  .eq-table { margin-top: 6px; width: 100%; border-collapse: collapse; border-spacing: 0; overflow: hidden; font-size: 11px; }
  .eq-table th, .eq-table td { padding: 3px 6px; border-bottom: 1px solid rgba(255, 255, 255, 0.55); }
  .eq-table thead th { font-size: 9px; font-weight: 700; color: var(--fg-muted); background: none; position: static; }
  .eq-table .rh { text-align: left; font-size: 10px; color: var(--fg-muted); font-weight: 700; }
  .eq-table .n { text-align: right; }
  .clear { margin-top: 4px; }
  .clear .strong { font-size: 20px; font-weight: 700; }
  .tiny { margin: 4px 0 0; font-size: 9.5px; line-height: 1.6; }
  .delete {
    flex-shrink: 0; align-self: stretch; padding: 8px 14px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--state-short-bd); font-size: var(--t-label); color: var(--danger);
  }
  .delete.confirm { background: var(--state-short-bg); font-weight: 700; }
</style>
