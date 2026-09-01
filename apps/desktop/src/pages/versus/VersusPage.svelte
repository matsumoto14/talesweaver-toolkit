<script lang="ts">
  // 対人: 2 人のキャラ(+使用スキル)を選び、両方向の命中率(A→B / B→A)を出す画面。
  // 攻撃側・防御側という役割は選ばせない ── 知りたいのは「殴り合ったらどっちがどれだけ当たるか」
  // であって、役割の入れ替え操作そのものが不要(ユーザー指摘 2026-09-01)。
  // 計算は Rust 側(preview_versus → domain::versus_accuracy)。ここは組み立てて渡すだけ。
  import { errorMessage, listSkills, previewVersus } from "../../api/commands";
  import type { AccuracyBoost, Skill, VersusAccuracy } from "../../api/types";
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
      requestLatest.run(async (isCurrent) => {
        try {
          const r = await previewVersus(
            payloadOf(a), buffSelectionFor(a), sid, payloadOf(d), buffSelectionFor(d),
          );
          if (isCurrent()) {
            result = r;
            error = null;
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

  // 押した行は動かない(§09 規則 2)。開いた行の直下にだけ内訳が生える
  let openDetail = $state<"AB" | "BA" | null>(null);
  function toggleDetail(key: "AB" | "BA") {
    openDetail = openDetail === key ? null : key;
  }

  // 必中かどうかの判定は domain(HitRate::capped)が持つ。ここで raw >= max を書き直さない
  function hitBadge(result: VersusAccuracy | null) {
    if (result === null) return { label: "?", state: "unknown" as const };
    return result.hit_rate.capped
      ? { label: "必中", state: "goal" as const }
      : { label: "命中率", state: "met" as const };
  }

  function boostLabel(boost: AccuracyBoost): string | null {
    if (boost === "none") return null;
    if (boost === "concentration") return "ペット集中 ・ 命中P ×1.05";
    return `的中剣 Lv${boost.precision_sword} ・ 命中P ×1.35`;
  }
</script>

{#snippet directionRow(
  key: "AB" | "BA",
  attacker: (typeof app.characters)[number] | null,
  defender: (typeof app.characters)[number] | null,
  dir: { result: VersusAccuracy | null; error: string | null },
)}
  {@const result = dir.result}
  {@const badge = hitBadge(result)}
  {@const open = openDetail === key}
  <div class="dir" class:has-result={result !== null}>
    <button
      type="button" class="dir-row" aria-expanded={open}
      disabled={!attacker || !defender}
      onclick={() => toggleDetail(key)}
    >
      <span class="dir-who">
        {#if attacker}
          <Icon kind="character" id={attacker.game_character_id} size={20} label={attacker.name} source={app.characterIcons[attacker.id] ?? null} />
          <span class="dir-name">{attacker.name}</span>
        {/if}
        <span class="dir-arrow">→</span>
        {#if defender}
          <Icon kind="character" id={defender.game_character_id} size={20} label={defender.name} source={app.characterIcons[defender.id] ?? null} />
          <span class="dir-name">{defender.name}</span>
        {/if}
      </span>
      <span class="dir-value">
        {#if dir.error}
          <span class="dir-note bad">{dir.error}</span>
        {:else if result === null}
          <span class="dir-num num dim">?</span>
        {:else if result.hit_rate.capped}
          <span class="dir-cap" style={badgeStyle(badge)} use:flash={() => "capped"}>必中</span>
        {:else}
          <span class="dir-num num" use:bump={() => result?.hit_rate.value ?? null}>{result.hit_rate.value}</span>
          <span class="dir-unit">%</span>
        {/if}
      </span>
      <span class="dir-caret dim" aria-hidden="true">{open ? "▾" : "▸"}</span>
    </button>

    {#if open && result}
      {@const boost = boostLabel(result.accuracy_boost)}
      <div class="dir-detail open-in">
        <div class="raw-line">
          命中P <span class="num strong" use:bump={() => result?.accuracy_point ?? null}>{result.accuracy_point}</span>
          <span class="op">−</span> 回避P <span class="num strong" use:bump={() => result?.evasion_point ?? null}>{result.evasion_point}</span>
          <span class="op">=</span> <span class="num strong" use:bump={() => result?.hit_rate.raw ?? null}>{result.hit_rate.raw}</span>
        </div>

        <div class="range-line dim">
          下限 <span class="num">{result.hit_rate.min}</span>
          〜 上限 <span class="num">{result.hit_rate.max}</span>
          <span class="range-detail">
            (15 + 最小命中率補正 {#if result.min_rates_recorded}<span class="num">0</span>{:else}<span class="unk">?</span>{/if}
            − 最小回避率補正 {#if result.min_rates_recorded}<span class="num">0</span>{:else}<span class="unk">?</span>{/if})
          </span>
        </div>

        {#if boost}
          <div class="boost-line">
            <span>{boost}</span>
            {#if !result.accuracy_boost_shift_recorded}
              <span class="unk">命中P変動 ?</span>
            {/if}
          </div>
        {/if}

        <div class="breakdown">
          <div class="side">
            <div class="side-title">攻撃側</div>
            <div class="row"><span class="label">DEX</span><span class="num">{result.attacker_dex}</span></div>
            <div class="row"><span class="label">装備命中</span><span class="num">{result.equipment_accuracy}</span></div>
            <div class="row"><span class="label">スキル命中</span><span class="num">{result.skill_accuracy}</span></div>
            <div class="row"><span class="label">依存ボーナス / ペナルティ</span><span class="num">+{result.correction_bonus} / −{result.correction_penalty}</span></div>
          </div>
          <div class="side">
            <div class="side-title">防御側({result.attack_type === "physical" ? "物理" : "魔法"}回避P)</div>
            <div class="row"><span class="label">AGI</span><span class="num">{result.defender_agi}</span></div>
            <div class="row"><span class="label">装備回避率</span><span class="num">{result.equipment_evasion}</span></div>
            <div class="row"><span class="label">装備敏捷度</span><span class="num">{result.equipment_agility}</span></div>
            <div class="row"><span class="label">攻撃タイプ別増加</span><span class="num">{result.attack_type_bonus.toFixed(1)}</span></div>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/snippet}

<div class="versus-page">
  <div class="scroll">
    {#if app.characters.length < 2}
      <p class="empty dim">対人にはキャラが 2 人以上必要です。キャラタブで登録してください。</p>
    {:else}
      <div class="sides">
        <!-- 2 人は対等。帯の色を変えると役割の違いに読める(§02 金は「行ける?」の意味を持つ) -->
        <SheetCard tone="blue" title="キャラ 1" note="命中Pに使うスキルも選ぶ">
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

        <SheetCard tone="blue" title="キャラ 2" note="命中Pに使うスキルも選ぶ">
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

      <div class="directions">
        {#if !charA || !charB}
          <p class="empty dim">キャラを 2 人とも選んでください。</p>
        {:else}
          {@render directionRow("AB", charA, charB, resultAB)}
          {@render directionRow("BA", charB, charA, resultBA)}
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .versus-page { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 820px; }
  .empty { font-size: 12px; }

  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; align-items: start; }
  .side-body { display: flex; flex-direction: column; gap: 10px; padding: 11px 13px 13px; }

  .directions { display: flex; flex-direction: column; gap: 10px; }

  /* 未計算(キャラ/スキル未選択・エラー)は破線 + ? のまま(design-system 原則) */
  .dir {
    border: 1px dashed var(--state-unknown-bd); border-radius: var(--r-window);
    background: var(--state-unknown-bg);
  }
  .dir.has-result {
    border: 1px solid var(--border); background: var(--bg-field);
  }

  /* 押した行は動かない。中身は横一列のまま、開くと直下に内訳が生える(§09 規則 2) */
  .dir-row {
    width: 100%; display: flex; align-items: center; gap: 10px;
    padding: 10px 13px; background: transparent; border: none; cursor: pointer;
    font: inherit; color: inherit; text-align: left;
  }
  .dir-row:disabled { cursor: default; }
  .dir-row:not(:disabled):hover { background: var(--bg-active); }

  .dir-who { display: flex; align-items: center; gap: 6px; min-width: 0; flex: 1; font-size: 12px; font-weight: 700; color: var(--fg-head); }
  .dir-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dir-arrow { flex-shrink: 0; font-size: 12px; font-weight: 700; color: var(--fg-dim); margin: 0 2px; }

  /* 主役: 命中率 %。桁が増えても隣が動かないよう幅を固定する(§09 規則 4) */
  .dir-value { flex-shrink: 0; display: flex; align-items: baseline; gap: 3px; min-width: 68px; justify-content: flex-end; }
  .dir-num { font-size: 20px; font-weight: 800; color: var(--fg-head); line-height: 1; }
  .dir-unit { font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .dir-cap {
    font-size: 10.5px; font-weight: 800; border-radius: var(--r-pill); padding: 3px 9px; border: 1px solid;
  }
  .dir-note { font-size: 10px; }
  .dir-note.bad { color: var(--danger); font-weight: 700; }
  .dir-caret { flex: none; width: 10px; text-align: center; font-size: 9px; }

  .dir-detail { padding: 2px 13px 14px; display: flex; flex-direction: column; align-items: center; gap: 8px; }

  .raw-line { font-size: 12px; color: var(--fg-sub); display: flex; align-items: baseline; gap: 5px; }
  .raw-line .op { color: var(--fg-dim); }
  .raw-line .num.strong { font-weight: 800; color: var(--fg-head); min-width: 34px; display: inline-block; text-align: right; }

  .range-line { font-size: 10.5px; display: flex; flex-wrap: wrap; align-items: center; gap: 4px; justify-content: center; }
  .range-detail { display: inline-flex; align-items: center; gap: 3px; flex-wrap: wrap; }

  .boost-line {
    font-size: 10.5px; font-weight: 700; color: var(--fg-sub); display: flex; align-items: center; gap: 6px;
  }

  /* 未収録(供給源が無いのでまだ 0 決め打ち)。0 や空白ではなく ? + 破線で示す */
  .unk {
    display: inline-block; padding: 0 5px; border: 1px dashed var(--state-unknown-bd);
    border-radius: var(--r-pill); color: var(--state-unknown-fg); font-size: 9.5px; font-weight: 700;
  }

  .breakdown { width: 100%; display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 4px; }
  .side {
    border: 1px solid var(--border-soft); border-radius: var(--r-panel); background: var(--surface-inset);
    padding: 8px 10px; display: flex; flex-direction: column; gap: 4px;
  }
  .side-title { font-size: 9.5px; font-weight: 800; color: var(--fg-dim); margin-bottom: 2px; }
  .side .row { display: flex; align-items: baseline; gap: 8px; font-size: 10.5px; }
  .side .row .label { min-width: 0; flex: 1; color: var(--fg-sub); }
  .side .row .num { min-width: 40px; text-align: right; font-weight: 700; color: var(--fg-head); }
</style>
