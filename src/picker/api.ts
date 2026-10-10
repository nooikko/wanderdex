import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ActivePet, Index, VariantPayload } from "../shared/types";

export const getIndex = () => invoke<Index>("get_index");
export const portraitSrc = async (dex: string) => convertFileSrc(await invoke<string>("ensure_portrait_cmd", { dex }));
export const loadVariant = (path: string) => invoke<VariantPayload>("load_variant", { path });
export const spawn = (variant: string) => invoke<void>("spawn", { variant });
export const listPets = () => invoke<ActivePet[]>("list_pets");
export const removePet = (label: string) => invoke<void>("remove_pet_cmd", { label });
export const removeAll = () => invoke<void>("remove_all_cmd");
export const onPetsChanged = (cb: () => void) => listen("pets:changed", cb);
