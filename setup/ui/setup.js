// Wanderdex Setup UI: the dusk scene, Wobbuffet, and the install flow.
(() => {
  const { invoke } = window.__TAURI__.core;
  const appWindow = window.__TAURI__.window.getCurrentWindow();
  const $ = (id) => document.getElementById(id);

  // ---- Scene: a 320x200 pixel canvas, scaled 2x by CSS. ----
  const W = 320, H = 200, GROUND = 168;
  const g = $("scene").getContext("2d");
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const BANDS = ["#1b1640", "#241b52", "#2f2163", "#3d2872", "#53307d", "#6d3a84", "#8a4687", "#a85686",
    "#c66a82", "#de8580", "#eea283", "#f6bd8f", "#fad3a3", "#fde3b8"];
  let seed = 202;
  const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const pick = (a) => a[Math.floor(rnd() * a.length)];
  const stars = Array.from({ length: 70 }, () => ({ x: Math.floor(rnd() * W), y: Math.floor(4 + rnd() * 58), p: rnd() * 6.28, c: pick(["#ffffff", "#cfd2ff", "#ffe9a8"]) }));
  const clouds = [{ x: 18, y: 104, w: 34, v: 0.05 }, { x: 250, y: 92, w: 40, v: 0.035 }, { x: 20, y: 62, w: 26, v: 0.025, warm: true }];
  const flowers = Array.from({ length: 9 }, (_, i) => 6 + i * 36 + Math.floor(rnd() * 10));
  const dirt = Array.from({ length: 90 }, () => [Math.floor(rnd() * W), Math.floor(177 + rnd() * 23), pick(["#6e4636", "#4a2f25", "#7a5040"])]);
  const px = (x, y, w, h, c) => { g.fillStyle = c; g.fillRect(x, y, w, h); };

  // ---- Wobbuffet, downloaded from SpriteCollab at the commit Wanderdex's index is pinned to.
  // CC BY-NC 4.0 art (CHUNSOFT), so it's fetched at runtime rather than shipped. Durations are 1/60 s ticks;
  // x/y/w/h crop each frame to the sprite so its feet sit on the grass. ----
  const SPRITES = "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/01b0a2d31e20ab2f4bfec1fa4a13ae566977579b/sprite/0202/";
  const ANIMS = {
    walkR: { sheet: "Walk", fw: 24, fh: 32, row: 2, x: 0, y: 0, w: 21, h: 23, durs: [8, 10, 8, 10] },
    walkL: { sheet: "Walk", fw: 24, fh: 32, row: 6, x: 3, y: 0, w: 21, h: 23, durs: [8, 10, 8, 10] },
    hop: { sheet: "Hop", fw: 32, fh: 80, row: 0, x: 4, y: 1, w: 24, h: 47, durs: [2, 1, 2, 3, 4, 4, 3, 2, 1, 2] },
    idle: { sheet: "Idle", fw: 24, fh: 40, row: 0, x: 4, y: 3, w: 16, h: 24, durs: [40, 4, 4, 4, 4, 4, 4, 4, 4] },
  };
  const sheets = {};
  let wobbuffet = "loading"; // "loading" | "ready" | "missing"
  Promise.all(["Walk", "Hop", "Idle"].map((name) => new Promise((ok, fail) => {
    const img = new Image();
    img.onload = () => ok((sheets[name] = img));
    img.onerror = fail;
    img.src = `${SPRITES}${name}-Anim.png`;
  })))
    .then(() => (wobbuffet = "ready"))
    .catch(() => (wobbuffet = "missing"));
  setTimeout(() => wobbuffet === "loading" && (wobbuffet = "missing"), 5000);
  for (const a of Object.values(ANIMS)) a.total = a.durs.reduce((s, d) => s + d, 0);

  function sprite(key, x, ms, once) {
    if (wobbuffet !== "ready") return;
    const a = ANIMS[key];
    let tick = Math.floor(ms / (1000 / 60));
    tick = once ? Math.min(tick, a.total - 1) : tick % a.total;
    let i = 0;
    while (tick >= a.durs[i]) tick -= a.durs[i++];
    g.drawImage(sheets[a.sheet], i * a.fw + a.x, a.row * a.fh + a.y, a.w, a.h, Math.round(x), GROUND + 2 - a.h, a.w, a.h);
  }

  function cloud(c, t) {
    const x = ((c.x + (reduce ? 0 : t * c.v * 0.06)) % (W + 60)) - 20;
    const f = c.warm ? "#f7c6b4" : "#fff0dc", s = c.warm ? "#d99a9a" : "#f3c9a8";
    px(x + 3, c.y, c.w - 6, 3, f); px(x, c.y + 3, c.w, 4, f); px(x + c.w / 3, c.y - 3, c.w / 3, 3, f); px(x + 1, c.y + 6, c.w - 2, 2, s);
  }

  const st = { screen: "welcome", progress: 0, walkX: 14, doneAt: 0 };

  function draw(t) {
    const bh = GROUND / BANDS.length;
    BANDS.forEach((c, i) => {
      const y0 = Math.floor(i * bh), y1 = Math.floor((i + 1) * bh);
      px(0, y0, W, y1 - y0 + 1, c);
      if (i + 1 < BANDS.length) for (let x = 0; x < W; x += 2) px(x, y1 - 1, 1, 1, BANDS[i + 1]);
    });
    for (const s of stars) {
      const tw = reduce ? 1 : 0.5 + 0.5 * Math.sin(t / 700 + s.p);
      if (tw > 0.25) { g.globalAlpha = tw; px(s.x, s.y, 1, 1, s.c); g.globalAlpha = 1; }
    }
    // Crescent moon: the pixels of one disc that aren't inside a second, offset disc.
    for (let y = -10; y <= 10; y++) for (let x = -10; x <= 10; x++)
      if (x * x + y * y <= 100 && (x - 5) ** 2 + (y + 3) ** 2 > 100) px(278 + x, 36 + y, 1, 1, "#fff4cf");
    clouds.forEach((c) => cloud(c, t));
    for (let x = 0; x < W; x++) {
      px(x, Math.floor(160 - 14 * (0.5 + 0.5 * Math.sin((x + 5) / 23))), 1, 12, "#c98a9a");
      px(x, Math.floor(166 - 9 * (0.5 + 0.5 * Math.sin((x + 40) / 15))), 1, 6, "#a8728f");
    }
    px(0, GROUND, W, H - GROUND, "#5b3a2e"); px(0, GROUND, W, 5, "#58b04a"); px(0, GROUND, W, 1, "#8fd16a");
    for (let x = 0; x < W; x += 4) px(x, GROUND + 6, 1, 1, "#58b04a");
    dirt.forEach(([x, y, c]) => px(x, y, 1, 1, c));
    flowers.forEach((x) => { px(x, GROUND - 2, 1, 1, "#ffe066"); px(x, GROUND - 1, 1, 1, "#3f8f3a"); });

    const anim = reduce ? 0 : t;
    if (st.screen === "error") {
      sprite("idle", 24, anim); // step aside so the Try again button stays clear
    } else if (st.screen === "install") {
      const end = 8 + st.progress * (W - 36);
      st.walkX += (end - st.walkX) * 0.12;
      if (wobbuffet === "missing") {
        // No sprite (offline): a plain pixel progress bar along the grass instead.
        px(8, GROUND + 9, W - 16, 4, "#3a241c");
        px(8, GROUND + 9, Math.max(0, st.walkX - 8 + 21), 4, "#ffd27a");
      } else {
        for (let x = 12; x < st.walkX - 2; x += 7) px(x, GROUND + 2, 2, 1, "#3f8f3a");
        sprite("walkR", st.walkX, anim);
      }
    } else if (st.screen === "done") {
      for (let x = 12; x < W - 40; x += 7) px(x, GROUND + 2, 2, 1, "#3f8f3a");
      const cycle = reduce ? 0 : (t - st.doneAt) % 2600; // a hop, then a beat of idle
      const hopMs = (ANIMS.hop.total * 1000) / 60;
      if (cycle < hopMs) sprite("hop", W - 40, cycle, true);
      else sprite("idle", W - 36, cycle - hopMs);
    } else {
      const span = W - 52, p = reduce ? 0.5 : (t / 9000) % 2;
      st.walkX = 14 + span * (p < 1 ? p : 2 - p);
      sprite(p < 1 ? "walkR" : "walkL", st.walkX, anim);
    }
  }
  const frame = (t) => { draw(t); requestAnimationFrame(frame); };
  requestAnimationFrame(frame);

  // ---- Screens and the install flow. ----
  const SCREENS = ["welcome", "options", "install", "done", "error"];
  function show(name) {
    st.screen = name;
    if (name === "done") st.doneAt = performance.now();
    for (const id of SCREENS) {
      const el = $("s-" + id);
      el.hidden = id !== name;
      if (id === name) {
        el.classList.add("enter");
        requestAnimationFrame(() => requestAnimationFrame(() => el.classList.remove("enter")));
        el.querySelector(".px-btn")?.focus({ preventScroll: true });
      }
    }
  }

  const opts = { dir: "", desktopShortcut: false, launchAtLogin: true };
  let exe = "";

  invoke("defaults").then((d) => {
    opts.dir = d.dir;
    $("path").textContent = d.dir;
    $("path").title = d.dir;
    if (d.installed) $("install").textContent = "Update";
  });
  window.__TAURI__.app.getVersion().then((v) => ($("meta").textContent = `v${v} · for your account · no admin needed`));

  const STEPS = [[0, "Unpacking files"], [0.3, "Copying Wanderdex"], [0.65, "Adding the Start menu shortcut"], [0.85, "Registering the uninstaller"]];
  async function runInstall() {
    show("install");
    st.progress = 0; st.walkX = 8;
    // The installer runs silently, so progress eases toward 90% while it works and finishes when it returns.
    const timer = setInterval(() => {
      st.progress += (0.9 - st.progress) * 0.025;
      $("pct").textContent = Math.round(st.progress * 100) + "%";
      $("status").textContent = STEPS.filter((s) => st.progress >= s[0]).pop()[1];
    }, 60);
    try {
      exe = await invoke("install", { options: opts });
      clearInterval(timer);
      $("status").textContent = "Finishing up";
      await new Promise((ok) => {
        const fin = setInterval(() => {
          st.progress = Math.min(1, st.progress + 0.03);
          $("pct").textContent = Math.round(st.progress * 100) + "%";
          if (st.progress >= 1) { clearInterval(fin); ok(); }
        }, 30);
      });
      setTimeout(() => show("done"), 400);
    } catch (e) {
      clearInterval(timer);
      $("error").textContent = String(e);
      show("error");
    }
  }

  $("install").onclick = runInstall;
  $("retry").onclick = runInstall;
  $("open-options").onclick = () => show("options");
  $("options-done").onclick = () => show("welcome");
  $("browse").onclick = async () => {
    const dir = await invoke("pick_folder");
    if (dir) {
      opts.dir = dir.endsWith("Wanderdex") ? dir : dir.replace(/[\\/]+$/, "") + "\\Wanderdex";
      $("path").textContent = opts.dir;
      $("path").title = opts.dir;
    }
  };
  $("opt-desk").onchange = (e) => (opts.desktopShortcut = e.target.checked);
  $("opt-login").onchange = (e) => {
    opts.launchAtLogin = e.target.checked;
    $("done-login").checked = e.target.checked;
  };
  $("done-login").onchange = (e) => {
    opts.launchAtLogin = e.target.checked;
    $("opt-login").checked = e.target.checked;
    invoke("set_launch_at_login", { exe, enabled: e.target.checked }).catch(() => {});
  };
  $("launch").onclick = () => invoke("launch", { exe }).catch((e) => { $("error").textContent = String(e); show("error"); });
  $("min").onclick = () => appWindow.minimize();
  $("close").onclick = () => st.screen !== "install" && appWindow.close(); // don't abandon a running install
})();
