/**
 * 出典 URL の組み立て。EUC-JP のパーセントエンコーディングは units.py 側で作って
 * page.url に保存済み(TextEncoder では EUC-JP に符号化できないため)。
 * Worker はそれとアンカーを連結するだけ。
 */
export function wikiUrl(pageUrl: string, anchor: string): string {
  if (!anchor || anchor === "top") return pageUrl;
  return `${pageUrl}#${anchor}`;
}
