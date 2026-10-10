const DRAG_THRESHOLD_PX = 3;

/**
 * Turns pointer events into pet commands, delivered strictly in order (drag start → moves → drop).
 * The backend runs commands concurrently, so a fire-and-forget drop could overtake a slow drag start
 * and leave the pet stuck in the dragged state.
 */
export class DragTracker {
  private origin: { x: number; y: number } | null = null;
  private dragging = false;
  private chain: Promise<unknown> = Promise.resolve();
  private moveQueued = false;

  constructor(private send: (cmd: string) => Promise<unknown>) {}

  private enqueue(cmd: string): Promise<unknown> {
    this.chain = this.chain.then(() => this.send(cmd)).catch(() => undefined);
    return this.chain;
  }

  down(x: number, y: number): void {
    this.origin = { x, y };
    this.dragging = false;
  }

  move(x: number, y: number): void {
    if (!this.origin) return;
    if (!this.dragging && Math.hypot(x - this.origin.x, y - this.origin.y) > DRAG_THRESHOLD_PX) {
      this.dragging = true;
      void this.enqueue("pet_drag_start");
    }
    if (this.dragging && !this.moveQueued) {
      // One queued move at a time: the backend reads the live cursor, so extra moves add nothing.
      this.moveQueued = true;
      this.chain = this.chain
        .then(() => {
          this.moveQueued = false;
          return this.send("pet_drag_move");
        })
        .catch(() => undefined);
    }
  }

  async up(): Promise<void> {
    if (!this.origin) return;
    const cmd = this.dragging ? "pet_drop" : "pet_click";
    this.origin = null;
    this.dragging = false;
    await this.enqueue(cmd);
  }

  /** Pointer capture lost (window hidden, system gesture…): finish like a release. */
  async cancel(): Promise<void> {
    return this.up();
  }
}
