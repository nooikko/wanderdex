import { useEffect, useRef } from "react";
import { Player } from "../shared/player";
import { drawFrame, setupCanvas } from "../shared/renderer";
import { loadSheets } from "../shared/sheets";
import { DIR, type VariantPayload, type Visual } from "../shared/types";

const FLOURISHES = ["Pose", "Hop", "Rotate", "Charge"]; // matches brain::FLOURISH_PREFERENCE
const PREVIEW_SCALE = 3;

/** Cycles Idle → Walk → signature/flourish using the same renderer as the pets. */
export function Preview({ payload }: { payload: VariantPayload }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let alive = true;
    let raf = 0;
    const has = (n: string) => payload.table.some((a) => a.name === n);
    const move = payload.signature && has(payload.signature) ? payload.signature : FLOURISHES.find(has);
    const steps: { v: Omit<Visual, "seq">; ms: number }[] = [
      { v: { anim: "Idle", dir: DIR.Down, looped: true }, ms: 2500 },
      { v: { anim: "Walk", dir: DIR.Right, looped: true }, ms: 2500 },
    ];
    if (move) {
      const secs = payload.table.find((a) => a.name === move)!.durations.reduce((a, b) => a + b, 0) / 60;
      steps.push({ v: { anim: move, dir: DIR.Down, looped: false }, ms: Math.max(1000, secs * 1000 + 500) });
    }
    loadSheets(payload.dir, payload.table).then((sheets) => {
      if (!alive || !canvasRef.current) return;
      const canvas = canvasRef.current;
      const ctx = setupCanvas(canvas, payload.contentBox, PREVIEW_SCALE, window.devicePixelRatio || 1);
      const player = new Player(payload.table);
      let step = 0;
      let seq = 0;
      let stepStart = performance.now();
      player.play({ ...steps[0].v, seq: seq++ }, stepStart);
      const render = (now: number) => {
        if (now - stepStart > steps[step].ms) {
          step = (step + 1) % steps.length;
          stepStart = now;
          player.play({ ...steps[step].v, seq: seq++ }, now);
        }
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        const f = player.frame(now);
        const img = f && sheets.get(f.anim.source);
        if (f && img) drawFrame(ctx, img, f.anim, f.dir, f.col, payload.contentBox, PREVIEW_SCALE);
        raf = requestAnimationFrame(render);
      };
      raf = requestAnimationFrame(render);
    });
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
    };
  }, [payload]);
  return <canvas ref={canvasRef} className="preview" />;
}
