<script lang="ts">
  // 対人: 2 人のキャラ(+使用スキル)を選び、両方向の命中率(A→B / B→A)を出す画面。
  // 攻撃側・防御側という役割は選ばせない ── 知りたいのは「殴り合ったらどっちがどれだけ当たるか」
  // であって、役割の入れ替え操作そのものが不要(ユーザー指摘 2026-09-01)。
  // 計算は Rust 側(preview_versus → domain::versus_accuracy)。ここは組み立てて渡すだけ。
  // 画面は「方向ごとに 1 列」(A が B に当てる / B が A に当てる)。previewVersus は攻撃側視点の
  // 命中P と防御側視点の回避Pを 1 回の呼び出しで返すので、1 列の中は同じ result を読むだけでよい
  // (以前の「2 人を横に並べた突き合わせ表」は廃止。ワイヤーどおりに作り直す。2026-09-02)。
  // 頭のキャラ選択カード(.sides)は廃止。「[A] が [B] に当てる」の頭そのものが選ぶ場になる
  // (ユーザー指摘 2026-09-02)。
  import { SvelteMap } from "svelte/reactivity";
  import { cubicOut } from "svelte/easing";
  import { errorMessage, listSkills, previewVersus } from "../../api/commands";
  import type {
    AccuracyBoost, BuffSelection, GrowthAction, GrowthGroup, GrowthGroupRooms, GrowthRoom, HitRate, Skill,
    StatFixedSource, VersusAccuracy,
  } from "../../api/types";
  import { PART_SLOT_LABELS, PET_SKILL_TIER_LABELS, RANDOM_OPTION_RANK_LABELS } from "../../labels";
  import { app, gameCharacterName, payloadOf } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { badgeStyle } from "../../ui/states";
  import { bump, flash } from "../../ui/motion.svelte";
  import { latest } from "../../ui/latest.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Picker, { type PickerOption } from "../../ui/Picker.svelte";

  type CharacterRef = (typeof app.characters)[number];

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
  const characterOptions = (): PickerOption[] =>
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

  // --- 使うバフセット(計算タブの「使うセット」と同じ) -------------------------------
  // 既定 = キャラのいつものバフ。対人タブ内だけの切り替えで、キャラには保存しない。
  // 同じキャラは 2 列に(攻撃側・防御側として)出るが、状態は 1 つなので両列で揃う
  const buffSetOverride = $state<Record<number, number>>({});
  function buffSetIdOf(c: CharacterRef): number | null {
    const o = buffSetOverride[c.id];
    return app.buffSets.some((set) => set.id === o) ? o : c.default_buff_set_id;
  }
  function buffSelectionOf(c: CharacterRef): BuffSelection {
    const set = app.buffSets.find((set) => set.id === buffSetIdOf(c));
    return JSON.parse(JSON.stringify(set?.choices ?? { choices: [] })) as BuffSelection;
  }
  const buffSetOptions = (): PickerOption[] =>
    app.buffSets.map((set) => ({ value: String(set.id), name: set.name }));

  // --- 使用スキル(キャラタブの主軸スキルが正。CalcPage と同じ組み方) -------------
  // 両方向の命中Pにそれぞれの攻撃スキルが要るので、1 人目・2 人目それぞれに要る。
  function useSkillList(characterOf: () => CharacterRef | null) {
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
    character: CharacterRef | null,
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

  function skillOptionsOf(skills: { list: Skill[] }): PickerOption[] {
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
  function attackerPayloadOf(c: CharacterRef) {
    const payload = payloadOf(c);
    const id = swordSkillId[c.id];
    if (!id) return payload;
    const skills = payload.stat_sources.character_skills;
    const has = skills.skill_ids.includes(id);
    if (swordIsOn(c.id) && !has) skills.skill_ids = [...skills.skill_ids, id];
    if (!swordIsOn(c.id) && has) skills.skill_ids = skills.skill_ids.filter((x) => x !== id);
    return payload;
  }

  // --- 「試す」(伸びしろの手を反映する / しない) ---------------------------------
  // 対人タブの中だけの状態(保存しない)。キー = `キャラid:acc|eva:行動のJSON`。
  // 素の Map は $state に入れても .set()/.delete() が再描画を起こさないので SvelteMap を使う
  // (pages/chars/Workspace.svelte の SvelteSet と同じ理由)。
  type TriedKind = "acc" | "eva";
  const triedRooms = new SvelteMap<string, GrowthRoom>();
  const actionKey = (action: GrowthAction) => JSON.stringify(action);
  const triedKey = (charId: number, kind: TriedKind, action: GrowthAction) =>
    `${charId}:${kind}:${actionKey(action)}`;
  const isTried = (charId: number, kind: TriedKind, action: GrowthAction) =>
    triedRooms.has(triedKey(charId, kind, action));
  /** チップを押した瞬間に切り替わる。数値は preview_versus の結果が返ったら動く */
  function toggleTry(charId: number, kind: TriedKind, room: GrowthRoom) {
    const key = triedKey(charId, kind, room.action);
    if (triedRooms.has(key)) triedRooms.delete(key);
    else triedRooms.set(key, room);
  }
  function clearTries(charId: number, kind: TriedKind) {
    const prefix = `${charId}:${kind}:`;
    for (const key of [...triedRooms.keys()]) {
      if (key.startsWith(prefix)) triedRooms.delete(key);
    }
  }
  /** その方向の呼び出しに渡す手(ON にしたぶんだけ)。GrowthRoom.action をそのまま送り返す */
  function triesActionsFor(charId: number | null, kind: TriedKind): GrowthAction[] {
    if (charId === null) return [];
    const prefix = `${charId}:${kind}:`;
    const out: GrowthAction[] = [];
    for (const [key, room] of triedRooms) {
      if (key.startsWith(prefix)) out.push(room.action);
    }
    return out;
  }
  const triedCountFor = (charId: number | null, kind: TriedKind) => triesActionsFor(charId, kind).length;

  // --- 命中P / 回避P ブロックの開閉 ------------------------------------------------
  // 「命中P」「回避P」の頭の行を押すと開閉する。2 列(A→B / B→A)は同じ位置に同種の
  // ブロックを持つので、開閉状態は列を跨いで共有する(片方だけ閉じると段が横で揃わない)。
  let accBlockOpen = $state(true);
  let evaBlockOpen = $state(true);

  // --- 命中率(preview_versus)。A→B と B→A を 2 回呼ぶだけ(引数の順を入れ替える) ------
  function useDirection(
    attackerOf: () => CharacterRef | null,
    skillIdOf: () => string,
    defenderOf: () => CharacterRef | null,
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
      // 依存(的中剣の ON / OFF・試している手・判明したスキル id)は effect の本体で同期に読む。
      // await のあとで読むと変更を追えず、チップを押しても再計算されない
      const attackerPayload = attackerPayloadOf(a);
      const attackerBuffs = buffSelectionOf(a);
      const defenderPayload = payloadOf(d);
      const defenderBuffs = buffSelectionOf(d);
      const attackerTries = triesActionsFor(a.id, "acc");
      const defenderTries = triesActionsFor(d.id, "eva");
      requestLatest.run(async (isCurrent) => {
        try {
          const r = await previewVersus(
            attackerPayload, attackerBuffs, sid, defenderPayload, defenderBuffs,
            attackerTries, defenderTries,
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
    if (result.hit_rate.capped) return { label: "必中", state: "goal" as const };
    if (result.hit_rate.floored) return { label: "下限", state: "short" as const };
    return { label: "命中率", state: "met" as const };
  }
  const rateText = (hr: HitRate) => (hr.capped ? "必中" : `${hr.value}%`);

  /** 列の答えの一文(計算タブの「目安の 2.42 倍。火力は足りています」と同じ役)。
   *  余裕 / 不足は domain(HitRate.to_cap / to_leave_floor)の値をそのまま置く。画面で引き算しない */
  function answerText(hr: HitRate): string {
    if (hr.capped) {
      return hr.to_cap === 0
        ? "ぎりぎり必中。相手の回避P が 1 上がると外れ始める"
        : `必中。相手の回避P があと ${-hr.to_cap} 上がるまで必中のまま`;
    }
    if (hr.floored) return `下限に張り付き。命中P があと ${hr.to_leave_floor} 上がると動き始める`;
    return `必中まで命中P あと ${hr.to_cap}`;
  }

  /** 倍率・スキル名・Lv は Rust が解決した値をそのまま出す(画面で計算しない) */
  function boostLabel(boost: AccuracyBoost): string | null {
    const source = boost.source;
    if (source === "none") return null;
    if (source === "concentration") return `ペット集中 ・ 命中P ×${boost.rate.toFixed(2)}`;
    return `${source.skill.name} Lv${source.skill.level} ・ 命中P ×${boost.rate.toFixed(2)}`;
  }

  // --- 「次にできること」(accuracy_growth / evasion_growth) ------------------------
  // 行は材料の名前ではなく**名詞**。区分の題のほうに「〜を使う / 上げる」が入るので、
  // 行の文言に動詞は要らない(ユーザー決定 2026-09-02)。

  /** ステの固定上昇源。ペットだけ「どの段階まで」が名詞の中身になる */
  function statFixedLabel(source: StatFixedSource): string {
    if (typeof source !== "string") {
      return `ペット Sスキル ${PET_SKILL_TIER_LABELS[source.pet_skill.target]}`;
    }
    switch (source) {
      case "rune": return "ルーンスキル";
      case "crown": return "クラウン";
      case "monster_card": return "モンスターカード";
      case "sacred_relic": return "神鳥の聖物";
    }
  }

  function actionLabel(action: GrowthAction): string {
    if ("buff" in action) return action.buff.name;
    if ("stat_buff" in action) return action.stat_buff.name;
    if ("ability_attach" in action) {
      const a = action.ability_attach;
      return `${PART_SLOT_LABELS[a.slot]}: ${a.ability_name}`;
    }
    if ("ability_replace" in action) {
      const a = action.ability_replace;
      return `${PART_SLOT_LABELS[a.slot]}: ${a.from_ability_name} → ${a.ability_name}`;
    }
    if ("random_option_attach" in action) {
      const a = action.random_option_attach;
      return `${PART_SLOT_LABELS[a.slot]}: ${a.option_name}(${RANDOM_OPTION_RANK_LABELS[a.rank]})`;
    }
    if ("random_option_rank_up" in action) {
      const a = action.random_option_rank_up;
      return `${PART_SLOT_LABELS[a.slot]}: ${a.option_name} ${RANDOM_OPTION_RANK_LABELS[a.from_rank]} → ${RANDOM_OPTION_RANK_LABELS[a.rank]}`;
    }
    if ("stat_fixed" in action) return statFixedLabel(action.stat_fixed.source);
    if ("enchant" in action) return `${PART_SLOT_LABELS[action.enchant.slot]}: エンチャント`;
    return "シエナのオーラ";
  }

  /** バフ由来の手だけアイコンを付ける(id は BuffDefinition.id と一致) */
  function buffIdOf(action: GrowthAction): string | null {
    if ("buff" in action) return action.buff.buff_id;
    if ("stat_buff" in action) return action.stat_buff.buff_id;
    return null;
  }

  /** 区分の題。同じ 4 区分でも「命中」か「回避」かで動詞の対象が変わる */
  function growthGroupLabel(group: GrowthGroup, kind: TriedKind): string {
    switch (group) {
      case "stat": return "ステータスを伸ばす";
      case "buff": return kind === "acc" ? "命中バフを使う" : "回避バフを使う";
      case "equipment": return kind === "acc" ? "装備命中補正を上げる" : "装備回避補正を上げる";
      case "enchant": return "エンチャントする";
    }
  }

  // 材料名は列幅に入り切らないことがある。CJK は任意の文字間で折り返せるので、何もしないと
  // 語の途中で切れる。助詞の直後にだけ折り返し位置(U+200B)を置き、CSS 側で
  // `word-break: keep-all` にしてそこ以外では切らない(既存の実装踏襲)
  function softBreaks(label: string): string {
    return label.replace(/([のをに])(?=\S)/g, "$1​");
  }

  /** 命中P / 回避P ブロックの開閉。svelte/transition の slide は height を動かすが、
   *  .stat-body は flex の子(flex: 1)なので height が無視されて動かない(実機で検出)。
   *  max-height なら flex でも効く。動いている間だけ親ブロックの flex を止め、下の段(回避P の頭)が
   *  中身の縮みに合わせて滑らかに寄るようにする(止めないと空いた分を親が取り続け、最後に跳ぶ) */
  function collapse(node: HTMLElement, { duration = 220 } = {}) {
    const block = node.parentElement as HTMLElement;
    const height = node.getBoundingClientRect().height;
    block.style.flex = "none";
    const release = () => (block.style.flex = "");
    const timer = setTimeout(release, duration + 60);
    // tick は始まり(intro は t=0 / outro は t=1)にも呼ばれるので、最初の 1 回は終わりと見なさない
    let started = false;
    return {
      duration,
      easing: cubicOut,
      css: (t: number) => `max-height: ${t * height}px; overflow: hidden; min-height: 0;`,
      tick: (t: number) => {
        if (started && (t === 1 || t === 0)) { clearTimeout(timer); release(); }
        started = true;
      },
    };
  }

  function formatHitRateGain(gain: number): string {
    if (gain > 0) return `+${gain}%`;
    if (gain < 0) return `${gain}%`;
    return "±0%";
  }

  function formatPointGain(gain: number): string {
    if (gain > 0) return `+${gain}`;
    if (gain < 0) return `${gain}`;
    return "±0";
  }
</script>

{#snippet numCell(value: number | null, sim: boolean = false)}
  {#if value === null}
    <span class="unk">?</span>
  {:else}
    <span class="num" class:sim-value={sim} use:bump={() => value}>{value}</span>
  {/if}
{/snippet}

{#snippet textCell(value: string | null)}
  {#if value === null}
    <span class="unk">?</span>
  {:else}
    <span class="num" use:flash={() => value}>{value}</span>
  {/if}
{/snippet}

{#snippet swordCell(
  character: CharacterRef,
  result: VersusAccuracy | null,
)}
  {@const skill = result?.accuracy_skill_available ?? null}
  {#if skill === null}
    <span class="unk">?</span>
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

{#snippet growthChip(charId: number | null, kind: TriedKind, room: GrowthRoom)}
  {@const on = charId !== null && isTried(charId, kind, room.action)}
  {@const label = actionLabel(room.action)}
  {@const buffId = buffIdOf(room.action)}
  <!-- 計算タブの .buff-chip と同じ見え方(枠・色・状態バッジ)。チェックボックスは使わない -->
  <div
    class="try-chip"
    class:on
    role="button"
    tabindex="0"
    aria-pressed={on}
    onclick={() => charId !== null && toggleTry(charId, kind, room)}
    onkeydown={(e) => {
      if ((e.key === "Enter" || e.key === " ") && charId !== null) { e.preventDefault(); toggleTry(charId, kind, room); }
    }}
  >
    {#if buffId}<Icon kind="buff" id={buffId} size={20} label={label} />{/if}
    <span class="try-chip-copy">
      <span>{softBreaks(label)}</span>
      {#if on}<span class="try-chip-note dim">{room.current} → {room.target}</span>{/if}
    </span>
    <span class="try-chip-gain num">{formatPointGain(room.gain)}</span>
    <span class="try-chip-state" class:on>{on ? "反映中" : ""}</span>
  </div>
{/snippet}

{#snippet growthList(
  charId: number | null,
  kind: TriedKind,
  rateWord: string,
  unit: string,
  groups: GrowthGroupRooms[],
  max: number | null,
  point: number | null,
  maxHitRateGain: number | null,
  triedCount: number,
  beforePoint: number | null,
  nowPoint: number | null,
)}
  <div class="try-section">
    <div class="try-head">
      {#if max === null || point === null || maxHitRateGain === null}
        <span class="unk">?</span>
      {:else}
        <span class="try-head-summary" use:flash={() => (maxHitRateGain === 0 ? "stuck" : "moves")}>
          {#if maxHitRateGain === 0}
            <!-- 必中か下限に張り付いていると全部積んでも命中率は動かない(§00 05) -->
            全部積んでも{rateWord}は動かない
          {:else}
            全部やると <span class="num" use:bump={() => maxHitRateGain}>{rateWord} {formatHitRateGain(maxHitRateGain)}</span>
          {/if}
          ・ <span class="num" use:bump={() => point}>{point}</span> → <span class="num" use:bump={() => max}>{max}</span>
        </span>
      {/if}
    </div>

    <!-- 状態行は常設(高さ固定)。試しの有無で下の一覧の高さが変わらないようにする -->
    <div class="try-status">
      {#if triedCount > 0 && beforePoint !== null && nowPoint !== null}
        <span class="try-status-on">
          試し <span class="num">{triedCount}</span>件を反映中 ・ {unit}
          <span class="num sim-value" use:bump={() => beforePoint}>{beforePoint}</span>
          <span class="try-arrow">→</span>
          <span class="num sim-value" use:bump={() => nowPoint}>{nowPoint}</span>
          <button type="button" class="try-clear" onclick={() => charId !== null && clearTries(charId, kind)}>
            全部外す
          </button>
        </span>
      {/if}
      <!-- 試していないときは空(空状態の文言を常設しない §00 02)。高さは固定のまま -->
    </div>

    <!-- 固定高さの中でスクロールする。手の数で列の高さが変わらないようにする。
         必中 / 下限に張り付いていて何を積んでも動かないときは面ごと薄くする(押せはする) -->
    <div class="try-list" class:stuck={maxHitRateGain === 0}>
      {#if groups.length === 0}
        <div class="try-empty dim">いま打てる手なし</div>
      {:else}
        {#each groups as g (g.group)}
          {@const lastResort = g.group === "enchant"}
          <div class="try-group-label" class:last-resort={lastResort}>
            <span>{growthGroupLabel(g.group, kind)}</span>
            <!-- 区分を全部打ったときの率への効き(Rust の再計算値)。+N は命中P、こちらは % -->
            <span class="try-group-gain num" use:bump={() => g.hit_rate_gain}>{rateWord} {formatHitRateGain(g.hit_rate_gain)}</span>
          </div>
          <div class="try-chips">
            {#each g.rooms as room (actionKey(room.action))}
              {@render growthChip(charId, kind, room)}
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/snippet}

{#snippet buffSetRow(character: CharacterRef | null)}
  <!-- 使うバフセット。計算タブの「使うセット」と同じ役。命中P(DEX・命中P増加)にも
       回避P(AGI)にも効くので、材料の行として両ブロックに置く(同じキャラなら同じ状態) -->
  <div class="stat-row with-picker">
    <div class="stat-label">バフセット</div>
    <div class="stat-val">
      {#if character}
        <Picker
          bind:value={
            () => { const id = buffSetIdOf(character); return id === null ? "" : String(id); },
            (v) => { if (v !== "") buffSetOverride[character.id] = Number(v); }
          }
          options={buffSetOptions()}
          placeholder="バフなし"
          disabled={app.buffSets.length === 0}
        />
      {:else}
        <span class="unk">?</span>
      {/if}
    </div>
  </div>
{/snippet}

{#snippet accBlock(
  attacker: CharacterRef | null,
  result: VersusAccuracy | null,
  accCount: number,
  skillId: string,
  skills: { list: Skill[]; override: string | null },
)}
  <div class="stat-block">
    <button
      type="button"
      class="stat-row main stat-toggle"
      aria-expanded={accBlockOpen}
      onclick={() => (accBlockOpen = !accBlockOpen)}
    >
      <div class="stat-label">
        <span class="stat-caret" class:open={accBlockOpen}>▸</span>命中P
        {#if !accBlockOpen && result}
          <!-- 畳んでも「何のスキルで・的中剣は」が残る(計算タブの折りたたみカードの右の要約と同じ役) -->
          <span class="stat-summary dim">
            {skills.list.find((s) => s.id === skillId)?.name ?? ""}
            {#if result.accuracy_skill_available && attacker}・ 的中剣 {swordIsOn(attacker.id) ? "ON" : "OFF"}{/if}
          </span>
        {/if}
      </div>
      <div class="stat-val">{@render numCell(result?.accuracy_point ?? null, accCount > 0)}</div>
    </button>
    {#if accBlockOpen}
      <!-- 開閉は高さが変わるので動かす(§00 04)。2 列で同時に開閉するので同時に動く -->
      <div class="stat-body" transition:collapse>
        <div class="stat-row">
          <div class="stat-label">DEX</div>
          <div class="stat-val">{@render numCell(result?.attacker_dex ?? null, accCount > 0)}</div>
        </div>
        <div class="stat-row">
          <div class="stat-label">装備の命中補正</div>
          <div class="stat-val">{@render numCell(result?.equipment_accuracy ?? null)}</div>
        </div>
        <div class="stat-row with-picker">
          <div class="stat-label">スキルの命中</div>
          <div class="stat-val">
            {#if attacker}
              <Picker
                bind:value={
                  () => skillId,
                  (v) => (skills.override = v)
                }
                options={skillOptionsOf(skills)}
                placeholder="スキルを選択してください"
                disabled={skills.list.length === 0}
              />
            {:else}
              <span class="unk">?</span>
            {/if}
          </div>
        </div>
        {@render buffSetRow(attacker)}
        <div class="stat-row">
          <div class="stat-label">依存の補正</div>
          <div class="stat-val">{@render textCell(result ? `+${result.correction_bonus} / −${result.correction_penalty}` : null)}</div>
        </div>
        <div class="stat-row">
          <div class="stat-label">{result?.accuracy_skill_available ? "的中剣" : "命中P割合"}</div>
          <div class="stat-val">
            {#if result?.accuracy_skill_available && attacker}
              {@render swordCell(attacker, result)}
            {:else}
              {@render textCell(result ? (boostLabel(result.accuracy_boost) ?? "なし") : null)}
            {/if}
          </div>
        </div>

        <!-- 「全部やると」は試す前(before_tries)を基準に引く。伸びしろ側(max)は試す前の
             payload から固定されているので、試した後の値と混ぜると差が嘘になる -->
        {@render growthList(
          attacker?.id ?? null, "acc", "命中率", "命中P",
          result?.accuracy_growth ?? [],
          result?.accuracy_max ?? null, result?.before_tries?.accuracy_point ?? result?.accuracy_point ?? null,
          result?.accuracy_max_hit_rate_gain ?? null,
          accCount, result?.before_tries?.accuracy_point ?? null, result?.accuracy_point ?? null,
        )}
      </div>
    {/if}
  </div>
{/snippet}

{#snippet evaBlock(
  defender: CharacterRef | null,
  result: VersusAccuracy | null,
  evaCount: number,
)}
  <div class="stat-block">
    <button
      type="button"
      class="stat-row main stat-toggle"
      aria-expanded={evaBlockOpen}
      onclick={() => (evaBlockOpen = !evaBlockOpen)}
    >
      <div class="stat-label">
        <span class="stat-caret" class:open={evaBlockOpen}>▸</span>回避P
        {#if !evaBlockOpen && result}
          <span class="stat-summary dim">AGI <span class="num">{result.defender_agi}</span></span>
        {/if}
      </div>
      <div class="stat-val">{@render numCell(result?.evasion_point ?? null, evaCount > 0)}</div>
    </button>
    {#if evaBlockOpen}
      <div class="stat-body" transition:collapse>
        <div class="stat-row">
          <div class="stat-label">AGI</div>
          <div class="stat-val">{@render numCell(result?.defender_agi ?? null, evaCount > 0)}</div>
        </div>
        <div class="stat-row">
          <div class="stat-label">装備の回避補正</div>
          <div class="stat-val">{@render numCell(result?.equipment_evasion ?? null)}</div>
        </div>
        <div class="stat-row">
          <div class="stat-label">装備の敏捷補正</div>
          <div class="stat-val">{@render numCell(result?.equipment_agility ?? null)}</div>
        </div>
        {@render buffSetRow(defender)}
        <div class="stat-row">
          <div class="stat-label">攻撃タイプの補正</div>
          <div class="stat-val">{@render textCell(result ? result.attack_type_bonus.toFixed(1) : null)}</div>
        </div>

        {@render growthList(
          defender?.id ?? null, "eva", "当てられる率", "回避P",
          result?.evasion_growth ?? [],
          result?.evasion_max ?? null, result?.before_tries?.evasion_point ?? result?.evasion_point ?? null,
          result?.evasion_max_hit_rate_gain ?? null,
          evaCount, result?.before_tries?.evasion_point ?? null, result?.evasion_point ?? null,
        )}
      </div>
    {/if}
  </div>
{/snippet}

{#snippet directionColumn(
  attacker: CharacterRef | null,
  defender: CharacterRef | null,
  dir: { result: VersusAccuracy | null; error: string | null },
  pickAttacker: (id: number | null) => void,
  pickDefender: (id: number | null) => void,
  skillId: string,
  skills: { list: Skill[]; override: string | null },
)}
  {@const result = dir.result}
  {@const badge = hitBadge(result)}
  {@const accCount = attacker ? triedCountFor(attacker.id, "acc") : 0}
  {@const evaCount = defender ? triedCountFor(defender.id, "eva") : 0}
  {@const totalTried = accCount + evaCount}
  <div class="direction">
    <!-- 頭がそのまま選択の場。上の別カードは廃止(ユーザー指摘 2026-09-02)。
         助詞で主語・目的語を決める ── 矢印だと「どちらが殴る側か」を読者が補うことになる -->
    <div class="dir-head">
      <span class="dir-who">
        <span class="dir-picker">
          <Picker
            bind:value={
              () => (attacker ? String(attacker.id) : ""),
              (v) => pickAttacker(v === "" ? null : Number(v))
            }
            options={characterOptions()}
            placeholder="キャラを選択"
          />
        </span>
        <span class="dir-particle">が</span>
        <span class="dir-picker">
          <Picker
            bind:value={
              () => (defender ? String(defender.id) : ""),
              (v) => pickDefender(v === "" ? null : Number(v))
            }
            options={characterOptions()}
            placeholder="キャラを選択"
          />
        </span>
        <span class="dir-particle">に当てる</span>
      </span>
      <span class="dir-right">
        {#if totalTried > 0}
          <span class="try-badge" use:bump={() => totalTried}>試し {totalTried}件</span>
        {/if}
        <!-- 数値 ⇄ 必中 は要素が入れ替わるので、入れ物のほうを「どちらの形か」で flash させる -->
        <span class="dir-value" use:flash={() => (result === null ? "none" : result.hit_rate.capped ? "capped" : "rate")}>
          {#if dir.error}
            <span class="rate-note bad">{dir.error}</span>
          {:else if result === null}
            <span class="rate-num num dim">?</span>
          {:else}
            {#if totalTried > 0 && result.before_tries && rateText(result.before_tries.hit_rate) !== rateText(result.hit_rate)}
              <!-- 率が動いたときだけ「元 → いま」。「必中 → 必中」は情報が無い(§00 02) -->
              <span class="rate-num num sim-value">{rateText(result.before_tries.hit_rate)}</span>
              <span class="try-arrow">→</span>
            {/if}
            {#if result.hit_rate.capped}
              <span class="rate-cap" style={badgeStyle(badge)}>必中</span>
            {:else}
              <span class="rate-num num" use:bump={() => result?.hit_rate.value ?? null}>{result.hit_rate.value}</span>
              <span class="rate-unit">%</span>
              {#if result.hit_rate.floored}
                <!-- 下限に張り付いている値は、式を読まなくても分かるようバッジで言う -->
                <span class="rate-floor" style={badgeStyle(badge)}>下限</span>
              {/if}
            {/if}
          {/if}
        </span>
      </span>
    </div>

    {#if result}
      {@const boost = boostLabel(result.accuracy_boost)}
      <!-- 答えの一文(主役の率の次に読む)。余裕 / 不足は domain の値 -->
      <div class="dir-answer" use:flash={() => answerText(result.hit_rate)}>{answerText(result.hit_rate)}</div>
      <div class="dir-why dim">
        命中P <span class="num" use:bump={() => result?.accuracy_point ?? null}>{result.accuracy_point}</span>
        <span class="op">−</span>
        相手の回避P <span class="num" use:bump={() => result?.evasion_point ?? null}>{result.evasion_point}</span>
        <span class="op">=</span>
        <span class="num" use:bump={() => result?.hit_rate.raw ?? null}>{result.hit_rate.raw}</span>
        ・ 下限 <span class="num" use:bump={() => result?.hit_rate.min ?? null}>{result.hit_rate.min}</span>
        ・ 上限 <span class="num">{result.hit_rate.max}</span>
        {#if boost}・ {boost}{/if}
      </div>
    {/if}

    {@render accBlock(attacker, result, accCount, skillId, skills)}
    {@render evaBlock(defender, result, evaCount)}
  </div>
{/snippet}

<div class="versus-page">
  <div class="scroll">
    {#if app.characters.length < 2}
      <p class="empty dim">2 人そろうと、どちらがどれだけ当てられるかを出せます。キャラタブで登録してください。</p>
    {:else}
      <!-- 方向ごとに 1 列。殴り合ったら、どっちがどれだけ当たるか(ユーザー決定 2026-09-02) -->
      <div class="directions">
        {@render directionColumn(charA, charB, resultAB, pickCharA, pickCharB, skillIdA, skillsA)}
        {@render directionColumn(charB, charA, resultBA, pickCharB, pickCharA, skillIdB, skillsB)}
      </div>
    {/if}
  </div>
</div>

<style>
  .versus-page { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 1160px; }
  .empty { font-size: 12px; }

  /* 方向ごとに 1 列。1280 幅のウィンドウで 2 列が横に並ぶ幅にする */
  /* 2 列は残りの高さを埋め、手の一覧がその中で伸び縮みする。1280×840 に縦スクロールなしで収める
     ため。窓が低すぎるときだけ .scroll 側のスクロールに落ちる(一覧の min-height が効く) */
  /* 折り返し(wrap)にすると行の高さが中身で決まり、一覧が縮まない(実機で検出)。2 列は常に横並び */
  /* min-height: 0 は付けない。縮んでよいのは手の一覧(スクロール面)だけで、それ以外を中身より
     縮めると一覧が箱からあふれて次の段に重なる(実機で検出)。窓が低いときは .scroll が縦に流れる */
  .directions { display: flex; flex-wrap: nowrap; gap: 14px; align-items: stretch; flex: 1; }
  .direction {
    flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 8px;
    padding: 12px 14px 14px; border-radius: var(--r-window);
    border: 1px solid var(--border-soft); background: var(--surface-inset);
  }

  /* 未収録(供給源が無いのでまだ 0 決め打ち)。0 や空白ではなく ? + 破線で示す */
  .unk {
    display: inline-block; padding: 0 5px; border: 1px dashed var(--state-unknown-bd);
    border-radius: var(--r-pill); color: var(--state-unknown-fg); font-size: 9.5px; font-weight: 700;
  }

  /* 試しで動いた値(命中P・DEX・回避P・AGI・率)はラベンダー(§03「保存されない」予約色) */
  :global(.sim-value) { color: var(--sim-fg) !important; }

  /* 頭: キャラの選択 + 主役の命中率 -------------------------------------------- */
  .dir-head { display: flex; align-items: center; gap: 8px; flex-wrap: nowrap; height: 34px; box-sizing: border-box; }
  .dir-who { display: flex; align-items: center; gap: 6px; min-width: 0; flex: 1 1 auto; font-size: 12px; font-weight: 700; color: var(--fg-head); }
  /* 頭の Picker は名前だけの見え方に寄せる(枠は薄く)。押した場所は動かない */
  .dir-picker { min-width: 0; flex: 1 1 auto; max-width: 190px; }
  /* 頭では登録名だけ見せる。職名まで並べると登録名が「イサ…」に切れる(実機で検出)。候補一覧には残す */
  .dir-picker :global(.picker-trigger .picker-meta) { display: none; }
  .dir-picker :global(.picker-trigger) { padding: 3px 8px; font-size: 12px; font-weight: 700; }
  .dir-particle { flex-shrink: 0; font-size: 11px; font-weight: 500; color: var(--fg-sub); margin: 0 1px; }
  .dir-right { flex: none; display: flex; align-items: center; gap: 8px; }

  .try-badge {
    flex-shrink: 0; padding: 2px 9px; border-radius: var(--r-pill);
    font-size: 9.5px; font-weight: 700; white-space: nowrap;
    background: var(--state-temp-bg); border: 1px solid var(--sim); color: var(--sim-fg);
  }

  /* 桁が増えても隣が動かないよう幅を固定する(§09 規則 4) */
  .dir-value { flex-shrink: 0; display: flex; align-items: baseline; gap: 3px; min-width: 76px; justify-content: flex-end; }
  /* 列の主役。1 列に 1 つ(44px の主役は画面に 1 つだけの規格なので、2 列の主役は 1 段落とす) */
  .rate-num { font-size: 28px; font-weight: 800; color: var(--fg-head); line-height: 1; }
  .rate-unit { font-size: 12px; font-weight: 700; color: var(--fg-sub); }
  .rate-cap { font-size: 13px; font-weight: 800; border-radius: var(--r-pill); padding: 5px 12px; border: 1px solid; }
  .rate-floor { font-size: 9.5px; font-weight: 800; border-radius: var(--r-pill); padding: 2px 7px; border: 1px solid; margin-left: 4px; }
  /* 答えの一文。1 行に固定 */
  .dir-answer { font-size: 11.5px; font-weight: 700; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rate-note { font-size: 10px; }
  .rate-note.bad { color: var(--danger); font-weight: 700; }
  .try-arrow { font-size: 10px; color: var(--fg-dim); }

  /* 式の 1 行。1 行に固定(nowrap + ellipsis) */
  .dir-why { font-size: 10.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dir-why .op { color: var(--fg-dim); }

  /* 命中P / 回避P の内訳。押すと開閉するブロック(計算タブのバフ欄くらいの開閉感) --------- */
  /* 開いているブロックだけが残りの高さを取る。閉じたブロックが flex: 1 のままだと空の余白が残る(実機で検出) */
  /* 命中P / 回避P はそれぞれ 1 枚のカード(app.css の .card と同じ面)。破線 1 本の区切りだと
     手の面の直後に次の頭が来て、どこまでが命中P か読めない(ユーザー指摘 2026-09-02) */
  .stat-block {
    display: flex; flex-direction: column; flex: 0 0 auto;
    padding: 5px 10px 8px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-strong);
  }
  .stat-block:has(.stat-body) .stat-row.main { border-bottom: 1px solid var(--border-soft); margin-bottom: 3px; }
  .stat-block:has(.stat-body) { flex: 1 1 auto; min-height: 0; }
  /* 行の高さは固定。的中剣チップの行だけ高くなると 2 列の命中P / 回避P の段がずれる(実機で 3px 検出) */
  .stat-row { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 0 2px; height: 21px; box-sizing: border-box; }
  /* スキルの Picker が入る行は 21px に収まらない。両列とも同じ行なので段はずれない */
  .stat-row.with-picker { height: 30px; }
  .stat-row.with-picker :global(.picker-trigger) { padding: 2px 8px; }
  .stat-label { display: flex; align-items: center; gap: 4px; font-size: 10.5px; color: var(--fg-sub); white-space: nowrap; }
  .stat-val { text-align: right; min-width: 0; }
  .stat-val :global(.num), .stat-val :global(.unk) { font-size: 11px; }
  .stat-row.main .stat-label { font-size: 11.5px; font-weight: 800; color: var(--fg-head); min-width: 0; }
  /* 畳んだときの要約(スキル名・的中剣 / AGI)。頭の行の高さは変えない */
  .stat-summary { margin-left: 6px; font-size: 10px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .stat-row.main .stat-val :global(.num) { font-size: 15px; font-weight: 800; color: var(--fg-head); }
  /* 頭の行(押すと開閉)。押した行自体は動かない(§00 03) */
  .stat-toggle { width: 100%; border: 0; background: none; font: inherit; cursor: pointer; }
  .stat-toggle:hover .stat-label { color: var(--accent); }
  .stat-caret { display: inline-block; width: 9px; font-size: 9px; color: var(--fg-dim); transition: transform 0.15s ease; }
  .stat-caret.open { transform: rotate(90deg); }
  .stat-body { flex: 1; min-height: 0; display: flex; flex-direction: column; }

  /* 的中剣の ON / OFF。押した場所は動かない(幅・高さは状態で変えない) */
  .sword-chip {
    font: inherit; font-size: 10px; font-weight: 700; cursor: pointer;
    border: 1px solid var(--border); border-radius: var(--r-pill);
    background: var(--bg-field); color: var(--fg-dim);
    padding: 2px 9px; white-space: nowrap;
  }
  .sword-chip.on { border-color: var(--accent); background: var(--bg-active); color: var(--fg-head); }
  .sword-chip:hover { border-color: var(--accent); }
  .sword-chip:focus-visible { outline: 1px solid var(--accent); outline-offset: 1px; }
  .sword-state { display: inline-block; min-width: 3ch; text-align: center; margin-left: 4px; font-size: 9px; letter-spacing: .06em; }

  /* 「全部やると」〜「手の面」の段 ------------------------------------------------- */
  .try-section { display: flex; flex-direction: column; gap: 3px; flex: 1; min-height: 0; }
  .try-head { display: flex; align-items: center; justify-content: flex-end; gap: 8px; height: 20px; box-sizing: border-box; }
  .try-head-summary {
    min-width: 0; font-size: 10px; color: var(--fg-sub);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  /* 状態行(常設・高さ固定)。試しの有無で下の一覧の高さが動かないようにする */
  .try-status { height: 20px; box-sizing: border-box; display: flex; align-items: center; }
  .try-status-on {
    display: inline-flex; align-items: center; gap: 5px; min-width: 0;
    padding: 3px 8px; border-radius: var(--r-inset);
    background: var(--state-temp-bg); border: 1px solid var(--sim);
    color: var(--sim-fg); font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .try-clear {
    flex-shrink: 0; margin-left: 3px; font: inherit; font-size: 9px; font-weight: 700;
    color: var(--sim-fg); text-decoration: underline; text-underline-offset: 2px;
  }
  .try-clear:hover { color: var(--sim-strong); }

  /* 固定高さの中でスクロールする手の一覧 */
  .try-list {
    /* height: 0 が要る。auto だと中身全部の高さが親の最小高さに効き、列が窓より伸びて一覧が縮まない
       (実機で検出)。0 + min-height で「最小 72px、余りがあれば伸びる」になる */
    flex: 1 1 auto; height: 0; min-height: 72px; overflow-y: auto; overscroll-behavior: contain;
    border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--bg-panel);
    padding: 4px 8px 6px;
  }
  .try-empty { padding: 14px; text-align: center; font-size: 10.5px; }
  .try-group-label {
    display: flex; align-items: baseline; justify-content: space-between; gap: 8px;
    padding: 6px 1px 3px; font-size: 9px; font-weight: 700; letter-spacing: 0.06em; color: var(--fg-muted);
  }
  .try-group-gain { letter-spacing: 0; font-weight: 800; }
  /* 何を積んでも率が動かないとき。手は薄く(押せる)、反映中の手だけ元の濃さ */
  .try-list.stuck .try-chip:not(.on) { opacity: 0.55; }
  /* 最終手段(エンチャント)は末尾に薄く。並びは Rust が決めているので、
     ここでやるのは「目立たせない」ことだけ(§00 02) */
  .try-group-label.last-resort { color: var(--fg-off); }

  /* 区分ごとにチップが折り返して並ぶ。計算タブの .buff-chip と同じ配色・状態バッジ */
  .try-chips { display: flex; flex-wrap: wrap; gap: 4px; padding-bottom: 2px; }
  .try-chip {
    display: flex; align-items: center; gap: 6px; max-width: 100%;
    padding: 3px 7px; border-radius: var(--r-inset);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    font-size: 10px; font-weight: 500; color: var(--fg-muted); cursor: pointer;
  }
  .try-chip:hover { border-color: var(--sim); }
  .try-chip-copy { min-width: 0; max-width: 190px; display: flex; flex-direction: column; justify-content: center; gap: 1px; overflow: hidden; }
  .try-chip-copy > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .try-chip-note { font-size: 8.5px; font-weight: 700; }
  .try-chip.on {
    background: var(--state-temp-bg); border-color: var(--sim); color: var(--sim-fg); font-weight: 700;
  }
  .try-chip-gain { flex: none; font-size: 10px; font-weight: 800; }
  /* 状態バッジの枠は常に確保する。付いた瞬間にチップが伸びると隣のチップの折り返しが動く(§09 規則 4) */
  .try-chip-state {
    flex: none; display: inline-block; min-width: 30px; text-align: center;
    padding: 0 5px; border-radius: var(--r-pill);
    background: transparent; border: 1px solid transparent;
    font-size: 8.5px; font-weight: 700; color: transparent;
  }
  .try-chip-state.on {
    background: rgba(255, 255, 255, 0.75); border-color: currentColor; color: inherit;
  }
</style>
