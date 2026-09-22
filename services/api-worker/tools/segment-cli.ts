// 分かち書き CLI。units.py が子プロセスとして呼ぶ(1 プロセスで 18.6 万行を流す。行ごとに
// プロセスを起動しない)。src/segment.ts をそのまま読み込むので、索引(取込)と検索クエリの
// 切れ目が一致する。
//
// 実行: node --experimental-strip-types tools/segment-cli.ts --dict <path>
// --dict: 1 行 1 語のテキスト(units.py が alias から書き出す)
// 標準入力: 1 行 1 テキスト。標準出力: 対応する行に、重複を落としてソートした語の空白区切り
// (0 語なら空行)。行数は必ず入力と一致する。
import { createInterface } from "node:readline";
import { readFileSync } from "node:fs";

import { buildDict, segment } from "../src/segment.ts";

function parseDictPath(argv: string[]): string {
  const i = argv.indexOf("--dict");
  const path = i === -1 ? undefined : argv[i + 1];
  if (path === undefined) {
    throw new Error("使い方: segment-cli.ts --dict <path>");
  }
  return path;
}

function loadDict(path: string): string[] {
  return buildDict(readFileSync(path, "utf-8").split("\n").map((line) => line.trim()));
}

async function main(): Promise<void> {
  const dict = loadDict(parseDictPath(process.argv.slice(2)));
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

  for await (const line of rl) {
    const tokens = [...new Set(segment(line, dict))].sort();
    process.stdout.write(`${tokens.join(" ")}\n`);
  }
}

main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
