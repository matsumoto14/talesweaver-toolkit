<script lang="ts">
  // ダメージ・DPS の質問の打ち手(2026-09-24、ADR-020)。wiki には計算式も装備もバフも無いので
  // サーバーは「damage_calc」という印だけを返し、この部品が**アプリのドメイン知識**で答える。
  //
  // 答えは 2 段で、上から取れる方を出す(§入力の 5 形態と同じ考え方: 自動で出せるなら聞かない):
  //   1. 質問のキャラが登録済み → 実際の値(合計 DPS・差し込みの損得)。Rust の回しそのまま
  //   2. 未登録 → 静的データだけ(スキルの継続火力 / 1 回ぶんの火力)。**キャラ登録が要らない**
  // 「キャラを登録しないと何も出せない」にはしない —— 技の性能は登録と無関係に分かる(ユーザー判断 2026-09-24)。
  import { listRotationChoices, previewDamage } from "../../api/commands";
  import type { RotationInsertChoice, Skill } from "../../api/types";
  import { fmtInt, fmtNum } from "../../format";
  import {
    app,
    buffSelectionFor,
    currentDamageTarget,
    loadSkills,
    payloadOf,
    selectedCharacter,
    skillsByCharacter,
  } from "../../state.svelte";
  import { latest } from "../../ui/latest.svelte";
  import Value from "../../ui/Value.svelte";
  import {
    DAMAGE_CALC_FILLER_LABEL,
    DAMAGE_CALC_GENERIC_NOTE,
    DAMAGE_CALC_INSERT_LABEL,
    DAMAGE_CALC_NOTE,
    DAMAGE_CALC_NO_SKILLS,
    DAMAGE_CALC_REGISTER_NOTE,
    DAMAGE_CALC_TITLE,
    PLAYBOOK_GO_TO_CALC,
    PLAYBOOK_TAG,
  } from "./lines";

  interface Props {
    /** 聞かれた文。ここからどのゲームキャラの話かを引く(登録キャラに頼らない) */
    question: string;
  }
  let { question }: Props = $props();

  /** 表に出す件数。多いと「読む表」になる(§00②) */
  const TOP_N = 3;

  /** 質問文に名前が出てくるゲームキャラ(長い名前を優先)。出てこなければ null */
  const askedCharacter = $derived(
    [...app.gameCharacters]
      .sort((a, b) => b.name.length - a.name.length)
      .find((g) => question.includes(g.name)) ?? null,
  );

  /** 答えるキャラ。質問で名指しされたキャラを優先し、無ければ選択中の登録キャラのキャラ */
  const selected = $derived(selectedCharacter());
  const gameCharacterId = $derived(askedCharacter?.id ?? selected?.game_character_id ?? null);
  const gameCharacterName = $derived(
    askedCharacter?.name ?? app.gameCharacters.find((g) => g.id === gameCharacterId)?.name ?? null,
  );

  /** 実際の値を出せる登録キャラ(そのゲームキャラで登録済みのもの。選択中を優先) */
  const character = $derived(
    gameCharacterId === null
      ? null
      : (selected?.game_character_id === gameCharacterId ? selected : null) ??
        app.characters.find((c) => c.game_character_id === gameCharacterId) ??
        null,
  );

  const skills = $derived(gameCharacterId === null ? [] : (skillsByCharacter[gameCharacterId] ?? []));
  /** 継続火力の目安。未収録なら 1 回ぶんの火力で比べる(StatusPane の自動選択と同じ規則) */
  const rate = (s: Skill) => s.power_per_second ?? s.power;
  /** 連打する技の候補(CT が無い = 連打できる)。継続火力順 */
  const fillers = $derived(
    skills
      .filter((s) => s.cooldown_seconds === null && s.attacker === "player" && !s.normal_attack)
      .sort((a, b) => rate(b) - rate(a))
      .slice(0, TOP_N),
  );
  /** 差し込む技の候補(CT がある)。1 回ぶんの火力順 —— 連打しないので継続火力では比べられない */
  const inserts = $derived(
    skills
      .filter((s) => s.cooldown_seconds !== null && s.attacker === "player")
      .sort((a, b) => b.power - a.power)
      .slice(0, TOP_N),
  );

  /** 合計 DPS(登録キャラがあるときだけ)。計算できないうちは null で `?` のまま */
  let expectedDps = $state<number | null>(null);
  /** 実際に差し込むと DPS が上がる技(Rust の回しが返した損得順)。登録キャラがあるときだけ */
  let gains = $state<RotationInsertChoice[]>([]);
  /** 実データで連打する技の名前(主軸、または主軸が CT 技なら自動で決まる合間の技) */
  let fillerName = $state<string | null>(null);
  const requestLatest = latest();

  $effect(() => {
    const id = gameCharacterId;
    if (id) void loadSkills(id);

    const c = character;
    const t = currentDamageTarget();
    expectedDps = null;
    gains = [];
    fillerName = null;
    const skillId = c?.main_skill_id;
    if (!c || !t || !skillId) return;
    const buffs = buffSelectionFor(c);
    const payload = payloadOf(c);
    requestLatest.run(async (isCurrent) => {
      try {
        const [damage, choices] = await Promise.all([
          previewDamage(payload, skillId, t.id, 0, null, null, buffs),
          listRotationChoices(payload, t.id, buffs, skillId),
        ]);
        if (!isCurrent()) return;
        expectedDps = damage.combined.expected_dps;
        fillerName = choices.filler_skill_name ?? skills.find((s) => s.id === skillId)?.name ?? null;
        gains = choices.candidates
          .filter((x) => x.effect === "improves" && x.expected_dps_gain !== null)
          .sort((a, b) => (b.expected_dps_gain ?? 0) - (a.expected_dps_gain ?? 0))
          .slice(0, TOP_N);
      } catch {
        // 未収録・失敗は静的データの段(下)だけで答える。導線は残る
      }
    });
    return () => requestLatest.cancel();
  });

  function goToCalc() {
    app.tab = "calc";
  }
</script>

<div class="handoff pop-in">
  <p class="intro">
    <span class="line">{gameCharacterName ? `${gameCharacterName}の${DAMAGE_CALC_TITLE}` : DAMAGE_CALC_TITLE}</span>
    <span class="tag code">{PLAYBOOK_TAG} コード</span>
  </p>

  {#if skills.length === 0}
    <p class="muted">{DAMAGE_CALC_NO_SKILLS}</p>
  {:else}
    {#if character && expectedDps !== null}
      <!-- 登録キャラがあるときは実際の値。回しは Rust が組んだものをそのまま出す -->
      <div class="values">
        <span class="dv"><span class="k">いまの合計 DPS</span><Value value={fmtInt(expectedDps)} class="v" /></span>
      </div>
    {/if}

    <div class="lists">
      <div class="list">
        <p class="list-title">{DAMAGE_CALC_FILLER_LABEL}</p>
        <ul>
          {#if character && fillerName}
            <li><span class="name">{fillerName}</span><span class="meta">いまの主軸</span></li>
          {/if}
          {#each fillers as s (s.id)}
            <li>
              <span class="name">{s.name}</span>
              <span class="meta">
                {s.hit_count} 段
                {#if s.base_actual_delay !== null} ・ 中 {fmtNum(s.base_actual_delay, 2, "s")}{/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>

      <div class="list">
        <p class="list-title">{DAMAGE_CALC_INSERT_LABEL}</p>
        <ul>
          {#if gains.length > 0}
            {#each gains as g (g.skill_id)}
              <li>
                <span class="name">{g.skill_name}</span>
                <span class="meta">
                  {g.is_field ? "持続" : "CT"} {fmtNum(g.cooldown_seconds, 1, "s")}
                </span>
                <Value value={`+${fmtInt(g.expected_dps_gain ?? 0)}`} class="gain" />
              </li>
            {/each}
          {:else}
            {#each inserts as s (s.id)}
              <li>
                <span class="name">{s.name}</span>
                <span class="meta">
                  {s.hit_count} 段
                  {#if s.cooldown_seconds !== null} ・ {s.field ? "持続" : "CT"} {fmtNum(s.cooldown_seconds, 1, "s")}{/if}
                </span>
              </li>
            {/each}
          {/if}
        </ul>
      </div>
    </div>

    <p class="muted">{character ? DAMAGE_CALC_NOTE : DAMAGE_CALC_GENERIC_NOTE}</p>
  {/if}

  <p class="footer">
    {character ? PLAYBOOK_TAG : DAMAGE_CALC_REGISTER_NOTE}
    {#if character}
      <button type="button" class="link" onclick={goToCalc}>{PLAYBOOK_GO_TO_CALC}</button>
    {/if}
  </p>
</div>

<style>
  .handoff {
    display: flex; flex-direction: column; gap: 8px; padding: 8px 10px; border-radius: var(--r-inset);
    background: var(--bg-panel); border: 1px solid var(--border-soft);
  }
  .intro { margin: 0; font-size: 10.5px; font-weight: 700; color: var(--fg-head); }
  .tag { margin-left: 6px; font-size: 8px; font-weight: 700; border-radius: var(--r-inset); padding: 0 5px; vertical-align: middle; }
  .tag.code { background: var(--state-met-bg); color: var(--state-met-fg); }
  .muted { margin: 0; font-size: 9.5px; color: var(--fg-muted); }

  .values { display: flex; flex-wrap: wrap; gap: 10px; }
  .dv { display: flex; align-items: baseline; gap: 4px; font-size: 9.5px; }
  .dv .k { color: var(--fg-muted); }
  .dv :global(.v) { font-weight: 700; min-width: 4em; text-align: right; }

  .lists { display: flex; flex-direction: column; gap: 8px; }
  .list { display: flex; flex-direction: column; gap: 3px; }
  .list-title { margin: 0; font-size: 10px; font-weight: 700; color: var(--fg-head); }
  .list ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 3px; }
  .list li { display: flex; align-items: baseline; gap: 6px; font-size: 9.5px; color: var(--fg-sub); }
  .name { flex: none; font-weight: 700; color: var(--fg); }
  .meta { flex: 1; min-width: 0; color: var(--fg-muted); }
  .list :global(.gain) { flex: none; font-weight: 700; min-width: 3.5em; text-align: right; color: var(--state-met-fg); }

  .footer { margin: 0; font-size: 9px; color: var(--fg-dim); display: flex; align-items: center; gap: 6px; }
  .link { background: none; border: none; padding: 0; font: inherit; color: var(--accent); cursor: pointer; text-decoration: underline; }
</style>
