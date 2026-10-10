import { useEffect, useState } from "react";
import { listPets, onPetsChanged, portraitSrc, removeAll, removePet } from "./api";
import type { ActivePet } from "../shared/types";

function PetChip({ pet }: { pet: ActivePet }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    portraitSrc(pet.dex).then(setSrc).catch(() => setSrc(null));
  }, [pet.dex]);
  const label = `${pet.shiny ? "Shiny " : ""}${pet.name}${pet.form !== "Normal" ? ` (${pet.form})` : ""}`;
  return (
    <button className="chip" title={`Remove ${label}`} onClick={() => removePet(pet.label)}>
      {src ? <img src={src} alt="" width={40} height={40} /> : <span>#{pet.dex}</span>}
    </button>
  );
}

export function DesktopStrip() {
  const [pets, setPets] = useState<ActivePet[]>([]);
  useEffect(() => {
    const refresh = () => listPets().then(setPets);
    refresh();
    const un = onPetsChanged(refresh);
    return () => {
      un.then((f) => f());
    };
  }, []);
  return (
    <footer className="strip">
      <span>On your desktop:</span>
      {pets.length === 0 && <em>nobody yet</em>}
      {pets.map((p) => <PetChip key={p.label} pet={p} />)}
      {pets.length > 0 && <button className="removeall" onClick={removeAll}>Remove all</button>}
    </footer>
  );
}
