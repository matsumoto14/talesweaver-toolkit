<script lang="ts">
  // 対人: 2 人のキャラ(+使用スキル)を選び、両方向の命中率(A→B / B→A)を出す画面。
  // 攻撃側・防御側という役割は選ばせない ── 知りたいのは「殴り合ったらどっちがどれだけ当たるか」
  // であって、役割の入れ替え操作そのものが不要(ユーザー指摘 2026-09-01)。
  // 計算は Rust 側(preview_versus → domain::versus_accuracy)。ここは組み立てて渡すだけ。
  import { errorMessage, listSkills, previewVersus } from "../../api/commands";
  import type { AccuracyBoost, GrowthRoom, GrowthSource, Skill, VersusAccuracy } from "../../api/types";
  import { limits } from "../../limits.svelte";
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

  /** 倍率は domain の定数(limits 経由)。画面に写経しない */
  function boostLabel(boost: AccuracyBoost): string | null {
    if (boost === "none") return null;
    if (boost === "concentration") {
      return `ペット集中 ・ 命中P ×${limits.concentration_accuracy_rate}`;
    }
    const level = boost.precision_sword;
    const rate = (1 + limits.precision_sword_accuracy_rate_per_level * level).toFixed(2);
    return `極・的中剣 Lv${level} ・ 命中P ×${rate}`;
  }

  // --- 命中P・回避Pの伸びしろ(accuracy_growth / evasion_growth) --------------------
  // 材料の出どころ(GrowthSource)を軸に、2 人ぶんを同じ行へ揃える。並び順はこの固定順
  // (Rust 側は gain 降順で返すので、そのままだと側ごとに順番がずれて突き合わせにくい)。
  const GROWTH_SOURCE_ORDER: GrowthSource[] = ["stat", "enchant", "siena", "precision_sword", "accuracy_buff"];

  interface GrowthRow { source: GrowthSource; label: string; a: GrowthRoom | null; b: GrowthRoom | null }

  function mergeGrowth(a: GrowthRoom[], b: GrowthRoom[]): GrowthRow[] {
    return GROWTH_SOURCE_ORDER.flatMap((source) => {
      const ai = a.find((g) => g.source === source) ?? null;
      const bi = b.find((g) => g.source === source) ?? null;
      if (!ai && !bi) return [];
      return [{ source, label: (ai ?? bi)!.label, a: ai, b: bi }];
    });
  }

  // 伸びしろは既定で畳む。開いたら材料ごとの内訳を出す(ユーザー指摘 2026-09-01)。
  // 命中P・回避Pで独立に開閉できる(押した行はその場に留まり、下に生えるだけ)
  let growthOpenAcc = $state(false);
  let growthOpenEva = $state(false);

  // ダメージ計算タブと同じ言い方(「結果への効きを %」)にそろえる。命中P・回避Pの素の増分は
  // 下限・上限で頭打ちのことがあるので、動かない材料は空白や「—」にせず ±0% と正直に出す。
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
  result: VersusAccuracy | null,
  growthCount: number,
  max: number | null,
  point: number | null,
  maxHitRate: number | null,
)}
  {#if result === null || max === null || point === null || maxHitRate === null}
    <span class="unk">?</span>
  {:else if growthCount === 0}
    <span class="growth-none dim">伸びしろなし</span>
  {:else}
    {@const hitGain = maxHitRate - result.hit_rate.value}
    <span class="growth-total">
      <span class="growth-total-hitrate num" use:bump={() => hitGain}>命中率 {formatHitRateGain(hitGain)}</span>
      <span class="growth-total-raw dim">あと +<span class="num">{max - point}</span></span>
    </span>
  {/if}
{/snippet}

{#snippet growthItemCell(item: GrowthRoom | null, unit: string)}
  {#if item === null}
    <span class="growth-none dim">—</span>
  {:else}
    <span class="growth-item">
      <span class="growth-hitrate num" use:bump={() => item.hit_rate_gain}>{formatHitRateGain(item.hit_rate_gain)}</span>
      {#if item.provisional}<span class="growth-provisional" style={badgeStyle({ label: "仮", state: "temp" })}>仮</span>{/if}
      <!-- 素の伸びしろと換算後は行を分ける。1 行に並べると列幅(140px)に入らず、
           途中で折り返して「1,178 → → 命中 +1514 / 2300 P」と読む順が壊れる(実機で検出) -->
      {#if item.detail}<span class="growth-detail dim">{item.detail}</span>{/if}
      <span class="growth-convert dim">
        <span class="growth-unit">{unit}</span>
        <span class="num">+{item.gain}</span>
      </span>
    </span>
  {/if}
{/snippet}

{#snippet growthBlock(
  label: string,
  unit: string,
  resultA: VersusAccuracy | null,
  resultB: VersusAccuracy | null,
  growthA: GrowthRoom[],
  growthB: GrowthRoom[],
  maxA: number | null,
  maxB: number | null,
  pointA: number | null,
  pointB: number | null,
  maxHitRateA: number | null,
  maxHitRateB: number | null,
  open: boolean,
  onToggle: () => void,
)}
  {@const rows = mergeGrowth(growthA, growthB)}
  {@const hasAny = growthA.length > 0 || growthB.length > 0}
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
    <div class="cell val">{@render growthSummaryCell(resultA, growthA.length, maxA, pointA, maxHitRateA)}</div>
    <div class="cell val">{@render growthSummaryCell(resultB, growthB.length, maxB, pointB, maxHitRateB)}</div>
  </button>
  {#if open}
    {#each rows as row (row.source)}
      <div class="grid-row sub growth-item-row">
        <div class="cell label">{row.label}</div>
        <div class="cell val">{@render growthItemCell(row.a, unit)}</div>
        <div class="cell val">{@render growthItemCell(row.b, unit)}</div>
      </div>
    {/each}
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
                "伸びしろ", "命中P", rAB, rBA,
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
                "伸びしろ", "回避P", rBA, rAB,
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
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 980px; }
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
    border: 1px solid var(--border); border-radius: var(--r-window); background: var(--bg-field);
    overflow: hidden;
  }
  .grid { display: flex; flex-direction: column; }
  /* 列幅は「中身が読める」ことを優先する。キャラ名が「マキシ…」で切れると、
     2 列のどちらがどちらか分からなくなる(§00 05 読めない文字は出さない) */
  .grid-row { display: grid; grid-template-columns: 150px 140px 140px; align-items: baseline; column-gap: 10px; }
  .cell { padding: 5px 12px; }
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
  /* button の既定見た目を消し、行としての見た目だけ残す */
  button.growth-total-row {
    background: none; border: none; margin: 0; padding: 0; font: inherit; color: inherit;
    text-align: inherit; width: 100%; cursor: default;
  }
  button.growth-total-row.openable { cursor: pointer; }
  button.growth-total-row.openable:hover .cell.label { color: var(--accent); }
  button.growth-total-row.openable:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
  .growth-chevron {
    display: inline-block; margin-left: 4px; font-size: 9px; color: var(--fg-dim);
    transition: transform 0.15s ease;
  }
  .growth-chevron.open { transform: rotate(90deg); }
  .growth-item-row .cell.label { padding-left: 40px; white-space: normal; }
  .growth-total-row .cell.val, .growth-item-row .cell.val { white-space: normal; overflow: visible; text-overflow: clip; }
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
  .rate-basis { display: inline-flex; align-items: baseline; gap: 3px; }
</style>
