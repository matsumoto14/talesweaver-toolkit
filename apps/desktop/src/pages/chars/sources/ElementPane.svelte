<script lang="ts">
  // 「element」補正源のペイン。主属性(装備の外の供給源=ペット / モンスターカード /
  // ルーンスキル)と、装備から自動で入る属性値(部位ごとの属性強化・月石などのアビリティ)。
  //
  // 属性値は**与ダメージに効くのが「自分 − 敵」の差だけ**なので、合計をただ出しても意味が
  // 分からない。敵の閾値(120 / 125 が多い)に対してどれだけ超えているかまで出す。
  // 合算・上限 255 の頭打ちは Rust 側(preview.elements)。ここは表示に整えるだけ(ADR 001)。
  import { untrack } from "svelte";
  import type { Element, PartSlot, Skill, StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { fmtInt, fmtSigned, fmtSignedPct } from "../../../format";
  import { ELEMENT_LABELS, ELEMENTS, PART_SLOT_LABELS } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { app } from "../../../state.svelte";
  import Chip from "../../../ui/Chip.svelte";
  import Choose from "../../../ui/Choose.svelte";
  import Value from "../../../ui/Value.svelte";
  import type { SourceId } from "../sourceId";
  import ExternalSourceList, { type ExternalSource } from "./ExternalSourceList.svelte";
  import { t } from "../../../i18n";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
    onOpenSource: (id: SourceId) => void;
  }
  let { draft, preview, skills, onOpenSource }: Props = $props();

  const elements = $derived(preview?.elements ?? null);
  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);

  // 属性は主軸スキルで決まる。無属性のスキルのときだけ、乗せる属性を選ばせる
  // (アンプルで属性を足す運用が多い)
  const skillElement = $derived(mainSkill?.element ?? null);
  const elementFromSkill = $derived(skillElement !== null && skillElement !== "neutral");
  let elementPickOpen = $state(false);

  // --- 主属性 -------------------------------------------------------------
  // 装備の外の供給源(ペット / モンスターカード / ルーンスキル)は、実際には**全部同じ属性に
  // 振る**。だから供給源ごとに聞かず、主属性を 1 回選ばせてまとめて乗せる
  // (§00「要らないものを見せない」)
  const elementSourceDefs = $derived(app.elementSources);
  const elementOptions = [
    { value: "", label: t("なし") },
    ...ELEMENTS.map((e) => ({ value: e, label: ELEMENT_LABELS[e] })),
  ];
  /** 供給源が全部同じ属性ならそれが主属性。ばらけていたら "" を返す */
  const mainElement = $derived.by(() => {
    const picked = elementSourceDefs.map((def) => draft.statSources.elements[def.id] ?? null);
    const first = picked[0] ?? null;
    return first !== null && picked.every((e) => e === first) ? first : "";
  });
  function setMainElement(value: string) {
    for (const def of elementSourceDefs) {
      draft.statSources.elements[def.id] = value === "" ? null : (value as Element);
    }
  }
  /** 属性を選んだら、属性ありスキルと同じ確定表示へ戻す。 */
  function chooseMainElement(value: string) {
    setMainElement(value);
    elementOverrideForSkill = elementFromSkill && value !== skillElement ? draft.mainSkillId : null;
    if (value !== "") elementPickOpen = false;
  }
  const elementSourceTotal = $derived(elementSourceDefs.reduce((n, def) => n + def.value, 0));
  // 保存済みデータがスキル属性と違う場合は、利用者が選んだ上書きとして維持する。
  let elementOverrideForSkill = $state(untrack(() =>
    elementFromSkill && mainElement !== "" && mainElement !== skillElement ? draft.mainSkillId : null
  ));
  let lastElementSkillId = untrack(() => draft.mainSkillId);
  /**
   * 属性は主軸スキルで決まるので、スキルを選んだら供給源もその属性に合わせる(自動値)。
   * 自分で「別の属性を乗せる」を開いたときは触らない — 例外操作を上書きしない
   */
  $effect(() => {
    const currentSkillId = draft.mainSkillId;
    if (currentSkillId !== lastElementSkillId) {
      lastElementSkillId = currentSkillId;
      elementOverrideForSkill = null;
      elementPickOpen = false;
    }
    if (!elementFromSkill || elementOverrideForSkill === draft.mainSkillId) return;
    if (mainElement === skillElement) return;
    setMainElement(skillElement as string);
  });

  /** いま実際に効いている属性(主軸スキルの属性、無属性なら乗せている属性) */
  const activeElement = $derived<Element | null>(
    elementFromSkill && elementOverrideForSkill !== draft.mainSkillId
      ? (skillElement as Element)
      : mainElement === ""
        ? null
        : (mainElement as Element),
  );
  /**
   * 内訳を出す属性。効いている属性を先頭に、**その属性だけに入っているものがある属性**を続ける。
   *
   * 全属性に乗るもの(ルミナの回廊・全属性バフ)だけの属性は出さない — それを出すと 8 属性
   * 全部が並んで、実際に積んである属性が埋もれる(§00 02 要らないものを見せない)。
   */
  const ownTotal = (e: Element) =>
    (elements?.base[e] ?? 0) +
    (elements?.equipment[e] ?? 0) +
    (elements?.ability[e] ?? 0) +
    (elements?.sources[e] ?? 0);
  const shownElements = $derived<Element[]>([
    ...(activeElement !== null && (elements?.total[activeElement] ?? 0) > 0 ? [activeElement] : []),
    ...ELEMENTS.filter((e) => e !== activeElement && ownTotal(e) > 0),
  ]);
  const activeTotal = $derived(activeElement === null ? 0 : elements?.total[activeElement] ?? 0);
  /** 敵の属性値との差 +1 ごとに増えるダメージ。上限に届いたかどうかも出す */
  const bonusPercent = (total: number, threshold: number) =>
    Math.min(
      limits.element_bonus_max * 100,
      Math.max(0, total - threshold) * limits.element_bonus_percent_per_point,
    );
  /** 収録済みの敵に多い閾値。ここは「だいたいどのくらい効くか」の目安 */
  const ENEMY_THRESHOLDS = [120, 125];

  /**
   * 装備アビリティで属性を持てる部位ごとの注記。**登録が無い部位も 0 の行で出す**ので、
   * 「属性値が足りないのは登録の抜け」だと画面で分かる(Rust 側が 0 の行も返す)
   */
  const ABILITY_PART_NOTE: Partial<Record<PartSlot, string>> = {
    helm: t("月石の本体(N +5 / R +10 / L +15 / G +20)と、G- の別属性枠 +20"),
    shield_plus: t("カフスのアビリティのランダム追加枠(属性 +10〜30)"),
    relic_pendant: t("レリック(ペンダント)のランダム追加枠(属性 +20〜30)"),
  };

  /** 装備から自動で入る分(部位ごとの属性強化と、部位ごとの装備アビリティ) */
  const fromEquipment = $derived<ExternalSource[]>(
    activeElement === null
      ? []
      : [
          {
            id: "equipment" as SourceId,
            name: t("装備の属性強化"),
            value: elements?.equipment[activeElement] ?? 0,
            format: (v: number) => fmtSigned(v),
            note: t("1 部位 1 属性・最大 9(盾+・レリックは対象外)"),
          },
          ...(elements?.ability_by_part ?? []).map((part) => ({
            id: "equipment" as SourceId,
            name: t("{slot}のアビリティ", { slot: PART_SLOT_LABELS[part.slot] }),
            value: part.values[activeElement],
            format: (v: number) => fmtSigned(v),
            note: ABILITY_PART_NOTE[part.slot],
          })),
        ],
  );

  /**
   * 装備以外から入ってくる分。**押しても移る先が無い行も出す**(`id` なし)— 主属性に紐づく
   * ペット・カード・ルーンの属性値はこのペインでしか触らないし、バフはバフタブで選ぶ。
   * 0 の行も残して「どこから来るか」の地図にする(`ExternalSourceList` の方針)
   */
  const fromOthers = $derived<ExternalSource[]>(
    activeElement === null
      ? []
      : [
          {
            id: "lumina" as SourceId,
            name: t("ルミナの回廊"),
            value: elements?.corridor[activeElement] ?? 0,
            format: (v: number) => fmtSigned(v),
            note: t("回廊効果「全属性増加」(Lv1 ごとに全属性 +1・最大 +10)"),
          },
          ...elementSourceDefs.map((def) => ({
            name: t(def.name),
            value:
              draft.statSources.elements[def.id] === activeElement ? def.value : 0,
            format: (v: number) => fmtSigned(v),
            note: t("主属性に選んだ属性へ乗ります(上の「主属性」で切り替え)"),
          })),
          {
            name: t("バフ(全属性 +15)"),
            value: elements?.buff[activeElement] ?? 0,
            format: (v: number) => fmtSigned(v),
            note: t("イルミネーション祭りのドリンク・ユキダルマン族の特製ポーション・迅速の秘薬。バフタブで選びます"),
          },
        ],
  );
</script>

<div class="card">
  <div class="card-title">{t("主属性")}</div>
  <!-- 属性はふつう主軸スキルで決まる。無属性のときだけ「何を乗せるか」を選ばせる -->
  {#if (elementFromSkill || mainElement !== "") && !elementPickOpen}
    {@const displayedElement = (elementFromSkill && elementOverrideForSkill !== draft.mainSkillId ? skillElement : mainElement) as Element}
    <p class="element-auto">
      <b class="element-picked elem-{displayedElement}"
        ><Value value={displayedElement}>{#snippet children()}{ELEMENT_LABELS[displayedElement]}{/snippet}</Value></b
      >
      <span class="dim">
        {elementFromSkill && elementOverrideForSkill !== draft.mainSkillId
          ? t("— 主軸スキル「{name}」で決まります", { name: t(mainSkill?.name ?? "") })
          : t("— アンプルなどで乗せる属性")}
      </span>
      <Chip class="quiet" onclick={() => (elementPickOpen = true)}>{t("変更")}</Chip>
    </p>
  {:else}
    {#if skillElement === "neutral"}
      <p class="hint dim">{t("主軸スキルが無属性なので、アンプルなどで乗せる属性を選びます。")}</p>
    {/if}
    <Choose
      label={t("乗せる属性")}
      options={elementOptions}
      cols={elementOptions.length}
      tone={(v) => (v === "" ? undefined : `elem-${v}`)}
      bind:value={() => mainElement, chooseMainElement}
    />
  {/if}
  <p class="hint dim">
    {t("ペット・モンスターカード・ルーンスキルの {v} をまとめて乗せます。 月石・カフス・レリックの属性は装備に登録したものから自動で入ります。", { v: fmtSigned(elementSourceTotal) })}
  </p>
</div>

{#if elements}
  <div class="card">
    <div class="card-title">{t("属性値")}</div>
    {#each shownElements as e (e)}
      <div class="element-row" class:active={e === activeElement}>
        <b class="element-picked elem-{e}">{ELEMENT_LABELS[e]}</b>
        <Value class="element-total" motion={() => elements.total[e]} value={fmtInt(elements.total[e])} />
        <span class="dim">
          {t("キャラ {a} + 装備 {b} + 装備アビリティ {c} + 主属性 {d} + 回廊 {e} + バフ {f}", {
            a: fmtInt(elements.base[e]), b: fmtInt(elements.equipment[e]),
            c: fmtInt(elements.ability[e]), d: fmtInt(elements.sources[e]),
            e: fmtInt(elements.corridor[e]), f: fmtInt(elements.buff[e]),
          })}
        </span>
      </div>
    {:else}
      <p class="hint dim">{t("まだどの属性も乗っていません。")}</p>
    {/each}
    {#if activeTotal > 0}
      <!-- 合計そのものより「敵に対して何 % か」が知りたい値。よくある閾値の 2 つで出す -->
      <div class="element-versus inset num">
        {#each ENEMY_THRESHOLDS as threshold (threshold)}
          <span class="versus-term">
            <span class="dim">{t("敵 {v}", { v: threshold })}</span>
            <b><Value motion={() => bonusPercent(activeTotal, threshold)} value={fmtSignedPct(bonusPercent(activeTotal, threshold) / 100)} /></b>
          </span>
        {/each}
      </div>
    {/if}
    <p class="hint dim">
      {t("与ダメージに効くのは")}<b>{t("攻撃側 − 敵")}</b>{t("の差で、差 +1 ごとに {a}、 {b} で上限 {c}(敵は 120 / 125 が多い)。属性値そのものの上限は {d} です。", {
        a: fmtSigned(limits.element_bonus_percent_per_point, { max: 2 }, "%"),
        b: fmtSigned(limits.element_bonus_max / (limits.element_bonus_percent_per_point / 100)),
        c: fmtSignedPct(limits.element_bonus_max),
        d: fmtInt(limits.element_value_max),
      })}
    </p>
  </div>
{/if}

<ExternalSourceList rows={fromEquipment} title={t("装備から自動で入る分")} {onOpenSource} />
<ExternalSourceList rows={fromOthers} title={t("装備以外から入る分")} {onOpenSource} />

<style>
  .element-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    font-size: 12px;
  }
  /* 効いていない属性は積んであるだけ。薄くして、いま効いている属性と取り違えさせない */
  .element-row:not(.active) { opacity: 0.6; }
  .element-versus {
    display: flex;
    gap: 16px;
    padding: 6px 10px;
    border-radius: var(--r-sm);
    margin-top: 6px;
  }
  .versus-term {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
</style>
