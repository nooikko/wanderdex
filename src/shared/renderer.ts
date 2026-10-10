import type { AnimInfo, ContentBox } from "./types";

export function frameSource(anim: AnimInfo, dir: number, col: number) {
  const row = anim.rows <= 1 ? 0 : dir % anim.rows;
  return { sx: col * anim.frameWidth, sy: row * anim.frameHeight, sw: anim.frameWidth, sh: anim.frameHeight };
}

/** Where a frame lands in the canvas (CSS px). Every frame's center sits on the anchor (-minX, -minY). */
export function frameDest(anim: AnimInfo, box: ContentBox, scale: number) {
  const ax = -box.minX;
  const ay = -box.minY;
  return {
    dx: (ax - Math.floor(anim.frameWidth / 2)) * scale,
    dy: (ay - Math.floor(anim.frameHeight / 2)) * scale,
    dw: anim.frameWidth * scale,
    dh: anim.frameHeight * scale,
  };
}

export function canvasSize(box: ContentBox, scale: number) {
  return { width: (box.maxX - box.minX) * scale, height: (box.maxY - box.minY) * scale };
}

/** Sizes the canvas (CSS px × device pixel ratio) and returns a context with smoothing off. */
export function setupCanvas(canvas: HTMLCanvasElement, box: ContentBox, scale: number, dpr: number): CanvasRenderingContext2D {
  const { width, height } = canvasSize(box, scale);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = false;
  return ctx;
}

export function drawFrame(
  ctx: CanvasRenderingContext2D,
  img: CanvasImageSource,
  anim: AnimInfo,
  dir: number,
  col: number,
  box: ContentBox,
  scale: number,
): void {
  const s = frameSource(anim, dir, col);
  const d = frameDest(anim, box, scale);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, s.sx, s.sy, s.sw, s.sh, d.dx, d.dy, d.dw, d.dh);
}
