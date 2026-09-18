/**
 * ブラウザ版の復旧(alias は vite.web.config.ts)。保存先の IndexedDB を消して開き直せるようにする。
 *
 * ブラウザ版の保存データはサイトデータ削除で消える前提のもので、バックアップは書き出し JSON。
 * 移行の途中で壊れるなどして開けなくなったとき、端末の設定からサイトデータを探させるのではなく、
 * アプリ自身が消して復帰できる経路を持つ(2026-09-18)。消すのは人が押したときだけ。
 */
import { deleteDatabase } from "../api/browserStore";

export const CAN_RESET_STORE = true;

export async function resetStore(): Promise<void> {
  await deleteDatabase();
}
