// 問い合わせの送信。中継サーバー(services/inquiry-worker)を経由して GitHub Issue にする。
//
// アプリに秘密を持たせないため、認証はしない。代わりに中継側が proof-of-work と
// レート制限で守っている。ここは「nonce をもらう → 解く → 送る」だけ。

import type {
  EquipmentPartList, EquipmentValues, PartSlot, RegisteredCharacter, SienaAuraList,
} from "./api/types";

/**
 * 中継サーバー(services/inquiry-worker)。
 *
 * **この URL は配布したアプリの CSP に焼き込まれる。** 変えると、既に入っている版からは
 * 二度と送信できなくなる(その版の `connect-src` が古い URL のままのため)。
 * 変えるときは `src-tauri/tauri.conf.json` の `connect-src` も必ず同じ URL にする。
 */
export const INQUIRY_ENDPOINT ="https://inquiry.tw-context.dev";

export type InquiryKind = "bug" | "data" | "feature";

export const INQUIRY_KINDS: { value: InquiryKind; label: string }[] = [
  { value: "bug", label: "不具合" },
  { value: "data", label: "データの誤り" },
  { value: "feature", label: "要望" },
];

export interface InquiryDraft {
  kind: InquiryKind;
  title: string;
  body: string;
  /** 自動で付ける情報。送信前に全文を見せて、ユーザーが外せるようにする */
  diagnostics: string;
  /** 選択中キャラの装備(`equipmentAttachment`)。付けないときは空 */
  equipment: string;
}

/**
 * 問い合わせに付ける装備データ。**装備中のものだけ**を 1 部位 1 行で書く。
 *
 * 公開のページに載るので、ユーザーが自由に書いた文字(キャラ名・部位やオーラのラベル)と
 * 端末内の id は入れない。0 の能力値は落とす(全文を送信前に見せるので、読める長さに保つ)。
 * カタログ外装備の `custom_name` はアイテム名なので残す(データの誤りの調査に要る)。
 */
export function equipmentAttachment(character: RegisteredCharacter): string {
  const { parts, siena, thesis_cores, avatar, polish, title } = character.equipment;
  const lines: string[] = [];

  for (const [slot, list] of Object.entries(parts) as [PartSlot, EquipmentPartList][]) {
    const part = list.registered.find((candidate) => candidate.id === list.selected_id);
    if (!part) continue;
    const { id: _id, label: _label, base, enchant, enchant_caps, ...rest } = part;
    lines.push(`${slot}: ${JSON.stringify({
      ...rest,
      base: nonZero(base),
      enchant: nonZero(enchant),
      enchant_caps: enchant_caps && nonZero(enchant_caps),
    })}`);
  }
  for (const [slot, list] of Object.entries(siena) as [string, SienaAuraList][]) {
    const selected = list.registered.find((candidate) => candidate.id === list.selected_id);
    if (selected) lines.push(`siena.${slot}: ${JSON.stringify(selected.aura)}`);
  }
  for (const [region, set] of Object.entries(thesis_cores)) {
    if (set.slots.some((core) => core !== null)) {
      lines.push(`thesis_cores.${region}: ${JSON.stringify(set.slots)}`);
    }
  }
  for (const [slot, values] of Object.entries(avatar)) {
    const kept = nonZero(values);
    if (Object.keys(kept).length > 0) lines.push(`avatar.${slot}: ${JSON.stringify(kept)}`);
  }
  if (polish.entries.length > 0) lines.push(`polish: ${JSON.stringify(polish.entries)}`);
  if (title) lines.push(`title: ${title}`);

  return lines.join("\n");
}

function nonZero(values: EquipmentValues): Partial<EquipmentValues> {
  return Object.fromEntries(Object.entries(values).filter(([, value]) => value !== 0));
}

export interface SentInquiry {
  url: string;
  number: number;
}

interface Challenge {
  nonce: string;
  difficultyBits: number;
}

/** 送信前に見せる、そのまま送られる本文。 */
export function preview(draft: InquiryDraft, includeDiagnostics: boolean): string {
  const kind = INQUIRY_KINDS.find((k) => k.value === draft.kind)?.label ?? "";
  const parts = [`件名: [${kind}] ${draft.title}`, "", draft.body];
  if (includeDiagnostics && draft.diagnostics) {
    parts.push("", "--- アプリが自動で付ける情報 ---", draft.diagnostics);
  }
  if (draft.equipment) parts.push("", "--- 選択中のキャラの装備 ---", draft.equipment);
  return parts.join("\n");
}

export async function send(
  draft: InquiryDraft,
  includeDiagnostics: boolean,
  onProgress?: (message: string) => void,
): Promise<SentInquiry> {
  onProgress?.("送信の準備をしています…");
  const challenge = await request<Challenge>("/challenge");

  onProgress?.("送信の検証中です…");
  const solution = await solve(challenge.nonce, challenge.difficultyBits);

  onProgress?.("送信しています…");
  return request<SentInquiry>("/inquiry", {
    nonce: challenge.nonce,
    solution,
    kind: draft.kind,
    title: draft.title,
    body: draft.body,
    diagnostics: includeDiagnostics ? draft.diagnostics : "",
    equipment: draft.equipment,
  });
}

async function request<T>(path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${INQUIRY_ENDPOINT}${path}`, {
      method: body ? "POST" : "GET",
      headers: body ? { "content-type": "application/json" } : undefined,
      body: body ? JSON.stringify(body) : undefined,
    });
  } catch {
    throw new Error("送信サーバーに接続できませんでした。ネットワークを確認してください。");
  }

  const payload = (await response.json().catch(() => null)) as { error?: string } | null;
  if (!response.ok) {
    throw new Error(payload?.error ?? `送信に失敗しました(${response.status})`);
  }
  return payload as T;
}

/**
 * sha256(nonce + ":" + counter) の先頭が規定ビット数だけ 0 になる counter を探す。
 * 20 ビットでおよそ 100 万回。UI を固めないよう、区切りごとに制御を返す。
 */
async function solve(nonce: string, difficultyBits: number): Promise<string> {
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
