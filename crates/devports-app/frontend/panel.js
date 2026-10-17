"use strict";

// Spike only: proves that the panel can call into the core and show the answer.
const { invoke } = window.__TAURI__.core;
const count = document.getElementById("count");

async function show(command) {
  count.textContent = String(await invoke(command));
}

document.getElementById("bump").addEventListener("click", () => show("bump_counter"));
document.getElementById("reset").addEventListener("click", () => show("reset_counter"));
// Tell the core the first frame is painted so it can show the window; two
// animation frames guarantee the browser has actually drawn it.
show("get_counter").finally(() => {
  requestAnimationFrame(() => requestAnimationFrame(() => invoke("panel_ready")));
});
