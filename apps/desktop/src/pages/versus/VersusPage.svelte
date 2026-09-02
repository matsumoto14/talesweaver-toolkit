<script lang="ts">
  // 対人: 2 人のキャラ(+使用スキル)を選び、両方向の命中率(A→B / B→A)を出す画面。
  // 攻撃側・防御側という役割は選ばせない ── 知りたいのは「殴り合ったらどっちがどれだけ当たるか」
  // であって、役割の入れ替え操作そのものが不要(ユーザー指摘 2026-09-01)。
  // 計算は Rust 側(preview_versus → domain::versus_accuracy)。ここは組み立てて渡すだけ。
  import { errorMessage, listSkills, previewVersus } from "../../api/commands";
  import type {
    AccuracyBoost, GrowthAction, GrowthGroup, GrowthGroupRooms, GrowthRoom, Skill, StatFixedSource, StatKind,
    VersusAccuracy,
  } from "../../api/types";
  import {
    EQUIPMENT_STAT_LABELS, PART_SLOT_LABELS, PET_SKILL_TIER_LABELS,
    RANDOM_OPTION_RANK_LABELS, STAT_LABELS,
  } from "../../labels";
  import { app, buffSelectionFor, gameCharacterName, payloadOf } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { badgeStyle } from "../../ui/states";
  import { bump, flash } from "../../ui/motion.svelte";
  import { latest } from "../../ui/latest.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Picker from "../../ui/Picker.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";

  // --- 1 人目 -------------------------------------------------------------
  // 既定 = いま選択中のキャラ(§ux 原則: 登録キャラデータを軸にする)
  let charAOverride = $state<number | null>(null);
  const charAId = $derived(
    app.characters.some((c) => c.id === charAOverride) ? charAOverride : app.selectedId,
  );
  const charA = $derived(app.characters.find((c) => c.id === charAId) ?? null);

  // --- 2 人目 -------------------------------------------------------------
  // 既定 = 1 人目以外の先頭キャラ(1 人しか登録が無ければ未選択のまま)
  let charBOverride = $state<number | null>(null);
  const defaultCharBId = $derived(app.characters.find((c) => c.id !== charAId)?.id ?? null);
  const charBId = $derived(
    app.characters.some((c) => c.id === charBOverride) ? charBOverride : defaultCharBId,
  );
  const charB = $derived(app.characters.find((c) => c.id === charBId) ?? null);

  // **相手側のキャラも候補から外さない**。外すと「2 人目に入っているキャラを 1 人目にしたい」
  // ときに、先に 2 人目を変えないと選べず、行き止まりになる(ユーザー指摘 2026-09-01)。
  // 同じキャラを選んだら 2 人を入れ替える(pickCharA / pickCharB)
  const characterOptions = () =>
    app.characters.map((c) => ({
      value: String(c.id),
      name: c.name,
      meta: gameCharacterName(c.game_character_id),
      iconId: c.game_character_id,
      iconKind: "character" as const,
    }));

  /** 1 人目を選ぶ。2 人目に入っているキャラを選んだら 2 人を入れ替える */
  function pickCharA(id: number | null) {
    const before = charAId;
    if (id !== null && id === charBId) charBOverride = before;
    charAOverride = id;
  }
  /** 2 人目を選ぶ。1 人目に入っているキャラを選んだら 2 人を入れ替える */
  function pickCharB(id: number | null) {
    const before = charBId;
    if (id !== null && id === charAId) charAOverride = before;
    charBOverride = id;
  }

  // --- 使用スキル(キャラタブの主軸スキルが正。CalcPage と同じ組み方) -------------
  // 両方向の命中Pにそれぞれの攻撃スキルが要るので、1 人目・2 人目それぞれに要る。
  function useSkillList(characterOf: () => (typeof app.characters)[number] | null) {
    let list = $state<Skill[]>([]);
    let gidSeen: string | null = null;
    let override = $state<string | null>(null);
    $effect(() => {
      const gid = characterOf()?.game_character_id ?? null;
      if (gid === gidSeen) return;
      gidSeen = gid;
      list = [];
      override = null;
      if (!gid) return;
      listSkills(gid)
        .then((l) => {
          if (gidSeen !== gid) return;
          list = l;
        })
        .catch((e) => reportError(errorMessage(e)));
    });
    return {
      get list() { return list; },
      get override() { return override; },
      set override(v: string | null) { override = v; },
    };
  }
  const skillsA = useSkillList(() => charA);
  const skillsB = useSkillList(() => charB);

  function resolveSkillId(
    character: (typeof app.characters)[number] | null,
    skills: { list: Skill[]; override: string | null },
  ) {
    const mainSkill = skills.list.find((s) => s.id === character?.main_skill_id) ?? null;
    return (
      (skills.override !== null && skills.list.some((s) => s.id === skills.override)
        ? skills.override
        : null)
      ?? mainSkill?.id
      ?? skills.list[0]?.id
      ?? ""
    );
  }
  const skillIdA = $derived(resolveSkillId(charA, skillsA));
  const skillIdB = $derived(resolveSkillId(charB, skillsB));

  function skillOptionsOf(skills: { list: Skill[] }) {
    return skills.list.map((s) => ({
      value: s.id,
      name: s.name,
      meta: s.accuracy !== null ? `命中 ${s.accuracy}` : "命中 未記載",
      iconId: s.id,
      iconKind: "skill" as const,
    }));
  }

  // --- 的中剣(極・的中剣)の つけ外し -------------------------------------------
  // 伸びしろの行ではなく **ON / OFF のチップ**(ユーザー決定 2026-09-02)。対人タブ内だけの
  // 状態で、キャラには保存しない ── `character_skills.skill_ids` を足し引きして
  // `preview_versus` を叩き直すだけ。既定 ON、SLv は上限(Rust の `level_of` の既定)。
  const swordOn = $state<Record<number, boolean>>({});
  // 覚えられるスキルの id は結果(accuracy_skill_available)で分かる。分かった時点で
  // payload に乗り、もう一度だけ問い合わせが走って落ち着く
  const swordSkillId = $state<Record<number, string>>({});
  const swordIsOn = (id: number | null) => (id === null ? true : (swordOn[id] ?? true));

  /** 攻撃側の payload に的中剣の ON / OFF を反映する(保存はしない) */
  function attackerPayloadOf(c: (typeof app.characters)[number]) {
    const payload = payloadOf(c);
    const id = swordSkillId[c.id];
    if (!id) return payload;
    const skills = payload.stat_sources.character_skills;
    const has = skills.skill_ids.includes(id);
    if (swordIsOn(c.id) && !has) skills.skill_ids = [...skills.skill_ids, id];
    if (!swordIsOn(c.id) && has) skills.skill_ids = skills.skill_ids.filter((x) => x !== id);
    return payload;
  }

  // --- 命中率(preview_versus)。A→B と B→A を 2 回呼ぶだけ(引数の順を入れ替える) ------
  function useDirection(
    attackerOf: () => (typeof app.characters)[number] | null,
    skillIdOf: () => string,
    defenderOf: () => (typeof app.characters)[number] | null,
  ) {
    let result = $state<VersusAccuracy | null>(null);
    let error = $state<string | null>(null);
    const requestLatest = latest({ debounce: 150 });
    $effect(() => {
      const a = attackerOf();
      const d = defenderOf();
      const sid = skillIdOf();
      if (!a || !d || !sid) {
        requestLatest.cancel();
        result = null;
        error = null;
        return;
      }
      // 依存(的中剣の ON / OFF・判明したスキル id)は effect の本体で同期に読む。
      // await のあとで読むと変更を追えず、チップを押しても再計算されない
      const attackerPayload = attackerPayloadOf(a);
      const attackerBuffs = buffSelectionFor(a);
      const defenderPayload = payloadOf(d);
      const defenderBuffs = buffSelectionFor(d);
      requestLatest.run(async (isCurrent) => {
        try {
          const r = await previewVersus(
            attackerPayload, attackerBuffs, sid, defenderPayload, defenderBuffs,
          );
          if (isCurrent()) {
            result = r;
            error = null;
            const learnable = r.accuracy_skill_available;
            if (learnable && swordSkillId[a.id] !== learnable.id) swordSkillId[a.id] = learnable.id;
          }
        } catch (e) {
          if (isCurrent()) {
            result = null;
            error = errorMessage(e);
          }
        }
      });
      return () => requestLatest.cancel();
    });
    return {
      get result() { return result; },
      get error() { return error; },
    };
  }
  const resultAB = useDirection(() => charA, () => skillIdA, () => charB);
  const resultBA = useDirection(() => charB, () => skillIdB, () => charA);

  // 必中かどうかの判定は domain(HitRate::capped)が持つ。ここで raw >= max を書き直さない
  function hitBadge(result: VersusAccuracy | null) {
    if (result === null) return { label: "?", state: "unknown" as const };
    return result.hit_rate.capped
      ? { label: "必中", state: "goal" as const }
      : { label: "命中率", state: "met" as const };
  }

  /** 倍率・スキル名・Lv は Rust が解決した値をそのまま出す(画面で計算しない) */
  function boostLabel(boost: AccuracyBoost): string | null {
    const source = boost.source;
    if (source === "none") return null;
    if (source === "concentration") return `ペット集中 ・ 命中P ×${boost.rate.toFixed(2)}`;
    return `${source.skill.name} Lv${source.skill.level} ・ 命中P ×${boost.rate.toFixed(2)}`;
  }

  // --- 「次にできること」(accuracy_growth / evasion_growth) ------------------------
  // 行は材料の名前ではなく **行動**(付ける・替える・上げる・乗せる)。文言はここで組む
  // ── Rust は id・名前・部位・段階だけを返す(ユーザー決定 2026-09-02)。

  /** ステの固定上昇源。ペットだけ「どの段階まで」が行動の中身になる */
  function statFixedLabel(stat: StatKind, source: StatFixedSource): string {
    const s = STAT_LABELS[stat];
    if (typeof source !== "string") {
      return `ペット S スキル(${s})を ${PET_SKILL_TIER_LABELS[source.pet_skill.target]} に`;
    }
    switch (source) {
      case "rune": return `ルーンスキル(${s})を上限まで`;
      case "crown": return `クラウン(${s})を上限まで`;
      case "monster_card": return `モンスターカード(${s})を上限まで`;
      case "sacred_relic": return `神鳥の聖物(${s})を上限まで`;
    }
  }

  function actionLabel(action: GrowthAction): string {
    if ("buff" in action) return `${action.buff.name}を使う`;
    if ("stat_buff" in action) {
      return `${action.stat_buff.name}を使う(${STAT_LABELS[action.stat_buff.stat]})`;
    }
    if ("ability_attach" in action) {
      const a = action.ability_attach;
      return `${PART_SLOT_LABELS[a.slot]}の空き枠に「${a.ability_name}」を付ける`;
    }
    if ("ability_replace" in action) {
      const a = action.ability_replace;
      return `${PART_SLOT_LABELS[a.slot]}の「${a.from_ability_name}」を「${a.ability_name}」に替える`;
    }
    if ("random_option_attach" in action) {
      const a = action.random_option_attach;
      return `${PART_SLOT_LABELS[a.slot]}の空き枠に「${a.option_name}」を ${RANDOM_OPTION_RANK_LABELS[a.rank]} で付ける`;
    }
    if ("random_option_rank_up" in action) {
      const a = action.random_option_rank_up;
      return `${PART_SLOT_LABELS[a.slot]}の「${a.option_name}」を ${RANDOM_OPTION_RANK_LABELS[a.rank]} に上げる`;
    }
    if ("stat_fixed" in action) return statFixedLabel(action.stat_fixed.stat, action.stat_fixed.source);
    if ("enchant" in action) {
      return `エンチャント枠の${EQUIPMENT_STAT_LABELS[action.enchant.stat]}を上限まで`;
    }
    return `シエナの空き段階に${EQUIPMENT_STAT_LABELS[action.siena.stat]}を積む`;
  }

  /** 同じ行動かを見る鍵。2 人が同じ手を持つときだけ 1 行にまとめる */
  const actionKey = (action: GrowthAction) => JSON.stringify(action);

  // 2 人ぶんを 1 つの表に並べる。Rust が返す並び(GrowthGroup = 費用の安い順)をそのまま使い、
  // 2 本のリストを区分ごとに突き合わせる。**この定数は Rust の GrowthGroup の宣言順の写し**で、
  // 並べ替えの指図ではない(2 列を 1 本に差し込むために順位が要るだけ)。
  const GROWTH_GROUP_ORDER: GrowthGroup[] =
    ["buff", "equipment_ability", "random_option", "stat_fixed", "enchant", "siena"];

  /** 区分の題。手の行は行動(〜を付ける)なので、区分は材料の呼び名で短く */
  function growthGroupLabel(group: GrowthGroup, stat: StatKind): string {
    switch (group) {
      case "buff": return "バフ";
      case "equipment_ability": return "装備アビリティ";
      case "random_option": return "ランダム OP";
      case "stat_fixed": return `${STAT_LABELS[stat]} の固定上昇`;
      case "enchant": return "エンチャント枠";
      case "siena": return "シエナのオーラ";
    }
  }

  interface GrowthGroupRow { group: GrowthGroup; a: GrowthGroupRooms | null; b: GrowthGroupRooms | null }
  interface GrowthRow { key: string; label: string; a: GrowthRoom | null; b: GrowthRoom | null }

  /** 区分の段。どちらかにある区分だけ、Rust の順で */
  function mergeGroups(a: GrowthGroupRooms[], b: GrowthGroupRooms[]): GrowthGroupRow[] {
    return GROWTH_GROUP_ORDER.flatMap((group) => {
      const row = {
        group,
        a: a.find((g) => g.group === group) ?? null,
        b: b.find((g) => g.group === group) ?? null,
      };
      return row.a || row.b ? [row] : [];
    });
  }

  /** 手の段。2 人が同じ手を持つときだけ 1 行にまとめる。順は Rust が返したまま(gain 降順) */
  function mergeRooms(a: GrowthRoom[], b: GrowthRoom[]): GrowthRow[] {
    const rows: GrowthRow[] = [];
    const index = new Map<string, GrowthRow>();
    const put = (room: GrowthRoom, side: "a" | "b") => {
      const key = actionKey(room.action);
      const existing = index.get(key);
      if (existing) {
        existing[side] = room;
        return;
      }
      const row: GrowthRow = { key, label: actionLabel(room.action), a: null, b: null };
      row[side] = room;
      index.set(key, row);
      rows.push(row);
    };
    a.forEach((room) => put(room, "a"));
    b.forEach((room) => put(room, "b"));
    return rows;
  }

  // 伸びしろの素の増分は合計行・内訳とも「いま → 上限まで積んだら」の矢印で出す
  // (ユーザー指摘 2026-09-02。「あと +N」「+825」「×1.35」と行ごとに言い方が割れていた)。
  // 伸びしろは既定で畳む。開いたら区分(バフ / 装備アビリティ / …)が並び、区分を開くと
  // 手が並ぶ ── **3 段**。手を一度に全部出すと多すぎて読めない(ユーザー指摘 2026-09-02)。
  // 命中P・回避Pで独立に開閉でき、区分もそれぞれ独立(押した行はその場に留まり、下に生えるだけ)
  let growthOpenAcc = $state(false);
  let growthOpenEva = $state(false);
  /** 開いている区分。鍵は `${ブロック}:${区分}`(命中P と 回避P で別) */
  let growthOpenGroups = $state<Record<string, boolean>>({});
  const toggleGrowthGroup = (key: string) => {
    growthOpenGroups[key] = !growthOpenGroups[key];
  };

  // ダメージ計算タブと同じ言い方(「結果への効きを %」)にそろえる。命中P・回避Pの素の増分は
  // 下限・上限で頭打ちのことがあるので、動かない材料は空白や「—」にせず ±0% と正直に出す。
  /**
   * 材料名は列幅(150px)に入り切らないことがある。CJK は任意の文字間で折り返せるので、
   * 何もしないと「エンチャント枠の命 / 中率補正を上限まで」と語の途中で切れる(実機で検出)。
   * 助詞の直後にだけ折り返し位置(U+200B)を置き、CSS 側で `word-break: keep-all` にして
   * そこ以外では切らない。幅を広げて解くと、開閉で列が動く(§09 規則 1)
   */
  function softBreaks(label: string): string {
    return label.replace(/([のをに])(?=\S)/g, "$1\u200b");
  }

  function formatHitRateGain(gain: number): string {
    if (gain > 0) return `+${gain}%`;
    if (gain < 0) return `${gain}%`;
    return "±0%";
  }
</script>

{#snippet numCell(value: number | null)}
  {#if value === null}
    <span class="unk">?</span>
  {:else}
    <span class="num" use:bump={() => value}>{value}</span>
  {/if}
{/snippet}

{#snippet textCell(value: string | null)}
  {#if value === null}
    <span class="unk">?</span>
  {:else}
    <span class="num" use:flash={() => value}>{value}</span>
  {/if}
{/snippet}

{#snippet growthSummaryCell(
  rateWord: string,
  result: VersusAccuracy | null,
  growthCount: number,
  max: number | null,
  point: number | null,
  maxHitRate: number | null,
)}
  {#if result === null || max === null || point === null || maxHitRate === null}
    <span class="unk">?</span>
  {:else if growthCount === 0}
    <span class="growth-none dim">いま打てる手なし</span>
  {:else}
    {@const hitGain = maxHitRate - result.hit_rate.value}
    <span class="growth-total">
      <span class="growth-total-hitrate num" use:bump={() => hitGain}>{rateWord} {formatHitRateGain(hitGain)}</span>
      {#if hitGain === 0}
        <!-- 必中か下限に張り付いていると全部積んでも命中率は動かない。素の増分だけ並ぶと
             「何をしても変わらない」と読めるので、頭打ちだと一言添える(§00 05) -->
        <span class="growth-capped dim">いま積んでも動かない(必中か下限)</span>
      {/if}
      <span class="growth-total-raw dim"><span class="num" use:bump={() => point}>{point}</span> → <span class="num" use:bump={() => max}>{max}</span></span>
    </span>
  {/if}
{/snippet}

{#snippet growthItemCell(item: GrowthRoom | null, unit: string)}
  {#if item === null}
    <!-- そのキャラには打てない手。相手側だけの行なので空欄にする -->
    <span class="growth-none dim">—</span>
  {:else}
    <span class="growth-item">
      <span class="growth-hitrate num" use:bump={() => item.hit_rate_gain}>{formatHitRateGain(item.hit_rate_gain)}</span>
      {#if item.provisional}<span class="growth-provisional" style={badgeStyle({ label: "仮", state: "temp" })}>仮</span>{/if}
      <!-- 材料の「いま → 打ったら」と、命中P/回避Pへの効きは行を分ける。1 行に並べると
           列幅(140px)に入らず、途中で折り返して読む順が壊れる(実機で検出) -->
      <span class="growth-detail dim" use:flash={() => `${item.current}→${item.target}`}>
        <span class="num">{item.current}</span> → <span class="num">{item.target}</span>
      </span>
      <span class="growth-convert dim">
        <span class="growth-unit">{unit}</span>
        <span class="num" use:bump={() => item.gain}>+{item.gain}</span>
      </span>
    </span>
  {/if}
{/snippet}

{#snippet growthGroupCell(item: GrowthGroupRooms | null, unit: string)}
  {#if item === null}
    <span class="growth-none dim">—</span>
  {:else}
    <!-- 区分の行も主役は「区分の手を全部打ったら命中率が何 % 動くか」。脇に手の数と素の増分 -->
    <span class="growth-item">
      <span class="growth-hitrate num" use:bump={() => item.hit_rate_gain}>{formatHitRateGain(item.hit_rate_gain)}</span>
      {#if item.provisional}<span class="growth-provisional" style={badgeStyle({ label: "仮", state: "temp" })}>仮</span>{/if}
      <span class="growth-convert dim">
        <span class="num" use:bump={() => item.rooms.length}>{item.rooms.length}</span><span class="growth-unit">手</span>
        <span class="growth-unit">・ {unit}</span>
        <span class="num" use:bump={() => item.gain}>+{item.gain}</span>
      </span>
    </span>
  {/if}
{/snippet}

{#snippet growthBlock(
  label: string,
  unit: string,
  stat: StatKind,
  rateWord: string,
  groupKey: string,
  resultA: VersusAccuracy | null,
  resultB: VersusAccuracy | null,
  growthA: GrowthGroupRooms[],
  growthB: GrowthGroupRooms[],
  maxA: number | null,
  maxB: number | null,
  pointA: number | null,
  pointB: number | null,
  maxHitRateA: number | null,
  maxHitRateB: number | null,
  open: boolean,
  onToggle: () => void,
)}
  {@const groups = mergeGroups(growthA, growthB)}
  {@const hasAny = groups.length > 0}
  <button
    type="button"
    class="grid-row sub growth-total-row"
    class:openable={hasAny}
    disabled={!hasAny}
    aria-expanded={open}
    onclick={onToggle}
  >
    <div class="cell label">
      {label}
      {#if hasAny}<span class="growth-chevron" class:open>▸</span>{/if}
    </div>
    <div class="cell val">{@render growthSummaryCell(rateWord, resultA, growthA.length, maxA, pointA, maxHitRateA)}</div>
    <div class="cell val">{@render growthSummaryCell(rateWord, resultB, growthB.length, maxB, pointB, maxHitRateB)}</div>
  </button>
  {#if open}
    {#each groups as row (row.group)}
      {@const key = `${groupKey}:${row.group}`}
      {@const groupOpen = growthOpenGroups[key] ?? false}
      {@const lastResort = row.group === "enchant" || row.group === "siena"}
      <!-- 区分の段。エンチャント枠(と見積りのシエナ)は費用が高い最終手段。末尾に薄く置く -->
      <button
        type="button"
        class="grid-row sub growth-group-row openable"
        class:last-resort={lastResort}
        aria-expanded={groupOpen}
        onclick={() => toggleGrowthGroup(key)}
      >
        <div class="cell label">
          {growthGroupLabel(row.group, stat)}
          <span class="growth-chevron" class:open={groupOpen}>▸</span>
        </div>
        <div class="cell val">{@render growthGroupCell(row.a, unit)}</div>
        <div class="cell val">{@render growthGroupCell(row.b, unit)}</div>
      </button>
      {#if groupOpen}
        {#each mergeRooms(row.a?.rooms ?? [], row.b?.rooms ?? []) as item (item.key)}
          <div class="grid-row sub growth-item-row" class:last-resort={lastResort}>
            <div class="cell label">{softBreaks(item.label)}</div>
            <div class="cell val">{@render growthItemCell(item.a, unit)}</div>
            <div class="cell val">{@render growthItemCell(item.b, unit)}</div>
          </div>
        {/each}
      {/if}
    {/each}
  {/if}
{/snippet}

{#snippet swordCell(
  character: (typeof app.characters)[number] | null,
  result: VersusAccuracy | null,
)}
  {@const skill = result?.accuracy_skill_available ?? null}
  {#if skill === null || character === null}
    <span class="growth-none dim">—</span>
  {:else}
    {@const on = swordIsOn(character.id)}
    <!-- チップは押した瞬間に切り替わる(§00 03/04)。数値は結果が返ったら動く -->
    <button
      type="button"
      class="sword-chip"
      class:on
      aria-pressed={on}
      onclick={() => (swordOn[character.id] = !on)}
      title="対人タブの中だけの切り替えです(キャラには保存しません)"
    >
      <span use:flash={() => (on ? "on" : "off")}>
        {skill.name} Lv{skill.max_level}
        <span class="sword-state">{on ? "ON" : "OFF"}</span>
      </span>
    </button>
  {/if}
{/snippet}

{#snippet sheetHead(a: (typeof app.characters)[number], b: (typeof app.characters)[number])}
  <div class="grid-row head">
    <div class="cell label"></div>
    <div class="cell col">
      <Icon kind="character" id={a.game_character_id} size={20} label={a.name} source={app.characterIcons[a.id] ?? null} />
      <span class="col-name">{a.name}</span>
    </div>
    <div class="cell col">
      <Icon kind="character" id={b.game_character_id} size={20} label={b.name} source={app.characterIcons[b.id] ?? null} />
      <span class="col-name">{b.name}</span>
    </div>
  </div>
{/snippet}

{#snippet hitRateLine(
  key: "AB" | "BA",
  attacker: (typeof app.characters)[number] | null,
  defender: (typeof app.characters)[number] | null,
  dir: { result: VersusAccuracy | null; error: string | null },
)}
  {@const result = dir.result}
  {@const badge = hitBadge(result)}
  {@const boost = result ? boostLabel(result.accuracy_boost) : null}
  <div class="rate-row" class:has-result={result !== null}>
    <!-- 矢印だと「どちらが殴る側か」を読者が補うことになる。助詞で主語・目的語を決める
         (ユーザー指摘 2026-09-02) -->
    <span class="rate-who">
      {#if attacker}
        <Icon kind="character" id={attacker.game_character_id} size={20} label={attacker.name} source={app.characterIcons[attacker.id] ?? null} />
        <span class="rate-name">{attacker.name}</span>
      {/if}
      <span class="rate-particle">が</span>
      {#if defender}
        <Icon kind="character" id={defender.game_character_id} size={20} label={defender.name} source={app.characterIcons[defender.id] ?? null} />
        <span class="rate-name">{defender.name}</span>
      {/if}
      <span class="rate-particle">に当てる</span>
    </span>

    <!-- 数値 ⇄ 必中 は要素が入れ替わる(mount では bump も flash も発火しない)ので、
         入れ物のほうを「どちらの形か」で flash させる -->
    <span class="rate-value" use:flash={() => (result === null ? "none" : result.hit_rate.capped ? "capped" : "rate")}>
      {#if dir.error}
        <span class="rate-note bad">{dir.error}</span>
      {:else if result === null}
        <span class="rate-num num dim">?</span>
      {:else if result.hit_rate.capped}
        <span class="rate-cap" style={badgeStyle(badge)}>必中</span>
      {:else}
        <span class="rate-num num" use:bump={() => result?.hit_rate.value ?? null}>{result.hit_rate.value}</span>
        <span class="rate-unit">%</span>
      {/if}
    </span>

    {#if result}
      <span class="rate-why dim">
        命中P <span class="num" use:bump={() => result?.accuracy_point ?? null}>{result.accuracy_point}</span>
        <span class="op">−</span>
        相手の回避P <span class="num" use:bump={() => result?.evasion_point ?? null}>{result.evasion_point}</span>
        <span class="op">=</span>
        <span class="num" use:bump={() => result?.hit_rate.raw ?? null}>{result.hit_rate.raw}</span>
        ・ 下限 <span class="num" use:bump={() => result?.hit_rate.min ?? null}>{result.hit_rate.min}</span>
        <span class="rate-basis dim">(15
          <span class="op">+</span>
          {#if result.min_hit_rate_recorded}
            <span class="num">{result.min_hit_rate}</span>
          {:else}
            <span class="unk" title="対人の最小命中率補正はプレイヤー側の供給源表が wiki に無いため未収録(0 決め打ち)">命中補正 ?</span>
          {/if}
          <span class="op">−</span>
          {#if result.min_evasion_rate_recorded}
            <span class="num">{result.min_evasion_rate}</span>
          {:else}
            <span class="unk">回避補正 ?</span>
          {/if}
          )</span>
        ・ 上限 <span class="num">{result.hit_rate.max}</span>
        {#if boost}
          ・ {boost}
        {/if}
      </span>
    {/if}
  </div>
{/snippet}

<div class="versus-page">
  <div class="scroll">
    {#if app.characters.length < 2}
      <p class="empty dim">2 人そろうと、どちらがどれだけ当てられるかを出せます。キャラタブで登録してください。</p>
    {:else}
      <div class="sides">
        <!-- 2 人は対等。帯の色を変えると役割の違いに読める(§02 金は「行ける?」の意味を持つ) -->
        <SheetCard tone="blue" title="キャラ 1" note="攻めるときに使うスキルも選ぶ">
          <div class="side-body">
            <Picker
              label="キャラ"
              bind:value={
                () => (charAId !== null ? String(charAId) : ""),
                (v) => pickCharA(v === "" ? null : Number(v))
              }
              options={characterOptions()}
              placeholder="キャラを選択してください"
            />
            {#if charA}
              <Picker
                label="使用スキル"
                bind:value={
                  () => skillIdA,
                  (v) => (skillsA.override = v)
                }
                options={skillOptionsOf(skillsA)}
                placeholder="スキルを選択してください"
                disabled={skillsA.list.length === 0}
              />
            {/if}
          </div>
        </SheetCard>

        <SheetCard tone="blue" title="キャラ 2" note="攻めるときに使うスキルも選ぶ">
          <div class="side-body">
            <Picker
              label="キャラ"
              bind:value={
                () => (charBId !== null ? String(charBId) : ""),
                (v) => pickCharB(v === "" ? null : Number(v))
              }
              options={characterOptions()}
              placeholder="キャラを選択してください"
            />
            {#if charB}
              <Picker
                label="使用スキル"
                bind:value={
                  () => skillIdB,
                  (v) => (skillsB.override = v)
                }
                options={skillOptionsOf(skillsB)}
                placeholder="スキルを選択してください"
                disabled={skillsB.list.length === 0}
              />
            {/if}
          </div>
        </SheetCard>
      </div>

      {#if !charA || !charB}
        <p class="empty dim">上の 2 枚でキャラを選ぶと、当たり方が出ます。</p>
      {:else}
        {@const rAB = resultAB.result}
        {@const rBA = resultBA.result}

        <!-- 主役の命中率 2 行を先に見せる。材料の突き合わせはその下(ユーザー指摘 2026-09-01) -->
        <div class="rates">
          {@render hitRateLine("AB", charA, charB, resultAB)}
          {@render hitRateLine("BA", charB, charA, resultBA)}
        </div>

        <!-- 突き合わせ表: 命中P・回避Pを横に並べて縦を半分にし、余った右側を埋める。
             伸びしろは既定で畳み、開いたときだけ材料の内訳を出す(ユーザー指摘 2026-09-01) -->
        <div class="twin">
          <div class="sheet">
            <div class="grid">
              {@render sheetHead(charA, charB)}
              <!-- 命中Pは各キャラが「攻撃側」になった方向の結果(A→B は A の命中P、B→A は B の命中P) -->
              <div class="grid-row main">
                <div class="cell label">命中P</div>
                <div class="cell val strong">{@render numCell(rAB?.accuracy_point ?? null)}</div>
                <div class="cell val strong">{@render numCell(rBA?.accuracy_point ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">DEX</div>
                <div class="cell val">{@render numCell(rAB?.attacker_dex ?? null)}</div>
                <div class="cell val">{@render numCell(rBA?.attacker_dex ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">装備の命中補正</div>
                <div class="cell val">{@render numCell(rAB?.equipment_accuracy ?? null)}</div>
                <div class="cell val">{@render numCell(rBA?.equipment_accuracy ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">スキルの命中</div>
                <div class="cell val">{@render numCell(rAB?.skill_accuracy ?? null)}</div>
                <div class="cell val">{@render numCell(rBA?.skill_accuracy ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">依存の補正</div>
                <div class="cell val">{@render textCell(rAB ? `+${rAB.correction_bonus} / −${rAB.correction_penalty}` : null)}</div>
                <div class="cell val">{@render textCell(rBA ? `+${rBA.correction_bonus} / −${rBA.correction_penalty}` : null)}</div>
              </div>
              <!-- 的中剣は伸びしろではなく **つけ外し**。覚えられるキャラにだけチップを出す
                   (§00 02 要らないものを見せない。両方とも覚えられなければ行ごと出さない) -->
              {#if rAB?.accuracy_skill_available || rBA?.accuracy_skill_available}
                <div class="grid-row sub">
                  <div class="cell label">的中剣</div>
                  <div class="cell val">{@render swordCell(charA, rAB)}</div>
                  <div class="cell val">{@render swordCell(charB, rBA)}</div>
                </div>
              {:else if rAB || rBA}
                <div class="grid-row sub">
                  <div class="cell label">命中P割合</div>
                  <div class="cell val">{@render textCell(rAB ? (boostLabel(rAB.accuracy_boost) ?? "なし") : null)}</div>
                  <div class="cell val">{@render textCell(rBA ? (boostLabel(rBA.accuracy_boost) ?? "なし") : null)}</div>
                </div>
              {/if}
              {@render growthBlock(
                "次にできること", "命中P", "dex", "命中率", "acc", rAB, rBA,
                rAB?.accuracy_growth ?? [], rBA?.accuracy_growth ?? [],
                rAB?.accuracy_max ?? null, rBA?.accuracy_max ?? null,
                rAB?.accuracy_point ?? null, rBA?.accuracy_point ?? null,
                rAB?.accuracy_max_hit_rate.value ?? null, rBA?.accuracy_max_hit_rate.value ?? null,
                growthOpenAcc, () => (growthOpenAcc = !growthOpenAcc),
              )}
            </div>
          </div>

          <div class="sheet">
            <div class="grid">
              {@render sheetHead(charA, charB)}
              <!-- 回避Pは各キャラが「防御側」になった方向の結果(A→B は B の回避P、B→A は A の回避P) -->
              <div class="grid-row main">
                <div class="cell label">回避P</div>
                <div class="cell val strong">{@render numCell(rBA?.evasion_point ?? null)}</div>
                <div class="cell val strong">{@render numCell(rAB?.evasion_point ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">AGI</div>
                <div class="cell val">{@render numCell(rBA?.defender_agi ?? null)}</div>
                <div class="cell val">{@render numCell(rAB?.defender_agi ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">装備の回避補正</div>
                <div class="cell val">{@render numCell(rBA?.equipment_evasion ?? null)}</div>
                <div class="cell val">{@render numCell(rAB?.equipment_evasion ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">装備の敏捷補正</div>
                <div class="cell val">{@render numCell(rBA?.equipment_agility ?? null)}</div>
                <div class="cell val">{@render numCell(rAB?.equipment_agility ?? null)}</div>
              </div>
              <div class="grid-row sub">
                <div class="cell label">攻撃タイプの補正</div>
                <div class="cell val">{@render textCell(rBA ? rBA.attack_type_bonus.toFixed(1) : null)}</div>
                <div class="cell val">{@render textCell(rAB ? rAB.attack_type_bonus.toFixed(1) : null)}</div>
              </div>
              {@render growthBlock(
                "次にできること", "回避P", "agi", "当てられる率", "eva", rBA, rAB,
                rBA?.evasion_growth ?? [], rAB?.evasion_growth ?? [],
                rBA?.evasion_max ?? null, rAB?.evasion_max ?? null,
                rBA?.evasion_point ?? null, rAB?.evasion_point ?? null,
                rBA?.evasion_max_hit_rate.value ?? null, rAB?.evasion_max_hit_rate.value ?? null,
                growthOpenEva, () => (growthOpenEva = !growthOpenEva),
              )}
            </div>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .versus-page { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  /* 幅は 2 表(名前の長い列を含めて最大 612px)が横に並ぶぶんまで。1280 幅のウィンドウで縦スクロールなしに収める */
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 1160px; }
  .empty { font-size: 12px; }

  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; align-items: start; }
  .side-body { display: flex; flex-direction: column; gap: 10px; padding: 11px 13px 13px; }

  /* 突き合わせ表: 命中P・回避Pを横に並べて縦を詰め、右の余白を埋める
     (ユーザー指摘 2026-09-01: 縦に伸びすぎ・右に余白が多い) */
  .twin { display: flex; flex-wrap: wrap; gap: 14px; align-items: flex-start; }

  /* 2 人の材料を閉じずに並べる(§ux 「閉じていると比べられない」)。
     中身ぶんの幅で左に寄せる ── 幅いっぱいまで伸ばすと数値の並びが間延びして読みにくい */
  .sheet {
    width: max-content; max-width: 100%;
    border: 1px solid var(--border-soft); border-radius: var(--r-window); background: var(--surface-inset);
    overflow: hidden;
  }
  /* 列幅は 1 つの grid で決め、行は subgrid で受ける。キャラ名の列は名前が入る幅まで伸ばす
     (140px 固定だと「マキシミン(全 / 身セイクリッ / ド)」と 3 行に折れていた。実機で検出)。
     上限は名前側(.col-name の max-width)で切る。minmax の上限を px で書くと max-content 幅の面では
     列が上限いっぱいまで広がる(実機で 2 表とも 220px になり縦に積まれた) */
  .grid { display: grid; grid-template-columns: 150px minmax(140px, max-content) minmax(140px, max-content); column-gap: 10px; }
  /* 列幅は「中身が読める」ことを優先する。キャラ名が「マキシ…」で切れると、
     2 列のどちらがどちらか分からなくなる(§00 05 読めない文字は出さない) */
  .grid-row { display: grid; grid-template-columns: subgrid; grid-column: 1 / -1; align-items: baseline; }
  .cell { padding: 5px 12px; }
  .cell.label { color: var(--fg-sub); font-size: 10.5px; white-space: nowrap; }
  .cell.val { text-align: right; min-width: 0; }
  .cell.val :global(.num), .cell.val :global(.unk) { font-size: 11px; }
  /* 的中剣のように文字で出す値は折り返さない(行の高さが列で食い違うと段がずれる) */
  .cell.val { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .grid-row.head { padding-top: 4px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
  .grid-row.head .cell { padding: 8px 13px; }
  .grid-row.head .cell.col { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
  .col-name { font-size: 12px; font-weight: 800; color: var(--fg-head); min-width: 0; max-width: 168px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* 主役の行(命中P・回避P)は太字で強めに、直下の内訳(sub)は控えめに */
  .grid-row.main { background: var(--bg-active); }
  .grid-row.main .cell.label { font-weight: 800; color: var(--fg-head); font-size: 11.5px; }
  .grid-row.main .cell.val :global(.num) { font-size: 15px; font-weight: 800; color: var(--fg-head); }
  .grid-row.sub .cell.label { padding-left: 24px; }
  .grid-row.sub .cell.val :global(.num) { color: var(--fg-sub); }

  /* 伸びしろ: 命中P・回避Pの直下(sub)に「あと +N」の合計行を出す。既定は畳んだまま
     ── 開いたときだけその下に材料の内訳(さらに一段字下げ)が生える(押した行は動かない)。
     値は文字列(「あと +N」「伸びしろなし」・内訳)を含むので折り返しを許す */
  .growth-total-row .cell.label { font-weight: 800; color: var(--fg-head); }
  /* 主役は「結果への効き(命中率 %)」。素の増分(あと +N)は脇へ小さく残す
     (ダメ計タブの「MAX で +N%」と同じ言い方にそろえる。ユーザー指示) */
  .growth-total { display: inline-flex; flex-direction: column; align-items: flex-end; gap: 1px; }
  .growth-total-hitrate { font-weight: 800; color: var(--fg-head); }
  .growth-total-raw { font-size: 9px; }
  .growth-total-raw :global(.num) { font-size: 9px; }
  /* button の既定見た目を消し、行としての見た目だけ残す(合計行・区分行とも) */
  button.growth-total-row, button.growth-group-row {
    background: none; border: none; margin: 0; padding: 0; font: inherit; color: inherit;
    text-align: inherit; width: 100%; cursor: default;
  }
  button.growth-total-row.openable, button.growth-group-row.openable { cursor: pointer; }
  button.growth-total-row.openable:hover .cell.label,
  button.growth-group-row.openable:hover .cell.label { color: var(--accent); }
  button.growth-total-row.openable:focus-visible,
  button.growth-group-row.openable:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
  .growth-chevron {
    display: inline-block; margin-left: 4px; font-size: 9px; color: var(--fg-dim);
    transition: transform 0.15s ease;
  }
  .growth-chevron.open { transform: rotate(90deg); }
  /* 3 段の字下げ: 合計 24px(sub)→ 区分 32px → 手 40px。区分の題は材料の呼び名で、
     手の行動(〜を付ける)より一段強く */
  .growth-group-row .cell.label { padding-left: 32px; font-weight: 700; }
  .growth-item-row .cell.label { padding-left: 40px; white-space: normal; word-break: keep-all; }
  .growth-total-row .cell.val, .growth-group-row .cell.val, .growth-item-row .cell.val {
    white-space: normal; overflow: visible; text-overflow: clip;
  }
  .growth-item { display: inline-flex; flex-direction: column; align-items: flex-end; gap: 1px; }
  /* 材料 1 件の主役も「命中率 ±N%」。素の増分(装備補正 +705 → 命中P +951)は
     その脇に小さく残す(ダメ計タブの「もし〜だったら」候補と同じ言い方) */
  .growth-hitrate { font-weight: 800; color: var(--fg-head); }
  .growth-detail { font-size: 9px; }
  /* 内訳は 1 行ずつ折り返さない。途中で折り返すと「1,178 → → 命中 +1514 / 2300 P」のように
     読む順が壊れる(実機で検出)。入り切らない幅なら列側を広げる */
  .growth-detail, .growth-convert { white-space: nowrap; }
  .growth-convert { display: inline-flex; align-items: baseline; gap: 3px; font-size: 9px; }
  .growth-convert :global(.num) { font-size: 9px; }
  .growth-unit { font-size: 9px; }
  .growth-provisional {
    font-size: 9px; font-weight: 800; border-radius: var(--r-pill); padding: 1px 5px; border: 1px solid;
    margin-left: 4px;
  }
  .growth-capped { font-size: 9px; }
  .growth-none { font-size: 10.5px; }
  /* 最終手段(エンチャント・シエナ)は末尾に薄く。並びは Rust が決めているので、
     ここでやるのは「目立たせない」ことだけ(§00 02) */
  .growth-group-row.last-resort .cell.label, .growth-item-row.last-resort .cell.label { color: var(--fg-dim); }
  .growth-group-row.last-resort .growth-hitrate, .growth-item-row.last-resort .growth-hitrate {
    font-weight: 700; color: var(--fg-sub);
  }

  /* 的中剣の ON / OFF。押した場所は動かない(幅・高さは状態で変えない) */
  .sword-chip {
    font: inherit; font-size: 10px; font-weight: 700; cursor: pointer;
    border: 1px solid var(--border); border-radius: var(--r-pill);
    background: var(--surface-inset); color: var(--fg-dim);
    padding: 2px 9px; white-space: nowrap;
  }
  .sword-chip.on { border-color: var(--accent); background: var(--bg-active); color: var(--fg-head); }
  .sword-chip:hover { border-color: var(--accent); }
  .sword-chip:focus-visible { outline: 1px solid var(--accent); outline-offset: 1px; }
  /* ON / OFF で幅が 5px 変わり押した場所が動く(実機で検出)。3 文字ぶんを取り切る */
  .sword-state { display: inline-block; min-width: 3ch; text-align: center; margin-left: 4px; font-size: 9px; letter-spacing: .06em; }

  /* 未収録(供給源が無いのでまだ 0 決め打ち)。0 や空白ではなく ? + 破線で示す */
  .unk {
    display: inline-block; padding: 0 5px; border: 1px dashed var(--state-unknown-bd);
    border-radius: var(--r-pill); color: var(--state-unknown-fg); font-size: 9.5px; font-weight: 700;
  }

  /* 主役: 2 方向の命中率。表の下にその根拠を添えて置く */
  .rates { display: flex; flex-direction: column; gap: 8px; }
  .rate-row {
    display: flex; flex-wrap: wrap; align-items: baseline; gap: 8px 10px;
    border: 1px dashed var(--state-unknown-bd); border-radius: var(--r-window);
    background: var(--state-unknown-bg); padding: 10px 13px;
  }
  /* 結果は読み取り専用なのでインセット面(§01)。白は編集できる面に予約 */
  .rate-row.has-result { border: 1px solid var(--border-soft); background: var(--surface-inset); }

  .rate-who { display: flex; align-items: center; gap: 6px; min-width: 0; flex-shrink: 0; font-size: 12px; font-weight: 700; color: var(--fg-head); }
  .rate-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rate-particle { flex-shrink: 0; font-size: 11px; font-weight: 500; color: var(--fg-sub); margin: 0 1px; }

  /* 桁が増えても隣が動かないよう幅を固定する(§09 規則 4) */
  .rate-value { flex-shrink: 0; display: flex; align-items: baseline; gap: 3px; min-width: 60px; }
  .rate-num { font-size: 19px; font-weight: 800; color: var(--fg-head); line-height: 1; }
  .rate-unit { font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .rate-cap {
    font-size: 10.5px; font-weight: 800; border-radius: var(--r-pill); padding: 3px 9px; border: 1px solid;
  }
  .rate-note { font-size: 10px; }
  .rate-note.bad { color: var(--danger); font-weight: 700; }

  .rate-why { font-size: 10.5px; display: flex; flex-wrap: wrap; align-items: baseline; gap: 3px; }
  .rate-why .op { color: var(--fg-dim); }
  .rate-basis { display: inline-flex; align-items: baseline; gap: 3px; }
</style>
