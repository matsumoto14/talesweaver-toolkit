<script lang="ts">
  // 問い合わせパネル。情報パネルとは分け、右上から直接開く。
  import { onMount, untrack } from "svelte";
  import { errorMessage, getAppInfo, getDamageSnapshot, previewEffectiveStats } from "./api/commands";
  import type { AppInfo } from "./api/types";
  import {
    CHARACTER_ATTACHMENT_MAX, INQUIRY_ENDPOINT, INQUIRY_KINDS, characterAttachment,
    characterAttachmentTooLong, preview, send,
    type InquiryDraft, type InquiryKind, type InquiryResult, type SentInquiry,
  } from "./inquiry";
  import { fmtInt } from "./format";
  import { app, payloadOf, simIsDirty } from "./state.svelte";
  import { reportError } from "./toast.svelte";
  import Modal from "./ui/Modal.svelte";
  import Choose from "./ui/Choose.svelte";
  import ToggleRow from "./ui/ToggleRow.svelte";
  import TextField from "./ui/TextField.svelte";

  let { onClose, prefill = null }: { onClose: () => void; prefill?: InquiryDraft | null } = $props();

  let info = $state<AppInfo | null>(null);
  // 実測から開いたときは中身が入った状態で始める(そのまま送れる。書き足しもできる)。
  // このパネルは開くたびに作り直されるので、下書きは**開いた時点の値**でよい
  // (あとから prefill が変わってユーザーの書きかけを上書きする方が困る)
  const seed = untrack(() => prefill);
  let kind = $state<InquiryKind>(seed?.kind ?? "bug");
  let title = $state(seed?.title ?? "");
  let body = $state(seed?.body ?? "");
  let includeDiagnostics = $state(true);
  let includeCharacter = $state(false);
  let result = $state<InquiryResult | null>(null);
  let sending = $state(false);
  let progress = $state("");
  let sent = $state<SentInquiry | null>(null);

  onMount(() => {
    getAppInfo()
      .then((value) => (info = value))
      .catch((error) => reportError(errorMessage(error)));
  });

  const selected = $derived(app.characters.find((candidate) => candidate.id === app.selectedId));

  /** 調査に効くのに本人が書けない情報だけを集める。個人を特定するものは入れない。 */
  const diagnostics = $derived.by(() => {
    const character = selected;
    const lines = [
      `アプリ: ${info?.version ?? "?"}`,
      `環境: ${navigator.userAgent}`,
      `画面: ${app.tab}`,
    ];
    if (character) {
      lines.push(`選択中のキャラ種: ${character.game_character_id}`);
      lines.push(`覚醒 ${character.awakening.stage} / エタ Lv${character.awakening.eternal_level}`);
      if (character.main_skill_id) lines.push(`主軸スキル: ${character.main_skill_id}`);
      if (simIsDirty()) lines.push("試し変更中");
    }
    if (app.calcTargetId) lines.push(`計算中の対象: ${app.calcTargetId}`);
    // 実測の条件(集計用の JSON)。中継側は診断情報だけをコードブロックに入れるので、
    // 機械で読む値はここに置く(本文の ``` は中継側で潰される)
    if (prefill?.diagnostics) lines.push("", prefill.diagnostics);
    return lines.join("\n");
  });

  // 計算タブで試し変更中なら、画面に出ている数字の元はそちら
  const shared = $derived(selected ? (simIsDirty() && app.sim ? app.sim : payloadOf(selected)) : null);

  // アプリが出していた数字も付ける(再現した値と突き合わせる基準)。取れなくても送信は止めない
  $effect(() => {
    const draftCharacter = shared;
    const id = selected?.id;
    result = null;
    if (!includeCharacter || !draftCharacter || id === undefined) return;
    Promise.all([
      previewEffectiveStats(
        draftCharacter.base_stats, draftCharacter.stat_sources, draftCharacter.equipment,
        draftCharacter.common_skills, draftCharacter.awakening, draftCharacter.game_character_id,
        draftCharacter.main_skill_id, app.calcBuffs,
      ),
      getDamageSnapshot(id),
    ]).then(([stats, snapshot]) => {
      if (shared !== draftCharacter) return;
      result = {
        stats: stats.stats,
        attack: stats.attack?.breakdown ?? null,
        last_damage: snapshot && {
          skill_id: snapshot.skill_id, content_id: snapshot.content_id, per_hit: snapshot.per_hit,
        },
      };
    }).catch(() => {});
  });

  // 量が多く公開のページに載るので、既定では付けない(押した人だけ)。
  const character = $derived(
    includeCharacter && shared ? characterAttachment(shared, app.calcBuffs, result) : "",
  );

  const draft = $derived<InquiryDraft>({ kind, title, body, diagnostics, character });
  /** 添付が上限を超えているか。切って送ると再現できない JSON が載るので、付けずに知らせる */
  const characterTooLong = $derived(characterAttachmentTooLong(character));
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

<Modal label="問い合わせ" class="modal-narrow" closeDisabled={sending} {onClose}>
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

          <Choose label="問い合わせの種類" bind:value={kind} options={INQUIRY_KINDS} full />

          <label class="line">
            <span class="line-label">件名</span>
            <TextField label="件名" bind:value={title} max={120} />
          </label>

          <label class="line">
            <span class="line-label">内容</span>
            <TextField label="内容" bind:value={body} max={4000} rows={5} />
          </label>

          <ToggleRow
            name="バージョンなどの情報を一緒に送る"
            tone="temp"
            on={includeDiagnostics}
            onToggle={() => (includeDiagnostics = !includeDiagnostics)}
          />
          {#if selected}
            <ToggleRow
              name="選択中のキャラのデータを一緒に送る(キャラ名は含めません)"
              tone="temp"
              on={includeCharacter}
              onToggle={() => (includeCharacter = !includeCharacter)}
            />
            {#if characterTooLong}
              <p class="muted warn-line">
                このキャラのデータは上限({fmtInt(CHARACTER_ATTACHMENT_MAX)} 文字)を超えるため<b>添付しません</b>。
                途中で切れたデータでは再現できないので、内容に「どの装備・どのスキルで」を書いてください。
              </p>
            {/if}
          {/if}

          <div class="preview-label">送られる内容</div>
          <div class="preview inset">{preview(draft, includeDiagnostics)}</div>
          <div class="endpoint">送信先: {INQUIRY_ENDPOINT}</div>

          <button type="button" class="btn primary" onclick={submit} disabled={!canSubmit || sending}>
            {sending ? progress || "送信中…" : "この内容で送る"}
          </button>
        {/if}
      </div>
    </div>
</Modal>

<style>
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
