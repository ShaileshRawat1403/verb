// Optional Cloudflare Access service token, so the journeys can run against the public URL. The
// owner creates the token; it is read from the environment, sent as headers on every request and
// never printed or recorded.
export function accessHeaders(env = process.env) {
  const id = env.VERB_ACCESS_CLIENT_ID;
  const secret = env.VERB_ACCESS_CLIENT_SECRET;
  if (!id || !secret) return {};
  return { "CF-Access-Client-Id": id, "CF-Access-Client-Secret": secret };
}
