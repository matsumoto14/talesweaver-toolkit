<script lang="ts">
  // 回しの段(計算タブの主役カード「DPS(回し)」の下に常設)。
  //
  // 出すのは 3 つ: **その場で試す差し込みチップ**・**1 周のタイムライン**・**技ごとの寄与**。
  // 数はすべて Rust(`domain::rotation_shares` → `commands::Rotation`)が技ごとに返したもので、
  // ここでは足し算も割り算もしない(合計と内訳が食い違わないため)。区画の**幅は実際の秒数**で、
  // 割合の帯ではない —— 割合の帯 2 本(時間 / DPS)は実機で見方が伝わらなかった(2026-09-21)。
  //
  // チップは**保存しない**(計算タブの原則)。ラベンダー = 保存されない(§03)。
  // チップ行は段のいちばん上に置く —— 押して下の行数が変わっても、押した場所は動かない(§00 ③)。
  import type { Rotation, RotationChoices, Skill } from "../../api/types";
  import { fmtInt, fmtNum, fmtPct, fmtShareOf, fmtSigned } from "../../format";
  import Choose from "../../ui/Choose.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Value from "../../ui/Value.svelte";
  import { changed } from "../../ui/motion.svelte";

  interface Props {
    /** 回し(連打する技 + 差し込む CT 技)。null = 回しを組んでいない */
    rotation: Rotation | null;
    /** 差し込める候補(計算タブのいまの対象・いまの材料で引いたもの)。null = 取得前 */
    choices: RotationChoices | null;
    /** いま ON の技 id(候補にあるものだけ) */
    onIds: string[];
    /** チップを押した(その場で試すだけ。保存しない) */
    onToggle: (id: string, on: boolean) => void;
    /** 保存値と違う組み合わせを試しているか */
    overridden: boolean;
    /** 保存値に戻す */
    onReset: () => void;
    /** このキャラのスキル一覧。回しに入る技がチャネリングかを引くのに使う(鎖の内訳は
     *  主軸しか見ないので、連打技・差し込みの tick はここで言う) */
    skills: Skill[];
  }
  let { rotation, choices, onIds, onToggle, overridden, onReset, skills }: Props = $props();

  /** 1 回の使用で何 tick 入るか(チャネリング技だけ)。他の技は null */
  const channelingOf = (id: string) => skills.find((s) => s.id === id)?.channeling ?? null;
  const tickNote = (id: string) => {
    const c = channelingOf(id);
    return c === null ? "" : ` ・ 1 回 = ${fmtInt(c.ticks)} tick(${fmtNum(c.tick_seconds, 2, "s")} 毎)`;
  };

  /** 区画の色(§03「内訳の色」)。連打技は土台、差し込みは流れの順に並べて取る。
   *  良し悪しではなく「隣の区画と見分ける」ためだけの色なので、状態の 6 系統は使わない */
  const INSERT_COLORS = [
    "var(--flow-1)", "var(--flow-3)", "var(--flow-6)", "var(--flow-2)",
    "var(--flow-5)", "var(--flow-7)", "var(--flow-4)",
  ];

  const FILLER_COLOR = "var(--flow-base)";

  interface Part {
    key: string;
    skillId: string;
    name: string;
    color: string;
    /** 回しの中での役(「連打」/「10.0s に 1 回」) */
    role: string;
    /** 1 回の所要時間・合間の連打回数 */
    detail: string;
    expectedDps: number;
    dpsShare: number;
  }

  /** 行の並びは**帯の左から右と同じ**「差し込む技 → 連打技」(§00 ①) */
  const parts = $derived.by<Part[]>(() => {
    if (!rotation) return [];
    const list: Part[] = [];
    const filler = rotation.filler;
    rotation.inserts.forEach((insert, index) => {
      list.push({
        key: `insert:${insert.skill_id}`,
        skillId: insert.skill_id,
        name: insert.skill_name,
        color: INSERT_COLORS[index % INSERT_COLORS.length],
        role: `${fmtNum(insert.interval_seconds, 1, "s")} に 1 回`,
        detail: `1 回 ${fmtNum(insert.seconds, 2, "s")}` + tickNote(insert.skill_id),
        expectedDps: insert.expected_dps,
        dpsShare: insert.dps_share,
      });
    });
    if (filler) {
      const uses = rotation.inserts.reduce((n, i) => n + i.filler_uses, 0);
      list.push({
        key: `filler:${filler.skill_id}`,
        skillId: filler.skill_id,
        name: filler.skill_name,
        color: FILLER_COLOR,
        role: uses > 0 && rotation.inserts.length === 1 ? `合間に × ${fmtInt(uses)} 回` : "連打",
        detail: `${fmtNum(filler.uses_per_minute, 1)} 回/分 ・ 1 回 ${fmtNum(filler.seconds, 2, "s")}`
          + tickNote(filler.skill_id),
        expectedDps: filler.expected_dps,
        dpsShare: filler.dps_share,
      });
    }
    return list;
  });
  /** 並びが変わったら段を動かす(§00 ④)。技の顔ぶれが変わったときだけ */
  const partsKey = $derived(parts.map((p) => p.key).join("|"));

  // --- 1 周のタイムライン ----------------------------------------------------
  // 差し込む技 1 つにつき 1 本。左端 0 秒 →[その技 c_i 秒][連打技 × k_i 回]→ 右端が
  // その技を撃つ間隔 T_i(`interval_seconds`)。**幅は実際の秒数の比**なので、
  // 「1 周のうちどこで何を撃っているか」がそのまま絵になる。
  // 差し込みが 2 つ以上あると周期が技ごとに違い、全体で 1 本の「1 周」は存在しないので、
  // 技ごとに 1 本ずつ並べて見出しにその技の名前を出す。
  /** 連打 1 回ぶんの刻みを描くかの下限(px)。これより細いと数えられないので出さない */
  const TICK_MIN_PX = 3;
  /** 帯の実幅(px)。刻みを出すかの判定に使う(全部の帯が同じ幅) */
  let trackWidth = $state(0);

  interface Timeline {
    key: string;
    name: string;
    color: string;
    /** その技を撃つ間隔(秒)= 帯の全幅 */
    total: number;
    /** 差し込む技 1 回(秒)とその割合(%) */
    seconds: number;
    insertPct: number;
    /** 合間の連打(回数・合計秒・割合) */
    fillerUses: number;
    fillerSeconds: number;
    fillerPct: number;
    /** 連打 1 回ごとの刻みを描くか */
    ticked: boolean;
    /** 残り(連打も差し込みも入らない時間)の割合と、その呼び名 */
    restPct: number;
    restLabel: string;
  }
  const timelines = $derived.by<Timeline[]>(() => {
    if (!rotation) return [];
    const filler = rotation.filler;
    return rotation.inserts.map((insert, index) => {
      const total = insert.interval_seconds;
      const insertPct = total > 0 ? (insert.seconds / total) * 100 : 0;
      const fillerSeconds = filler ? filler.seconds * insert.filler_uses : 0;
      const fillerPct = total > 0 ? (fillerSeconds / total) * 100 : 0;
      const restPct = Math.max(0, 100 - insertPct - fillerPct);
      const perUsePx = insert.filler_uses > 0
        ? (trackWidth * fillerPct) / 100 / insert.filler_uses
        : 0;
      return {
        key: `line:${insert.skill_id}`,
        name: insert.skill_name,
        color: INSERT_COLORS[index % INSERT_COLORS.length],
        total,
        seconds: insert.seconds,
        insertPct,
        fillerUses: insert.filler_uses,
        fillerSeconds,
        fillerPct,
        ticked: perUsePx >= TICK_MIN_PX,
        restPct,
        // 連打する技が無いときは CT が明くのを待つだけ。詰まっている(crowded)ときは
        // 他の差し込みを撃っている時間
        restLabel: rotation.crowded ? "他の差し込み" : "待ち",
      };
    });
  });
  /** 差し込みが 2 つ以上あるか(見出しに技の名前を出すか) */
  const manyLines = $derived(timelines.length > 1);

  const candidates = $derived(choices?.candidates ?? []);
  /** 差し込むと DPS が上がる候補。下がるものは畳んだ先へ(キャラタブと同じ扱い。ADR-019) */
  const ups = $derived(candidates.filter((c) => (c.expected_dps_gain ?? 0) >= 0));
  /** 損得を出せない候補(1 回の所要時間が未収録)。上段には出すが割合の代わりに「?」。
   *  既定 ON にはならない(判定は domain の `choose_rotation`。画面では決めない) */
  const unknown = (id: string) =>
    candidates.find((c) => c.skill_id === id)?.expected_dps_gain === null;
  const drops = $derived(
    candidates.filter((c) => c.expected_dps_gain !== null && c.expected_dps_gain < 0),
  );
  const dropsOn = $derived(drops.filter((c) => onIds.includes(c.skill_id)));
  const optionsOf = (list: typeof candidates) =>
    list.map((c) => ({ value: c.skill_id, label: c.skill_name }));
  /** 回し全体に対する割合。絶対値だけだと桁が大きく、実際より深刻に見える(キャラタブと同じ) */
  const gainShare = (gain: number) => fmtShareOf(gain, choices?.expected_dps ?? null);
  const gainLabel = (id: string) => {
    const gain = candidates.find((c) => c.skill_id === id)?.expected_dps_gain ?? null;
    return gain === null ? null : gainShare(Math.round(gain));
  };
  const chipTitle = (id: string) => {
    const c = candidates.find((x) => x.skill_id === id);
    if (!c) return undefined;
    const gain = c.expected_dps_gain;
    if (gain === null) {
      return `CT ${c.cooldown_seconds}s ・ 差し込んだときの損得は出せません(1 回の所要時間が未収録)`;
    }
    const share = gainShare(gain);
    const amount = `${share === null ? "" : `${share}(`}${fmtSigned(Math.round(gain))}${share === null ? "" : ")"}`;
    return `CT ${c.cooldown_seconds}s ・ ${gain < 0 ? "差し込むと DPS が下がる" : "差し込むと DPS が上がる"} ${amount}`;
  };
</script>

{#if candidates.length > 0 || parts.length > 0}
  <div class="rotation inset">
    <!-- 押した場所は動かない(§00 ③): チップ行は段のいちばん上。下の行数が変わっても動かない -->
    <div class="rot-pick">
      <span class="rot-title">
        回し
        <!-- 上限は無いので「n / 候補数」を値の隣に常設する(§07)。候補が無い回しでは出さない -->
        {#if candidates.length > 0}
          <Value
            class="dim normal"
            motion={() => onIds.length}
            value={`${onIds.length} / ${candidates.length}`}
          />
        {/if}
      </span>
      {#if ups.length > 0}
        <Choose
          label="ここで試す差し込み CT 技"
          class="chiprow"
          options={optionsOf(ups)}
          values={onIds}
          {onToggle}
          tone={() => "sim"}
          titleFor={chipTitle}
        >
          {#snippet item(o)}
            {@const cd = candidates.find((c) => c.skill_id === o.value)?.cooldown_seconds ?? 0}
            {o.label} <Value class="chip-ct" value={`${cd}s`} />
            <!-- 損得を出せない候補は割合の代わりに「?」(未収録は 0 や空白にしない。§08) -->
            {#if unknown(o.value)}<Value class="chip-gain" value={null} />{/if}
          {/snippet}
        </Choose>
      {/if}
      {#if drops.length > 0}
        <!-- 下がる技は畳んだ先(キャラタブと同じ。ON にしているものは summary で分かる) -->
        <Disclosure class="rot-drops" summaryClass="chip quiet">
          {#snippet summary()}
            DPS が下がる技 <Value class="dim normal" motion={() => drops.length} value={`${drops.length} 件`} />
            {#if dropsOn.length > 0}
              <Value class="badge drop-badge" motion={() => dropsOn.length} value={`${dropsOn.length} 件 ON`} />
            {/if}
          {/snippet}
          <Choose
            label="DPS が下がる差し込み CT 技(ここで試す)"
            class="chiprow"
            options={optionsOf(drops)}
            values={onIds}
            {onToggle}
            tone={() => "sim"}
            titleFor={chipTitle}
          >
            {#snippet item(o)}
              {@const cd = candidates.find((c) => c.skill_id === o.value)?.cooldown_seconds ?? 0}
              {o.label}
              <Value class="chip-ct" value={`${cd}s`} />
              {#if gainLabel(o.value)}
                <Value class="chip-gain" tone="down" value={gainLabel(o.value)} />
              {/if}
            {/snippet}
          </Choose>
        </Disclosure>
      {/if}
      {#if overridden}
        <button type="button" class="rot-reset badge-in" title="キャラに保存した組み合わせに戻す" onclick={onReset}>
          保存値に戻す
        </button>
      {/if}
    </div>

    {#if parts.length > 0}
      <!-- タイムラインと行。技の顔ぶれが変われば丸ごと入れ直すので、入場クラスを 1 つ載せて
           `use:changed` で再生する(§10「動きの部品」) -->
      <div class="rot-body pane-in" use:changed={() => partsKey}>
        <!-- 1 周のタイムライン(差し込む技ごとに 1 本)。幅は実際の秒数の比 -->
        <div class="rot-lines" bind:clientWidth={trackWidth}>
          {#each timelines as line (line.key)}
            <div class="rot-line">
              <span class="line-head">
                {manyLines ? `${line.name} の 1 周` : "1 周"}
                <Value class="line-total" motion={() => line.total} value={fmtNum(line.total, 1, "s")} />
              </span>
              <span class="track inset">
                <span class="zone" style="width: {line.insertPct}%; background: {line.color};">
                  <span class="zone-text">{line.name} {fmtNum(line.seconds, 1, "s")}</span>
                </span>
                {#if line.fillerPct > 0 && rotation?.filler}
                  <!-- 連打の区画は 1 回ごとに刻む(細くて数えられないときは刻まず回数だけ) -->
                  <span
                    class="zone filler"
                    class:ticked={line.ticked}
                    style="width: {line.fillerPct}%; background: {FILLER_COLOR}; --uses: {line.fillerUses};"
                  >
                    <span class="zone-text">
                      {rotation.filler.skill_name} × {fmtInt(line.fillerUses)}({fmtNum(line.fillerSeconds, 1, "s")})
                    </span>
                  </span>
                {/if}
                {#if line.restPct > 0.5}
                  <span class="zone rest" style="width: {line.restPct}%;">
                    <span class="zone-text">{line.restLabel}</span>
                  </span>
                {/if}
              </span>
              <!-- 秒の目盛り: 0s / 差し込みが終わる秒 / 1 周の秒 -->
              <span class="scale">
                <span class="mark start"><Value value="0s" /></span>
                {#if line.insertPct > 6 && line.insertPct < 94}
                  <span class="mark mid" style="left: {line.insertPct}%;">
                    <Value motion={() => line.seconds} value={fmtNum(line.seconds, 1, "s")} />
                  </span>
                {/if}
                <span class="mark end">
                  <Value motion={() => line.total} value={fmtNum(line.total, 1, "s")} />
                </span>
              </span>
            </div>
          {/each}
        </div>
        <div class="rot-rows">
          {#each parts as p (p.key)}
            <div class="rot-row">
              <span class="dot" style="background: {p.color};"></span>
              <Icon kind="skill" id={p.skillId} size={20} label={p.name} />
              <span class="rot-name">{p.name}</span>
              <span class="rot-role">{p.role}</span>
              <span class="rot-detail">{p.detail}</span>
              <Value class="rot-dps" motion={() => Math.round(p.expectedDps)} value={fmtInt(Math.round(p.expectedDps))} />
              <Value class="rot-share dim" motion={() => p.dpsShare * 100} value={fmtPct(p.dpsShare)} />
            </div>
          {/each}
        </div>
        {#if rotation?.crowded}
          <p class="rot-note dim">差し込む技だけで時間が埋まり、連打する余裕がありません(全部は CT どおりに撃てません)。</p>
        {/if}
      </div>
    {:else}
      <p class="rot-note dim">差し込む技を選ぶと、連打の合間に差し込んだときの DPS を出します。</p>
    {/if}
  </div>
{/if}

<style>
  /* 読み取り専用の値(帯・寄与)なのでインセット面 + 上端のハイライト 1 本(§01 / §08)。
     色で面を塗らない —— 意味のある帯(金・ラベンダー)の枠を食わないため(§02) */
  .rotation {
    margin-top: 8px; padding: 7px 9px 8px;
    display: flex; flex-direction: column; gap: 7px;
    box-shadow: inset 0 1px 0 #fff;
  }
  .rot-pick { display: flex; flex-wrap: wrap; align-items: center; gap: 7px; min-width: 0; }
  .rot-title {
    flex-shrink: 0; display: inline-flex; align-items: baseline; gap: 5px;
    font-size: var(--t-label); font-weight: 800; letter-spacing: 0.1em; color: var(--fg-muted);
  }
  /* 保存されない操作なのでラベンダー(§03)。行の高さは変えない */
  .rot-reset {
    flex-shrink: 0; padding: 3px 9px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--sim); color: var(--sim-fg);
    font-size: var(--t-label); font-weight: 700; white-space: nowrap;
  }
  .rot-reset:hover { background: var(--state-temp-bg); }
  .rot-pick :global(.chip-ct) { color: var(--fg-sub); font-size: var(--t-label); }
  .rot-pick :global(.chip-gain) { margin-left: 5px; min-width: 5.5em; text-align: right; }
  .rot-pick :global(.drop-badge) {
    background: var(--state-temp-bg); border: 1px solid var(--sim); color: var(--sim-fg);
  }
  .rot-pick :global(details.rot-drops) { min-width: 0; }
  .rot-pick :global(details.rot-drops > summary) { display: inline-flex; }
  .rot-pick :global(details.rot-drops .chiprow) { margin-top: 5px; }

  .rot-body { display: flex; flex-direction: column; gap: 7px; }

  /* 1 周のタイムライン。読み取り専用の値なのでインセットの溝に区画を載せる(§08)。
     幅の動きは進捗バーと同じ間合い(--dur-bar)で、数値と同時に動く */
  .rot-lines { display: flex; flex-direction: column; gap: 7px; min-width: 0; }
  .rot-line { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .line-head {
    display: flex; align-items: baseline; gap: 6px;
    font-size: var(--t-label); font-weight: 700; color: var(--fg-sub);
  }
  .line-head :global(.line-total) { font-weight: 800; color: var(--fg); }
  .track {
    position: relative; display: flex; height: 18px; overflow: hidden;
    border-radius: var(--r-inset);
  }
  .zone {
    position: relative; min-width: 0; display: flex; align-items: center;
    overflow: hidden; transition: width var(--dur-bar) var(--ease-in-out);
  }
  .zone + .zone { box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.85); }
  /* 連打の区画は 1 回ごとに細い刻み(`--uses` 等分)。刻みが 3px 未満になるときは
     `ticked` を外して回数の文字だけにする(数えられない線は置かない) */
  .filler.ticked {
    background-image: linear-gradient(90deg, rgba(255, 255, 255, 0.75) 0 1px, transparent 1px 100%);
    background-size: calc(100% / var(--uses)) 100%;
  }
  /* 連打も差し込みも入らない時間(CT 待ち・詰まっているぶん)。塗らずに溝のまま見せる */
  .rest { background: repeating-linear-gradient(135deg, #DCE5F1 0 5px, #CFDAEA 5px 10px); }
  .zone-text {
    padding: 0 5px; min-width: 0;
    font-size: var(--t-label); font-weight: 700; color: #fff; white-space: nowrap;
    overflow: hidden; text-overflow: ellipsis;
  }
  .rest .zone-text { color: var(--fg-muted); }
  /* 目盛り。数値は tabular-nums(Value が .num を載せる)。区切りの位置に中央で置く */
  .scale { position: relative; height: 13px; font-size: var(--t-label); color: var(--fg-muted); }
  .mark { position: absolute; top: 0; white-space: nowrap; }
  .mark.start { left: 0; }
  .mark.mid { transform: translateX(-50%); }
  .mark.end { right: 0; }

  .rot-rows { display: flex; flex-direction: column; gap: 3px; }
  .rot-row { display: flex; align-items: center; gap: 7px; min-width: 0; font-size: var(--t-body); }
  .rot-row .dot { flex: none; width: 8px; height: 8px; border-radius: 50%; }
  .rot-name { flex: 1 1 auto; min-width: 0; font-weight: 700; color: var(--fg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rot-role { flex: none; min-width: 6.5em; font-size: var(--t-label); color: var(--fg-sub); white-space: nowrap; }
  .rot-detail { flex: none; min-width: 0; font-size: var(--t-label); color: var(--fg-sub); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* 数値欄は幅固定 + tabular-nums(Value が .num を載せる)。桁が増えても行が動かない */
  .rot-rows :global(.rot-dps) { flex: none; min-width: 7.5em; text-align: right; font-weight: 700; }
  .rot-rows :global(.rot-share) { flex: none; min-width: 3.4em; text-align: right; font-size: var(--t-label); }
  .rot-note { font-size: var(--t-label); line-height: 1.5; }

  @media (max-width: 720px) {
    .rot-row { flex-wrap: wrap; }
  }
</style>
