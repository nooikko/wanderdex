import { expect, test } from "vitest";
import { DragTracker } from "./drag";

/** A fake invoke whose calls finish after the given delays, recording the order they *complete* in. */
function fakeSend(delays: Record<string, number>) {
  const done: string[] = [];
  const send = (cmd: string) =>
    new Promise<void>((resolve) =>
      setTimeout(() => {
        done.push(cmd);
        resolve();
      }, delays[cmd] ?? 0),
    );
  return { done, send };
}

test("a quick flick still delivers drag start before drop", async () => {
  const { done, send } = fakeSend({ pet_drag_start: 30, pet_drop: 0 });
  const t = new DragTracker(send);
  t.down(0, 0);
  t.move(10, 0); // > 3px: starts the drag (slow on the backend)
  await t.up(); // released in the same frame
  expect(done[0]).toBe("pet_drag_start");
  expect(done[done.length - 1]).toBe("pet_drop");
});

test("small movement is a click", async () => {
  const { done, send } = fakeSend({});
  const t = new DragTracker(send);
  t.down(0, 0);
  t.move(2, 1);
  await t.up();
  expect(done).toEqual(["pet_click"]);
});

test("moves are coalesced while one is in flight, and a cancelled pointer drops the pet", async () => {
  const { done, send } = fakeSend({ pet_drag_move: 20 });
  const t = new DragTracker(send);
  t.down(0, 0);
  for (let i = 1; i <= 10; i++) t.move(i * 10, 0);
  await t.cancel();
  await new Promise((r) => setTimeout(r, 300)); // let any stray in-flight calls finish
  expect(done[0]).toBe("pet_drag_start");
  expect(done[done.length - 1]).toBe("pet_drop");
  expect(done.filter((c) => c === "pet_drag_move").length).toBeLessThanOrEqual(2);
});
