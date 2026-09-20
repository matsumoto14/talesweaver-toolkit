<script lang="ts">
  // 「avatar」補正源のペイン。アバターで効いている量は 2 つの出どころからできている:
  //
  //   1. 補正付きアバター(アイテム名の末尾が「Ａ」)= アバター本体。1 点で装備補正 9 値すべてに +1。
  //      5 部位そろうと 5 セット効果で 9 値すべてにさらに +10(合わせて +15)
  //   2. アバター強化剤 = 部位ごとに 9 値のどれにでも固定値を付与。現行は +10 / +12、旧品に +1 / +3
  //
  // どちらも同じ 5 部位(兜・頭・体・脚・エフェクト)で、強化能力値へ合流する。
  //
  // 画面は 2 枚。**上が合計、下が入力**で、9 補正の列を 2 枚で同じ位置に立てている
  // (上 = ラベル 153px + 9 等分、下 = 58px + gap 3px + 92px = 153px + 9 等分)。
  // 「兜の突き」を押すと、そのまま真上の「突き」の合計が動く(§00 01 視線を動かさない)。
  //
  // 開閉は持たない。強化剤は実際には数個しか付けないので、畳んで隠すより
  // 「どこに何が付いているか」が一目で分かるほうが効く(2026-09-21 のワイヤー検討で決定)。
  import type { AvatarPart, EquipmentStatKind, SkillDependency } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { avatarCorrectionCount, avatarEnhanceTotals } from "../../../equipment";
  import { fmtSigned } from "../../../format";
  import {
    AVATAR_PARTS, AVATAR_PART_LABELS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT,
    OTHER_EQUIPMENT_STATS, PRIMARY_EQUIPMENT_STATS,
  } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { STATE } from "../../../ui/states";
  import Chip from "../../../ui/Chip.svelte";
  import Disclosure from "../../../ui/Disclosure.svelte";
  import Value from "../../../ui/Value.svelte";
  import { equipmentAttackKindsFor } from "../summaries";

  interface Props {
    draft: Draft;
    /** 主軸スキルの依存種別。どの補正がこのキャラの火力に効くかを決める */
    dependency: SkillDependency | null;
  }
  let { draft, dependency }: Props = $props();

  // 列の並び。主要 4 つ(突き・斬り・魔攻・魔防)を先に、物防から敏捷までを後ろに置く
  // (物防は命中の左。装備ペイン・研磨ペインと同じ並び)
  const COLUMNS: EquipmentStatKind[] = [...PRIMARY_EQUIPMENT_STATS, ...OTHER_EQUIPMENT_STATS];
  /** 主軸スキルが実際に使う補正。ここだけ少し強く出す(塗らない。文字の濃さと太さだけ) */
  const keyStats = $derived(equipmentAttackKindsFor(dependency));
  const isKey = (kind: EquipmentStatKind) => keyStats.includes(kind);

  // 現行のアバター強化剤の値(クライアント DB dm_00000_0408/0425/0481)。旧品(+1/+3等)が
  // 既に入っている枠は、その値も巡回の輪に入れる(壊さない)。
  const CHOICES = [10, 12];

  // --- 合計(上のカード)------------------------------------------------------
  const enhanceTotals = $derived(avatarEnhanceTotals(draft.equipment.avatar));
  const count = $derived(avatarCorrectionCount(draft.equipment.avatar_corrections));
  const isSet = $derived(count === AVATAR_PARTS.length);
  const itemAmount = $derived(count * limits.avatar_correction_per_part);
  const setAmount = $derived(isSet ? limits.avatar_set_bonus : 0);
  const total = (kind: EquipmentStatKind) => itemAmount + setAmount + enhanceTotals[kind];
  /** 0 は「—」。空白や 0 のまま置くと、付いていないのか 0 なのか読めない(§00 05) */
  const amount = (v: number) => (v === 0 ? "—" : fmtSigned(v));

  // --- 入力(下のカード)------------------------------------------------------
  /** その枠が巡回する値の輪(なし → +10 → +12 → なし。旧品の値が入っていればそれも輪に入る) */
  function cycleValues(part: AvatarPart, kind: EquipmentStatKind): number[] {
    const current = draft.equipment.avatar[part][kind];
    const values = [0, ...CHOICES];
    if (!values.includes(current)) values.push(current);
    return values.sort((a, b) => a - b);
  }
  function cycleCell(part: AvatarPart, kind: EquipmentStatKind) {
    const values = cycleValues(part, kind);
    const next = (values.indexOf(draft.equipment.avatar[part][kind]) + 1) % values.length;
    draft.equipment.avatar[part][kind] = values[next];
  }
</script>

<!-- 上: 効いている量。実機の装備画面と 9 列で突き合わせられる -->
<div class="card">
  <div class="card-head">
    <span class="card-title">アバターで効いている量</span>
    <span class="small dim">強化能力値へ合流</span>
  </div>

  <div class="inset sum">
    <div class="sum-grid stat-head">
      <span></span>
      {#each COLUMNS as kind (kind)}
        <span class:key-stat={isKey(kind)}>{EQUIPMENT_STAT_SHORT[kind]}</span>
      {/each}
    </div>

    <div class="sum-grid">
      <span class="sum-label">補正付きアバター {count}点</span>
      {#each COLUMNS as kind (kind)}
        <Value
          class={"sum-v " + (itemAmount === 0 ? "dim" : "")}
          motion={() => itemAmount}
          value={amount(itemAmount)}
        />
      {/each}
    </div>

    <div class="sum-grid" class:set-on={isSet}>
      <span class="sum-label">5セット効果</span>
      {#each COLUMNS as kind (kind)}
        <Value
          class={"sum-v " + (setAmount === 0 ? "dim" : "")}
          motion={() => setAmount}
          value={amount(setAmount)}
        />
      {/each}
    </div>

    <div class="sum-grid last-part">
      <span class="sum-label">強化剤 5部位計</span>
      {#each COLUMNS as kind (kind)}
        <Value
          class={"sum-v " + (enhanceTotals[kind] === 0 ? "dim" : "")}
          motion={() => enhanceTotals[kind]}
          value={amount(enhanceTotals[kind])}
        />
      {/each}
    </div>

    <div class="sum-grid total-row">
      <span class="sum-label">合計</span>
      {#each COLUMNS as kind (kind)}
        <Value
          class={"sum-total " + (isKey(kind) ? "key-total" : "")}
          motion={() => total(kind)}
          value={amount(total(kind))}
        />
      {/each}
    </div>
  </div>
</div>

<!-- 下: 入力。開閉は無い。列は上のカードとそのまま揃う -->
<div class="card input-card">
  <div class="avatar-grid stat-head">
    <span class="part-head">部位</span>
    <span class="a-head">補正付きアバター</span>
    {#each COLUMNS as kind (kind)}
      <span class:key-stat={isKey(kind)}>{EQUIPMENT_STAT_SHORT[kind]}</span>
    {/each}
  </div>

  {#each AVATAR_PARTS as part (part)}
    <div class="avatar-grid">
      <span class="part-name">{AVATAR_PART_LABELS[part]}</span>
      <Chip
        class="cell a-cell"
        on={draft.equipment.avatar_corrections[part]}
        onToggle={() =>
          (draft.equipment.avatar_corrections[part] = !draft.equipment.avatar_corrections[part])}
      >
        全値 {fmtSigned(limits.avatar_correction_per_part)}
      </Chip>
      {#each COLUMNS as kind (kind)}
        {@const value = draft.equipment.avatar[part][kind]}
        <Chip
          class={"cell num " + (value > 0 ? "on" : "")}
          title="{AVATAR_PART_LABELS[part]}の{EQUIPMENT_STAT_SHORT[kind]}(押すと なし → {fmtSigned(
            CHOICES[0],
          )} → {fmtSigned(CHOICES[1])})"
          onclick={() => cycleCell(part, kind)}
        >
          <Value
            class={value === 0 ? "dim" : ""}
            motion={() => value}
            value={value === 0 ? "—" : String(value)}
          />
        </Chip>
      {/each}
    </div>
  {/each}

  <!-- セット効果は 5 つのチップの列のすぐ下。そろった瞬間にここが動く(§00 04) -->
  <div
    class="set-row"
    style="background: {isSet ? STATE.met.bg : 'transparent'}; border-color: {isSet
      ? STATE.met.bd
      : 'var(--border-soft)'}; color: {isSet ? STATE.met.fg : 'var(--fg-muted)'};"
  >
    <b>5セット効果</b>
    <Value
      value={isSet
        ? `5部位そろいました — 全9値 ${fmtSigned(limits.avatar_set_bonus)} が上の合計に入っています`
        : `あと${AVATAR_PARTS.length - count}部位そろえると 全9値 ${fmtSigned(limits.avatar_set_bonus)}`}
    />
  </div>

  <!-- 説明は毎回読むものではない。入力の下に畳む(§00 02) -->
  <Disclosure class="fold">
    {#snippet summary()}この画面の読み方{/snippet}
    <div class="fold-body">
      <p class="hint dim">
        <b>補正付きアバター</b>はアイテム名の末尾に「Ａ」が付くアバターです。1 点で装備補正
        {EQUIPMENT_STAT_KINDS.length}値すべてに{fmtSigned(limits.avatar_correction_per_part)}、5 部位
        そろえるとセット効果でさらに{fmtSigned(limits.avatar_set_bonus)}(合計{fmtSigned(
          AVATAR_PARTS.length * limits.avatar_correction_per_part + limits.avatar_set_bonus,
        )})。セット効果の移動速度 +10 はこのツールでは扱いません。
      </p>
      <p class="hint dim">
        <b>アバター強化剤</b>は部位ごとに{EQUIPMENT_STAT_KINDS.length}値のどれにでも固定値を付与でき、
        <b>同じ部位に複数の値を重ねられます</b>(例: 兜に突き+{limits.avatar_enhance_max}と命中+{limits.avatar_enhance_max}の両方)。
        枠は押すたび なし → {fmtSigned(CHOICES[0])} → {fmtSigned(CHOICES[1])} と回ります。
        現行の強化剤は期限つきですが、期限はこのツールでは扱いません。旧品の +1 / +3 などが
        既に入っている枠は、その値も回ります。移動速度の強化剤は対象外です。
      </p>
      <p class="hint dim">どちらもエンチャント・テシスコアと同じ「強化能力値」に合流します。</p>
    </div>
  </Disclosure>
</div>

<style>
  /* 9 補正の列を上下 2 枚のカードで同じ位置に立てる。ラベル側の合計を合わせるのが要点 —
     下は 58px + gap 3px + 92px = 153px で、上のラベル列と 1px もずらさない(§00 01) */
  .sum-grid,
  .avatar-grid {
    display: grid; gap: 3px; align-items: center;
  }
  .sum-grid { grid-template-columns: 153px repeat(9, minmax(0, 1fr)); }
  .avatar-grid { grid-template-columns: 58px 92px repeat(9, minmax(0, 1fr)); }

  .sum { padding: 7px 9px; display: flex; flex-direction: column; gap: 3px; }
  .stat-head { font-size: 9px; color: var(--fg-dim); text-align: center; }
  /* 主軸スキルが使う補正は「ちょっとだけ」強く。面を塗らず、文字の濃さと太さだけで言う(§02) */
  .stat-head .key-stat { color: var(--accent-hover); font-weight: 700; }
  .sum-label { text-align: left; font-size: 10.5px; color: var(--fg-sub); }
  .sum-grid :global(.sum-v) { font-size: 10.5px; color: var(--fg-sub); text-align: center; }
  .sum-grid.set-on :global(.sum-v) { color: var(--good); }
  .last-part { padding-bottom: 4px; border-bottom: 1px solid var(--border-soft); }
  .total-row { padding-top: 3px; }
  .total-row .sum-label { font-size: 11px; font-weight: 700; color: var(--fg); }
  .total-row :global(.sum-total) {
    font-size: 17px; font-weight: 700; color: var(--fg-sub); text-align: center;
  }
  .total-row :global(.sum-total.key-total) { color: var(--fg); }

  .input-card { margin-top: 9px; display: flex; flex-direction: column; gap: 3px; }
  .part-head { text-align: left; font-size: 11px; font-weight: 700; color: var(--fg-head); }
  .a-head { font-size: 9.5px; font-weight: 700; color: var(--fg-muted); }
  .part-name { font-size: 11px; font-weight: 700; }

  /* 枠は 1 行に 11 個並ぶので、チップの既定(pill・左右 11px)ではなく
     幅いっぱい・角丸小さめにする。桁が増えても幅は列が持つので動かない */
  .avatar-grid :global(.chip.cell) {
    width: 100%; justify-content: center; padding: 6px 0;
    border-radius: var(--r-inset); font-size: 11px;
  }
  .avatar-grid :global(.chip.a-cell) { border-radius: var(--r-pill); font-size: 10px; }

  .set-row {
    display: flex; align-items: center; gap: 8px;
    margin-top: 3px; padding: 6px 9px;
    border: 1px solid; border-radius: var(--r-inset);
    font-size: 10.5px;
  }
  .set-row b { font-size: 10px; }
</style>
