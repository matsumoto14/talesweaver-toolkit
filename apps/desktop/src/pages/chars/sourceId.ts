// 補正源 16 種の識別子。SourcePane.svelte とその子ペイン(pages/chars/sources/*)、
// Workspace.svelte が共有する。SourcePane.svelte の <script module> から re-export しているので
// 既存の `import { type SourceId } from "./SourcePane.svelte"` は変わらず使える。
export type SourceId =
  | "status"
  | "equipment"
  | "pet"
  | "rune"
  | "crown"
  | "monsterCard"
  | "relic"
  | "siena"
  | "randomOption"
  | "title"
  | "commonSkill"
  | "actualDelay"
  | "criticalRate"
  | "thesis"
  | "skills";
