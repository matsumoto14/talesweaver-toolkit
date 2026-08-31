// 実測ダメージの送信。ゲーム内で実際に出たダメージを、計算に使った条件ごと集める。
//
// 敵の防御力・カット率・被害減少は wiki が「約」「推定値」としか書いておらず、
// ゲーム内でも見られない。**実測から逆算するしか確かめる方法がない**(手順は
// docs/enemy-verification.md)。集めた実測を突き合わせて `[仮]` を外していく。
//
// 送信経路は問い合わせと同じ中継サーバー(services/inquiry-worker → GitHub Issue)。
// 条件は **1 行の JSON** で診断情報に入れる。中継側が本文の ``` を潰すので本文には置けず、
// 診断情報だけが Issue でコードブロックに収まるため(services/inquiry-worker の clean / renderIssueBody)。
import type { DamageResult, Skill } from "./api/types";
import { fmtInt } from "./format";
import type { InquiryDraft } from "./inquiry";

export interface MeasurementConditions {
  gameCharacterId: string;
  awakeningStage: number;
  eternalLevel: number;
  skill: Skill;
  comboSkillType: string | null;
  contentId: string;
  contentName: string;
  /** 敵の収録値そのものは送らない。id とアプリの版が分かれば gamedata から引けるため */
  enemyId: string | null;
  /** コンボで挟んでいる通常攻撃(コンボしていないなら null) */
  normalAttack: Skill | null;
  result: DamageResult;
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

/** 計算値のうち、実測と突き合わせる側の値 */
export const expectedDamage = (result: DamageResult, critical: boolean): number =>
  critical ? result.per_hit.critical : result.per_hit.max;

/** 実測と計算の差(割合)。計算が 0 のときは null */
export const damageGap = (measured: number, expected: number): number | null =>
  expected > 0 ? measured / expected - 1 : null;

/** 中継サーバーに送る下書き。送信前にそのまま全文が表示される(問い合わせと同じ作法) */
export function measurementDraft(
  conditions: MeasurementConditions,
  entry: MeasurementEntry,
): InquiryDraft {
  const expected = expectedDamage(conditions.result, entry.critical);
  const gap = damageGap(entry.damage, expected);
  const lines = [
    `${conditions.contentName} を ${conditions.skill.name} で殴った実測です。`,
    "",
    `- 実測: ${fmtInt(entry.damage)}(${entry.critical ? "クリティカル" : "非クリティカル"}・${fmtInt(entry.hits)} 発中の最大)`,
    `- このツールの計算: ${fmtInt(Math.trunc(expected))}`,
    `- 差: ${gap === null ? "—" : `${gap >= 0 ? "+" : ""}${(gap * 100).toFixed(1)}%`}`,
  ];
  if (entry.note.trim()) lines.push("", `気づいたこと: ${entry.note.trim()}`);
  lines.push(
    "",
    "計算に使った条件は下の「アプリが自動で付けた情報」に入っています(集計用の JSON)。",
  );

  return {
    kind: "data",
    title: `実測 ${conditions.contentName} / ${conditions.skill.name}`,
    body: lines.join("\n"),
    diagnostics: JSON.stringify(measurementPayload(conditions, entry)),
  };
}

/** 集計する側が読む形。行が増えても壊れないよう、値は素直な入れ子にする */
function measurementPayload(conditions: MeasurementConditions, entry: MeasurementEntry) {
  const { result } = conditions;
  return {
    kind: "measurement",
    version: 1,
    character: {
      id: conditions.gameCharacterId,
      awakening: conditions.awakeningStage,
      eternal: conditions.eternalLevel,
    },
    skill: {
      id: conditions.skill.id,
      combo_type: conditions.comboSkillType,
      multiplier: result.effective_skill_multiplier,
      hits: result.hit_count,
    },
    target: {
      content: conditions.contentId,
      enemy: conditions.enemyId,
    },
    combo: conditions.normalAttack
      ? {
          normal_attack: conditions.normalAttack.id,
          interval: conditions.normalAttack.combo_interval,
          cycle_seconds: result.combo?.seconds ?? null,
        }
      : null,
    calculated: {
      attack: result.trace.attack.value,
      per_hit_max: result.per_hit.max,
      per_hit_critical: result.per_hit.critical,
      total_max: result.total.max,
      total_critical: result.total.critical,
      // 上限に当たっていると実測との突き合わせが線形にならない(docs/enemy-verification.md)
      capped_loss: result.capped_loss,
      damage_cap: result.damage_cap,
    },
    measured: {
      damage: entry.damage,
      critical: entry.critical,
      hits: entry.hits,
    },
  };
}
