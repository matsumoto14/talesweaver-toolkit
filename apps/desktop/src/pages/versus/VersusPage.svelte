<script lang="ts">
  // 対人: 攻撃側キャラ(+スキル)と防御側キャラを選び、命中率を出す画面。
  // 計算は Rust 側(preview_versus → domain::versus_accuracy)。ここは組み立てて渡すだけ。
  import { errorMessage, listSkills, previewVersus } from "../../api/commands";
  import type { AccuracyBoost, Skill, VersusAccuracy } from "../../api/types";
  import { app, buffSelectionFor, gameCharacterName, payloadOf } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { badgeStyle, STATE } from "../../ui/states";
  import { bump, flash } from "../../ui/motion.svelte";
  import { latest } from "../../ui/latest.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Picker from "../../ui/Picker.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";

  // --- 攻撃側 -----------------------------------------------------------
  // 既定 = いま選択中のキャラ(§ux 原則: 登録キャラデータを軸にする)
  let attackerOverride = $state<number | null>(null);
  const attackerId = $derived(
    app.characters.some((c) => c.id === attackerOverride) ? attackerOverride : app.selectedId,
  );
  const attacker = $derived(app.characters.find((c) => c.id === attackerId) ?? null);

  // --- 防御側 -----------------------------------------------------------
  // 既定 = 攻撃側以外の先頭キャラ(1 人しか登録が無ければ未選択のまま)
  let defenderOverride = $state<number | null>(null);
  const defaultDefenderId = $derived(
    app.characters.find((c) => c.id !== attackerId)?.id ?? null,
  );
  const defenderId = $derived(
    app.characters.some((c) => c.id === defenderOverride) ? defenderOverride : defaultDefenderId,
  );
  const defender = $derived(app.characters.find((c) => c.id === defenderId) ?? null);

  // **相手側のキャラも候補から外さない**。外すと「防御側に入っているキャラを攻撃側にしたい」
  // ときに、先に防御側を変えないと選べず、行き止まりになる(ユーザー指摘 2026-09-01)。
  // 同じキャラを選んだら 2 人を入れ替える(pickAttacker / pickDefender)
  const characterOptions = () =>
    app.characters
      .map((c) => ({
        value: String(c.id),
        name: c.name,
        meta: gameCharacterName(c.game_character_id),
        iconId: c.game_character_id,
        iconKind: "character" as const,
      }));

  // --- 攻撃側のスキル(キャラタブの主軸スキルが正。CalcPage と同じ組み方) --------
  let skills = $state<Skill[]>([]);
  let skillsGid: string | null = null;
  $effect(() => {
    const gid = attacker?.game_character_id ?? null;
    if (gid === skillsGid) return;
    skillsGid = gid;
    skills = [];
    skillOverride = null;
    if (!gid) return;
    listSkills(gid)
      .then((list) => {
        if (skillsGid !== gid) return;
        skills = list;
      })
      .catch((e) => reportError(errorMessage(e)));
  });
  let skillOverride = $state<string | null>(null);
  const mainSkill = $derived(skills.find((s) => s.id === attacker?.main_skill_id) ?? null);
  const skillId = $derived(
    (skillOverride !== null && skills.some((s) => s.id === skillOverride) ? skillOverride : null)
      ?? mainSkill?.id
      ?? skills[0]?.id
      ?? "",
  );
  const skillOptions = $derived(
    skills.map((s) => ({
      value: s.id,
      name: s.name,
      meta: s.accuracy !== null ? `命中 ${s.accuracy}` : "命中 未記載",
      iconId: s.id,
      iconKind: "skill" as const,
    })),
  );

  /** 攻撃側を選ぶ。防御側に入っているキャラを選んだら 2 人を入れ替える */
  function pickAttacker(id: number | null) {
    const before = attackerId;
    if (id !== null && id === defenderId) defenderOverride = before;
    attackerOverride = id;
  }
  /** 防御側を選ぶ。攻撃側に入っているキャラを選んだら 2 人を入れ替える */
  function pickDefender(id: number | null) {
    const before = defenderId;
    if (id !== null && id === attackerId) attackerOverride = before;
    defenderOverride = id;
  }

  // --- 命中率(preview_versus) -------------------------------------------
  let result = $state<VersusAccuracy | null>(null);
  let resultError = $state<string | null>(null);
  const versusLatest = latest({ debounce: 150 });
  $effect(() => {
    const a = attacker;
    const d = defender;
    const sid = skillId;
    if (!a || !d || !sid) {
      versusLatest.cancel();
      result = null;
      resultError = null;
      return;
    }
    versusLatest.run(async (isCurrent) => {
      try {
        const r = await previewVersus(
          payloadOf(a), buffSelectionFor(a), sid, payloadOf(d), buffSelectionFor(d),
        );
        if (isCurrent()) {
          result = r;
          resultError = null;
        }
      } catch (e) {
        if (isCurrent()) {
          result = null;
          resultError = errorMessage(e);
        }
      }
    });
    return () => versusLatest.cancel();
  });

  // 必中かどうかの判定は domain(HitRate::capped)が持つ。ここで raw >= max を書き直さない
  const capped = $derived(result?.hit_rate.capped ?? false);
  const hitBadge = $derived(
    result === null
      ? { label: "?", state: "unknown" as const }
      : capped
        ? { label: "必中", state: "goal" as const }
        : { label: "命中率", state: "met" as const },
  );

  function boostLabel(boost: AccuracyBoost): string | null {
    if (boost === "none") return null;
    if (boost === "concentration") return "ペット集中 ・ 命中P ×1.05";
    return `的中剣 Lv${boost.precision_sword} ・ 命中P ×1.35`;
  }
</script>

<div class="versus-page">
  <div class="scroll">
    {#if app.characters.length < 2}
      <p class="empty dim">対人にはキャラが 2 人以上必要です。キャラタブで登録してください。</p>
    {:else}
      <div class="sides">
        <SheetCard tone="gold" title="攻撃側" note="命中Pに使うスキルも選ぶ">
          <div class="side-body">
            <Picker
              label="攻撃側キャラ"
              bind:value={
                () => (attackerId !== null ? String(attackerId) : ""),
                (v) => pickAttacker(v === "" ? null : Number(v))
              }
              options={characterOptions()}
              placeholder="キャラを選択してください"
            />
            {#if attacker}
              <Picker
                label="使用スキル"
                bind:value={
                  () => skillId,
                  (v) => (skillOverride = v)
                }
                options={skillOptions}
                placeholder="スキルを選択してください"
                disabled={skills.length === 0}
              />
            {/if}
          </div>
        </SheetCard>

        <SheetCard tone="blue" title="防御側" note="回避Pの相手">
          <div class="side-body">
            <Picker
              label="防御側キャラ"
              bind:value={
                () => (defenderId !== null ? String(defenderId) : ""),
                (v) => pickDefender(v === "" ? null : Number(v))
              }
              options={characterOptions()}
              placeholder="キャラを選択してください"
            />
          </div>
        </SheetCard>
      </div>

      <div class="result" class:has-result={result !== null}>
        <div class="result-head">
          <span class="badge" style={badgeStyle(hitBadge)}>{result === null ? "?" : capped ? "◎" : "%"}</span>
          <span class="result-title">命中率</span>
          {#if resultError}
            <span class="result-note bad">{resultError}</span>
          {:else if !result}
            <span class="result-note dim">攻撃側・防御側・使用スキルを選んでください</span>
          {/if}
        </div>
        <div class="result-body">
          <div class="matchup">
            {#if attacker}
              <span class="who">
                <Icon kind="character" id={attacker.game_character_id} size={20} label={attacker.name} source={app.characterIcons[attacker.id] ?? null} />
                {attacker.name}
              </span>
            {/if}
            <span class="vs">vs</span>
            {#if defender}
              <span class="who">
                <Icon kind="character" id={defender.game_character_id} size={20} label={defender.name} source={app.characterIcons[defender.id] ?? null} />
                {defender.name}
              </span>
            {/if}
          </div>

          {#if result}
            {@const boost = boostLabel(result.accuracy_boost)}
            <div class="hit-value">
              <span class="hit-num num" use:bump={() => result?.hit_rate.value ?? null}>{result.hit_rate.value}</span>
              <span class="hit-unit">%</span>
              {#if capped}<span class="hit-capped" use:flash={() => "capped"}>必中(上限に到達)</span>{/if}
            </div>

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
          {:else if !resultError}
            <div class="placeholder-value num" style="color: {STATE.unknown.fg};">?</div>
          {/if}
        </div>
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

  /* 結果面。未計算(キャラ/スキル未選択・エラー)は破線 + ? のまま(design-system 原則) */
  .result {
    border: 1px dashed var(--state-unknown-bd); border-radius: var(--r-window);
    background: var(--state-unknown-bg);
  }
  .result.has-result {
    border: 1px solid var(--border); background: var(--bg-field);
  }
  .result-head {
    display: flex; align-items: center; gap: 8px; padding: 8px 13px;
    border-bottom: 1px dashed var(--state-unknown-bd);
  }
  .result.has-result .result-head { border-bottom: 1px solid var(--border-soft); }
  .result-head .badge {
    flex-shrink: 0; width: 18px; height: 18px; display: grid; place-items: center;
    border-radius: 50%; border: 1px solid; font-size: 10px; font-weight: 800;
  }
  .result-title { font-size: 12px; font-weight: 800; color: var(--fg-head); }
  .result-note { min-width: 0; flex: 1; text-align: right; font-size: 10px; }
  .result-note.bad { color: var(--danger); font-weight: 700; }
  .result-body { padding: 12px 13px 16px; display: flex; flex-direction: column; align-items: center; gap: 8px; }
  .matchup { display: flex; align-items: center; gap: 10px; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .who { display: flex; align-items: center; gap: 6px; }
  .vs { font-size: 9.5px; font-weight: 700; color: var(--fg-dim); }
  .placeholder-value { font-size: 34px; font-weight: 800; line-height: 1; }

  /* 主役: 命中率 %。桁が増えても隣が動かないよう min-width を固定する(§09 規則4) */
  .hit-value { display: flex; align-items: baseline; gap: 8px; margin-top: 2px; }
  .hit-num { font-size: 40px; font-weight: 800; line-height: 1; color: var(--fg-head); min-width: 92px; text-align: right; }
  .hit-unit { font-size: 16px; font-weight: 700; color: var(--fg-sub); }
  .hit-capped {
    font-size: 10px; font-weight: 800; color: var(--state-goal-fg);
    background: var(--state-goal-bg); border: 1px solid var(--state-goal-bd);
    border-radius: var(--r-pill); padding: 2px 8px;
  }

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
