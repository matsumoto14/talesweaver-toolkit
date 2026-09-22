<script lang="ts">
  // リアクション「役に立った / 違った」(s6.txt「リアクション」)。押した瞬間に送る(「送信」を挟まない)。
  // 「違った」は同じ場所に理由チップ 3 つ + 同意チェックが開く(ui/Disclosure。レイアウトを押さない)。
  // answer_id につき 1 回だけ押せる(端末側の抑止。サーバー側も KV で 2 回目を無視する)。
  import { errorMessage } from "../../api/commands";
  import { react, type ReactPayload, type WrongReason } from "../../ask";
  import { reportError } from "../../toast.svelte";
  import Choose, { type ChooseOption } from "../../ui/Choose.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import { REACTION_HELPFUL_LINE, REACTION_WRONG_LINE, WRONG_REASON_LABEL } from "./lines";

  interface Props {
    answerId: string;
    unitIds: string[];
    /** 「質問文も一緒に送る」に同意したときだけ添える質問文 */
    question: string;
  }
  let { answerId, unitIds, question }: Props = $props();

  const REASON_OPTIONS: ChooseOption[] = (Object.keys(WRONG_REASON_LABEL) as WrongReason[])
    .map((value) => ({ value, label: WRONG_REASON_LABEL[value] }));

  let sent = $state<"helpful" | "wrong" | null>(null);
  let wrongOpen = $state(false);
  let wrongReason = $state<WrongReason | undefined>(undefined);
  let consent = $state(false); // 次回に持ち越さない(既定オフ)

  async function send(payload: ReactPayload) {
    try {
      await react(payload);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }

  function helpful() {
    if (sent) return;
    sent = "helpful";
    void send({ answer_id: answerId, kind: "helpful", unit_ids: unitIds });
  }

  function wrong() {
    if (sent) return;
    sent = "wrong";
    wrongOpen = true;
    void send({ answer_id: answerId, kind: "wrong", unit_ids: unitIds });
  }

  function sendDetail() {
    void send({
      answer_id: answerId,
      kind: "wrong",
      reason: wrongReason,
      unit_ids: unitIds,
      question: consent ? question : undefined,
    });
  }

  // 理由チップは Choose の radio(単一選択)なので、選ばれた瞬間は onToggle ではなく
  // 値そのものの変化で拾う(押した瞬間に結果が動く。§08)。初回のマウントでは送らない
  // (「違った」を押した時点で無印の 1 回はすでに `wrong()` が送っている)
  let firstRun = true;
  $effect(() => {
    wrongReason;
    consent;
    if (firstRun) {
      firstRun = false;
      return;
    }
    sendDetail();
  });
</script>

<div class="reaction">
  {#if sent === null}
    <span class="prompt">この答えは役に立った</span>
    <button type="button" class="btn quiet" onclick={helpful}>役に立った</button>
    <button type="button" class="btn quiet" onclick={wrong}>違った</button>
  {:else if sent === "helpful"}
    <span class="reply badge-in">{REACTION_HELPFUL_LINE}</span>
  {:else}
    <span class="reply badge-in">{REACTION_WRONG_LINE}</span>
  {/if}
</div>

{#if sent === "wrong"}
  <Disclosure class="wrong-detail" summaryClass="chip quiet" bind:open={wrongOpen}>
    {#snippet summary()}理由を伝える{/snippet}
    {#snippet children()}
      <div class="detail open-in">
        <Choose label="違った理由" bind:value={wrongReason} options={REASON_OPTIONS} class="chiprow" />
        <label class="consent">
          <input type="checkbox" bind:checked={consent} />
          質問文も一緒に送る(保存されます)
        </label>
      </div>
    {/snippet}
  </Disclosure>
{/if}

<style>
  .reaction { display: flex; align-items: center; gap: 8px; font-size: 10px; }
  .prompt { color: var(--fg-dim); }
  .reply { font-weight: 700; color: var(--sim-fg); }
  .detail { display: flex; flex-direction: column; gap: 6px; padding: 6px 0 2px; }
  .consent { display: flex; align-items: center; gap: 5px; font-size: 9.5px; color: var(--fg-muted); }
</style>
