<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listRotationChoices, listSkills, listUpgradeCandidates,
    previewDamage, previewDefense,
  } from "../../api/commands";
  import type {
    Adjustments, CharacterDamageResult, ComboSkillType, ContentEvaluation, DefenseProfile, NewCharacter,
    RotationChoices, Skill, UpgradeCandidate,
  } from "../../api/types";
  import { fmtDuration, fmtInt, fmtNum, fmtPct } from "../../format";
  import { ELEMENT_LABELS, STAT_KINDS } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { tables } from "../../tables.svelte";
  import { app, damageContents, flatContents } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Choose from "../../ui/Choose.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Popover from "../../ui/Popover.svelte";
  import ReadRow from "../../ui/ReadRow.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";
  import SplitPage from "../../ui/SplitPage.svelte";
  import Value from "../../ui/Value.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { changed, markRecalculated } from "../../ui/motion.svelte";
  import { REACH_BADGES, REACH_STATE, reachOk, STATE, type Badge } from "../../ui/states";
  import DamageChain from "./DamageChain.svelte";
  import DefensePanel from "./DefensePanel.svelte";
  import MaterialsPane from "./MaterialsPane.svelte";
  import RotationPane from "./RotationPane.svelte";
  import WhyPanel from "./WhyPanel.svelte";
  import { changedFlowKeys, deltaText, flowRowsOf, stepValue, stepsOf } from "./damageDetail";
  import { DetailStore } from "./detailStore.svelte";
  import { SimStore } from "./simStore.svelte";

  const DEFAULT_RIGHT_WIDTH = 380;

  const COMBO_SKILL_TYPE_OPTIONS = [
    { value: "general", label: "一般" },
    { value: "instant", label: "瞬撃" },
    { value: "chain", label: "連撃" },
  ];

  /** 試し変更(この計算だけの変更。保存しない)。KNOBS と上限は calc/simStore */
  const sim = new SimStore();
  const character = $derived(sim.character);
  const savedPayload = $derived(sim.saved);
  const payload = $derived(sim.payload);

  // --- 対象(コンテンツ) --------------------------------------------------
  // ダメージ計算には敵データが要るので、enemy_id を持つコンテンツだけを対象に出す
  // (敵未収録のコンテンツはホームで入場条件のみ判定する)。
  // 絞り込み(敵データを持つか)は state の `damageContents` が持つ。キャラタブの
  // 「いま見ている対象」も同じ 1 本(`currentDamageTarget`)から決まる
  const damageIds = $derived(new Set(damageContents().map((c) => c.id)));
  const contents = $derived(
    flatContents().filter(
      (x): x is typeof x & { content: { enemy_id: string } } => damageIds.has(x.content.id),
    ),
  );
  // 対象ピッカーも同じ絞り込みで描画する(選べない行を一覧に残さない)。
  // 敵が 1 件も無いエリアは見出しごと落とす。
  const targetAreas = $derived(
    app.areas
      .map((a) => ({ ...a, contents: a.contents.filter((c) => c.enemy_id !== null) }))
      .filter((a) => a.contents.length > 0),
  );
  const targetIndex = $derived(
    Math.max(0, contents.findIndex((x) => x.content.id === app.calcTargetId)),
  );
  const target = $derived(contents[targetIndex] ?? null);
  function stepTarget(dir: number) {
    if (contents.length === 0) return;
    app.calcTargetId = contents[(targetIndex + dir + contents.length) % contents.length].content.id;
  }

  // --- スキル(キャラタブの主軸スキルが正) --------------------------------
  let skills = $state<Skill[]>([]);
  // 取得済みスキルが属するキャラ種(非リアクティブ)。キャラ種が変わった瞬間に一覧を
  // 同期的に空へ戻す。残すと listSkills の応答まで「別キャラのステ × 前キャラのスキル」で
  // 計算・表示されてしまう(Rust 側はスキル所有チェックをしない。PR レビュー指摘)。
  let skillsGid: string | null = null;
  $effect(() => {
    const gid = character?.game_character_id ?? null;
    if (gid === skillsGid) return; // 保存等でキャラのオブジェクトだけ変わった場合は選択を保つ
    skillsGid = gid;
    skills = [];
    skillOverride = null;
    if (!gid) return;
    listSkills(gid)
      .then((list) => {
        if (skillsGid !== gid) return; // 切替済みの古い応答は捨てる
        skills = list;
      })
      .catch((e) => reportError(errorMessage(e)));
  });
  /** キャラタブで選んだ主軸の武器形態(イェフネン)。形態を持たないキャラは null。
   *  形態は技の属性なので `Skill::form` から逆引きする(対応表は TS に持たない) */
  const mainForm = $derived(
    skills.find((s) => s.id === character?.main_skill_id)?.form ?? null,
  );
  /** 本体が撃てるスキルだけ(熊 = 魔法人形が撃つスキルは主軸に選べない。ADR-016)。
   *  形態を持つキャラは**いまの形態の技だけ**にする(形態を跨ぐ選び直しはキャラタブで行う)。
   *  このタブの主軸ピッカー・一覧計算はここから絞る */
  const bodySkills = $derived(
    skills.filter((s) => s.attacker === "player" && (mainForm === null || s.form === mainForm)),
  );
  /** キャラタブで選んだ主軸スキル。この画面のスキルはこれが正 */
  const mainSkill = $derived(bodySkills.find((s) => s.id === character?.main_skill_id) ?? null);
  /**
   * この画面での選び直し(例外操作)。null = 主軸に従う。
   * キャラを替えたとき・キャラタブで主軸を変えたときは主軸に揃え直す(下の $effect)。
   * 保存はしない ＝ ラベンダー(--sim)で見せる。
   */
  let skillOverride = $state<string | null>(null);
  let lastMainSkillId = untrack(() => character?.main_skill_id ?? null);
  $effect(() => {
    const id = character?.main_skill_id ?? null;
    if (id === lastMainSkillId) return;
    lastMainSkillId = id;
    skillOverride = null;
  });
  // 主軸が未設定・未収録のときだけ先頭スキルにフォールバックする
  const skillId = $derived(
    (skillOverride !== null && bodySkills.some((s) => s.id === skillOverride) ? skillOverride : null)
      ?? mainSkill?.id
      ?? bodySkills[0]?.id
      ?? "",
  );
  const skill = $derived(bodySkills.find((s) => s.id === skillId) ?? null);
  let comboSkillType = $state<ComboSkillType>("general");
  const selectedComboSkillType = $derived<ComboSkillType | null>(
    skill && skill.combo_variants.length > 0 ? comboSkillType : null,
  );
  let lastComboSkillId = untrack(() => skillId);
  $effect(() => {
    if (skillId === lastComboSkillId) return;
    lastComboSkillId = skillId;
    comboSkillType = "general";
  });
  /** 主軸と違うスキルで計算している状態 */
  const skillOverridden = $derived(mainSkill !== null && skillId !== mainSkill.id);
  let skillOpen = $state(false);
  /** ピッカーの並びは合計ダメージの降順(v4 指定)。合計が未取得のものは登録順で末尾 */
  const pickerSkills = $derived(
    [...bodySkills].sort((a, b) => (skillTotals[b.id]?.total ?? -1) - (skillTotals[a.id]?.total ?? -1)),
  );

  // スキル一覧の対象別ダメージ(ドロップダウンを開いたときに計算)。熊(魔法人形)が撃つ
  // スキルは本体では撃てない一覧なので出さない
  let skillTotals = $state<Record<string, { perHit: number; total: number }>>({});
  const skillLatest = latest();
  $effect(() => {
    // 対象・キャラ・試し変更が変わったら古い合計を出さない(PR レビュー指摘)
    skillTotals = {};
    if (!skillOpen || !payload || !target || bodySkills.length === 0) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    const temp = JSON.parse(JSON.stringify(NEUTRAL_ADJUSTMENTS)) as Adjustments;
    const contentId = target.content.id;
    const selectedSkillId = skillId;
    const comboType = selectedComboSkillType;
    const buffs = JSON.parse(JSON.stringify(app.calcBuffs));
    skillLatest.run((isCurrent) =>
      Promise.all(
        bodySkills.map(async (s) => [
          s.id,
          await previewDamage(
            p, s.id, contentId, comboCount, temp,
            s.id === selectedSkillId ? comboType : (s.combo_variants.length > 0 ? "general" : null),
            buffs,
          ),
        ] as const),
      )
        .then((rs) => {
          if (!isCurrent()) return;
          skillTotals = Object.fromEntries(
            // 合計は <フラグ> 爆発まで込みの合算(combined)。並び順もこの値で決まる
            rs.map(([id, r]) => [id, { perHit: r.body.per_hit_primary, total: r.combined.total_primary }]),
          );
        })
        .catch((e) => reportError(errorMessage(e))),
    );
  });

  let combo = $state(false);
  /** コンボボーナスが付く最小のコンボ数(Rust の limits)。ON のときだけ渡す */
  const comboCount = $derived(combo ? limits.combo_bonus_threshold : 0);

  // --- コンボで挟む通常攻撃 --------------------------------------------------
  // コンボボーナス(倍率A・中ディレイ半減)は通常攻撃を挟んで初めて成立する。
  // どれを挟むかで CI(次のスキルの中ディレイの下限)が変わるので、速い順に段で並べ、
  // **既定は一番速いもの**にする(ユーザーに選ばせないのが既定。§ux 原則 1)。
  let normalAttackOverride = $state<string | null>(null);
  const normalAttacks = $derived(
    [...skills.filter((s) => s.normal_attack)].sort(
      (a, b) => (a.combo_interval ?? Infinity) - (b.combo_interval ?? Infinity),
    ),
  );
  const normalAttackOptions = $derived(
    normalAttacks.map((s) => ({
      value: s.id,
      // 「†極・突き」の飾りは段では邪魔なので落とし、CI を添える(未収録は ?)
      label: `${s.name.replace(/^†[^・]*・/, "")} ${s.combo_interval !== null ? fmtNum(s.combo_interval, 2, "s") : "?"}`,
    })),
  );
  const normalAttackId = $derived(
    normalAttacks.some((s) => s.id === normalAttackOverride)
      ? normalAttackOverride
      : (normalAttacks[0]?.id ?? null),
  );
  const comboNormalAttackId = $derived(combo ? normalAttackId : null);

  // --- 回し(差し込む CT 技。この計算だけの選択で保存しない) -----------------
  // 候補・既定 ON・損得は Rust の回しそのもの(list_rotation_choices)が返す。キャラタブと
  // 同じ 1 本で、CT 判定も既定の選び方もここには写さない。
  // null = キャラの保存値(未設定なら既定)どおり。キャラ・主軸・対象を変えたら null に戻す。
  let rotationOverride = $state<string[] | null>(null);
  /** 一時の差し込みを当てた payload。保存はしない(計算タブの試し変更と同じ流儀) */
  const withRotation = (p: NewCharacter): NewCharacter =>
    rotationOverride === null ? p : { ...p, rotation_skill_ids: rotationOverride };
  let rotationChoices = $state<RotationChoices | null>(null);
  const rotationLatest = latest({ debounce: 200 });
  $effect(() => {
    // 候補も損得も**この画面の材料**(選び直した技・コンボ・一時調整)で出す。
    // 依存種別はキャラの主軸から決まる(preview_damage と同じ。Rust 側が持つ規則)
    const pJson = payload ? JSON.stringify(withRotation(payload)) : null;
    const sid = skillId;
    const t = target;
    const comboType = selectedComboSkillType;
    const normalId = comboNormalAttackId;
    const count = comboCount;
    const tempJson = JSON.stringify(NEUTRAL_ADJUSTMENTS);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !sid || !t) {
      rotationLatest.cancel();
      rotationChoices = null;
      return;
    }
    rotationLatest.run((isCurrent) =>
      listRotationChoices(
        JSON.parse(pJson), t.content.id, JSON.parse(buffsJson),
        sid, count, comboType, normalId, JSON.parse(tempJson),
      )
        .then((r) => {
          if (isCurrent()) rotationChoices = r;
        })
        .catch(() => {
          // 編集途中で検証が通らないだけ。直前の候補をそのまま残す(押そうとしたチップを消さない)
        }),
    );
    return () => rotationLatest.cancel();
  });
  const rotationCandidates = $derived(rotationChoices?.candidates ?? []);
  /** いま ON の技。未設定なら既定 ON がそのまま点いて見える(初期値は常に埋まっている) */
  const rotationOnIds = $derived(
    (rotationOverride
      ?? character?.rotation_skill_ids
      ?? rotationCandidates.filter((c) => c.default_on).map((c) => c.skill_id)
    ).filter((id) => rotationCandidates.some((c) => c.skill_id === id)),
  );
  /** 押した瞬間に計算し直す(「適用」を挟まない)。並びは候補の並びのまま */
  function toggleRotationSkill(id: string, on: boolean) {
    const next = on ? [...rotationOnIds, id] : rotationOnIds.filter((x) => x !== id);
    rotationOverride = rotationCandidates.map((c) => c.skill_id).filter((x) => next.includes(x));
  }
  // キャラ・主軸(計算する技)・対象が変われば、一時の選択は保存値に戻す
  let lastRotationKey = untrack(() => `${character?.id}|${skillId}|${target?.content.id}`);
  $effect(() => {
    const key = `${character?.id}|${skillId}|${target?.content.id}`;
    if (key === lastRotationKey) return;
    lastRotationKey = key;
    rotationOverride = null;
  });

  // --- 一時調整 -------------------------------------------------------------
  // この画面に編集 UI は無い(「調整(一時)」カードは削除済み)。previewDamage 系コマンドは
  // Adjustments を必須パラメータとして取るため、中立値を渡す。
  const NEUTRAL_ADJUSTMENTS: Adjustments =
    Object.fromEntries(STAT_KINDS.map((k) => [k, { add: 0, pin: null }])) as Adjustments;
  // キャラを切り替えたらスキルの選び直しをリセット(前のキャラの選択を引き継がない)
  let lastCharacterId = untrack(() => character?.id);
  $effect(() => {
    const id = character?.id;
    if (id === lastCharacterId) return;
    lastCharacterId = id;
    skillOverride = null;
  });

  // --- 計算(payload と saved の両方) -------------------------------------
  // 戻りは本体(body)・熊(summon。無ければ null)・合計(combined)の 3 つ組(ADR-016)。
  // 画面はほぼ全部 body を読み、熊がいるキャラだけ summon / combined も読む
  let result = $state<CharacterDamageResult | null>(null);
  let savedResult = $state<CharacterDamageResult | null>(null);
  let calculating = $state(false);
  const body = $derived(result?.body ?? null);
  const summon = $derived(result?.summon ?? null);
  /** <フラグ>(技とは別枠のダメージ。イェフネン)。積んでいなければ null。
   *  合算(1 発の合計・DPS・討伐時間)は combined が持つので、画面は足し算をしない */
  const flag = $derived(result?.flag ?? null);
  /** 回し(連打する技 + 差し込む CT 技)。本体の鎖の DPS はこの回しで出している */
  const rotation = $derived(result?.rotation ?? null);
  const combined = $derived(result?.combined ?? null);
  /** 召喚獣(熊・精霊)が撃つスキル本体(結果 JSON は id しか持たないので skills 一覧から引く) */
  const summonSkillFull = $derived(skills.find((s) => s.id === summon?.skill_id) ?? null);
  /** 召喚獣が撃つスキルの表示名 */
  const summonSkillName = $derived(summonSkillFull?.name ?? "");
  /** 召喚獣の鎖のラベル(熊 / 精霊) */
  // 合計(本体 + 召喚獣)の内訳。本体は回しがあれば回しの DPS、召喚獣は陣で居ないぶんを引いた値
  // (Rust の combine_damage と同じ足し算。ここで計算し直さず、材料をそのまま見せる)
  const bodyShareDps = $derived(rotation?.expected_dps ?? body?.expected_dps ?? null);
  const summonAbsentShare = $derived(rotation?.summon_absent_share ?? 0);
  const summonShareDps = $derived(
    summon?.result.expected_dps == null ? null : summon.result.expected_dps * (1 - summonAbsentShare),
  );
  /** 回しの顔ぶれ(連打技 + 差し込む技)。合計の内訳の注記用 */
  const rotationSummary = $derived(
    rotation
      ? [rotation.filler?.skill_name, ...rotation.inserts.map((i) => i.skill_name)].filter(Boolean).join(" + ")
      : "",
  );
  const summonLabel = $derived(summonSkillFull?.attacker === "destruction_spirit" ? "精霊" : "熊");
  /** 召喚獣の鎖のバッジに置く絵。熊はルシベア専用スキル(anais_rucy_*)ならルシベア、他はミカベア
   *  (突き・ジャッジメントスピン等は両方の人形が撃つので、どちらの人形かは保存していない)。
   *  精霊はそのスキルの属性(雷/水/火)からアンフェル/グレシス/イグニーを選ぶ(陣は本体扱いで
   *  召喚欄に出ないので、精霊のスキルは必ず雷・水・火のいずれか) */
  const summonIconId = $derived(
    summonSkillFull?.attacker === "destruction_spirit"
      ? summonSkillFull.element === "water"
        ? "anais_gureshisu_summon"
        : summonSkillFull.element === "fire"
          ? "anais_igni_summon"
          : "anais_anferu_summon"
      : summon?.skill_id.startsWith("anais_rucy_")
        ? "anais_rucy_bear_summon"
        : "anais_mica_bear_summon",
  );
  /** 召喚獣の DPS 節に出す間隔の注記。中ディレイ未収録(interval_seconds が null)なら出さない */
  const summonIntervalNote = $derived(
    summon?.interval_seconds != null
      ? `${fmtNum(summon.interval_seconds, 2)}s 間隔(中ディレイ + 0.0705s)・コンボは乗りません`
      : null,
  );
  const requestLatest = latest({ debounce: 120 });
  $effect(() => {
    // sim のネスト変更も拾う。回しの一時選択(保存しない)もここで payload に載せる
    const pJson = payload ? JSON.stringify(withRotation(payload)) : null;
    const sp = savedPayload ? withRotation(savedPayload) : null;
    const t = target;
    const sid = skillId;
    const comboType = selectedComboSkillType;
    const normalId = comboNormalAttackId;
    const simActive = app.sim !== null;
    const tempJson = JSON.stringify(NEUTRAL_ADJUSTMENTS);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !sp || !t || !sid) {
      requestLatest.cancel();
      result = null;
      savedResult = null;
      return;
    }
    calculating = true;
    requestLatest.run(async (isCurrent) => {
      try {
        const main = await previewDamage(
          JSON.parse(pJson), sid, t.content.id, comboCount, JSON.parse(tempJson), comboType, JSON.parse(buffsJson),
          normalId,
        );
        const saved = simActive
          ? await previewDamage(
              sp, sid, t.content.id, comboCount, JSON.parse(tempJson), comboType, JSON.parse(buffsJson),
              normalId,
            )
          : main;
        if (isCurrent()) {
          result = main;
          savedResult = saved;
          // 差分枠(↑↓)は「今回の計算で動いた値」にだけ出す。動かなかった値では差分枠の
          // effect が走らないので、結果を入れたこの場で世代を進めて全部の枠に見直させる。
          // $effect からではなく代入と同じ場所で呼ぶ — 枠側の effect より先に確定させるため
          markRecalculated();
        }
      } catch (e) {
        if (isCurrent()) {
          result = null;
          reportError(errorMessage(e));
        }
      } finally {
        if (isCurrent()) calculating = false;
      }
    });
    return () => requestLatest.cancel();
  });

  // --- 入場条件・通るのは(payload 基準、Rust 側で判定) --------------------
  let evals = $state<ContentEvaluation[]>([]);
  const evalLatest = latest({ debounce: 200 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    // 計算タブは「今このスキルで戦う」文脈なので、装備条件も選択中スキルの依存で判定する
    // (ホームはコンテンツごとの最大ダメージスキルで判定する)。
    const sid = skillId;
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson) {
      evalLatest.cancel();
      evals = [];
      return;
    }
    evalLatest.run((isCurrent) => {
      evaluateContents(JSON.parse(pJson), sid || undefined, JSON.parse(buffsJson))
        .then((rs) => {
          if (isCurrent()) evals = rs;
        })
        .catch((e) => reportError(errorMessage(e)));
    });
    return () => evalLatest.cancel();
  });
  const targetEval = $derived(target ? (evals.find((e) => e.content_id === target.content.id) ?? null) : null);

  // --- 表示値 -------------------------------------------------------------
  // 主役の値の選び方(クリ発生率 > 0 ならクリティカル、0 なら非クリ最大)は Rust 側
  // (DamageTriple::primary)に一元化済み。ここは per_hit_primary / total_primary を読むだけ。
  // critMode はトレースの段・内訳表示の切替(表示都合)にだけ使う。
  const critMode = $derived((body?.critical_chance ?? 0) > 0);
  const pick = <T extends { max: number; critical: number }>(t: T | null | undefined): number | null =>
    t ? (critMode ? t.critical : t.max) : null;
  const perHit = $derived(body?.per_hit_primary ?? null);
  const savedPerHit = $derived(savedResult?.body.per_hit_primary ?? null);
  const deltaPct = $derived(
    perHit !== null && savedPerHit !== null && savedPerHit > 0
      ? Math.round((perHit / savedPerHit - 1) * 100)
      : 0,
  );
  // 討伐時間の目安。段の境目は Rust が配る(tables.reach_seconds)。画面は写経しない
  const closeSeconds = $derived(tables.reach_seconds.close);
  // 討伐時間・到達段は combined(本体 + 熊)から 1 か所で決める。熊が無いキャラは
  // combined が body と同じ値になる(Rust combine_damage のフォールバック)ので、
  // 分岐を画面側に持たない(ADR-016 決定 9・10)
  const defeatSeconds = $derived(combined?.defeat_seconds ?? null);
  /** メーター比。討伐時間が長いほど伸びる(0 = 一瞬、100% = 目安ぴったり) */
  const meterRatio = $derived(defeatSeconds !== null ? Math.min(1, defeatSeconds / closeSeconds) : 0);
  const hasReqs = $derived((target?.content.requirements.length ?? 0) > 0);
  // 評価が未取得の間は入場条件を「不明」として扱い、未達コンテンツに「通る/余裕」を
  // 出さない(ダメージ 120ms・評価 200ms のデバウンス差で毎回この窓が開く。PR レビュー指摘)
  const entryKnown = $derived(!hasReqs || targetEval !== null);
  const entryOk = $derived(!hasReqs || (targetEval?.entry_ok ?? false));
  /** 目安に対する到達段(Rust 側の判定。目安なしは null) */
  const reach = $derived(combined?.reach ?? null);
  const reached = $derived(reachOk(reach));
  const badgeState = $derived.by(() => {
    if (perHit === null || !entryKnown || reach === null) return 6;
    if (hasReqs && !entryOk) return reached ? 5 : 4;
    return REACH_STATE[reach];
  });
  // 言葉はこの画面のもの、色は 6 系統から選ぶ(design-system §03)。先頭 6 件は共通(ui/states.ts)
  const BADGE: Badge[] = [...REACH_BADGES, { label: "判定中", state: "unknown" }];

  // --- なぜこの数字?(トレースの式から組み立て) ---------------------------
  // 段の組み立ては calc/damageDetail.ts、面そのものは calc/WhyPanel.svelte(本体だけ。
  // ユーザーの強い指示で元デザインを変えない)。鎖の節ごとの内訳(1 発 / 合計 / DPS)は
  // calc/DamageChain.svelte が本体・熊それぞれの結果から組む(二重実装しない)。
  // ここに残すのは、「行ける?」の文・メーター・WhyPanel が共有する body 由来の値だけ。
  // 主役がクリティカル前提なら、トレースの段もクリティカル側の到達値で揃える。
  const steps = $derived(stepsOf(body, critMode));
  const atkA = $derived(body?.trace.attack?.value ?? null);
  const defenseValue = $derived(
    body?.trace.categories.find((c) => c.symbol === "C")?.value ?? null,
  );
  const pierced = $derived(stepValue(steps, "攻撃力−防御力"));
  const noPierce = $derived(pierced !== null && pierced <= 0);
  const flowRows = $derived(flowRowsOf(steps, pierced));

  // --- 数値を開いて詳細を確認する(§00 03: 開くのは押した行の下だけ) ----------
  // 開いている面・変わった行の控えは鎖と「なぜこの数字?」で共有する(calc/detailStore)。
  // 攻撃者ごとに 1 つずつ持つ(本体 = details / 熊 = summonDetails): 「変わった行」の控えは
  // 前回値との比較なので、本体と熊を 1 つの控えに混ぜると、見る鎖を切り替えただけで全行が
  // 「変わった」扱いになる。値はすべて Rust 由来(DamageTrace / DamageResult)。
  // UI で作るのは 2 値の差分だけ。
  const details = new DetailStore();
  const summonDetails = new DetailStore();
  /** 鎖の ↑ に下線を出すか(その下に変わった段があるか)。判定は WhyPanel と同じ関数。熊も同じ */
  const flowChanged = $derived(changedFlowKeys(details, flowRows, body).length > 0);
  const summonFlowChanged = $derived.by(() => {
    const r = summon?.result ?? null;
    if (!r) return false;
    const steps = stepsOf(r, r.critical_chance > 0);
    return changedFlowKeys(summonDetails, flowRowsOf(steps, stepValue(steps, "攻撃力−防御力")), r).length > 0;
  });
  /**
   * 「なぜこの数字?」がどちらの鎖を掘り下げているか。最後に節を押した鎖に付いていく
   * (召喚獣の鎖を見ているときだけ召喚獣の値・係数行を出す)。召喚獣が消えたら本体に戻る。
   * 面の開閉(flowOpen)は切り替えても引き継ぐ — 押した瞬間に面が閉じては困る(§00 ③)
   */
  let whyView = $state<"body" | "summon">("body");
  // 召喚獣が消えたら(召喚獣なしキャラへ切替・召喚スキル解除)本体に戻す。viewWhy を通さない —
  // 別キャラの summonDetails.flowOpen を details に持ち込まない(レビュー指摘 2026-09-18)
  $effect(() => {
    if (summon === null) whyView = "body";
  });
  const whySummon = $derived(whyView === "summon" && summon !== null);
  const whyStore = $derived(whySummon ? summonDetails : details);
  const whyResult = $derived(whySummon ? (summon?.result ?? null) : body);
  function viewWhy(view: "body" | "summon") {
    if (view === whyView) return;
    const from = view === "summon" ? details : summonDetails;
    const to = view === "summon" ? summonDetails : details;
    to.flowOpen = from.flowOpen;
    whyView = view;
  }

  // --- 攻撃 / 防御タブ(規格シート 5c) --------------------------------------
  let side = $state<"attack" | "defense">("attack");
  let defense = $state<DefenseProfile | null>(null);

  let defenseError = $state<string | null>(null);
  const defenseLatest = latest();
  $effect(() => {
    // 防御側は対象コンテンツに依らない。キャラ(試し変更込み)が変わったときだけ引き直す
    const p = payload;
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!p) {
      defense = null;
      return;
    }
    defenseLatest.run((isCurrent) =>
      previewDefense(p, JSON.parse(buffsJson))
        .then((d) => {
          if (isCurrent()) {
            defense = d;
            defenseError = null;
            markRecalculated("defense");
          }
        })
        .catch((e) => {
          if (isCurrent()) defenseError = errorMessage(e);
        }),
    );
  });

  // --- 試し変更(sim) ------------------------------------------------------
  // 何を 1 操作として戻すか(KNOBS)と上限は calc/simStore が持つ。右カラムの帯・印と
  // 「足りない分をどう埋める?」はこの 1 つを共有する。

  // --- もし〜だったら ------------------------------------------------------
  // 列挙・試算・並び順(+0 除外)は list_upgrade_candidates(Rust 側 domain::candidate)。
  // コンボ・一時調整は「この一発」表示と同条件(現在のコンボ・一時調整)で試算する。
  let whatIf = $state<UpgradeCandidate[]>([]);
  /** 押した候補は、移動先の差分チップと同時に短く退出させる(§10「移った」)。 */
  let leavingWhatIfId = $state<string | null>(null);
  /** 「足りない分をどう埋める?」1 行の 2 位以降。押した行の直下に開く(§00 03) */
  let fillMoreOpen = $state(false);
  const whatIfLatest = latest({ debounce: 250 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    const t = target;
    const sid = skillId;
    const base = perHit;
    const comboType = selectedComboSkillType;
    const tempJson = JSON.stringify(NEUTRAL_ADJUSTMENTS);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !t || !sid || base === null) {
      whatIfLatest.cancel();
      whatIf = [];
      return;
    }
    whatIfLatest.run(async (isCurrent) => {
      try {
        const current = JSON.parse(pJson) as NewCharacter;
        const rs = await listUpgradeCandidates(
          current, sid, t.content.id, comboCount, comboType, JSON.parse(tempJson), JSON.parse(buffsJson),
        );
        if (isCurrent()) {
          whatIf = rs;
          leavingWhatIfId = null;
        }
      } catch (e) {
        if (isCurrent()) reportError(errorMessage(e));
      }
    });
    return () => whatIfLatest.cancel();
  });
  function applyWhatIf(w: UpgradeCandidate) {
    leavingWhatIfId = w.id;
    sim.applyCandidate(w.applied);
  }

</script>

<SplitPage
  midTitle="行ける？"
  midNote="→ なぜこの数字？"
  rightTitle="計算の材料"
  rightNote={character?.name ?? ""}
  persistKey="tw-v4-calc"
  defaultRight={DEFAULT_RIGHT_WIDTH}
  minMid={320}
  minRight={280}
  splitterLabel="計算シートと材料の境界"
  rightScrollStyle="padding: 11px;"
>
  {#snippet mid()}
      {#if !character}
        <p class="empty dim">キャラを登録するとダメージ計算ができます。</p>
      {:else if !target}
        <p class="empty dim">コンテンツデータがありません。</p>
      {:else}
        <!-- 攻撃 / 防御(同列タブ) -->
        <Choose
          label="攻撃 / 防御"
          class="chiprow side-tabs"
          options={[{ value: "attack", label: "攻撃" }, { value: "defense", label: "防御" }]}
          bind:value={() => side, (v) => (side = v as "attack" | "defense")}
        />
      {/if}
      {#if !character || !target}
        <!-- 上のブロックで案内済み -->
      {:else if side === "defense"}
        <!-- 攻撃 / 防御 は面ごと入れ替わる。入ってくる面を短く動かす(§10 型 3b) -->
        <div class="swap-in"><DefensePanel profile={defense} error={defenseError} /></div>
      {:else}
        <!-- 攻撃側は「行ける?」カード・なぜこの数字?・トレースで 1 枚の面。**入場クラスは面の
             いちばん外に 1 つ**掛ける(§00 04「同時に変わるものは全部動かす」)。カードだけに
             掛けていたときは、防御 → 攻撃 に戻すと 1,500 の文字が無音で出ていた(実機 2026-09-17) -->
        <div class="swap-in">
        <!-- 行ける?カード -->
        <SheetCard tone="gold" title="行ける？" note={character.name} busy={calculating}>
          <!-- 対象プレート -->
          <div class="target-row">
            <button type="button" class="step" onclick={() => stepTarget(-1)}>◀</button>
            <Popover label="計算する対象" triggerClass="target-trigger" panelClass="target-pop">
              {#snippet trigger(open)}
              <span class="t-line1">
                <Icon kind="content" id={target.content.id} fallback={{ kind: "mob", id: target.content.enemy_id }} size={28} label={target.content.name} />
                <Value class="t-name" value={target.content.name} />
                <span class="caret" class:rot={open}>▼</span>
                <Value class="t-index dim" motion={() => targetIndex + 1} value={`${targetIndex + 1} / ${contents.length}`} />
              </span>
              <span class="t-line2">
                <Value class="t-area dim" value={target.areaName} />
                <Value class="t-def" motion={() => defenseValue} value={defenseValue !== null ? `防御 ${fmtInt(defenseValue)}` : "防御 —"} />
                <Value class="t-need" motion={() => closeSeconds} value={`目安 ${fmtDuration(closeSeconds)}以内`} />
              </span>
              {/snippet}
              {#snippet children(close)}
              {#each targetAreas as area (area.id)}
                <div class="pop-head"><span class="pop-diamond"></span><span>{area.name}</span><span class="num dim">{area.contents.length} 件</span></div>
                {#each area.contents as c (c.id)}
                  {@const ev = evals.find((e) => e.content_id === c.id)}
                  <!-- 収録度は行頭に 1 つだけ(§14 決定 5)。分かっている行には出さない -->
                  {@const cov = !ev ? "判定中" : c.enemy_id === null ? "敵データなし" : !ev.damage ? "スキル未収録" : null}
                  <button
                    type="button"
                    class="pop-row"
                    class:on={c.id === target.content.id}
                    onclick={() => {
                      app.calcTargetId = c.id;
                      close();
                    }}
                  >
                    <span class="dot" style="background: {ev?.clear ? STATE.met.bd : ev?.entry_ok === false ? STATE.short.bd : STATE.unknown.bd};"></span>
                    <!-- コンテンツの絵が無ければそのコンテンツの敵の絵。サイズ固定なので行の高さは動かない -->
                    <Icon kind="content" id={c.id} fallback={{ kind: "mob", id: c.enemy_id }} size={20} label={c.name} />
                    {#if cov !== null}<span class="badge unknown">{cov}</span>{/if}
                    <span class="pop-name">{c.name}</span>
                    <span class="num dim">{ev?.damage ? fmtInt(ev.damage.per_hit_primary) : "—"}</span>
                  </button>
                {/each}
              {/each}
              {/snippet}
            </Popover>
            <button type="button" class="step" onclick={() => stepTarget(1)}>▶</button>
          </div>

          <!-- スキル行 -->
          <div class="skill-row">
            {#if bodySkills.length === 0}
              <span class="dim">このキャラのスキルデータは未収録です(仮スキルはありません)。</span>
            {:else}
              <Popover
                label="計算するスキル"
                triggerClass="skill-trigger"
                panelClass="skill-pop"
                disabled={bodySkills.length <= 1}
                onToggle={(open) => (skillOpen = open)}
              >
                {#snippet trigger(open)}
                <span class="sk-line1">
                  <Icon kind="skill" id={skill?.id ?? null} size={20} label={skill?.name ?? "スキル"} />
                  <span class="sk-name">{skill?.name ?? ""}</span>
                  {#if bodySkills.length > 1}<span class="caret" class:rot={open}>▼</span>{/if}
                  <!-- 主軸(キャラタブ)と違うスキルで計算している例外状態。保存されないので
                       ラベンダー(--sim)。行の高さは変えない -->
                  {#if skillOverridden}
                    <span class="sk-override badge-in" use:changed={() => skillId}>ここで上書き中</span>
                  {/if}
                </span>
                <span class="sk-meta num dim">
                  ×<Value value={body ? fmtNum(body.effective_skill_multiplier) : "—"} />
                  ・ <Value value={body ? String(body.hit_count) : "—"} />段
                  ・ 中 <Value value={body?.effective_base_actual_delay != null ? `${fmtNum(body.effective_base_actual_delay)}s` : "?"} />
                  ・ Cri×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                  {#if skill}・ {ELEMENT_LABELS[skill.element]}属性{/if}
                  {#if body?.accuracy_point != null}・ 命中P {fmtInt(body.accuracy_point)}{/if}
                </span>
                {/snippet}
                {#snippet children(close)}
                <div class="pop-head gold"><span>スキル {bodySkills.length} 種 ／ この対象への合計ダメージ順</span></div>
                {#each pickerSkills as s (s.id)}
                  {@const d = skillTotals[s.id]}
                  <button
                    type="button"
                    class="pop-row"
                    class:on={s.id === skillId}
                    onclick={() => {
                      skillOverride = s.id;
                      close();
                    }}
                  >
                    <Icon kind="skill" id={s.id} size={20} label={s.name} />
                    <span class="pop-name">{s.name}</span>
                    <span class="num dim">×{fmtNum(s.multiplier)} / {s.hit_count}段</span>
                    <span class="num strong">{d ? fmtInt(d.total) : "…"}</span>
                  </button>
                {/each}
                {/snippet}
              </Popover>
              <!-- 1 クリックで主軸に戻す。ボタン in ボタンにできないので行の中の兄弟に置く -->
              {#if skillOverridden}
                <button
                  type="button" class="sk-reset badge-in"
                  title={mainSkill ? `主軸スキル「${mainSkill.name}」に戻す` : ""}
                  onclick={() => (skillOverride = null)}
                >主軸に戻す</button>
              {/if}
            {/if}
          </div>

          {#if skill && skill.combo_variants.length > 0}
            <div class="combo-type-row inset">
              <div class="field">
                <span class="field-label">コンボタイプ</span>
                <Choose
                  label="コンボタイプ"
                  options={COMBO_SKILL_TYPE_OPTIONS}
                  full
                  bind:value={
                    () => comboSkillType,
                    (v) => (comboSkillType = v as ComboSkillType)
                  }
                />
              </div>
              <span class="combo-type-note dim">
                {comboSkillType === "chain"
                  ? "シエナのオーラの中ディレイ減少に応じて倍率・段数も変わります"
                  : "タイプを押すと倍率・段数・中ディレイへすぐ反映します"}
              </span>
            </div>
          {/if}

          <!-- この一発 -->
          <div class="hero">
            <!-- 鎖(§14 決定 1)。1 発は**ゲート**(防御を抜けるか・目安を超えるかの閾値判定)、
                 DPS は**レート**(どれくらいの速さで削れるか)で、種類の違う量。ゲートを通らな
                 ければレートに意味が無いので、軸を切り替えず因果の順に繋ぐ。
                 判定(バッジ)はゲートの位置だけに置き、レートには付けない — 「何秒までなら
                 合格」の基準がゲーム側に存在しないので、付けたら嘘になる。
                 召喚獣(熊・破壊精霊)が撃つスキルがあるキャラは、本体の鎖の下に召喚獣の鎖を
                 もう 1 本足す(ADR-016)。2 本の鎖は同じ寸法・同じ列位置で描く(本体だけ大きく
                 しない。ユーザー判断 2026-09-18 — 答えは下の合計面と「行ける?」帯が持つ)。
                 討伐時間は combined(下の「合計」面)で 1 か所に決めるので、召喚獣がいるときは
                 どちらの鎖にも討伐時間節を出さない(§00 ②)。 -->
            {#if body}
              <DamageChain
                result={body} {skill} store={details}
                attackerLabel="本体" attackerSkillName={skill?.name ?? ""}
                icon={{ kind: "character", id: character?.game_character_id ?? null, source: character ? (app.characterIcons[character.id] ?? null) : null }}
                showDefeat={!summon} heroNumber={true}
                onView={() => viewWhy("body")}
                onPerHitDeltaFollow={() => { viewWhy("body"); details.follow("perHit"); }}
                {flowChanged}
                {flag}
                {rotation}
                combined={summon ? null : combined}
              />
              <!-- 回しの段(「DPS(回し)」の下に常設)。差し込みをその場で試せて、時間と DPS の
                   取り分が技ごとに読める。数は Rust の回しが技ごとに返したものをそのまま出す -->
              <RotationPane
                {rotation} choices={rotationChoices} onIds={rotationOnIds} skills={bodySkills}
                onToggle={toggleRotationSkill}
                overridden={rotationOverride !== null}
                onReset={() => (rotationOverride = null)}
              />
            {/if}
            {#if summon}
              <DamageChain
                result={summon.result} skill={null} store={summonDetails}
                attackerLabel={summonLabel} isSummon={true} attackerSkillName={summonSkillName}
                icon={{ kind: "skill", id: summonIconId }}
                showDefeat={false} heroNumber={true}
                intervalNote={summonIntervalNote}
                onView={() => viewWhy("summon")}
                onPerHitDeltaFollow={() => { viewWhy("summon"); summonDetails.follow("perHit"); }}
                flowChanged={summonFlowChanged}
              />
              <!-- 合計(本体 + 召喚獣)。討伐時間はここだけに出す(§00 ②。ADR-016 決定 5・10)。
                   読み取り専用の値 2 つなので ReadRow の面(§08: インセット + ラベル + 右端固定幅の値)。
                   主役の数字(44px の金の帯 = 答えは 1 つ)と競わせないため、本文の大きさに留める。
                   討伐時間そのものの判定(バッジ・メーター・文)は下の「行ける?」帯が持つ -->
              <div class="combined readrows inset">
                <span class="combined-title">合計(本体 + {summonLabel})</span>
                <!-- 合計の内訳。本体は回しがあればその DPS(主軸単独ではない)、召喚獣は陣で消えている
                     ぶんを引いた値。合計だけ出すと「本体の鎖の数字 + 召喚獣の鎖の数字」に見えて
                     合わないので(実機 2026-09-23)、足し算の 2 項をそのまま行にする -->
                <ReadRow
                  label="本体"
                  value={bodyShareDps != null ? fmtInt(Math.round(bodyShareDps)) : "—"}
                  motion={() => (bodyShareDps == null ? null : Math.round(bodyShareDps))}
                  delta={{}}
                >
                  {#snippet note()}{rotation ? `スキル回し(${rotationSummary})` : `${skill?.name ?? "主軸"}を撃ち続けた値`}{/snippet}
                </ReadRow>
                <ReadRow
                  label={summonLabel}
                  value={summonShareDps != null ? fmtInt(Math.round(summonShareDps)) : "—"}
                  motion={() => (summonShareDps == null ? null : Math.round(summonShareDps))}
                  delta={{}}
                >
                  {#snippet note()}{#if summonAbsentShare > 0 && summon?.result.expected_dps != null}{fmtInt(Math.round(summon.result.expected_dps))} × (1 − {fmtPct(summonAbsentShare, 1)})。陣を置くと消え、呼び直すまで居ない{:else}召喚中も本体の手は止まらないので、そのまま足す{/if}{/snippet}
                </ReadRow>
                <ReadRow
                  label="合計 DPS"
                  value={combined?.expected_dps != null ? fmtInt(Math.round(combined.expected_dps)) : "—"}
                  motion={() => (combined?.expected_dps == null ? null : Math.round(combined.expected_dps))}
                  delta={{}}
                >
                  {#snippet note()}本体 + {summonLabel}{/snippet}
                </ReadRow>
                <ReadRow
                  label="討伐時間"
                  value={combined?.defeat_seconds != null ? fmtDuration(combined.defeat_seconds) : "—"}
                  motion={() => (combined?.defeat_seconds == null ? null : Math.round(combined.defeat_seconds))}
                  delta={{ unit: "秒", digits: 0 }} deltaClass="less-is-better"
                >
                  {#snippet note()}敵 HP ÷ 合計 DPS(ソロ){/snippet}
                </ReadRow>
              </div>
            {/if}
            <!-- 討伐時間が出せない(敵 HP か中ディレイが未収録)ときはメーターも文言も出さない
                 (§00 02。0 や「届かない」で埋めると嘘になる)。計算中・防御力を抜けていない
                 は討伐時間の有無に関わらず伝えるべき事実なので、その 2 つだけは別枠で出す -->
            {#if perHit === null}
              <div class="meter big"><div class="fill" style="width: 0%; background: {STATE.unknown.bar};"></div></div>
              <div class="hero-sentence"><span class="sentence">計算中…</span></div>
            {:else if noPierce}
              <div class="meter big"><div class="fill" style="width: 0%; background: {STATE.short.bar};"></div></div>
              <div class="hero-sentence">
                <span class="sentence ng">防御力を抜けていません(攻撃力 {atkA !== null ? fmtInt(atkA) : "—"} ≤ 防御力 {defenseValue !== null ? fmtInt(defenseValue) : "—"})</span>
              </div>
            {:else if defeatSeconds !== null && reach !== null}
              <div class="meter big"><div class="fill" style="width: {meterRatio * 100}%; background: {STATE[BADGE[badgeState].state].bar};"></div></div>
              <div class="hero-sentence">
                <span class="sentence" class:ok={reached} class:ng={!reached}>
                  {#if reach === "comfortable"}
                    {fmtDuration(tables.reach_seconds.comfortable)}かからずに倒せます。
                  {:else if reach === "reached"}
                    {fmtDuration(defeatSeconds)} で倒せます。
                  {:else if reach === "close"}
                    {fmtDuration(defeatSeconds)}。{fmtDuration(closeSeconds)}の目安ぎりぎりです。
                  {:else}
                    {fmtDuration(defeatSeconds)}。{fmtDuration(closeSeconds)}を超えるので厳しいです。
                  {/if}
                </span>
                <span class="num dim">目安 {fmtDuration(closeSeconds)}以内</span>
              </div>
            {/if}
            <!-- 足りない分をどう埋める? を 1 行に(旧: 紫のパネル)。候補が無い・すでに目安に
                 届いているときは行ごと消す(§00 02) -->
            {#if perHit !== null && !reached && whatIf.length > 0}
              {@const top = whatIf[0]}
              <div class="fill-line">
                <button
                  type="button" class="fill-btn"
                  class:whatif-leaving={leavingWhatIfId === top.id}
                  disabled={leavingWhatIfId === top.id}
                  onclick={() => applyWhatIf(top)}
                >
                  <span class="dim">→ 一番効くのは</span>
                  <span class="fill-label">{top.label}</span>
                  <span class="num fill-pct" class:flat={top.delta_pct === 0}>表記 {deltaText(top.delta_pct)}</span>
                  <span class="num fill-total dim">合計 {deltaText(top.delta_total_pct)}</span>
                </button>
                {#if whatIf.length > 1}
                  <!-- トリガは上の行の中、一覧は行の下。<details> を行に溶かして(display: contents)
                       両方を .fill-line の直接の子にする -->
                  <Disclosure class="fill-fold" summaryClass="fill-more-toggle" bind:open={fillMoreOpen}>
                    {#snippet summary(open)}{open ? "閉じる" : `他 ${whatIf.length - 1} 件`}{/snippet}
                    {#snippet children(open)}
                    {#if open}
                <div class="fill-list inset">
                  {#each whatIf.slice(1) as w (w.id)}
                    <button
                      type="button" class="fill-more-row"
                      class:whatif-leaving={leavingWhatIfId === w.id}
                      disabled={leavingWhatIfId === w.id}
                      onclick={() => applyWhatIf(w)}
                    >
                      <span class="fill-more-label">{w.label}</span>
                      <span class="num">表記 {deltaText(w.delta_pct)}</span>
                      <span class="num fill-total dim">合計 {deltaText(w.delta_total_pct)}</span>
                    </button>
                  {/each}
                </div>
                    {/if}
                    {/snippet}
                  </Disclosure>
                {/if}
              </div>
            {/if}
            {#if body?.actual_delay}
              {@const d = body.actual_delay}
              <div class="delay-note dim">
                中ディレイ {fmtNum(d.base, 2, "s")}
                {#if d.fixed}
                  ×(固定・減少が効かない)
                {:else if d.reduction > 0}
                  × (1 − {fmtPct(d.reduction)}){#if d.reduction_raw > d.reduction}<span class="warn"> ※減少値は上限 {fmtPct(limits.actual_delay_reduction_max)}({fmtPct(d.reduction_raw)} ぶん選択中)</span>{/if}
                {/if}
                {#if d.combo_rate < 1}× {fmtNum(d.combo_rate)}(コンボ){/if}
                <!-- チャージは中ディレイ減少も倍率A も下限も受けず、下限を取ったあとに足す。
                     式が表示中の値に到達するように、足している間は段を出す -->
                {#if d.charge > 0}
                  ＋ チャージ <Value motion={() => d.charge} value={fmtNum(d.charge, 2, "s")} />
                {/if}
                = <Value motion={() => d.value} value={fmtNum(d.value, 2, "s")} />{#if d.floored}<span class="warn"> ※下限 {fmtNum(limits.actual_delay_min, 1, "s")}</span>{/if}
                {#if d.contributions.length > 0}
                  ／ 減少源: {d.contributions.map((c) => `${c.source} ${fmtPct(c.rate)}`).join(" ・ ")}
                {/if}
                <br />
                {#if body?.combo}
                  {@const c = body.combo}
                  1 サイクル = 通常攻撃 {fmtNum(c.normal_delay, 2, "s")} + max(スキル {fmtNum(c.skill_delay, 2, "s")},
                  CI {c.interval !== null ? fmtNum(c.interval, 2, "s") : "?"}) = {fmtNum(c.seconds, 2, "s")}
                  ／ 1 秒あたり = (スキル + {c.normal_attack_name})の合計 ÷ 1 サイクル
                {:else if rotation}
                  <!-- 回しがあるときの DPS は「この回数で連打」ではない。上の回しの段と食い違う式を出さない -->
                  連打し続けた場合は {Math.round(d.uses_per_minute)} 回/分。DPS は上の「スキル回し」(連打 + 差し込み)の配分で出しています
                {:else}
                  1 秒あたり = 合計 × {Math.round(d.uses_per_minute)} 回/分 ÷ 60
                  {#if d.uses_measured}
                    (<b>実測表</b>: 総減少 {fmtPct(d.reduction)} × 基本 {fmtNum(d.base, 2, "s")})
                  {:else}
                    (実測表の範囲外なので 60 ÷ 中ディレイ の式で算出)
                  {/if}
                {/if}
              </div>
            {/if}
            {#if body?.critical_rate}
              {@const c = body.critical_rate}
              <div class="delay-note dim">
                クリティカル率 (装備クリ補正 {fmtInt(c.equipment_critical)} + 1) × 2 × (AGI {fmtInt(c.agi)} / (AGI + 対象AGI {fmtInt(c.target_agi)}))
                {#if c.siena_rate > 0}× シエナのオーラ {fmtNum(1 + c.siena_rate, 2)}{/if}
                = {fmtNum(c.from_agi, 1, "%")}
                ＋ スキル Cri値 {fmtInt(c.skill)}%{#if c.bonus > 0} ＋ 増加 {fmtInt(c.bonus)}%{/if}
                − 対象のクリティカル被撃率 {fmtInt(-c.target_taken_rate)}%
                = <b>{fmtNum(c.value, 1, "%")}</b>{#if c.raw < 0}<span class="warn"> ※下限 0%</span>{:else if c.raw > 100}<span class="warn"> ※上限 100%</span>{/if}
              </div>
            {:else if body && skill}
              <div class="delay-note dim">
                クリティカル率は出せません(この敵の AGI / クリティカル被撃率、またはスキルの Cri値が wiki 未記載)。
              </div>
            {/if}
            {#if body && body.effective_base_actual_delay === null}
              <div class="delay-note dim">このスキルは wiki に基本中ディレイ(「動作」列)が無いため、1 秒あたりの火力を出せません。</div>
            {/if}
          </div>
        </SheetCard>

        <!-- なぜこの数字?(元デザインは変えない)。見出しの 本体 / 召喚獣(熊 or 精霊)の 2 択、
             または最後に節を押した鎖に付いて、その攻撃者の値を掘り下げる。熊のときは
             「係数 STAB(熊)」の行が ① の掘り下げに 1 行増えるだけ、精霊は本体と同じ行のまま
             (ADR-016 突き合わせ) -->
        <WhyPanel
          result={whyResult} {defense} perHit={whyResult?.per_hit_primary ?? null}
          critMode={(whyResult?.critical_chance ?? 0) > 0} store={whyStore}
          attacker={whySummon ? (summonSkillFull?.attacker ?? "magic_doll") : "player"}
          summonAttacker={summon ? (summonSkillFull?.attacker ?? "magic_doll") : null}
          onAttacker={summon ? (a) => viewWhy(a === "player" ? "body" : "summon") : undefined}
        />
        </div>
      {/if}
  {/snippet}
  {#snippet right()}
      {#if character && payload}
        <MaterialsPane
          {payload} characterId={character?.id} {skill} {skillId} targetId={target?.content.id ?? null}
          result={body} {deltaPct} {comboCount} comboType={selectedComboSkillType}
          adjustments={NEUTRAL_ADJUSTMENTS} {sim} {normalAttackId} {normalAttackOptions}
          bind:combo bind:normalAttackOverride
        />
      {:else}
        <p class="empty dim">キャラを選択してください。</p>
      {/if}
  {/snippet}
</SplitPage>

<style>
  /* .layout / section / .scroll は ui/SplitPage.svelte(padding の差は rightScrollStyle で指定) */
  .empty { font-size: 12px; }

  /* 行ける?カード。.sheet-card/.sheet-head/.gem/.sheet-title/.sheet-char は ui/SheetCard.svelte */

  .target-row { position: relative; z-index: 3; display: flex; align-items: center; gap: 8px; padding: 10px 11px 0; background: linear-gradient(180deg, #F4F9FE, #fff); }
  .step {
    flex-shrink: 0; width: 25px; height: 25px; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); background: linear-gradient(180deg, #fff, #E9F1FB); border: 1px solid #9FB4D0;
    font-size: 9px; font-weight: 700; color: var(--fg-sub);
  }
  .step:hover { background: var(--bg-active); }
  /* トリガは ui/Popover.svelte が描くので、祖先経由でスコープ付き CSS を届かせる */
  .target-row :global(.target-trigger) { min-width: 0; flex: 1; padding: 3px 8px; border-radius: var(--r-panel); border: 1px solid transparent; text-align: left; }
  .target-row :global(.target-trigger:hover), .target-row :global(.target-trigger[aria-expanded="true"]) { background: var(--bg-rail); border-color: #9FB4D0; }
  .t-line1 { display: flex; align-items: center; gap: 6px; min-width: 0; }
  /* 対象プレートの 5 つは ui/Value.svelte が描く(子コンポーネントの要素なので :global) */
  .target-row :global(.t-name) { min-width: 0; font-size: 15px; font-weight: 800; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .target-row :global(.t-index) { flex-shrink: 0; margin-left: auto; font-size: 8.5px; }
  .t-line2 { margin-top: 1px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .target-row :global(.t-area) { min-width: 0; font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .target-row :global(.t-def) { flex-shrink: 0; font-size: 8.5px; color: var(--danger); }
  .target-row :global(.t-need) { flex-shrink: 0; font-size: 8.5px; color: var(--fg-sub); }

  /* 対象・スキルの候補面。重なり方と閉じ方は ui/Popover.svelte ＋ app.css の .popover が持つので、
     ここは行を端まで使うための余白なしと幅・高さだけ。トップレイヤに乗るので祖先が無く、:global で書く */
  :global(.target-pop), :global(.skill-pop) {
    gap: 0; padding: 0; max-height: 262px; border-radius: var(--r-window);
    min-width: anchor-size(width); font-size: var(--t-body);
  }
  :global(.skill-pop) { border-color: #A9821F; }
  .pop-head {
    position: sticky; top: 0; z-index: 1; display: flex; align-items: center; gap: 7px;
    padding: 6px 13px 6px 11px;
    background: linear-gradient(180deg, #DBE6F8, #C6D8F0); border-bottom: 1px solid var(--border);
    font-size: 9.5px; font-weight: 800; letter-spacing: 0.1em; color: var(--fg-head);
  }
  .pop-head.gold { background: linear-gradient(180deg, #F2E3BD, #DCC27E); border-bottom-color: #BFA155; color: #4A3C12; }
  .pop-head .num { margin-left: auto; font-weight: 400; }
  .pop-diamond { width: 6px; height: 6px; transform: rotate(45deg); background: var(--head-bar); }
  .pop-row {
    width: 100%; display: flex; align-items: center; gap: 9px; padding: 7px 13px 7px 11px;
    border-bottom: 1px solid #EDF2F9; text-align: left;
  }
  .pop-row:hover { background: #F1F7FE; }
  .pop-row.on { background: var(--sel-card); }
  .pop-row .dot { width: 7px; height: 7px; flex-shrink: 0; border-radius: 50%; }
  .pop-name { min-width: 0; flex: 1; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pop-row.on .pop-name { font-weight: 700; }
  .pop-row .strong { font-weight: 700; }

  .skill-row { position: relative; z-index: 2; padding: 8px 11px 0; display: flex; align-items: center; gap: 7px; }
  /* トリガは ui/Popover.svelte が描く。スキルが 1 つのときは開かない(disabled)が、
     行としては同じ面なので薄くしない */
  .skill-row :global(.skill-trigger) {
    min-width: 0; flex: 1; display: flex; flex-direction: column; align-items: stretch; gap: 1px;
    padding: 5px 9px; border-radius: var(--r-panel); background: #F4F9FE; border: 1px solid #D6E2F0; text-align: left;
  }
  .skill-row :global(.skill-trigger:hover:not(:disabled)) { background: var(--bg-rail); }
  .skill-row :global(.skill-trigger:disabled) { opacity: 1; cursor: default; }
  .sk-line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .sk-name { min-width: 0; flex: 1; font-size: 11.5px; font-weight: 700; color: #3E2B26; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sk-meta { font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* 主軸を上書き中の印と戻し先。保存されない状態なのでラベンダー(--sim)。
     どちらも行の高さを変えない小物で、主軸どおりのときは出ない(§00 02) */
  .sk-override {
    flex-shrink: 0; padding: 0 6px; border-radius: var(--r-pill);
    background: var(--state-temp-bg); border: 1px solid var(--sim); color: var(--sim-fg);
    font-size: 8.5px; font-weight: 700; white-space: nowrap;
  }
  .sk-reset {
    flex-shrink: 0; padding: 4px 9px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--sim); color: var(--sim-fg);
    font-size: 9.5px; font-weight: 700; white-space: nowrap;
  }
  .sk-reset:hover { background: var(--state-temp-bg); }
  .combo-type-row {
    margin: 8px 11px 0; padding: 8px 10px;
    display: grid; grid-template-columns: minmax(220px, 320px) minmax(0, 1fr); align-items: end; gap: 10px;
  }
  .combo-type-note { min-width: 0; padding-bottom: 3px; font-size: 9px; line-height: 1.45; }
  @media (max-width: 720px) {
    .combo-type-row { grid-template-columns: minmax(0, 1fr); align-items: stretch; }
  }

  .hero { padding: 11px 13px 12px; }
  /* .hero-num は ui/Value.svelte が描く(子コンポーネントの要素なのでスコープ付き CSS が届かない) */
  .hero :global(.hero-num) { font-size: var(--t-result); line-height: 1; font-weight: var(--w-strong); }
  .meter.big { margin-top: 10px; height: 12px; border-radius: var(--r-inset); }
  .hero-sentence { margin-top: 7px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .sentence { min-width: 0; flex: 1; font-size: 11px; font-weight: 700; text-wrap: pretty; }
  .sentence.ok { color: var(--good); }
  .sentence.ng { color: var(--danger); }
  .hero-sentence .num { flex-shrink: 0; font-size: 9.5px; }
  /* 鎖(.chain 一式)は calc/DamageChain.svelte が持つ(攻撃者ごとに 2 回描くため子コンポーネント化。ADR-016)。
     2 本目(熊)が続くときの区切りだけはここで足す — DamageChain 単体では
     「自分の隣に自分と同じ要素があるか」を知らない(:global は実 DOM の隣接関係を見る)。
     本体の鎖の下には回しの段(.rotation)が挟まるので、その次に来る鎖にも同じ区切りを付ける
     (無いと 本体→回し は 8px、回し→精霊 は 4px で詰まって見えた。実機 2026-09-23) */
  :global(.hero .chain-block + .chain-block),
  :global(.hero .rotation + .chain-block) {
    margin-top: 10px; padding-top: 10px; border-top: 1px dashed var(--border-soft);
  }
  /* 合計(本体 + 熊)の面。ReadRow の面(.readrows.inset)に見出し 1 行を足しただけ(ホームの
     「いまの実力」パネルと同じ形)。上端のハイライト 1 本で「平たい箱」にしない(§08) */
  .combined { margin-top: 8px; padding-top: 6px; padding-bottom: 6px; box-shadow: inset 0 1px 0 #fff; }
  .combined-title { padding: 0 0 3px; font-size: 8.5px; font-weight: 700; letter-spacing: 0.1em; color: var(--fg-muted); }
  .delay-note { margin-top: 6px; font-size: 9px; line-height: 1.5; }
  .delay-note .warn { color: var(--danger, #B5443A); }

  /* 足りない分をどう埋める? を 1 行に(旧: 紫のパネル)。押した場所は動かない(§00 03) */
  .fill-line { margin-top: 8px; display: flex; flex-wrap: wrap; align-items: baseline; gap: 8px; min-width: 0; }
  .fill-btn {
    min-width: 0; flex: 1; display: flex; align-items: baseline; gap: 6px; padding: 3px 6px;
    border-radius: var(--r-inset); text-align: left; font-size: 10.5px;
  }
  .fill-btn:hover:not(:disabled) { background: var(--state-temp-bg); }
  .fill-label { min-width: 0; flex-shrink: 1; font-weight: 700; color: var(--sim-fg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fill-pct { flex-shrink: 0; font-weight: 700; color: var(--good); }
  /* ±0% は「増えた」ではない。色で増減を言っているので、動かないときは色も外す */
  .fill-pct.flat { color: var(--fg-dim); }
  /* 合計ダメージ側の伸び率。桁が変わっても表記側の位置が動かないよう幅を固定する */
  .fill-total { flex-shrink: 0; min-width: 74px; text-align: right; font-size: 9.5px; font-weight: 700; }
  /* 段に溶かした <details> の本文は ::details-content という箱を 1 枚挟む(display: block)。
     行の flex item はその箱なので、全幅に落とすのは箱のほう(実測 2026-09-17) */
  .fill-line :global(details.fill-fold) { display: contents; }
  .fill-line :global(details.fill-fold::details-content) { flex-basis: 100%; }
  .fill-line :global(summary.fill-more-toggle) { flex-shrink: 0; display: flex; align-items: center; gap: 3px; padding: 2px 6px; border-radius: var(--r-inset); font-size: 9px; color: var(--fg-muted); }
  .fill-line :global(summary.fill-more-toggle:hover) { color: var(--fg); background: var(--bg-active); }
  .fill-list {
    margin-top: 4px; padding: 5px 7px; display: flex; flex-direction: column; gap: 2px;
  }
  .fill-more-row { width: 100%; display: flex; align-items: center; gap: 8px; padding: 2px 3px; border-radius: var(--r-inset); text-align: left; }
  .fill-more-row:hover:not(:disabled) { background: var(--bg-active); }
  .fill-more-label { min-width: 0; flex: 1; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* 攻撃 / 防御。粒の並び(`.chips`)の上に、この画面だけの大きさと地を重ねる */
  :global(.side-tabs) { gap: 6px; margin-bottom: 9px; }
  :global(.side-tabs > .chip) {
    padding: 6px 18px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #E9F1FB); border: 1px solid var(--border-strong);
    font-size: 11.5px; font-weight: 700; color: #2B3C57;
  }
  :global(.side-tabs > .chip.on) {
    border-radius: var(--r-panel);
    background: var(--sel-card); border-color: var(--accent); color: var(--sel-fg);
    box-shadow: inset 0 1px 0 #fff;
  }


</style>
