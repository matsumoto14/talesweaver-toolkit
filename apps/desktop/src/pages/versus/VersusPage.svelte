<script lang="ts">
  // 対人: 2 人のキャラ(+使用スキル)を選び、両方向の命中率(A→B / B→A)を出す画面。
  // 攻撃側・防御側という役割は選ばせない ── 知りたいのは「殴り合ったらどっちがどれだけ当たるか」
  // であって、役割の入れ替え操作そのものが不要(ユーザー指摘 2026-09-01)。
  // 計算は Rust 側(preview_versus → domain::versus_accuracy)。ここは組み立てて渡すだけ。
  import { errorMessage, listSkills, previewVersus } from "../../api/commands";
  import type { AccuracyBoost, GrowthRoom, GrowthSource, Skill, VersusAccuracy } from "../../api/types";
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

  // --- 命中P・回避Pの伸びしろ(accuracy_growth / evasion_growth) --------------------
  // 材料の出どころ(GrowthSource)を軸に、2 人ぶんを同じ行へ揃える。並び順はこの固定順
  // (Rust 側は gain 降順で返すので、そのままだと側ごとに順番がずれて突き合わせにくい)。
  const GROWTH_SOURCE_ORDER: GrowthSource[] = ["stat", "enchant", "siena", "precision_sword"];

  interface GrowthRow { source: GrowthSource; label: string; a: GrowthRoom | null; b: GrowthRoom | null }

  function mergeGrowth(a: GrowthRoom[], b: GrowthRoom[]): GrowthRow[] {
    return GROWTH_SOURCE_ORDER.flatMap((source) => {
      const ai = a.find((g) => g.source === source) ?? null;
      const bi = b.find((g) => g.source === source) ?? null;
      if (!ai && !bi) return [];
      return [{ source, label: (ai ?? bi)!.label, a: ai, b: bi }];
    });
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

{#snippet growthSummaryCell(result: VersusAccuracy | null, growthCount: number, max: number | null, point: number | null)}
  {#if result === null || max === null || point === null}
    <span class="unk">?</span>
  {:else if growthCount === 0}
    <span class="growth-none dim">伸びしろなし</span>
  {:else}
    <span class="growth-total num" use:bump={() => max - point}>あと +{max - point}</span>
  {/if}
{/snippet}

{#snippet growthItemCell(item: GrowthRoom | null)}
  {#if item === null}
    <span class="growth-none dim">—</span>
  {:else}
    <span class="growth-item">
      <span class="num" use:bump={() => item.gain}>+{item.gain}</span>
      {#if item.detail}<span class="growth-detail dim">{item.detail}</span>{/if}
      {#if item.provisional}<span class="growth-provisional" style={badgeStyle({ label: "仮", state: "temp" })}>仮</span>{/if}
    </span>
  {/if}
{/snippet}

{#snippet growthBlock(
  label: string,
  resultA: VersusAccuracy | null,
  resultB: VersusAccuracy | null,
  growthA: GrowthRoom[],
  growthB: GrowthRoom[],
  maxA: number | null,
  maxB: number | null,
  pointA: number | null,
  pointB: number | null,
)}
  {@const rows = mergeGrowth(growthA, growthB)}
  <div class="grid-row sub growth-total-row">
    <div class="cell label">{label}</div>
    <div class="cell val">{@render growthSummaryCell(resultA, growthA.length, maxA, pointA)}</div>
    <div class="cell val">{@render growthSummaryCell(resultB, growthB.length, maxB, pointB)}</div>
  </div>
  {#each rows as row (row.source)}
    <div class="grid-row sub growth-item-row">
      <div class="cell label">{row.label}</div>
      <div class="cell val">{@render growthItemCell(row.a)}</div>
      <div class="cell val">{@render growthItemCell(row.b)}</div>
    </div>
  {/each}
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
    <span class="rate-who">
      {#if attacker}
        <Icon kind="character" id={attacker.game_character_id} size={20} label={attacker.name} source={app.characterIcons[attacker.id] ?? null} />
        <span class="rate-name">{attacker.name}</span>
      {/if}
      <span class="rate-arrow">→</span>
      {#if defender}
        <Icon kind="character" id={defender.game_character_id} size={20} label={defender.name} source={app.characterIcons[defender.id] ?? null} />
        <span class="rate-name">{defender.name}</span>
      {/if}
    </span>

    <span class="rate-value">
      {#if dir.error}
        <span class="rate-note bad">{dir.error}</span>
      {:else if result === null}
        <span class="rate-num num dim">?</span>
      {:else if result.hit_rate.capped}
        <span class="rate-cap" style={badgeStyle(badge)} use:flash={() => "capped"}>必中</span>
      {:else}
        <span class="rate-num num" use:bump={() => result?.hit_rate.value ?? null}>{result.hit_rate.value}</span>
        <span class="rate-unit">%</span>
      {/if}
    </span>

    {#if result}
      <span class="rate-why dim">
        (<span class="num" use:bump={() => result?.accuracy_point ?? null}>{result.accuracy_point}</span>
        <span class="op">−</span>
        <span class="num" use:bump={() => result?.evasion_point ?? null}>{result.evasion_point}</span>
        <span class="op">=</span>
        <span class="num" use:bump={() => result?.hit_rate.raw ?? null}>{result.hit_rate.raw}</span>)
        ・ 下限 <span class="num">{result.hit_rate.min}</span> 〜 上限 <span class="num">{result.hit_rate.max}</span>
        {#if !result.min_rates_recorded}<span class="unk">下限根拠 ?</span>{/if}
        {#if boost}
          ・ {boost}
          {#if !result.accuracy_boost_shift_recorded}<span class="unk">命中P変動 ?</span>{/if}
        {/if}
      </span>
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

      {#if !charA || !charB}
        <p class="empty dim">キャラを 2 人とも選んでください。</p>
      {:else}
        {@const rAB = resultAB.result}
        {@const rBA = resultBA.result}
        <!-- 突き合わせ表: 閉じたら比べられない画面なので、内訳は最初から全部出す(ユーザー指摘 2026-09-01) -->
        <div class="sheet">
          <div class="grid">
            <div class="grid-row head">
              <div class="cell label"></div>
              <div class="cell col">
                <Icon kind="character" id={charA.game_character_id} size={20} label={charA.name} source={app.characterIcons[charA.id] ?? null} />
                <span class="col-name">{charA.name}</span>
              </div>
              <div class="cell col">
                <Icon kind="character" id={charB.game_character_id} size={20} label={charB.name} source={app.characterIcons[charB.id] ?? null} />
                <span class="col-name">{charB.name}</span>
              </div>
            </div>

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
              <div class="cell label">装備命中</div>
              <div class="cell val">{@render numCell(rAB?.equipment_accuracy ?? null)}</div>
              <div class="cell val">{@render numCell(rBA?.equipment_accuracy ?? null)}</div>
            </div>
            <div class="grid-row sub">
              <div class="cell label">スキル命中</div>
              <div class="cell val">{@render numCell(rAB?.skill_accuracy ?? null)}</div>
              <div class="cell val">{@render numCell(rBA?.skill_accuracy ?? null)}</div>
            </div>
            <div class="grid-row sub">
              <div class="cell label">依存ボーナス / ペナルティ</div>
              <div class="cell val">{@render textCell(rAB ? `+${rAB.correction_bonus} / −${rAB.correction_penalty}` : null)}</div>
              <div class="cell val">{@render textCell(rBA ? `+${rBA.correction_bonus} / −${rBA.correction_penalty}` : null)}</div>
            </div>
            <div class="grid-row sub">
              <div class="cell label">的中剣</div>
              <div class="cell val">{@render textCell(rAB ? (boostLabel(rAB.accuracy_boost) ?? "なし") : null)}</div>
              <div class="cell val">{@render textCell(rBA ? (boostLabel(rBA.accuracy_boost) ?? "なし") : null)}</div>
            </div>
            {@render growthBlock(
              "伸びしろ", rAB, rBA,
              rAB?.accuracy_growth ?? [], rBA?.accuracy_growth ?? [],
              rAB?.accuracy_max ?? null, rBA?.accuracy_max ?? null,
              rAB?.accuracy_point ?? null, rBA?.accuracy_point ?? null,
            )}

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
              <div class="cell label">装備回避率</div>
              <div class="cell val">{@render numCell(rBA?.equipment_evasion ?? null)}</div>
              <div class="cell val">{@render numCell(rAB?.equipment_evasion ?? null)}</div>
            </div>
            <div class="grid-row sub">
              <div class="cell label">装備敏捷度</div>
              <div class="cell val">{@render numCell(rBA?.equipment_agility ?? null)}</div>
              <div class="cell val">{@render numCell(rAB?.equipment_agility ?? null)}</div>
            </div>
            <div class="grid-row sub">
              <div class="cell label">攻撃タイプ別増加</div>
              <div class="cell val">{@render textCell(rBA ? rBA.attack_type_bonus.toFixed(1) : null)}</div>
              <div class="cell val">{@render textCell(rAB ? rAB.attack_type_bonus.toFixed(1) : null)}</div>
            </div>
            {@render growthBlock(
              "伸びしろ", rBA, rAB,
              rBA?.evasion_growth ?? [], rAB?.evasion_growth ?? [],
              rBA?.evasion_max ?? null, rAB?.evasion_max ?? null,
              rBA?.evasion_point ?? null, rAB?.evasion_point ?? null,
            )}
          </div>
        </div>

        <div class="rates">
          {@render hitRateLine("AB", charA, charB, resultAB)}
          {@render hitRateLine("BA", charB, charA, resultBA)}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .versus-page { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 820px; }
  .empty { font-size: 12px; }

  .sides { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; align-items: start; }
  .side-body { display: flex; flex-direction: column; gap: 10px; padding: 11px 13px 13px; }

  /* 突き合わせ表: 2 人の材料を閉じずに並べる(§ux 「閉じていると比べられない」)。
     中身ぶんの幅で左に寄せる ── 幅いっぱいまで伸ばすと数値の並びが間延びして読みにくい */
  .sheet {
    width: max-content; max-width: 100%; align-self: flex-start;
    border: 1px solid var(--border); border-radius: var(--r-window); background: var(--bg-field);
    overflow: hidden;
  }
  .grid { display: flex; flex-direction: column; }
  /* 列幅は「中身が読める」ことを優先する。キャラ名が「マキシ…」で切れると、
     2 列のどちらがどちらか分からなくなる(§00 05 読めない文字は出さない)。
     材料の名前も折り返さない幅を取る */
  .grid-row { display: grid; grid-template-columns: 200px 200px 200px; align-items: baseline; column-gap: 14px; }
  .cell { padding: 5px 13px; }
  .cell.label { color: var(--fg-sub); font-size: 10.5px; white-space: nowrap; }
  .cell.val { text-align: right; min-width: 0; }
  .cell.val :global(.num), .cell.val :global(.unk) { font-size: 11px; }
  /* 的中剣のように文字で出す値は折り返さない(行の高さが列で食い違うと段がずれる) */
  .cell.val { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .grid-row.head { padding-top: 4px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
  .grid-row.head .cell { padding: 8px 13px; }
  .grid-row.head .cell.col { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
  .col-name { font-size: 12px; font-weight: 800; color: var(--fg-head); min-width: 0; overflow: hidden; text-overflow: ellipsis; }

  /* 主役の行(命中P・回避P)は太字で強めに、直下の内訳(sub)は控えめに */
  .grid-row.main { background: var(--bg-active); }
  .grid-row.main .cell.label { font-weight: 800; color: var(--fg-head); font-size: 11.5px; }
  .grid-row.main .cell.val :global(.num) { font-size: 15px; font-weight: 800; color: var(--fg-head); }
  .grid-row.sub .cell.label { padding-left: 24px; }
  .grid-row.sub .cell.val :global(.num) { color: var(--fg-sub); }

  /* 伸びしろ: 命中P・回避Pの直下(sub)に「あと +N」の合計を出し、その下に材料(さらに一段
     字下げ)を並べる。値は文字列(「あと +N」「伸びしろなし」)を含むので折り返しを許す
     ── 他の sub 行と違い一発の数値だけでは終わらない */
  .growth-total-row .cell.label { font-weight: 800; color: var(--fg-head); }
  .growth-total-row .cell.val :global(.growth-total) { font-weight: 800; color: var(--fg-head); }
  .growth-item-row .cell.label { padding-left: 40px; }
  .growth-total-row .cell.val, .growth-item-row .cell.val { white-space: normal; overflow: visible; text-overflow: clip; }
  .growth-item { display: inline-flex; flex-direction: column; align-items: flex-end; gap: 1px; }
  .growth-detail { font-size: 9px; }
  .growth-provisional {
    font-size: 9px; font-weight: 800; border-radius: var(--r-pill); padding: 1px 5px; border: 1px solid;
    margin-left: 4px;
  }
  .growth-none { font-size: 10.5px; }

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
  .rate-row.has-result { border: 1px solid var(--border); background: var(--bg-field); }

  .rate-who { display: flex; align-items: center; gap: 6px; min-width: 0; flex-shrink: 0; font-size: 12px; font-weight: 700; color: var(--fg-head); }
  .rate-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rate-arrow { flex-shrink: 0; font-size: 12px; font-weight: 700; color: var(--fg-dim); margin: 0 2px; }

  /* 桁が増えても隣が動かないよう幅を固定する(§09 規則 4) */
  .rate-value { flex-shrink: 0; display: flex; align-items: baseline; gap: 3px; min-width: 60px; }
  .rate-num { font-size: 18px; font-weight: 800; color: var(--fg-head); line-height: 1; }
  .rate-unit { font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .rate-cap {
    font-size: 10.5px; font-weight: 800; border-radius: var(--r-pill); padding: 3px 9px; border: 1px solid;
  }
  .rate-note { font-size: 10px; }
  .rate-note.bad { color: var(--danger); font-weight: 700; }

  .rate-why { font-size: 10.5px; display: flex; flex-wrap: wrap; align-items: baseline; gap: 3px; }
  .rate-why .op { color: var(--fg-dim); }
</style>
