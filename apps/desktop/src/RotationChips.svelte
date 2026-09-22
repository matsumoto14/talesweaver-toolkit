<script lang="ts">
  // 「差し込む CT 技」を選ぶチップ一式(キャラタブの入力欄・計算タブの回しの段が共有)。
  //
  // **振り分け(上がる / 下がる / 不明)は Rust が返す `effect`** をそのまま使う ——
  // 画面が `gain >= 0` のような境界を持つと、既定 ON の判定(`gain > 0`)と食い違う。
  // 下がる技は畳んだ先(`Disclosure`)に置き、見たい人だけが開いて手で ON にできる
  // (ADR-019 決定 5。既定で ON にならないものを並べても選ぶ手がかりにならない)。
  //
  // 2 つの画面で違うのは「保存するか」だけ。キャラタブは押した瞬間に保存、計算タブは
  // その場で試すだけなので、チップをラベンダー(保存されない。§03)で出す。
  import type { Snippet } from "svelte";
  import type { RotationInsertChoice } from "./api/types";
  import { fmtCooldown, fmtShareOf, fmtSigned } from "./format";
  import Choose from "./ui/Choose.svelte";
  import Disclosure from "./ui/Disclosure.svelte";
  import Icon from "./ui/Icon.svelte";
  import Value from "./ui/Value.svelte";

  interface Props {
    /** 差し込める候補(Rust `list_rotation_choices`) */
    candidates: RotationInsertChoice[];
    /** いま ON の技 id(候補にあるものだけ) */
    onIds: string[];
    onToggle: (id: string, on: boolean) => void;
    /** 損得を割合で見せるための分母(回し全体の期待 DPS)。出せないなら null */
    expectedDps: number | null;
    /** 保存されない一時の選択か(計算タブ)。true ならチップをラベンダーで出す */
    temporary?: boolean;
    /** 畳んだ先(下がる技)の末尾に置く説明。画面ごとの言い回しなので呼ぶ側が持つ */
    dropHint?: Snippet;
  }
  let { candidates, onIds, onToggle, expectedDps, temporary = false, dropHint }: Props = $props();

  const ups = $derived(candidates.filter((c) => c.effect !== "reduces"));
  const drops = $derived(candidates.filter((c) => c.effect === "reduces"));
  const dropsOn = $derived(drops.filter((c) => onIds.includes(c.skill_id)));
  const optionsOf = (list: RotationInsertChoice[]) =>
    list.map((c) => ({ value: c.skill_id, label: c.skill_name }));
  const find = (id: string) => candidates.find((c) => c.skill_id === id);
  /** 回し全体に対する割合。絶対値だけだと桁が大きく、実際より深刻に見える(実機 2026-09-21) */
  const share = (gain: number) => fmtShareOf(gain, expectedDps);
  /** 段に添える割合。損得を出せない候補は `null` = 未収録の「?」になる(§08) */
  const gainLabel = (id: string) => {
    const gain = find(id)?.expected_dps_gain ?? null;
    return gain === null ? null : share(Math.round(gain));
  };
  /** 陣は CT ではなく持続(切れたら置き直す)。数字の意味が違うので言い方を変える */
  const cdLabel = (c: { cooldown_seconds: number; is_field: boolean }) =>
    `${c.is_field ? "持続" : "CT"} ${fmtCooldown(c.cooldown_seconds)}`;
  const chipTitle = (id: string) => {
    const c = find(id);
    if (!c) return undefined;
    const cd = cdLabel(c);
    if (c.expected_dps_gain === null) {
      return `${cd} ・ 差し込んだときの損得は出せません(1 回の所要時間が未収録)`;
    }
    const gain = Math.round(c.expected_dps_gain);
    const pct = share(gain);
    const amount = `${pct === null ? "" : `${pct}(`}${fmtSigned(gain)}${pct === null ? "" : ")"}`;
    const effect = c.effect === "improves" ? "差し込むと DPS が上がる" : "差し込むと DPS が下がる";
    return `${cd} ・ ${effect} ${amount}`;
  };
  // 見た目は主軸スキルのチップ(`ui/Picker.svelte`)と同じ段: アイコン + 技名 + 補足のピル。
  // すぐ上の主軸と同じ作法で並ぶので、別物に見えない(§00 ⑤)
  const tone = $derived(() => `picker-chip with-icon${temporary ? " sim" : ""}`);
</script>

{#if ups.length > 0}
  <Choose
    label={temporary ? "ここで試す差し込み CT 技" : "差し込む CT 技"}
    class="chiprow picker-chips"
    options={optionsOf(ups)}
    values={onIds}
    {onToggle}
    {tone}
    titleFor={chipTitle}
  >
    {#snippet item(o)}
      <Icon kind="skill" id={o.value} size={20} label={o.label} />
      <span class="picker-chip-name">{o.label}</span>
      <span class="picker-chip-meta num">{find(o.value) ? cdLabel(find(o.value)!) : ""}</span>
      <!-- 損得を出せない候補は割合の代わりに「?」(未収録は 0 や空白にしない。§08) -->
      {#if find(o.value)?.effect === "unknown"}<Value class="chip-gain" value={null} />{/if}
    {/snippet}
  </Choose>
{/if}
{#if drops.length > 0}
  <!-- 下がる技は畳んだ先。ON にしているものは summary で分かる(§00 ②④) -->
  <Disclosure class="drop-pick" summaryClass="chip quiet">
    {#snippet summary()}
      DPS が下がる技 <Value class="dim normal" motion={() => drops.length} value={`${drops.length} 件`} />
      {#if dropsOn.length > 0}
        <Value class="badge drop-badge" motion={() => dropsOn.length} value={`${dropsOn.length} 件 ON`} />
      {/if}
    {/snippet}
    <Choose
      label="DPS が下がる差し込み CT 技"
      class="chiprow picker-chips"
      options={optionsOf(drops)}
      values={onIds}
      {onToggle}
      {tone}
      titleFor={chipTitle}
    >
      {#snippet item(o)}
        <Icon kind="skill" id={o.value} size={20} label={o.label} />
        <span class="picker-chip-name">{o.label}</span>
        <span class="picker-chip-meta num">{find(o.value) ? cdLabel(find(o.value)!) : ""}</span>
        {#if gainLabel(o.value)}
          <Value class="chip-gain" tone="down" value={gainLabel(o.value)} />
        {/if}
      {/snippet}
    </Choose>
    {@render dropHint?.()}
  </Disclosure>
{/if}
