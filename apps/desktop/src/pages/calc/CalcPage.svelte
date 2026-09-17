<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listSkills, listUpgradeCandidates, previewDamage, previewDefense,
  } from "../../api/commands";
  import type {
    Adjustments, ComboSkillType, ContentEvaluation, DamageResult, DefenseProfile, NewCharacter, Skill,
    UpgradeCandidate,
  } from "../../api/types";
  import { fmtDuration, fmtInt, fmtNum, fmtPct, fmtRate, fmtSigned, fmtSignedPct } from "../../format";
  import { ELEMENT_LABELS, STAT_KINDS } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { tables } from "../../tables.svelte";
  import { app, flatContents } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Choose from "../../ui/Choose.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Popover from "../../ui/Popover.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";
  import SplitPage from "../../ui/SplitPage.svelte";
  import Value from "../../ui/Value.svelte";
  import { critChanceStage } from "../../ui/critChance";
  import { latest } from "../../ui/latest.svelte";
  import { changed } from "../../ui/motion.svelte";
  import { REACH_BADGES, REACH_STATE, reachOk, STATE, type Badge } from "../../ui/states";
  import DefensePanel from "./DefensePanel.svelte";
  import DetailRows from "./DetailRows.svelte";
  import MaterialsPane from "./MaterialsPane.svelte";
  import WhyPanel from "./WhyPanel.svelte";
  import { changedFlowKeys, deltaText, flowRowsOf, pick as pickSide, stepOf, stepsOf, stepValue } from "./damageDetail";
  import { DetailStore, type Detail, type Mat } from "./detailStore.svelte";
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
  const contents = $derived(
    flatContents().filter(
      (x): x is typeof x & { content: { enemy_id: string } } => x.content.enemy_id !== null,
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
  /** キャラタブで選んだ主軸スキル。この画面のスキルはこれが正 */
  const mainSkill = $derived(skills.find((s) => s.id === character?.main_skill_id) ?? null);
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
    (skillOverride !== null && skills.some((s) => s.id === skillOverride) ? skillOverride : null)
      ?? mainSkill?.id
      ?? skills[0]?.id
      ?? "",
  );
  const skill = $derived(skills.find((s) => s.id === skillId) ?? null);
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
    [...skills].sort((a, b) => (skillTotals[b.id]?.total ?? -1) - (skillTotals[a.id]?.total ?? -1)),
  );

  // スキル一覧の対象別ダメージ(ドロップダウンを開いたときに計算)
  let skillTotals = $state<Record<string, { perHit: number; total: number }>>({});
  const skillLatest = latest();
  $effect(() => {
    // 対象・キャラ・試し変更が変わったら古い合計を出さない(PR レビュー指摘)
    skillTotals = {};
    if (!skillOpen || !payload || !target || skills.length === 0) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    const temp = JSON.parse(JSON.stringify(NEUTRAL_ADJUSTMENTS)) as Adjustments;
    const contentId = target.content.id;
    const selectedSkillId = skillId;
    const comboType = selectedComboSkillType;
    const buffs = JSON.parse(JSON.stringify(app.calcBuffs));
    skillLatest.run((isCurrent) =>
      Promise.all(
        skills.map(async (s) => [
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
            rs.map(([id, r]) => [id, { perHit: r.per_hit_primary, total: r.total_primary }]),
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
  let result = $state<DamageResult | null>(null);
  let savedResult = $state<DamageResult | null>(null);
  let calculating = $state(false);
  const requestLatest = latest({ debounce: 120 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null; // sim のネスト変更も拾う
    const sp = savedPayload;
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
  const critMode = $derived((result?.critical_chance ?? 0) > 0);
  const pick = <T extends { max: number; critical: number }>(t: T | null | undefined): number | null =>
    t ? (critMode ? t.critical : t.max) : null;
  const perHit = $derived(result?.per_hit_primary ?? null);
  const savedPerHit = $derived(savedResult?.per_hit_primary ?? null);
  const totalValue = $derived(result?.total_primary ?? null);
  const dpsValue = $derived(pickSide(result?.dps, critMode));
  const deltaPct = $derived(
    perHit !== null && savedPerHit !== null && savedPerHit > 0
      ? Math.round((perHit / savedPerHit - 1) * 100)
      : 0,
  );
  // 討伐時間の目安。段の境目は Rust が配る(tables.reach_seconds)。画面は写経しない
  const closeSeconds = $derived(tables.reach_seconds.close);
  const defeatSeconds = $derived(result?.defeat_seconds ?? null);
  /** メーター比。討伐時間が長いほど伸びる(0 = 一瞬、100% = 目安ぴったり) */
  const meterRatio = $derived(defeatSeconds !== null ? Math.min(1, defeatSeconds / closeSeconds) : 0);
  const hasReqs = $derived((target?.content.requirements.length ?? 0) > 0);
  // 評価が未取得の間は入場条件を「不明」として扱い、未達コンテンツに「通る/余裕」を
  // 出さない(ダメージ 120ms・評価 200ms のデバウンス差で毎回この窓が開く。PR レビュー指摘)
  const entryKnown = $derived(!hasReqs || targetEval !== null);
  const entryOk = $derived(!hasReqs || (targetEval?.entry_ok ?? false));
  /** 目安に対する到達段(Rust 側の判定。目安なしは null) */
  const reach = $derived(result?.reach ?? null);
  const reached = $derived(reachOk(reach));
  const badgeState = $derived.by(() => {
    if (perHit === null || !entryKnown || reach === null) return 6;
    if (hasReqs && !entryOk) return reached ? 5 : 4;
    return REACH_STATE[reach];
  });
  // 言葉はこの画面のもの、色は 6 系統から選ぶ(design-system §03)。先頭 6 件は共通(ui/states.ts)
  const BADGE: Badge[] = [...REACH_BADGES, { label: "判定中", state: "unknown" }];

  // --- なぜこの数字?(トレースの式から組み立て) ---------------------------
  // 段の組み立ては calc/damageDetail.ts、面そのものは calc/WhyPanel.svelte。
  // ここに残すのは、鎖(1 発 / 合計 / DPS)と「行ける?」の文が共有する値だけ。
  // 主役がクリティカル前提なら、トレースの段もクリティカル側の到達値で揃える。
  const steps = $derived(stepsOf(result, critMode));
  const atkA = $derived(result?.trace.attack?.value ?? null);
  const defenseValue = $derived(
    result?.trace.categories.find((c) => c.symbol === "C")?.value ?? null,
  );
  const pierced = $derived(stepValue(steps, "攻撃力−防御力"));
  const noPierce = $derived(pierced !== null && pierced <= 0);
  const flowRows = $derived(flowRowsOf(steps, pierced));
  const flowMultLabel = $derived(
    pierced !== null && pierced > 0 && perHit !== null ? fmtRate(perHit / pierced, 1) : "—",
  );

  // --- 数値を開いて詳細を確認する(§00 03: 開くのは押した行の下だけ) ----------
  // 開いている面・変わった行の控えは鎖と「なぜこの数字?」で共有する(calc/detailStore)。
  // 値はすべて Rust 由来(DamageTrace / DamageResult)。UI で作るのは 2 値の差分だけ。
  const details = new DetailStore();
  /** 鎖の ↑ に下線を出すか(その下に変わった段があるか)。判定は WhyPanel と同じ関数 */
  const flowChanged = $derived(changedFlowKeys(details, flowRows, result).length > 0);

  /** 鎖「1 発」: 抜けた分から 1 発までの各段(倍率・実数・到達値) */
  const perHitDetail = $derived.by<Detail | null>(() => {
    const r = result;
    if (r === null || perHit === null || pierced === null) return null;
    const mats: Mat[] = flowRows.map((f) => ({
      label: f.k,
      mult: f.mult === "—" ? undefined : f.mult,
      value: fmtSigned(f.add),
      sub: `ここまで ${fmtInt(Math.round(f.to))}`,
      n: Math.round(f.add),
    }));
    if (r.capped_loss.max > 0) {
      mats.push({
        label: "ダメージ上限(1 段ごと)",
        value: fmtInt(r.damage_cap),
        n: r.damage_cap,
        sub: `上限で ${fmtSigned(-r.capped_loss.max, { max: 3 })}`,
      });
    }
    return {
      mult: flowMultLabel,
      delta: perHit - pierced,
      to: perHit,
      mats,
      idle: 0,
      expr: "ゲームの表記ダメージ(スキル分のみ)。武器強化の追加固定ダメージは含まない(合計の内訳を見る)",
    };
  });
  /** 鎖「合計」: (1 発 × 段数) ＋ (武器強化の追加固定 × 段数) ＋ 割合追加ダメージ。クリ率は段階表示で読む */
  const totalDetail = $derived.by<Detail | null>(() => {
    const r = result;
    if (r === null || perHit === null || totalValue === null) return null;
    const added = pickSide(r.added_damage, critMode) ?? 0;
    const skillTotal = pickSide(r.skill_total, critMode) ?? 0;
    const mats: Mat[] = [
      {
        label: `1 発(表記ダメージ) ${fmtInt(perHit)} × ${r.hit_count} 段`,
        mult: `×${r.hit_count}`,
        value: fmtInt(skillTotal),
        n: skillTotal,
      },
    ];
    if (r.weapon_added_per_hit !== 0) {
      // 段ごとの分割は内部の丸めの都合で、使う側は「1 スキルに 1 回乗る固定値」と捉えている。
      // 段数や 1 段あたりの値は見せず、スキルに乗る総額だけ出す
      mats.push({
        label: "武器強化(追加固定)",
        mult: "+",
        value: fmtInt(r.weapon_added_total),
        n: r.weapon_added_total,
        sub: "上限なし・表記ダメージとは別枠",
      });
    }
    if (added !== 0) {
      mats.push({
        label: "割合追加ダメージ(合計に乗る)",
        mult: fmtSignedPct(r.added_damage_rate, { max: 4 }),
        value: fmtInt(added),
        n: added,
        sub: "シャープネスビジョン・ランダムOP・称号",
      });
    }
    if (!critMode) {
      mats.push({
        label: "クリティカルなら",
        mult: skill ? `×${fmtNum(skill.critical_multiplier)}` : undefined,
        value: fmtInt(r.total.critical),
        n: r.total.critical,
      });
    }
    mats.push({ label: "乱数が最小のとき", value: fmtInt(r.total.min), n: r.total.min });
    return {
      mult: `×${r.hit_count} 段`,
      delta: totalValue - perHit,
      to: totalValue,
      mats,
      idle: 0,
      expr: stepOf(steps, "割合追加ダメージ(合計に乗る)")?.expression ?? null,
    };
  });
  /** 鎖「1 秒あたり」: 合計 × 回/分 ÷ 60 と、実ディレイの内訳。期待値はクリ率で按分した材料も足す */
  const dpsDetail = $derived.by<Detail | null>(() => {
    const r = result;
    const d = r?.actual_delay ?? null;
    if (r === null || d === null || r.dps === null || dpsValue === null || totalValue === null) return null;
    const mats: Mat[] = [
      { label: "合計ダメージ", value: fmtInt(totalValue), n: totalValue },
      {
        label: "基本中ディレイ",
        value: fmtNum(d.base, 2, "s"),
        n: d.base, unit: "s",
        sub: d.fixed ? "固定(減少が効かない)" : undefined,
      },
    ];
    for (const c of d.contributions) {
      mats.push({ label: `↳ ${c.source}`, value: fmtSignedPct(-c.rate), n: -Math.round(c.rate * 100), unit: "%" });
    }
    mats.push({
      label: `中ディレイ減少(上限 ${fmtPct(limits.actual_delay_reduction_max)})`,
      value: fmtPct(d.reduction),
      n: Math.round(d.reduction * 100), unit: "%",
      sub: d.reduction_raw > d.reduction ? `選択中は ${fmtPct(d.reduction_raw)}` : undefined,
    });
    if (d.combo_rate < 1) {
      mats.push({ label: "コンボ(倍率A。間に通常攻撃を挟む)", mult: `×${fmtNum(d.combo_rate)}`, value: "" });
    }
    mats.push({
      label: "中ディレイ",
      value: fmtNum(d.value, 2, "s"),
      n: d.value, unit: "s",
      sub: d.floored ? `下限 ${fmtNum(limits.actual_delay_min, 1, "s")} で頭打ち` : undefined,
    });
    const cycle = r.combo;
    if (cycle) {
      // コンボは 1 サイクル(通常攻撃 → スキル)で割る。実測表はコンボなしの計測なので使わない
      mats.push({
        label: `通常攻撃(${cycle.normal_attack_name})`,
        // 合計ダメージ(total_primary)と同じ側を出す。ここだけ非クリだと足し算が合わなく見える
        value: fmtInt(pickSide(cycle.normal_attack_total, critMode) ?? 0),
        n: pickSide(cycle.normal_attack_total, critMode) ?? 0,
        sub: `中ディレイ ${fmtNum(cycle.normal_delay, 2, "s")}`,
      });
      mats.push({
        label: "コンボインターバル",
        value: cycle.interval !== null ? fmtNum(cycle.interval, 2, "s") : "?",
        n: cycle.interval ?? undefined, unit: "s",
        sub: cycle.interval === null
          ? "wiki 未収録。スキルの中ディレイをそのまま使っています"
          : cycle.interval_binding
            ? "スキルの中ディレイより長いので、こちらが下限になります"
            : "スキルの中ディレイのほうが長いので効きません",
      });
      mats.push({
        label: "1 サイクル",
        value: fmtNum(cycle.seconds, 2, "s"),
        n: cycle.seconds, unit: "s",
        sub: `通常攻撃 ${fmtNum(cycle.normal_delay, 2, "s")} + ${fmtNum(cycle.skill_gap, 2, "s")}`,
      });
    } else {
      mats.push({
        label: "スキル回数",
        value: `${Math.round(d.uses_per_minute)} 回/分`,
        n: Math.round(d.uses_per_minute),
        sub: d.uses_measured ? "実測表から" : "式 60 ÷ 中ディレイ",
      });
    }
    if (r.expected_dps !== null && r.critical_chance > 0 && r.critical_chance < 1) {
      mats.push({
        label: "期待値(クリ率で按分)",
        value: fmtInt(Math.round(r.expected_dps)),
        n: Math.round(r.expected_dps),
        sub: `合計(非クリ) × ${fmtPct(1 - r.critical_chance, 1)} + 合計(クリ) × ${fmtPct(r.critical_chance, 1)}`,
      });
    }
    return {
      mult: `÷ ${fmtNum(cycle?.seconds ?? d.value, 2, "s")}`,
      delta: null,
      to: Math.round(dpsValue),
      mats,
      idle: 0,
      expr: cycle
        ? "1 秒あたり = (スキルの合計 + 通常攻撃の合計) ÷ 1 サイクル"
        : "1 秒あたり = 合計 × スキル回数(回/分) ÷ 60",
    };
  });

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
            {#if skills.length === 0}
              <span class="dim">このキャラのスキルデータは未収録です(仮スキルはありません)。</span>
            {:else}
              <Popover
                label="計算するスキル"
                triggerClass="skill-trigger"
                panelClass="skill-pop"
                disabled={skills.length <= 1}
                onToggle={(open) => (skillOpen = open)}
              >
                {#snippet trigger(open)}
                <span class="sk-line1">
                  <Icon kind="skill" id={skill?.id ?? null} size={20} label={skill?.name ?? "スキル"} />
                  <span class="sk-name">{skill?.name ?? ""}</span>
                  {#if skills.length > 1}<span class="caret" class:rot={open}>▼</span>{/if}
                  <!-- 主軸(キャラタブ)と違うスキルで計算している例外状態。保存されないので
                       ラベンダー(--sim)。行の高さは変えない -->
                  {#if skillOverridden}
                    <span class="sk-override badge-in" use:changed={() => skillId}>ここで上書き中</span>
                  {/if}
                </span>
                <span class="sk-meta num dim">
                  ×<Value value={result ? fmtNum(result.effective_skill_multiplier) : "—"} />
                  ・ <Value value={result ? String(result.hit_count) : "—"} />段
                  ・ 中 <Value value={result?.effective_base_actual_delay != null ? `${fmtNum(result.effective_base_actual_delay)}s` : "?"} />
                  ・ Cri×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                  {#if skill}・ {ELEMENT_LABELS[skill.element]}属性{/if}
                  {#if result?.accuracy_point != null}・ 命中P {fmtInt(result.accuracy_point)}{/if}
                </span>
                {/snippet}
                {#snippet children(close)}
                <div class="pop-head gold"><span>スキル {skills.length} 種 ／ この対象への合計ダメージ順</span></div>
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
                 44px の主役数値は増やさない(金の帯 = 答えは 1 つ。§02)。鎖が右に伸びるだけ。 -->
            <div class="chain">
              <button
                type="button" class="node gate"
                aria-expanded={details.isOpen("perHit")} onclick={() => details.toggle("perHit")}
              >
                <span class="nl">表記ダメージ(1 発)</span>
                <!-- 差分(前回の値からいくつ動いたか)。数値の行には置かない — 44px の数値の横は
                     枠(248px)に入らず、右の節に被る(実機 2026-09-15)。空でも行を取り、出た瞬間に下が動かない -->
                <Value class="hero-num nv" motion={() => perHit} value={perHit !== null ? fmtInt(perHit) : "—"} />
                <span class="nsub num">
                  <!-- 副行は空でも .nsub-line で行を取る。取らないと、差分枠が出た瞬間に節が 11px 伸び、
                       鎖は下ぞろえなので 44px の主役数字がその分だけ持ち上がる(実機 2026-09-17、§00 ③) -->
                  <span class="nsub-line">
                    <Value
                      motion={() => perHit} delta={{}}
                      deltaClass={flowChanged ? "follow" : ""}
                      onDelta={() => details.follow("perHit")}
                    />
                  </span>
                </span>
              </button>
              <button
                type="button" class="node mid"
                aria-expanded={details.isOpen("total")} onclick={() => details.toggle("total")}
              >
                <span class="nl">合計ダメージ <span class="num">(×<Value motion={() => result?.hit_count ?? null} value={String(result?.hit_count ?? 1)} /> 段)</span></span>
                <Value class="nv" motion={() => totalValue} value={totalValue !== null ? fmtInt(totalValue) : "—"} />
                <!-- クリ率はバッジではなく文で(バッジは要らない — ユーザー判断 2026-09-15)。
                     副行は 3 節とも 1 行にそろえ、数値の縦位置を合わせる -->
                <!-- 副行は縦に積む(差分 → クリ率。「クリなら」は内訳にあるので出さない — ユーザー判断 2026-09-15)。数値の横に並べると節が横に伸びて
                     鎖が折り返す(ユーザー指摘 2026-09-15)。数値の縦位置は .nv の行高で合わせる -->
                <span class="nsub num">
                  <span class="nsub-line"><Value motion={() => totalValue} delta={{}} /></span>
                  <span class="nsub-line">
                    {#if result}
                      {#if result.critical_rate === null}
                        <span>クリ率 未記載 → 確定扱い</span>
                      {:else}
                        <!-- クリが出ないときは「出ない」を言わず、率だけ(ユーザー判断 2026-09-15) -->
                        <!-- 0% は赤(届かない)、100% 未満は黄(ぎりぎり)の状態色。100% は地の色(ユーザー判断 2026-09-15) -->
                        <Value
                          class={`${result.critical_chance <= 0 ? "crit-none" : ""} ${result.critical_chance > 0 && result.critical_chance < 1 ? "crit-partial" : ""}`}
                          value={critChanceStage(result!.critical_chance * 100).label}
                        >
                          {#snippet children()}クリ率 {fmtNum(result!.critical_rate!.value, 1, "%")}{critMode ? ` ・ ${critChanceStage(result!.critical_chance * 100).label}` : ""}{/snippet}
                        </Value>
                      {/if}
                    {/if}
                  </span>
                </span>
              </button>
              <button
                type="button" class="node rate"
                aria-expanded={details.isOpen("dps")} onclick={() => details.toggle("dps")}
              >
                <span class="nl">DPS <span class="num">(÷ <Value motion={() => result?.actual_delay?.value ?? null} value={result?.actual_delay ? fmtNum(result.actual_delay.value, 2, "s") : "—"} />)</span></span>
                <Value class="nv" motion={() => dpsValue} value={dpsValue !== null ? fmtInt(Math.round(dpsValue)) : "—"} />
                <span class="nsub dim">
                  <span class="nsub-line"><Value motion={() => (dpsValue === null ? null : Math.round(dpsValue))} delta={{}} /></span>
                  <span class="nsub-line">
                    <!-- コンボ中は「スキルを何回撃てるか」= 1 分 ÷ サイクル。
                         スキルの中ディレイだけで数えると、通常攻撃を挟むぶんを落として速く見える -->
                    <span>
                      {#if result?.combo}
                        {Math.round(result.combo.uses_per_minute)} 回/分 ・
                      {:else if result?.actual_delay}
                        {Math.round(result.actual_delay.uses_per_minute)} 回/分 ・
                      {/if}{critMode ? "クリ確定" : "非クリ"}
                    </span>
                  </span>
                  {#if result && result.expected_dps !== null && result.critical_chance > 0 && result.critical_chance < 1}
                    <span class="nsub-line">
                      期待値 <Value motion={() => result?.expected_dps ?? null} value={fmtInt(Math.round(result.expected_dps))} />(クリ率 {fmtPct(result.critical_chance, 1)})
                    </span>
                  {/if}
                </span>
              </button>
              <!-- 討伐時間。敵 HP か中ディレイが未収録なら出せないので、ノードごと出さない
                   (§00 02。0 や「—」で埋めると画面が嘘をつく)。HP はソロの値 -->
              {#if result && result.defeat_seconds !== null && result.enemy_hp !== null}
                <div class="node rate">
                  <span class="nl">討伐時間 <Value motion={() => result?.enemy_hp ?? null} value={`(HP ${fmtInt(result.enemy_hp)})`} /></span>
                  <Value class="nv" motion={() => result?.defeat_seconds ?? null} value={fmtDuration(result.defeat_seconds)} />
                  <span class="nsub dim">
                    <!-- 何秒縮んだか。表示は秒に丸めた値なので、差分もその丸めた値から出す
                         (画面の 27秒 → 26秒 と ↓1秒 が食い違わない)。討伐時間は**短いほど良い**ので
                         色だけ入れ替える(矢印は数のとおり。§10) -->
                    <span class="nsub-line">
                      <Value
                        motion={() => (result?.defeat_seconds == null ? null : Math.round(result.defeat_seconds))}
                        delta={{ unit: "秒", digits: 0 }}
                        deltaClass="less-is-better"
                      />
                    </span>
                    <!-- クリ確定 / 非クリは隣の DPS 節に出ている(重ねない。§00 02) -->
                    <span class="nsub-line">ソロ</span>
                  </span>
                </div>
              {/if}
            </div>
            <!-- 鎖の各数値の内訳。押した節は動かず、鎖の直下に増える(§00 03) -->
            {#if perHitDetail}<DetailRows boxed d={perHitDetail} store={details} open={details.isOpen("perHit")} />{/if}
            {#if totalDetail}<DetailRows boxed d={totalDetail} store={details} open={details.isOpen("total")} />{/if}
            {#if dpsDetail}<DetailRows boxed d={dpsDetail} store={details} open={details.isOpen("dps")} />{/if}
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
            {#if result?.actual_delay}
              {@const d = result.actual_delay}
              <div class="delay-note dim">
                中ディレイ {fmtNum(d.base, 2, "s")}
                {#if d.fixed}
                  ×(固定・減少が効かない)
                {:else if d.reduction > 0}
                  × (1 − {fmtPct(d.reduction)}){#if d.reduction_raw > d.reduction}<span class="warn"> ※減少値は上限 {fmtPct(limits.actual_delay_reduction_max)}({fmtPct(d.reduction_raw)} ぶん選択中)</span>{/if}
                {/if}
                {#if d.combo_rate < 1}× {fmtNum(d.combo_rate)}(コンボ){/if}
                = {fmtNum(d.value, 2, "s")}{#if d.floored}<span class="warn"> ※下限 {fmtNum(limits.actual_delay_min, 1, "s")}</span>{/if}
                {#if d.contributions.length > 0}
                  ／ 減少源: {d.contributions.map((c) => `${c.source} ${fmtPct(c.rate)}`).join(" ・ ")}
                {/if}
                <br />
                {#if result?.combo}
                  {@const c = result.combo}
                  1 サイクル = 通常攻撃 {fmtNum(c.normal_delay, 2, "s")} + max(スキル {fmtNum(c.skill_delay, 2, "s")},
                  CI {c.interval !== null ? fmtNum(c.interval, 2, "s") : "?"}) = {fmtNum(c.seconds, 2, "s")}
                  ／ 1 秒あたり = (スキル + {c.normal_attack_name})の合計 ÷ 1 サイクル
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
            {#if result?.critical_rate}
              {@const c = result.critical_rate}
              <div class="delay-note dim">
                クリティカル率 (装備クリ補正 {fmtInt(c.equipment_critical)} + 1) × 2 × (AGI {fmtInt(c.agi)} / (AGI + 対象AGI {fmtInt(c.target_agi)}))
                {#if c.siena_rate > 0}× シエナのオーラ {fmtNum(1 + c.siena_rate, 2)}{/if}
                = {fmtNum(c.from_agi, 1, "%")}
                ＋ スキル Cri値 {fmtInt(c.skill)}%{#if c.bonus > 0} ＋ 増加 {fmtInt(c.bonus)}%{/if}
                − 対象のクリティカル被撃率 {fmtInt(-c.target_taken_rate)}%
                = <b>{fmtNum(c.value, 1, "%")}</b>{#if c.raw < 0}<span class="warn"> ※下限 0%</span>{:else if c.raw > 100}<span class="warn"> ※上限 100%</span>{/if}
              </div>
            {:else if result && skill}
              <div class="delay-note dim">
                クリティカル率は出せません(この敵の AGI / クリティカル被撃率、またはスキルの Cri値が wiki 未記載)。
              </div>
            {/if}
            {#if result && result.effective_base_actual_delay === null}
              <div class="delay-note dim">このスキルは wiki に基本中ディレイ(「動作」列)が無いため、1 秒あたりの火力を出せません。</div>
            {/if}
          </div>
        </SheetCard>

        <!-- なぜこの数字? -->
        <WhyPanel {result} {defense} {perHit} {critMode} store={details} />
        </div>
      {/if}
  {/snippet}
  {#snippet right()}
      {#if character && payload}
        <MaterialsPane
          {payload} characterId={character?.id} {skill} {skillId} targetId={target?.content.id ?? null}
          {result} {deltaPct} {comboCount} comboType={selectedComboSkillType}
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
  /* 鎖(§14 決定 1)。44px の主役数値は増やさない — 金の帯 = 答えは 1 つ(§02)を
     壊さず、鎖が右に伸びるだけ。狭いときは折り返す(桁で隣が動かないよう各段に min-width) */
  /* 節の高さは 3 つとも同じ(stretch)。タイトル行は天井に固定し、数値と副行は底に寄せる
     (= 下ぞろえ。タイトルの位置は固定 — ユーザー判断 2026-09-15) */
  /* 押せる範囲(節)は 3 つとも同じ高さ(stretch)。内側の余白 4px / 6px を節に持たせ、
     hover の塗りが節の形そのものになるようにする。左端は余白ぶん外に出して、上の行と文字位置をそろえる */
  .chain { display: flex; align-items: stretch; gap: 4px; flex-wrap: wrap; margin: 0 -6px; }
  /* 枠は付けない(付けると窮屈 — ユーザー判断 2026-09-15)。見出しはタイトル行として太字で主張させる。
     余白: タイトル行 → 数値 6px、数値 → 差分 4px、差分 → 補足 4px(8 / 4 / 4 の一段階トーンダウン。
     5 / 2 だと差分と補足が密着して 1 塊に読める — レビュー 2026-09-15) */
  .chain .node { display: flex; flex-direction: column; gap: 0; min-width: 0; padding: 4px 6px 5px; }
  .chain .nl {
    line-height: 14px; font-size: 10px; font-weight: 700; color: var(--fg-head); white-space: nowrap;
  }
  /* .num は ui/Value.svelte が描く子要素にも付くので :global で届かせる */
  .chain .nl :global(.num) { font-weight: 600; color: var(--fg-muted); }
  .chain :global(.nv) { margin-top: auto; padding-top: 6px; }
  .chain .nsub { margin-top: 4px; gap: 4px; }
  .chain :global(.nv) { font-weight: 700; color: var(--fg); white-space: nowrap; }
  .chain .node.gate :global(.nv) { min-width: 120px; }
  .chain .node.mid :global(.nv) { font-size: 15px; min-width: 68px; }
  /* DPS は主役(1 発 = --t-result)の隣の見出し数字。--t-result だと 1 発と並んで大きすぎる(ユーザー 2026-09-16) */
  .chain .node.rate :global(.nv) { font-size: var(--t-heading); min-width: 68px; }
  /* 押せるノードは桁・状態で動かない。実測(tools/design-audit/live/digits.js)では
     1 発 180〜220px・バッジ 39〜106px・合計 75〜83px と揺れ、右のノードが最大 29px 逃げていた
     (§09 規則 4)。幅を取り切り、中身が短いときは空けておく。値の上限
     (与ダメージ 29,500,000 / 合計 10 桁)が入る幅にしてあるので、桁が溢れて切れることはない */
  .chain .node.gate { width: 260px; }
  .chain .node.mid { min-width: 136px; }
  .chain .nsub { font-size: 9px; color: var(--fg-dim); white-space: nowrap; display: flex; flex-direction: column; }
  .chain .nsub-line { display: flex; align-items: baseline; gap: 5px; min-height: 14px; }
  /* ui/Value.svelte が描く crit-none / crit-partial は子コンポーネントの要素 */
  .chain :global(.crit-none) { color: var(--state-short-fg); font-weight: 700; }
  .chain :global(.crit-partial) { color: var(--state-edge-fg); font-weight: 700; }
  /* 差分(いくつ変わったか)は数値の真下。9px だと 44px の数値の下で読めないので 11px。
     差分枠は ui/Value.svelte が描く(スコープ付き CSS が届かないので :global) */
  .chain .nsub :global(.delta) { font-size: 11px; line-height: 14px; }
  /* 押すと内訳が鎖の直下に開く。hover は塗りだけで、余白を足して隣を動かさない(§00 03) */
  .chain button.node { text-align: left; border-radius: var(--r-inset); }
  .chain button.node:hover { background: var(--bg-active); }
  .chain button.node:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
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
