/**
 * 管理ホスト(admin.tw-context.dev)の認証。Cloudflare Access が付ける `Cf-Access-Jwt-Assertion`
 * を `jose` で検証するだけ(セッションや Cookie は持たない。Access 自体がログイン画面を出す)。
 * `ACCESS_TEAM_DOMAIN` / `ACCESS_AUD` が空(Access アプリ未作成)なら常に閉じる側に倒す。
 */
import { createRemoteJWKSet, jwtVerify } from "jose";
import type { JWTVerifyGetKey } from "jose";

export interface AccessEnv {
  ACCESS_TEAM_DOMAIN?: string;
  ACCESS_AUD?: string;
}

// JWKS はチームドメインごとに使い回す(インスタンスが生きている間。jose 自身もキャッシュを持つ)。
let jwksCache: { teamDomain: string; jwks: JWTVerifyGetKey } | null = null;

function jwksFor(teamDomain: string): JWTVerifyGetKey {
  if (jwksCache && jwksCache.teamDomain === teamDomain) return jwksCache.jwks;
  const jwks = createRemoteJWKSet(new URL(`https://${teamDomain}/cdn-cgi/access/certs`));
  jwksCache = { teamDomain, jwks };
  return jwks;
}

/** 管理ホストへのリクエストが本人か。ヘッダ無し・期限切れ・aud 不一致・署名不正・設定欠けはすべて false。 */
export async function verifyAccess(request: Request, env: AccessEnv): Promise<boolean> {
  const teamDomain = env.ACCESS_TEAM_DOMAIN;
  const aud = env.ACCESS_AUD;
  if (!teamDomain || !aud) return false;

  const token = request.headers.get("Cf-Access-Jwt-Assertion");
  if (!token) return false;

  try {
    await jwtVerify(token, jwksFor(teamDomain), { issuer: `https://${teamDomain}`, audience: aud });
    return true;
  } catch {
    return false;
  }
}
