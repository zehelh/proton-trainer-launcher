// proton-trainer GUI logic. Vanilla JS: no framework, no build step.
// Heavy work happens in Rust commands; this file renders state and
// calls invoke().

const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;

const state = {
  games: [],
  config: { schema_version: 1, trainers: [], game_trainers: [] },
  detail: null, // the GameRow whose detail screen is shown
  entries: [],
  plans: [],
  attached: null,
  activeTab: "games",
};

const screens = [
  "games",
  "detail",
  "entries",
  "plan",
  "result",
  "config",
];

function showScreen(name) {
  for (const screen of screens) {
    document.getElementById(`screen-${screen}`).classList.add("hidden");
  }
  document.getElementById(`screen-${name}`).classList.remove("hidden");
}

function setTab(tab) {
  state.activeTab = tab;
  document.getElementById("tab-games").classList.toggle(
    "active",
    tab === "games",
  );
  document.getElementById("tab-config").classList.toggle(
    "active",
    tab === "config",
  );
  if (tab === "games") {
    showScreen("games");
  } else {
    showScreen("config");
    renderConfig();
  }
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

// ---------------------------------------------------------------- games

function renderGames() {
  const list = document.getElementById("game-list");
  list.replaceChildren();
  document
    .getElementById("games-empty")
    .classList.toggle("hidden", state.games.length > 0);
  for (const game of state.games) {
    const li = document.createElement("li");

    const name = document.createElement("span");
    name.textContent = game.name;

    const store = document.createElement("span");
    store.className = "badge";
    store.textContent = game.store;

    const id = document.createElement("small");
    id.textContent = ` (${game.id})`;

    li.append(name, store, id);
    li.addEventListener("click", () => openDetail(game));
    list.appendChild(li);
  }
}

async function refreshGames() {
  const games = await call("list_games", {}, "scanning Steam and Heroic…");
  if (games !== null) {
    state.games = games;
    renderGames();
    showScreen("games");
  }
}

// -------------------------------------------------------------- detail

function openDetail(game) {
  state.detail = game;
  clearBanner();

  document.getElementById("detail-title").textContent = game.name;
  document.getElementById("detail-sub").textContent =
    `${game.store} — ${game.id}` +
    (game.wine_prefix ? ` — prefix: ${game.wine_prefix}` : "");

  renderDetailTrainers();

  const nativeCard = document.getElementById("native-card");
  if (game.native_trainer) {
    nativeCard.classList.remove("hidden");
    document.getElementById("native-note").textContent =
      `This game has a native trainer definition: ${game.native_trainer}`;
  } else {
    nativeCard.classList.add("hidden");
  }

  showScreen("detail");
}

function renderDetailTrainers() {
  const game = state.detail;
  const list = document.getElementById("trainer-list");
  list.replaceChildren();

  const options = [];
  for (const trainer of state.config.trainers) {
    options.push({ label: `${trainer.name} (global)`, path: trainer.path });
  }
  for (const link of state.config.game_trainers) {
    if (link.store === game.store && link.game_id === game.id) {
      options.push({ label: `${link.name} (this game)`, path: link.path });
    }
  }

  document
    .getElementById("no-trainers")
    .classList.toggle("hidden", options.length > 0);

  for (const option of options) {
    const li = document.createElement("li");
    const label = document.createElement("span");
    label.textContent = option.label;
    li.appendChild(label);
    li.addEventListener("click", () => launchTrainer(option.path));
    list.appendChild(li);
  }
}

async function launchPlain() {
  const result = await call(
    "launch",
    { row: state.detail },
    "launching the game…",
  );
  if (result !== null) {
    banner("info", "Game launched.");
  }
}

async function launchTrainer(trainerPath) {
  const result = await call(
    "launch_trainer",
    { row: state.detail, trainerPath },
    "launching the game with the trainer (the prefix may take a while on first run)…",
  );
  if (result !== null) {
    banner(
      "info",
      "Game launched; the trainer is running in the game's prefix.",
    );
  }
}

async function linkTrainer() {
  const picked = await dialog.open({
    title: "Choose a trainer executable",
    filters: [{ name: "Trainers", extensions: ["exe"] }],
    multiple: false,
  });
  if (!picked) {
    return;
  }
  const path = typeof picked === "string" ? picked : picked[0];
  const name = path.split("/").pop();
  state.config.game_trainers.push({
    store: state.detail.store,
    game_id: state.detail.id,
    name,
    path,
  });
  const saved = await call(
    "save_config",
    { config: state.config },
    "saving the configuration…",
  );
  if (saved !== null) {
    renderDetailTrainers();
    banner("info", `Linked ${name} to this game.`);
  }
}

// ------------------------------------------------- native trainer flow

async function openNative() {
  const outcome = await call(
    "attach",
    { trainerId: state.detail.native_trainer },
    "attaching…",
  );
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
  document.getElementById("entries-target").textContent = state.attached
    ? `target: ${state.attached.exe_name} (pid ${state.attached.pid})`
    : "";
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

async function stopSession() {
  await call("stop_session", {}, "releasing the attach…");
  state.attached = null;
  state.entries = [];
  state.plans = [];
}

// --------------------------------------------------------------- config

function renderConfig() {
  const list = document.getElementById("config-trainer-list");
  list.replaceChildren();
  for (const trainer of state.config.trainers) {
    const li = document.createElement("li");
    li.style.cursor = "default";
    const label = document.createElement("span");
    label.textContent = `${trainer.name} — ${trainer.path}`;
    const remove = document.createElement("button");
    remove.textContent = "Remove";
    remove.addEventListener("click", async () => {
      state.config.trainers = state.config.trainers.filter(
        (t) => t.id !== trainer.id,
      );
      await call(
        "save_config",
        { config: state.config },
        "saving the configuration…",
      );
      renderConfig();
    });
    li.append(label, remove);
    list.appendChild(li);
  }

  const gameList = document.getElementById("config-game-trainer-list");
  gameList.replaceChildren();
  document
    .getElementById("no-game-trainers")
    .classList.toggle(
      "hidden",
      state.config.game_trainers.length > 0,
    );
  for (const link of state.config.game_trainers) {
    const li = document.createElement("li");
    li.style.cursor = "default";
    const label = document.createElement("span");
    label.textContent = `${link.name} — ${link.store}/${link.game_id}`;
    const remove = document.createElement("button");
    remove.textContent = "Remove";
    remove.addEventListener("click", async () => {
      state.config.game_trainers = state.config.game_trainers.filter(
        (l) => l !== link,
      );
      await call(
        "save_config",
        { config: state.config },
        "saving the configuration…",
      );
      renderConfig();
    });
    li.append(label, remove);
    gameList.appendChild(li);
  }
}

function slugify(text) {
  return (
    text
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "trainer"
  );
}

async function addTrainer() {
  const name = document.getElementById("new-trainer-name").value.trim();
  const path = document.getElementById("new-trainer-path").value.trim();
  if (!name || !path) {
    banner("info", "Give the trainer a name and a path.");
    return;
  }
  let id = slugify(name);
  let suffix = 2;
  while (state.config.trainers.some((t) => t.id === id)) {
    id = `${slugify(name)}-${suffix}`;
    suffix += 1;
  }
  state.config.trainers.push({ id, name, path });
  const saved = await call(
    "save_config",
    { config: state.config },
    "saving the configuration…",
  );
  if (saved !== null) {
    document.getElementById("new-trainer-name").value = "";
    document.getElementById("new-trainer-path").value = "";
    renderConfig();
    banner("info", `Added ${name}.`);
  }
}

async function browseTrainer() {
  const picked = await dialog.open({
    title: "Choose a trainer executable",
    filters: [{ name: "Trainers", extensions: ["exe"] }],
    multiple: false,
  });
  if (picked) {
    const path = typeof picked === "string" ? picked : picked[0];
    document.getElementById("new-trainer-path").value = path;
  }
}

// ---------------------------------------------------------------- wiring

function wire() {
  document
    .getElementById("tab-games")
    .addEventListener("click", () => setTab("games"));
  document
    .getElementById("tab-config")
    .addEventListener("click", () => setTab("config"));

  document.getElementById("refresh").addEventListener("click", refreshGames);
  document
    .getElementById("back-to-games")
    .addEventListener("click", refreshGames);
  document
    .getElementById("launch-plain")
    .addEventListener("click", launchPlain);
  document
    .getElementById("link-trainer")
    .addEventListener("click", linkTrainer);
  document
    .getElementById("native-open")
    .addEventListener("click", openNative);

  document
    .getElementById("back-to-detail-from-entries")
    .addEventListener("click", async () => {
      await stopSession();
      showScreen("detail");
    });
  document.getElementById("plan-selected").addEventListener("click", plan);
  document.getElementById("apply-selected").addEventListener("click", apply);
  document.getElementById("apply-plan").addEventListener("click", apply);
  document
    .getElementById("back-to-entries")
    .addEventListener("click", () => showScreen("entries"));
  document
    .getElementById("back-to-entries-2")
    .addEventListener("click", () => showScreen("entries"));

  document.getElementById("add-trainer").addEventListener("click", addTrainer);
  document
    .getElementById("browse-trainer")
    .addEventListener("click", browseTrainer);
}

async function init() {
  wire();
  const config = await call("get_config", {}, "loading the configuration…");
  if (config !== null) {
    state.config = config;
  }
  await refreshGames();
}

init();
