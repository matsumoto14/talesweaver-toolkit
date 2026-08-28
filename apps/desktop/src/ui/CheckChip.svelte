<script lang="ts">
  // オン / オフのチェックチップ(§07 形態 3)。素のチェックボックスは 5 形態のどれでもない
  // (pages/chars/sources の各ペインで使われていた `.check` の形を部品に昇格した)。
  import type { Snippet } from "svelte";

  interface Props {
    checked: boolean;
    onCheckedChange: (checked: boolean) => void;
    children: Snippet;
  }
  let { checked, onCheckedChange, children }: Props = $props();
</script>

<label class="check">
  <input type="checkbox" {checked} onchange={(e) => onCheckedChange(e.currentTarget.checked)} />
  {@render children()}
</label>
