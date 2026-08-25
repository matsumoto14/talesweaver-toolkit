// 状態の 6 系統(design-system §03)。到達判定のバッジ・量バーはここから選ぶ。
// 色の実値は app.css の --state-* が持つ。ここは CSS 変数参照だけを配って、
// 画面ごとのラベル(「余裕」「通る」「入場OK」…)は各画面が付ける。
// 新しい状態を足したくなったら、まずこの 6 つのどれかに寄せられないか考える。

/** 余裕・目標 / 足りている / ぎりぎり・操作待ち / 届かない・危険 / 対象外・判定不能 / 一時・チーム条件 */
export type StateKey = "goal" | "met" | "edge" | "short" | "unknown" | "temp";

export interface StateColors {
  /** バッジの地 */
  bg: string;
  /** バッジの枠 */
  bd: string;
  /** バッジの文字 */
  fg: string;
  /** 量バーの塗り */
  bar: string;
}

const of = (key: StateKey): StateColors => ({
  bg: `var(--state-${key}-bg)`,
  bd: `var(--state-${key}-bd)`,
  fg: `var(--state-${key}-fg)`,
  bar: `var(--state-${key}-bar)`,
});

export const STATE: Record<StateKey, StateColors> = {
  goal: of("goal"),
  met: of("met"),
  edge: of("edge"),
  short: of("short"),
  unknown: of("unknown"),
  temp: of("temp"),
};

/** バッジ 1 種 = 画面ごとの言葉 + 6 系統のどれか。 */
export interface Badge {
  label: string;
  state: StateKey;
}

export const badgeStyle = (b: Badge): string => {
  const c = STATE[b.state];
  return `background: ${c.bg}; border-color: ${c.bd}; color: ${c.fg};`;
};
