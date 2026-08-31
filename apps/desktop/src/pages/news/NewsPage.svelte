<script lang="ts">
  // お知らせ: アプリ自身の更新 + 公開済みの版の更新内容 + これから実装するもの + 既知の不具合。
  // 中身は配信元(R2)の news.json が正で、取れなければ同梱ぶん(src/news.ts)。
  // 更新はここからだけ当てる(勝手に落として当てない)。
  import { onMount } from "svelte";
  import { errorMessage, getAppInfo } from "../../api/commands";
  import { fmtInt, fmtMonthDay } from "../../format";
  import { BUNDLED_NEWS, CHANGE_LABELS, fetchNews, type News } from "../../news";
  import { reportError } from "../../toast.svelte";
  import { installUpdate, restartApp, updater } from "../../update.svelte";
  import { bump } from "../../ui/motion.svelte";

  let news = $state<News>(BUNDLED_NEWS);
  let currentVersion = $state("");

  onMount(() => {
    void fetchNews().then((v) => (news = v));
    getAppInfo()
      .then((info) => (currentVersion = info.version))
      .catch((e) => reportError(errorMessage(e)));
  });
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
    <!-- アプリ自身の更新。最新のときは何も出さない(§00 02 要らないものを見せない) -->
    {#if updater.status !== "idle" && updater.status !== "checking" && updater.status !== "current"}
      <div class="update" class:done={updater.status === "ready"} class:failed={updater.status === "failed"}>
        <div class="update-head">
          <span class="rn-flag update-flag">更新</span>
          {#if updater.status === "ready"}
            <b class="update-title">v{updater.version} を入れました</b>
            <span class="update-note">再起動すると新しい版になります</span>
            <button type="button" class="btn primary" onclick={() => void restartApp()}>再起動して使う</button>
          {:else if updater.status === "failed"}
            <b class="update-title">更新できませんでした</b>
            <span class="update-note">{updater.error}</span>
            <button type="button" class="btn" onclick={() => void installUpdate()}>もう一度</button>
          {:else if updater.status === "available"}
            <b class="update-title">新しい版 v{updater.version} があります</b>
            <span class="update-note">いまの版は v{currentVersion}</span>
            <button type="button" class="btn primary" onclick={() => void installUpdate()}>更新する</button>
          {:else}
            <b class="update-title">v{updater.version} を{updater.status === "installing" ? "入れています" : "落としています"}</b>
            <span class="update-note num" use:bump={() => updater.percent}>
              {updater.percent >= 0 ? `${updater.percent}%` : "…"}
            </span>
          {/if}
        </div>
        {#if updater.status === "downloading" || updater.status === "installing"}
          <div class="meter">
            <div class="fill" style:width={`${updater.percent >= 0 ? updater.percent : 100}%`}></div>
          </div>
        {/if}
        {#if updater.status === "available" && updater.notes}
          <p class="update-body">{updater.notes}</p>
        {/if}
      </div>
    {/if}

    <!-- 版・予定・不具合は同じ畳みの形で並べ、最新の版だけ開いて出す(§00 02)。
         予定と不具合は「まだ版に入っていない」ので破線 + 専用バッジで公開済みと区別する -->
    <div class="section">
      <div class="area-head">
        <span class="area-name">更新内容</span>
        <span class="area-rule"></span>
      </div>
      {#each news.releases as note, index (note.version)}
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

    {#if news.planned.length > 0}
      <div class="section">
        <div class="area-head">
          <span class="area-name">これから</span>
          <span class="rn-flag planned">予定</span>
          <span class="area-rule"></span>
          <span class="rn-count">{fmtInt(news.planned.length)} 件</span>
        </div>
        <div class="rn-list pending">
          {#each news.planned as item (item.text)}
            {@render backlogRow("予定", item.title, item.text)}
          {/each}
        </div>
      </div>
    {/if}

    {#if news.knownIssues.length > 0}
      <div class="section">
        <div class="area-head">
          <span class="area-name">既知の不具合</span>
          <span class="rn-flag issue">不具合</span>
          <span class="area-rule"></span>
          <span class="rn-count">{fmtInt(news.knownIssues.length)} 件</span>
        </div>
        <div class="rn-list pending">
          {#each news.knownIssues as item (item.text)}
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

  /* アプリ自身の更新。押すまで何も起きないので、面は編集できる白 + 操作待ちの金枠 */
  .update {
    display: flex; flex-direction: column; gap: 8px; padding: 11px 13px; border-radius: var(--r-window);
    background: linear-gradient(180deg, #fff, var(--state-edge-bg) 96%); border: 1px solid var(--state-edge-bd);
    box-shadow: inset 0 1px 0 #fff, 0 1px 3px rgba(30, 44, 74, 0.1);
  }
  .update.done { background: linear-gradient(180deg, #fff, var(--state-met-bg) 96%); border-color: var(--state-met-bd); }
  .update.failed { background: linear-gradient(180deg, #fff, var(--state-short-bg) 96%); border-color: var(--state-short-bd); }
  .update-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .update-flag { background: var(--state-edge-bg); border-color: var(--state-edge-bd); color: var(--state-edge-fg); }
  .update-title { flex: none; font-size: 12px; font-weight: 800; color: var(--fg-head); }
  .update-note { min-width: 0; flex: 1; font-size: 10px; color: var(--fg-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .update-head .btn { flex: none; }
  .update .meter > .fill { background: var(--state-edge-bar); }
  .update-body { margin: 0; font-size: 10.5px; color: var(--fg-sub); line-height: 1.6; white-space: pre-wrap; }

  .foot { margin: 0; font-size: 10px; line-height: 1.7; }
</style>
