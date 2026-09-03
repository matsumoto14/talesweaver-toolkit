<script lang="ts">
  // 情報パネル: 版・非公式表記・出典・データの扱い。
  // 明示クローズ式オーバーレイ(装備登録と同じ形)。背景クリックでは閉じず、
  // 「閉じる ×」か Escape だけで閉じる(§00 押した場所は動かない)。
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import authorPortrait from "./assets/author-xkanba.png";
  import { errorMessage, getAppInfo } from "./api/commands";
  import { exportAll, importAll, parseTransferFile, suggestedFileName } from "./api/transfer";
  import type { AppInfo } from "./api/types";
  import { IS_DESKTOP } from "./platform";
  import { reportError, reportNotice } from "./toast.svelte";
  import { setUnlocked, unlock, UNLOCK_TAP_WINDOW_MS, UNLOCK_TAPS } from "./unlock.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let info = $state<AppInfo | null>(null);
  /** 書き出し / 読み込みの途中。押した瞬間から結果が出るまで、押した場所に出す */
  let transfer = $state<{ busy: boolean; message: string; imported: boolean }>({
    busy: false, message: "", imported: false,
  });

  onMount(() => {
    getAppInfo()
      .then((v) => (info = v))
      .catch((e) => reportError(errorMessage(e)));
  });

  // バージョン表記を続けて押すとロックを切り替える(unlock.svelte.ts)。見た目は変えない
  let taps = 0;
  let lastTapAt = 0;
  function tapVersion() {
    const now = Date.now();
    taps = now - lastTapAt > UNLOCK_TAP_WINDOW_MS ? 1 : taps + 1;
    lastTapAt = now;
    if (taps < UNLOCK_TAPS) return;
    taps = 0;
    setUnlocked(!unlock.on);
    reportNotice(unlock.on ? "追加機能を有効にしました" : "追加機能を無効にしました");
  }

  const closeOnEscape = (event: KeyboardEvent) => {
    if (event.key === "Escape") onClose();
  };

  /** 全部を JSON 1 ファイルにして保存する。預け先はユーザーが選ぶ(保存先を勝手に決めない) */
  async function exportData() {
    transfer = { busy: true, message: "書き出しています…", imported: false };
    try {
      const json = JSON.stringify(await exportAll(), null, 2);
      const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = suggestedFileName();
      link.click();
      URL.revokeObjectURL(url);
      transfer = { busy: false, message: `${suggestedFileName()} を書き出しました`, imported: false };
    } catch (error) {
      transfer = { busy: false, message: "", imported: false };
      reportError(errorMessage(error));
    }
  }

  /** 読み込みは「足す」。いま入っているものは消さない(消す判断をこちらでしない) */
  async function importData(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    // 同じファイルをもう一度選べるようにする(選び直しても change が来ない)
    input.value = "";
    if (!file) return;
    transfer = { busy: true, message: "読み込んでいます…", imported: false };
    try {
      const result = await importAll(parseTransferFile(JSON.parse(await file.text())));
      transfer = {
        busy: false,
        message: `キャラ ${result.characters} 件・バフセット ${result.buffSets} 件を読み込みました`,
        imported: true,
      };
    } catch (error) {
      transfer = { busy: false, message: "", imported: false };
      reportError(errorMessage(error));
    }
  }

  async function openExternal(event: MouseEvent, url: string) {
    event.preventDefault();
    try {
      await openUrl(url);
    } catch (error) {
      reportError(errorMessage(error));
    }
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
        <!-- 続けて押すとロックを切り替える。入口は見せない(見た目・キーボード操作は元のまま) -->
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
        <div class="version num" onclick={tapVersion}>{info?.version ?? "—"}</div>
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
        <!-- ネクソンの FAQ「ファンサイトで公式サイトの画像などを使用できますか?」が、
             ゲームに関連するコンテンツを含むページにこの表記を求めている。文言は変えない -->
        <p class="copyright">Copyrights (C) NEXON Corporation and NEXON Co., Ltd. All Rights Reserved.</p>
        <p class="muted">
          <a
            class="source-link"
            href="https://talesweaver.nexon.co.jp/"
            target="_blank"
            rel="noreferrer"
            onclick={(event) => openExternal(event, "https://talesweaver.nexon.co.jp/")}
          >テイルズウィーバー公式サイト</a>
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
          問い合わせを送る場合も、送信内容は別画面で事前に全文表示されます。
          <!-- 自動バックアップはデスクトップ版だけの話。ブラウザ版で言うと嘘になる -->
          {#if IS_DESKTOP}
            アップデートのたびに、このファイルのバックアップを直近 3 世代まで自動で保存します。
          {:else}
            ブラウザのサイトデータを消すと一緒に消えるので、大事なものは「データの持ち出し」で書き出しておいてください。
          {/if}
        </p>
      </div>

      <div class="card">
        <div class="card-title">データの持ち出し</div>
        <p>
          登録キャラ・バフセット・画像・計算の記録を、JSON 1 ファイルにまとめて書き出せます。
          読み込むと、いま入っているものを消さずに足します。
        </p>
        <div class="transfer">
          <button type="button" class="btn" onclick={exportData} disabled={transfer.busy}>書き出す</button>
          <label class="btn">
            読み込む
            <input type="file" accept="application/json,.json" onchange={importData} disabled={transfer.busy} />
          </label>
        </div>
        {#if transfer.message}
          <p class="muted transfer-message">
            {transfer.message}
            {#if transfer.imported}
              <button type="button" class="btn" onclick={() => window.location.reload()}>画面に出す</button>
            {/if}
          </p>
        {/if}
      </div>

      <div class="card">
        <div class="card-title">数値・計算仕様の参考資料</div>
        <p>
          スキル倍率・敵ステータス・装備補正などは、コミュニティ運営の
          <b>Tale Wiki</b>(talewiki.com)を一次ソースとして取り込んでいます。
        </p>
        <p>
          一部のステータスや計算式などは、せせなぎさんが公開している実測・検証情報や
          ダメージ計算ツールを参考に収録・整理しています。
        </p>
        <p class="muted">
          参考にした公開情報・ツール:
          <a
            class="source-link"
            href="https://x.com/sese_nagi1125?s=11"
            target="_blank"
            rel="noreferrer"
            onclick={(event) => openExternal(event, "https://x.com/sese_nagi1125?s=11")}
          >せせなぎさん（@sese_nagi1125）</a>
        </p>
        <p class="muted">
          この記載は公開情報へのクレジットであり、本ツールの公認・監修・共同開発を示すものではありません。
        </p>
      </div>

      <div class="card">
        <div class="card-title">作者について</div>
        <div class="author">
          <img class="author-portrait" src={authorPortrait} alt="作者 xかんばのゲーム内キャラクター" />
          <div class="author-detail">
            <b class="author-name">xかんば</b>
            <span class="author-server">エルフィンタサーバーで活動しています</span>
            <p>
              システム開発のお仕事のご依頼・ご相談は、ゲーム内の「xかんば」または
              XのDMへお願いします。
            </p>
            <a
              class="source-link"
              href="https://x.com/tw_xkanba?s=11"
              target="_blank"
              rel="noreferrer"
              onclick={(event) => openExternal(event, "https://x.com/tw_xkanba?s=11")}
            >@tw_xkanba（X）</a>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-title">ライセンス</div>
        <p>ソースコードと文書は MIT License。同梱しているゲーム由来の画像・数値データは対象外です。</p>
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
  /* 権利表記。読み飛ばされない程度に地の文と揃え、装飾はしない */
  .card p.copyright { font-size: var(--t-label); color: var(--fg-muted); }

  /* 続けて押しても文字が選択されないように(ロック切替の入口) */
  .version { font-size: 19px; font-weight: var(--w-strong); user-select: none; }
  .num { font-family: var(--font-num); font-variant-numeric: tabular-nums; }

  .path {
    padding: 6px 8px; margin: 4px 0 8px;
    font-family: var(--font-num); font-size: 10.5px; color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    word-break: break-all;
  }

  /* ファイル選択は見た目をボタンに合わせる(入力欄の素の見た目を出さない) */
  .transfer { display: flex; gap: 8px; align-items: center; }
  .transfer label.btn { cursor: pointer; }
  .transfer input[type="file"] { display: none; }
  .transfer-message { display: flex; align-items: center; gap: 8px; margin-top: 8px; }

  .source-link {
    color: var(--accent); font-weight: var(--w-strong);
    text-decoration: underline; text-underline-offset: 2px;
  }
  .source-link:hover { color: var(--accent-hover); }

  .author { display: grid; grid-template-columns: 64px minmax(0, 1fr); gap: 12px; align-items: start; }
  .author-portrait {
    width: 64px; height: 64px; object-fit: cover; object-position: center 34%;
    background: var(--bg-raised); border: 1px solid var(--border); border-radius: var(--r-inset);
    box-shadow: inset 0 1px rgba(255, 255, 255, 0.75);
  }
  .author-detail { min-width: 0; }
  .author-name { display: block; font-size: 13px; color: var(--fg); }
  .author-server { display: block; margin: 1px 0 6px; font-size: var(--t-label); color: var(--fg-muted); }
  .author-detail p { margin-bottom: 5px; }
</style>
