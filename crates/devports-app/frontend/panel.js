"use strict";

// Spike only: proves that the panel can call into the core and show the answer.
const { invoke } = window.__TAURI__.core;
const count = document.getElementById("count");

async function show(command) {
  count.textContent = String(await invoke(command));
}

document.getElementById("bump").addEventListener("click", () => show("bump_counter"));
document.getElementById("reset").addEventListener("click", () => show("reset_counter"));
show("get_counter");
