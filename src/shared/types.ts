// Mirrors of Rust payloads (serde camelCase).
export interface AnimInfo {
  name: string;
  source: string;
  frameWidth: number;
  frameHeight: number;
  durations: number[]; // ticks at 60/s
  rows: number;
}
export interface ContentBox { minX: number; minY: number; maxX: number; maxY: number }
export interface Visual { anim: string; dir: number; looped: boolean; seq: number }
export interface PetInit { dir: string; table: AnimInfo[]; contentBox: ContentBox; scale: number; visual: Visual }
export interface VariantPayload { dir: string; table: AnimInfo[]; contentBox: ContentBox; signature: string | null }
export interface Variant { path: string; formId: string; form: string; shiny: boolean; female: boolean; credits: string[] }
export interface Species { dex: string; name: string; hasPortrait: boolean; variants: Variant[] }
export interface Index { spriteCollabSha: string; species: Species[] }
export interface ActivePet { label: string; variant: string; dex: string; name: string; form: string; shiny: boolean; female: boolean }
export interface CreditEntry { name: string; form: string; shiny: boolean; female: boolean; credits: string[] }

export const DIR = { Down: 0, DownRight: 1, Right: 2, UpRight: 3, Up: 4, UpLeft: 5, Left: 6, DownLeft: 7 } as const;
