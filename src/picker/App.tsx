import { useEffect, useMemo, useState } from "react";
import { getIndex } from "./api";
import { DesktopStrip } from "./DesktopStrip";
import { DetailPanel } from "./DetailPanel";
import { SpeciesGrid } from "./SpeciesGrid";
import type { Index } from "../shared/types";

export function App() {
  const [index, setIndex] = useState<Index | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => {
    getIndex().then(setIndex);
  }, []);
  const shown = useMemo(() => {
    if (!index) return [];
    const q = query.trim().toLowerCase();
    if (!q) return index.species;
    const n = Number(q);
    return index.species.filter((s) => s.name.toLowerCase().includes(q) || (!Number.isNaN(n) && Number(s.dex) === n));
  }, [index, query]);
  const species = index?.species.find((s) => s.dex === selected) ?? null;
  return (
    <div className="app">
      <header>
        <h1>Wanderdex</h1>
        <input
          autoFocus
          placeholder="Search by name or number"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          autoComplete="off"
        />
      </header>
      <main>
        {index ? <SpeciesGrid species={shown} selected={selected} onSelect={setSelected} /> : <p className="loading">Loading…</p>}
        {species && <DetailPanel key={species.dex} species={species} />}
      </main>
      <DesktopStrip />
    </div>
  );
}
