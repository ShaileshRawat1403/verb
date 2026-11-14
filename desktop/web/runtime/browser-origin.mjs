// Exact HTTP(S) site grants. Public hosts have no explicit port: never treat every
// default-port website as Verb itself when the browser runs on a companion.
export function browserOrigin(value, forbidden) {
  const url = new URL(value),
    own = new URL(forbidden);
  if (
    !["https:", "http:"].includes(url.protocol) ||
    url.username ||
    url.password
  )
    throw new Error(
      "Use an HTTP or HTTPS address without embedded credentials.",
    );
  if (
    url.origin === own.origin ||
    (own.port ? url.port === own.port : url.hostname === own.hostname)
  )
    throw new Error("Verb itself cannot be opened in this browser session.");
  return url.origin;
}
