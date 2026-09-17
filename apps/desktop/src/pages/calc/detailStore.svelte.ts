// 「押した数値の内訳」の開閉と「直近で何が変わったか」を 1 か所で持つ。
// 鎖(1 発 / 合計 / DPS)と「なぜこの数字?」の帯は別の入れ物にいるが、どちらも同じ内訳の面を
// 開き、↑ を押すと変わった行だけを辿る(緑を辿る。ユーザー要望 2026-09-15)ので、
// 開いている面・変わった子の控えはこの 1 つを共有する。
import { tick } from "svelte";
import type { DamageContribution } from "../../api/types";
import { ChangeMemo, PresenceMemo, type Presence } from "../../ui/presence";

/** 内訳 1 行。列は band-row と同じ段(ラベル / 倍率 / 実数 / 補足) */
export interface Mat {
  label: string;
  /** ラベルの前に小さく置く番号など(「2.」)。行の主語ではないので薄く出す */
  prefix?: string;
  mult?: string;
  value: string;
  sub?: string;
  /** `value` の数値(表示単位。% 表示なら 100 倍した値)。変わったら動かす + いくつ変わったかを出す(§00 04)ためだけに使う */
  n?: number;
  /** 差分タグに付ける単位("%" など)。`n` が `value` と同じ単位のときだけ */
  unit?: string;
  /** 差分タグの小数桁 */
  digits?: number;
  /** 押すと直下に `subs` が開く行。省略なら開かない行 */
  key?: string;
  subs?: Mat[];
  /** 供給源の行の出入り。抜けた行は次に集合が変わるまで残す(ui/presence.ts) */
  state?: Presence;
  /** 供給源の行の一意キー(`{#each}` 用) */
  id?: string;
  /** 直近の計算で値か供給源が変わった行。↑ を押して辿る先(ui/presence.ts ChangeMemo) */
  changed?: boolean;
  /** 直近で入れ替わった供給源(「称号【A】 → 称号【B】」)。閉じたままでもどこに効いたか分かる */
  note?: string;
}

export interface Detail {
  /** この段の倍率(×n) */
  mult: string;
  /** この段が足した実数(+n / −n)。倍率だけの段は null */
  delta: number | null;
  /** その段までの到達値 */
  to: number | null;
  mats: Mat[];
  /** 中立(±0)で出さなかったカテゴリ枠の数 */
  idle: number;
  /** Rust の式(FormulaStep.expression) */
  expr: string | null;
  /** 倍率 / 実数 / 結果の見出しを出すか。段を持たない一覧(次の候補)では出さない */
  head?: boolean;
  /** 一覧の下に付ける読み方の 1 行 */
  note?: string;
}

export class DetailStore {
  /** 開いている内訳。行ごとに独立させる(1 つ開いても他は閉じない ＝ 押した行が動かない) */
  openDetails = $state<string[]>([]);
  /** 「なぜこの数字?」を開いているか。鎖の ↑ から辿るときはこの面を開いてから中へ降りる */
  flowOpen = $state(false);
  /** 「直近の計算で変わった行」の判定(ui/presence.ts) */
  readonly changes = new ChangeMemo();
  /** 供給源の行の出入りをカテゴリごとに覚える(reactive にしない) */
  readonly contributions = new PresenceMemo<DamageContribution>();
  /** 親キー → その下で変わった子キー。描画時に控え、↑ を押したときに辿る */
  private children = new Map<string, string[]>();

  isOpen(key: string): boolean {
    return this.openDetails.includes(key);
  }

  /** ui/Disclosure の bind:open 用。開閉そのものは <details> が持ち、ここは覚えるだけ */
  setOpen(key: string, open: boolean) {
    if (open === this.isOpen(key)) return;
    this.openDetails = open ? [...this.openDetails, key] : this.openDetails.filter((k) => k !== key);
  }

  toggle(key: string) {
    this.setOpen(key, !this.isOpen(key));
  }

  /** 描画のついでに「この親の下で変わった子」を控える */
  register(parentKey: string, d: Detail): Detail {
    this.setChildren(parentKey, d.mats.filter((m) => m.changed && m.key).map((m) => m.key!));
    return d;
  }

  setChildren(parentKey: string, keys: string[]) {
    this.children.set(parentKey, keys);
  }

  /** ↑↓ を押すと、その下で変わった行だけを順に開く */
  async follow(key: string) {
    // 鎖の 1 発 / 攻撃力は「なぜこの数字?」の面が入口。面を開いてから中の行へ降りる
    if (key === "perHit" || key === "atkA") this.flowOpen = true;
    else if (!this.isOpen(key)) this.openDetails = [...this.openDetails, key];
    await tick(); // 開いて描画されてから、その中で控えた「変わった子」を読む
    for (const child of this.children.get(key) ?? []) await this.follow(child);
  }
}
