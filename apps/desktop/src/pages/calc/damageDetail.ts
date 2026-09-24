// 計算結果(DamageResult / DamageTrace)を「読める段」に組み直す。
// **値はすべて Rust 由来**で、ここが作るのは並べ方と 2 値の差分だけ。式は持たない。
// 画面(CalcPage の鎖 / WhyPanel の帯)はここが返した Detail / 行を描くだけにする。
import type {
  Attacker, AttackPowerBreakdown, CategoryTrace, DamageCategory, DamageResult, DefenseProfile, FormulaStep,
  StatKind,
} from "../../api/types";
import { fmtInt, fmtNum, fmtPct, fmtRate, fmtSigned, fmtSignedPct, formatLayerValue } from "../../format";
import { t } from "../../i18n";
import { EQUIPMENT_STAT_LABELS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
import { limits } from "../../limits.svelte";
import { swapNote, type Presence } from "../../ui/presence";
import type { Detail, DetailStore, Mat } from "./detailStore.svelte";

/** `t()` の undefined 許容版。値が無いときは undefined のまま通す */
const tOrUndef = (text: string | undefined): string | undefined => (text === undefined ? undefined : t(text));

/** 主役がクリティカル前提なら、段も到達値もクリティカル側でそろえる */
export const pick = <T extends { max: number; critical: number }>(
  t: T | null | undefined,
  critMode: boolean,
): number | null => (t ? (critMode ? t.critical : t.max) : null);

export const stepsOf = (result: DamageResult | null, critMode: boolean): FormulaStep[] =>
  (critMode ? result?.trace.steps_critical : result?.trace.steps_max) ?? [];

export const stepValue = (steps: FormulaStep[], name: string): number | null =>
  steps.find((s) => s.name === name)?.value ?? null;

export const stepOf = (steps: FormulaStep[], name: string): FormulaStep | null =>
  steps.find((s) => s.name === name) ?? null;

export const categoryOf = (result: DamageResult | null, c: DamageCategory): CategoryTrace | null =>
  result?.trace.categories.find((x) => x.category === c) ?? null;

// --- 攻撃力(A)の内訳 ------------------------------------------------------
// 構成は Rust の AttackPowerBreakdown をそのまま使う(UI で式を持たない)
export interface AtkRow {
  k: string;
  v: number;
  c: string;
  note: string;
  /** 帯の幅(%) */
  pct: string;
  /** 行に出す割合 */
  share: string;
  /** その段までの到達値 */
  to: number;
}

export function attackRows(atk: AttackPowerBreakdown | null): AtkRow[] {
  if (atk === null) return [];
  const raw = [
    { k: "ステ攻撃力", v: atk.stat_attack, c: "var(--flow-base)", note: t("素ステ・補正源から") },
    { k: "装備攻撃力", v: atk.equipment_attack, c: "var(--flow-1)", note: t("基本/強化 × 依存別係数") },
    { k: "装備攻撃力強化倍率", v: atk.enhance_bonus, c: "var(--flow-2)", note: t("パワーW・ストロングW") },
  ].filter((x) => x.v > 0);
  const total = raw.reduce((a, x) => a + x.v, 0) || 1;
  let running = 0;
  return raw.map((x, i) => {
    running += x.v;
    return {
      ...x,
      pct: fmtNum(Math.max(1.5, (x.v / total) * 100), 2, "%"),
      share: fmtPct(x.v / total),
      // 最後の段は必ず A に着地させる(切捨ての端数で足し算が合わなくなるのを防ぐ)
      to: i === raw.length - 1 ? atk.value : running,
    };
  });
}

// --- 倍率の段(③ 倍率で伸ばす の帯) ---------------------------------------
export interface FlowRow {
  k: string;
  add: number;
  mult: string;
  /** 倍率の実数(前段との比)。段の順序に依存しない「効き」の指標 */
  factor: number;
  c: string;
  /** その段までの到達値 */
  to: number;
  /** 対応する Rust の段名。材料(カテゴリ)はこの段の `categories` から引く */
  step: string;
}

export const FLOW_COLORS: Record<string, string> = {
  "スキル倍率": "var(--flow-1)",
  "クリティカル": "var(--flow-2)",
  "コンボ・属性・カット率・オーラ": "var(--flow-3)",
  "最終ダメージ固定値(下限)": "var(--flow-4)",
  "最終ダメージ・カット率A・被害減少": "var(--flow-5)",
  "各種ダメージ増減": "var(--flow-6)",
  "攻撃ダメージ・PVP補正": "var(--flow-7)",
};

export function flowRowsOf(steps: FormulaStep[], pierced: number | null): FlowRow[] {
  if (pierced === null) return [];
  let running = pierced;
  const rows: FlowRow[] = [
    { k: "防御を抜けた攻撃力(素通り)", add: pierced, mult: "—", factor: 1, c: "var(--fg-dim)", to: pierced, step: "攻撃力−防御力" },
  ];
  for (const s of steps) {
    if (s.kind !== "factor" && s.kind !== "running") continue;
    // 段の種別は Rust の FormulaStep.kind。到達値は FormulaStep.reached。
    // 倍率列は倍率の段はその値、到達値で返る段は前段との比(表示用)
    const isFactor = s.kind === "factor";
    const factor = isFactor ? s.value : running > 0 ? s.reached / running : 1;
    const mult = isFactor || running > 0 ? fmtRate(factor) : "—";
    rows.push({ k: s.name, add: s.reached - running, mult, factor, c: FLOW_COLORS[s.name] ?? "var(--fg-dim)", to: s.reached, step: s.name });
    running = s.reached;
  }
  return rows;
}

// --- 倍率の材料(カテゴリ) ------------------------------------------------
export const activeCategoriesOf = (result: DamageResult | null): CategoryTrace[] =>
  (result?.trace.categories ?? []).filter((c) => c.kind !== "assigned" && c.value !== 0);

/** 上限(減算系は下限)まであと。伸ばす方向はカテゴリで違う。唯一の正は Rust の damage_levers */
export const catHeadroom = (c: CategoryTrace): number | null => {
  if (!c.cap) return null;
  const bound = c.subtractive ? c.cap.min : c.cap.max;
  if (bound === null || bound === undefined) return null;
  return c.subtractive ? c.value - bound : bound - c.value;
};
export const catAtCap = (c: CategoryTrace) => {
  const room = catHeadroom(c);
  return room !== null && room <= 1e-9;
};
/** 上限(減算系は下限)で捨てられた分。0 なら捨てていない */
export const catLoss = (c: CategoryTrace) => Math.abs(c.raw - c.value);
export const fmtCatValue = (c: CategoryTrace) =>
  c.kind === "rate" ? fmtSignedPct(c.value, { max: 4 }) : fmtNum(c.value);
export const fmtCatRaw = (c: CategoryTrace) =>
  c.kind === "rate" ? fmtSignedPct(c.raw, { max: 4 }) : fmtNum(c.raw);
export const fmtCatLoss = (c: CategoryTrace) => {
  const loss = catLoss(c);
  return c.kind === "rate" ? fmtSignedPct(-loss, { max: 4 }) : fmtSigned(-loss, { max: 4 });
};

/** カテゴリX 攻撃ダメージは子(X1〜X6)の合計で、供給源は子に積まれる(domain category.rs ATTACK_DAMAGE_CHILDREN) */
const ATTACK_DAMAGE_CHILDREN: DamageCategory[] = [
  "attack_damage_isabel", "attack_damage_general", "attack_damage_basic_trigger",
  "attack_damage_skill", "attack_damage_special", "attack_damage_japan",
];
/** カテゴリに実際に値を足した供給源(トレースの category_contributions から)。X は子の供給源をまとめて返す */
const catContributions = (result: DamageResult | null, c: string) => {
  const all = result?.trace.category_contributions ?? [];
  return c === "attack_damage_rate"
    ? all.filter((x) => ATTACK_DAMAGE_CHILDREN.includes(x.category))
    : all.filter((x) => x.category === c);
};
const fmtContributionValue = (kind: CategoryTrace["kind"], v: number) =>
  kind === "rate" ? fmtSignedPct(v, { max: 4 }) : fmtNum(v);

export function catMat(
  store: DetailStore, result: DamageResult | null, c: CategoryTrace, atkRows: AtkRow[],
): Mat {
  // A 攻撃力は ① と同じ構成(ステ攻撃力 / 装備攻撃力 / 強化倍率)で開く。供給源 1 行では読めない
  if (c.category === "attack_power" && atkRows.length > 0) {
    return {
      label: `${c.symbol} ${c.label}`,
      value: fmtCatValue(c),
      n: c.value,
      changed: store.changes.touch("cat:attack_power", c.value, result),
      key: "cat:attack_power",
      subs: atkRows.map((a) => ({
        label: t(a.k),
        value: fmtInt(Math.round(a.v)),
        sub: a.note,
        n: Math.round(a.v),
      })),
    };
  }
  const contributions = store.contributions.mark(
    c.category, catContributions(result, c.category), (x) => x.source,
  );
  const names = (st: Presence) => contributions.filter((x) => x.state === st).map((x) => x.item.source);
  const n = c.kind === "rate" ? c.value * 100 : c.value;
  return {
    changed: store.changes.touch(`cat:${c.category}`, n, result) || contributions.some((x) => x.state !== "same"),
    label: `${c.symbol} ${t(c.label)}`,
    mult: c.kind === "rate" ? `×${fmtNum(c.factor)}` : undefined,
    value: fmtCatValue(c),
    n,
    unit: c.kind === "rate" ? "%" : undefined,
    sub: catLoss(c) > 1e-9 ? t("上限で {loss}", { loss: fmtCatLoss(c) }) : undefined,
    note: swapNote(names("gone"), names("added")),
    key: contributions.length > 0 ? `cat:${c.category}` : undefined,
    subs:
      contributions.length > 0
        ? contributions.map(({ item: x, state, key }) => ({
            id: key,
            label: t(x.source),
            value: fmtContributionValue(c.kind, x.value),
            n: c.kind === "rate" ? x.value * 100 : x.value,
            unit: c.kind === "rate" ? "%" : undefined,
            sub: x.category === c.category ? undefined : tOrUndef(categoryOf(result, x.category)?.label),
            state,
          }))
        : undefined,
  };
}

/** 段の内訳。材料は Rust が段ごとに申告したカテゴリ(FormulaStep.categories)から引く */
export function stepDetail(
  store: DetailStore, result: DamageResult | null, steps: FormulaStep[], atkRows: AtkRow[],
  name: string, mult: string, delta: number | null, to: number | null,
): Detail {
  const step = stepOf(steps, name);
  const cats = (step?.categories ?? [])
    .map((c) => categoryOf(result, c))
    .filter((c): c is CategoryTrace => c !== null);
  const active = cats.filter((c) => c.kind === "assigned" || c.value !== 0);
  return {
    mult, delta, to,
    mats: active.map((c) => catMat(store, result, c, atkRows)),
    idle: cats.length - active.length,
    expr: step?.expression ?? null,
  };
}

/**
 * ステ 1 つに効かせている要因の一覧(素ステ + 補正源 + 上限で捨てた分)。
 * 実数(何ポイント動かしたか)は Rust の `StatSourceEffect.effect`(上限込み)。UI で再計算しない。
 * 素ステ + Σ実数 = 最終能力値。
 */
export function statFactorMats(result: DamageResult | null, kind: StatKind): Mat[] {
  const st = result?.trace.stats.find((s) => s.kind === kind);
  if (!st) return [];
  const mats: Mat[] = [
    { label: t("素ステ(振り分け)"), value: fmtInt(st.base), n: st.base },
  ];
  for (const c of result?.trace.stat_source_effects ?? []) {
    if (c.kind !== kind) continue;
    mats.push({
      label: t(c.source),
      value: fmtSigned(c.effect, { max: 3 }),
      sub: `${STAT_LAYER_LABELS[c.layer]} ${formatLayerValue(c.layer, c.value)}`,
      n: c.effect,
    });
  }
  if (st.capped_loss > 0) {
    mats.push({
      label: t("上限で捨てた分"),
      value: fmtSigned(-st.capped_loss, { max: 3 }),
      sub: t("上限 {cap}", { cap: fmtInt(st.stat_cap) }),
      n: -st.capped_loss,
    });
  }
  if (st.pinned_from !== null) {
    mats.push({
      label: t("一時調整で固定"),
      value: fmtInt(st.effective),
      sub: t("固定前 {v}", { v: fmtInt(st.pinned_from) }),
      n: st.effective,
    });
  }
  return mats;
}

const EQUIPMENT_ATTACK_LAYER_LABELS: Record<string, string> = { base: t("基本"), enhanced: t("強化") };

/**
 * 熊(魔法人形)の係数行 — wiki 計算式まとめ `STAB(熊)` の 1 行を、トレースに載っている係数から
 * そのまま並べ直す(値は Rust 由来。TS に係数を書き写さない)。wiki 自身が「2026/4/1 以前の情報」と
 * 断っている行なので、実測と合わないときに真っ先に疑う場所として掘り下げの先頭に置く(ADR-016)。
 * 本体(player)には無い行。
 */
const MAGIC_DOLL_SOURCE_NOTE = t(
  "熊の係数は wiki 計算式まとめ STAB(熊) の行(2026/4/1 以前の情報)。実測と合わなければ、敵の値より先にこの行を疑う",
);
/** 係数はラベル列(唯一の広い列)に載せる。数値列は 64px 固定で文字列が折り返す */
function magicDollCoefficientMat(result: DamageResult | null, row: AtkRow["k"]): Mat | null {
  if (row === "ステ攻撃力") {
    const parts = result?.trace.stat_attack_parts ?? [];
    if (parts.length === 0) return null;
    return {
      label: t("係数 STAB(熊): {list}", {
        list: parts.map((p) => `${STAT_LABELS[p.kind]} ×${fmtNum(p.coefficient)}`).join(" / "),
      }),
      value: "",
    };
  }
  if (row === "装備攻撃力") {
    const parts = result?.trace.equipment_attack_parts ?? [];
    if (parts.length === 0) return null;
    // 値種ごとに 基本/強化 の係数を並べる(係数 0 の層はトレースに行が無いので 0 と書く)
    const kinds = [...new Set(parts.map((p) => p.value))];
    const rate = (kind: (typeof kinds)[number], layer: "base" | "enhanced") =>
      fmtNum(parts.find((p) => p.value === kind && p.layer === layer)?.coefficient ?? 0);
    return {
      label: t("係数 STAB(熊)(基本/強化): {list}", {
        list: kinds.map((k) => `${EQUIPMENT_STAT_LABELS[k]} ${rate(k, "base")}/${rate(k, "enhanced")}`).join("・"),
      }),
      value: "",
    };
  }
  return null;
}

/** 攻撃力の構成行の内訳。ステ攻撃力は「実際に使っている依存ステ」だけを並べ、押すと要因まで開く */
export function atkDetail(
  result: DamageResult | null, steps: FormulaStep[], a: AtkRow, attacker: Attacker,
): Detail {
  const atk = result?.trace.attack ?? null;
  const mats: Mat[] = [];
  const doll = attacker === "magic_doll" ? magicDollCoefficientMat(result, a.k) : null;
  if (doll) mats.push(doll);
  if (a.k === "ステ攻撃力") {
    for (const p of result?.trace.stat_attack_parts ?? []) {
      mats.push({
        label: STAT_LABELS[p.kind],
        mult: `×${fmtNum(p.coefficient)}`,
        value: fmtInt(Math.round(p.contribution)),
        sub: t("能力値 {v}", { v: fmtInt(p.effective) }),
        n: Math.round(p.contribution),
        key: `atkstat:${p.kind}`,
        subs: statFactorMats(result, p.kind),
      });
    }
  } else if (a.k === "装備攻撃力") {
    for (const p of result?.trace.equipment_attack_parts ?? []) {
      mats.push({
        label: `${EQUIPMENT_ATTACK_LAYER_LABELS[p.layer]} ${EQUIPMENT_STAT_LABELS[p.value]}`,
        mult: `×${fmtNum(p.coefficient)}`,
        value: fmtInt(Math.round(p.contribution)),
        sub: t("装備値 {v}", { v: fmtInt(p.amount) }),
        n: Math.round(p.contribution),
        key: `eqatk:${p.layer}:${p.value}`,
        subs: p.sources.map((s) => ({
          label: t(s.source),
          mult: `×${fmtNum(p.coefficient)}`,
          value: fmtInt(Math.round(s.contribution)),
          sub: t("装備値 {v}", { v: fmtInt(s.amount) }),
          n: Math.round(s.contribution),
        })),
      });
    }
  } else if (a.k === "装備攻撃力強化倍率") {
    for (const s of result?.trace.equipment_enhance_sources ?? []) {
      mats.push({
        label: t(s.source),
        mult: fmtSignedPct(s.value, { max: 4 }),
        value: fmtSignedPct(s.value, { max: 4 }),
        n: s.value * 100,
        unit: "%",
      });
    }
  }
  return {
    mult: a.k === "装備攻撃力強化倍率" && atk ? fmtSignedPct(atk.enhance_rate, { max: 4 }) : "—",
    delta: a.v,
    to: a.to,
    mats,
    idle: 0,
    note: doll ? MAGIC_DOLL_SOURCE_NOTE : undefined,
    expr: a.k === "装備攻撃力" ? (stepOf(steps, "装備攻撃力")?.expression ?? null) : null,
  };
}

// --- 効いていない分の棚卸し(design-system §14 決定 2)-----------------------
// 上限で捨てた分は 能力値上限 / カテゴリ上限 / ダメージ上限 / 防御力上限 / 中ディレイ
// の 5 階層に散っている。5 箇所を回らないと総ロスが分からない状態は
// 「効いていない量を見せるのがこの道具の価値」(§00)を薄めるので、1 箇所に集める。
export interface LostRow {
  /** どの上限か */
  k: string;
  /** 上限にぶつかる前の値 */
  raw: string;
  /** 実際に効いている値 */
  val: string;
  /** 捨てている量 */
  loss: string;
  /** 効いている割合(0〜1)。塗り = 効いている量、斜線 = 捨てた量 */
  kept: number;
}

export function lostRowsOf(
  result: DamageResult | null, defense: DefenseProfile | null, perHit: number | null, critMode: boolean,
): LostRow[] {
  const out: LostRow[] = [];
  // 能力値上限(覚醒段階 + エタの意志 Lv)
  for (const s of result?.trace.stats ?? []) {
    if (s.capped_loss > 1e-9) {
      const before = s.effective + s.capped_loss;
      out.push({
        k: t("能力値上限 {stat}", { stat: STAT_LABELS[s.kind] }),
        raw: fmtInt(before),
        val: fmtInt(s.stat_cap),
        loss: fmtInt(s.capped_loss),
        kept: before > 0 ? s.effective / before : 1,
      });
    }
  }
  // カテゴリ上限。合算してから切るので、積んだのに効いていない量が数値で見えないと詰み手前が分からない
  for (const c of activeCategoriesOf(result).filter((x) => catLoss(x) > 1e-9)) {
    out.push({
      k: t("カテゴリ上限 {cat}", { cat: t(c.label) }),
      raw: fmtCatRaw(c),
      val: fmtCatValue(c),
      loss: fmtCatLoss(c),
      kept: Math.abs(c.raw) > 1e-9 ? Math.abs(c.value) / Math.abs(c.raw) : 1,
    });
  }
  // ダメージ上限(wiki: Quest/覚醒クエスト。多段スキルでも 1 段ごとに適用)
  if (result !== null && perHit !== null) {
    const loss = pick(result.capped_loss, critMode) ?? 0;
    if (loss > 0) {
      const before = perHit + loss;
      out.push({
        k: t("ダメージ上限(1 段ごと)"),
        raw: fmtInt(before),
        val: fmtInt(result.damage_cap),
        loss: fmtInt(loss),
        kept: before > 0 ? perHit / before : 1,
      });
    }
  }
  // 防御力上限。防御タブと同じ値だが、棚卸しのために回らせない
  if (defense !== null) {
    const rows: [string, number, number][] = [
      [t("物理"), defense.physical_defense, defense.physical_defense_loss],
      [t("魔法"), defense.magic_defense, defense.magic_defense_loss],
      [t("複合"), defense.composite_defense, defense.composite_defense_loss],
    ];
    for (const [name, value, loss] of rows) {
      if (loss > 1e-9) {
        const before = value + loss;
        out.push({
          k: t("防御力上限 {kind}", { kind: name }),
          raw: fmtInt(before),
          val: fmtInt(defense.defense_cap),
          loss: fmtInt(loss),
          kept: before > 0 ? value / before : 1,
        });
      }
    }
  }
  // 中ディレイ。減少値の上限と秒そのものの下限は別の捨て方なので分けて出す
  const ad = result?.actual_delay ?? null;
  if (ad !== null) {
    if (ad.reduction_raw > ad.reduction + 1e-9) {
      out.push({
        k: t("中ディレイ減少の上限({pct})", { pct: fmtPct(limits.actual_delay_reduction_max) }),
        raw: fmtPct(ad.reduction_raw),
        val: fmtPct(ad.reduction),
        loss: fmtPct(ad.reduction_raw - ad.reduction),
        kept: ad.reduction_raw > 0 ? ad.reduction / ad.reduction_raw : 1,
      });
    }
    if (ad.floored) {
      const want = ad.raw;
      out.push({
        k: t("中ディレイの下限({s})", { s: fmtNum(limits.actual_delay_min, 1, "s") }),
        raw: fmtNum(want, 2, "s"),
        val: fmtNum(ad.value, 2, "s"),
        loss: t("{v} ぶん遅い", { v: fmtNum(ad.value - want, 2, "s") }),
        kept: ad.value > 0 ? want / ad.value : 1,
      });
    }
  }
  return out;
}

/**
 * 直近の計算で変わった段のキー。段の「足した分」は前段が変われば全部変わる(結果)ので、
 * 段自身の倍率で判定する(原因)。倍率を持たない先頭の段(素通り)だけは値で判定する。
 * 鎖の ↑(CalcPage)と帯の行(WhyPanel)の両方が同じ判定を読む。
 */
export function changedFlowKeys(
  store: DetailStore, flowRows: FlowRow[], result: DamageResult | null,
): string[] {
  const own = (f: FlowRow) => (f.mult === "—" ? Math.round(f.add) : Math.round(f.factor * 10000));
  return flowRows
    .filter((f) => store.changes.touch(`flow:${f.k}`, own(f), result))
    .map((f) => `flow:${f.k}`);
}

/**
 * 伸び率の表示。**表記ダメージと合計ダメージの 2 本**を並べる — シャープネスビジョンや
 * 武器強化のように「表記は動かないのに合計は伸びる」ものがあり、片方だけだと
 * 「効いていない」と読めてしまう(ユーザー判断 2026-09-01)。
 */
export const deltaText = (pct: number) => (pct === 0 ? "±0%" : fmtSigned(pct, { max: 2 }, "%"));
