<script lang="ts">
  // 「この値は違う」(stage2-spec.md desktop 14)。行の値(<Value>)を押すと同じ場所に
  // Popover が開き、「この値は違う」ボタンを押した瞬間に value_wrong を送る(§08 押すと結果が動く)。
  // その後、同じ Popover に「正しいと思う値(任意)」「根拠(任意)」「質問文も一緒に送る」が開き、
  // 入力があれば同じ answer_id・unit・col で再送する(Worker は追記として受ける)。列ごとに 1 回。
  import { errorMessage } from "../../api/commands";
  import { react, type ValueWrongPayload } from "../../ask";
  import { reportError } from "../../toast.svelte";
  import Popover from "../../ui/Popover.svelte";
  import TextField from "../../ui/TextField.svelte";
  import Value from "../../ui/Value.svelte";
  import { VALUE_WRONG_ACK_LINE } from "./lines";

  interface Props {
    answerId: string;
    unitId: string;
    col: string;
    /** 答えに出ていたユニットの ID(helpful/wrong と同じ欄) */
    unitIds: string[];
    /** 「質問文も一緒に送る」に同意したときだけ添える質問文 */
    question: string;
    value: string | null;
    /** Value に渡す見た目(数値書体・幅など) */
    class?: string;
  }
  let { answerId, unitId, col, unitIds, question, value, class: klass = "" }: Props = $props();

  let sent = $state(false);
  let claim = $state("");
  let note = $state("");
  let consent = $state(false);

  function basePayload(): ValueWrongPayload {
    return { answer_id: answerId, kind: "value_wrong", unit_id: unitId, col, unit_ids: unitIds };
  }

  async function send(payload: ValueWrongPayload) {
    try {
      await react(payload);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }

  function markWrong() {
    if (sent) return;
    sent = true;
    void send(basePayload());
  }

  // 詳しい入力は「確定したとき」だけ追記として再送する。自由記述欄は 1 文字ごとに送らない
  // (blur / Enter の onCommit で確定。同意チェックは押した瞬間)。/react は /ask と 1 日の上限を共有するため
  function sendDetail() {
    if (!sent) return;
    void send({
      ...basePayload(),
      claim: claim.trim() || undefined,
      note: note.trim() || undefined,
      question: consent ? question : undefined,
    });
  }
</script>

<Popover label="この値について" triggerClass="value-wrong-trigger {klass}" panelClass="value-wrong-panel">
  {#snippet trigger()}
    <Value {value} class={klass} />
  {/snippet}
  {#snippet children(close)}
    <div class="value-wrong">
      {#if !sent}
        <button type="button" class="btn quiet" onclick={markWrong}>この値は違う</button>
      {:else}
        <p class="ack badge-in">{VALUE_WRONG_ACK_LINE}</p>
        <TextField label="正しいと思う値" bind:value={claim} max={100} onCommit={sendDetail} />
        <TextField label="根拠" bind:value={note} max={300} rows={2} onCommit={sendDetail} />
        <label class="consent">
          <input type="checkbox" bind:checked={consent} onchange={sendDetail} />
          質問文も一緒に送る(保存されます)
        </label>
        <button type="button" class="btn quiet" onclick={close}>閉じる</button>
      {/if}
    </div>
  {/snippet}
</Popover>

<style>
  /* トリガは値そのもの。押しても値の見た目は変えない(押した場所は動かない) */
  :global(.value-wrong-trigger) {
    background: none; border: none; padding: 0; margin: 0; font: inherit; cursor: pointer;
    display: inline-flex; align-items: baseline;
  }
  :global(.value-wrong-panel) { width: 220px; }
  .value-wrong { display: flex; flex-direction: column; gap: 6px; padding: 2px; }
  .ack { margin: 0; font-size: 9.5px; font-weight: 700; color: var(--sim-fg); }
  .consent { display: flex; align-items: center; gap: 5px; font-size: 9px; color: var(--fg-muted); }
</style>
