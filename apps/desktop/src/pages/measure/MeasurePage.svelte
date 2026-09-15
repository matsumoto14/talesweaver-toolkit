<script lang="ts">
  // 実測: ゲームで実際に出たダメージを、条件ごと送る画面。
  //
  // 計算タブと分けているのは **まだ収録していない敵**も対象にするため。新しく実装された
  // モブは wiki にも載っていないので計算タブの対象一覧には出ない。ここでは敵の名前を
  // 自分で書いて送れる(計算値は出せないが、攻撃側の条件は敵に依らず出せる)。
  //
  // 逆算そのものはここでやらない。集めた実測を突き合わせて gamedata を直す
  // (手順は docs/enemy-verification.md)。
  import { canSeparateMeasurement, errorMessage, listSkills, previewDamage, previewEffectiveStats } from "../../api/commands";
  import type { AttackPowerBreakdown, DamageResult, EffectiveStats, Skill } from "../../api/types";
  import { skillMeta } from "../../characterSkills";
  import { fmtInt, fmtSignedPct } from "../../format";
  import {
    damageGap, expectedDamage, measurementDraft, type MeasurementSample,
  } from "../../measurement";
  import { app, flatContents, payloadOf, selectedCharacter } from "../../state.svelte";
  import type { NewCharacter } from "../../api/types";
  import { reportError } from "../../toast.svelte";
  import { latest } from "../../ui/latest.svelte";
  import ReadRow from "../../ui/ReadRow.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import Picker from "../../ui/Picker.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import ToggleRow from "../../ui/ToggleRow.svelte";
  import TextField from "../../ui/TextField.svelte";

  const character = $derived(selectedCharacter());
  const savedPayload = $derived(character ? payloadOf(character) : null);

  // --- 測るときの武器 --------------------------------------------------------
  // 2 点目は「攻撃力を変えた点」でないと意味がない。キャラタブへ戻って装備を替えて
  // 帰ってくる導線は重いので、**登録済みの武器をここで切り替えられる**ようにする。
  // 切り替えても**保存しない**(この画面のコピーを差し替えるだけ)。
  const weapons = $derived(savedPayload?.equipment.parts.weapon.registered ?? []);
  const savedWeaponId = $derived(savedPayload?.equipment.parts.weapon.selected_id ?? null);
  let weaponOverride = $state<number | null>(null);
  /** `null` = 外して測る(登録が 1 件でも攻撃力を変えられる) */
  let weaponRemoved = $state(false);
  const weaponId = $derived(
    weaponRemoved
      ? null
      : (weapons.some((w) => w.id === weaponOverride) ? weaponOverride : savedWeaponId),
  );
  const weaponLabel = (id: number | null) =>
    id === null ? "武器なし" : (weapons.find((w) => w.id === id)?.label || "(名前なし)");
  const weaponOptions = $derived([
    ...weapons.map((w) => ({ value: String(w.id), label: w.label || "(名前なし)" })),
    { value: "none", label: "外す" },
  ]);
  /** 武器だけ差し替えたコピー。DB にも app.sim にも書かない */
  const payload = $derived.by<NewCharacter | null>(() => {
    if (!savedPayload) return null;
    if (weaponId === savedWeaponId) return savedPayload;
    const copy = JSON.parse(JSON.stringify(savedPayload)) as NewCharacter;
    copy.equipment.parts.weapon.selected_id = weaponId;
    return copy;
  });

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
  let measuredDamage = $state(0); // 0 = 未入力
  let measuredCritical = $state(false);
  let measuredHits = $state(10);
  let measuredNote = $state("");
  let samples = $state<MeasurementSample[]>([]);
  const expected = $derived(expectedDamage(result, measuredCritical));
  const gap = $derived(measuredDamage > 0 ? damageGap(measuredDamage, expected) : null);
  const targetReady = $derived(
    targetKind === "listed" ? content !== null : unlistedName.trim().length > 0,
  );
  const canAdd = $derived(measuredDamage > 0 && skill !== null && targetReady);
  const canSend = $derived(samples.length > 0 && skill !== null && character !== null && targetReady);
  // 防御力とカット率を分けて逆算できるか。判定は Rust(can_separate_measurement)。
  let separable = $state(false);
  const separableLatest = latest();
  $effect(() => {
    const attacks = samples.map((s) => s.attack);
    separableLatest.run((isCurrent) =>
      canSeparateMeasurement(attacks)
        .then((value) => { if (isCurrent()) separable = value; })
        .catch(() => { if (isCurrent()) separable = false; }),
    );
    return () => separableLatest.cancel();
  });

  /** いまの入力を 1 点として記録し、入力欄は次の点のために空にする */
  function addSample() {
    if (measuredDamage <= 0) return;
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
        weapon: weaponLabel(weaponId),
      },
    ];
    measuredDamage = 0;
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
      separable,
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
            options={contents.map((x) => ({ value: x.content.id, name: x.content.name, meta: x.areaName }))}
          />
        {:else}
          <div class="fields">
            <label class="field">
              <span class="label">敵の名前</span>
              <TextField label="敵の名前(ゲーム内の表記どおりに)" bind:value={unlistedName} max={60} />
            </label>
            <label class="field">
              <span class="label">出た場所(任意)</span>
              <TextField label="出た場所(マップ名・コンテンツ名)" bind:value={unlistedPlace} max={60} />
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
          options={skills.map((s) => ({ value: s.id, name: s.name, meta: skillMeta(s), iconId: s.id, iconKind: "skill" as const }))}
        />
        {#if weapons.length > 0}
          <!-- 攻撃力を変える一番かんたんな手段。押した瞬間に攻撃力が変わる(保存はされない)。
               登録が 1 件でも「外す」で 2 点目を作れる -->
          <StepSelect
            label="測るときの武器"
            options={weaponOptions}
            cols={2}
            bind:value={
              () => (weaponId === null ? "none" : String(weaponId)),
              (v) => {
                weaponRemoved = v === "none";
                if (v !== "none") weaponOverride = Number(v);
              }
            }
          />
        {/if}
        {#if weaponId !== savedWeaponId}
          <p class="note dim">
            この画面だけ <b>{weaponLabel(weaponId)}</b> で計算しています。
            キャラに登録した装備(<b>{weaponLabel(savedWeaponId)}</b>)は変わりません。
          </p>
        {/if}
        <div class="readrows inset">
          <ReadRow label="攻撃力(A)" value={attack ? fmtInt(attack.value) : "—"} motion={() => attack?.value ?? null}>
            {#snippet note()}最終能力値も一緒に送られます(逆算の入力になります){/snippet}
          </ReadRow>
        </div>
      </div>

      <div class="section">
        <div class="area-head"><span class="area-name">出たダメージ</span><span class="area-rule"></span></div>
        <div class="fields">
          <!-- 実測値は外部データで取れない値なので自由入力(§07 形態 5)。理由チップで例外だと示す。
               0 = 未入力(点として溜められない)。上限は無い(max <= min で縛らない) -->
          <div class="field">
            <span class="label">実測ダメージ(1 発)</span>
            <StatInput label="実測ダメージ(1 発)" hideLabel min={0} max={0} gauge={false} reason="実測値 · 一時" digits={8} bind:value={measuredDamage} />
          </div>
          <!-- 何発は 1 押しに意味がある(1 発ずつ数えた値)のでステッパー(形態 4) -->
          <div class="field">
            <span class="label">何発中の最大</span>
            <StatInput label="何発中の最大" hideLabel min={1} max={999} gauge={false} stepper bind:value={measuredHits} />
          </div>
          <ToggleRow
            name="クリティカルだった"
            on={measuredCritical}
            tone="temp"
            onToggle={() => (measuredCritical = !measuredCritical)}
          />
        </div>

        {#if targetKind === "listed"}
          <div class="readrows inset">
            <!-- どちら側と比べているかを必ず書く(計算タブの「1 発」はクリ率 > 0 ならクリ側) -->
            <ReadRow
              label={`このツールの計算(${measuredCritical ? "クリティカル" : "非クリ最大"})`}
              value={expected !== null ? fmtInt(Math.trunc(expected)) : "—"}
              motion={() => expected}
            />
            <ReadRow
              label="差"
              value={gap === null ? "—" : fmtSignedPct(gap, 1)}
              motion={() => gap}
              tone={gap !== null && Math.abs(gap) >= 0.05 ? "down" : null}
            />
          </div>
        {/if}

        <label class="field wide">
          <span class="label">気づいたこと(任意)</span>
          <TextField label="気づいたこと" bind:value={measuredNote} max={200} />
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
                <span class="meta-pill">{index + 1}</span>
                <span class="dim">攻撃力</span>
                <span class="num">{sample.attack !== null ? fmtInt(sample.attack) : "—"}</span>
                <span class="dim">実測</span>
                <span class="num">{fmtInt(sample.damage)}</span>
                {#if sample.critical}<span class="meta-pill crit">クリ</span>{/if}
                <span class="dim">{fmtInt(sample.hits)} 発中</span>
                {#if sample.weapon}<span class="dim sample-note">{sample.weapon}</span>{/if}
                {#if sample.note}<span class="dim sample-note">{sample.note}</span>{/if}
                <button type="button" class="btn danger sample-del" onclick={() => removeSample(index)}>消す</button>
              </div>
            {/each}
          </div>
          <p class="note dim" class:ready={separable}>
            {separable
              ? "攻撃力の違う点が 2 つ以上あります。防御力とカット率を分けて逆算できます。"
              : "攻撃力が同じ点だけです。上の「測るときの武器」を替えて(外すのでも構いません)、もう 1 点測ってください。"}
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
  .note {
    margin: 0; padding: 5px 9px; font-size: 10px; line-height: 1.6;
    border: 1px dashed var(--border); border-radius: var(--r-panel); background: var(--bg-rail);
  }


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

  .crit { background: var(--state-edge-bg); border-color: var(--state-edge-bd); color: var(--state-edge-fg); }
  /* 2 点そろったら「分けられる」と分かるようにする(§00 05) */
  .note.ready { border-style: solid; border-color: var(--state-met-bd); background: var(--state-met-bg); }
  .foot { margin: 0; font-size: 10px; line-height: 1.7; }
</style>
