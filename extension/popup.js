async function update(type) {
  const el = document.getElementById("status");
  try {
    const status = await chrome.runtime.sendMessage({ type });
    el.textContent = status.connected ? "Browser transport connected. Account login is checked by CLI requests." : status.error || "Bridge disconnected.";
  } catch (error) { el.textContent = String(error.message || error); }
}
document.getElementById("connect").addEventListener("click", () => update("connect"));
document.getElementById("disconnect").addEventListener("click", () => update("disconnect"));
update("status");
setInterval(() => update("status"), 1000);
