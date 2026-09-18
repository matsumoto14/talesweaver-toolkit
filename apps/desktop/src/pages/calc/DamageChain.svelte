<script lang="ts">
  // 鎖(1 発 → 合計 → DPS[→ 討伐時間])1 本ぶん。攻撃者(本体 / 召喚獣)ごとに CalcPage が
  // この部品を 1 回ずつ描く(ADR-016)。中身の組み立ては calc/damageDetail.ts、
  // 内訳の面は calc/DetailRows.svelte(そのままここでも使う)。
  //
  // 討伐時間の節は showDefeat で切り替える: 召喚スキルが無いキャラは今までどおり鎖の内に
  // 討伐時間まで出す。召喚スキルがあるキャラは
  // 本体・召喚獣どちらの鎖にも討伐時間節を出さず、CalcPage 側の「合計」面(combined)にだけ出す
  // (同じ情報を 2 箇所に出さない。§00 ②)。
  import type { DamageResult, Skill } from "../../api/types";
  import { fmtDuration, fmtInt, fmtNum, fmtPct, fmtRate, fmtSigned, fmtSignedPct } from "../../format";
  import { limits } from "../../limits.svelte";
  import Icon, { type IconKind } from "../../ui/Icon.svelte";
  import Value from "../../ui/Value.svelte";
  import { critChanceStage } from "../../ui/critChance";
  import DetailRows from "./DetailRows.svelte";
  import { flowRowsOf, pick as pickSide, stepOf, stepValue, stepsOf } from "./damageDetail";
  import { DetailStore, type Detail, type Mat } from "./detailStore.svelte";

  interface Props {
    result: DamageResult;
    /** 内訳(「クリティカルなら」の倍率表示)に要る。未収録なら null */
    skill: Skill | null;
    /** この鎖の開閉・変わった行の控え。攻撃者ごとに 1 つ(CalcPage が本体用・熊用を分けて持つ) */
    store: DetailStore;
    /** 誰の鎖か(「熊」/「精霊」/「本体」) */
    attackerLabel: string;
    /** この鎖が召喚獣(熊 or 精霊)のものか。バッジのスタイル分岐に使う(attackerLabel の
     *  文字列一致ではなく props で受ける。2026-09-18) */
    isSummon?: boolean;
    attackerSkillName: string;
    /** バッジに置く撃つ人の絵(本体 = キャラ、召喚獣 = 魔法人形 / 破壊精霊)。source は登録キャラの任意画像 */
    icon: { kind: IconKind; id: string | null; source?: string | null };
    /** 討伐時間の節を鎖の中に出すか(召喚スキルが無いキャラだけ true。上の説明参照) */
    showDefeat: boolean;
    /** 44px の主役数字(.hero-num)を使うか。鎖が 2 本(召喚スキルあり)のときも両方 true —
     *  本体と熊で寸法を変えない(ユーザー判断 2026-09-18) */
    heroNumber: boolean;
    /** 熊の DPS 節に出す間隔の注記(「{n}s 間隔(中ディレイ + 0.0705s)・コンボは乗りません」)。
     *  本体は null(通常の「◯回/分」表示のまま) */
    intervalNote?: string | null;
    /** 節を押した(= この鎖を見ている)。「なぜこの数字?」がこの鎖に付いてくる合図 */
    onView?: () => void;
    /** 「1 発」の差分を押すと「なぜこの数字?」へ辿る(本体だけが持つ機能。熊には無い) */
    onPerHitDeltaFollow?: () => void;
    /** 「なぜこの数字?」の中に変わった段があるか(本体の ↑ に下線を出す判定。熊は常に false) */
    flowChanged?: boolean;
  }
  let {
    result, skill, store, attackerLabel, isSummon = false, attackerSkillName, icon, showDefeat, heroNumber,
    intervalNote = null, onView, onPerHitDeltaFollow, flowChanged = false,
  }: Props = $props();

  const toggle = (k: string) => {
    onView?.();
    store.toggle(k);
  };

  const critMode = $derived(result.critical_chance > 0);
  const perHit = $derived(result.per_hit_primary);
  const totalValue = $derived(result.total_primary);
  const dpsValue = $derived(pickSide(result.dps, critMode));

  // 段の組み立ては calc/damageDetail.ts(CalcPage の「なぜこの数字?」と同じ関数)。
  // 熊には WhyPanel が無いが、材料(トレースの段)は本体と同じ形で Rust が返すので流用できる。
  const steps = $derived(stepsOf(result, critMode));
  const pierced = $derived(stepValue(steps, "攻撃力−防御力"));
  const flowRows = $derived(flowRowsOf(steps, pierced));
  const flowMultLabel = $derived(
    pierced !== null && pierced > 0 && perHit !== null ? fmtRate(perHit / pierced, 1) : "—",
  );

  /** 鎖「1 発」: 抜けた分から 1 発までの各段(倍率・実数・到達値) */
  const perHitDetail = $derived.by<Detail | null>(() => {
    if (perHit === null || pierced === null) return null;
    const mats: Mat[] = flowRows.map((f) => ({
      label: f.k,
      mult: f.mult === "—" ? undefined : f.mult,
      value: fmtSigned(f.add),
      sub: `ここまで ${fmtInt(Math.round(f.to))}`,
      n: Math.round(f.add),
    }));
    if (result.capped_loss.max > 0) {
      mats.push({
        label: "ダメージ上限(1 段ごと)",
        value: fmtInt(result.damage_cap),
        n: result.damage_cap,
        sub: `上限で ${fmtSigned(-result.capped_loss.max, { max: 3 })}`,
      });
    }
    return {
      mult: flowMultLabel,
      delta: perHit - pierced,
      to: perHit,
      mats,
      idle: 0,
      expr: "ゲームの表記ダメージ(スキル分のみ)。武器強化の追加固定ダメージは含まない(合計の内訳を見る)",
    };
  });

  /** 鎖「合計」: (1 発 × 段数) ＋ (武器強化の追加固定 × 段数) ＋ 割合追加ダメージ */
  const totalDetail = $derived.by<Detail | null>(() => {
    const added = pickSide(result.added_damage, critMode) ?? 0;
    const skillTotal = pickSide(result.skill_total, critMode) ?? 0;
    const mats: Mat[] = [
      {
        label: `1 発(表記ダメージ) ${fmtInt(perHit)} × ${result.hit_count} 段`,
        mult: `×${result.hit_count}`,
        value: fmtInt(skillTotal),
        n: skillTotal,
      },
    ];
    if (result.weapon_added_per_hit !== 0) {
      mats.push({
        label: "武器強化(追加固定)",
        mult: "+",
        value: fmtInt(result.weapon_added_total),
        n: result.weapon_added_total,
        sub: "上限なし・表記ダメージとは別枠",
      });
    }
    if (added !== 0) {
      mats.push({
        label: "割合追加ダメージ(合計に乗る)",
        mult: fmtSignedPct(result.added_damage_rate, { max: 4 }),
        value: fmtInt(added),
        n: added,
        sub: "シャープネスビジョン・ランダムOP・称号",
      });
    }
    if (!critMode) {
      mats.push({
        label: "クリティカルなら",
        mult: skill ? `×${fmtNum(skill.critical_multiplier)}` : undefined,
        value: fmtInt(result.total.critical),
        n: result.total.critical,
      });
    }
    mats.push({ label: "乱数が最小のとき", value: fmtInt(result.total.min), n: result.total.min });
    return {
      mult: `×${result.hit_count} 段`,
      delta: totalValue - perHit,
      to: totalValue,
      mats,
      idle: 0,
      expr: stepOf(steps, "割合追加ダメージ(合計に乗る)")?.expression ?? null,
    };
  });

  /** 鎖「1 秒あたり」: 合計 × 回/分 ÷ 60。熊は実測表を使わず式(intervalNote)で出す */
  const dpsDetail = $derived.by<Detail | null>(() => {
    const d = result.actual_delay;
    if (d === null || result.dps === null || dpsValue === null) return null;
    const mats: Mat[] = [
      { label: "合計ダメージ", value: fmtInt(totalValue), n: totalValue },
      {
        label: "基本中ディレイ",
        value: fmtNum(d.base, 2, "s"),
        n: d.base, unit: "s",
        sub: d.fixed ? "固定(減少が効かない)" : undefined,
      },
    ];
    for (const c of d.contributions) {
      mats.push({ label: `↳ ${c.source}`, value: fmtSignedPct(-c.rate), n: -Math.round(c.rate * 100), unit: "%" });
    }
    mats.push({
      label: `中ディレイ減少(上限 ${fmtPct(limits.actual_delay_reduction_max)})`,
      value: fmtPct(d.reduction),
      n: Math.round(d.reduction * 100), unit: "%",
      sub: d.reduction_raw > d.reduction ? `選択中は ${fmtPct(d.reduction_raw)}` : undefined,
    });
    if (d.combo_rate < 1) {
      mats.push({ label: "コンボ(倍率A。間に通常攻撃を挟む)", mult: `×${fmtNum(d.combo_rate)}`, value: "" });
    }
    mats.push({
      label: intervalNote ? "攻撃間隔" : "中ディレイ",
      value: fmtNum(d.value, 2, "s"),
      n: d.value, unit: "s",
      // 熊は 基本 × (1 − 減少) + 0.0705s(発動遅延)。下限 0.3s もコンボ倍率も無い(domain::apply_summon_interval)
      sub: intervalNote
        ? "基本 × (1 − 減少) + 0.0705s(発動遅延)"
        : d.floored ? `下限 ${fmtNum(limits.actual_delay_min, 1, "s")} で頭打ち` : undefined,
    });
    const cycle = result.combo;
    if (cycle) {
      // コンボは 1 サイクル(通常攻撃 → スキル)で割る。実測表はコンボなしの計測なので使わない
      mats.push({
        label: `通常攻撃(${cycle.normal_attack_name})`,
        value: fmtInt(pickSide(cycle.normal_attack_total, critMode) ?? 0),
        n: pickSide(cycle.normal_attack_total, critMode) ?? 0,
        sub: `中ディレイ ${fmtNum(cycle.normal_delay, 2, "s")}`,
      });
      mats.push({
        label: "コンボインターバル",
        value: cycle.interval !== null ? fmtNum(cycle.interval, 2, "s") : "?",
        n: cycle.interval ?? undefined, unit: "s",
        sub: cycle.interval === null
          ? "wiki 未収録。スキルの中ディレイをそのまま使っています"
          : cycle.interval_binding
            ? "スキルの中ディレイより長いので、こちらが下限になります"
            : "スキルの中ディレイのほうが長いので効きません",
      });
      mats.push({
        label: "1 サイクル",
        value: fmtNum(cycle.seconds, 2, "s"),
        n: cycle.seconds, unit: "s",
        sub: `通常攻撃 ${fmtNum(cycle.normal_delay, 2, "s")} + ${fmtNum(cycle.skill_gap, 2, "s")}`,
      });
    } else {
      mats.push({
        label: "スキル回数",
        value: `${Math.round(d.uses_per_minute)} 回/分`,
        n: Math.round(d.uses_per_minute),
        sub: intervalNote ?? (d.uses_measured ? "実測表から" : "式 60 ÷ 中ディレイ"),
      });
    }
    if (result.expected_dps !== null && result.critical_chance > 0 && result.critical_chance < 1) {
      mats.push({
        label: "期待値(クリ率で按分)",
        value: fmtInt(Math.round(result.expected_dps)),
        n: Math.round(result.expected_dps),
        sub: `合計(非クリ) × ${fmtPct(1 - result.critical_chance, 1)} + 合計(クリ) × ${fmtPct(result.critical_chance, 1)}`,
      });
    }
    return {
      mult: `÷ ${fmtNum(cycle?.seconds ?? d.value, 2, "s")}`,
      delta: null,
      to: Math.round(dpsValue),
      mats,
      idle: 0,
      expr: cycle
        ? "1 秒あたり = (スキルの合計 + 通常攻撃の合計) ÷ 1 サイクル"
        : "1 秒あたり = 合計 × スキル回数(回/分) ÷ 60",
    };
  });
</script>

<div class="chain-block">
  <div class="chain">
    <div class="chain-badge badge-in" class:bear={isSummon}>
      <Icon kind={icon.kind} id={icon.id} source={icon.source ?? null} size={28} label={attackerLabel} />
      <span class="badge-text">
        <span class="badge-who">{attackerLabel}</span>
        <span class="badge-skill">{attackerSkillName}</span>
      </span>
    </div>
    <button
      type="button" class="node gate"
      aria-expanded={store.isOpen("perHit")} onclick={() => toggle("perHit")}
    >
      <span class="nl">表記ダメージ(1 発)</span>
      <Value class={heroNumber ? "hero-num nv" : "nv"} motion={() => perHit} value={fmtInt(perHit)} />
      <span class="nsub num">
        <span class="nsub-line">
          <Value
            motion={() => perHit} delta={{}}
            deltaClass={flowChanged ? "follow" : ""}
            onDelta={onPerHitDeltaFollow}
          />
        </span>
      </span>
    </button>
    <button
      type="button" class="node mid"
      aria-expanded={store.isOpen("total")} onclick={() => toggle("total")}
    >
      <span class="nl">合計ダメージ <span class="num">(×<Value motion={() => result.hit_count} value={String(result.hit_count)} /> 段)</span></span>
      <Value class="nv" motion={() => totalValue} value={fmtInt(totalValue)} />
      <span class="nsub num">
        <span class="nsub-line"><Value motion={() => totalValue} delta={{}} /></span>
        <span class="nsub-line">
          {#if result.critical_rate === null}
            <span>クリ率 未記載 → 確定扱い</span>
          {:else}
            <Value
              class={`${result.critical_chance <= 0 ? "crit-none" : ""} ${result.critical_chance > 0 && result.critical_chance < 1 ? "crit-partial" : ""}`}
              value={critChanceStage(result.critical_chance * 100).label}
            >
              {#snippet children()}クリ率 {fmtNum(result.critical_rate!.value, 1, "%")}{critMode ? ` ・ ${critChanceStage(result.critical_chance * 100).label}` : ""}{/snippet}
            </Value>
          {/if}
        </span>
      </span>
    </button>
    <button
      type="button" class="node rate"
      aria-expanded={store.isOpen("dps")} onclick={() => toggle("dps")}
    >
      <span class="nl">DPS <span class="num">(÷ <Value motion={() => result.actual_delay?.value ?? null} value={result.actual_delay ? fmtNum(result.actual_delay.value, 2, "s") : "—"} />)</span></span>
      <Value class="nv" motion={() => dpsValue} value={dpsValue !== null ? fmtInt(Math.round(dpsValue)) : "—"} />
      <span class="nsub dim">
        <span class="nsub-line"><Value motion={() => (dpsValue === null ? null : Math.round(dpsValue))} delta={{}} /></span>
        <span class="nsub-line">
          <span>
            {#if intervalNote}
              {intervalNote}
            {:else if result.combo}
              {Math.round(result.combo.uses_per_minute)} 回/分 ・{critMode ? "クリ確定" : "非クリ"}
            {:else if result.actual_delay}
              {Math.round(result.actual_delay.uses_per_minute)} 回/分 ・{critMode ? "クリ確定" : "非クリ"}
            {/if}
          </span>
        </span>
        {#if result.expected_dps !== null && result.critical_chance > 0 && result.critical_chance < 1}
          <span class="nsub-line">
            期待値 <Value motion={() => result.expected_dps} value={fmtInt(Math.round(result.expected_dps ?? 0))} />(クリ率 {fmtPct(result.critical_chance, 1)})
          </span>
        {/if}
      </span>
    </button>
    {#if showDefeat && result.defeat_seconds !== null && result.enemy_hp !== null}
      <div class="node rate">
        <span class="nl">討伐時間 <Value motion={() => result.enemy_hp} value={`(HP ${fmtInt(result.enemy_hp ?? 0)})`} /></span>
        <Value class="nv" motion={() => result.defeat_seconds} value={fmtDuration(result.defeat_seconds ?? 0)} />
        <span class="nsub dim">
          <span class="nsub-line">
            <Value
              motion={() => (result.defeat_seconds == null ? null : Math.round(result.defeat_seconds))}
              delta={{ unit: "秒", digits: 0 }}
              deltaClass="less-is-better"
            />
          </span>
          <span class="nsub-line">ソロ</span>
        </span>
      </div>
    {/if}
  </div>
  {#if perHitDetail}<DetailRows boxed d={perHitDetail} {store} open={store.isOpen("perHit")} />{/if}
  {#if totalDetail}<DetailRows boxed d={totalDetail} {store} open={store.isOpen("total")} />{/if}
  {#if dpsDetail}<DetailRows boxed d={dpsDetail} {store} open={store.isOpen("dps")} />{/if}
</div>

<style>
  /* CalcPage.svelte の元の .chain 一式をそのまま持ってきたもの(見た目は変えない)。
     .hero(padding)・.meter/.hero-sentence/.fill-line・.delay-note は CalcPage 側に残る
     (鎖の外・combined を使うため) */
  .chain { display: flex; align-items: stretch; gap: 4px; flex-wrap: wrap; margin: 0 -6px; }
  .chain-badge {
    flex-shrink: 0; align-self: stretch; display: flex; align-items: center;
    gap: 7px; padding: 4px 9px 4px 6px; margin-right: 2px; border-radius: var(--r-inset);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    /* 幅を固定して 2 本の鎖の列位置をそろえる(スキル名の長さで節がずれない。§09) */
    width: 136px; box-sizing: border-box;
  }
  .badge-text { display: flex; flex-direction: column; justify-content: center; gap: 1px; min-width: 0; }
  .chain-badge.bear { background: var(--state-temp-bg); border-color: var(--sim); }
  .badge-who { font-size: 10px; font-weight: 800; color: var(--fg-head); white-space: nowrap; }
  .chain-badge.bear .badge-who { color: var(--sim-fg); }
  .badge-skill { font-size: 8.5px; color: var(--fg-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chain .node { display: flex; flex-direction: column; gap: 0; min-width: 0; padding: 4px 6px 5px; }
  .chain .nl {
    line-height: 14px; font-size: 10px; font-weight: 700; color: var(--fg-head); white-space: nowrap;
  }
  .chain .nl :global(.num) { font-weight: 600; color: var(--fg-muted); }
  .chain :global(.nv) { margin-top: auto; padding-top: 6px; }
  .chain .nsub { margin-top: 4px; gap: 4px; }
  .chain :global(.nv) { font-weight: 700; color: var(--fg); white-space: nowrap; }
  .chain .node.gate :global(.nv) { min-width: 120px; }
  .chain .node.mid :global(.nv) { font-size: 15px; min-width: 68px; }
  .chain .node.rate :global(.nv) { font-size: var(--t-heading); min-width: 68px; }
  .chain .node.gate { width: 260px; }
  .chain .node.mid { min-width: 136px; }
  .chain .nsub { font-size: 9px; color: var(--fg-dim); white-space: nowrap; display: flex; flex-direction: column; }
  .chain .nsub-line { display: flex; align-items: baseline; gap: 5px; min-height: 14px; }
  .chain :global(.crit-none) { color: var(--state-short-fg); font-weight: 700; }
  .chain :global(.crit-partial) { color: var(--state-edge-fg); font-weight: 700; }
  .chain .nsub :global(.delta) { font-size: 11px; line-height: 14px; }
  .chain button.node { text-align: left; border-radius: var(--r-inset); }
  .chain button.node:hover { background: var(--bg-active); }
  .chain button.node:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
</style>
