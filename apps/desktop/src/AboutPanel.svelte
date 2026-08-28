<script lang="ts">
  // 情報パネル: 版・非公式表記・出典・データの扱い。
  // 明示クローズ式オーバーレイ(装備登録と同じ形)。背景クリックでは閉じず、
  // 「閉じる ×」か Escape だけで閉じる(§00 押した場所は動かない)。
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

  onMount(() => {
    getAppInfo()
      .then((v) => (info = v))
      .catch((e) => reportError(errorMessage(e)));
  });

  const closeOnEscape = (event: KeyboardEvent) => {
    if (event.key === "Escape" && !sending) onClose();
  };

  // --- 問い合わせ -------------------------------------------------------------
  let kind = $state<InquiryKind>("bug");
  let title = $state("");
  let body = $state("");
  let includeDiagnostics = $state(true);
  let sending = $state(false);
  let progress = $state("");
  let sent = $state<SentInquiry | null>(null);

  /** 調査に効くのに本人が書けない情報だけを集める。個人を特定するものは入れない。 */
  const diagnostics = $derived.by(() => {
    const character = app.characters.find((c) => c.id === app.selectedId);
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
    if (app.calcSkillId) lines.push(`計算中のスキル: ${app.calcSkillId}`);
    if (app.calcTargetId) lines.push(`計算中の対象: ${app.calcTargetId}`);
    return lines.join("\n");
  });

  const draft = $derived<InquiryDraft>({ kind, title, body, diagnostics });
  const canSubmit = $derived(title.trim().length > 0 && body.trim().length > 0);

  async function submit() {
    sending = true;
    progress = "";
    try {
      sent = await send(draft, includeDiagnostics, (m) => (progress = m));
    } catch (e) {
      reportError(errorMessage(e));
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

<div class="modal-overlay about-overlay" role="presentation">
  <div class="panel modal-surface pane-in" role="dialog" aria-modal="true" aria-label="このアプリについて">
    <div class="panel-header">
      <b>このアプリについて</b>
      <button type="button" class="btn" onclick={onClose}>閉じる <span aria-hidden="true">×</span></button>
    </div>

    <div class="panel-body">
      <div class="card">
        <div class="card-title">バージョン</div>
        <div class="version num">{info?.version ?? "—"}</div>
      </div>

      <div class="card warn">
        <div class="card-title">非公式ツールです</div>
        <p>
          TalesWeaver の開発元・運営元とは一切関係がなく、公認・提携・後援を受けていません。
          ゲームクライアントには接続せず、ゲームのファイルも読み書きしません。
        </p>
        <p>
          ゲーム内の名称・用語、および同梱しているアイコン画像の権利は、それぞれの権利者に帰属します。
        </p>
      </div>

      <div class="card">
        <div class="card-title">データの扱い</div>
        <p>
          登録したキャラクター情報は、<b>この PC の中だけ</b>に保存されます。
          自動的に外部へ送られることはありません。
        </p>
        <div class="path inset">{info?.databasePath ?? "—"}</div>
        <p class="muted">
          外部へ送信するのは、問い合わせを送ったときだけです。
          送る内容は送信前に全文表示されます。
        </p>
        <div class="path inset">{INQUIRY_ENDPOINT}</div>
        <p class="muted">
          アップデートのたびに、このファイルのバックアップを直近 3 世代まで自動で保存します。
        </p>
      </div>

      <div class="card">
        <div class="card-title">数値の出典</div>
        <p>
          スキル倍率・敵ステータス・装備補正などは、コミュニティ運営の
          <b>Tale Wiki</b>(talewiki.com)を一次ソースとして取り込んでいます。
        </p>
        <p class="muted">
          wiki に記載が無く、コミュニティの実測値に依っている数値は、画面上で
          <span class="provisional">[仮]</span> と表示しています。
        </p>
      </div>

      <div class="card">
        <div class="card-title">ライセンス</div>
        <p>ソースコードと文書は MIT License。同梱しているゲーム由来の画像・数値データは対象外です。</p>
      </div>

      <!-- 問い合わせ。押した場所より上には何も差し込まない(§00 押した場所は動かない) -->
      <div class="card inquiry">
        <div class="card-title">問い合わせ</div>

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

          <!-- 送る全文は常に出しておく。「確認する」を挟むとボタンが下へ押し出されるうえ、
               1 手増える(§00 押した場所は動かない / 考えさせない) -->
          <div class="preview-label">送られる内容</div>
          <div class="preview inset">{preview(draft, includeDiagnostics)}</div>

          <button type="button" class="btn primary" onclick={submit} disabled={!canSubmit || sending}>
            {sending ? progress || "送信中…" : "この内容で送る"}
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .about-overlay {
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

  .panel-body { overflow-y: auto; padding: 12px 14px 16px; display: flex; flex-direction: column; gap: 10px; }

  .card {
    background: var(--bg-panel); border: 1px solid var(--border-soft);
    border-radius: var(--r-panel); padding: 10px 12px 11px;
  }
  .card.warn { border-color: var(--state-edge-bd); background: var(--state-edge-bg); }
  .card-title {
    font-size: 11px; font-weight: var(--w-strong); color: var(--fg-muted);
    margin-bottom: 6px;
  }
  .card p { font-size: var(--t-body); line-height: 1.65; margin: 0 0 6px; }
  .card p:last-child { margin-bottom: 0; }
  .card p.muted { color: var(--fg-muted); }

  .version { font-size: 20px; font-weight: var(--w-strong); }
  .num { font-family: var(--font-num); font-variant-numeric: tabular-nums; }

  .path {
    padding: 6px 8px; margin: 4px 0 8px;
    font-family: var(--font-num); font-size: 10.5px; color: var(--fg-muted);
    word-break: break-all;
  }

  .provisional { color: var(--accent); font-weight: var(--w-strong); }

  /* --- 問い合わせ --- */
  .inquiry { display: flex; flex-direction: column; gap: 8px; }
  .inquiry .card-title, .inquiry p { margin: 0; }
  .warn-line { color: var(--state-edge-fg); }

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
    white-space: pre-wrap; word-break: break-word;
  }
</style>
