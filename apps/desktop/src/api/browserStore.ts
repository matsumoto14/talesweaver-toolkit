/**
 * ブラウザ版の保存先(IndexedDB)。
 *
 * 保存は Rust(WASM)に持たせない。WASM から JS の非同期ストレージを呼ぶ手間に見合わないので、
 * `crates/web` は「渡されたデータを計算して返す」ままにし、保存だけをここ(TS)で受け持つ。
 *
 * 振る舞いはデスクトップ版(`crates/storage` の各リポジトリ)に合わせる:
 * 採番・並び順(id 昇順)・最終保存日時・見つからないときの文言・キャラ削除時の連鎖削除まで同じにする。
 * 検証(名前が空・値域・装備カタログ整合)は domain を持つ WASM 側に任せ、ここでは扱わない
 * (同じ検証を TS に写すと必ずずれるため。保存前に問うのは呼び出し側 = invoke.wasm.ts)。
 */
import type {
  BuffSelection, BuffSet, CharacterIcon, DamageSnapshot, NewCharacter, RegisteredCharacter,
  ValidationLocation,
} from "./types";

const DB_NAME = "tw-context";
/**
 * スキーマ版。ストアを足す・作り直すときに上げ、`onupgradeneeded` で移行する。
 * v2 でキャラに `goal_content_id`(ホームの「次の目標」)が加わった
 * (SQLite 側の v13 と同じ移行。既存キャラは未設定 = 自動判定のまま)。
 */
const SCHEMA_VERSION = 2;

const CHARACTERS = "characters";
const BUFF_SETS = "buff_sets";
const ICONS = "character_icons";
const SNAPSHOTS = "damage_snapshots";
/** 採番用。SQLite の rowid に相当する「次に使う id」を種類ごとに持つ */
const COUNTERS = "counters";

/** 画面のエラー帯が読む形(message / location)。Tauri 版・WASM 版の CommandError と同じ。 */
export interface CommandFailure {
  message: string;
  location: ValidationLocation | null;
}

/** エラーは Error ではなくこの形で投げる。WASM 側が投げるものと形を揃えるため。 */
export const failure = (
  message: string,
  location: ValidationLocation | null = null,
): CommandFailure => ({ message, location });

const characterNotFound = (id: number) => failure(`キャラクター(id=${id})が見つかりません`);
const buffSetNotFound = (id: number) => failure(`バフセット(id=${id})が見つかりません`);
const invalidIcon = (reason: string) => failure(`キャラクター画像が不正です: ${reason}`);

/** SQLite 側の strftime('%Y-%m-%dT%H:%M:%fZ','now') と同じ形(UTC・ミリ秒まで) */
const nowIso = () => new Date().toISOString();

const wrap = <T>(request: IDBRequest<T>): Promise<T> =>
  new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(failure(`保存先(IndexedDB)の操作に失敗しました: ${request.error?.message ?? ""}`));
  });

let connection: Promise<IDBDatabase> | null = null;

function open(): Promise<IDBDatabase> {
  connection ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, SCHEMA_VERSION);
    request.onupgradeneeded = (event) => {
      const db = request.result;
      // キーは SQLite の主キーと同じ列にする(取り出し順もそのまま id 昇順になる)
      if (!db.objectStoreNames.contains(CHARACTERS)) db.createObjectStore(CHARACTERS, { keyPath: "id" });
      if (!db.objectStoreNames.contains(BUFF_SETS)) db.createObjectStore(BUFF_SETS, { keyPath: "id" });
      if (!db.objectStoreNames.contains(ICONS)) db.createObjectStore(ICONS, { keyPath: "characterId" });
      if (!db.objectStoreNames.contains(SNAPSHOTS)) db.createObjectStore(SNAPSHOTS, { keyPath: "character_id" });
      if (!db.objectStoreNames.contains(COUNTERS)) db.createObjectStore(COUNTERS, { keyPath: "name" });
      // v2: 既存キャラに「次の目標」を未設定(null)として足す。列の無い行を残して
      // undefined のまま読ませない(SQLite 側の ALTER TABLE と同じ扱いに揃える)。
      if (event.oldVersion > 0 && event.oldVersion < 2) {
        const characters = request.transaction!.objectStore(CHARACTERS);
        const cursorRequest = characters.openCursor();
        cursorRequest.onsuccess = () => {
          const cursor = cursorRequest.result;
          if (!cursor) return;
          const row = cursor.value as Partial<RegisteredCharacter>;
          if (row.goal_content_id === undefined) cursor.update({ ...row, goal_content_id: null });
          cursor.continue();
        };
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(failure("保存先(IndexedDB)を開けません。プライベートウィンドウでは保存できないことがあります"));
    request.onblocked = () =>
      reject(failure("別のタブが古い保存先を開いています。そのタブを閉じてから読み込み直してください"));
  });
  return connection;
}

/**
 * 1 つの操作を 1 トランザクションで行う。`run` の中で待ってよいのは IndexedDB の要求だけ
 * (それ以外を待つとトランザクションが先に閉じる)。
 */
async function transact<T>(
  stores: string | string[],
  mode: IDBTransactionMode,
  run: (tx: IDBTransaction) => Promise<T>,
): Promise<T> {
  const db = await open();
  return run(db.transaction(stores, mode));
}

/** 次の id を 1 つ払い出す。消した id は再利用しない(参照が別のものに繋がる事故を避ける) */
async function nextId(tx: IDBTransaction, name: string): Promise<number> {
  const store = tx.objectStore(COUNTERS);
  const row = await wrap(store.get(name) as IDBRequest<{ name: string; next: number } | undefined>);
  const id = row?.next ?? 1;
  await wrap(store.put({ name, next: id + 1 }));
  return id;
}

// --- 登録キャラクター(character_repository.rs) ---

export const listCharacters = () =>
  transact(CHARACTERS, "readonly", (tx) =>
    wrap(tx.objectStore(CHARACTERS).getAll() as IDBRequest<RegisteredCharacter[]>));

export const getCharacter = (id: number) =>
  transact(CHARACTERS, "readonly", async (tx) => {
    const row = await wrap(tx.objectStore(CHARACTERS).get(id) as IDBRequest<RegisteredCharacter | undefined>);
    if (!row) throw characterNotFound(id);
    return row;
  });

export const createCharacter = (character: NewCharacter) =>
  transact([CHARACTERS, COUNTERS], "readwrite", async (tx) => {
    const id = await nextId(tx, CHARACTERS);
    const saved: RegisteredCharacter = { ...character, id, updated_at: nowIso() };
    await wrap(tx.objectStore(CHARACTERS).add(saved));
    return saved;
  });

export const updateCharacter = (id: number, character: NewCharacter) =>
  transact(CHARACTERS, "readwrite", async (tx) => {
    const store = tx.objectStore(CHARACTERS);
    const existing = await wrap(store.get(id) as IDBRequest<RegisteredCharacter | undefined>);
    if (!existing) throw characterNotFound(id);
    const saved: RegisteredCharacter = { ...character, id, updated_at: nowIso() };
    await wrap(store.put(saved));
    return saved;
  });

/** キャラを消すと画像と記録も消える(SQLite 側の ON DELETE CASCADE と同じ) */
export const deleteCharacter = (id: number) =>
  transact([CHARACTERS, ICONS, SNAPSHOTS], "readwrite", async (tx) => {
    const store = tx.objectStore(CHARACTERS);
    const existing = await wrap(store.get(id) as IDBRequest<RegisteredCharacter | undefined>);
    if (!existing) throw characterNotFound(id);
    await wrap(store.delete(id));
    await wrap(tx.objectStore(ICONS).delete(id));
    await wrap(tx.objectStore(SNAPSHOTS).delete(id));
  });

// --- バフセット(buff_set_repository.rs) ---

export const listBuffSets = () =>
  transact(BUFF_SETS, "readonly", (tx) =>
    wrap(tx.objectStore(BUFF_SETS).getAll() as IDBRequest<BuffSet[]>));

const readBuffSet = async (tx: IDBTransaction, id: number): Promise<BuffSet> => {
  const row = await wrap(tx.objectStore(BUFF_SETS).get(id) as IDBRequest<BuffSet | undefined>);
  if (!row) throw buffSetNotFound(id);
  return row;
};

export const createBuffSet = (name: string, choices: BuffSelection) =>
  transact([BUFF_SETS, COUNTERS], "readwrite", async (tx) => {
    const saved: BuffSet = { id: await nextId(tx, BUFF_SETS), name: name.trim(), choices };
    await wrap(tx.objectStore(BUFF_SETS).add(saved));
    return saved;
  });

export const updateBuffSet = (id: number, name: string, choices: BuffSelection) =>
  transact(BUFF_SETS, "readwrite", async (tx) => {
    await readBuffSet(tx, id);
    const saved: BuffSet = { id, name: name.trim(), choices };
    await wrap(tx.objectStore(BUFF_SETS).put(saved));
    return saved;
  });

export const duplicateBuffSet = (id: number) =>
  transact([BUFF_SETS, COUNTERS], "readwrite", async (tx) => {
    const source = await readBuffSet(tx, id);
    const copy: BuffSet = {
      id: await nextId(tx, BUFF_SETS),
      name: `${source.name}のコピー`,
      choices: source.choices,
    };
    await wrap(tx.objectStore(BUFF_SETS).add(copy));
    return copy;
  });

/** 消したバフセットを既定にしていたキャラは未選択に戻す(SQLite 側の ON DELETE SET NULL と同じ) */
export const deleteBuffSet = (id: number) =>
  transact([BUFF_SETS, CHARACTERS], "readwrite", async (tx) => {
    await readBuffSet(tx, id);
    await wrap(tx.objectStore(BUFF_SETS).delete(id));
    const characters = tx.objectStore(CHARACTERS);
    const rows = await wrap(characters.getAll() as IDBRequest<RegisteredCharacter[]>);
    for (const row of rows) {
      if (row.default_buff_set_id === id) await wrap(characters.put({ ...row, default_buff_set_id: null }));
    }
  });

/** 既定バフセットの付け替えは最終保存日時を動かさない(デスクトップ版もこの列だけを更新する) */
export const setDefaultBuffSet = (characterId: number, buffSetId: number | null) =>
  transact([BUFF_SETS, CHARACTERS], "readwrite", async (tx) => {
    if (buffSetId !== null) await readBuffSet(tx, buffSetId);
    const characters = tx.objectStore(CHARACTERS);
    const row = await wrap(characters.get(characterId) as IDBRequest<RegisteredCharacter | undefined>);
    if (!row) throw characterNotFound(characterId);
    const saved: RegisteredCharacter = { ...row, default_buff_set_id: buffSetId };
    await wrap(characters.put(saved));
    return saved;
  });

// --- キャラ画像(character_icon_repository.rs) ---

const MAX_SOURCE_BYTES = 5 * 1024 * 1024;
const MAX_SOURCE_PIXELS = 16_000_000;
const ICON_SIZE = 128;

const startsWith = (bytes: Uint8Array, signature: number[], offset = 0) =>
  signature.every((value, i) => bytes[offset + i] === value);

/** 拡張子ではなく先頭バイトで見る(デスクトップ版も中身で形式を判定している) */
const isSupportedImage = (bytes: Uint8Array) =>
  startsWith(bytes, [0x89, 0x50, 0x4e, 0x47]) // PNG
  || startsWith(bytes, [0xff, 0xd8, 0xff]) // JPEG
  || (startsWith(bytes, [0x52, 0x49, 0x46, 0x46]) && startsWith(bytes, [0x57, 0x45, 0x42, 0x50], 8)); // WebP

/** 中央を正方形に切って 128px の PNG にする(デスクトップ版の normalize_icon と同じ形) */
async function normalizeIcon(source: Uint8Array): Promise<string> {
  if (source.length === 0 || source.length > MAX_SOURCE_BYTES) throw invalidIcon("画像は5 MiB以下にしてください");
  if (!isSupportedImage(source)) throw invalidIcon("PNG、JPEG、WebPを選んでください");

  let image: ImageBitmap;
  try {
    image = await createImageBitmap(new Blob([source as BlobPart]));
  } catch {
    throw invalidIcon("画像を読み取れません");
  }
  const { width, height } = image;
  if (width === 0 || height === 0 || width * height > MAX_SOURCE_PIXELS) {
    image.close();
    throw invalidIcon("画像は合計1600万画素以下にしてください");
  }
  const side = Math.min(width, height);
  const canvas = document.createElement("canvas");
  canvas.width = ICON_SIZE;
  canvas.height = ICON_SIZE;
  const context = canvas.getContext("2d");
  if (!context) {
    image.close();
    throw invalidIcon("画像を保存用に変換できません");
  }
  context.drawImage(
    image,
    Math.floor((width - side) / 2), Math.floor((height - side) / 2), side, side,
    0, 0, ICON_SIZE, ICON_SIZE,
  );
  image.close();
  return canvas.toDataURL("image/png");
}

export const listCharacterIcons = () =>
  transact(ICONS, "readonly", (tx) =>
    wrap(tx.objectStore(ICONS).getAll() as IDBRequest<CharacterIcon[]>));

/**
 * 保存するのは正規化済みの data URL。デスクトップ版は PNG のバイト列を持ち、返すときに
 * data URL へ変換しているが、ブラウザ版は画面が使う形のまま持つ(往復の変換を増やさない)。
 */
export const setCharacterIcon = async (characterId: number, source: Uint8Array): Promise<CharacterIcon> => {
  await getCharacter(characterId);
  const dataUrl = await normalizeIcon(source);
  return transact(ICONS, "readwrite", async (tx) => {
    const icon: CharacterIcon = { characterId, dataUrl };
    await wrap(tx.objectStore(ICONS).put(icon));
    return icon;
  });
};

export const resetCharacterIcon = async (characterId: number): Promise<void> => {
  await getCharacter(characterId);
  await transact(ICONS, "readwrite", (tx) => wrap(tx.objectStore(ICONS).delete(characterId)));
};

// --- ダメージ記録(damage_snapshot_repository.rs) ---

export const getDamageSnapshot = (characterId: number) =>
  transact(SNAPSHOTS, "readonly", async (tx) => {
    const row = await wrap(tx.objectStore(SNAPSHOTS).get(characterId) as IDBRequest<DamageSnapshot | undefined>);
    return row ?? null;
  });

/** 1 キャラ 1 件。前の記録は上書きし、`taken_at` はその都度入れ直す */
export const setDamageSnapshot = async (
  characterId: number, skillId: string, contentId: string, perHit: number,
): Promise<DamageSnapshot> => {
  // 存在しないキャラの記録は残さない(SQLite 側は外部キーが弾いている)
  await getCharacter(characterId);
  return transact(SNAPSHOTS, "readwrite", async (tx) => {
    const snapshot: DamageSnapshot = {
      character_id: characterId, skill_id: skillId, content_id: contentId,
      per_hit: perHit, taken_at: nowIso(),
    };
    await wrap(tx.objectStore(SNAPSHOTS).put(snapshot));
    return snapshot;
  });
};
