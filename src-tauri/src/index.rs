//! The bundled SpriteCollab index: which species/variants exist, their animations and credits.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub sprite_collab_sha: String,
    pub species: Vec<Species>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Species {
    pub dex: String,
    pub name: String,
    pub has_portrait: bool,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    /// SpriteCollab path under `sprite/`, e.g. `0202/0000/0001`.
    pub path: String,
    pub form_id: String,
    pub form: String,
    pub shiny: bool,
    pub female: bool,
    /// Primary artist first, then secondary.
    pub credits: Vec<String>,
}

fn parse_credit_names(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let Some(name) = cols.first().map(|s| s.trim()).filter(|s| !s.is_empty()) else { continue };
        map.insert(name.to_string(), name.to_string());
        if let Some(discord) = cols.get(1).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            map.insert(discord.to_string(), name.to_string());
        }
    }
    map
}

fn resolve(token: &str, names: &HashMap<String, String>) -> String {
    names.get(token).cloned().unwrap_or_else(|| token.to_string())
}

fn collect<'a>(
    node: &'a Value,
    dex: &str,
    segs: &[String],
    form_name: Option<&'a str>,
    names: &HashMap<String, String>,
    out: &mut Vec<Variant>,
) {
    let form_name = if segs.len() == 1 { node["name"].as_str() } else { form_name };
    // tracker.json maps each existing file to its lock flag, so presence (not the value) means "exists".
    let files = node["sprite_files"].as_object();
    let has = |k: &str| files.is_some_and(|f| f.contains_key(k));
    if has("Walk") && has("Idle") {
        let form_id = segs.first().cloned().unwrap_or_else(|| "0000".to_string());
        let form = if form_id == "0000" {
            "Normal".to_string()
        } else {
            match form_name.filter(|n| !n.is_empty()) {
                Some(n) => n.replace('_', " "),
                None => format!("Form {form_id}"),
            }
        };
        let credit = &node["sprite_credit"];
        let mut credits = Vec::new();
        if let Some(p) = credit["primary"].as_str().filter(|p| !p.is_empty()) {
            credits.push(resolve(p, names));
        }
        for s in credit["secondary"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            credits.push(resolve(s, names));
        }
        let mut path = dex.to_string();
        for s in segs {
            path.push('/');
            path.push_str(s);
        }
        out.push(Variant {
            path,
            form_id,
            form,
            shiny: segs.get(1).is_some_and(|s| s == "0001"),
            female: segs.get(2).is_some_and(|s| s == "0002"),
            credits,
        });
    }
    if let Some(subs) = node["subgroups"].as_object() {
        let mut keys: Vec<&String> = subs.keys().collect();
        keys.sort();
        for k in keys {
            let mut next = segs.to_vec();
            next.push(k.clone());
            collect(&subs[k], dex, &next, form_name, names, out);
        }
    }
}

/// Builds the index from SpriteCollab's tracker.json and credit_names.txt.
pub fn build(tracker: &Value, credit_names: &str, sha: &str) -> Index {
    let names = parse_credit_names(credit_names);
    let mut species = Vec::new();
    if let Some(obj) = tracker.as_object() {
        let mut dexes: Vec<&String> = obj.keys().collect();
        dexes.sort();
        for dex in dexes.into_iter().filter(|d| d.as_str() != "0000") {
            let entry = &obj[dex];
            let mut variants = Vec::new();
            collect(entry, dex, &[], None, &names, &mut variants);
            if variants.is_empty() {
                continue;
            }
            species.push(Species {
                dex: dex.clone(),
                name: entry["name"].as_str().unwrap_or("").replace('_', " "),
                has_portrait: entry["portrait_files"].as_object().is_some_and(|p| p.contains_key("Normal")),
                variants,
            });
        }
    }
    Index { sprite_collab_sha: sha.to_string(), species }
}

/// The bundled index as JSON, sent to the picker as is (no clone, no re-serialize).
pub const BUNDLED_JSON: &str = include_str!("../resources/index.json");

static BUNDLED: LazyLock<Index> = LazyLock::new(|| serde_json::from_str(BUNDLED_JSON).expect("bundled index.json is valid"));

pub fn bundled() -> &'static Index {
    &BUNDLED
}

pub fn find_variant<'a>(index: &'a Index, path: &str) -> Option<(&'a Species, &'a Variant)> {
    let dex = path.split('/').next()?;
    let species = index.species.iter().find(|s| s.dex == dex)?;
    species.variants.iter().find(|v| v.path == path).map(|v| (species, v))
}

#[derive(Deserialize)]
struct Overrides {
    signatures: HashMap<String, String>,
}

static OVERRIDES: LazyLock<Overrides> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../resources/overrides.json")).expect("overrides.json is valid")
});

/// A species' signature move (an animation name), if one is configured.
pub fn signature_for(dex: &str) -> Option<&'static str> {
    OVERRIDES.signatures.get(dex).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CREDIT_NAMES: &str = "Name\tDiscord\tContact\nCHUNSOFT\tCHUNSOFT\thttps://www.spike-chunsoft.com/\nTacocoa\t<@!234720295292895232>\t\n";

    fn files(names: &[&str]) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for n in names { m.insert((*n).into(), json!(true)); }
        m.insert("Hurt".into(), json!(false)); // the value is SpriteCollab's lock flag; the key means the file exists
        serde_json::Value::Object(m)
    }

    fn tracker() -> serde_json::Value {
        json!({
            "0000": { "name": "Missingno_", "sprite_files": files(&["Walk", "Idle"]), "subgroups": {} },
            "0202": {
                "name": "Wobbuffet",
                "portrait_files": { "Normal": true },
                "sprite_files": files(&["Walk", "Idle", "Withdraw"]),
                "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] },
                "subgroups": {
                    "0000": { "name": "", "sprite_files": {}, "subgroups": {
                        "0000": { "name": "", "sprite_files": {}, "subgroups": {
                            "0002": { "name": "Female", "sprite_files": files(&["Walk", "Idle"]),
                                      "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] }, "subgroups": {} } } },
                        "0001": { "name": "Shiny", "sprite_files": files(&["Walk", "Idle"]),
                                  "sprite_credit": { "primary": "<@!234720295292895232>", "secondary": ["SomeoneNew"] }, "subgroups": {} } } }
                }
            },
            "0019": {
                "name": "Rattata",
                "portrait_files": { "Happy": true },
                "sprite_files": files(&["Walk", "Idle"]),
                "sprite_credit": { "primary": "", "secondary": [] },
                "subgroups": {
                    "0001": { "name": "Alola", "sprite_files": files(&["Walk", "Idle"]),
                              "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] }, "subgroups": {
                        "0001": { "name": "Shiny", "sprite_files": files(&["Walk", "Idle"]),
                                  "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] }, "subgroups": {} } } },
                    "0003": { "name": "Gigantamax", "sprite_files": files(&["Idle"]), "subgroups": {} }
                }
            },
            "0151": { "name": "Mew", "sprite_files": files(&["Idle"]), "subgroups": {} },
            "0359": {
                "name": "Absol",
                "portrait_files": { "Normal": false },
                "sprite_files": { "Walk": false, "Idle": false },
                "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] },
                "subgroups": {}
            }
        })
    }

    #[test]
    fn builds_variants_with_paths_flags_and_form_names() {
        let idx = build(&tracker(), CREDIT_NAMES, "abc123");
        assert_eq!(idx.sprite_collab_sha, "abc123");
        let dexes: Vec<&str> = idx.species.iter().map(|s| s.dex.as_str()).collect();
        assert_eq!(dexes, ["0019", "0202", "0359"], "sorted, skips 0000 and species without usable variants");

        let absol = &idx.species[2];
        assert!(absol.has_portrait, "unlocked (false) portraits still exist");
        assert_eq!(absol.variants.len(), 1, "unlocked (false) sprites still exist");

        let wob = &idx.species[1];
        assert_eq!(wob.name, "Wobbuffet");
        assert!(wob.has_portrait);
        let summary: Vec<(&str, &str, bool, bool)> =
            wob.variants.iter().map(|v| (v.path.as_str(), v.form.as_str(), v.shiny, v.female)).collect();
        assert_eq!(summary, [
            ("0202", "Normal", false, false),
            ("0202/0000/0000/0002", "Normal", false, true),
            ("0202/0000/0001", "Normal", true, false),
        ]);

        let rat = &idx.species[0];
        assert!(!rat.has_portrait);
        let paths: Vec<(&str, &str, &str, bool)> =
            rat.variants.iter().map(|v| (v.path.as_str(), v.form_id.as_str(), v.form.as_str(), v.shiny)).collect();
        assert_eq!(paths, [
            ("0019", "0000", "Normal", false),
            ("0019/0001", "0001", "Alola", false),
            ("0019/0001/0001", "0001", "Alola", true),
        ], "Gigantamax has no Walk and is excluded");
    }

    #[test]
    fn resolves_credit_names_and_keeps_unknown_names() {
        let idx = build(&tracker(), CREDIT_NAMES, "abc123");
        let wob = &idx.species[1];
        assert_eq!(wob.variants[0].credits, ["CHUNSOFT"]);
        assert_eq!(wob.variants[2].credits, ["Tacocoa", "SomeoneNew"]);
        assert!(idx.species[0].variants[0].credits.is_empty(), "empty primary is dropped");
    }

    #[test]
    fn finds_variants_by_path() {
        let idx = build(&tracker(), CREDIT_NAMES, "abc123");
        let (s, v) = find_variant(&idx, "0202/0000/0001").unwrap();
        assert_eq!((s.name.as_str(), v.shiny), ("Wobbuffet", true));
        assert!(find_variant(&idx, "9999").is_none());
    }

    #[test]
    fn signature_overrides_load() {
        assert_eq!(signature_for("0202"), Some("Withdraw"));
        assert_eq!(signature_for("0025"), None);
    }

    #[test]
    fn bundled_index_is_real_and_pinned() {
        let idx = bundled();
        assert_eq!(idx.sprite_collab_sha.len(), 40, "pinned to a full commit SHA");
        assert!(idx.species.len() > 900);
        assert!(find_variant(idx, "0202").is_some(), "Wobbuffet present");
    }
}
