<script lang="ts">
  // 防御側パネル(規格シート 5c)。攻撃タブと同列で「自分がどれだけ耐えるか」を出す。
  // 計算は Rust 側(crates/domain/src/defense.rs)。ここは表示だけ。
  import type { DefenseProfile } from "../../api/types";
  import { fmtInt, fmtNum } from "../../format";
  import { limits } from "../../limits.svelte";
  import { bump } from "../../ui/motion.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";

  interface Props {
    profile: DefenseProfile | null;
    error: string | null;
  }
  let { profile, error }: Props = $props();

  const pct = (v: number) => `${fmtNum(Math.round(v * 1000) / 10)}%`;
  // 防御力の上限で捨てられた分(3 種のどれかが 0 超なら注記を出す)
  const capLoss = $derived(
    profile === null
      ? 0
      : profile.physical_defense_loss + profile.magic_defense_loss + profile.composite_defense_loss,
  );
</script>

<SheetCard tone="blue" title="どれだけ耐える？" note="対象コンテンツに依らない自分の値です" busy={!error && !profile}>
  {#if error}
    <p class="err">{error}</p>
  {:else if !profile}
    <!-- まだ値が来ていない。文言は出さず(待っていることは見出しの印が伝える)、
         行の高さだけ確保して、値が入った瞬間に下がずれないようにする -->
    <p class="empty dim" aria-hidden="true">&nbsp;</p>
  {:else}
    <div class="block">
      <div class="block-head">
        <span class="block-title">防御力</span>
        <span class="formula num dim">
          [ステ×{limits.defense_stat_multiplier} + 装備防御×倍率×{limits.defense_equipment_multiplier}]
          (複合は (DEF+MR)×{limits.composite_defense_stat_multiplier} + 装備×{limits.composite_defense_equipment_multiplier})
        </span>
      </div>
      <div class="rows">
        <div class="row">
          <span class="rl">物理防御力</span>
          <span class="num rv" use:bump={() => profile.physical_defense}>{fmtInt(profile.physical_defense)}</span>
          <span class="rn dim">
            DEF×{limits.defense_stat_multiplier} + 装備物防 {fmtInt(profile.equipment_physical_defense)}×{fmtNum(profile.defense_rates.physical)}×{limits.defense_equipment_multiplier}
          </span>
        </div>
        <div class="row">
          <span class="rl">魔法防御力</span>
          <span class="num rv" use:bump={() => profile.magic_defense}>{fmtInt(profile.magic_defense)}</span>
          <span class="rn dim">
            MR×{limits.defense_stat_multiplier} + 装備魔防 {fmtInt(profile.equipment_magic_defense)}×{fmtNum(profile.defense_rates.magic)}×{limits.defense_equipment_multiplier}
          </span>
        </div>
        <div class="row">
          <span class="rl">複合防御力</span>
          <span class="num rv" use:bump={() => profile.composite_defense}>{fmtInt(profile.composite_defense)}</span>
          <span class="rn dim">(DEF+MR)×{limits.composite_defense_stat_multiplier} + 装備×{limits.composite_defense_equipment_multiplier}</span>
        </div>
        <div class="row">
          <span class="rl">装備防御力倍率</span>
          <span class="num rv" use:bump={() => profile.defense_rates.physical}>物 {fmtInt(profile.defense_rates.physical * 100)}%</span>
          <span class="rn dim">
            魔 {fmtInt(profile.defense_rates.magic * 100)}%。共通スキル(コートアーマー / プロテクトアーマー)+
            シエナのオーラの防御力増加。<b>リンゴの島・ベリネンルミでは常に 100%</b>
          </span>
        </div>
        <div class="row">
          <span class="rl">防御力の上限</span>
          <span class="num rv" use:bump={() => profile.defense_cap}>{fmtInt(profile.defense_cap)}</span>
          <span class="rn dim">覚醒段階 + エタの意志 Lv で開放(wiki: Quest/覚醒クエスト・エタの意志)</span>
        </div>
      </div>
      {#if capLoss > 0}
        <p class="note dim">
          上限で捨てられた分: 物理 {fmtInt(profile.physical_defense_loss)} ・
          魔法 {fmtInt(profile.magic_defense_loss)} ・ 複合 {fmtInt(profile.composite_defense_loss)}。
          ここから先の軽減はカット率 J が担います。
        </p>
      {/if}
    </div>

    <div class="block">
      <div class="block-head">
        <span class="block-title">カット率(与ダメージ式の J)</span>
        <span class="formula num dim">
          r = 1 − a / (a + {limits.cut_rate_denominator})、a = {limits.cut_rate_a_base} + [(防御ステ + 装備防御 − 1) / {limits.cut_rate_divisor}]
        </span>
      </div>
      <div class="rows">
        <div class="row">
          <span class="rl">物理</span>
          <span class="num rv" use:bump={() => profile.physical_cut_rate}>{pct(profile.physical_cut_rate)}</span>
          <span class="rn dim">DEF + 装備物防 から</span>
        </div>
        <div class="row">
          <span class="rl">魔法</span>
          <span class="num rv" use:bump={() => profile.magic_cut_rate}>{pct(profile.magic_cut_rate)}</span>
          <span class="rn dim">MR + 装備魔防 から</span>
        </div>
        <div class="row">
          <span class="rl">複合</span>
          <span class="num rv" use:bump={() => profile.composite_cut_rate}>{pct(profile.composite_cut_rate)}</span>
          <span class="rn dim">DEF + 装備物防 + MR + 装備魔防 から(除数 {limits.cut_rate_composite_divisor})</span>
        </div>
      </div>
      <p class="note dim">防御力には上限があり、上限に届いたあとの軽減はこのカット率が担います。</p>
    </div>

    <div class="block">
      <div class="block-head">
        <span class="block-title">回避</span>
        <span class="formula num dim">
          回避P = [{limits.evasion_point_base} + (AGI + 装備回避率)×{limits.evasion_point_agi_rate} + 装備敏捷度/{limits.evasion_type_divisor} + 攻撃タイプ別増加]
        </span>
      </div>
      <div class="rows">
        <div class="row">
          <span class="rl">回避P(物理)</span>
          <span class="num rv" use:bump={() => profile.evasion_point.physical}>{fmtInt(profile.evasion_point.physical)}</span>
          <span class="rn dim">+ (DEF×2 + [(突き+斬り)/{limits.evasion_physical_attack_divisor}]) / {limits.evasion_type_divisor}</span>
        </div>
        <div class="row">
          <span class="rl">回避P(魔法)</span>
          <span class="num rv" use:bump={() => profile.evasion_point.magic}>{fmtInt(profile.evasion_point.magic)}</span>
          <span class="rn dim">+ MR×2 / {limits.evasion_type_divisor}</span>
        </div>
        <div class="row">
          <span class="rl">回避P(複合)</span>
          <span class="num rv" use:bump={() => profile.evasion_point.composite}>{fmtInt(profile.evasion_point.composite)}</span>
          <span class="rn dim">
            + (DEF+MR) / {limits.evasion_type_divisor}。装備回避率 {fmtInt(profile.equipment_evasion)}×{limits.evasion_point_agi_rate} + 装備敏捷度 {fmtInt(profile.equipment_agility)}/{limits.evasion_type_divisor} を含む
          </span>
        </div>
        <div class="row">
          <span class="rl">特殊回避(コンボ)</span>
          <span class="num rv" use:bump={() => profile.combo_evasion}>{pct(profile.combo_evasion)}</span>
          <span class="rn dim">(10 + MR/15 + AGI/7.5)%、下限 20% / 上限 63%</span>
        </div>
      </div>
      <p class="note dim">
        通常回避「率」は敵の命中Pが要り、その入力(wiki 狩り場情報一覧「上限回避P」)が全行未記載なので出しません。
        回避Pを上げるほど当たりにくくなります(上限 85%)。特殊回避は成功すると多段攻撃の全段を回避します。
      </p>
    </div>
  {/if}
</SheetCard>

<style>
  /* .sheet-card/.sheet-head/.gem/.sheet-title/.sheet-char は ui/SheetCard.svelte */
  .empty, .err { margin: 0; padding: 16px 13px; font-size: 11px; }
  .err { color: var(--danger); }

  .block { padding: 11px 13px; border-bottom: 1px solid var(--border-soft); }
  .block:last-child { border-bottom: 0; }
  .block-head { display: flex; align-items: baseline; gap: 9px; flex-wrap: wrap; }
  .block-title { font-size: 11px; font-weight: 700; color: var(--fg-head); white-space: nowrap; }
  .formula { font-size: 9px; line-height: 1.5; }

  .rows { margin-top: 7px; display: flex; flex-direction: column; gap: 4px; }
  .row {
    display: flex; align-items: baseline; gap: 10px; padding: 5px 9px;
    background: var(--surface-inset); border-radius: var(--r-inset); border: 1px solid var(--border-soft);
  }
  .rl { flex-shrink: 0; width: 120px; font-size: 10px; color: var(--fg-muted); }
  .rv { flex-shrink: 0; width: 84px; text-align: right; font-size: 13px; font-weight: 700; }
  .rn { min-width: 0; font-size: 9px; line-height: 1.5; }
  .note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }
</style>
