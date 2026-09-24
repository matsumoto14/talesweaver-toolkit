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
  BuffSelection, BuffSet, CharacterIcon, CharacterSkills, DamageSnapshot, NewCharacter,
  RegisteredCharacter, ValidationLocation,
} from "./types";
import { t } from "../i18n";

const DB_NAME = "tw-context";
/**
 * スキーマ版。ストアを足す・作り直すときに上げ、`onupgradeneeded` で移行する。
 * v2 でキャラに `goal_content_id`(ホームの「次の目標」)が加わった
 * (SQLite 側の v13 と同じ移行。既存キャラは未設定 = 自動判定のまま)。
 * v3 で装備に `avatar`(アバター強化)と `polish`(装備研磨)が加わった。SQLite 側は
 * `#[serde(default)]` で読めるが、IndexedDB は素の JSON を返すので既存行に中立値を足す
 * (無いと `cloneEquipment` が undefined を読んでキャラタブが開けない。2026-09-15)。
 * v4 で装備に `owned_titles`(所持称号一覧)が加わった(SQLite 側の v14 と同じ移行)。
 * 既存行は `title`(表示中)があれば `[title]`、無ければ `[]` を補う。
 * v5 でレリックの聖域 20段の content id が `relic_sanctuary_kisinik` から `relic_sanctuary_20`
 * に変わった(SQLite 側の v15 と同じ移行)。保存済みの「次の目標」を書き換える。
 * v6 でキャラに `summon_skill_id`(魔法人形の召喚スキル)が加わった(SQLite 側の v16 と同じ移行。
 * 既存キャラは未選択のまま)。
 * v7 で、主軸(`main_skill_id`)に召喚スキルが紛れている行を召喚欄(`summon_skill_id`)へ移す
 * (SQLite 側の v17 と同じ移行。破壊精霊を足す前は本体の主軸に召喚スキルを選べてしまっていた穴)。
 * 新しいストア・列は無いので `onupgradeneeded` には何もしない。判定・移す先の決定は Rust
 * (WASM)の正規化関数を要るため `onupgradeneeded` の版変更トランザクション中には呼べず、
 * `normalizeSummonSkillSelections`(ストアを開いた直後に呼ぶ通常のトランザクション)として
 * 実装する(2026-09-18 追記。呼び出し側は invoke.wasm.ts)。
 * v8 で補正源に `lumina_corridor`(ルミナの回廊の回廊効果)が加わった。SQLite は JSON 列
 * (`stat_sources`)なので列追加も migrate も要らないが、IndexedDB は v3 と同じ理由で既存行に
 * 中立値(全 Lv0)を足す(2026-09-19)。
 * v10 で、カタログから消えたキャラスキル(イェフネンの 鋭い欠片<フラグ> / べたつく欠片<フラグ>)を
 * 保存済みの選択から落とす(SQLite 側の v18 と同じ移行)。v7 と同じく新しいストア・列は無く、
 * どの id が消えたかの判定は Rust(WASM)のカタログを引く正規化関数しか持っていないので、
 * `onupgradeneeded` ではなく `normalizeStoredSkillSelections`(ストアを開いた直後の通常の
 * トランザクション)で行う(2026-09-21)。
 * v11 でキャラに `rotation_skill_ids`(回しに差し込む CT 技)が加わった(SQLite 側の v19 と
 * 同じ移行)。既存行は未設定(null)= 既定の 1 つを自動で差し込むまま。
 * v9 で装備に `avatar_corrections`(補正付きアバターをどの部位に着けているか)が加わった。
 * SQLite は JSON 列(`equipment`)なので列追加も migrate も要らないが、IndexedDB は v3 と同じ理由で
 * 既存行に中立値(全部位 false)を足す(2026-09-21)。
 *
 * **埋め直しは 1 本のカーソルにまとめる**(2026-09-21)。版ごとに `openCursor()` を分けていたが、
 * IndexedDB は要求を置かれた順に処理するので、同じストアに N 本開くと N 本とも「更新前の行」を
 * 読んでから順に書き戻し、最後の 1 本以外の埋め直しが消えていた(v5 の DB を v9 で開くと
 * summon_skill_id と lumina_corridor が落ちる)。版ごとの分岐も持たず、欠けている欄だけを埋める。
 */
const SCHEMA_VERSION = 11;

/** v3 で足した装備の欄の中立値。形の正は crates/domain の `AvatarEnhancements` / `EquipmentPolishes` */
const ZERO_EQUIPMENT_VALUES = {
  thrust: 0, slash: 0, physical_defense: 0, magic_attack: 0, magic_defense: 0,
  accuracy: 0, critical: 0, evasion: 0, agility: 0,
};
const NEUTRAL_AVATAR = () => ({
  helm: { ...ZERO_EQUIPMENT_VALUES }, head: { ...ZERO_EQUIPMENT_VALUES }, body: { ...ZERO_EQUIPMENT_VALUES },
  legs: { ...ZERO_EQUIPMENT_VALUES }, effect: { ...ZERO_EQUIPMENT_VALUES },
});
const NEUTRAL_POLISH = () => ({ entries: [] });
/** v9 で足した装備の欄の中立値。形の正は crates/domain の `AvatarCorrections` */
const NEUTRAL_AVATAR_CORRECTIONS = () => ({
  helm: false, head: false, body: false, legs: false, effect: false,
});
/** v8 で足した補正源の中立値。形の正は crates/domain の `LuminaCorridor` */
const NEUTRAL_LUMINA_CORRIDOR = () => ({
  final_damage_level: 0, all_element_level: 0, damage_reduction_level: 0, hp_mp_sp_level: 0,
});

/**
 * 装備に v3/v4/v9 の欄が無ければ中立値を足す。読み込み(transfer.ts)で旧い書き出しを受けたときも
 * ここを通るので、保存する行は常に今の形になる(SQLite 側は serde default が同じことをする)。
 */
function withEquipmentDefaults(character: NewCharacter): NewCharacter {
  const equipment = character.equipment as Partial<NewCharacter["equipment"]>;
  const partial = character as Partial<NewCharacter>;
  const statSources = character.stat_sources as Partial<NewCharacter["stat_sources"]>;
  if (
    equipment.avatar !== undefined &&
    equipment.avatar_corrections !== undefined &&
    equipment.polish !== undefined &&
    equipment.owned_titles !== undefined &&
    partial.summon_skill_id !== undefined &&
    partial.rotation_skill_ids !== undefined &&
    statSources.lumina_corridor !== undefined
  ) {
    return character;
  }
  return {
    ...character,
    // v6: 召喚スキル(旧い書き出し JSON には欄が無い)。未収録は未選択(null)扱い
    summon_skill_id: partial.summon_skill_id ?? null,
    // v11: 差し込む CT 技(旧い行・旧い書き出し JSON には欄が無い)。未設定(null)= 既定
    rotation_skill_ids: partial.rotation_skill_ids ?? null,
    equipment: {
      ...character.equipment,
      avatar: equipment.avatar ?? NEUTRAL_AVATAR(),
      // v9: 補正付きアバター(旧い行・旧い書き出し JSON には欄が無い)。未装備を中立値で足す
      avatar_corrections: equipment.avatar_corrections ?? NEUTRAL_AVATAR_CORRECTIONS(),
      polish: equipment.polish ?? NEUTRAL_POLISH(),
      owned_titles: equipment.owned_titles ?? (equipment.title ? [equipment.title] : []),
    },
    // v8: ルミナの回廊(旧い行・旧い書き出し JSON には欄が無い)。未習得を中立値で足す
    stat_sources: {
      ...character.stat_sources,
      lumina_corridor: statSources.lumina_corridor ?? NEUTRAL_LUMINA_CORRIDOR(),
    },
  };
}

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

const characterNotFound = (id: number) => failure(t("キャラクター(id={id})が見つかりません", { id }));
const buffSetNotFound = (id: number) => failure(t("バフセット(id={id})が見つかりません", { id }));
const invalidIcon = (reason: string) => failure(t("キャラクター画像が不正です: {reason}", { reason }));

/** SQLite 側の strftime('%Y-%m-%dT%H:%M:%fZ','now') と同じ形(UTC・ミリ秒まで) */
const nowIso = () => new Date().toISOString();

const wrap = <T>(request: IDBRequest<T>): Promise<T> =>
  new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(failure(t("保存先(IndexedDB)の操作に失敗しました: {reason}", { reason: request.error?.message ?? "" })));
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
      // --- 既存行の埋め直し ------------------------------------------------
      // **1 本のカーソルにまとめる。**同じストアに複数のカーソルを開いてはいけない:
      // IndexedDB は要求を置かれた順に処理するので、カーソルを N 本開くと N 本とも
      // 「更新前の行」を読んでから順に書き戻し、最後の 1 本以外の埋め直しが消える。
      // 実際、v5 の DB を v9 で開くと summon_skill_id(v6)と lumina_corridor(v8)が
      // 落ちて、キャラタブが開けない行ができていた(2026-09-21 に発見・修正)。
      //
      // 版ごとの分岐も持たない(migrate_* と同じく、起動のたび全部走っても同じ結果になる形)。
      // 欠けている欄だけが埋まり、既に今の形の行は書き戻さない。
      if (event.oldVersion > 0 && event.oldVersion < SCHEMA_VERSION) {
        const characters = request.transaction!.objectStore(CHARACTERS);
        const cursorRequest = characters.openCursor();
        cursorRequest.onsuccess = () => {
          const cursor = cursorRequest.result;
          if (!cursor) return;
          const row = cursor.value as RegisteredCharacter;
          // v3(avatar / polish)・v4(owned_titles)・v6(summon_skill_id)・
          // v8(lumina_corridor)・v9(avatar_corrections)・v11(rotation_skill_ids)。
          // 欠けている欄に中立値を足す
          // (SQLite 側で Rust が serde default / ALTER TABLE で埋めるのと同じ意味)
          const filled = withEquipmentDefaults(row) as RegisteredCharacter;
          // v2: 「次の目標」を未設定(null)として足す
          const goal = filled.goal_content_id ?? null;
          // v5: レリックの聖域 20段の content id を直す(旧 id のままだと目標が解決できない)
          const goalFixed = goal === "relic_sanctuary_kisinik" ? "relic_sanctuary_20" : goal;
          if (filled !== row || goalFixed !== row.goal_content_id) {
            cursor.update({ ...filled, goal_content_id: goalFixed });
          }
          cursor.continue();
        };
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(failure(t("保存先(IndexedDB)を開けません。プライベートウィンドウでは保存できないことがあります")));
    request.onblocked = () =>
      reject(failure(t("別のタブが古い保存先を開いています。そのタブを閉じてから読み込み直してください")));
  });
  return connection;
}

/**
 * 保存先を丸ごと消す。開けなくなった保存データから復旧するための最後の手段で、
 * **呼ぶ前に必ず人へ確認する**(消えたキャラは書き出し JSON からしか戻せない)。
 * 他のタブが握っていて今すぐ消せない場合も、削除は予約されるので待たない。
 */
export async function deleteDatabase(): Promise<void> {
  const opened = connection ? await connection.catch(() => null) : null;
  opened?.close();
  connection = null;
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.deleteDatabase(DB_NAME);
    request.onsuccess = () => resolve();
    request.onblocked = () => resolve();
    request.onerror = () =>
      reject(failure(t("保存先(IndexedDB)を消せませんでした: {reason}", { reason: request.error?.message ?? "" })));
  });
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
    const saved: RegisteredCharacter = { ...withEquipmentDefaults(character), id, updated_at: nowIso() };
    await wrap(tx.objectStore(CHARACTERS).add(saved));
    return saved;
  });

export const updateCharacter = (id: number, character: NewCharacter) =>
  transact(CHARACTERS, "readwrite", async (tx) => {
    const store = tx.objectStore(CHARACTERS);
    const existing = await wrap(store.get(id) as IDBRequest<RegisteredCharacter | undefined>);
    if (!existing) throw characterNotFound(id);
    const saved: RegisteredCharacter = { ...withEquipmentDefaults(character), id, updated_at: nowIso() };
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

/**
 * ストアを開いた直後に 1 回だけ走る、保存済みの選択の作り直し。
 *
 * - v7: 主軸(`main_skill_id`)に召喚スキルが紛れている行を召喚欄(`summon_skill_id`)へ移す
 *   (SQLite 側の v17 移行 `migrate_summon_skill_out_of_main` と同じ意味)
 * - v10: カタログから消えたキャラスキルの id を落とす(SQLite 側の v18 移行
 *   `migrate_removed_character_skills` と同じ意味)。残っていると計算がまるごと止まる
 * - v11: 選べなくなった「差し込む CT 技」の id を落とす(SQLite 側の v19 移行
 *   `migrate_removed_rotation_skills` と同じ意味)。残っていると保存の検証で弾かれる
 *
 * **判定はすべて `normalize`(呼び出し側 = invoke.wasm.ts が Rust の正規化関数を渡す)に委ね、
 * ここは結果を書き込むだけ**(スキル id の一覧を TS に書き写さない)。`onupgradeneeded` の
 * 版変更トランザクション中に WASM 呼び出しを挟むと安全に完了しないため、通常の
 * トランザクションとして実装する。起動のたびに呼んでも、2 回目以降は対象行が無いので
 * 実質何もしない(SQLite 側の `migrate_*` と同じ冪等性)。
 */
export const normalizeStoredSkillSelections = (normalize: {
  summon: (
    mainSkillId: string | null,
    summonSkillId: string | null,
  ) => { main_skill_id: string | null; summon_skill_id: string | null };
  characterSkills: (characterSkills: CharacterSkills) => CharacterSkills;
  rotationSkills: (
    rotationSkillIds: string[] | null,
    gameCharacterId: string,
  ) => string[] | null;
}) =>
  transact(CHARACTERS, "readwrite", async (tx) => {
    const store = tx.objectStore(CHARACTERS);
    const rows = await wrap(store.getAll() as IDBRequest<RegisteredCharacter[]>);
    for (const row of rows) {
      const summon = normalize.summon(row.main_skill_id, row.summon_skill_id ?? null);
      // 欄が無い / 欠けた古い行(キャラスキルを保存する前の版)は中立値で読む。
      // ここで例外を投げると移行がまるごと止まり、ready が reject してブラウザ版が全滅する
      const stored = row.stat_sources?.character_skills;
      const before: CharacterSkills = {
        skill_ids: stored?.skill_ids ?? [],
        skill_levels: stored?.skill_levels ?? {},
      };
      const skills = normalize.characterSkills(before);
      // v11: 選べなくなった差し込む CT 技を落とす(欄の無い古い行は未設定のまま)
      const rotation = normalize.rotationSkills(
        row.rotation_skill_ids ?? null,
        row.game_character_id,
      );
      const rotationChanged =
        JSON.stringify(rotation ?? null) !== JSON.stringify(row.rotation_skill_ids ?? null);
      const skillsChanged =
        skills.skill_ids.length !== before.skill_ids.length
        || Object.keys(skills.skill_levels).length !== Object.keys(before.skill_levels).length
        || stored?.skill_ids === undefined
        || stored?.skill_levels === undefined;
      if (
        summon.main_skill_id === row.main_skill_id
        && summon.summon_skill_id === (row.summon_skill_id ?? null)
        && !skillsChanged
        && !rotationChanged
      ) continue;
      await wrap(store.put({
        ...row,
        main_skill_id: summon.main_skill_id,
        summon_skill_id: summon.summon_skill_id,
        rotation_skill_ids: rotation ?? null,
        stat_sources: { ...row.stat_sources, character_skills: skills },
      }));
    }
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
      name: t("{name}のコピー", { name: source.name }),
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
  if (source.length === 0 || source.length > MAX_SOURCE_BYTES) throw invalidIcon(t("画像は5 MiB以下にしてください"));
  if (!isSupportedImage(source)) throw invalidIcon(t("PNG、JPEG、WebPを選んでください"));

  let image: ImageBitmap;
  try {
    image = await createImageBitmap(new Blob([source as BlobPart]));
  } catch {
    throw invalidIcon(t("画像を読み取れません"));
  }
  const { width, height } = image;
  if (width === 0 || height === 0 || width * height > MAX_SOURCE_PIXELS) {
    image.close();
    throw invalidIcon(t("画像は合計1600万画素以下にしてください"));
  }
  const side = Math.min(width, height);
  const canvas = document.createElement("canvas");
  canvas.width = ICON_SIZE;
  canvas.height = ICON_SIZE;
  const context = canvas.getContext("2d");
  if (!context) {
    image.close();
    throw invalidIcon(t("画像を保存用に変換できません"));
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
