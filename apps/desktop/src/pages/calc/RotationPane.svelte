<script lang="ts">
  // 回しの段(計算タブの主役カード「DPS(回し)」の下に常設)。
  //
  // 出すのは 3 つ: **その場で試す差し込みチップ**・**本体のスキル回し(1 本のタイムライン)**・**技ごとの寄与**。
  // 秒・回数・寄与・分類はすべて Rust(`domain::plan_rotation` / `rotation_shares` →
  // `commands::Rotation`)が返したものをそのまま出す。**ここでするのは「秒 ÷ 間隔」を
  // 幅(%)に換算することだけ**で、時間の配分そのものを組み立て直さない。
  // 区画の**幅は実際の秒数**で、割合の帯ではない —— 割合の帯 2 本(時間 / DPS)は
  // 実機で見方が伝わらなかった(2026-09-21)。
  //
  // チップは**保存しない**(計算タブの原則)。ラベンダー = 保存されない(§03)。
  // チップ行は段のいちばん上に置く —— 押して下の行数が変わっても、押した場所は動かない(§00 ③)。
  import type { Rotation, RotationChoices, Skill } from "../../api/types";
  import { fmtInt, fmtNum, fmtPct } from "../../format";
  import RotationChips from "../../RotationChips.svelte";
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
    /** 召喚獣(精霊)を出しているか。陣で消える精霊の「不在 / 攻撃中」の帯はこのときだけ描く */
    hasSummon?: boolean;
  }
  let { rotation, choices, onIds, onToggle, overridden, onReset, skills, hasSummon = false }: Props = $props();

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
    /** 1 回の所要時間・チャネリングの tick */
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
        // 陣は「置く + 精霊の呼び直し」で 1 回(どちらの秒も Rust が返す)
        detail: insert.resummon_seconds > 0
          ? `1 回 ${fmtNum(insert.seconds, 2, "s")}(置く ${fmtNum(insert.cast_seconds, 2, "s")} + ${insert.resummon_skill_name ?? "召喚"} ${fmtNum(insert.resummon_seconds, 2, "s")})`
          : `1 回 ${fmtNum(insert.seconds, 2, "s")}` + tickNote(insert.skill_id)
            // 自分のダメージは 0。得は精霊の側に入る(額は精霊の鎖と合計の精霊の行)
            + (insert.summon_hit_bonus_seconds > 0
              ? ` ・ 精霊に ${fmtNum(insert.summon_hit_bonus_seconds, 0, "s")} 追加ダメージ`
              : ""),
        expectedDps: insert.expected_dps,
        dpsShare: insert.dps_share,
      });
    });
    if (filler) {
      // 「合間に × n 回」は差し込みが 1 つのときだけ言える(2 つ以上は周期ごとに違う)
      const single = rotation.inserts.length === 1 ? rotation.inserts[0] : null;
      list.push({
        key: `filler:${filler.skill_id}`,
        skillId: filler.skill_id,
        name: filler.skill_name,
        color: FILLER_COLOR,
        role: single && single.filler_uses > 0 ? `合間に × ${fmtInt(single.filler_uses)} 回` : "連打",
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

  // --- 本体のスキル回し(1 本のタイムライン) ---------------------------------
  // 本体が実際に撃つ順を 1 本の時間軸に並べる。区画の時刻・秒・回数と召喚獣の状態は
  // すべて Rust(`domain::rotation_timeline` → `Rotation.timeline`)が返したもので、
  // ここでするのは「秒 ÷ 軸の長さ」を幅(%)に換算することと、区画に色と名前を当てることだけ。
  /** 連打 1 回ぶんの刻みを描くかの下限(px)。これより細いと数えられないので出さない */
  const TICK_MIN_PX = 3;
  /** 帯の実幅(px)。刻みを出すかの判定に使う */
  let trackWidth = $state(0);

  interface Zone {
    key: string;
    /** 区画の種類(描き分け用) */
    kind: "insert" | "resummon" | "filler" | "idle";
    pct: number;
    color: string;
    /** 帯の中に書く文字(細くて入らなければ省略される) */
    text: string;
    /** カーソルを合わせると出る名前と秒 */
    title: string;
    uses: number;
    ticked: boolean;
  }
  /** 秒を帯の幅(%)に換算する。割合そのものは Rust に無い「見せ方」なのでここで作る */
  const pctOf = (seconds: number, total: number) => (total > 0 ? (seconds / total) * 100 : 0);
  const zones = $derived.by<Zone[]>(() => {
    if (!rotation) return [];
    const { total_seconds: total, segments } = rotation.timeline;
    return segments.map((seg, i) => {
      const pct = pctOf(seg.seconds, total);
      const sec = fmtNum(seg.seconds, 1, "s");
      const key = `z:${i}`;
      if (seg.kind === "insert" || seg.kind === "resummon") {
        const insert = rotation.inserts[seg.slot];
        if (seg.kind === "resummon") {
          const name = insert?.resummon_skill_name ?? "召喚";
          return { key, kind: "resummon", pct, color: "", text: `${name} ${sec}`, title: `${name} ${sec}(精霊の呼び直し)`, uses: 0, ticked: false };
        }
        const name = insert?.skill_name ?? "";
        return {
          key, kind: "insert", pct, color: INSERT_COLORS[seg.slot % INSERT_COLORS.length],
          text: `${name} ${sec}`, title: `${name} ${sec}`, uses: 0, ticked: false,
        };
      }
      if (seg.kind === "filler") {
        const name = rotation.filler?.skill_name ?? "";
        const perUsePx = seg.uses > 0 ? (trackWidth * pct) / 100 / seg.uses : 0;
        const text = `${name} × ${fmtInt(seg.uses)}(${sec})`;
        return { key, kind: "filler", pct, color: FILLER_COLOR, text, title: text, uses: seg.uses, ticked: perUsePx >= TICK_MIN_PX };
      }
      return { key, kind: "idle", pct, color: "", text: "待ち", title: `待ち ${sec}(CT が明くのを待つ)`, uses: 0, ticked: false };
    });
  });
  /** 同じ軸での召喚獣の状態(陣で不在 / 追加ダメージ中 / 攻撃中)と、状態ごとの合計秒 */
  const summonZones = $derived(
    rotation ? rotation.timeline.summon.map((s) => ({ ...s, pct: pctOf(s.seconds, rotation!.timeline.total_seconds) })) : [],
  );
  const summonTotals = $derived.by(() => {
    const t = { present: 0, absent: 0, bonus: 0 };
    for (const s of summonZones) t[s.state] += s.seconds;
    return t;
  });
  const SUMMON_LABEL = { present: "攻撃中", absent: "不在", bonus: "追加ダメージ" } as const;

  const candidates = $derived(choices?.candidates ?? []);
  // 候補が「差し込むと DPS が下がる技」だけなら段ごと出さない(§00 ②。選ぶ意味のある技が無い)。
  // 下がる技を ON にしている(スキル回しがある)ときと、ここで試している最中は出したままにする —
  // 押したチップが OFF にした瞬間に段ごと消えないように(§00 ③)
  const visible = $derived(
    parts.length > 0 || overridden || candidates.some((c) => c.effect !== "reduces"),
  );
</script>

{#if visible}
  <div class="rotation inset">
    <!-- 押した場所は動かない(§00 ③): チップ行は段のいちばん上。下の行数が変わっても動かない -->
    <div class="rot-pick">
      <span class="rot-title">
        スキル回し
        <!-- 上限は無いので「n / 候補数」を値の隣に常設する(§07)。候補が無い回しでは出さない -->
        {#if candidates.length > 0}
          <Value
            class="dim normal"
            motion={() => onIds.length}
            value={`${onIds.length} / ${candidates.length}`}
          />
        {/if}
      </span>
      <!-- チップ一式はキャラタブと同じ部品(振り分けは Rust の `effect`)。ここでの選択は
           保存しないので `temporary`(ラベンダー = 保存されない。§03) -->
      <RotationChips
        {candidates}
        {onIds}
        {onToggle}
        expectedDps={choices?.expected_dps ?? null}
        temporary
      >
        {#snippet dropHint()}
          <p class="rot-note dim">ここの技は、差し込むと連打していたぶんが減って DPS が下がります。</p>
        {/snippet}
      </RotationChips>
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
        <!-- 本体のスキル回し(1 本)。幅は実際の秒数の比。細い区画はカーソルを合わせると名前と秒が出る -->
        <div class="rot-lines" bind:clientWidth={trackWidth}>
          {#if rotation}
            <div class="rot-line">
              <span class="line-head">
                本体のスキル回し
                <Value class="line-total" motion={() => rotation?.timeline.total_seconds ?? 0} value={fmtNum(rotation.timeline.total_seconds, 1, "s")} />
              </span>
              <span class="track inset">
                {#each zones as z (z.key)}
                  <span
                    class="zone {z.kind}"
                    class:ticked={z.ticked}
                    style="width: {z.pct}%;{z.color ? ` background: ${z.color};` : ''}{z.uses > 0 ? ` --uses: ${z.uses};` : ''}"
                    title={z.title}
                  >
                    <span class="zone-text">{z.text}</span>
                  </span>
                {/each}
              </span>
              {#if hasSummon && summonZones.length > 0}
                <!-- 精霊の帯: 同じ時間軸で、陣で不在 / 追加ダメージ中 / 攻撃中。合計の増減の出どころ -->
                <span class="track inset spirit">
                  {#each summonZones as s (s.start_seconds)}
                    <span class="zone {s.state}" style="width: {s.pct}%;" title="精霊 {SUMMON_LABEL[s.state]} {fmtNum(s.seconds, 1, 's')}"></span>
                  {/each}
                </span>
              {/if}
              <!-- 秒の目盛り: 0s / 1 周の秒。精霊の凡例は中央に -->
              <span class="scale">
                <span class="mark start"><Value value="0s" /></span>
                {#if hasSummon && summonZones.length > 0}
                  <span class="mark center spirit-note num">
                    精霊
                    {#each ["absent", "bonus", "present"] as const as state (state)}
                      {#if summonTotals[state] > 0}
                        <span class="sw {state}"></span>{SUMMON_LABEL[state]} {fmtNum(summonTotals[state], 1, "s")}
                      {/if}
                    {/each}
                  </span>
                {/if}
                <span class="mark end">
                  <Value motion={() => rotation?.timeline.total_seconds ?? 0} value={fmtNum(rotation.timeline.total_seconds, 1, "s")} />
                </span>
              </span>
            </div>
          {/if}
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
  .rot-pick :global(.chip-gain) { margin-left: 5px; min-width: 5.5em; text-align: right; }
  .rot-pick :global(.drop-badge) {
    background: var(--state-temp-bg); border: 1px solid var(--sim); color: var(--sim-fg);
  }
  /* 畳んだ先(下がる技)は RotationChips が `drop-pick` で描く。行に溶かして、
     開いたときだけチップが下に出るようにする */
  .rot-pick :global(details.drop-pick) { min-width: 0; }
  .rot-pick :global(details.drop-pick > summary) { display: inline-flex; }
  .rot-pick :global(details.drop-pick .chiprow) { margin-top: 5px; }

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
  .zone.filler.ticked {
    background-image: linear-gradient(90deg, rgba(255, 255, 255, 0.75) 0 1px, transparent 1px 100%);
    background-size: calc(100% / var(--uses)) 100%;
  }
  /* 連打も差し込みも入らない時間(CT 待ち・詰まっているぶん)。塗らずに溝のまま見せる */
  .idle { background: repeating-linear-gradient(135deg, #DCE5F1 0 5px, #CFDAEA 5px 10px); }
  /* 精霊の呼び直し(本体の手順)。差し込みの色の薄い版で「同じ 1 回の続き」に見せる */
  .resummon { background: var(--state-temp-bd); }
  /* 陣・呼び直しは 27 秒のうち 1 秒未満で、比のままだと 1〜2px に消える。色の塊として残るよう下限を置く
     (文字は帯に入れず、右の凡例と下の行が言う) */
  .zone { min-width: 6px; }
  /* 精霊の帯。細く、不在 = 届かない色、攻撃中 = 足りている色(状態色 §03)。文字は帯の外の凡例に */
  .track.spirit { height: 7px; margin-top: 2px; }
  .absent { background: var(--state-short-bd); }
  .present { background: var(--state-met-bd); }
  /* 追加ダメージが乗っている時間。攻撃中より濃い緑で「上乗せ中」を分ける */
  .bonus { background: var(--state-met-fg); }
  .mark.center { left: 50%; transform: translateX(-50%); }
  .spirit-note { display: inline-flex; align-items: center; gap: 4px; color: var(--fg-muted); }
  .sw { display: inline-block; width: 7px; height: 7px; border-radius: 2px; margin-left: 4px; }
  .zone-text {
    padding: 0 5px; min-width: 0;
    font-size: var(--t-label); font-weight: 700; color: #fff; white-space: nowrap;
    overflow: hidden; text-overflow: ellipsis;
  }
  .idle .zone-text { color: var(--fg-muted); }
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
