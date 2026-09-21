<script lang="ts">
  // 鎖(1 発 → 合計 → DPS[→ 討伐時間])1 本ぶん。攻撃者(本体 / 召喚獣)ごとに CalcPage が
  // この部品を 1 回ずつ描く(ADR-016)。中身の組み立ては calc/damageDetail.ts、
  // 内訳の面は calc/DetailRows.svelte(そのままここでも使う)。
  //
  // 討伐時間の節は showDefeat で切り替える: 召喚スキルが無いキャラは今までどおり鎖の内に
  // 討伐時間まで出す。召喚スキルがあるキャラは
  // 本体・召喚獣どちらの鎖にも討伐時間節を出さず、CalcPage 側の「合計」面(combined)にだけ出す
  // (同じ情報を 2 箇所に出さない。§00 ②)。
  import type { CombinedDamage, DamageResult, FlagDamage, Rotation, Skill } from "../../api/types";
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
    /** <フラグ>(技とは別枠のダメージ)。本体の鎖だけが受け取る。null = 積んでいない */
    flag?: FlagDamage | null;
    /** 回し(連打する技 + 差し込む CT 技)。本体の鎖だけが受け取る。null = 回しを組まない */
    rotation?: Rotation | null;
    /** この鎖に合流する別枠込みの合算(1 発の合計・DPS・討伐時間)。Rust が足した値で、
     *  画面は側を選ぶだけ。null = 合算を鎖に出さない(熊がいるときは CalcPage の「合計」面が持つ) */
    combined?: CombinedDamage | null;
  }
  let {
    result, skill, store, attackerLabel, isSummon = false, attackerSkillName, icon, showDefeat, heroNumber,
    intervalNote = null, onView, onPerHitDeltaFollow, flowChanged = false, flag = null, rotation = null,
    combined = null,
  }: Props = $props();

  const toggle = (k: string) => {
    onView?.();
    store.toggle(k);
  };

  const critMode = $derived(result.critical_chance > 0);
  const perHit = $derived(result.per_hit_primary);
  // 合計・DPS・討伐時間は「この鎖に合流する別枠込み」の値を出す。合流する別枠が無ければ
  // combined は body と同じ値になる(Rust の combine_damage)ので、画面に分岐を持たせない
  const totalValue = $derived(combined?.total_primary ?? result.total_primary);
  const dpsValue = $derived(pickSide(combined?.dps ?? result.dps, critMode));
  const defeatSeconds = $derived(combined?.defeat_seconds ?? result.defeat_seconds);
  const expectedDps = $derived(combined?.expected_dps ?? result.expected_dps);
  /** <フラグ> 爆発(主軸が スレイ / クラッシュ のときだけ)。合計に合流する */
  const burst = $derived(flag?.burst ?? null);
  /** 回しの中の主軸(CT 技として差し込んでいるとき)。DPS は回しで出している */
  const mainInsert = $derived(rotation?.inserts.find((i) => i.is_main) ?? null);
  /** 連打している技(主軸そのものなら is_main)。null = 連打できる技が無い */
  const filler = $derived(rotation?.filler ?? null);
  /** DPS の分母。主軸を CT ごとに 1 回撃つならその間隔、回しを組んでいなければ中ディレイ
   *  (コンボならサイクル)。**主軸を連打して別の技を差し込む形は 1 つの数で割っていない**ので
   *  null(分母を出さない。§00 05) */
  const dpsDenominator = $derived(
    mainInsert
      ? mainInsert.interval_seconds
      : rotation
        ? null
        : (result.combo?.seconds ?? result.actual_delay?.value ?? null),
  );
  /** 主軸を 1 分間に何回撃つか。回しの中では「差し込みは間隔ごとに 1 回」
   *  「連打は空いた時間ぶん」なので、そのまま回数に直す */
  const usesPerMinute = $derived.by<number | null>(() => {
    if (mainInsert) return 60 / mainInsert.interval_seconds;
    if (rotation) {
      // 回しの中の回数は Rust が技ごとに返している(画面で割り戻さない)
      return filler && filler.is_main ? filler.uses_per_minute : null;
    }
    return result.combo?.uses_per_minute ?? result.actual_delay?.uses_per_minute ?? null;
  });

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
        sub: "シャープネスビジョン・ランダムOP・称号・キャラスキル",
      });
    }
    if (burst) {
      // 技とは別枠のダメージ。技 1 回につき 1 度なので、1 発の合計にそのまま合流する
      mats.push({
        label: `<フラグ> 爆発(スタック ${flag?.stacks ?? 0})`,
        mult: `×${burst.hit_count} 段`,
        value: fmtInt(burst.total_primary),
        n: burst.total_primary,
        sub: `技とは別枠・倍率 ${fmtPct(flag?.multiplier ?? 0)}・Cri倍率 ×2.5`,
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
    if (d.charge > 0) {
      // チャージは中ディレイ減少も倍率A も下限 0.3s も受けず、下限を取ったあとに足す
      mats.push({
        label: "チャージ(最大までためる)",
        mult: "+",
        value: fmtNum(d.charge, 2, "s"),
        n: d.charge, unit: "s",
        sub: "減少も下限も効かない(そのまま足す)",
      });
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
        // 回しの中では「差し込みは間隔ごとに 1 回」「連打は空いた時間ぶん」なので、
        // この回数は連打し続けたときの上限になる(実際の回数は下の段)
        sub: (intervalNote ?? (d.uses_measured ? "実測表から" : "式 60 ÷ 中ディレイ"))
          + (rotation ? " ・ 連打し続けたときの回数(実際は下の回しのとおり)" : ""),
      });
    }
    // チャネリング技(押している間、一定間隔で攻撃を繰り返す)。上の「合計ダメージ」は
    // 1 tick ぶん(ゲーム内の表示と同じ)なので、1 回の使用で何回入るかをここで言う
    if (skill?.channeling) {
      const { ticks, tick_seconds } = skill.channeling;
      mats.push({
        label: "チャネリング",
        value: `${fmtInt(ticks)} 回`,
        n: ticks,
        sub: `合計ダメージ(${fmtInt(result.hit_count)} 段)を ${fmtNum(tick_seconds, 2, "s")} 毎に、`
          + `最大 ${fmtNum(tick_seconds * ticks, 2, "s")} 撃ち続けます`,
      });
    }
    // 回し(連打する技 + 差し込む CT 技)。**技ごとの寄与・取り分・合間の回数は主役カードの
    // 「回し」の段が常設で出している**ので、ここに同じ内訳を二重に持たない(§00 ②)。
    // 残すのは段では言えない**理由** —— その間隔が何で決まっているか(CT 律速 / 積み直し律速)と、
    // 差し込みだけで時間が埋まっていること
    if (rotation) {
      for (const insert of rotation.inserts) {
        const reason = rotation.crowded
          ? "差し込む技だけで時間が埋まり、頻度を縮めています(全部は CT どおりに撃てません)"
          : insert.cooldown_bound
            ? `CT ${fmtNum(insert.cooldown_seconds, 0, "s")} が明くまで連打技を挟むので、この間隔になります`
            : insert.filler_uses > 0
              ? `<フラグ> を積み直すのに連打技を ${insert.filler_uses} 回挟むので、CT より長くなります`
              : `CT ${fmtNum(insert.cooldown_seconds, 0, "s")} が明けたらすぐ撃てます`;
        mats.push({
          label: `↳ ${insert.skill_name} の間隔`,
          value: fmtNum(insert.interval_seconds, 2, "s"),
          n: insert.interval_seconds, unit: "s",
          sub: reason + (insert.burst ? " ・ <フラグ> 爆発つき" : ""),
        });
      }
    }
    // 技とは別枠のダメージ(<フラグ>)。Rust が「この秒数に 1 回」を当てた dps を持っている
    // ので、画面は側を選んで並べるだけ(合算は combined が持つ)
    if (flag) {
      const durationDps = pickSide(flag.duration.dps, critMode);
      mats.push({
        label: `<フラグ> 持続(${fmtNum(flag.tick_seconds, 1, "s")}ごと)`,
        value: durationDps !== null ? fmtInt(Math.round(durationDps)) : "—",
        n: durationDps === null ? undefined : Math.round(durationDps),
        sub: `1 回 ${fmtInt(pickSide(flag.duration.total, critMode) ?? 0)}・持続 ${fmtNum(flag.lasts_seconds, 0, "s")}`,
      });
    }
    if (burst) {
      const burstDps = pickSide(burst.dps, critMode);
      mats.push({
        label: "<フラグ> 爆発",
        value: burstDps !== null ? fmtInt(Math.round(burstDps)) : "—",
        n: burstDps === null ? undefined : Math.round(burstDps),
        sub: mainInsert
          ? `${attackerSkillName} 1 回につき 1 度(${fmtNum(mainInsert.interval_seconds, 2, "s")} に 1 回)`
          : "技 1 回につき 1 度",
      });
    }
    if (expectedDps !== null && result.critical_chance > 0 && result.critical_chance < 1) {
      mats.push({
        label: "期待値(クリ率で按分)",
        value: fmtInt(Math.round(expectedDps)),
        n: Math.round(expectedDps),
        sub: `${flag ? "<フラグ> 込みの" : ""}合計(非クリ) × ${fmtPct(1 - result.critical_chance, 1)} + 合計(クリ) × ${fmtPct(result.critical_chance, 1)}`,
      });
    }
    return {
      mult: dpsDenominator !== null ? `÷ ${fmtNum(dpsDenominator, 2, "s")}` : "回し",
      delta: null,
      to: Math.round(dpsValue),
      mats,
      idle: 0,
      expr: rotation
        ? "1 秒あたり = 差し込む技(＋ <フラグ> 爆発) ÷ その間隔 ＋ 連打技 × 空いた時間"
          + (flag ? " ＋ <フラグ> 持続(1 秒ごと)" : "")
        : (cycle
            ? "1 秒あたり = (スキルの合計 + 通常攻撃の合計) ÷ 1 サイクル"
            : "1 秒あたり = 合計 × スキル回数(回/分) ÷ 60")
          + (flag ? " ＋ <フラグ> 持続(1 秒ごと)" : ""),
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
      <span class="nl">DPS {#if dpsDenominator !== null}<span class="num">(÷ <Value motion={() => dpsDenominator} value={fmtNum(dpsDenominator, 2, "s")} />{mainInsert ? " ごとに 1 回" : ""})</span>{:else if rotation}<span class="num">(回し)</span>{/if}</span>
      <Value class="nv" motion={() => dpsValue} value={dpsValue !== null ? fmtInt(Math.round(dpsValue)) : "—"} />
      <span class="nsub dim">
        <span class="nsub-line"><Value motion={() => (dpsValue === null ? null : Math.round(dpsValue))} delta={{}} /></span>
        <span class="nsub-line">
          <span>
            {#if intervalNote}
              {intervalNote}
            {:else if usesPerMinute !== null}
              {attackerSkillName} {fmtNum(usesPerMinute, 1)} 回/分{rotation && filler && !filler.is_main ? ` ・ 合間に ${filler.skill_name}` : ""} ・{critMode ? "クリ確定" : "非クリ"}
            {:else if rotation}
              連打と差し込みの回し ・{critMode ? "クリ確定" : "非クリ"}
            {/if}
          </span>
        </span>
        {#if expectedDps !== null && result.critical_chance > 0 && result.critical_chance < 1}
          <span class="nsub-line">
            期待値 <Value motion={() => expectedDps} value={fmtInt(Math.round(expectedDps ?? 0))} />(クリ率 {fmtPct(result.critical_chance, 1)})
          </span>
        {/if}
      </span>
    </button>
    {#if showDefeat && defeatSeconds !== null && result.enemy_hp !== null}
      <div class="node rate">
        <span class="nl">討伐時間 <Value motion={() => result.enemy_hp} value={`(HP ${fmtInt(result.enemy_hp ?? 0)})`} /></span>
        <Value class="nv" motion={() => defeatSeconds} value={fmtDuration(defeatSeconds ?? 0)} />
        <span class="nsub dim">
          <span class="nsub-line">
            <Value
              motion={() => (defeatSeconds == null ? null : Math.round(defeatSeconds))}
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
