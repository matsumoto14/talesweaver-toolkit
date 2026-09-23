// 管理ホストの認証(src/access.ts)。jose でテスト鍵を作って署名する。JWKS の取得(node:https)は
// createRemoteJWKSet を createLocalJWKSet に差し替えてネットワークを使わずに見る。
import { beforeAll, describe, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({ keys: [] as Record<string, unknown>[] }));

vi.mock("jose", async (importOriginal) => {
  const actual = await importOriginal<typeof import("jose")>();
  return { ...actual, createRemoteJWKSet: () => actual.createLocalJWKSet({ keys: state.keys as never }) };
});

import { SignJWT, exportJWK, generateKeyPair } from "jose";
import type { KeyLike } from "jose";

import { verifyAccess } from "../src/access";

const TEAM = "test-team.cloudflareaccess.com";
const AUD = "test-aud";
const KID = "test-kid";

let privateKey: KeyLike;

beforeAll(async () => {
  const { privateKey: priv, publicKey } = await generateKeyPair("RS256");
  privateKey = priv;
  state.keys = [{ ...(await exportJWK(publicKey)), alg: "RS256", use: "sig", kid: KID }];
});

async function makeToken(overrides: { aud?: string; iss?: string; exp?: string | number } = {}): Promise<string> {
  return new SignJWT({})
    .setProtectedHeader({ alg: "RS256", kid: KID })
    .setIssuedAt()
    .setIssuer(overrides.iss ?? `https://${TEAM}`)
    .setAudience(overrides.aud ?? AUD)
    .setExpirationTime(overrides.exp ?? "5m")
    .sign(privateKey);
}

function requestWith(token?: string): Request {
  const headers = new Headers();
  if (token) headers.set("Cf-Access-Jwt-Assertion", token);
  return new Request("https://admin.tw-context.dev/", { headers });
}

describe("verifyAccess", () => {
  it("有効なトークンは true", async () => {
    const token = await makeToken();
    expect(await verifyAccess(requestWith(token), { ACCESS_TEAM_DOMAIN: TEAM, ACCESS_AUD: AUD })).toBe(true);
  });

  it("期限切れは false", async () => {
    const token = await makeToken({ exp: "-10s" });
    expect(await verifyAccess(requestWith(token), { ACCESS_TEAM_DOMAIN: TEAM, ACCESS_AUD: AUD })).toBe(false);
  });

  it("aud 不一致は false", async () => {
    const token = await makeToken({ aud: "other-aud" });
    expect(await verifyAccess(requestWith(token), { ACCESS_TEAM_DOMAIN: TEAM, ACCESS_AUD: AUD })).toBe(false);
  });

  it("署名が壊れていれば false", async () => {
    const token = await makeToken();
    const tampered = token.slice(0, -2) + (token.at(-2) === "A" ? "B" : "A") + token.at(-1);
    expect(await verifyAccess(requestWith(tampered), { ACCESS_TEAM_DOMAIN: TEAM, ACCESS_AUD: AUD })).toBe(false);
  });

  it("ヘッダが無ければ false", async () => {
    expect(await verifyAccess(requestWith(), { ACCESS_TEAM_DOMAIN: TEAM, ACCESS_AUD: AUD })).toBe(false);
  });

  it("ACCESS_TEAM_DOMAIN / ACCESS_AUD が空なら常に false(閉じる側に倒す)", async () => {
    const token = await makeToken();
    expect(await verifyAccess(requestWith(token), {})).toBe(false);
    expect(await verifyAccess(requestWith(token), { ACCESS_TEAM_DOMAIN: TEAM })).toBe(false);
    expect(await verifyAccess(requestWith(token), { ACCESS_AUD: AUD })).toBe(false);
  });
});
