import { expect, test } from "vitest";
import { canvasMetrics, canvasSize, frameDest, frameSource } from "./renderer";
import type { AnimInfo, ContentBox } from "./types";

const idle: AnimInfo = { name: "Idle", source: "Idle", frameWidth: 24, frameHeight: 40, durations: [40], rows: 8 };
const sleep: AnimInfo = { name: "Sleep", source: "Sleep", frameWidth: 24, frameHeight: 40, durations: [30, 35], rows: 1 };
const box: ContentBox = { minX: -8, minY: -16, maxX: 8, maxY: 7 }; // Wobbuffet's measured box

test("frameSource picks column and direction row; single-row sheets ignore direction", () => {
  expect(frameSource(idle, 6, 3)).toEqual({ sx: 72, sy: 240, sw: 24, sh: 40 });
  expect(frameSource(sleep, 6, 1)).toEqual({ sx: 24, sy: 0, sw: 24, sh: 40 });
});

test("frameDest centers the frame on the shared anchor at integer scale", () => {
  // anchor at (8,16) sprite px from the window's top-left; frame top-left = anchor - (12,20)
  expect(frameDest(idle, box, 3)).toEqual({ dx: -12, dy: -12, dw: 72, dh: 120 });
});

test("canvasSize is the content box times scale", () => {
  expect(canvasSize(box, 3)).toEqual({ width: 48, height: 69 });
});

test("canvasMetrics snaps each sprite pixel to whole device pixels at fractional DPI", () => {
  // 125% Windows: 3x would be 3.75 device px per sprite px → snapped to 4.
  expect(canvasMetrics(box, 3, 1.25)).toEqual({ width: 64, height: 92, cssWidth: 51.2, cssHeight: 73.6, ratio: 4 / 3 });
  // Retina: 3x → exactly 6 device px, CSS size unchanged.
  expect(canvasMetrics(box, 3, 2)).toEqual({ width: 96, height: 138, cssWidth: 48, cssHeight: 69, ratio: 2 });
});
