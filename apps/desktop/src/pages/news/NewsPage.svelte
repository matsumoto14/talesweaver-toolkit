<script lang="ts">
  // お知らせ: 公開済みの版の更新内容 + これから実装するもの + 既知の不具合。
  // データは src/releaseNotes.ts(正は CHANGELOG.md)。この画面は表示と開閉だけで、通信も判定もしない。
  import { fmtInt, fmtMonthDay } from "../../format";
  import { CHANGE_LABELS, KNOWN_ISSUES, PLANNED, RELEASE_NOTES } from "../../releaseNotes";
</script>

{#snippet backlogRow(label: string, title: string | undefined, text: string)}
  <div class="rn-row">
    <span class="tag">{label}</span>
    {#if title}<b class="rn-title">{title}</b>{/if}
    <span class="rn-text">{text}</span>
  </div>
{/snippet}

<div class="news">
  <div class="scroll">
    <!-- 版・予定・不具合は同じ畳みの形で並べ、最新の版だけ開いて出す(§00 02)。
         予定と不具合は「まだ版に入っていない」ので破線 + 専用バッジで公開済みと区別する -->
    <div class="section">
      <div class="area-head">
        <span class="area-name">更新内容</span>
        <span class="area-rule"></span>
      </div>
      {#each RELEASE_NOTES as note, index (note.version)}
        <details class="fold rn-fold" open={index === 0}>
          <summary>
            <span class="rn-version">v{note.version}</span>
            <span class="rn-date num">{fmtMonthDay(note.date)}</span>
            {#if note.headline}<span class="rn-headline">{note.headline}</span>{/if}
            <span class="rn-count">{fmtInt(note.changes.length)} 件</span>
          </summary>
          <div class="fold-body rn-list">
            {#each note.changes as change (change.text)}
              {@render backlogRow(CHANGE_LABELS[change.kind], change.title, change.text)}
            {/each}
          </div>
        </details>
      {/each}
    </div>

    {#if PLANNED.length > 0}
      <div class="section">
        <div class="area-head">
          <span class="area-name">これから</span>
          <span class="rn-flag planned">予定</span>
          <span class="area-rule"></span>
          <span class="rn-count">{fmtInt(PLANNED.length)} 件</span>
        </div>
        <div class="rn-list pending">
          {#each PLANNED as item (item.text)}
            {@render backlogRow("予定", item.title, item.text)}
          {/each}
        </div>
      </div>
    {/if}

    {#if KNOWN_ISSUES.length > 0}
      <div class="section">
        <div class="area-head">
          <span class="area-name">既知の不具合</span>
          <span class="rn-flag issue">不具合</span>
          <span class="area-rule"></span>
          <span class="rn-count">{fmtInt(KNOWN_ISSUES.length)} 件</span>
        </div>
        <div class="rn-list pending">
          {#each KNOWN_ISSUES as item (item.text)}
            {@render backlogRow("不具合", item.title, item.text)}
          {/each}
        </div>
      </div>
    {/if}

    <p class="foot dim">
      更新内容はツール自身の変更履歴です。ゲーム側の情報(公式・韓国)はまだ扱っていません。
    </p>
  </div>
</div>

<style>
  .news { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 16px 22px 22px; display: flex; flex-direction: column; gap: 14px; max-width: 940px; }

  /* 見出し・帯ラベルはホームと同じ形(§00 01 視線を動かさない) */
  .section { display: flex; flex-direction: column; gap: 6px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: var(--r-inset); background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }
  .tag {
    flex: none; width: 52px; text-align: center; padding: 1px 0; border-radius: var(--r-pill);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted); white-space: nowrap;
  }

  .rn-fold:first-of-type { margin-top: 0; padding-top: 0; border-top: none; }
  .rn-list { display: flex; flex-direction: column; gap: 6px; }
  .rn-row {
    display: flex; align-items: flex-start; gap: 9px; padding: 7px 12px; border-radius: var(--r-window);
    background: var(--bg-field); border: 1px solid var(--border-soft);
  }
  .rn-row .tag { margin-top: 1px; }
  .rn-title { flex: none; font-size: 11px; font-weight: 700; color: var(--fg); line-height: 1.5; }
  .rn-text { min-width: 0; flex: 1; font-size: 11px; color: var(--fg-sub); line-height: 1.5; }
  .rn-version {
    flex: none; padding: 1px 8px; border-radius: var(--r-pill);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
    font-size: 9px; font-weight: 700; color: var(--fg-muted); white-space: nowrap;
  }
  .rn-date { flex: none; font-size: 9.5px; color: var(--fg-dim); }
  .rn-headline { min-width: 0; font-size: 10px; color: var(--fg-dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rn-count { flex: none; margin-left: auto; font-size: 9.5px; color: var(--fg-dim); }

  /* まだ版に入っていないもの(予定・不具合)。§03 の 6 系統のバッジ + 破線で、公開済みの版と読み違えないようにする */
  .rn-flag { flex: none; padding: 1px 8px; border-radius: var(--r-pill); border: 1px solid; font-size: 9px; font-weight: 700; white-space: nowrap; }
  .rn-flag.planned { background: var(--state-goal-bg); border-color: var(--state-goal-bd); color: var(--state-goal-fg); }
  .rn-flag.issue { background: var(--state-short-bg); border-color: var(--state-short-bd); color: var(--state-short-fg); }
  .rn-list.pending .rn-row { border-style: dashed; background: var(--bg-panel); }

  .foot { margin: 0; font-size: 10px; line-height: 1.7; }
</style>
