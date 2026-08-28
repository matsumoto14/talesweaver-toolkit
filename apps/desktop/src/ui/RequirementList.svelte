<script lang="ts">
  // 入場条件の一覧。CalcPage(入場条件カード)と HomePage(選択中カード)で
  // markup ごと重複していたものを部品化した。見た目・振る舞いは変えていない。
  import { fmtInt } from "../format";
  import { bump } from "./motion.svelte";

  interface Check {
    label: string;
    current: number;
    required: number;
    ok: boolean;
  }
  interface Props {
    checks: Check[];
    /** 呼び出し側ごとの margin-top の微差を吸収する(スコープが跨るので CSS 上書きが効かない) */
    style?: string;
  }
  let { checks, style = "" }: Props = $props();
</script>

<div class="reqs" {style}>
  {#each checks as c (c.label)}
    <div class="req" class:ng={!c.ok}>
      <span class="req-label">{c.label}</span>
      <span class="num dim" use:bump={() => c.current}>{fmtInt(c.current)} / {fmtInt(c.required)}</span>
      <span class="req-tag">{c.ok ? "OK" : `あと ${fmtInt(c.required - c.current)}`}</span>
    </div>
  {/each}
</div>
