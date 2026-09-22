<script lang="ts">
  // ゼリッピの立ち絵。型ごとに 8 コマのアニメ GIF を出す。絵も色も原画のまま
  // (ドット絵には落とさない — ユーザー決定 2026-09-23)。素材の作り方は tools/spritecut/。
  //
  // 動きの規格(design-system §10、0.5s 以内・変わったときだけ動く)の外にある。SpriteFx と同じ理由で、
  // これは「何が変わったか」を伝える動きではなくキャラが生きている表現だから。
  // 動きを消す設定のときは、GIF は止められないので 1 コマ目の静止画に差し替える。
  import { POSE_GIF, POSE_STILL, type Pose } from "./lines";

  interface Props {
    pose: Pose;
    /** 会話の外(まだ質問が無い画面)で中央に置くとき */
    center?: boolean;
  }
  let { pose, center = false }: Props = $props();

  const query = window.matchMedia("(prefers-reduced-motion: reduce)");
  let still = $state(query.matches);
  $effect(() => {
    const onChange = (e: MediaQueryListEvent) => { still = e.matches; };
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  });
</script>

<img class="zerippi" class:center src={still ? POSE_STILL[pose] : POSE_GIF[pose]} alt="ゼリッピ" />

<style>
  /* 地に焼いた絵なので枠ぴったりに出す。縦を揃えて、型が変わっても足元が動かないようにする */
  .zerippi { flex: none; height: 64px; width: auto; display: block; align-self: flex-end; }
  .zerippi.center { align-self: center; }
</style>
