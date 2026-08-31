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
// **1 点では足りない**。敵側は 防御力 C(引き算)・カット率 V(掛け算)・被害減少 M(引き算)で
// 効き方が違い、`y = (x − C) × K − M` の直線になる。傾き K と切片を出すには
// **攻撃力を変えた 2 点以上**が要る(docs/enemy-verification.md)。だから 1 通に複数点を入れる。
//
// 送信経路は問い合わせと同じ中継サーバー(services/inquiry-worker → GitHub Issue)。
// 条件は **1 行の JSON** で診断情報に入れる。中継側が本文の ``` を潰すので本文には置けず、
// 診断情報だけが Issue でコードブロックに収まるため(services/inquiry-worker の clean / renderIssueBody)。
import type { DamageResult, EffectiveStats, Skill } from "./api/types";
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
}

/**
 * 1 点ぶんの測定。**攻撃力と最終能力値も点ごとに持つ** — 装備を替えて攻撃力を変えるのが
 * 2 点測定のやり方なので、条件側に置くと点ごとの違いが消える。
 */
export interface MeasurementSample {
  /** ゲーム内で出た 1 発のダメージ */
  damage: number;
  /** クリティカルだったか */
  critical: boolean;
  /** 撃った回数(同じ条件で何発見て、その最大値を採ったか) */
  hits: number;
  /** 気づいたこと(強打が乗った、上限に当たっていそう、など) */
  note: string;
  /** そのときの攻撃力(A)。逆算の x 軸 */
  attack: number | null;
  /** そのときの最終能力値 */
  stats: EffectiveStats | null;
  /** そのときのツールの計算値(敵が未収録なら null) */
  expected: number | null;
  /** そのとき装備していた武器の名前。点ごとに何を替えたのかが分かるように残す */
  weapon: string | null;
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

/**
 * 防御力とカット率を分けて逆算できるか。
 * **攻撃力が違う点が 2 つ以上**必要(同じ攻撃力を何度測っても直線は引けない)。
 */
export const canSeparate = (samples: MeasurementSample[]): boolean =>
  new Set(samples.map((s) => s.attack).filter((a): a is number => a !== null)).size >= 2;

/** 中継サーバーに送る下書き。送信前にそのまま全文が表示される(問い合わせと同じ作法) */
export function measurementDraft(
  conditions: MeasurementConditions,
  samples: MeasurementSample[],
): InquiryDraft {
  const label = targetLabel(conditions);
  const listed = conditions.content !== null;
  const lines = [
    `${label} を ${conditions.skill.name} で殴った実測 ${samples.length} 点です。`,
    "",
    listed ? "| 武器 | 攻撃力 | 実測 | 計算 | 差 | 発数 |" : "| 武器 | 攻撃力 | 実測 | 発数 |",
    listed ? "|---|---:|---:|---:|---:|---:|" : "|---|---:|---:|---:|",
  ];
  for (const sample of samples) {
    const attack = sample.attack !== null ? fmtInt(sample.attack) : "—";
    const damage = `${fmtInt(sample.damage)}${sample.critical ? "(クリ)" : ""}`;
    const weapon = sample.weapon ?? "—";
    if (listed) {
      const gap = damageGap(sample.damage, sample.expected);
      lines.push(
        `| ${weapon} | ${attack} | ${damage} | ${sample.expected !== null ? fmtInt(Math.trunc(sample.expected)) : "—"} `
        + `| ${gap === null ? "—" : `${gap >= 0 ? "+" : ""}${(gap * 100).toFixed(1)}%`} | ${fmtInt(sample.hits)} |`,
      );
    } else {
      lines.push(`| ${weapon} | ${attack} | ${damage} | ${fmtInt(sample.hits)} |`);
    }
  }
  if (!listed) {
    lines.push("", "この敵はまだ収録していないので、ツールの計算値はありません。");
    if (conditions.unlisted?.place.trim()) lines.push(`出た場所: ${conditions.unlisted.place.trim()}`);
  }
  lines.push(
    "",
    canSeparate(samples)
      ? "攻撃力の違う点が 2 つ以上あるので、防御力とカット率を分けて逆算できます。"
      : "攻撃力が同じ点だけなので、防御力とカット率は分けられません(装備を替えてもう 1 点あると分けられます)。",
  );
  const notes = samples.map((s) => s.note.trim()).filter(Boolean);
  if (notes.length > 0) lines.push("", `気づいたこと: ${notes.join(" / ")}`);
  lines.push("", "条件は下の「アプリが自動で付ける情報」に入っています(集計用の JSON)。");

  return {
    kind: "data",
    title: `実測 ${label} / ${conditions.skill.name}(${samples.length} 点)`,
    body: lines.join("\n"),
    diagnostics: JSON.stringify(measurementPayload(conditions, samples)),
  };
}

/** 集計する側が読む形。項目が増えても壊れないよう `version` を持つ */
function measurementPayload(conditions: MeasurementConditions, samples: MeasurementSample[]) {
  return {
    kind: "measurement",
    version: 3,
    character: {
      id: conditions.gameCharacterId,
      awakening: conditions.awakeningStage,
      eternal: conditions.eternalLevel,
    },
    skill: {
      id: conditions.skill.id,
      combo_type: conditions.comboSkillType,
      dependency: conditions.skill.dependency,
      multiplier: conditions.skill.multiplier,
      hits: conditions.skill.hit_count,
      element: conditions.skill.element,
    },
    target: conditions.content
      ? { listed: true, content: conditions.content.id, enemy: conditions.content.enemyId }
      : { listed: false, name: conditions.unlisted?.name ?? "", place: conditions.unlisted?.place ?? "" },
    samples: samples.map((sample) => ({
      damage: sample.damage,
      critical: sample.critical,
      hits: sample.hits,
      attack: sample.attack,
      stats: sample.stats,
      expected: sample.expected,
      weapon: sample.weapon,
      note: sample.note.trim() || null,
    })),
  };
}
