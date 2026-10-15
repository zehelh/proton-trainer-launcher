// proton-trainer GUI logic. Vanilla JS: no framework, no build step.
// All heavy work happens in Rust commands (see src/commands.rs); this
// file only renders state and calls invoke().

const { invoke } = window.__TAURI__.core;

const state = {
  games: [],
  entries: [],
  plans: [],
  attached: null,
};

const screens = ["games", "entries", "plan", "result"];

function showScreen(name) {
  for (const screen of screens) {
    document.getElementById(`screen-${screen}`).classList.add("hidden");
  }
  document.getElementById(`screen-${name}`).classList.remove("hidden");
}

function setStatus(text) {
  document.getElementById("status").textContent = text;
}

function banner(kind, text) {
  const el = document.getElementById("banner");
  el.className = `banner ${kind}`;
  el.textContent = text;
  el.classList.remove("hidden");
}

function clearBanner() {
  document.getElementById("banner").classList.add("hidden");
}

async function call(name, args, busyLabel) {
  setStatus(busyLabel);
  for (const button of document.querySelectorAll("button")) {
    button.disabled = true;
  }
  try {
    return await invoke(name, args);
  } catch (error) {
    banner("error", String(error));
    return null;
  } finally {
    for (const button of document.querySelectorAll("button")) {
      button.disabled = false;
    }
    setStatus("idle");
  }
}

function renderGames() {
  const list = document.getElementById("game-list");
  list.replaceChildren();
  document.getElementById("games-empty").classList.toggle(
    "hidden",
    state.games.length > 0,
  );
  for (const game of state.games) {
    const li = document.createElement("li");
    li.textContent = "";
    const name = document.createElement("span");
    name.textContent = game.name;
    const id = document.createElement("small");
    id.textContent = ` (${game.app_id})`;
    li.append(name, id);
    li.addEventListener("click", () => attach(game.trainer_id));
    list.appendChild(li);
  }
}

async function refreshGames() {
  const games = await call("list_games", {}, "scanning Steam libraries…");
  if (games !== null) {
    state.games = games;
    renderGames();
    showScreen("games");
  }
}

function renderEntries() {
  const list = document.getElementById("entry-list");
  list.replaceChildren();
  for (const entry of state.entries) {
    const li = document.createElement("li");
    const label = document.createElement("label");
    label.style.display = "flex";
    label.style.alignItems = "center";
    label.style.gap = "0.6rem";

    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = true;
    label.appendChild(checkbox);

    const text = document.createElement("span");
    text.textContent = entry.label;
    label.appendChild(text);

    if (entry.freeze) {
      const badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = "freeze";
      label.appendChild(badge);
    }

    li.appendChild(label);
    list.appendChild(li);
  }
  document.getElementById("entries-target").textContent =
    state.attached
      ? `target: ${state.attached.exe_name} (pid ${state.attached.pid})`
      : "";
}

async function attach(trainerId) {
  const outcome = await call("attach", { trainerId }, "attaching…");
  if (outcome === null) {
    return;
  }
  if (outcome.Refused) {
    banner("error", `REFUSED: ${outcome.Refused.explanation}`);
    return;
  }
  if (outcome.NotRunning) {
    banner(
      "info",
      `No running process found for ${JSON.stringify(
        outcome.NotRunning.exe_names,
      )}. Launch the game, then retry.`,
    );
    return;
  }
  clearBanner();
  state.attached = outcome.Running;
  state.entries = outcome.Running.entries;
  renderEntries();
  showScreen("entries");
}

function selectedEntryIds() {
  const ids = [];
  const items = document.getElementById("entry-list").children;
  state.entries.forEach((entry, index) => {
    const checkbox = items[index]?.querySelector("input");
    if (checkbox && checkbox.checked) {
      ids.push(entry.id);
    }
  });
  return ids;
}

async function plan() {
  const entryIds = selectedEntryIds();
  if (entryIds.length === 0) {
    banner("info", "No entries selected.");
    return;
  }
  const plans = await call(
    "plan",
    { entryIds },
    "scanning the target's memory… (this can take a while)",
  );
  if (plans === null) {
    return;
  }
  clearBanner();
  state.plans = plans;
  const text = plans
    .map(
      (p) =>
        `${p.entry_id} (${p.label})${p.freeze ? " [freeze]" : ""}\n` +
        `  pid      : ${p.pid}\n` +
        `  address  : 0x${p.address.toString(16)}\n` +
        `  region   : 0x${p.region_start.toString(16)}-0x${p.region_end.toString(16)}\n` +
        `  old      : ${p.old_hex}\n` +
        `  new      : ${p.new_hex}`,
    )
    .join("\n");
  document.getElementById("plan-text").textContent = text || "(no plans)";
  showScreen("plan");
}

async function apply() {
  const results = await call("apply", {}, "applying and verifying…");
  if (results === null) {
    return;
  }
  clearBanner();
  const list = document.getElementById("result-list");
  list.replaceChildren();
  let frozen = 0;
  for (const result of results) {
    const li = document.createElement("li");
    const label = document.createElement("span");
    label.textContent = `${result.entry_id} (${result.label}): `;
    li.appendChild(label);
    const status = document.createElement("span");
    if (result.outcome.Ok !== undefined) {
      status.className = "status-ok";
      status.textContent = "applied and verified";
    } else {
      status.className = "status-fail";
      status.textContent = `FAILED - ${result.outcome.Err}`;
    }
    li.appendChild(status);
    list.appendChild(li);
    frozen += state.plans.filter(
      (p) => p.entry_id === result.entry_id && p.freeze,
    ).length;
  }
  const note = document.getElementById("freeze-note");
  note.textContent = frozen
    ? "freeze: active for the entries marked [freeze] — leave the app open; closing the app or pressing Back stops it"
    : "";
  note.classList.toggle("hidden", frozen === 0);
  showScreen("result");
}

async function backToGames() {
  await call("stop_session", {}, "releasing the attach…");
  state.attached = null;
  state.entries = [];
  state.plans = [];
  showScreen("games");
}

function backToEntries() {
  showScreen("entries");
}

function wire() {
  document.getElementById("refresh").addEventListener("click", refreshGames);
  document
    .getElementById("back-to-games")
    .addEventListener("click", backToGames);
  document.getElementById("plan-selected").addEventListener("click", plan);
  document.getElementById("apply-selected").addEventListener("click", apply);
  document.getElementById("apply-plan").addEventListener("click", apply);
  document
    .getElementById("back-to-entries")
    .addEventListener("click", backToEntries);
  document
    .getElementById("back-to-entries-2")
    .addEventListener("click", backToEntries);
}

wire();
refreshGames();
