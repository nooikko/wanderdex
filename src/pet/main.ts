import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Player } from "../shared/player";
import { drawFrame, setupCanvas } from "../shared/renderer";
import { loadSheets } from "../shared/sheets";
import type { PetInit, Visual } from "../shared/types";

const DRAG_THRESHOLD_PX = 3;

async function main() {
  const canvas = document.getElementById("pet") as HTMLCanvasElement;
  const win = getCurrentWebviewWindow();
  const init = await invoke<PetInit>("pet_ready");
  const sheets = await loadSheets(init.dir, init.table);
  const player = new Player(init.table);
  let scale = init.scale;
  let ctx = setupCanvas(canvas, init.contentBox, scale, window.devicePixelRatio || 1);
  player.play(init.visual, performance.now());

  await win.listen<Visual>("pet:anim", (e) => player.play(e.payload, performance.now()));
  await win.listen<number>("pet:scale", (e) => {
    scale = e.payload;
    ctx = setupCanvas(canvas, init.contentBox, scale, window.devicePixelRatio || 1);
  });

  const render = (now: number) => {
    const f = player.frame(now);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (f) {
      const img = sheets.get(f.anim.source);
      if (img) drawFrame(ctx, img, f.anim, f.dir, f.col, init.contentBox, scale);
    }
    requestAnimationFrame(render);
  };
  requestAnimationFrame(render);

  let down: { x: number; y: number } | null = null;
  let dragging = false;
  let movePending = false;
  canvas.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    canvas.setPointerCapture(e.pointerId);
    down = { x: e.screenX, y: e.screenY };
    dragging = false;
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!down) return;
    if (!dragging && Math.hypot(e.screenX - down.x, e.screenY - down.y) > DRAG_THRESHOLD_PX) {
      dragging = true;
      void invoke("pet_drag_start");
    }
    if (dragging && !movePending) {
      movePending = true;
      requestAnimationFrame(() => {
        movePending = false;
        void invoke("pet_drag_move");
      });
    }
  });
  canvas.addEventListener("pointerup", (e) => {
    if (e.button !== 0 || !down) return;
    canvas.releasePointerCapture(e.pointerId);
    void invoke(dragging ? "pet_drop" : "pet_click");
    down = null;
    dragging = false;
  });
  canvas.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void invoke("pet_context_menu");
  });
}

main().catch((err) => console.error("pet window failed", err));
