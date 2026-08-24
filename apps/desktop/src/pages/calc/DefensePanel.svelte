<script lang="ts">
  // 防御側パネル(規格シート 5c)。攻撃タブと同列で「自分がどれだけ耐えるか」を出す。
  // 計算は Rust 側(crates/domain/src/defense.rs)。ここは表示だけ。
  import type { DefenseProfile } from "../../api/types";
  import { fmtInt, fmtNum } from "../../format";

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

<div class="sheet">
  <div class="sheet-head">
    <span class="gem"></span>
    <span class="sheet-title">どれだけ耐える？</span>
    <span class="sheet-char dim">対象コンテンツに依らない自分の値です</span>
  </div>

  {#if error}
    <p class="err">{error}</p>
  {:else if !profile}
    <p class="empty dim">計算中…</p>
  {:else}
    <div class="block">
      <div class="block-head">
        <span class="block-title">防御力</span>
        <span class="formula num dim">[ステ×3 + 装備防御×倍率×6](複合は (DEF+MR)×1.5 + 装備×3)</span>
      </div>
      <div class="rows">
        <div class="row">
          <span class="rl">物理防御力</span>
          <span class="num rv">{fmtInt(profile.physical_defense)}</span>
          <span class="rn dim">DEF×3 + 装備物防 {fmtInt(profile.equipment_physical_defense)}×6</span>
        </div>
        <div class="row">
          <span class="rl">魔法防御力</span>
          <span class="num rv">{fmtInt(profile.magic_defense)}</span>
          <span class="rn dim">MR×3 + 装備魔防 {fmtInt(profile.equipment_magic_defense)}×6</span>
        </div>
        <div class="row">
          <span class="rl">複合防御力</span>
          <span class="num rv">{fmtInt(profile.composite_defense)}</span>
          <span class="rn dim">(DEF+MR)×1.5 + 装備×3</span>
        </div>
        <div class="row">
          <span class="rl">防御力の上限</span>
          <span class="num rv">{fmtInt(profile.defense_cap)}</span>
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
        <span class="formula num dim">r = 1 − a / (a + 80)、a = 3 + [(防御ステ + 装備防御 − 1) / 10]</span>
      </div>
      <div class="rows">
        <div class="row">
          <span class="rl">物理</span>
          <span class="num rv">{pct(profile.physical_cut_rate)}</span>
          <span class="rn dim">DEF + 装備物防 から</span>
        </div>
        <div class="row">
          <span class="rl">魔法</span>
          <span class="num rv">{pct(profile.magic_cut_rate)}</span>
          <span class="rn dim">MR + 装備魔防 から</span>
        </div>
        <div class="row">
          <span class="rl">複合</span>
          <span class="num rv">{pct(profile.composite_cut_rate)}</span>
          <span class="rn dim">除数 20 <span class="tmp">[仮]</span></span>
        </div>
      </div>
      <p class="note dim">防御力には上限があり、上限に届いたあとの軽減はこのカット率が担います。</p>
    </div>

    <div class="block">
      <div class="block-head">
        <span class="block-title">回避</span>
        <span class="formula num dim">回避P = [15 + AGI×1.2 + 攻撃タイプ別増加]、通常回避 = 1 − (敵命中P − 回避P)/100</span>
      </div>
      <div class="rows">
        <div class="row">
          <span class="rl">回避P(物理)</span>
          <span class="num rv">{fmtInt(profile.evasion_point.physical)}</span>
          <span class="rn dim">+ (DEF×2 + [(突き+斬り)/100]) / 7</span>
        </div>
        <div class="row">
          <span class="rl">回避P(魔法)</span>
          <span class="num rv">{fmtInt(profile.evasion_point.magic)}</span>
          <span class="rn dim">+ MR×2 / 7</span>
        </div>
        <div class="row">
          <span class="rl">回避P(複合)</span>
          <span class="num rv">{fmtInt(profile.evasion_point.composite)}</span>
          <span class="rn dim">
            + (DEF+MR) / 7。装備回避率 {fmtInt(profile.equipment_evasion)}×1.2 + 装備敏捷度 {fmtInt(profile.equipment_agility)}/7 を含む
          </span>
        </div>
        <div class="row">
          <span class="rl">通常回避(上限)</span>
          <span class="num rv">{pct(profile.normal_evasion_cap)}</span>
          <span class="rn dim">
            敵命中P が 回避P+15 以下なら上限、回避P+100 以上で 0%(必中)。命中率は下限 15% / 上限 100%
          </span>
        </div>
        <div class="row">
          <span class="rl">特殊回避(コンボ)</span>
          <span class="num rv">{pct(profile.combo_evasion)}</span>
          <span class="rn dim">(10 + MR/15 + AGI/7.5)%、下限 20% / 上限 63%</span>
        </div>
        <div class="row">
          <span class="rl">最終被弾率</span>
          <span class="num rv">{pct(profile.hit_taken_rate_at_cap)}</span>
          <span class="rn dim">上限回避時 (1 − 85%) × (1 − 特殊回避)。回避Pが足りなければこれより悪化</span>
        </div>
      </div>
      <p class="note dim">
        上限回避に必要な回避Pはコンテンツごとに異なります(wiki 狩り場情報一覧「上限回避P」は現状すべて未記載)。
        特殊回避は成功すると多段攻撃の全段を回避します(通常回避は 1 段ずつ判定)。
      </p>
    </div>
  {/if}
</div>

<style>
  .sheet {
    border-radius: var(--r-window); background: var(--bg-panel);
    border: 1px solid var(--border-strong); overflow: hidden;
  }
  .sheet-head {
    display: flex; align-items: center; gap: 8px; padding: 8px 13px;
    background: linear-gradient(180deg, #E9F1FB, #D8E6F6); border-bottom: 1px solid var(--border);
  }
  .gem {
    width: 9px; height: 9px; flex-shrink: 0; transform: rotate(45deg);
    background: var(--head-bar); border: 1px solid #4C6689;
  }
  .sheet-title { font-size: 12px; font-weight: 800; color: #26334A; white-space: nowrap; }
  .sheet-char { margin-left: auto; font-size: 9.5px; min-width: 0; overflow: hidden; text-overflow: ellipsis; }

  .empty, .err { margin: 0; padding: 16px 13px; font-size: 11px; }
  .err { color: var(--danger); }

  .block { padding: 11px 13px; border-bottom: 1px solid var(--border-soft); }
  .block:last-child { border-bottom: 0; }
  .block-head { display: flex; align-items: baseline; gap: 9px; flex-wrap: wrap; }
  .block-title { font-size: 11px; font-weight: 700; color: #26334A; white-space: nowrap; }
  .formula { font-size: 9px; line-height: 1.5; }

  .rows { margin-top: 7px; display: flex; flex-direction: column; gap: 4px; }
  .row {
    display: flex; align-items: baseline; gap: 10px; padding: 5px 9px;
    background: var(--surface-inset); border-radius: var(--r-inset); border: 1px solid var(--border-soft);
  }
  .rl { flex-shrink: 0; width: 120px; font-size: 10px; color: var(--fg-muted); }
  .rv { flex-shrink: 0; width: 84px; text-align: right; font-size: 13px; font-weight: 700; }
  .rn { min-width: 0; font-size: 9px; line-height: 1.5; }
  .tmp { color: var(--warm); font-weight: 700; }
  .note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }
</style>
