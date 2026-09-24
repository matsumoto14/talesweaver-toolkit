// 問い合わせの送信。中継サーバー(services/inquiry-worker)を経由して GitHub Issue にする。
//
// アプリに秘密を持たせないため、認証はしない。代わりに中継側が proof-of-work と
// レート制限で守っている。ここは「nonce をもらう → 解く → 送る」だけ。

import type {
  AttackPowerBreakdown, BuffSelection, DamageSnapshot, EffectiveStats, EquipmentPartList, EquipmentParts, NewCharacter,
  SienaAuraList,
  SienaAuras,
} from "./api/types";
import { t } from "./i18n";
import { solveChallenge } from "./pow";

/**
 * 中継サーバー(services/inquiry-worker)。
 *
 * **この URL は配布したアプリの CSP に焼き込まれる。** 変えると、既に入っている版からは
 * 二度と送信できなくなる(その版の `connect-src` が古い URL のままのため)。
 * 変えるときは `src-tauri/tauri.conf.json` の `connect-src` も必ず同じ URL にする。
 */
export const INQUIRY_ENDPOINT ="https://inquiry.tw-context.dev";

/**
 * 添付するキャラのデータの上限(文字)。中継サーバー(services/inquiry-worker の
 * `LIMITS.character`)と同じ値にする。**超えたら切らずに付けない** —— 途中で切れた JSON は
 * 貼り直しても再現に使えないので、「送ったのに再現できない」問い合わせになるだけ。
 */
export const CHARACTER_ATTACHMENT_MAX = 40000;

/** 添付が上限を超えているか(送信前に警告して添付を止める) */
export const characterAttachmentTooLong = (character: string): boolean =>
  character.length > CHARACTER_ATTACHMENT_MAX;

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
  /** 選択中キャラのデータ(`characterAttachment`)。付けないときは空 */
  character: string;
}

/** アプリがその入力から出していた数字。再現した値と突き合わせる基準になる */
export interface InquiryResult {
  stats: EffectiveStats;
  attack: AttackPowerBreakdown | null;
  /** 直近のダメージ計算(保存済みのキャラだけ) */
  last_damage: Pick<DamageSnapshot, "skill_id" | "content_id" | "per_hit"> | null;
}

/**
 * 問い合わせに付けるキャラのデータ。**計算を再現できる入力一式**(素ステ・補正源・装備・
 * 共通スキル・計算に使っているバフ)と、アプリが出していた結果。
 *
 * `character` は保存形(`NewCharacter`)のままなので、テストの入力にそのまま使える。
 * 公開のページに載るので、ユーザーが自由に書いた文字(キャラ名・部位やオーラのラベル・
 * バフセット名)と端末内の id は入れない。装備は**装備中のものだけ**を残し、所持称号や
 * 次の目標のような計算に効かない欄は空にする。カタログ外装備の `custom_name` は
 * アイテム名なので残す(データの誤りの調査に要る)。
 *
 * 全文を送信前に見せるので、1 欄 1 行で書く(全体は 1 つの JSON として読める)。
 */
export function characterAttachment(
  character: NewCharacter, buffs: BuffSelection, result: InquiryResult | null,
): string {
  const { equipment } = character;
  const shared: NewCharacter = {
    name: "",
    game_character_id: character.game_character_id,
    base_stats: character.base_stats,
    awakening: character.awakening,
    main_skill_id: character.main_skill_id,
    summon_skill_id: character.summon_skill_id,
    common_skills: character.common_skills,
    stat_sources: character.stat_sources,
    equipment: {
      ...equipment,
      parts: selectedOnly(equipment.parts),
      siena: selectedOnly(equipment.siena),
      owned_titles: equipment.title ? [equipment.title] : [],
    },
    goal_content_id: null,
    rotation_skill_ids: character.rotation_skill_ids,
    default_buff_set_id: null,
  };
  return `{"character":{\n${fields(shared)}\n},\n${fields({ buffs, result })}\n}`;
}

/**
 * 登録リストを「装備中の 1 件だけ」にする。id は振り直し、ラベルは空にする。
 *
 * **`custom_name`(カタログ外装備の名前)は消さない** —— ユーザーが打った文字だが
 * 個人を指す情報ではなくアイテム名で、「その装備の値が違う」という問い合わせの調査に要る
 * (上の `characterAttachment` の但し書きどおり、ここが唯一の例外)。
 */
function selectedOnly<T extends EquipmentParts | SienaAuras>(lists: T): T {
  const entries = Object.entries(lists) as [string, EquipmentPartList | SienaAuraList][];
  return Object.fromEntries(entries.map(([slot, list]) => {
    const registered: { id: number }[] = list.registered;
    const selected = registered.find((candidate) => candidate.id === list.selected_id);
    return [slot, selected
      ? { registered: [{ ...selected, id: 1, label: "" }], selected_id: 1 }
      : { registered: [], selected_id: null }];
  })) as unknown as T;
}

function fields(value: object): string {
  return Object.entries(value)
    .map(([key, field]) => `${JSON.stringify(key)}:${JSON.stringify(field)}`)
    .join(",\n");
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
  if (characterAttachmentTooLong(draft.character)) {
    parts.push("", "--- 選択中のキャラのデータ ---", "(上限を超えるため添付しません)");
  } else if (draft.character) {
    parts.push("", "--- 選択中のキャラのデータ ---", draft.character);
  }
  return parts.join("\n");
}

export async function send(
  draft: InquiryDraft,
  includeDiagnostics: boolean,
  onProgress?: (message: string) => void,
): Promise<SentInquiry> {
  onProgress?.(t("送信の準備をしています…"));
  const challenge = await request<Challenge>("/challenge");

  onProgress?.(t("送信の検証中です…"));
  const solution = await solveChallenge(challenge.nonce, challenge.difficultyBits);

  onProgress?.(t("送信しています…"));
  return request<SentInquiry>("/inquiry", {
    nonce: challenge.nonce,
    solution,
    kind: draft.kind,
    title: draft.title,
    body: draft.body,
    diagnostics: includeDiagnostics ? draft.diagnostics : "",
    // 上限を超える添付は送らない(中継側で切られると壊れた JSON が issue に載る)
    character: characterAttachmentTooLong(draft.character) ? "" : draft.character,
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
    throw new Error(t("送信サーバーに接続できませんでした。ネットワークを確認してください。"));
  }

  const payload = (await response.json().catch(() => null)) as { error?: string } | null;
  if (!response.ok) {
    throw new Error(payload?.error ?? t("送信に失敗しました({status})", { status: response.status }));
  }
  return payload as T;
}
