import { useEffect, useMemo, useState } from "react";
import { loadVariant, spawn } from "./api";
import { Preview } from "./Preview";
import type { Species, VariantPayload } from "../shared/types";

export function DetailPanel({ species }: { species: Species }) {
  const forms = useMemo(() => [...new Map(species.variants.map((v) => [v.formId, v.form])).entries()], [species]);
  const [formId, setFormId] = useState(forms[0][0]);
  const [shiny, setShiny] = useState(false);
  const [female, setFemale] = useState(false);
  const [payload, setPayload] = useState<VariantPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    setFormId(forms[0][0]);
    setShiny(false);
    setFemale(false);
  }, [species, forms]);

  const inForm = species.variants.filter((v) => v.formId === formId);
  const canShiny = inForm.some((v) => v.shiny);
  const canFemale = inForm.some((v) => v.female);
  const variant =
    inForm.find((v) => v.shiny === (shiny && canShiny) && v.female === (female && canFemale)) ??
    inForm.find((v) => v.shiny === (shiny && canShiny)) ??
    inForm[0];

  useEffect(() => {
    let alive = true;
    setPayload(null);
    setError(null);
    loadVariant(variant.path)
      .then((p) => alive && setPayload(p))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, [variant.path, attempt]);

  const doSpawn = async () => {
    setBusy(true);
    setError(null);
    try {
      await spawn(variant.path);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="detail">
      <h2>
        {species.name} <span className="dex">#{species.dex}</span>
      </h2>
      <div className="stage">{payload ? <Preview payload={payload} /> : !error && <div className="loading">Loading…</div>}</div>
      {forms.length > 1 && (
        <label>
          Form{" "}
          <select value={formId} onChange={(e) => setFormId(e.target.value)}>
            {forms.map(([id, name]) => (
              <option key={id} value={id}>{name}</option>
            ))}
          </select>
        </label>
      )}
      {canShiny && (
        <label><input type="checkbox" checked={shiny} onChange={(e) => setShiny(e.target.checked)} /> Shiny</label>
      )}
      {canFemale && (
        <label><input type="checkbox" checked={female} onChange={(e) => setFemale(e.target.checked)} /> Female</label>
      )}
      <p className="credits">Sprites: {variant.credits.length ? variant.credits.join(", ") : "Uncredited"}</p>
      {error && (
        <p className="error">
          Couldn't download sprites. Check your connection. <button onClick={() => setAttempt((a) => a + 1)}>Retry</button>
        </p>
      )}
      <button className="spawn" disabled={busy || !payload} onClick={doSpawn}>
        {busy ? "Spawning…" : "Spawn"}
      </button>
    </aside>
  );
}
