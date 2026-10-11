import { describe, expect, test } from "vitest";
import { columnAt, Player } from "./player";
import type { AnimInfo } from "./types";

const idle: AnimInfo = { name: "Idle", source: "Idle", frameWidth: 24, frameHeight: 40, durations: [40, 4, 4, 4, 4, 4, 4, 4, 4], rows: 8 };
const hop: AnimInfo = { name: "Hop", source: "Hop", frameWidth: 32, frameHeight: 80, durations: [2, 1, 2], rows: 8 };

describe("columnAt", () => {
  test("walks durations and loops", () => {
    expect(columnAt(idle.durations, 0, true)).toBe(0);
    expect(columnAt(idle.durations, 39, true)).toBe(0);
    expect(columnAt(idle.durations, 40, true)).toBe(1);
    expect(columnAt(idle.durations, 72, true)).toBe(0); // wrapped
  });
  test("one-shot returns null when finished", () => {
    expect(columnAt(hop.durations, 4, false)).toBe(2);
    expect(columnAt(hop.durations, 5, false)).toBeNull();
  });
});

describe("Player", () => {
  test("restarts timing on new anim or seq, keeps timing on direction change", () => {
    const p = new Player([idle, hop]);
    p.play({ anim: "Idle", dir: 0, looped: true, seq: 1 }, 0);
    expect(p.frame(700)!.col).toBe(1); // 700ms = 42 ticks
    p.play({ anim: "Idle", dir: 2, looped: true, seq: 1 }, 700);
    expect(p.frame(700)).toMatchObject({ dir: 2, col: 1 });
    p.play({ anim: "Idle", dir: 2, looped: true, seq: 2 }, 700);
    expect(p.frame(700)!.col).toBe(0);
  });
  test("holds the last frame after a one-shot ends", () => {
    const p = new Player([idle, hop]);
    p.play({ anim: "Hop", dir: 0, looped: false, seq: 5 }, 0);
    expect(p.frame(10_000)!.col).toBe(2);
  });
  test("reports when the next frame is due, and nothing after a one-shot ends", () => {
    const p = new Player([idle, hop]);
    p.play({ anim: "Idle", dir: 0, looped: true, seq: 1 }, 1000);
    expect(p.frame(1000)!.nextMs).toBeCloseTo(1000 + (40 * 1000) / 60); // first frame lasts 40 ticks
    expect(p.frame(1000 + (73 * 1000) / 60)!.nextMs).toBeCloseTo(1000 + (112 * 1000) / 60); // wrapped: 72 + 40
    p.play({ anim: "Hop", dir: 0, looped: false, seq: 2 }, 0);
    expect(p.frame(0)!.nextMs).toBeCloseTo((2 * 1000) / 60);
    expect(p.frame(10_000)!.nextMs).toBeNull();
  });
  test("unknown animation yields null", () => {
    const p = new Player([idle]);
    p.play({ anim: "Nope", dir: 0, looped: true, seq: 1 }, 0);
    expect(p.frame(0)).toBeNull();
  });
});
