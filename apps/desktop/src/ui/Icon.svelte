<script lang="ts" module>
  // アイコンの唯一の部品(規格シート 3a)。4 系統を枠の色と角丸で識別し、サイズは 4 段に固定する。
  // 画像は `assets/icons/<系統>/<id>.png` を **gamedata の id から機械的に解決**する
  // (手動のマッピング表は作らない)。解決できない id は破線 + `?` で「データが無い」と示し、
  // 空白にはしない。サイズは固定なので実画像 0 枚でもレイアウトは崩れない。
  //
  // **アイコン単独表示は禁止**(名前と併記する)。例外はキャラレールを畳んだときだけで、
  // そのときは呼び出し側が title を付ける。
  export type IconKind = "character" | "mob" | "skill" | "buff" | "mastery";
  /** 20 = 行内・チップ / 28 = 一覧行 / 40 = 選択カード / 64 = キャラ詳細 */
  export type IconSize = 20 | 28 | 40 | 64;

  const DIRS: Record<IconKind, string> = {
    character: "characters",
    mob: "mobs",
    skill: "skills",
    buff: "buffs",
    mastery: "masteries",
  };

  /// 枠の見た目。マスタリーはスキルの一種なのでスキルの枠を使う(段は色で区別しない)
  const FRAMES: Record<IconKind, string> = {
    character: "character",
    mob: "mob",
    skill: "skill",
    buff: "buff",
    mastery: "skill",
  };

  // Vite の glob import。実画像が 1 枚も無ければ空オブジェクトになるだけで、ビルドは通る。
  const FILES = import.meta.glob("../assets/icons/*/*.png", {
    eager: true,
    query: "?url",
    import: "default",
  }) as Record<string, string>;

  // 同じ id で何度も warn しない(一覧の再描画ごとに出すとログが埋まる)
  const warned = new Set<string>();

  export function iconUrl(kind: IconKind, id: string): string | null {
    const path = `../assets/icons/${DIRS[kind]}/${id}.png`;
    const url = FILES[path] ?? null;
    if (url === null) {
      const key = `${kind}/${id}`;
      if (!warned.has(key)) {
        warned.add(key);
        console.warn(`[icon] 画像がありません: src/assets/icons/${DIRS[kind]}/${id}.png`);
      }
    }
    return url;
  }
</script>

<script lang="ts">
  interface Props {
    kind: IconKind;
    /** gamedata の id。`null` = そもそも対象が無い枠(警告を出さず縞のまま) */
    id: string | null;
    size?: IconSize;
    /** 読み上げ・title に使う名前。単独表示のとき(レール折りたたみ)は必須 */
    label: string;
  }
  let { kind, id, size = 28, label }: Props = $props();

  const url = $derived(id === null ? null : iconUrl(kind, id));
  const missing = $derived(id !== null && url === null);
</script>

<span
  class="icon {FRAMES[kind]}"
  class:missing
  style="--icon-size: {size}px"
  role="img"
  aria-label={label}
  title={label}
>
  {#if url}
    <img src={url} alt="" />
  {:else if missing}
    <span class="q" aria-hidden="true">?</span>
  {/if}
</span>

<style>
  .icon {
    width: var(--icon-size); height: var(--icon-size);
    flex-shrink: 0; display: flex; align-items: center; justify-content: center; overflow: hidden;
    border: 1px solid; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.75);
    font-size: calc(var(--icon-size) * 0.42); font-weight: 700; line-height: 1;
  }
  .icon img { width: 100%; height: 100%; object-fit: cover; display: block; }
  /* 実画像が来るまでの縞プレースホルダ。系統ごとに地の色を変える */
  .icon.character {
    border-radius: var(--r-window); border-color: var(--border-strong); color: var(--fg-sub);
    background: repeating-linear-gradient(135deg, #E4EDF9 0 4px, #CFDFF2 4px 8px);
  }
  .icon.mob {
    border-radius: var(--r-window); border-color: var(--mob); color: #6B4F49;
    background: repeating-linear-gradient(135deg, #F3E7E4 0 4px, #E6D3CD 4px 8px);
  }
  .icon.skill {
    border-radius: var(--r-panel); border-color: var(--gold); color: #7A5F22;
    background: repeating-linear-gradient(135deg, #FBF2DE 0 4px, #F0E2C2 4px 8px);
  }
  .icon.buff {
    border-radius: var(--r-pill); border-color: var(--sim); color: var(--sim-fg);
    background: repeating-linear-gradient(135deg, #F2F1FA 0 4px, #E4E2F2 4px 8px);
  }
  /* 解決できない id。空白にせず破線 + ? で「データが無い」と分かるようにする */
  .icon.missing { border-style: dashed; box-shadow: none; }
</style>
