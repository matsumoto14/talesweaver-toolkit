export const fmtInt = (n: number) => n.toLocaleString("ja-JP");

/** 小数は最大4桁、整数は桁区切りのみ */
export const fmtNum = (n: number) =>
  n.toLocaleString("ja-JP", { maximumFractionDigits: 4 });
