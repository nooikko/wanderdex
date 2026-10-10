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
  test("unknown animation yields null", () => {
    const p = new Player([idle]);
    p.play({ anim: "Nope", dir: 0, looped: true, seq: 1 }, 0);
    expect(p.frame(0)).toBeNull();
  });
});
