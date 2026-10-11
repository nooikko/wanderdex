//! Settings and active pets, saved as JSON in the app config directory.

use crate::index::{find_variant, Index};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SCALES: [u32; 3] = [2, 3, 4];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub scale: u32,
    pub hidden: bool,
    pub pets: Vec<SavedPet>,
    /// Variant paths ever spawned, for the credits window.
    pub credited: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { scale: 3, hidden: false, pets: Vec::new(), credited: Vec::new() }
    }
}

/// A pet that was out. Only which one, not where: it reappears at a random spot. (Older files
/// also have `x`/`y`; serde ignores them.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPet {
    pub variant: String,
}

pub fn load(path: &Path) -> Settings {
    let mut s: Settings = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if !SCALES.contains(&s.scale) {
        s.scale = 3;
    }
    s
}

pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s).expect("settings serialize"))?;
    std::fs::rename(tmp, path)
}

/// Saved pets whose variant still exists in the index.
pub fn restorable<'a>(s: &'a Settings, index: &Index) -> Vec<&'a SavedPet> {
    s.pets.iter().filter(|p| find_variant(index, &p.variant).is_some()).collect()
}

/// The pets to save: live pets plus saved pets not (yet) restored, so an offline launch or a quit
/// mid-restore never erases them.
pub fn pets_to_save(mut live: Vec<SavedPet>, pending: &[SavedPet]) -> Vec<SavedPet> {
    live.extend_from_slice(pending);
    live
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrestored_pets_survive_saves() {
        let live = vec![SavedPet { variant: "0202".into() }];
        let pending = vec![SavedPet { variant: "0025".into() }];
        let saved = pets_to_save(live, &pending);
        let variants: Vec<&str> = saved.iter().map(|p| p.variant.as_str()).collect();
        assert_eq!(variants, ["0202", "0025"]);
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let s = Settings {
            scale: 4,
            hidden: true,
            pets: vec![SavedPet { variant: "0202".into() }],
            credited: vec!["0202".into()],
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
    }

    #[test]
    fn missing_or_corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path), Settings::default());
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        assert_eq!(Settings::default().scale, 3);
    }

    #[test]
    fn older_files_with_pet_positions_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"scale": 4, "pets": [{"variant": "0202", "x": 1555.2, "y": 2068.0}]}"#).unwrap();
        assert_eq!(load(&path).pets, [SavedPet { variant: "0202".into() }]);
    }

    #[test]
    fn invalid_scale_is_reset_and_unknown_fields_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"scale": 7, "future": 1}"#).unwrap();
        assert_eq!(load(&path).scale, 3);
    }

    #[test]
    fn restorable_skips_variants_missing_from_the_index() {
        let index = crate::index::Index {
            sprite_collab_sha: "x".into(),
            species: vec![crate::index::Species {
                dex: "0202".into(),
                name: "Wobbuffet".into(),
                has_portrait: true,
                variants: vec![crate::index::Variant {
                    path: "0202".into(),
                    form_id: "0000".into(),
                    form: "Normal".into(),
                    shiny: false,
                    female: false,
                    credits: vec![],
                }],
            }],
        };
        let s = Settings {
            pets: vec![
                SavedPet { variant: "0202".into() },
                SavedPet { variant: "9999/0001".into() },
            ],
            ..Settings::default()
        };
        let r = restorable(&s, &index);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].variant, "0202");
    }
}
