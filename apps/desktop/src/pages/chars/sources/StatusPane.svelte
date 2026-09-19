<script lang="ts">
  // 「status」補正源のペイン。キャラ選択・覚醒・エタの意志・主軸スキル・能力値の一覧。
  // 属性は独立した補正源(`sources/ElementPane.svelte`)へ移した(2026-09-19)。
  import { untrack } from "svelte";
  import type { Skill, StatKind, StatPreview, StatSourceGroup } from "../../../api/types";
  import { errorMessage, resetCharacterIcon, setCharacterIcon } from "../../../api/commands";
  import { mainSkillOptions as buildMainSkillOptions } from "../../../characterSkills";
  import { ETERNAL_MILESTONES, type Draft } from "../../../draft";
  import { fmtInt, fmtSigned, fmtSignedPct, formatLayerValue } from "../../../format";
  import {
    STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS, STAT_SOURCE_GROUPS, STAT_SOURCE_GROUP_LABELS,
  } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { app } from "../../../state.svelte";
  import { reportError } from "../../../toast.svelte";
  import Chip from "../../../ui/Chip.svelte";
  import Disclosure from "../../../ui/Disclosure.svelte";
  import FilePick from "../../../ui/FilePick.svelte";
  import Icon from "../../../ui/Icon.svelte";
  import { latest } from "../../../ui/latest.svelte";
  import { changed } from "../../../ui/motion.svelte";
  import Value from "../../../ui/Value.svelte";
  import Picker from "../../../ui/Picker.svelte";
  import NumberField from "../../../ui/NumberField.svelte";
  import Choose from "../../../ui/Choose.svelte";
  import TextField from "../../../ui/TextField.svelte";

  interface Props {
    characterId: number;
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
  }
  let { characterId, draft, preview, skills }: Props = $props();

  const STAT_MIN = 1;

  // キャラは名前で探すより顔で選ぶほうが速い(ゲーム内も顔で選ぶ)。§06 の 40px。
  // 名前は必ず併記する(アイコン単独表示は禁止)
  const gameCharacterName = $derived(
    app.gameCharacters.find((c) => c.id === draft.gameCharacterId)?.name ?? "未選択",
  );
  /** キャラは登録時に決めるもの。ふだんは畳んでおく */
  let charPickOpen = $state(false);
  let iconSaving = $state(false);
  /** 画像を選ぶ素の入力。見た目は Chip なので、ここは押されたとき開くだけ */
  /** アイコンが変わったことを弾ませて見せるための印。値は見ず参照の不一致だけ使うので、
   *  変更のたびに新しいオブジェクトを積む(`use:changed` 参照) */
  let iconChangeMark = $state<object | null>(null);
  function markIconChanged() {
    iconChangeMark = {};
  }
  async function chooseIcon(file: File) {
    iconSaving = true;
    try {
      const saved = await setCharacterIcon(characterId, new Uint8Array(await file.arrayBuffer()));
      app.characterIcons[characterId] = saved.dataUrl;
      markIconChanged();
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      iconSaving = false;
    }
  }
  async function resetIcon() {
    iconSaving = true;
    try {
      await resetCharacterIcon(characterId);
      delete app.characterIcons[characterId];
      markIconChanged();
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      iconSaving = false;
    }
  }
  function setGameCharacterId(id: string) {
    if (id === draft.gameCharacterId) return;
    draft.gameCharacterId = id;
    draft.mainSkillId = "";
    // 召喚スキルも捨てる。前キャラのスキル id が残ると validate_summon_skill が
    // 「そのキャラのスキルではありません」で弾き、自動保存が止まる(レビュー指摘 2026-09-18)
    draft.summonSkillId = "";
  }

  // エタの意志 Lv は 0〜100 の**数値**。101 個を並べても段階にならないので、
  // 節目(20 / 40 / 60 / 80 / 90)を選べる形 + 数値の微調整にする。
  // 節目はそこを超えると上限の増え方が一段上がる地点で、育成の目標地点そのもの。
  const eternalMilestoneOptions = $derived(
    ETERNAL_MILESTONES.filter((lv) => lv <= limits.eternal_level_max).map((lv) => ({
      value: String(lv),
      label: String(lv),
    })),
  );
  /** エタの意志は覚醒 5 の先にあるものなので、触った時点で覚醒は 5 で確定する */
  function setEternalLevel(level: string) {
    draft.eternalLevel = level;
    if (Number(level) > 0) draft.stage = String(limits.eternal_awakening_stage);
  }
  // 覚醒段階は 4 と 5 しか使わない(このツールの対象)。それ以外は開いたときだけ出す
  const stageOptions = Array.from({ length: limits.awakening_stage_max + 1 }, (_, i) => ({ value: String(i), label: String(i) }));
  const stageMainOptions = [4, 5].map((i) => ({ value: String(i), label: String(i) }));
  let stageAllOpen = $state(false);
  const stageIsLow = $derived(Number(draft.stage) < 4);

  // 主軸スキル。未収録のキャラがあるので未選択("")を許す。
  /** 中ディレイ込みの継続火力順。主軸に選ばれるのはほぼこの上位なので、候補として先に出す。 */
  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);
  // 並びは list_skills(Rust)が主軸候補順で返し、先頭 3 件がチップに固定される
  const mainSkillOptions = $derived(buildMainSkillOptions(skills, "未選択", "攻撃力を出さない"));

  // 召喚獣(熊・破壊精霊)が撃つスキル。アナイス以外はこのキャラのスキルに本体以外の
  // 攻撃者が 1 件も無いので欄自体を出さない(§00②「要らないものを見せない」。ADR-016)。
  const hasSummon = $derived(skills.some((s) => s.attacker !== "player"));
  /** 主軸(ベアステップ・陣)が決めた召喚獣の型。共通スキル等では決まらず null
   * (2026-09-18 追記。Skill::summon_form が唯一の正、対応表は TS に書き写さない) */
  const summonForm = $derived(mainSkill?.summon_form ?? null);
  const summonSkillOptions = $derived(
    buildMainSkillOptions(skills, "未選択", "召喚獣の鎖を出しません", "summon", summonForm),
  );
  /** 主軸を変えたら、召喚スキルが新しい型の候補に無ければその型で一番火力の出るスキルへ
   * 差し替える。候補に入っていれば(=同じ型を保ったままの変更)手で選んだものを尊重する。
   * 主軸が型を決めないときは触らない(入力は「自動」が最上位。欄は上書きの例外操作 —
   * ux-guidelines)。ユーザー判断 2026-09-18
   *
   * 候補の**並び**は主軸と同じ「単体優先 → 継続火力順」(Rust の main_skill_order)だが、
   * 自動で入れる 1 件は並びの先頭ではなく継続火力(power_per_second)が最大のものにする。
   * 召喚獣は自分で撃ち続けるので単体/範囲の区別より DPS が効く — 先頭を採ると
   * ミカベアで †極・突き(単体の基本攻撃)が入ってしまい、熊連より弱い既定になる(実機確認)。 */
  let lastMainSkillId = untrack(() => draft.mainSkillId);
  $effect(() => {
    const id = draft.mainSkillId;
    if (id === lastMainSkillId) return;
    lastMainSkillId = id;
    if (summonForm === null) return;
    const candidateIds = summonSkillOptions.filter((o) => o.value !== "").map((o) => o.value);
    if (candidateIds.includes(draft.summonSkillId)) return;
    const rate = (s: Skill) => s.power_per_second ?? s.power;
    const best = skills
      .filter((s) => candidateIds.includes(s.id))
      .reduce<Skill | null>((top, s) => (top === null || rate(s) > rate(top) ? s : top), null);
    draft.summonSkillId = best?.id ?? "";
  });

  const traceFor = (k: StatKind) => preview?.traces.find((t) => t.kind === k) ?? null;
  const signed = (n: number) => fmtSigned(n, { max: 3 });

  // ゲーム内の能力値と突き合わせるとき、合わない原因は「登録内容の抜け」であって計算ではない。
  // どこから来た上昇かが見えないと、抜けているのがバフなのか装備なのかを人が当てるしかない。
  // 区分ごとの帰属は Rust が出す(group_effects)。ここでは引くだけで、足し算はしない(ADR 001)
  const groupEffect = (k: StatKind, g: StatSourceGroup) =>
    preview?.group_effects.find((e) => e.kind === k && e.group === g)?.effect ?? null;
  /** 「補正」列に重ねる出どころの内訳。開かなくても hover で答えが出る(§00 05) */
  const groupTitle = (k: StatKind) =>
    preview === null
      ? ""
      : STAT_SOURCE_GROUPS.map(
          (g) => `${STAT_SOURCE_GROUP_LABELS[g]} ${signed(groupEffect(k, g) ?? 0)}`,
        ).join(" / ");
</script>

<div class="card">
  <div class="fields">
    <label class="text">
      <span class="label">名前</span>
      <TextField label="名前" bind:value={draft.name} max={32} auto={gameCharacterName} autoNote="キャラ名を使用" />
    </label>
    <!-- キャラは登録のときに決めて、ふだんは変えない。いまのキャラだけ出して、
         変えるときに顔を並べる(§00 02)。名前はアイコンに必ず併記する -->
    <div class="wide">
      <span class="label">キャラ</span>
      <div class="char-now">
        <span class="current-icon" use:changed={() => iconChangeMark}>
          <Icon kind="character" id={draft.gameCharacterId} size={40} label={gameCharacterName} source={app.characterIcons[characterId] ?? null} />
        </span>
        <span class="char-name">{gameCharacterName}</span>
        <FilePick class="quiet" accept="image/png,image/jpeg,image/webp" disabled={iconSaving} onPick={chooseIcon}>
          {iconSaving ? "画像を処理中…" : app.characterIcons[characterId] ? "画像を変更" : "画像を選ぶ"}
        </FilePick>
        {#if app.characterIcons[characterId]}
          <Chip class="quiet" disabled={iconSaving} onclick={resetIcon}>標準に戻す</Chip>
        {/if}
        <Disclosure class="char-pick" summaryClass="chip quiet" bind:open={charPickOpen}>
          {#snippet summary(open)}{open ? "閉じる" : "変更"}{/snippet}
          <!-- 顔は多いので行を折り返して全幅に落とす(details は display:contents で行に溶ける) -->
          <div class="pick-grid">
            {#each app.gameCharacters as c (c.id)}
              <button
                type="button"
                class="pick"
                class:on={c.id === draft.gameCharacterId}
                onclick={() => { setGameCharacterId(c.id); charPickOpen = false; }}
              >
                <Icon kind="character" id={c.id} size={40} label={c.name} />
                <span class="pick-name">{c.name}</span>
              </button>
            {/each}
          </div>
        </Disclosure>
      </div>
    </div>
    <!-- エタの意志は覚醒 5 の先にあるもの。**選んだ時点で覚醒は 5 で確定する**ので、
         覚醒より先に置く(§00 01 決める順に並べる) -->
    <div class="wide">
      <span class="label">エタの意志 Lv</span>
      <div class="eternal-row">
        <NumberField
          label="エタの意志 Lv"
          max={limits.eternal_level_max}
          bind:value={
            () => Number(draft.eternalLevel),
            (v) => setEternalLevel(String(v))
          }
        />
        <Choose
          label="エタの意志の節目"
          options={eternalMilestoneOptions}
          cols={eternalMilestoneOptions.length}
          bind:value={() => draft.eternalLevel, setEternalLevel}
        />
      </div>
      <p class="hint dim">節目(20 / 40 / 60 / 80 / 90)を超えると、ダメージ上限・防御力上限・能力値上限の伸びが一段上がります。Lv を入れると覚醒は 5 段階になります。</p>
    </div>
    <!-- 覚醒段階は 4 と 5 しか使わない。それ以外は開いたときだけ出す(§00 02) -->
    <div class="stage-field wide">
      <span class="label">覚醒段階</span>
      <div class="stage-row">
        <Choose
          label="覚醒段階"
          options={stageAllOpen || stageIsLow ? stageOptions : stageMainOptions}
          cols={stageAllOpen || stageIsLow ? stageOptions.length : stageMainOptions.length}
          bind:value={draft.stage}
        />
        {#if !stageIsLow}
          <Chip class="quiet" on={stageAllOpen} onToggle={() => (stageAllOpen = !stageAllOpen)}>
            {stageAllOpen ? "4 / 5 だけ" : "それ以外"}
          </Chip>
        {/if}
      </div>
    </div>
    <!-- 主軸に選ばれるのはほぼ火力上位。上位 3 つをチップで手前に固定し、残りは候補面
         (§07「1 つ選ぶ」)。スキルは名前だけでは選べないので 単 / 範・段数・属性・中ディレイを併記 -->
    <div class="wide">
      <span class="label">主軸スキル</span>
      <Picker
        label="主軸スキル"
        options={mainSkillOptions}
        note="単体を優先・継続火力の目安順(倍率 × 段数 ÷ 基本中ディレイ)"
        bind:value={draft.mainSkillId}
      />
    </div>
    {#if hasSummon}
      <!-- アナイス専用: 魔法人形(ミカベア/ルシベア)または破壊精霊(アンフェル/グレシス/
           イグニー)に自動で撃たせるスキル(ADR-016)。主軸と同じ Picker 形。
           未選択なら計算タブに召喚獣の鎖は出ない(0 で埋めない) -->
      <div class="wide">
        <span class="label">召喚獣が撃つスキル</span>
        <Picker
          label="召喚獣が撃つスキル"
          options={summonSkillOptions}
          note="魔法人形(ミカベア / ルシベア)・破壊精霊(アンフェル / グレシス / イグニー)が自動で撃つスキル"
          bind:value={draft.summonSkillId}
        />
      </div>
    {/if}
  </div>
  <p class="hint dim">
    {#if skills.length === 0}
      このキャラのスキルはまだ未収録です。収録されるまで攻撃力は出せません。
    {:else if draft.mainSkillId === ""}
      主軸スキルを選ぶと攻撃力が出ます。スキルの依存種別(突き / 斬り / 魔攻 / 魔防 / 複合)で装備の係数が変わるためです。
    {:else}
      攻撃力はこのスキルの依存種別で計算します(テシスコアの能力値は地域ごとのため未加算・地域なしの値)。ダメージ計算タブは選んだスキルごとに計算します。
    {/if}
  </p>
</div>

<style>

  /* アイコン変更は §10 型 5「状態が変わった」そのものなので、独自の keyframes は持たず
     app.css 共通の .badge-in(弾む)に乗る。動きを消す設定のときだけ、弾みの代わりに
     枠線で「変わった」を残す(色・弾みが両方消えると何も伝わらなくなる) */
  .current-icon { display: inline-flex; border-radius: var(--r-window); }
  @media (prefers-reduced-motion: reduce) {
    /* .badge-in は app.css の共通クラスを `use:changed` が実行時に付ける。
       静的な markup に出てこないので :global で受ける */
    .current-icon:global(.badge-in) { outline: 2px solid var(--accent); outline-offset: 2px; }
  }
</style>
<div class="card">
  <div class="card-title">能力値 <span class="dim normal">設定を触ると即時更新</span></div>
  <div class="tbl">
    <table class="grid">
      <thead><tr><th>ステ</th><th class="n">素</th><th class="n">補正</th><th>素ステ → 最終</th><th class="n">最終</th></tr></thead>
      <tbody>
        {#each STAT_KINDS as k (k)}
          {@const trace = traceFor(k)}
          {@const diff = preview ? preview.stats[k] - draft.baseStats[k] : null}
          {@const cap = trace?.stat_cap ?? 0}
          {@const basePct = cap > 0 ? Math.min(100, (draft.baseStats[k] / cap) * 100) : 0}
          {@const addPct = cap > 0 && diff !== null ? Math.max(0, Math.min(100 - basePct, (diff / cap) * 100)) : 0}
          <tr>
            <td>{STAT_LABELS[k]}</td>
            <td class="n stat-cell">
              <NumberField label="{STAT_LABELS[k]}の素ステ" min={STAT_MIN} max={limits.base_stat_max} bind:value={draft.baseStats[k]} />
            </td>
            <td class="n muted ro" title={groupTitle(k)}><Value motion={() => diff} value={diff === null ? "—" : signed(diff)} /></td>
            <!-- 素ステ → 最終を 1 本のバーで(§11)。数字の羅列ではなく「どれだけ伸びたか」を見せる。
                 灰が素ステ(振り分け)、青が補正で乗った分。長さは最終能力値の上限に対する割合 -->
            <td class="ro grow-cell">
              <span
                class="grow inset"
                title={cap > 0 ? `上限 ${fmtInt(cap)}(覚醒段階 + エタの意志 Lv)` : "上限は計算中"}
              >
                <i class="base" style="width: {basePct}%"></i>
                <i class="add" style="width: {addPct}%"></i>
              </span>
            </td>
            <td class="n final ro">
              <Value class="strong" motion={() => preview?.stats[k] ?? null} value={preview ? fmtInt(preview.stats[k]) : "—"} />
              <!-- 「満」の枠は常に確保する。出たときに行がずれない(§09 規則 4 / §11) -->
              <Value
                class={`cap-badge${trace !== null && trace !== undefined && trace.capped_loss > 0 ? " on" : ""}`}
                title={trace && trace.capped_loss > 0
                  ? `上限 ${fmtInt(trace.stat_cap)} で ${fmtInt(trace.capped_loss)} 捨てています。上限は覚醒段階とエタの意志 Lv で上がります`
                  : ""}
                value={trace !== null && trace !== undefined && trace.capped_loss > 0 ? "cap" : "open"}
              >{#snippet children()}{trace && trace.capped_loss > 0 ? "満" : ""}{/snippet}</Value>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <!-- ゲーム内の数字と合わないときに見る場所。ふだんは畳んでおき(§00 02)、
       開くと「バフ / 装備 / そのほか」で上昇分が割れる。区分の合計は必ず最終能力値に一致する -->
  <Disclosure class="contrib">
    {#snippet summary()}上昇の出どころ <span class="dim">バフ / 装備 / そのほか</span>{/snippet}
    {#if !preview || preview.source_effects.length === 0}
      <p class="empty dim">補正源なし(素ステのみ)</p>
    {:else}
      <div class="tbl">
        <table class="grid ro group-tbl">
          <thead>
            <tr>
              <th>ステ</th>
              <th class="n">素</th>
              {#each STAT_SOURCE_GROUPS as g (g)}<th class="n">{STAT_SOURCE_GROUP_LABELS[g]}</th>{/each}
              <th class="n">最終</th>
            </tr>
          </thead>
          <tbody>
            {#each STAT_KINDS as k (k)}
              <tr>
                <td>{STAT_LABELS[k]}</td>
                <td class="n muted">{fmtInt(draft.baseStats[k])}</td>
                {#each STAT_SOURCE_GROUPS as g (g)}
                  {@const e = groupEffect(k, g)}
                  <!-- 0 の区分は薄く出す。行や列が消えると、次に見たとき同じ場所を探し直すことになる -->
                  <td class="n" class:zero={e === 0}><Value motion={() => e} value={e === null ? "—" : signed(e)} /></td>
                {/each}
                <td class="n strong">
                  <Value motion={() => preview?.stats[k] ?? null} value={preview ? fmtInt(preview.stats[k]) : "—"} />
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="note dim">素 + バフ + 装備 + そのほか = 最終(上限で捨てた分も織り込み済み)。倍率をかける補正源は、先に乗った固定値を増やした分も自分の区分で受け取ります。</p>
      <!-- 「どの層の 1 件が抜けているか」までは、ここを開いて 1 件ずつ見る -->
      <Disclosure class="contrib inner">
        {#snippet summary()}1 件ずつ見る <span class="dim">{preview.source_effects.length} 件</span>{/snippet}
        <div class="tbl">
          <table class="grid ro">
            <thead><tr><th>ステ</th><th>区分</th><th>出典</th><th>層</th><th class="n">値</th><th class="n">効果</th></tr></thead>
            <tbody>
              {#each preview.source_effects as e, i (i)}
                <tr>
                  <td>{STAT_LABELS[e.kind]}</td>
                  <td class="muted">{STAT_SOURCE_GROUP_LABELS[e.group]}</td>
                  <td class="muted">{e.source}</td>
                  <td class="muted">{STAT_LAYER_LABELS[e.layer]}</td>
                  <td class="n">{formatLayerValue(e.layer, e.value)}</td>
                  <td class="n"><Value motion={() => e.effect} value={signed(e.effect)} /></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>

      </Disclosure>
    {/if}

  </Disclosure>
</div>
