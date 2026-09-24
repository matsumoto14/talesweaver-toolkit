/**
 * ブラウザ版。crates/web が公開する `invoke` を Tauri と同じシグネチャに合わせる。
 *
 * WASM 側は同期関数だが、画面は Promise を前提にしているので async で包む。
 * エラーは crates/web が投げる CommandError(message / location)がそのまま伝わる。
 *
 * 保存が要るコマンドは WASM に渡さず、ここで IndexedDB(browserStore)に振り分ける。
 * 保存を Rust に持たせず TS 側で受けるのは、WASM から JS の非同期ストレージを呼び戻す
 * 手間に見合わないため。検証だけは domain を持つ WASM に問う(文言をデスクトップ版と揃える)。
 */
import init, { invoke as callWasm } from "tw-web";

import { version as appVersion } from "../../package.json";
import * as store from "./browserStore";
import type { BuffSelection, CharacterSkills, NewCharacter } from "./types";
import { t } from "../i18n";

/**
 * 「追加機能の解除」で取得した追加装備(unlock.svelte.ts)。デスクトップ版は Rust 側が
 * app_data_dir のファイルに保存して起動時に読み直すが、ブラウザ版は WASM が生きている間しか
 * 持てないので、同じ役目をここで localStorage に持たせる(解除状態と同じくこのブラウザにだけ残る)。
 * 無いと、追加装備を着けたキャラがリロード後に「未知の装備アイテム」で保存・計算できなくなる。
 */
const DOWNLOADED_EQUIPMENT_KEY = "tw-v4-extra-equipment";

// 初期化は 1 回だけ。最初に呼ばれた invoke がこれを待つ(呼び出し側に初期化を意識させない)。
// 初期化の中で、前回取得した追加装備をカタログへ戻す(壊れていれば捨てて未収録のまま進む)。
const ready = init().then(async () => {
  let json: string | null = null;
  try {
    json = localStorage.getItem(DOWNLOADED_EQUIPMENT_KEY);
  } catch {
    json = null;
  }
  if (json !== null) {
    try {
      callWasm("install_downloaded_equipment", { json });
    } catch {
      localStorage.removeItem(DOWNLOADED_EQUIPMENT_KEY);
    }
  }
  // v7: 主軸に召喚スキルが紛れている行を召喚欄へ移す / v10: カタログから消えたキャラスキルを
  // 落とす(browserStore.ts の normalizeStoredSkillSelections 参照)。判定はどちらも
  // Rust(WASM)側の正規化関数に委ね、ここは呼ぶだけ(2026-09-21 追記)。
  await store.normalizeStoredSkillSelections({
    summon: (mainSkillId, summonSkillId) =>
      callWasm("normalize_summon_skill_selection", { mainSkillId, summonSkillId }) as {
        main_skill_id: string | null;
        summon_skill_id: string | null;
      },
    characterSkills: (characterSkills) =>
      callWasm("normalize_character_skills", { characterSkills }) as CharacterSkills,
    rotationSkills: (rotationSkillIds, gameCharacterId) =>
      callWasm("retain_rotation_skills", { rotationSkillIds, gameCharacterId }) as string[] | null,
  });
});

type Args = Record<string, unknown>;

async function wasm<T>(command: string, args: Args): Promise<T> {
  await ready;
  return callWasm(command, args) as T;
}

/** 保存する前に WASM 側の検証を通す。落ちれば CommandError がそのまま投げられる */
const validate = (command: string, args: Args) => wasm<void>(command, args);

/**
 * 保存が要るコマンド。ここに載っているものだけ IndexedDB で処理し、残りは WASM に流す。
 * 戻り値の形はデスクトップ版のコマンド(apps/desktop/src-tauri/src/commands.rs)に合わせる。
 */
const stored: Record<string, (args: Args) => Promise<unknown>> = {
  /** 保存先は OS のパスではない。ブラウザの中だと正直に言う(端末のパスを返すと嘘になる) */
  get_app_info: async () => ({
    version: appVersion,
    databasePath: t("このブラウザの中(IndexedDB: tw-context)"),
  }),
  /** ブラウザ版にはバックアップからの復元がないので、起動時に伝えることがない */
  get_startup_notice: async () => null,

  list_characters: () => store.listCharacters(),
  create_character: async (a) => {
    await validate("validate_character", { character: a.character });
    return store.createCharacter(a.character as NewCharacter);
  },
  update_character: async (a) => {
    await validate("validate_character", { character: a.character });
    return store.updateCharacter(a.id as number, a.character as NewCharacter);
  },
  delete_character: (a) => store.deleteCharacter(a.id as number),

  list_buff_sets: () => store.listBuffSets(),
  create_buff_set: async (a) => {
    await validate("validate_buff_set", { name: a.name, choices: a.choices });
    return store.createBuffSet(a.name as string, a.choices as BuffSelection);
  },
  update_buff_set: async (a) => {
    await validate("validate_buff_set", { name: a.name, choices: a.choices });
    return store.updateBuffSet(a.id as number, a.name as string, a.choices as BuffSelection);
  },
  duplicate_buff_set: (a) => store.duplicateBuffSet(a.id as number),
  delete_buff_set: (a) => store.deleteBuffSet(a.id as number),
  set_default_buff_set: (a) =>
    store.setDefaultBuffSet(a.characterId as number, a.buffSetId as number | null),

  list_character_icons: () => store.listCharacterIcons(),
  set_character_icon: (a) =>
    store.setCharacterIcon(a.characterId as number, Uint8Array.from(a.source as number[])),
  reset_character_icon: (a) => store.resetCharacterIcon(a.characterId as number),

  /** 検証・合流は WASM。通ったぶんだけ localStorage に残し、次回の初期化で読み直す */
  install_downloaded_equipment: async (a) => {
    const count = await wasm<number>("install_downloaded_equipment", { json: a.json });
    try {
      localStorage.setItem(DOWNLOADED_EQUIPMENT_KEY, a.json as string);
    } catch {
      // private モード等で残せなければ、この起動中だけ有効
    }
    return count;
  },

  get_damage_snapshot: (a) => store.getDamageSnapshot(a.characterId as number),
  set_damage_snapshot: (a) =>
    store.setDamageSnapshot(
      a.characterId as number,
      a.skillId as string,
      a.contentId as string,
      a.perHit as number,
    ),

  /**
   * 登録済みキャラでの計算。デスクトップ版と同じ順序で、保存先からキャラを引いてから
   * 計算そのものは WASM(preview_damage)に投げる。
   */
  calculate_damage: async (a) => {
    const character = await store.getCharacter(a.characterId as number);
    return wasm("preview_damage", {
      character,
      buffs: a.buffs,
      skillId: a.skillId,
      contentId: a.contentId,
      comboCount: a.comboCount,
      comboSkillType: a.comboSkillType,
      normalAttackId: a.normalAttackId,
      temporaryAdjustments: a.temporaryAdjustments,
    });
  },
};

export async function invoke<T>(command: string, args: Args = {}): Promise<T> {
  // 保存先を読むコマンドも初期化(`ready`)を待つ。IndexedDB は WASM の読み込みより速いので、
  // 待たないと v7 の正規化(主軸に紛れた召喚スキルを召喚欄へ移す)が終わる前に
  // `list_characters` が未正規化のキャラを返し、その状態で編集すると自動保存が検証エラーで
  // 止まる(移行が防ぐはずの壊れ方が起動 1 回ぶん再発する。レビュー指摘 2026-09-18)。
  await ready;
  const handler = stored[command];
  if (handler) return handler(args) as Promise<T>;
  return wasm<T>(command, args);
}
