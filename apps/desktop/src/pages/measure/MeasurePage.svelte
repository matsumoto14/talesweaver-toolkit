<script lang="ts">
  // 実測: ゲームで実際に出たダメージを、条件ごと送る画面。
  //
  // 計算タブと分けているのは **まだ収録していない敵**も対象にするため。新しく実装された
  // モブは wiki にも載っていないので計算タブの対象一覧には出ない。ここでは敵の名前を
  // 自分で書いて送れる(計算値は出せないが、攻撃側の条件は敵に依らず出せる)。
  //
  // 逆算そのものはここでやらない。集めた実測を突き合わせて gamedata を直す
  // (手順は docs/enemy-verification.md)。
  import { errorMessage, listSkills, previewDamage, previewEffectiveStats } from "../../api/commands";
  import type { AttackPowerBreakdown, DamageResult, EffectiveStats, Skill } from "../../api/types";
  import { fmtInt } from "../../format";
  import {
    canSeparate, damageGap, expectedDamage, measurementDraft, type MeasurementSample,
  } from "../../measurement";
  import { app, flatContents, payloadOf, selectedCharacter } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import CheckChip from "../../ui/CheckChip.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { bump } from "../../ui/motion.svelte";
  import Picker from "../../ui/Picker.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";

  const character = $derived(selectedCharacter());
  const payload = $derived(character ? payloadOf(character) : null);

  // --- 対象 -----------------------------------------------------------------
  const TARGET_KINDS = [
    { value: "listed", label: "一覧から選ぶ" },
    { value: "unlisted", label: "一覧に無い敵" },
  ];
  let targetKind = $state("listed");
  const contents = $derived(
    flatContents().filter(
      (x): x is typeof x & { content: { enemy_id: string } } => x.content.enemy_id !== null,
    ),
  );
  let pickedContentId = $state("");
  const content = $derived(
    contents.find((x) => x.content.id === pickedContentId) ?? contents[0] ?? null,
  );
  let unlistedName = $state("");
  let unlistedPlace = $state("");

  // --- スキル ---------------------------------------------------------------
  let skills = $state<Skill[]>([]);
  let skillsGid: string | null = null;
  $effect(() => {
    const gid = character?.game_character_id ?? null;
    if (!gid || gid === skillsGid) return;
    skillsGid = gid;
    listSkills(gid)
      .then((list) => (skills = list))
      .catch((e) => reportError(errorMessage(e)));
  });
  let pickedSkillId = $state("");
  const skill = $derived(
    skills.find((s) => s.id === pickedSkillId)
      ?? skills.find((s) => s.id === character?.main_skill_id)
      ?? skills[0]
      ?? null,
  );

  // --- 攻撃側(敵に依らない)と、収録済みの敵での計算 ------------------------
  let result = $state<DamageResult | null>(null);
  let attack = $state<AttackPowerBreakdown | null>(null);
  let stats = $state<EffectiveStats | null>(null);
  const damageLatest = latest({ debounce: 150 });
  $effect(() => {
    const payloadJson = payload ? JSON.stringify(payload) : null;
    const skillIdForCalc = skill?.id ?? null;
    const contentIdForCalc = targetKind === "listed" ? (content?.content.id ?? null) : null;
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!payloadJson || !skillIdForCalc) {
      damageLatest.cancel();
      result = null;
      attack = null;
      return;
    }
    damageLatest.run(async (isCurrent) => {
      try {
        const parsed = JSON.parse(payloadJson);
        const preview = await previewEffectiveStats(
          parsed.base_stats, parsed.stat_sources, parsed.equipment, parsed.common_skills,
          parsed.awakening, skillIdForCalc, JSON.parse(buffsJson),
        );
        // 収録済みの敵のときだけ計算値を出す。未収録は「出せない」と正直に見せる
        const damage = contentIdForCalc
          ? await previewDamage(
              JSON.parse(payloadJson), skillIdForCalc, contentIdForCalc, 0, null, null,
              JSON.parse(buffsJson),
            )
          : null;
        if (!isCurrent()) return;
        // 収録済みの敵では計算に使った攻撃力をそのまま採る(テシスコアは対象の地域で
        // 解決されるので、地域なしの preview_effective_stats とは値がずれる)
        attack = damage?.trace.attack ?? preview.attack?.breakdown ?? null;
        stats = preview.stats;
        result = damage;
      } catch (error) {
        reportError(errorMessage(error));
      }
    });
  });

  // --- 実測(点を溜めてから送る)---------------------------------------------
  // 1 点では防御力とカット率を分けられない。**装備を替えて攻撃力を変えた 2 点以上**が要る
  // (docs/enemy-verification.md)ので、ここで溜めて 1 通で送る。
  let measuredDamage = $state<number | null>(null);
  let measuredCritical = $state(false);
  let measuredHits = $state(10);
  let measuredNote = $state("");
  let samples = $state<MeasurementSample[]>([]);
  const expected = $derived(expectedDamage(result, measuredCritical));
  const gap = $derived(measuredDamage !== null ? damageGap(measuredDamage, expected) : null);
  const targetReady = $derived(
    targetKind === "listed" ? content !== null : unlistedName.trim().length > 0,
  );
  const canAdd = $derived(measuredDamage !== null && measuredDamage > 0 && skill !== null && targetReady);
  const canSend = $derived(samples.length > 0 && skill !== null && character !== null && targetReady);
  const separable = $derived(canSeparate(samples));

  /** いまの入力を 1 点として記録し、入力欄は次の点のために空にする */
  function addSample() {
    if (measuredDamage === null) return;
    samples = [
      ...samples,
      {
        damage: measuredDamage,
        critical: measuredCritical,
        hits: measuredHits,
        note: measuredNote,
        attack: attack?.value ?? null,
        stats,
        expected,
      },
    ];
    measuredDamage = null;
    measuredNote = "";
  }

  function removeSample(index: number) {
    samples = samples.filter((_, i) => i !== index);
  }

  function send() {
    if (!skill || !character || samples.length === 0) return;
    app.inquiryPrefill = measurementDraft(
      {
        gameCharacterId: character.game_character_id,
        awakeningStage: character.awakening.stage,
        eternalLevel: character.awakening.eternal_level,
        skill,
        comboSkillType: null,
        content: targetKind === "listed" && content
          ? { id: content.content.id, name: content.content.name, enemyId: content.content.enemy_id }
          : null,
        unlisted: targetKind === "unlisted"
          ? { name: unlistedName.trim(), place: unlistedPlace.trim() }
          : null,
      },
      samples,
    );
  }
</script>

<div class="measure-page">
  <div class="scroll">
    {#if !character}
      <p class="empty dim">キャラを選択してください。</p>
    {:else}
      <p class="lead dim">
        敵の防御力・カット率は wiki でも「約」「推定値」で、ゲーム内では見られません。
        <b>実測を集めて逆算する</b>しか確かめる方法がないので、出た値を送ってもらえると助かります。
        まだ収録していない敵(新しいモブ)も、名前を書いて送れます。
      </p>

      <div class="section">
        <div class="area-head"><span class="area-name">対象</span><span class="area-rule"></span></div>
        <StepSelect options={TARGET_KINDS} bind:value={targetKind} full />
        {#if targetKind === "listed"}
          <Picker
            label="対象"
            bind:value={
              () => content?.content.id ?? "",
              (v) => (pickedContentId = v)
            }
            options={contents.map((x) => ({ value: x.content.id, name: x.content.name }))}
          />
        {:else}
          <div class="fields">
            <label class="field">
              <span class="label">敵の名前</span>
              <input class="text-field" type="text" maxlength="60" bind:value={unlistedName}
                placeholder="ゲーム内の表記どおりに" />
            </label>
            <label class="field">
              <span class="label">出た場所(任意)</span>
              <input class="text-field" type="text" maxlength="60" bind:value={unlistedPlace}
                placeholder="マップ名・コンテンツ名" />
            </label>
          </div>
          <p class="note dim">
            この敵はまだ収録していないので、ツールの計算値は出せません。実測と攻撃側の条件を送ってもらえれば、
            こちらで敵の値を逆算します。
          </p>
        {/if}
      </div>

      <div class="section">
        <div class="area-head"><span class="area-name">使ったスキル</span><span class="area-rule"></span></div>
        <Picker
          label="スキル"
          bind:value={
            () => skill?.id ?? "",
            (v) => (pickedSkillId = v)
          }
          options={skills.map((s) => ({ value: s.id, name: s.name }))}
        />
        <div class="attack-row">
          <span class="dim">攻撃力(A)</span>
          <span class="num" use:bump={() => attack?.value ?? null}>{attack ? fmtInt(attack.value) : "—"}</span>
          <span class="dim">最終能力値も一緒に送られます(逆算の入力になります)</span>
        </div>
      </div>

      <div class="section">
        <div class="area-head"><span class="area-name">出たダメージ</span><span class="area-rule"></span></div>
        <div class="fields">
          <label class="field">
            <span class="label">実測ダメージ(1 発)</span>
            <input
              class="num-field" type="number" min="1" inputmode="numeric"
              value={measuredDamage ?? ""}
              oninput={(e) => {
                const v = Number(e.currentTarget.value);
                measuredDamage = Number.isFinite(v) && v > 0 ? Math.trunc(v) : null;
              }}
            />
          </label>
          <label class="field short">
            <span class="label">何発中の最大</span>
            <input
              class="num-field" type="number" min="1" inputmode="numeric"
              value={measuredHits}
              oninput={(e) => {
                const v = Number(e.currentTarget.value);
                measuredHits = Number.isFinite(v) && v > 0 ? Math.trunc(v) : 1;
              }}
            />
          </label>
          <CheckChip checked={measuredCritical} onCheckedChange={(v) => (measuredCritical = v)}>
            <span>クリティカルだった</span>
          </CheckChip>
        </div>

        {#if targetKind === "listed"}
          <div class="compare">
            <!-- どちら側と比べているかを必ず書く(計算タブの「1 発」はクリ率 > 0 ならクリ側) -->
            <span class="dim">このツールの計算({measuredCritical ? "クリティカル" : "非クリ最大"})</span>
            <span class="num">{expected !== null ? fmtInt(Math.trunc(expected)) : "—"}</span>
            <span class="dim">差</span>
            <span class="num" class:warn={gap !== null && Math.abs(gap) >= 0.05} use:bump={() => gap}>
              {gap === null ? "—" : `${gap >= 0 ? "+" : ""}${(gap * 100).toFixed(1)}%`}
            </span>
          </div>
        {/if}

        <label class="field wide">
          <span class="label">気づいたこと(任意)</span>
          <input class="text-field" type="text" maxlength="200" bind:value={measuredNote}
            placeholder="強打が乗ったかも / 上限に当たっていそう など" />
        </label>

        <div class="send">
          <button type="button" class="btn" disabled={!canAdd} onclick={addSample}>この 1 点を記録する</button>
          <span class="dim">記録したら、装備を替えて攻撃力を変え、もう 1 点測ってください。</span>
        </div>
      </div>

      <div class="section">
        <div class="area-head">
          <span class="area-name">記録した点</span>
          <span class="area-rule"></span>
          <span class="count num">{samples.length}</span>
        </div>
        {#if samples.length === 0}
          <p class="note dim">
            まだ 1 点もありません。<b>攻撃力を変えた 2 点以上</b>あると、防御力とカット率を分けて
            逆算できます(1 点だけでも送れますが、分けられません)。
          </p>
        {:else}
          <div class="samples">
            {#each samples as sample, index (index)}
              <div class="sample-row">
                <span class="tag">{index + 1}</span>
                <span class="dim">攻撃力</span>
                <span class="num">{sample.attack !== null ? fmtInt(sample.attack) : "—"}</span>
                <span class="dim">実測</span>
                <span class="num">{fmtInt(sample.damage)}</span>
                {#if sample.critical}<span class="tag crit">クリ</span>{/if}
                <span class="dim">{fmtInt(sample.hits)} 発中</span>
                {#if sample.note}<span class="dim sample-note">{sample.note}</span>{/if}
                <button type="button" class="btn danger sample-del" onclick={() => removeSample(index)}>外す</button>
              </div>
            {/each}
          </div>
          <p class="note dim" class:ready={separable}>
            {separable
              ? "攻撃力の違う点が 2 つ以上あります。防御力とカット率を分けて逆算できます。"
              : "攻撃力が同じ点だけです。装備を替えてもう 1 点足すと、防御力とカット率を分けられます。"}
          </p>
        {/if}

        <div class="send">
          <button type="button" class="btn primary" disabled={!canSend} onclick={send}>
            {samples.length} 点まとめて送る
          </button>
          <span class="dim">送信前に全文を確認できます。</span>
        </div>
      </div>

      <p class="foot dim">
        測り方(上限に当たっていないか・非クリの最大だけを採る・攻撃力を 2 段階にして 2 点測る)は
        docs/enemy-verification.md にまとめてあります。
      </p>
    {/if}
  </div>
</div>

<style>
  .measure-page { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 720px; }
  .empty { font-size: 12px; }
  .lead { margin: 0; font-size: 11px; line-height: 1.7; }

  /* 見出しはホーム・お知らせと同じ形(§00 01 視線を動かさない) */
  .section { display: flex; flex-direction: column; gap: 8px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: var(--r-inset); background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }

  .fields { display: flex; align-items: flex-end; gap: 10px; flex-wrap: wrap; }
  .field { display: flex; flex-direction: column; gap: 3px; }
  .field .label { font-size: 9px; font-weight: 700; letter-spacing: 0.06em; color: var(--fg-muted); }
  .field.wide { width: 100%; }
  .field.short .num-field { width: 72px; }
  /* 上限のない自由入力(§07 形態 5)。スピナーは他の数値欄に無いので消してそろえる */
  .num-field { width: 148px; appearance: textfield; }
  .num-field::-webkit-outer-spin-button,
  .num-field::-webkit-inner-spin-button { appearance: none; margin: 0; }
  .text-field {
    width: 100%; min-width: 220px; padding: 4px 8px; border-radius: var(--r-panel);
    background: #fff; border: 1px solid var(--border); font-size: 11px; color: var(--fg);
  }
  .note {
    margin: 0; padding: 5px 9px; font-size: 10px; line-height: 1.6;
    border: 1px dashed var(--border); border-radius: var(--r-panel); background: var(--bg-rail);
  }

  .attack-row, .compare {
    display: flex; align-items: baseline; gap: 8px; padding: 6px 10px; border-radius: var(--r-window);
    background: var(--surface-inset); border: 1px solid var(--border-soft); font-size: 10px;
  }
  .attack-row .num, .compare .num { font-size: 12.5px; font-weight: 700; }
  .compare .num.warn { color: var(--warm); }

  .send { display: flex; align-items: center; gap: 10px; font-size: 9.5px; flex-wrap: wrap; }

  /* 記録した点。行は増えるだけで、押した場所より上は動かない(§00 03) */
  .count { font-size: 10.5px; font-weight: 700; color: var(--fg-sub); }
  .samples { display: flex; flex-direction: column; gap: 5px; }
  .sample-row {
    display: flex; align-items: baseline; gap: 8px; padding: 5px 10px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-soft); font-size: 10px;
  }
  .sample-row .num { font-size: 11.5px; font-weight: 700; }
  .sample-note { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sample-del { margin-left: auto; flex: none; font-size: 9px; padding: 1px 8px; }
  .tag {
    flex: none; padding: 1px 7px; border-radius: var(--r-pill);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted);
  }
  .tag.crit { background: var(--state-edge-bg); border-color: var(--state-edge-bd); color: var(--state-edge-fg); }
  /* 2 点そろったら「分けられる」と分かるようにする(§00 05) */
  .note.ready { border-style: solid; border-color: var(--state-met-bd); background: var(--state-met-bg); }
  .foot { margin: 0; font-size: 10px; line-height: 1.7; }
</style>
