/**
 * wasm-pack が生成する glue(crates/web/pkg)の型。生成物はビルド時にしか無いので、
 * 型はここに手で置く(生成前でも svelte-check が通るようにするため)。
 * 実体の解決は vite.web.config.ts の alias。
 */
declare module "tw-web" {
  export default function init(): Promise<unknown>;
  export function invoke(command: string, args: unknown): unknown;
}
