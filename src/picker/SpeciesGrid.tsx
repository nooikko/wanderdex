import { useEffect, useRef, useState } from "react";
import { portraitSrc } from "./api";
import type { Species } from "../shared/types";

function Tile({ s, selected, onSelect }: { s: Species; selected: boolean; onSelect: () => void }) {
  const ref = useRef<HTMLButtonElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!s.hasPortrait || !ref.current) return;
    const io = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) {
        io.disconnect();
        portraitSrc(s.dex).then(setSrc).catch(() => setSrc(null));
      }
    });
    io.observe(ref.current);
    return () => io.disconnect();
  }, [s.dex, s.hasPortrait]);
  return (
    <button ref={ref} className={`tile${selected ? " selected" : ""}`} onClick={onSelect}>
      {src ? <img src={src} alt="" width={80} height={80} /> : <div className="noportrait">#{Number(s.dex)}</div>}
      <span className="name">{s.name}</span>
      <span className="dex">#{s.dex}</span>
    </button>
  );
}

export function SpeciesGrid({ species, selected, onSelect }: { species: Species[]; selected: string | null; onSelect: (dex: string) => void }) {
  return (
    <div className="grid">
      {species.map((s) => (
        <Tile key={s.dex} s={s} selected={s.dex === selected} onSelect={() => onSelect(s.dex)} />
      ))}
    </div>
  );
}
