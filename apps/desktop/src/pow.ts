// proof-of-work の共通処理。inquiry.ts(問い合わせ)と ask.ts(wiki に聞く)が同じ中継の作法
// (nonce をもらう → 解く → 送る)を使うため、ここに 1 つだけ置く。

/**
 * sha256(nonce + ":" + counter) の先頭が規定ビット数だけ 0 になる counter を探す。
 * 20 ビットでおよそ 100 万回。UI を固めないよう、区切りごとに制御を返す。
 */
export async function solveChallenge(nonce: string, difficultyBits: number): Promise<string> {
  const encoder = new TextEncoder();
  for (let counter = 0; ; counter += 1) {
    const digest = new Uint8Array(
      await crypto.subtle.digest("SHA-256", encoder.encode(`${nonce}:${counter}`)),
    );
    if (hasLeadingZeroBits(digest, difficultyBits)) return String(counter);
    if (counter % 2000 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

function hasLeadingZeroBits(digest: Uint8Array, bits: number): boolean {
  let remaining = bits;
  for (const byte of digest) {
    if (remaining >= 8) {
      if (byte !== 0) return false;
      remaining -= 8;
      continue;
    }
    return byte >>> (8 - remaining) === 0;
  }
  return true;
}
