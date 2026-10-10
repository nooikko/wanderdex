import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { DragTracker } from "../shared/drag";
import { Player } from "../shared/player";
import { drawFrame, setupCanvas } from "../shared/renderer";
import { loadSheets } from "../shared/sheets";
import type { PetInit, Visual } from "../shared/types";

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

  const drag = new DragTracker((cmd) => invoke(cmd));
  canvas.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    canvas.setPointerCapture(e.pointerId);
    drag.down(e.screenX, e.screenY);
  });
  canvas.addEventListener("pointermove", (e) => drag.move(e.screenX, e.screenY));
  canvas.addEventListener("pointerup", (e) => {
    if (e.button !== 0) return;
    if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
    void drag.up();
  });
  canvas.addEventListener("pointercancel", () => void drag.cancel());
  canvas.addEventListener("lostpointercapture", () => void drag.cancel());
  canvas.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void invoke("pet_context_menu");
  });
}

main().catch((err) => console.error("pet window failed", err));
