// 実測ダメージの送信。ゲーム内で実際に出たダメージを、計算に使った条件ごと集める。
//
// 敵の防御力・カット率・被害減少は wiki が「約」「推定値」としか書いておらず、
// ゲーム内でも見られない。**実測から逆算するしか確かめる方法がない**(手順は
// docs/enemy-verification.md)。集めた実測を突き合わせて `[仮]` を外していく。
//
// **まだ収録していない敵**も対象にする(新しく実装されたモブは wiki にも載っていない)。
// その場合ツール側で計算値は出せないが、**攻撃側の条件は敵に依らず出せる**ので、
// 攻撃力(A)・最終能力値・スキルと実測を送れば、こちらで逆算できる。
//
// 送信経路は問い合わせと同じ中継サーバー(services/inquiry-worker → GitHub Issue)。
// 条件は **1 行の JSON** で診断情報に入れる。中継側が本文の ``` を潰すので本文には置けず、
// 診断情報だけが Issue でコードブロックに収まるため(services/inquiry-worker の clean / renderIssueBody)。
import type { AttackPowerBreakdown, DamageResult, EffectiveStats, Skill } from "./api/types";
import { fmtInt } from "./format";
import type { InquiryDraft } from "./inquiry";

export interface MeasurementConditions {
  gameCharacterId: string;
  awakeningStage: number;
  eternalLevel: number;
  skill: Skill;
  comboSkillType: string | null;
  /** 収録済みの対象。「一覧に無い敵」のときは null */
  content: { id: string; name: string; enemyId: string } | null;
  /** 一覧に無い敵。収録済みのときは null */
  unlisted: { name: string; place: string } | null;
  /** コンボで挟んでいる通常攻撃(コンボしていないなら null) */
  normalAttack: Skill | null;
  /** ツールの計算。敵が未収録だと出せないので null */
  result: DamageResult | null;
  /** 攻撃力(A)。敵に依らないので未収録でも出せる */
  attack: AttackPowerBreakdown | null;
  /** 最終能力値。逆算の入力になる */
  stats: EffectiveStats | null;
}

export interface MeasurementEntry {
  /** ゲーム内で出た 1 発のダメージ */
  damage: number;
  /** クリティカルだったか */
  critical: boolean;
  /** 撃った回数(同じ条件で何発見て、その最大値を採ったか) */
  hits: number;
  /** 気づいたこと(強打が乗った、上限に当たっていそう、など) */
  note: string;
}

/** 計算値のうち、実測と突き合わせる側の値。敵が未収録なら null */
export const expectedDamage = (result: DamageResult | null, critical: boolean): number | null =>
  result ? (critical ? result.per_hit.critical : result.per_hit.max) : null;

/** 実測と計算の差(割合)。比べる相手が無い / 0 のときは null */
export const damageGap = (measured: number, expected: number | null): number | null =>
  expected !== null && expected > 0 ? measured / expected - 1 : null;

/** 対象の呼び名(収録済みはコンテンツ名、未収録は入力された敵の名前) */
export const targetLabel = (conditions: MeasurementConditions): string =>
  conditions.content?.name ?? conditions.unlisted?.name ?? "対象";

/** 中継サーバーに送る下書き。送信前にそのまま全文が表示される(問い合わせと同じ作法) */
export function measurementDraft(
  conditions: MeasurementConditions,
  entry: MeasurementEntry,
): InquiryDraft {
  const expected = expectedDamage(conditions.result, entry.critical);
  const gap = damageGap(entry.damage, expected);
  const label = targetLabel(conditions);
  const lines = [
    `${label} を ${conditions.skill.name} で殴った実測です。`,
    "",
    `- 実測: ${fmtInt(entry.damage)}(${entry.critical ? "クリティカル" : "非クリティカル"}・${fmtInt(entry.hits)} 発中の最大)`,
  ];
  if (expected === null) {
    lines.push(
      "- このツールの計算: **出せません**(この敵はまだ収録していません)",
      `- 攻撃力(A): ${conditions.attack ? fmtInt(conditions.attack.value) : "—"}`,
    );
    if (conditions.unlisted?.place.trim()) {
      lines.push(`- 出た場所: ${conditions.unlisted.place.trim()}`);
    }
  } else {
    lines.push(
      `- このツールの計算: ${fmtInt(Math.trunc(expected))}`,
      `- 差: ${gap === null ? "—" : `${gap >= 0 ? "+" : ""}${(gap * 100).toFixed(1)}%`}`,
    );
  }
  if (entry.note.trim()) lines.push("", `気づいたこと: ${entry.note.trim()}`);
  lines.push(
    "",
    "計算に使った条件は下の「アプリが自動で付ける情報」に入っています(集計用の JSON)。",
  );

  return {
    kind: "data",
    title: `実測 ${label} / ${conditions.skill.name}`,
    body: lines.join("\n"),
    diagnostics: JSON.stringify(measurementPayload(conditions, entry)),
  };
}

/** 集計する側が読む形。項目が増えても壊れないよう `version` を持つ */
function measurementPayload(conditions: MeasurementConditions, entry: MeasurementEntry) {
  const { result, attack, stats } = conditions;
  return {
    kind: "measurement",
    version: 2,
    character: {
      id: conditions.gameCharacterId,
      awakening: conditions.awakeningStage,
      eternal: conditions.eternalLevel,
      stats,
    },
    skill: {
      id: conditions.skill.id,
      combo_type: conditions.comboSkillType,
      dependency: conditions.skill.dependency,
      multiplier: result?.effective_skill_multiplier ?? conditions.skill.multiplier,
      hits: result?.hit_count ?? conditions.skill.hit_count,
      element: conditions.skill.element,
    },
    target: conditions.content
      ? { listed: true, content: conditions.content.id, enemy: conditions.content.enemyId }
      : { listed: false, name: conditions.unlisted?.name ?? "", place: conditions.unlisted?.place ?? "" },
    combo: conditions.normalAttack
      ? {
          normal_attack: conditions.normalAttack.id,
          interval: conditions.normalAttack.combo_interval,
          cycle_seconds: result?.combo?.seconds ?? null,
        }
      : null,
    // 攻撃力は敵に依らないので、未収録の敵でも入る(逆算の x 軸になる)
    attack: attack
      ? {
          value: attack.value,
          stat_attack: attack.stat_attack,
          equipment_attack: attack.equipment_attack,
          enhance_rate: attack.enhance_rate,
        }
      : null,
    calculated: result
      ? {
          per_hit_max: result.per_hit.max,
          per_hit_critical: result.per_hit.critical,
          total_max: result.total.max,
          total_critical: result.total.critical,
          // 上限に当たっていると実測との突き合わせが線形にならない(docs/enemy-verification.md)
          capped_loss: result.capped_loss,
          damage_cap: result.damage_cap,
        }
      : null,
    measured: {
      damage: entry.damage,
      critical: entry.critical,
      hits: entry.hits,
    },
  };
}
