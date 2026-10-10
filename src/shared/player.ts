import type { AnimInfo, Visual } from "./types";

/** Frame column at a tick (60/s); null once a non-looping animation has finished. */
export function columnAt(durations: number[], tick: number, looped: boolean): number | null {
  const total = durations.reduce((a, b) => a + b, 0);
  if (!looped && tick >= total) return null;
  let t = looped ? tick % total : tick;
  for (let i = 0; i < durations.length; i++) {
    if (t < durations[i]) return i;
    t -= durations[i];
  }
  return durations.length - 1;
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

  frame(nowMs: number): { anim: AnimInfo; dir: number; col: number } | null {
    if (!this.visual) return null;
    const anim = this.byName.get(this.visual.anim);
    if (!anim) return null;
    const tick = Math.floor(((nowMs - this.startMs) * 60) / 1000);
    const col = columnAt(anim.durations, tick, this.visual.looped) ?? anim.durations.length - 1;
    return { anim, dir: this.visual.dir, col };
  }
}
