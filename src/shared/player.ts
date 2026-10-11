import type { AnimInfo, Visual } from "./types";

/**
 * Frame column at a tick (60/s) and the tick it changes at; null once a non-looping animation has finished.
 * `nextTick` is null when the column never changes again (last frame of a one-shot).
 */
export function frameAt(durations: number[], tick: number, looped: boolean): { col: number; nextTick: number | null } | null {
  const total = durations.reduce((a, b) => a + b, 0);
  if (!looped && tick >= total) return null;
  const base = looped ? tick - (tick % total) : 0;
  let end = 0;
  for (let i = 0; i < durations.length; i++) {
    end += durations[i];
    if (tick - base < end) return { col: i, nextTick: !looped && i === durations.length - 1 ? null : base + end };
  }
  return { col: durations.length - 1, nextTick: null };
}

/** Frame column at a tick (60/s); null once a non-looping animation has finished. */
export function columnAt(durations: number[], tick: number, looped: boolean): number | null {
  return frameAt(durations, tick, looped)?.col ?? null;
}

export class Player {
  private byName: Map<string, AnimInfo>;
  private visual: Visual | null = null;
  private startMs = 0;

  constructor(table: AnimInfo[]) {
    this.byName = new Map(table.map((a) => [a.name, a]));
  }

  play(v: Visual, nowMs: number): void {
    const prev = this.visual;
    if (!prev || prev.anim !== v.anim || prev.seq !== v.seq) this.startMs = nowMs;
    this.visual = v;
  }

  /** The frame to show at `nowMs`, and when the next one is due (null: never, until the next `play`). */
  frame(nowMs: number): { anim: AnimInfo; dir: number; col: number; nextMs: number | null } | null {
    if (!this.visual) return null;
    const anim = this.byName.get(this.visual.anim);
    if (!anim) return null;
    const tick = Math.floor(((nowMs - this.startMs) * 60) / 1000);
    const f = frameAt(anim.durations, tick, this.visual.looped);
    const col = f?.col ?? anim.durations.length - 1;
    const nextMs = f?.nextTick == null ? null : this.startMs + (f.nextTick * 1000) / 60;
    return { anim, dir: this.visual.dir, col, nextMs };
  }
}
