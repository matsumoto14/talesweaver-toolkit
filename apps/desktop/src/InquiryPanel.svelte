<script lang="ts">
  // 問い合わせパネル。情報パネルとは分け、右上から直接開く。
  import { onMount } from "svelte";
  import { errorMessage, getAppInfo } from "./api/commands";
  import type { AppInfo } from "./api/types";
  import {
    INQUIRY_ENDPOINT, INQUIRY_KINDS, preview, send,
    type InquiryDraft, type InquiryKind, type SentInquiry,
  } from "./inquiry";
  import { app } from "./state.svelte";
  import { reportError } from "./toast.svelte";
  import StepSelect from "./ui/StepSelect.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let info = $state<AppInfo | null>(null);
  let kind = $state<InquiryKind>("bug");
  let title = $state("");
  let body = $state("");
  let includeDiagnostics = $state(true);
  let sending = $state(false);
  let progress = $state("");
  let sent = $state<SentInquiry | null>(null);

  onMount(() => {
    getAppInfo()
      .then((value) => (info = value))
      .catch((error) => reportError(errorMessage(error)));
  });

  const closeOnEscape = (event: KeyboardEvent) => {
    if (event.key === "Escape" && !sending) onClose();
  };

  /** 調査に効くのに本人が書けない情報だけを集める。個人を特定するものは入れない。 */
  const diagnostics = $derived.by(() => {
    const character = app.characters.find((candidate) => candidate.id === app.selectedId);
    const lines = [
      `アプリ: ${info?.version ?? "?"}`,
      `環境: ${navigator.userAgent}`,
      `画面: ${app.tab}`,
    ];
    if (character) {
      lines.push(`選択中のキャラ種: ${character.game_character_id}`);
      lines.push(`覚醒 ${character.awakening.stage} / エタ Lv${character.awakening.eternal_level}`);
      if (character.main_skill_id) lines.push(`主軸スキル: ${character.main_skill_id}`);
    }
    if (app.calcTargetId) lines.push(`計算中の対象: ${app.calcTargetId}`);
    return lines.join("\n");
  });

  const draft = $derived<InquiryDraft>({ kind, title, body, diagnostics });
  const canSubmit = $derived(title.trim().length > 0 && body.trim().length > 0);

  async function submit() {
    sending = true;
    progress = "";
    try {
      sent = await send(draft, includeDiagnostics, (message) => (progress = message));
    } catch (error) {
      reportError(errorMessage(error));
    } finally {
      sending = false;
      progress = "";
    }
  }

  function reset() {
    sent = null;
    title = "";
    body = "";
  }
</script>

<svelte:window onkeydown={closeOnEscape} />

<div class="modal-overlay inquiry-overlay" role="presentation">
  <div class="panel modal-surface pane-in" role="dialog" aria-modal="true" aria-label="問い合わせ">
    <div class="panel-header">
      <b>問い合わせ</b>
      <button type="button" class="btn" onclick={onClose} disabled={sending}>閉じる <span aria-hidden="true">×</span></button>
    </div>

    <div class="panel-body">
      <div class="card inquiry">
        {#if sent}
          <p>送信しました。やり取りはこのページで行います。</p>
          <div class="path inset">{sent.url}</div>
          <p class="muted">アプリからは返信を受け取れないので、この URL を控えてください。</p>
          <button type="button" class="btn" onclick={reset}>続けて送る</button>
        {:else}
          <p class="muted warn-line">
            送った内容は<b>公開のページに載ります</b>。本名・メールアドレス・ゲーム内 ID は書かないでください。
          </p>

          <StepSelect bind:value={kind} options={INQUIRY_KINDS} full />

          <label class="line">
            <span class="line-label">件名</span>
            <input type="text" bind:value={title} maxlength="120" placeholder="例) 極・連撃のダメージが 0 になる" />
          </label>

          <label class="line">
            <span class="line-label">内容</span>
            <textarea bind:value={body} maxlength="4000" rows="5" placeholder="どう操作すると起きるか、本当はどうなるはずかを書いてください"></textarea>
          </label>

          <label class="diag-toggle">
            <input type="checkbox" bind:checked={includeDiagnostics} />
            バージョンなどの情報を一緒に送る
          </label>

          <div class="preview-label">送られる内容</div>
          <div class="preview inset">{preview(draft, includeDiagnostics)}</div>
          <div class="endpoint">送信先: {INQUIRY_ENDPOINT}</div>

          <button type="button" class="btn primary" onclick={submit} disabled={!canSubmit || sending}>
            {sending ? progress || "送信中…" : "この内容で送る"}
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .inquiry-overlay {
    z-index: 90; padding: 3vh max(14px, 6vw);
    display: flex; justify-content: center; align-items: flex-start;
  }
  .panel { width: min(560px, 100%); max-height: 94vh; display: flex; flex-direction: column; }

  .panel-header {
    flex-shrink: 0; display: flex; align-items: center; gap: 12px;
    padding: 11px 14px; border-bottom: 1px solid var(--border-soft);
  }
  .panel-header b { font-size: var(--t-heading); }
  .panel-header .btn { margin-left: auto; }

  .panel-body { overflow-y: auto; padding: 12px 14px 16px; }
  .card {
    background: var(--bg-panel); border: 1px solid var(--border-soft);
    border-radius: var(--r-panel); padding: 10px 12px 11px;
  }
  .inquiry { display: flex; flex-direction: column; gap: 8px; }
  .inquiry p { margin: 0; font-size: var(--t-body); line-height: 1.65; }
  .inquiry p.muted { color: var(--fg-muted); }
  .warn-line { color: var(--state-edge-fg); }

  .path {
    padding: 6px 8px; margin: 4px 0 8px;
    font-family: var(--font-num); font-size: 10.5px; color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    word-break: break-all;
  }
  .line { display: flex; flex-direction: column; gap: 3px; }
  .line-label { font-size: var(--t-label); color: var(--fg-muted); }
  .line input, .line textarea {
    width: 100%; padding: 6px 8px;
    background: var(--bg-field); border: 1px solid var(--border); border-radius: var(--r-inset);
    font-family: inherit; font-size: var(--t-body); color: var(--fg);
  }
  .line textarea { resize: vertical; line-height: 1.6; }
  .line input:focus, .line textarea:focus { outline: 2px solid var(--accent); outline-offset: -1px; }

  .diag-toggle {
    display: flex; align-items: center; gap: 6px;
    font-size: var(--t-label); color: var(--fg-muted);
  }
  .preview-label { font-size: var(--t-label); color: var(--fg-muted); }
  .preview {
    padding: 8px 10px; max-height: 170px; overflow-y: auto;
    font-family: var(--font-num); font-size: 11px; line-height: 1.6;
    font-variant-numeric: tabular-nums;
    white-space: pre-wrap; word-break: break-word;
  }
  .endpoint {
    font-family: var(--font-num); font-size: 9.5px; color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
</style>
