import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import type { CreditEntry } from "../shared/types";
import "../picker/picker.css";

/** External link that opens in the default browser (webviews ignore target=_blank). */
function Ext({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        void openUrl(href);
      }}
    >
      {children}
    </a>
  );
}

function Credits() {
  const [entries, setEntries] = useState<CreditEntry[]>([]);
  useEffect(() => {
    invoke<CreditEntry[]>("get_credits").then(setEntries);
  }, []);
  return (
    <div style={{ padding: 24, overflowY: "auto", height: "100vh" }}>
      <h1>Credits &amp; license</h1>
      <p>
        Pokémon sprites come from the <Ext href="https://github.com/PMDCollab/SpriteCollab">PMD Sprite Repository (SpriteCollab)</Ext> and
        are licensed under <Ext href="https://creativecommons.org/licenses/by-nc/4.0/">CC BY-NC 4.0</Ext>. They may not be used commercially.
      </p>
      <p>Wanderdex is a non-commercial fan project. Pokémon is © Nintendo / Creatures Inc. / GAME FREAK inc. Wanderdex is not affiliated with or endorsed by them.</p>
      <h2>Artists for Pokémon you've spawned</h2>
      {entries.length === 0 && <p className="credits">Spawn a Pokémon to see its artists here.</p>}
      <ul>
        {entries.map((e, i) => (
          <li key={i}>
            <strong>{e.shiny ? "Shiny " : ""}{e.name}</strong>
            {e.form !== "Normal" ? ` (${e.form})` : ""}
            {e.female ? " ♀" : ""}: {e.credits.length ? e.credits.join(", ") : "Uncredited"}
          </li>
        ))}
      </ul>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Credits />);
