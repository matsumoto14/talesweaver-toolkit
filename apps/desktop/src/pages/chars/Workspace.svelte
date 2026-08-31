<script module lang="ts">
  import type { Draft } from "../../draft";

  /**
   * キャラ id → まだ確定していない draft のキャッシュ(モジュールスコープ、複数マウントで共有)。
   *
   * 親 CharsPage/App が {#key} でこのコンポーネントを作り直す設計だが、破棄時の flush(下の
   * onDestroy)は fire-and-forget なので、保存の IPC が完了して app.characters が更新される前に
   * 再マウントが起きうる。そのとき素直に buildDraft(character) すると古い値に戻ってしまい、
   * そこへ何か 1 つ編集しただけで直前に保存された値をまるごと上書きしてしまう
   * (独立レビュー指摘: 再マウント時の stale draft によるデータ消失)。
   *
   * dirty になった時点・保存が in-flight の間はここへ入れておき、mount 時はまずここを見る。
   * 保存が成功してそれ以上の未送信の変更が無くなった時点、およびキャラ削除時に消す。
   */
  const pendingDrafts = new Map<number, { draft: Draft; initialSnapshot: string }>();
</script>

<script lang="ts">
  // 選択キャラのワークスペース(v4): 左に補正源リスト、右に選択した補正源の編集ペイン、
  // 下に「いまの実力」シート。draft(編集状態)を 1 つの $state に持ち、保存はキャラ単位で 1 ボタン。
  // 親 CharsPage が {#key character.id} で作り直す前提($effect による再同期は書かない)。
  import { SvelteSet } from "svelte/reactivity";
  import { onDestroy, untrack } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import {
    errorLocation, errorMessage, previewEffectiveStats, resolveCharacterSkillEffects,
  } from "../../api/commands";
  import type {
    BuffSelection, CharacterSkillEffectsView, CommonSkills, Equipment, EquipmentPart, PartSlot,
    RegisteredCharacter, StatPreview, StatSources,
  } from "../../api/types";
  import { deleteCharacter } from "../../api/commands";
  import { dropForeignSkills } from "../../characterSkills";
  import { buildDraft, draftToPayload } from "../../draft";
  import { cloneEquipmentPart, randomOptionCount, sienaPartCount } from "../../equipment";
  import { fmtInt } from "../../format";
  import {
    EQUIPMENT_STAT_SHORT, PART_SLOTS, STAT_KINDS, ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import { app, characterSourceFocus, enqueueCharacterSave, equipmentFocus, loadSkills, removeCharacter, skillsByCharacter, upsertCharacter } from "../../state.svelte";
  import { reportError, reportNotice } from "../../toast.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { adjustDropIndex, dropHalfIndex } from "../../ui/reorder.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import SourcePane, { type SourceId } from "./SourcePane.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  // .eq-summary / .result-value / .tiny(攻撃力カードで使う)は補正源ペインと共有するグローバル CSS
  import "./sources/pane-shared.css";
  import {
    defenseRatePercent as defenseRatePercentOf,
    equipmentAttackKindsFor,
    equipmentAttackRatePercent,
    equipmentBaseTotal,
    equipmentEnhancedTotal,
    randomOptionRecordOnlyCount,
    sharpnessRatePercent as sharpnessRatePercentOf,
    unleashSummary as unleashSummaryOf,
  } from "./summaries";

  interface Props {
    character: RegisteredCharacter;
  }
  let { character }: Props = $props();

  const DEFAULT_LIST_WIDTH = 280;
  const layoutWidths = persisted("tw-v4-chars", { list: DEFAULT_LIST_WIDTH });
  const gridTemplateColumns = $derived(
    `minmax(220px, ${layoutWidths.value.list ?? DEFAULT_LIST_WIDTH}px) 6px minmax(300px, 1fr)`,
  );

  // 親が {#key} でこのコンポーネントを作り直す前提なので初期値だけ untrack で取る。
  // ただし直前のマウントに確定していない draft(pendingDrafts)が残っていればそちらを使う —
  // このファイル冒頭(module スコープ)のコメント参照。
  const initial = untrack(() => character);
  const pending = pendingDrafts.get(initial.id);
  let initialSnapshot = $state(pending ? pending.initialSnapshot : JSON.stringify(buildDraft(initial)));
  let draft = $state<Draft>(pending ? pending.draft : buildDraft(initial));
  const draftSnapshot = $derived(JSON.stringify(draft));
  const dirty = $derived(draftSnapshot !== initialSnapshot);

  /**
   * ホームの「今日の強化」タイルは、このワークスペースを開いたままでも同じキャラの
   * レリック左右・カフスの成長値・エンチャント・神鳥の聖物を直接保存できる
   * (state.svelte.ts の upsertCharacter 経由)。draft/initialSnapshot は mount 時点の
   * スナップショットで固定する設計(このファイル冒頭の注記)なので、そのままだと直更新の
   * 内容が見えず、ここで何か保存すると古い値で上書きしてしまう。
   *
   * ホームが直更新しうる対象は「装備部位のどれか」と「stat_sources.sacred_relic のどれか」に
   * 広がった(武器・鎧の強化Lvだけの頃と違い、部位を個別に列挙する意味が薄い)ので、部位は
   * 13 部位すべて(PART_SLOTS)、聖物は 7 ステすべてを対象に、**この画面でまだ触っていない
   * (dirty でない)項目だけ**追随させるという同じ規則を、項目ごとに素直に流す形にした。
   * 触っている最中の項目は編集内容を優先してそのまま残す(そのケースは稀な衝突として許容し、
   * 警告は出さない — 触った直後に保存すればホーム側の変更を意図せず巻き戻すが、同じキャラの
   * 同じ項目をホームとキャラタブの両方で同時に触る状況自体が稀なため)。
   */
  $effect(() => {
    // --- 装備部位(13 部位すべて。武器・鎧の強化Lvもここに乗るので slot は限定しない) ---
    for (const slot of PART_SLOTS) {
      const incomingList = character.equipment.parts[slot];
      const incomingPart = incomingList.registered.find((p) => p.id === incomingList.selected_id);
      if (!incomingPart) continue;
      const draftList = draft.equipment.parts[slot];
      const draftIdx = draftList.registered.findIndex((p) => p.id === draftList.selected_id);
      if (draftIdx < 0 || draftList.registered[draftIdx].id !== incomingPart.id) continue;
      const draftPart = draftList.registered[draftIdx];
      if (JSON.stringify(draftPart) === JSON.stringify(incomingPart)) continue; // 差分なし
      const baseline = JSON.parse(initialSnapshot) as Draft;
      const baselineList = baseline.equipment.parts[slot];
      const baselinePart = baselineList.registered.find((p: EquipmentPart) => p.id === draftPart.id);
      if (baselinePart && JSON.stringify(baselinePart) !== JSON.stringify(draftPart)) continue; // 編集中は上書きしない
      draftList.registered[draftIdx] = cloneEquipmentPart(incomingPart);
      if (baselinePart) {
        const idx = baselineList.registered.findIndex((p: EquipmentPart) => p.id === incomingPart.id);
        if (idx >= 0) baselineList.registered[idx] = incomingPart;
        initialSnapshot = JSON.stringify(baseline);
      }
    }
    // --- 神鳥の聖物(stat_sources.sacred_relic。ステごとに独立して直更新されるので 1 ステずつ見る) ---
    for (const k of STAT_KINDS) {
      const incomingValue = character.stat_sources.sacred_relic[k];
      const draftValue = draft.statSources.sacred_relic[k];
      if (incomingValue === draftValue) continue; // 差分なし
      const baseline = JSON.parse(initialSnapshot) as Draft;
      const baselineValue = baseline.statSources.sacred_relic[k];
      if (baselineValue !== draftValue) continue; // 編集中は上書きしない
      draft.statSources.sacred_relic[k] = incomingValue;
      baseline.statSources.sacred_relic[k] = incomingValue;
      initialSnapshot = JSON.stringify(baseline);
    }
  });

  let saving = $state(false);
  // 名前未入力・キャラ種未選択のときは保存できない(失敗させない)。手動保存時代の canSubmit を転用。
  const canAutoSave = $derived(draft.name.trim().length > 0 && draft.gameCharacterId !== "");
  /** ツールバーの状態表示用。保存できない理由(§00 05 考えさせない — 理由を言葉で出す) */
  const unsavedReason = $derived(
    draft.gameCharacterId === "" ? "キャラを選んでください" : draft.name.trim().length === 0 ? "名前を入力してください" : null,
  );

  /**
   * dirty(保存が要る)か saving(送信中)のあいだ、このキャラの draft をモジュールスコープの
   * pendingDrafts へ入れておく。再マウント時の stale draft 対策(このファイル冒頭の注記)。
   * 両方とも false になった時点(=送信済みで、それ以上の未送信の変更も無い)でエントリを消す。
   */
  $effect(() => {
    if (dirty || saving) {
      pendingDrafts.set(character.id, { draft, initialSnapshot });
    } else {
      pendingDrafts.delete(character.id);
    }
  });

  // キャラ種を切り替えたら旧キャラ専用のキャラスキルを落とす(幽霊スキル対策、既存決定を踏襲)。
  let lastGameCharacterId = draft.gameCharacterId;
  $effect(() => {
    const currentId = draft.gameCharacterId;
    if (currentId === lastGameCharacterId || app.characterSkills.length === 0) return;
    lastGameCharacterId = currentId;
    draft.statSources.character_skills.skill_ids = dropForeignSkills(
      draft.statSources.character_skills.skill_ids,
      app.characterSkills,
      currentId,
    );
  });

  // 主軸スキル(攻撃力の依存種別を決める)。選択肢はキャラ種のスキル一覧。
  $effect(() => {
    void loadSkills(draft.gameCharacterId);
  });
  const skills = $derived(skillsByCharacter[draft.gameCharacterId] ?? []);
  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);

  // 即時プレビュー(100ms debounce)。エラーはペイン内に控えめに表示(トーストは出さない)。
  let preview = $state<StatPreview | null>(null);
  let previewError = $state<string | null>(null);
  const previewLatest = latest({ debounce: 100 });
  $effect(() => {
    const baseStats = { ...draft.baseStats };
    const statSources = JSON.parse(JSON.stringify(draft.statSources)) as StatSources;
    // シエナのオーラのステ加算が最終能力値に乗るので、装備もプレビューの入力に含める
    const equipment = JSON.parse(JSON.stringify(draft.equipment)) as Equipment;
    // 浅いコピーだとネストした値(アンリーシュの枠・極限スキル)の変更を $effect が追跡できず、
    // 触っても再計算が走らない。装備と同じく deep copy で全プロパティを読む
    const commonSkills = JSON.parse(JSON.stringify(draft.commonSkills)) as CommonSkills;
    // 最終能力値の上限は覚醒段階 + エタの意志 Lv で決まるので、覚醒もプレビューの入力に含める
    const awakening = { stage: Number(draft.stage), eternal_level: Number(draft.eternalLevel) };
    const mainSkillId = draft.mainSkillId === "" ? null : draft.mainSkillId;
    // いつものバフも**この場で**読む。previewLatest.run は debounce するので、run に渡す
    // closure の中で読んだ値は $effect の依存に入らない — バフを付け替えても再計算が走らず、
    // 能力値だけ古いまま残っていた(上の deep copy が同じ理由でここに置かれているのと同じ)
    const buffs = JSON.parse(JSON.stringify(
      app.buffSets.find((set) => set.id === draft.defaultBuffSetId)?.choices ?? { choices: [] },
    )) as BuffSelection;
    previewLatest.run((isCurrent) =>
      previewEffectiveStats(
        baseStats, statSources, equipment, commonSkills, awakening, mainSkillId, buffs,
      )
        .then((p) => {
          if (isCurrent()) {
            preview = p;
            previewError = null;
          }
        })
        .catch((e) => {
          if (isCurrent()) previewError = errorMessage(e);
        }),
    );
    return () => previewLatest.cancel();
  });

  // キャラスキル全件ぶんの、選んでいるマスタリーを踏まえた実際の効果(マスタリー解決は Rust 側)。
  // CharacterSkillPane / ActualDelayPane の効果ラベル表示に使う
  let resolvedSkillEffects = $state<CharacterSkillEffectsView[]>([]);
  const skillEffectsLatest = latest({ debounce: 100 });
  $effect(() => {
    const masteries = { picked: [...draft.statSources.masteries.picked] };
    skillEffectsLatest.run((isCurrent) =>
      resolveCharacterSkillEffects(masteries).then((r) => {
        if (isCurrent()) resolvedSkillEffects = r;
      }),
    );
    return () => skillEffectsLatest.cancel();
  });

  /** 直前に自動保存で出したエラーメッセージ。同じ内容の連打を防ぐ(空 flush では読まない)。 */
  let lastAutoSaveError: string | null = null;
  /** 保存が in-flight のあいだに来た呼び出しを捨てず、完了後にもう一度だけやり直すためのフラグ
   *  (独立レビュー指摘: 保存中の debounce が捨てられ、未送信分が「保存済み」扱いになる)。 */
  let saveAgain = false;

  async function autoSave() {
    if (!canAutoSave) return;
    if (saving) {
      saveAgain = true;
      return;
    }
    saving = true;
    // 保存直前の時点で app.sim(試し変更)があったかどうかで、破棄したことを知らせるか決める。
    // upsertCharacter が無条件で app.sim を null にする(state.svelte.ts のコメント参照)ので先に見ておく
    const hadSim = app.sim !== null;
    try {
      // ホームの直更新タイルと同じキューへ通し、キャラ単位で保存を直列化する
      // (理由は上の HOME_DIRECT_SLOTS の $effect コメント参照)。
      // enqueueCharacterSave は「自分の番」が来てからペイロードビルダーを呼ぶので、
      // initialSnapshot に採る draft のスナップショットも同じビルダーの中(=実際に送信した瞬間)
      // で取る。await の後に draft を読み直すと、保存中に入った編集まで「保存済み」に取り込んで
      // しまう(独立レビュー指摘)。
      let sent = "";
      const saved = await enqueueCharacterSave(character.id, () => {
        sent = JSON.stringify(draft);
        return draftToPayload(draft);
      });
      initialSnapshot = sent;
      upsertCharacter(saved);
      lastAutoSaveError = null;
      if (hadSim) reportNotice("キャラを保存したので、ダメージ計算の試し変更を解除しました");
    } catch (e) {
      // どこの話か分かるエラーは帯から飛べるようにする。キャラは呼び出し側しか知らない。
      // ただし自動保存は手動より高頻度なので、直前と同じメッセージなら再表示しない(連打防止)
      const message = errorMessage(e);
      if (message !== lastAutoSaveError) {
        lastAutoSaveError = message;
        const location = errorLocation(e);
        reportError(message, location ? { characterId: character.id, location } : null);
      }
    } finally {
      saving = false;
      if (saveAgain) {
        saveAgain = false;
        void autoSave();
      }
    }
  }

  let confirmDelete = $state(false);
  let confirmDeleteTimer: ReturnType<typeof setTimeout> | null = null;
  async function removeThis() {
    if (!confirmDelete) {
      confirmDelete = true;
      if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
      confirmDeleteTimer = setTimeout(() => (confirmDelete = false), 4000);
      return;
    }
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = null;
    try {
      await deleteCharacter(character.id);
      pendingDrafts.delete(character.id);
      removeCharacter(character.id);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }

  // --- 補正源リスト -------------------------------------------------------
  let openSource = $state<SourceId>("status");

  /** この編集で変更した補正源(左のリストに小さなドットで出す)。クリアはしない — 再マウントで消える。
      素の Set は $state に入れても深く追跡されない(.add() が再描画を起こさない)ので SvelteSet を使う */
  const changedSources = new SvelteSet<SourceId>();

  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  // draft が変わったら 600ms の debounce で自動保存する。開いていた補正源があれば「変更元」として記録する
  $effect(() => {
    void draftSnapshot; // 依存: draft の深い変更を拾う
    if (!dirty) return;
    changedSources.add(openSource);
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = null;
      void autoSave();
    }, 600);
  });

  // タブ切替・キャラ切替でこのワークスペースが破棄されるとき、debounce 待ち中の未送信分を
  // 待たずに即 flush する(タイマーごと消えると「未保存の変更が消える」問題がそのまま残る)。
  // タイマーの有無ではなく dirty を見る — !canAutoSave で早期 return した直後に離脱すると
  // タイマーは既に消えているが未送信の変更はまだ残っている(独立レビュー指摘)。
  // その変更は pendingDrafts に残るので、!canAutoSave のときここで autoSave() を呼んでも
  // 何も送らず消える心配は無い。保存の完了は待たない(fire-and-forget)。
  onDestroy(() => {
    if (saveTimer !== null) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (dirty) void autoSave();
  });

  // 別タブからの「この補正源を開く」要求。読んだらその場で捨てる — 残したままだと、
  // 自動保存が app.characters を差し替えるたびにこの $effect が(character プロップの変化で)
  // 走り直し、ユーザーが自分で開き直した補正源を古い要求で上書きしてしまう
  // (例: ホームのシエナタイル → 共通スキルを開く → Lv 変更 → 保存 → シエナに戻される)。
  $effect(() => {
    const request = characterSourceFocus.request;
    if (!request || app.selectedId !== character.id) return;
    openSource = request.sourceId;
    characterSourceFocus.request = null;
  });

  // エラー帯の「ここを開く」で指された場所は、まず補正源を開くところまでをここが担う。
  // 部位を開いて該当行を光らせるのは、開いた先のペイン(装備 / ランダムOP)が続きをやる。
  $effect(() => {
    const request = equipmentFocus.request;
    if (!request || app.selectedId !== character.id) return;
    openSource = request.randomOptionId !== null ? "randomOption" : "equipment";
  });

  const petCount = $derived(STAT_KINDS.filter((k) => draft.statSources.pet_skills[k] !== null).length);
  const runeTotal = $derived(STAT_KINDS.reduce((s, k) => s + draft.statSources.rune_levels[k], 0));
  const crownTotal = $derived(STAT_KINDS.reduce((s, k) => s + draft.statSources.crown[k], 0));
  const monsterCardTotal = $derived(
    STAT_KINDS.reduce((s, k) => s + draft.statSources.monster_cards[k], 0),
  );
  // ソウルリンクは基本値 4 種 + クリ/最終/武器倍率の最大 7 項目を持つが、列幅 157px には
  // 短い項目でも 2 つが精一杯。ゼロの基本値(例: 突+0)は出さず、効き方の強い順
  // (最終ダメ → クリダメ → 武器倍率 → 基本値)に並べて先頭 2 件だけを残す
  // (開いた先の ContribCard.svelte で全項目が見える)
  const soulLinkSummary = $derived.by(() => {
    if (!preview) return "計算中";
    const v = preview.soul_link.equipment_values;
    const finalPct = Number((preview.soul_link.final_damage_rate * 100).toFixed(1));
    const critPct = Number((preview.soul_link.critical_damage_rate * 100).toFixed(1));
    const weapon = preview.soul_link.weapon_added_damage_multiplier;
    const candidates = [
      finalPct > 0 ? `最終+${finalPct}%` : null,
      critPct > 0 ? `クリ+${critPct}%` : null,
      weapon > 1 ? `武器×${weapon.toFixed(1)}` : null,
      v.thrust > 0 ? `突+${v.thrust}` : null,
      v.slash > 0 ? `斬+${v.slash}` : null,
      v.magic_attack > 0 ? `魔攻+${v.magic_attack}` : null,
      v.magic_defense > 0 ? `魔防+${v.magic_defense}` : null,
    ].filter((x): x is string => x !== null);
    return candidates.length > 0 ? candidates.slice(0, 2).join(" ・ ") : NEUTRAL;
  });
  const relicTotal = $derived(preview?.sacred_relic_total ?? 0);
  const skillCount = $derived(draft.statSources.character_skills.skill_ids.length);
  /** 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン)。行サブタイトルと共通スキルペインの
   *  両方が使うので summaries.ts の共有関数(計算は Rust 側 preview) */
  const enhanceRatePercent = $derived(equipmentAttackRatePercent(preview));
  /** 基本能力値の合計(Σ part.base + 装備アビリティ + 称号 + ソウルリンク)。行サブタイトルと
   *  装備ペインの両方が使うので summaries.ts の共有関数(計算は Rust 側 preview) */
  const eqBaseTotal = $derived(equipmentBaseTotal(preview));
  /** 強化能力値の合計。いまの実力バーの装備ブロックと共有(summaries.ts。計算は Rust 側 preview) */
  const eqEnhancedTotal = $derived(equipmentEnhancedTotal(preview));
  const equipmentAttackKinds = $derived(equipmentAttackKindsFor(mainSkill?.dependency ?? null));
  const equipmentSummary = $derived(
    `基本合計 ${equipmentAttackKinds.map((k) => `${EQUIPMENT_STAT_SHORT[k]}${fmtInt(eqBaseTotal[k])}`).join(" / ")}`,
  );
  const sienaParts = $derived(sienaPartCount(draft.equipment));
  /** シエナのオーラの攻撃力増加(New1)の合計 %。計算は Rust 側(preview) */
  const sienaRate = $derived(Math.round((preview?.siena_attack_rate ?? 0) * 100));
  /** シエナのオーラのステ加算(能力値スロット + 全ステ増加)の 7 ステ合計。計算は Rust 側(preview) */
  const sienaStats = $derived(preview?.siena_stat_total ?? 0);
  // テシスコアの合計(コア効果の最大)は行サブタイトルにだけ出す。セット効果・地域別の内訳は
  // ThesisCorePane 側(結果の置き場所)。計算は Rust 側(preview.thesis_cores)
  const coreBestTotal = $derived(Math.max(0, ...(preview?.thesis_cores.map((r) => r.total_bonus) ?? [])));
  const roCount = $derived(randomOptionCount(draft.equipment));
  /** ランダムOP のうち記録するだけの枠数。行サブタイトルと RandomOptionPane の両方が使うので
   *  summaries.ts の共有関数(計算は Rust 側 preview) */
  const roRecordOnly = $derived(randomOptionRecordOnlyCount(preview));
  const pct = (v: number) => Number((v * 100).toFixed(2));
  const NEUTRAL = "未設定(中立値で計算)";

  // 中ディレイ減少(wiki: ステータス「中ディレイ倍率B」)。ここはキャラスキルのぶんだけ。
  // 共通の供給源(フルスロットル / カフスの RO / シエナのオーラ)はそれぞれの補正源で設定する。
  const delaySummary = $derived.by(() => {
    const ids = draft.statSources.character_skills.skill_ids;
    if (ids.length === 0) return NEUTRAL;
    // 供給源別の内訳(preview.character_skill_actual_delay)は Rust 側で解決済み。ここは合計するだけ
    const percent = pct((preview?.character_skill_actual_delay ?? []).reduce((sum, c) => sum + c.rate, 0));
    if (percent === 0) return `${ids.length} 件`;
    return `${ids.length} 件 ・ 合計 −${percent}%`;
  });

  // 共通スキルの効き先(結果側の表示用)。入力は補正源、計算は Rust 側(preview / limits)を参照する。
  // 行サブタイトルと共通スキルペインの両方が使うものは summaries.ts の共有関数を呼ぶ
  const defenseRatePercent = $derived(defenseRatePercentOf(preview));
  const sharpnessRatePercent = $derived(sharpnessRatePercentOf(draft));
  const unleashSummary = $derived(unleashSummaryOf(draft));

  // 共通スキル(wiki: Skill/共通)。効き先ごとに 1 行でまとめる
  const commonSkillSummary = $derived.by(() => {
    const c = draft.commonSkills;
    const parts: string[] = [];
    if (enhanceRatePercent > 0) parts.push(`装備攻撃力 +${enhanceRatePercent}%`);
    if (defenseRatePercent.physical > 0) parts.push(`装備防御力 物+${defenseRatePercent.physical}%`);
    if (c.sharpness_vision_level > 0) {
      parts.push(`追加ダメージ +${sharpnessRatePercent}%`);
    }
    const ultimate = c.ultimate.slots.filter((u) => u !== null);
    if (ultimate.length > 0) {
      parts.push(`極限 ${ultimate.map((u) => ULTIMATE_SKILL_LABELS[u]).join(" / ")}`);
    }
    // アンリーシュ(能力解放)。効き先は能力値倍率B
    if (unleashSummary !== "未使用") {
      parts.push(`解放 ${unleashSummary}`);
    }
    // 行の補足は 1 行に収まる分だけ。全部詰めると必ず切れる(実測: 4 項目は言うまでもなく、
    // 2 項目でも「装備攻撃力 +20% ・ 装備防御力 物+144%」で列幅 157px を超えて切れる)ので、
    // 一番効いている項目を 1 つだけ出す。全項目は開いた先のペインで見える
    return parts.length === 0 ? NEUTRAL : parts.slice(0, 1).join(" ・ ");
  });

  // 称号は 1 枠。表示中の 1 件だけが効く(wiki: 称号システム)。
  // 列幅 157px には称号名(依存違いの変種は「- 斬り」まで含む)と値を両方出す余地が無いので、
  // 一番効いている 1 値(ダメ増加があればそれ、無ければ装備基本能力値の合計)だけを添える
  // (詳しい内訳は開いた先の TitlePane.svelte で見える)
  const titleSummary = $derived.by(() => {
    const t = app.titles.find((x) => x.id === draft.equipment.title);
    if (!t) return NEUTRAL;
    const headline = t.attack_damage_percent > 0 ? `ダメ +${t.attack_damage_percent}%` : `合計 +${fmtInt(t.equipment_value_total)}`;
    return `${t.name}(${headline})`;
  });

  // クリティカル率(wiki: 計算式まとめ #CriticalChance)。ここはペット会心と増加だけ。
  // 装備クリティカル補正・AGI・スキルの Cri値は登録済みのデータから自動で入る。
  const criticalRateSummary = $derived.by(() => {
    const c = draft.statSources.critical_rate;
    const parts: string[] = [];
    if (c.pet) parts.push("ペット会心 ×1.1");
    const bonus = preview?.critical_rate_bonus.value ?? 0;
    if (bonus > 0) parts.push(`増加 +${Math.round(bonus)}%`);
    return parts.length === 0 ? NEUTRAL : parts.join(" ・ ");
  });

  const sources = $derived<{ id: SourceId; name: string; sub: string }[]>([
    {
      id: "status",
      name: "キャラステータス",
      // 一番効いている 2 項目(覚醒段階とエタの意志 Lv は能力値上限を決める)だけ。属性はここでは
      // 出さない(開いた先のペインに出る。全部詰めると必ず切れる)
      sub: `覚醒 ${draft.stage} 段階 ・ エタの意志 Lv${draft.eternalLevel}`,
    },
    {
      id: "commonSkill",
      name: "共通スキル",
      sub: commonSkillSummary,
    },
    {
      id: "equipment",
      name: "装備",
      sub: equipmentSummary,
    },
    {
      id: "soulLink",
      name: "ソウルリンク",
      sub: soulLinkSummary,
    },
    {
      id: "title",
      name: "称号",
      sub: titleSummary,
    },
    {
      id: "randomOption",
      name: "ランダムOP",
      sub:
        roCount > 0
          ? `${roCount} 枠${roRecordOnly > 0 ? ` ・ うち ${roRecordOnly} 枠は記録のみ` : ""}`
          : NEUTRAL,
    },
    {
      id: "siena",
      name: "シエナのオーラ",
      sub:
        sienaParts > 0
          ? [
              `${sienaParts} 部位`,
              ...(sienaRate > 0 ? [`攻撃力 +${sienaRate}%`] : []),
              ...(sienaStats > 0 ? [`ステ +${fmtInt(sienaStats)}`] : []),
            ].join(" ・ ")
          : NEUTRAL,
    },
    {
      id: "thesis",
      name: "テシスコア",
      sub: coreBestTotal > 0 ? `最大 合計 ${fmtInt(coreBestTotal)}` : NEUTRAL,
    },
    { id: "relic", name: "神鳥の聖物", sub: relicTotal > 0 ? `合計 +${fmtInt(relicTotal)}` : NEUTRAL },
    { id: "crown", name: "クラウン", sub: crownTotal > 0 ? `合計 +${fmtInt(crownTotal)}` : NEUTRAL },
    {
      id: "monsterCard",
      name: "モンスターカード",
      sub: monsterCardTotal > 0 ? `合計 +${fmtInt(monsterCardTotal)}` : NEUTRAL,
    },
    { id: "skills", name: "キャラスキル", sub: skillCount > 0 ? `${skillCount} 件選択` : NEUTRAL },
    { id: "actualDelay", name: "中ディレイ減少", sub: delaySummary },
    { id: "criticalRate", name: "クリティカル率", sub: criticalRateSummary },
    { id: "pet", name: "ペット S スキル", sub: petCount > 0 ? `${petCount} 種` : NEUTRAL },
    { id: "rune", name: "ルーンスキル", sub: runeTotal > 0 ? `合計 +${fmtInt(runeTotal)}` : NEUTRAL },
  ]);
  // 補正源の並びはプレイヤーが決める(design-system §14 決定 3)。
  //
  // 全部を同じ行の重さで並べるのは、設計をしていないのと同じ。ただし「どれをよく触るか」は
  // 人によって違うので、こちらで頻度を決め打ちしない。**お気に入り(★)で重さを分け、
  // 並びはドラッグで動かす**。§09 規則 5「並びは、ユーザーが頼まない限り変わらない」の
  // 裏返しで、頼まれたら変わってよい。設定画面は作らない — リストの上で直接動かす。
  //
  // ★ はホームタブのコンテンツと同じ操作なので、覚えることが増えない。
  const DEFAULT_ORDER: SourceId[] = [
    "status", "skills", "equipment", "soulLink", "commonSkill", "thesis", "siena", "relic",
    "crown", "monsterCard", "pet", "rune", "actualDelay", "criticalRate",
    "title", "randomOption",
  ];
  interface SourceLayout {
    /** お気に入り。上に置いて常に開く */
    fav: string[];
    /** そのほか */
    rest: string[];
  }
  const layout = persisted("chars.sourceLayout", { fav: [], rest: [...DEFAULT_ORDER] } as SourceLayout);
  /** 保存済みの並びに無い補正源(あとから増えたもの)は「そのほか」の既定位置に戻す */
  const ordered = $derived.by<SourceLayout>(() => {
    const known = new Set<string>(sources.map((s) => s.id));
    const fav = (layout.value.fav ?? []).filter((id) => known.has(id));
    const seen = new Set(fav);
    const rest = (layout.value.rest ?? []).filter((id) => known.has(id) && !seen.has(id));
    rest.forEach((id) => seen.add(id));
    for (const id of DEFAULT_ORDER) if (known.has(id) && !seen.has(id)) rest.push(id);
    return { fav, rest };
  });
  const itemsOf = (ids: string[]) => ids.map((id) => sources.find((s) => s.id === id)).filter((s) => s !== undefined);

  /** ★ を外したときの戻り先。既定の並びの位置に戻す — 末尾に飛ばすと移動距離が長くなる */
  function insertByDefaultOrder(rest: string[], id: string): string[] {
    const rank = DEFAULT_ORDER.indexOf(id as SourceId);
    const at = rest.findIndex((x) => DEFAULT_ORDER.indexOf(x as SourceId) > rank);
    const next = [...rest];
    next.splice(at === -1 ? next.length : at, 0, id);
    return next;
  }
  /**
   * ★ の付け外し。付けたら お気に入りの末尾、外したら **既定の並びの位置**へ。
   *
   * ここでは**スクロールしない**。押した ★ は指の下にあるので、画面が動くと
   * 次に押すつもりだった行が別のものに入れ替わる(§00 03「押した場所は動かない」)。
   * どこへ行ったかは着地の弾みで見せる。自分で運ぶドラッグは別で、そちらは追いかける
   */
  function toggleFavorite(id: string) {
    const { fav, rest } = ordered;
    layout.value = fav.includes(id)
      ? { fav: fav.filter((x) => x !== id), rest: insertByDefaultOrder(rest, id) }
      : { fav: [...fav, id], rest: rest.filter((x) => x !== id) };
    mark(id);
  }
  /**
   * 動きを消す設定(prefers-reduced-motion)のときは 0 にする。
   * CSS のアニメーションは app.css が一括で殺しているが、Svelte の animate は JS なので
   * ここで見る必要がある(§10「動きを消しても変化が分かること」)。
   */
  const motionDuration = (ms: number) =>
    typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : ms;

  /**
   * 群をまたいで動いた行。同じ群の中の並べ替えは `animate:flip` が滑らせるが、
   * 群をまたぐと別の `{#each}` になるので繋がらない。着地を弾ませて、
   * どれが動いたのかを目で追えるようにする(§10 型 5「状態が変わった」)。
   */
  let movedId = $state<string | null>(null);
  let movedTimer: ReturnType<typeof setTimeout> | undefined;

  /** 着地を弾ませるだけ。画面は動かさない */
  function mark(id: string) {
    clearTimeout(movedTimer);
    movedId = null;
    requestAnimationFrame(() => {
      movedId = id;
      movedTimer = setTimeout(() => (movedId = null), 400);
    });
  }
  /** 行を追いかける。自分で起こした移動(ドラッグ・未設定ジャンプ)だけで使う(§09 規則 5) */
  function follow(id: string) {
    clearTimeout(movedTimer);
    movedId = null;
    requestAnimationFrame(() => {
      document.querySelector(`[data-source-id="${id}"]`)?.scrollIntoView({ block: "nearest" });
      movedId = id;
      movedTimer = setTimeout(() => (movedId = null), 400);
    });
  }

  // --- ドラッグで並べ替え ---------------------------------------------------
  let dragId = $state<string | null>(null);
  /** いま落ちる位置。行の上半分なら手前、下半分なら後ろ */
  let dropAt = $state<{ list: "fav" | "rest"; index: number } | null>(null);

  function onDragStart(e: DragEvent, id: string) {
    dragId = id;
    e.dataTransfer?.setData("text/plain", id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
  function onDragOverRow(e: DragEvent, list: "fav" | "rest", index: number) {
    if (dragId === null) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropAt = { list, index: dropHalfIndex(r, e.clientY, index, "y") };
  }
  function onDragOverList(e: DragEvent, list: "fav" | "rest", count: number) {
    if (dragId === null) return;
    e.preventDefault();
    if (dropAt === null || dropAt.list !== list) dropAt = { list, index: count };
  }
  function onDrop(e: DragEvent) {
    e.preventDefault();
    const id = dragId;
    const at = dropAt;
    dragId = null;
    dropAt = null;
    if (id === null || at === null) return;
    const next: SourceLayout = {
      fav: ordered.fav.filter((x) => x !== id),
      rest: ordered.rest.filter((x) => x !== id),
    };
    const arr = next[at.list];
    const from = ordered[at.list].indexOf(id);
    const index = adjustDropIndex(from, at.index);
    arr.splice(Math.max(0, Math.min(arr.length, index)), 0, id);
    layout.value = next;
    follow(id);
  }
  /** ドラッグできない環境(キーボード)向け。表には出さない */
  function onRowKey(e: KeyboardEvent, list: "fav" | "rest", index: number, id: string) {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    dragId = id;
    dropAt = { list, index: index + (e.key === "ArrowUp" ? -1 : 2) };
    onDrop(new DragEvent("drop"));
  }

  const PLANNED: string[] = [];
  /** 未設定の補正源(表示順)。バッジから順に開いて回れる */
  const neutralIds = $derived(
    [...ordered.fav, ...ordered.rest].filter((id) => sources.find((s) => s.id === id)?.sub === NEUTRAL),
  );
  const neutralCount = $derived(neutralIds.length);

  /** 未設定バッジで次の未設定を開く。自分で起こした移動なので追いかける(§09 規則 5) */
  function jumpToNeutral() {
    if (neutralIds.length === 0) return;
    const at = neutralIds.indexOf(openSource);
    const id = neutralIds[(at + 1) % neutralIds.length] as SourceId;
    openSource = id;
    follow(id);
  }
</script>

<div class="workspace">
  <div class="toolbar">
    <span class="char-name">{draft.name || "(名前未設定)"}</span>
    <span class="spacer"></span>
    <label class="buff-default">
      <span>いつものバフ</span>
      <!-- バフセットはユーザーが増やしていくもので上限が無い。§07 形態 2(段階選択)は
           「選択肢が有限で並べても横に溢れない」ときのもの(ui/StepSelect.svelte 冒頭コメント)。
           いまは 2 件でも、セットが増えるたびに段が横に溢れて折り返すのでは意味が変わってしまう
           ので、件数に関わらず select で固定する(機械監査「選択肢2は§07形態2に降ろせる」への回答) -->
      <select bind:value={draft.defaultBuffSetId}>
        <option value={null}>なし</option>
        {#each app.buffSets as set (set.id)}
          <option value={set.id}>{set.name}</option>
        {/each}
      </select>
    </label>
    <!-- debounce 待ち(dirty かつ未送信)も「保存中…」に含める。まだ書き込んでいないあいだ
         「保存済み」と出すのは嘘になる — この表示の役目は保存を信用させることなので譲れない。
         ただし保存できない状態(名前未入力・キャラ種未選択)のときは「保存中…」ではなく理由を
         警告として出す(独立レビュー指摘: 理由の無い「保存中…」のまま編集が消えていた) -->
    <span class="save-status dim" class:warn={!canAutoSave && dirty}>
      {!canAutoSave && dirty ? `未保存 — ${unsavedReason}` : saving || dirty ? "保存中…" : "保存済み"}
    </span>
    <button type="button" class="btn danger delete" class:confirm={confirmDelete} onclick={removeThis}>
      {confirmDelete ? "もう一度押すと削除します" : "このキャラを削除"}
    </button>
  </div>

  <div class="cols" style="grid-template-columns: {gridTemplateColumns};">
    <section class="sources">
      <div class="src-head">
        <span class="src-title">補正源</span>
        {#if neutralCount > 0}
          <button type="button" class="src-unset" title="次の未設定を開く" onclick={jumpToNeutral}>
            未設定 {neutralCount} 件 ›
          </button>
        {/if}
      </div>
      <div class="src-list">
        <!-- お気に入りと そのほか の 2 段。重さの差はプレイヤーが ★ で決める(§14 決定 3) -->
        {#each [{ key: "fav" as const, title: "お気に入り", ids: ordered.fav }, { key: "rest" as const, title: "そのほか", ids: ordered.rest }] as list (list.key)}
          <div
            class="src-group"
            role="list"
            ondragover={(e) => onDragOverList(e, list.key, list.ids.length)}
            ondrop={onDrop}
          >
            <div class="group-head">
              <span class="group-title">{list.title}</span>
              <span class="group-note dim">{list.ids.length} 件</span>
            </div>
            {#if list.ids.length === 0}
              <p class="group-empty dim">★ を押すか、行をここへ運ぶと上がります。</p>
            {/if}
            {#each itemsOf(list.ids) as s, i (s.id)}
              <!-- 行そのものが面。★ はその中のボタンで、面を 2 枚に割らない(§01)。
                   並べ替えたら**動いた行だけ**が新しい場所へ滑る(§10「変わった要素だけ動かす」)。
                   0.5s を超えない — 待たせるための動きは要らない -->
              <div
                animate:flip={{ duration: motionDuration(260), easing: cubicOut }}
                class="src src-line"
                class:badge-in={movedId === s.id}
                class:on={openSource === s.id}
                class:dragging={dragId === s.id}
                class:drop-before={dropAt?.list === list.key && dropAt.index === i}
                class:drop-after={dropAt?.list === list.key && dropAt.index === i + 1 && i === list.ids.length - 1}
                data-source-id={s.id}
                role="button"
                tabindex="0"
                draggable="true"
                onclick={() => (openSource = s.id)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    openSource = s.id;
                  } else {
                    onRowKey(e, list.key, i, s.id);
                  }
                }}
                ondragstart={(e) => onDragStart(e, s.id)}
                ondragend={() => { dragId = null; dropAt = null; }}
                ondragover={(e) => onDragOverRow(e, list.key, i)}
                ondrop={onDrop}
              >
                <span class="grip" aria-hidden="true" title="つかんで並べ替え">⠿</span>
                <button
                  type="button"
                  class="fav"
                  class:on={list.key === "fav"}
                  aria-label="{s.name} を{list.key === 'fav' ? 'お気に入りから外す' : 'お気に入りに入れる'}"
                  title={list.key === "fav" ? "お気に入りから外す" : "お気に入りに入れる"}
                  onclick={(e) => { e.stopPropagation(); toggleFavorite(s.id); }}
                >★</button>
                <span class="src-main">
                  <span class="src-name">{s.name}</span>
                  <span class="src-sub num" use:flash={() => s.sub}>{s.sub}</span>
                </span>
                <span
                  class="src-changed"
                  class:show={changedSources.has(s.id)}
                  use:flash={() => (changedSources.has(s.id) ? "on" : "off")}
                  title={changedSources.has(s.id) ? "この編集で変更(保存済み)" : undefined}
                  aria-hidden="true"
                ></span>
                <span class="chev dim">›</span>
              </div>
            {/each}
          </div>
        {/each}
        {#each PLANNED as name (name)}
          <div class="src planned">
            <span class="src-main">
              <span class="src-name">{name}</span>
              <span class="src-sub">これから</span>
            </span>
          </div>
        {/each}
      </div>
      <p class="src-note dim">常用バフは<b>ダメージ計算</b>タブの「計算の材料」で選べます。グレーの補正源はこれから。</p>
    </section>

    <Splitter
      bind:value={layoutWidths.value.list}
      min={220}
      defaultValue={DEFAULT_LIST_WIDTH}
      controls="prev"
      label="補正源リストと編集ペインの境界"
    />

    <section class="detail">
      <SourcePane
        characterId={character.id}
        {draft}
        {preview}
        {previewError}
        {skills}
        {resolvedSkillEffects}
        sourceId={openSource}
        onOpenSource={(id) => (openSource = id)}
      />
    </section>
  </div>

  <div class="sheet">
    <span class="sheet-title">いまの実力</span>
    <span class="sheet-equipment num dim">
      装備
      {#each equipmentAttackKinds as k, i (k)}
        {#if i > 0}<span class="sep"> ・ </span>{/if}{EQUIPMENT_STAT_SHORT[k]}
        <span use:bump={() => eqBaseTotal[k]}>{fmtInt(eqBaseTotal[k])}</span>
        <span class="enhance" use:bump={() => eqEnhancedTotal[k]}>+{fmtInt(eqEnhancedTotal[k])}</span>
      {/each}
    </span>
    {#if mainSkill}
      <span class="sheet-attack">
        <span class="attack-label">攻撃力(A)</span>
        <span class="num strong" use:bump={() => preview?.attack?.breakdown.value ?? null}>
          {preview?.attack ? fmtInt(preview.attack.breakdown.value) : "—"}
        </span>
        <span class="dim">{mainSkill.name}</span>
      </span>
    {:else}
      <span class="sheet-attack dim">主軸スキルを選ぶと攻撃力が出ます</span>
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost sheet-goto" onclick={() => (app.tab = "calc")}>
      ダメージを見る ›
    </button>
  </div>
</div>

<style>
  .buff-default { display: flex; align-items: center; gap: 7px; color: var(--fg-muted); font-size: 10px; }
  .buff-default select { min-width: 150px; height: 28px; border: 1px solid var(--border); border-radius: var(--r-inset); background: var(--bg-field); color: var(--fg); }
  .workspace { flex: 1; min-height: 0; display: flex; flex-direction: column; }

  .toolbar {
    flex-shrink: 0; display: flex; align-items: center; gap: 10px;
    padding: 10px 16px 0;
  }
  .char-name { font-size: 13px; font-weight: 800; }
  /* 押せない状態表示(§00 03 押した場所は動かない — ボタンにしない)。
     文言が「保存済み」/「保存中…」/「未保存 — 理由」で長さが変わるので、右寄せ + 幅を
     先に確保して隣の削除ボタンが動かないようにする(§00 03、design-system §09 規則 4) */
  .save-status { flex-shrink: 0; min-width: 190px; font-size: 10px; text-align: right; white-space: nowrap; }
  .save-status.warn { font-weight: 700; color: var(--warm); }
  .spacer { flex: 1; }

  .cols { flex: 1; min-height: 0; display: grid; padding: 10px 16px 8px; column-gap: 0; }
  section { min-width: 0; min-height: 0; display: flex; flex-direction: column; }

  .src-head { display: flex; align-items: baseline; gap: 8px; padding: 0 2px 7px; }
  .src-title { font-size: var(--t-label); font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); }
  .src-unset {
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted);
    background: var(--bg-field); border: 1px solid var(--border-strong); border-radius: var(--r-pill); padding: 0 6px;
    cursor: pointer;
  }
  .src-unset:hover { border-color: var(--accent); color: var(--accent); }
  /* お気に入りと そのほか の 2 段(§14 決定 3)。重さの差はプレイヤーが決める */
  .src-group { min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .group-head {
    min-width: 0; display: flex; align-items: center; gap: 8px; padding: 2px 2px 0;
    font-size: 9.5px; letter-spacing: 0.08em;
  }
  .group-title { flex-shrink: 0; font-weight: 800; color: var(--fg-muted); }
  .group-note { min-width: 0; flex: 1; letter-spacing: 0; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .group-empty { margin: 0 0 2px; padding: 0 2px; font-size: 9.5px; }

  /* つかんで運ぶ。落ちる位置は行の縁に線で出す — 隙間を差し込むと下が全部ずれる(§09 規則 1) */
  .src-line {
    position: relative; cursor: grab;
    /* 中のテキストが選択されるとドラッグがそちらに取られて、掴んでも動かない */
    user-select: none; -webkit-user-drag: element;
  }
  .src-line:active { cursor: grabbing; }
  /* 掴めることを見た目でも言う。ふだんは薄く、行に触れたら濃くなる */
  .grip {
    flex-shrink: 0; width: 9px; text-align: center;
    font-size: 11px; line-height: 1; color: var(--fg-off); letter-spacing: -1px;
  }
  .src:hover .grip, .src-line.dragging .grip { color: var(--accent); }
  .src-line.dragging { opacity: 0.45; }
  .src-line.drop-before::before, .src-line.drop-after::after {
    content: ""; position: absolute; left: 0; right: 0; height: 2px;
    background: var(--accent); border-radius: var(--r-pill);
  }
  .src-line.drop-before::before { top: -4px; }
  .src-line.drop-after::after { bottom: -4px; }
  /* ★ はホームタブのコンテンツと同じ操作・同じ寸法。金 = あなたの操作待ち(§03 予約色) */
  .src .fav {
    width: 20px; height: 20px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); border: 1px solid var(--border-soft);
    font-size: 10px; color: var(--fg-off);
  }
  .src .fav:hover { border-color: var(--gold); color: var(--gold); }
  .src .fav.on { background: #FDF9EE; border-color: var(--gold); color: var(--gold); }

  .src-list { flex: 1; min-height: 0; overflow: auto; scrollbar-gutter: stable; display: flex; flex-direction: column; gap: 6px; }
  .src {
    display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .src:hover:not(.planned) { border-color: var(--accent); }
  .src.on { background: var(--sel-card); border-color: var(--accent); }
  .src.planned { background: #F0F3F7; border-style: dashed; cursor: default; }
  .src-main { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .src-name { font-size: 11px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .src.planned .src-name, .src.planned .src-sub { color: var(--fg-off); }
  .src-sub { font-size: 9px; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* この編集で変更した補正源の印。場所は常に確保し、opacity だけで出し入れする
     (§09 規則 4「あとから幅が変わらない」)。初回だけ badge-in(use:flash)で光る */
  .src-changed {
    flex-shrink: 0; width: 6px; height: 6px; border-radius: 50%;
    background: var(--accent); opacity: 0;
  }
  .src-changed.show { opacity: 1; }
  .chev { flex-shrink: 0; font-size: 11px; }

  .src-note {
    flex-shrink: 0; margin: 10px 0 0; padding: 9px 11px; border-radius: var(--r-panel);
    background: var(--bg-rail); border: 1px dashed var(--border);
    font-size: 9.5px; line-height: 1.5; color: var(--fg-muted);
  }

  /* 縦スクロールバーが後から出ると幅が縮み、右寄せのものが左へ飛ぶ。
     場所を先に確保しておく(§09 規則 4「あとから幅が変わらない」) */
  .detail { overflow: auto; scrollbar-gutter: stable; padding-left: 6px; }

  /* 開閉しない 1 行の結果バー。押した場所は動かない — 高さ固定で桁が増えても伸び縮みしない */
  .sheet {
    flex-shrink: 0; height: 36px; box-sizing: border-box; display: flex; align-items: center; gap: 9px;
    border-top: 1px solid var(--border-strong); background: var(--bg-mid); padding: 0 16px;
  }
  .sheet-title { flex-shrink: 0; font-size: var(--t-label); font-weight: 700; letter-spacing: 0.06em; color: var(--fg-head); white-space: nowrap; }
  /* 火力の材料(装備値)。相手ありきのダメージは出さない — このバーの役目は装備↑攻撃力(A)まで */
  /* flex: 1 で伸ばすと装備値と攻撃力が 1216px の両端に離れ、視線が横断する(§00 01)。
     材料 → 結果 は隣り合わせにして左に固め、余白は右のボタンの手前に集める */
  .sheet-equipment {
    min-width: 0; flex: 0 1 auto; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .sheet-equipment .enhance { color: var(--good); }
  /* 主軸スキルの攻撃力(A)。ダメージと取り違えないようラベルを必ず数字の前に置く */
  .sheet-attack { flex-shrink: 0; margin-left: 9px; display: flex; align-items: baseline; gap: 6px; font-size: 9.5px; white-space: nowrap; }
  .attack-label { font-weight: 700; color: var(--fg-head); }
  .sheet-attack .strong { font-size: 13px; }
  .sheet-goto { flex-shrink: 0; padding: 5px 10px; font-size: var(--t-label); white-space: nowrap; }
  /* 「保存」の真隣に置くと押し間違える。ひと呼吸ぶん離す(2 段階確認はそのまま) */
  .delete {
    flex-shrink: 0; margin-left: 14px; padding: 7px 12px; border-radius: var(--r-panel);
    background: var(--bg-field); border-color: var(--state-short-bd); font-size: var(--t-label); color: var(--danger);
  }
  .delete.confirm { background: var(--state-short-bg); font-weight: 700; }
</style>
