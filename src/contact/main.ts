import { invoke } from "@tauri-apps/api/core";
import { loadSheets } from "../shared/sheets";
import { drawFrame, frameDest } from "../shared/renderer";
import type { VariantPayload } from "../shared/types";

const SCALE = 4;
const PAD = 12;
const LABEL = 18;
const COLUMNS = 5;

async function main() {
  const variant = new URLSearchParams(location.search).get("variant") ?? "0202";
  const payload = await invoke<VariantPayload>("load_variant", { path: variant });
  const sheets = await loadSheets(payload.dir, payload.table);
  const cellW = Math.max(...payload.table.map((a) => a.frameWidth)) * SCALE + PAD * 2;
  const cellH = Math.max(...payload.table.map((a) => a.frameHeight)) * SCALE + PAD * 2 + LABEL;
  const rows = Math.ceil(payload.table.length / COLUMNS);
  const canvas = document.getElementById("sheet") as HTMLCanvasElement;
  canvas.width = cellW * COLUMNS;
  canvas.height = cellH * rows;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!; // 1 canvas px = 1 CSS px: exact checks
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const lines: string[] = [`Contact sheet for ${variant} (${payload.table.length} animations, ${SCALE}x)`];
  let ok = true;

  payload.table.forEach((anim, i) => {
    const x0 = (i % COLUMNS) * cellW + PAD;
    const y0 = Math.floor(i / COLUMNS) * cellH + PAD;
    const fw = anim.frameWidth * SCALE;
    const fh = anim.frameHeight * SCALE;
    ctx.fillStyle = "#eee";
    ctx.fillRect(x0, y0, fw, fh);
    // Draw with the app renderer, using a box whose anchor puts the frame exactly in this cell.
    const box = { minX: -anim.frameWidth / 2, minY: -anim.frameHeight / 2, maxX: anim.frameWidth / 2, maxY: anim.frameHeight / 2 };
    const d = frameDest(anim, box, SCALE);
    ctx.save();
    ctx.translate(x0 - d.dx, y0 - d.dy);
    drawFrame(ctx, sheets.get(anim.source)!, anim, 0, 0, box, SCALE);
    ctx.restore();
    ctx.strokeStyle = "#d33";
    ctx.strokeRect(x0 - 0.5, y0 - 0.5, fw + 1, fh + 1);
    ctx.fillStyle = "#000";
    ctx.font = "12px system-ui";
    ctx.fillText(`${anim.name}${anim.source !== anim.name ? ` → ${anim.source}` : ""} ${anim.frameWidth}×${anim.frameHeight} rows ${anim.rows}`, x0, y0 + fh + 14);

    const px = ctx.getImageData(x0, y0, fw, fh).data;
    let off = 0;
    for (let y = 0; y < fh; y++) {
      for (let x = 0; x < fw; x++) {
        const a = (y * fw + x) * 4;
        const b = ((y - (y % SCALE)) * fw + (x - (x % SCALE))) * 4;
        if (px[a] !== px[b] || px[a + 1] !== px[b + 1] || px[a + 2] !== px[b + 2] || px[a + 3] !== px[b + 3]) off++;
      }
    }
    if (off > 0) ok = false;
    lines.push(`${anim.name.padEnd(10)} ${off === 0 ? "crisp" : `${off} non-uniform pixels`}`);
  });
  lines.push(ok ? "PASS" : "FAIL");
  document.getElementById("report")!.textContent = lines.join("\n");
  await invoke("save_contact_sheet", { dataUrl: canvas.toDataURL("image/png"), report: lines.join("\n"), ok });
}

main().catch((e) => {
  document.getElementById("report")!.textContent = String(e);
  void invoke("save_contact_sheet", { dataUrl: "data:,", report: `ERROR ${e}`, ok: false });
});
