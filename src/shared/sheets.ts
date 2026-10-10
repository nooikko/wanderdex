import { convertFileSrc } from "@tauri-apps/api/core";
import type { AnimInfo } from "./types";

/** Loads every source sheet of a variant from the local cache, keyed by source name. */
export async function loadSheets(dir: string, table: AnimInfo[]): Promise<Map<string, HTMLImageElement>> {
  const sources = [...new Set(table.map((a) => a.source))];
  const entries = await Promise.all(
    sources.map(
      (source) =>
        new Promise<[string, HTMLImageElement]>((resolve, reject) => {
          const img = new Image();
          img.crossOrigin = "anonymous"; // keeps canvases readable (contact-sheet pixel checks)
          img.onload = () => resolve([source, img]);
          img.onerror = () => reject(new Error(`Couldn't load ${source}-Anim.png`));
          img.src = convertFileSrc(`${dir}/${source}-Anim.png`);
        }),
    ),
  );
  return new Map(entries);
}
