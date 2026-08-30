<script lang="ts">
  // 「thesis」補正源のペイン。テシスコア(地域ごとに 6 枠)。
  import type { CoreRegion, CoreType, StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { zeroValues } from "../../../equipment";
  import { fmtInt } from "../../../format";
  import { CORE_POWER_TYPES, CORE_REGIONS, CORE_REGION_LABELS, CORE_SLOT_COUNT, CORE_SUPPORT_TYPES, CORE_TYPE_LABELS } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { bump, flash } from "../../../ui/motion.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
  }
  let { draft, preview }: Props = $props();

  // --- テシスコア(地域ごとに 6 枠) ---------------------------------------
  let coreRegion = $state<CoreRegion>("abyss");
  const coreSlotIndexes = Array.from({ length: CORE_SLOT_COUNT }, (_, i) => i);
  // 選ぶのは火力 4 タイプがほとんど。補助タイプ(物防・回避・敏捷・命中)は装備攻撃力に
  // 入らないので、ふだんは畳んでおく(§00「要らないものを見せない」)。
  // 既に補助タイプが入っている枠があるときは、畳んだままだと選択中の段が消えるので開く
  let coreShowSupport = $state(false);
  // 「未装着」は段に入れない。ほぼ選ばれないものが常に 1 列を占めるのは割に合わない。
  // 外すのは行末の小さな × 1 つで足りる(§00 02「要らないものを見せない」)
  const corePowerOptions = CORE_POWER_TYPES.map((t) => ({ value: t, label: CORE_TYPE_LABELS[t] }));
  // ラベルに「(補助)」を付けない。段が分かれていて、上にも注記がある — 4 回同じ語を読ませない
  const coreSupportOptions = CORE_SUPPORT_TYPES.map((t) => ({
    value: t,
    label: CORE_TYPE_LABELS[t],
  }));
  const coreSupportInUse = $derived(
    draft.equipment.thesis_cores[coreRegion].slots.some(
      (c) => c !== null && !CORE_POWER_TYPES.includes(c.core_type),
    ),
  );
  /** 補助の段を閉じるときは、補助タイプの枠も外す。
      見えない段に選択が残ると、画面に出ていない値が効き続ける */
  function toggleCoreSupport() {
    if (!(coreShowSupport || coreSupportInUse)) {
      coreShowSupport = true;
      return;
    }
    const slots = draft.equipment.thesis_cores[coreRegion].slots;
    slots.forEach((core, i) => {
      if (core && !CORE_POWER_TYPES.includes(core.core_type)) slots[i] = null;
    });
    coreShowSupport = false;
  }
  const coreTypeOptions = $derived(
    coreShowSupport || coreSupportInUse ? [...corePowerOptions, ...coreSupportOptions] : corePowerOptions,
  );
  // 進化と強化は別々に選ばせず「4-4」で 1 回で決める(5×5 = 25 通り)。
  // 押した枠の下に重ねて出すので、他の枠は動かない(§09 規則 3)
  let openCoreStage = $state<number | null>(null);
  const coreStagePairs = $derived(
    Array.from({ length: limits.core_evolution_max + 1 }, (_, ev) =>
      Array.from({ length: limits.core_enhancement_max + 1 }, (_, en) => ({ ev, en })),
    ),
  );
  /** コア 1 個の補正値(表示用)。テーブル自体は Rust 側のデータ(limits.core_*_bonus_table)。 */
  const coreBonus = (type: CoreType, evolution: number, enhancement: number): number => {
    const table = CORE_POWER_TYPES.includes(type) ? limits.core_power_bonus_table : limits.core_support_bonus_table;
    return table[evolution]?.[enhancement] ?? 0;
  };
  // 入場条件「コア N」の説明文で使う例(火力: 進化1強化4 / 進化4強化4、補助: 進化4強化4)。数値は limits の表から引く
  const powerEvo1En4 = $derived(coreBonus("thrust", 1, limits.core_enhancement_max) * limits.core_slot_count);
  const powerEvo4En4 = $derived(
    coreBonus("thrust", limits.core_evolution_max, limits.core_enhancement_max) * limits.core_slot_count,
  );
  const supportEvo4En4 = $derived(
    coreBonus("physical_defense", limits.core_evolution_max, limits.core_enhancement_max),
  );
  const supportEvo4En4Total = $derived(supportEvo4En4 * limits.core_slot_count);
  const coreAt = (index: number) => draft.equipment.thesis_cores[coreRegion].slots[index] ?? null;
  function setCoreType(index: number, value: string) {
    const slots = draft.equipment.thesis_cores[coreRegion].slots;
    slots[index] = value === "" ? null : { core_type: value as CoreType, evolution: 0, enhancement: 0 };
  }
  function setCoreStagePair(index: number, evolution: number, enhancement: number) {
    const core = draft.equipment.thesis_cores[coreRegion].slots[index];
    if (!core) return;
    core.evolution = evolution;
    core.enhancement = enhancement;
  }
  // 補助タイプは与ダメージ(攻撃力)には効かないが、装備値 9 種として防御側・回避Pに効く
  // 地域ごとのコアセット効果はタブが持つ(ゲーム内 UI の地域カードと同じ)。
  // 全地域の合計は「いまの実力」に出す — 結果を入力エリアに積まない。計算は Rust 側(preview)
  const coreRegionPreview = (region: CoreRegion) =>
    preview?.thesis_cores.find((r) => r.region === region) ?? null;
  const coreRegionTotal = (region: CoreRegion) => coreRegionPreview(region)?.total_bonus ?? 0;
  const coreSetOf = (region: CoreRegion) => coreRegionPreview(region);
  /** その地域のコアセット効果(タブに出す短い形)。進化段階ごとの分は合算済み */
  const coreSetLabelOf = (region: CoreRegion) => {
    const e = coreSetOf(region);
    if (!e || e.set_groups.length === 0) return "";
    const parts: string[] = [];
    if (e.set_bonus.final_damage_rate > 0) parts.push(`+${Math.round(e.set_bonus.final_damage_rate * 100)}%`);
    if (e.set_bonus.final_damage_fixed > 0) parts.push(`+${fmtInt(e.set_bonus.final_damage_fixed)}`);
    return parts.join(" ");
  };
  const coreSupport = $derived(coreRegionPreview(coreRegion)?.values ?? zeroValues());
  const coreSupportSummary = $derived(
    [
      ["物防", coreSupport.physical_defense],
      ["回避", coreSupport.evasion],
      ["敏捷", coreSupport.agility],
      ["命中", coreSupport.accuracy],
    ]
      .filter(([, v]) => (v as number) > 0)
      .map(([label, v]) => `${label} +${fmtInt(v as number)}`)
      .join(" ・ "),
  );
</script>

<div class="card">
  <div class="card-title">地域</div>
  <!-- 地域は「同じ形の 6 枠を切り替える」ので §08 のタブ。選んだ地域の下と地続きになる -->
  <div class="tabs">
    {#each CORE_REGIONS as region (region)}
      <button
        type="button"
        class="tab"
        class:on={coreRegion === region}
        onclick={() => (coreRegion = region)}
      >
        {CORE_REGION_LABELS[region]}
        <span class="num dim" use:bump={() => coreRegionTotal(region)}>{fmtInt(coreRegionTotal(region))}</span>
        {#if (coreSetOf(region)?.set_groups.length ?? 0) > 0}
          <span class="tab-set num" use:flash={() => coreSetLabelOf(region)}>{coreSetLabelOf(region)}</span>
        {:else if coreRegionTotal(region) > 0}
          <span class="tab-set off num">あと {3 - (coreSetOf(region)?.ready ?? 0)}</span>
        {/if}
      </button>
    {/each}
  </div>
  <div class="tab-rule"></div>
  <!-- 説明は毎回読むものではない。畳んで、入力の場所を押し下げないようにする(§00 02) -->
  <details class="fold">
    <summary>この画面の読み方(wiki テシスコア)</summary>
    <div class="fold-body">
      <p class="hint dim">
        コアの能力値増加は対象ダンジョン内でのみ有効なので、計算対象のコンテンツに
        対応する地域のコアだけが装備攻撃力に入ります。コアセット効果(最終ダメージ)は全地域で発動し、
        地域ごとの発動分が足されます。
      </p>
      <p class="hint dim">
        補助タイプ(物防/回避/敏捷/命中)も装着状態として記録できます。与ダメージ式の装備係数が 0 なので
        装備攻撃力には入らず、入場条件「コア N」の合計と防御タブ(防御力・カット率・回避P)に効きます。
        経験値タイプのみのシオカンヘイムコアは火力にもセット効果にも効かないため地域を持ちません。
      </p>
      <p class="hint dim">
        入場条件の「コア N」はこの {limits.core_slot_count} 枠の合計と同じ値です(火力の進化1強化{limits.core_enhancement_max}
        ×{limits.core_slot_count} = {powerEvo1En4}、進化{limits.core_evolution_max}強化{limits.core_enhancement_max}
        ×{limits.core_slot_count} = {powerEvo4En4}。補助タイプは進化{limits.core_evolution_max}強化{limits.core_enhancement_max}
        でも {supportEvo4En4} なので {limits.core_slot_count} 枠でも {supportEvo4En4Total} 止まり)。
        コアセット効果は強化 4 段階のコアが 3 個以上そろうと発動します(タイプは問いません)。
      </p>
    </div>
  </details>
</div>
{#key coreRegion}
<div class="card swap-in">
  <div class="card-title inline">
    {CORE_REGION_LABELS[coreRegion]} の 6 枠
    <!-- 「補助も出す」は段の見え方を変える操作なので、段より先に目に入る位置に置く。
         控えめなチップ 1 つ(§07 形態 3)。下に置くと、段を見たあとで見え方が変わって読み直しになる -->
    <button
      type="button"
      class="chip quiet"
      class:on={coreShowSupport || coreSupportInUse}
      onclick={toggleCoreSupport}
    >{coreShowSupport || coreSupportInUse ? "補助タイプを閉じる" : "補助タイプも出す"}</button>
  </div>
  <!-- この画面で知りたいのは「いくつになったか」と「セット効果が出ているか」の 2 つ。
       小さな注記ではなく、段より先に読める場所に出す -->
  {#if coreSupportSummary}
    <p class="hint dim">
      このうち補助タイプ({coreSupportSummary})は装備攻撃力には入らず、防御タブの防御力・カット率・回避Pに効きます。
    </p>
  {/if}
  <!-- 列の名前は 1 回だけ。行ごとにラベルを置くと、6 回同じ言葉を読ませることになる -->
  <div class="core-head">
    <span></span><span>タイプ</span><span></span><span class="lead">進化 - 強化</span><span class="r">コア効果</span>
  </div>
  <div class="core-list">
    {#each coreSlotIndexes as index (index)}
      {@const core = coreAt(index)}
      <div class="core-row">
        <span class="core-slot dim">{index + 1}</span>
        <!-- 6 枠が同じ列で並ぶように列を固定する。行ごとに幅が違うと端を探し直す(§00 01)。
             補助タイプは別の段にする — 1 つの段に 9 個入れると列が余って空きセルが出る -->
        <span class="core-types">
          <StepSelect
            label=""
            options={corePowerOptions}
            cols={4}
            bind:value={() => core?.core_type ?? "", (v) => setCoreType(index, v)}
          />
          {#if coreShowSupport || coreSupportInUse}
            <!-- 段が増えるのは「開いた」なので下に伸ばす(§10 型 6) -->
            <div class="open-in">
              <StepSelect
                label=""
                options={coreSupportOptions}
                cols={4}
                bind:value={() => core?.core_type ?? "", (v) => setCoreType(index, v)}
              />
            </div>
          {/if}
        </span>
        <!-- 進化 - 強化。押すと 5×5 が重なって出るので、押した枠は動かない(§09 規則 3) -->
        <button
          type="button"
          class="core-clear"
          disabled={core === null}
          onclick={() => setCoreType(index, "")}
        >外す</button>
        <span class="core-stage">
          <button
            type="button"
            class="stage-trigger num"
            disabled={core === null}
            aria-label="進化と強化"
            onclick={() => (openCoreStage = openCoreStage === index ? null : index)}
          >
            <span use:flash={() => (core ? `${core.evolution}-${core.enhancement}` : "-")}>
              {core ? `${core.evolution}-${core.enhancement}` : "—"}
            </span>
          </button>
          {#if openCoreStage === index && core}
            <button type="button" class="stage-overlay" aria-label="閉じる" onclick={() => (openCoreStage = null)}></button>
            <!-- 下の枠は上に開く。下に開くとペインの外へ出て、選ぶのにスクロールが要る -->
            <div class="stage-pop pop-in" class:up={index >= 3}>
              <div class="stage-pop-h">進化 - 強化</div>
              <div class="stage-grid">
                {#each coreStagePairs as row (row[0].ev)}
                  {#each row as p (p.en)}
                    <!-- 段の名前だけだと「それでいくつになるのか」を毎回考えることになる。
                         結果(補正値)をその場に小さく乗せる(§00 05「考えさせない」) -->
                    <button
                      type="button"
                      class="stage-cell"
                      class:on={core.evolution === p.ev && core.enhancement === p.en}
                      onclick={() => { setCoreStagePair(index, p.ev, p.en); openCoreStage = null; }}
                    >
                      <b class="num">{p.ev}-{p.en}</b>
                      <span class="cell-bonus num">+{fmtInt(coreBonus(core.core_type, p.ev, p.en))}</span>
                    </button>
                  {/each}
                {/each}
              </div>
            </div>
          {/if}
        </span>
        <span
          class="core-bonus num"
          class:support={core !== null && !CORE_POWER_TYPES.includes(core.core_type)}
          use:bump={() => (core ? coreBonus(core.core_type, core.evolution, core.enhancement) : null)}
        >
          {core ? `+${fmtInt(coreBonus(core.core_type, core.evolution, core.enhancement))}` : "—"}
        </span>
      </div>
    {/each}
  </div>

</div>
{/key}
