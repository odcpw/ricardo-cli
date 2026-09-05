// Read routes and narrowly scoped saved-article actions only. No cookie
// permission, arbitrary JavaScript, external origins, bids, or messages.
export function validatePath(path) {
  if (typeof path !== "string" || !path.startsWith("/") || path.startsWith("//") || /[\\\x00-\x20]/.test(path)) throw new Error("Invalid Ricardo path");
  const url = new URL(path, "https://www.ricardo.ch");
  if (url.origin !== "https://www.ricardo.ch" || url.hash || /%2f|%5c|%2e/i.test(path)) throw new Error("Invalid Ricardo path");
  const allowed = /^\/de\/s\/[^/]+\/?$/.test(url.pathname)
    || /^\/de\/a\/(?:[^/]*-)?\d+\/?$/.test(url.pathname)
    || ["/de/", "/de/my-ricardo/saved/articles/", "/api/frontend/notifications", "/api/mfa/notifications", "/api/orders-spa/buying/offers", "/api/orders-spa/saved-articles", "/de/my-ricardo/selling/offers/"].includes(url.pathname);
  if (!allowed) throw new Error("Route is not an approved read endpoint");
  return url;
}
export async function handleRequest(request, chromeApi = globalThis.chrome) {
  if (request?.op === "resolve") {
    if (typeof request.article !== "string" || !/^\d{1,20}$/.test(request.article)) throw new Error("Article must be a numeric Ricardo ID");
    const tabs = await chromeApi.tabs.query({url: "https://www.ricardo.ch/*"});
    const tab = tabs.find(t => t.active) || tabs[0];
    if (!tab?.id) throw new Error("Open Ricardo in this Chrome profile and sign in, then retry");
    const results = await chromeApi.scripting.executeScript({
      target: {tabId: tab.id}, world: "MAIN", args: [request.article],
      func: async (article) => {
        if (location.origin !== "https://www.ricardo.ch") throw new Error("Ricardo tab changed origin");
        // same-origin mode rejects external redirects before fetching them.
        const response = await fetch(`https://www.ricardo.ch/de/a/${article}/`, {
          method: "GET", credentials: "include", cache: "no-store",
          mode: "same-origin", redirect: "follow", signal: AbortSignal.timeout(10000)
        });
        try {
          if (response.status !== 200) throw new Error(`Article resolution returned HTTP ${response.status}`);
          const resolved = new URL(response.url);
          if (resolved.origin !== "https://www.ricardo.ch") throw new Error("Article redirected outside Ricardo");
          if (!/^\/de\/a\/[^/]+-\d+\/$/.test(resolved.pathname) || !resolved.pathname.endsWith(`-${article}/`)) throw new Error("Article did not resolve to its canonical listing; it may be unavailable or blocked");
          return {url: resolved.origin + resolved.pathname};
        } finally { await response.body?.cancel().catch(() => {}); }
      }
    });
    if (results[0]?.error) throw new Error(results[0].error.message || "Article URL resolution failed");
    if (!results[0]?.result) throw new Error("Browser returned no article URL");
    return results[0].result;
  }

  let url; let method = "GET"; let body = null;
  if (request?.op === "favorite") {
    if (Object.keys(request).some(key => !["op", "article", "action"].includes(key))) throw new Error("Favorite request has unsupported fields");
    if (typeof request.article !== "string" || !/^\d{1,20}$/.test(request.article)) throw new Error("Article must be a numeric Ricardo ID");
    if (!["add", "remove"].includes(request.action)) throw new Error("Favorite action must be add or remove");
    method = request.action === "add" ? "POST" : "DELETE";
    const path = "/marketplace-spa/api/saved-articles" + (request.action === "remove" ? `/${request.article}` : "");
    url = new URL(path, "https://www.ricardo.ch");
    if (request.action === "add") body = JSON.stringify({articleId: request.article});
  } else {
    if (!request || request.op !== "fetch") throw new Error("Unsupported browser operation; only fetch, resolve and favorite are enabled");
    url = validatePath(request.path);
  }
  const tabs = await chromeApi.tabs.query({ url: "https://www.ricardo.ch/*" });
  const tab = tabs.find(t => t.active) || tabs[0];
  if (!tab?.id) throw new Error("Open Ricardo in this Chrome profile and sign in, then retry");
  const results = await chromeApi.scripting.executeScript({
    target: { tabId: tab.id }, world: "MAIN", args: [url.href, method, body],
    func: async (url, method, body) => {
      if (location.origin !== "https://www.ricardo.ch") throw new Error("Ricardo tab changed origin");
      const options = { method, credentials: "include", cache: "no-store", redirect: "error", signal: AbortSignal.timeout(30000) };
      if (body !== null) { options.body = body; options.headers = {"Content-Type": "application/json"}; }
      // One attempt only: a failed response does not prove a mutation was rejected.
      const response = await fetch(url, options);
      const reader = response.body?.getReader();
      const chunks = []; let size = 0;
      if (reader) {
        while (true) {
          const chunk = await reader.read(); if (chunk.done) break;
          size += chunk.value.byteLength;
          if (size > 4 * 1024 * 1024) { await reader.cancel(); throw new Error("Ricardo response exceeds 4 MiB limit"); }
          chunks.push(chunk.value);
        }
      }
      const data = new Uint8Array(size); let offset = 0;
      for (const chunk of chunks) { data.set(chunk, offset); offset += chunk.length; }
      return { status: response.status, url: response.url, content_type: response.headers.get("content-type"), body: new TextDecoder().decode(data), checked_at: new Date().toISOString() };
    }
  });
  const result = results[0];
  if (result?.error) throw new Error(result.error.message || "Browser fetch failed");
  if (!result?.result) throw new Error("Browser returned no result");
  return result.result;
}
let port = null;
let lastError = null;
function connect() {
  if (port) return;
  lastError = null;
  port = chrome.runtime.connectNative("com.odcpw.ricardo_cli");
  port.onMessage.addListener(async (message) => {
    if (!message || typeof message.id !== "string") return;
    const current = port;
    try { const result = await handleRequest(message.request); current?.postMessage({ id: message.id, result }); }
    catch (error) { current?.postMessage({ id: message.id, error: { code: "browser_error", message: String(error.message || error) } }); }
  });
  port.onDisconnect.addListener(() => { lastError = chrome.runtime.lastError?.message || "Disconnected"; port = null; });
}
if (globalThis.chrome?.runtime?.onMessage) {
  chrome.runtime.onMessage.addListener((message, _sender, respond) => {
    if (message?.type === "connect") connect();
    if (message?.type === "disconnect") { port?.disconnect(); port = null; lastError = null; }
    if (["connect", "disconnect", "status"].includes(message?.type)) respond({ connected: Boolean(port), error: lastError });
  });
}
