<script lang="ts">
  // 「wiki に聞く」(段階 1)。案内役ゼリッピと対話する形で wiki を検索する。
  // 結論文(LLM · 検証済)・手順・出典・訂正・リアクションは pages/ask/AnswerBubble.svelte。
  // 会話は保存しない(ラベンダー = 保存されない面)。
  import { onMount } from "svelte";
  import { errorMessage } from "../../api/commands";
  import { ask, health, type AskResponse, type AskState, type NextQuestion, type PrevTurn } from "../../ask";
  import { fmtMonthDay } from "../../format";
  import { reportError } from "../../toast.svelte";
  import { changed, reveal } from "../../ui/motion.svelte";
  import TextField from "../../ui/TextField.svelte";
  import AnswerBubble from "./AnswerBubble.svelte";
  import { askErrorLine, emptyLine, EXPRESSION_SPRITE, noneLine, offlineLine, progressLine, thinkingLine, type Expression } from "./lines";

  const SEND_HOST = new URL("https://api.tw-context.dev").host;

  interface Turn {
    id: number;
    question: string;
    status: "thinking" | "done" | "error";
    expression: Expression;
    /** 「考え中」「見つからない」「つながらない」など、AnswerBubble が描かない状態のときだけ使う一言 */
    message: string;
    response: AskResponse | null;
    /** 続きの判定に渡す直前のページ名(答えられた手順の 1 つ目から取る) */
    page: string | null;
  }

  let healthState = $state<"checking" | "ok" | "down">("checking");
  let syncedAt = $state("");
  let question = $state("");
  let turns = $state<Turn[]>([]);
  let nextTurnId = 0;
  let bottomRef = $state<HTMLDivElement | undefined>();

  onMount(() => void checkHealth());

  async function checkHealth() {
    try {
      const result = await health();
      if (result.ok) {
        healthState = "ok";
        syncedAt = result.synced_at;
      } else {
        healthState = "down";
      }
    } catch {
      healthState = "down";
    }
  }

  // 新しい発言が積まれた・状態が変わったら末尾を追う(会話は下に積む。押した場所は動かさない —
  // 動くのは末尾だけで、既存の発言の位置は変わらない)
  $effect(() => {
    turns.length;
    turns.at(-1)?.status;
    reveal(bottomRef, "end", { instant: true });
  });

  function submit() {
    const q = question.trim();
    if (!q || healthState !== "ok") return;
    question = "";
    void askQuestion(q);
  }

  /** 続きバッジの × を押したとき: 同じ質問を prev: null で聞き直す(新しいターンとして下に積む) */
  function restartFresh(q: string) {
    void askQuestion(q, null);
  }

  /** 次の一手のチップを押したとき: 今の答えのページを prev にして、入力欄に入れずに即送信する */
  function askNext(nq: NextQuestion, prev: PrevTurn | null) {
    void askQuestion(nq.question, prev);
  }

  /**
   * `prevOverride` を渡さなければ直前のターンから自動で決める(通常の会話の続き)。
   * `null` を渡すと続きの判定を切り離し、`PrevTurn` を渡すとそれを使う(次の一手のチップ用)。
   */
  async function askQuestion(q: string, prevOverride?: PrevTurn | null): Promise<void> {
    // 登録キャラは素のレベルを持たない(アプリはカンスト前提。awakening.eternal_level はエタ Lv で
    // wiki の「Lv」とは別物)。段階 1 は状態を送らない。レベルの欄が保存形に入ったらここで詰める
    const state: AskState = {};
    const prevTurn = turns.at(-1);
    const prev: PrevTurn | null = prevOverride !== undefined
      ? prevOverride
      : prevTurn?.page ? { question: prevTurn.question, page: prevTurn.page } : null;

    nextTurnId += 1;
    // push した元のオブジェクトではなく、$state 配列の中の proxy を書き換える
    // (元のオブジェクトを触っても画面は更新されない。Svelte 5 の深い状態の作法)
    const index = turns.push({
      id: nextTurnId, question: q, status: "thinking", expression: "thinking", message: thinkingLine(q),
      response: null, page: null,
    }) - 1;
    const turn = turns[index];
    if (!turn) return;
    try {
      const res = await ask(q, state, prev, (step) => {
        turn.message = progressLine(step);
      });
      turn.response = res;
      turn.status = "done";
      if (res.kind === "answer") {
        turn.expression = "answered";
        turn.message = "";
        turn.page = res.steps[0]?.source.page ?? null;
      } else {
        const line = noneLine(res.reason, q);
        turn.expression = line.expression;
        turn.message = res.search.length > 0 ? line.message : emptyLine(q);
      }
    } catch (e) {
      const line = askErrorLine(e, q);
      turn.expression = line.expression;
      turn.message = line.message;
      turn.status = "error";
      reportError(errorMessage(e));
    }
  }
</script>

<div class="ask">
  {#if healthState !== "ok"}
    <div class="status">
      {#if healthState === "checking"}
        <p class="dim">つないでいます…</p>
      {:else}
        <img class="face swap-in" src={EXPRESSION_SPRITE.offline} alt="ゼリッピ" />
        <p class="line pop-in">{offlineLine("")}</p>
      {/if}
    </div>
  {:else}
    <div class="scroll">
      <div class="meta">
        <span class="synced">wiki は {fmtMonthDay(syncedAt)} 時点</span>
      </div>

      {#if turns.length === 0}
        <div class="intro">
          <img class="face" src={EXPRESSION_SPRITE.answered} alt="ゼリッピ" />
        </div>
      {/if}

      {#each turns as turn (turn.id)}
        <div class="turn">
          <div class="bubble user pop-in">{turn.question}</div>
          <div class="reply">
            {#key turn.expression}
              <img class="face swap-in" src={EXPRESSION_SPRITE[turn.expression]} alt="ゼリッピ" />
            {/key}
            <div class="bubble zerippi pop-in">
              {#if turn.status !== "done" || turn.response?.kind !== "answer"}
                <p class="line" use:changed={() => turn.message}>{turn.message}</p>
              {/if}
              {#if turn.status === "done" && turn.response}
                <AnswerBubble
                  question={turn.question}
                  response={turn.response}
                  level={null}
                  onRestartFresh={restartFresh}
                  onAskNext={askNext}
                />
              {/if}
            </div>
          </div>
        </div>
      {/each}
      <div bind:this={bottomRef}></div>
    </div>

    <div class="composer">
      <TextField label="ゼリッピへの質問" bind:value={question} max={200} onEnter={submit} />
      <button type="button" class="btn primary" onclick={submit} disabled={!question.trim()}>聞く</button>
    </div>
    <p class="notice dim">
      質問は回答サーバー({SEND_HOST})を通して外部の AI サービスに送ります。質問と答えは改善のために記録します(キャラや装備の中身は送りません)。
      値を押すと「この値は違う」を送れます
    </p>
  {/if}
</div>

<style>
  .ask { min-width: 0; min-height: 0; flex: 1; display: flex; flex-direction: column; background: var(--bg-mid); }

  .status {
    flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px;
  }
  .status .dim { font-size: 11px; }

  .scroll {
    flex: 1; min-height: 0; overflow: auto; scrollbar-gutter: stable;
    padding: 16px 22px 10px; display: flex; flex-direction: column; gap: 14px; max-width: 780px; width: 100%;
    margin: 0 auto; box-sizing: border-box;
  }

  .meta { display: flex; justify-content: center; }
  .synced { font-size: 9.5px; color: var(--fg-dim); font-variant-numeric: tabular-nums; }

  .intro { display: flex; justify-content: center; padding: 18px 0; }

  .turn { display: flex; flex-direction: column; gap: 8px; }

  /* 自分の発言は右寄せ・水色に近い読み取り面。答えを送る保存済みの入力ではないが、
     wiki に聞くのと同じ帯の中で「自分の言葉」と分かる濃さにする */
  .bubble {
    max-width: 86%; padding: 8px 12px; border-radius: var(--r-window); font-size: 11.5px; line-height: 1.6;
  }
  .bubble.user {
    align-self: flex-end; background: var(--bg-field); border: 1px solid var(--border-soft); color: var(--fg);
    transform-origin: top right;
  }

  .reply { display: flex; align-items: flex-start; gap: 8px; align-self: flex-start; max-width: 100%; min-width: 0; }
  /* ゼリッピは会話が保存されない面(§03 予約色)。ラベンダーの淡い地で常に見分けられる */
  .bubble.zerippi {
    background: var(--state-temp-bg); border: 1px solid var(--sim); color: var(--fg);
    display: flex; flex-direction: column; gap: 8px; min-width: 0;
  }
  .bubble.zerippi .line { margin: 0; color: var(--sim-fg); font-weight: 700; }

  /* 36×54 の枠に底辺揃え・2 倍・ドットのまま(design案 s6.txt) */
  .face {
    flex: none; width: 72px; height: 108px; object-fit: contain; object-position: bottom center;
    image-rendering: pixelated; align-self: flex-end;
  }
  .intro .face { align-self: center; }

  .composer {
    flex: none; display: flex; gap: 8px; align-items: center; padding: 10px 22px 4px;
    max-width: 780px; width: 100%; margin: 0 auto; box-sizing: border-box;
  }
  .composer .btn { flex: none; }

  .notice {
    flex: none; margin: 0; padding: 2px 22px 12px; text-align: center; font-size: 9px;
    max-width: 780px; width: 100%; margin-left: auto; margin-right: auto; box-sizing: border-box;
  }
</style>
