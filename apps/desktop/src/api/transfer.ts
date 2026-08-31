/**
 * 登録データの持ち出し / 取り込み(JSON 1 ファイル)。
 *
 * ブラウザ版の保存先(IndexedDB)はサイトデータを消すと一緒に消えるので、これが無いと
 * データを預けられない。デスクトップ版にも同じものを付けて、同じファイルを行き来できるようにする。
 *
 * 実装はコマンド層(api/commands.ts)だけを使う。保存先が SQLite か IndexedDB かをここで
 * 気にしなくて済み、取り込みも登録と同じ検証を通る。
 */
import {
  createBuffSet, createCharacter, getDamageSnapshot, listBuffSets, listCharacterIcons,
  listCharacters, setCharacterIcon, setDamageSnapshot, setDefaultBuffSet,
} from "./commands";
import type {
  BuffSet, CharacterIcon, DamageSnapshot, NewCharacter, RegisteredCharacter,
} from "./types";

/** 中身が変わったら上げる。読み込み側は知らない版を拒む(黙って一部だけ入れない) */
const FORMAT = "tw-context-data";
const FORMAT_VERSION = 1;

export interface TransferFile {
  format: typeof FORMAT;
  version: number;
  /** 書き出した日時(ISO8601 UTC)。どちらが新しいかを人が見分けるためだけに持つ */
  exportedAt: string;
  characters: RegisteredCharacter[];
  buffSets: BuffSet[];
  icons: CharacterIcon[];
  snapshots: DamageSnapshot[];
}

export const suggestedFileName = () =>
  `tw-context-${new Date().toISOString().slice(0, 10)}.json`;

/** いま保存されているものを全部集める。 */
export async function exportAll(): Promise<TransferFile> {
  const characters = await listCharacters();
  const snapshots: DamageSnapshot[] = [];
  for (const character of characters) {
    const snapshot = await getDamageSnapshot(character.id);
    if (snapshot) snapshots.push(snapshot);
  }
  return {
    format: FORMAT,
    version: FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    characters,
    buffSets: await listBuffSets(),
    icons: await listCharacterIcons(),
    snapshots,
  };
}

/** 手で書き換えられるファイルなので、形が違うものは読み込む前に断る */
export function parseTransferFile(value: unknown): TransferFile {
  const file = value as TransferFile | null;
  if (typeof file !== "object" || file === null || file.format !== FORMAT) {
    throw new Error("このファイルは TW Context の書き出しファイルではありません");
  }
  if (file.version !== FORMAT_VERSION) {
    throw new Error(`このファイル(形式 v${file.version})は、いまの版では読み込めません`);
  }
  if (!Array.isArray(file.characters) || !Array.isArray(file.buffSets)
    || !Array.isArray(file.icons) || !Array.isArray(file.snapshots)) {
    throw new Error("ファイルの中身が壊れています");
  }
  return file;
}

const bytesOfDataUrl = (dataUrl: string): Uint8Array => {
  const base64 = dataUrl.slice(dataUrl.indexOf(",") + 1);
  const binary = atob(base64);
  return Uint8Array.from(binary, (c) => c.charCodeAt(0));
};

export interface ImportResult {
  characters: number;
  buffSets: number;
}

/**
 * 読み込む。いまあるデータは消さずに足す(消す判断をこちらでしない)。
 * id は保存先が新しく振り直すので、キャラの既定バフセットは新しい id に読み替える。
 */
export async function importAll(file: TransferFile): Promise<ImportResult> {
  const buffSetIds = new Map<number, number>();
  for (const set of file.buffSets) {
    const created = await createBuffSet(set.name, set.choices);
    buffSetIds.set(set.id, created.id);
  }

  const characterIds = new Map<number, number>();
  for (const character of file.characters) {
    // 登録に要るのは NewCharacter の分だけ。id と最終保存日時は保存先が新しく付ける
    const draft: NewCharacter = {
      name: character.name,
      game_character_id: character.game_character_id,
      base_stats: character.base_stats,
      awakening: character.awakening,
      stat_sources: character.stat_sources,
      equipment: character.equipment,
      common_skills: character.common_skills,
      main_skill_id: character.main_skill_id,
      default_buff_set_id: null,
    };
    const created = await createCharacter(draft);
    characterIds.set(character.id, created.id);
    // 既定バフセットは、そのバフセットも同じファイルに入っていたときだけ繋ぎ直す
    const buffSetId = character.default_buff_set_id;
    if (buffSetId !== null && buffSetIds.has(buffSetId)) {
      await setDefaultBuffSet(created.id, buffSetIds.get(buffSetId)!);
    }
  }

  for (const icon of file.icons) {
    const id = characterIds.get(icon.characterId);
    if (id !== undefined) await setCharacterIcon(id, bytesOfDataUrl(icon.dataUrl));
  }
  for (const snapshot of file.snapshots) {
    const id = characterIds.get(snapshot.character_id);
    if (id !== undefined) await setDamageSnapshot(id, snapshot.skill_id, snapshot.content_id, snapshot.per_hit);
  }

  return { characters: characterIds.size, buffSets: buffSetIds.size };
}
