export async function readApiResponse(response) {
  if (response.type === "opaqueredirect" || (response.status >= 300 && response.status < 400)) {
    throw Object.assign(new Error("Verb redirected this request. Reload the page and sign in if prompted."), {
      code: "access_redirect",
      status: response.status || 302,
    });
  }
  const type = response.headers.get("content-type")?.split(";")[0].trim().toLowerCase();
  let value;
  if (type === "application/json" || type?.endsWith("+json")) {
    try {
      value = await response.json();
    } catch {
      // A proxy or interrupted response may return a body that is not valid JSON.
    }
  }
  if (!response.ok || value === undefined) {
    const message = typeof value?.error === "string" && value.error
      ? value.error
      : [401, 403].includes(response.status)
        ? "Access to Verb was denied. Reload the page and sign in if prompted."
        : response.ok
          ? "Verb returned an unexpected response. Reload the page or try again."
          : `Request failed (HTTP ${response.status}). Try again.`;
    throw Object.assign(new Error(message), { status: response.status, code: value?.code });
  }
  return value;
}
