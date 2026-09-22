// 結論文(lead)セグメント → 画面が描く部品配列への変換。純関数(vitest で検証する想定 —
// apps/desktop には vitest が入っていないため、この段階ではテストを追加していない。報告に明記)。
//
// `ref` は候補ユニットの ID、`col` はその列名。訂正があれば訂正後の値に差し替える
// (s12 answer.ts の「lead の参照は訂正後の値で埋める」を画面側でやる版)。
import type { Correction, LeadSeg, Unit } from "../../ask";

export interface LeadPart {
  /** 地の文の断片 */
  text?: string;
  /** `<Value>` で数値書体にする値(訂正があれば訂正後) */
  value?: string;
}

export function renderLead(
  segments: LeadSeg[],
  unitsById: Map<string, Unit>,
  corrections: Correction[],
): LeadPart[] {
  return segments.map((seg): LeadPart => {
    if ("t" in seg) return { text: seg.t };
    const correction = corrections.find((c) => c.unit === seg.ref && c.col === seg.col);
    if (correction) return { value: correction.value };
    const unit = unitsById.get(seg.ref);
    const value = unit && unit.kind === "row" ? unit.cells[seg.col] : undefined;
    return { value: value ?? "?" };
  });
}

/** 応答内の全手順のユニットを ID で引けるようにする(lead の参照先探し用) */
export function collectUnits(steps: { units: Unit[] }[]): Map<string, Unit> {
  const map = new Map<string, Unit>();
  for (const step of steps) {
    for (const unit of step.units) map.set(unit.id, unit);
  }
  return map;
}
