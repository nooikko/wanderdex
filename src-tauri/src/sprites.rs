//! Downloads a variant's AnimData.xml and sheets from SpriteCollab (pinned SHA), caches and validates them.

use crate::animdata::{self, AnimTable};
use crate::bounds::{decode_alpha, AlphaSheet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub trait Fetch {
    fn get(&self, url: &str) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + Send;
}

pub struct HttpFetch(reqwest::Client);

impl HttpFetch {
    pub fn new() -> Self {
        HttpFetch(reqwest::Client::builder().user_agent("Wanderdex").build().expect("HTTP client"))
    }
}

impl Default for HttpFetch {
    fn default() -> Self { Self::new() }
}

impl Fetch for HttpFetch {
    async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = self.0.get(url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {url}", resp.status()));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SpriteError {
    #[error("Couldn't download sprites: {0}")]
    Download(String),
    #[error("Downloaded sprites are invalid: {0}")]
    Invalid(String),
    #[error("Couldn't write the sprite cache: {0}")]
    Io(String),
}

impl serde::Serialize for SpriteError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug)]
pub struct LoadedVariant {
    pub path: String,
    pub dir: PathBuf,
    pub table: AnimTable,
    pub sheets: HashMap<String, AlphaSheet>,
}

pub fn raw_url(sha: &str, rel: &str) -> String {
    format!("https://raw.githubusercontent.com/PMDCollab/SpriteCollab/{sha}/{rel}")
}

fn load_cached(dir: &Path, path: &str) -> Result<LoadedVariant, SpriteError> {
    let xml = std::fs::read_to_string(dir.join("AnimData.xml")).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    let mut table = animdata::parse(&xml).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    let mut sheets = HashMap::new();
    for source in table.sources() {
        let bytes = std::fs::read(dir.join(format!("{source}-Anim.png")))
            .map_err(|e| SpriteError::Invalid(format!("{source}: {e}")))?;
        let sheet = decode_alpha(&bytes).map_err(|e| SpriteError::Invalid(format!("{source}: {e}")))?;
        sheets.insert(source, sheet);
    }
    table
        .validate(|s| sheets.get(s).map(|a| (a.width, a.height)))
        .map_err(|e| SpriteError::Invalid(e.to_string()))?;
    Ok(LoadedVariant { path: path.to_string(), dir: dir.to_path_buf(), table, sheets })
}

async fn download<F: Fetch>(fetch: &F, dir: &Path, sha: &str, path: &str) -> Result<(), SpriteError> {
    std::fs::create_dir_all(dir).map_err(|e| SpriteError::Io(e.to_string()))?;
    let xml = fetch.get(&raw_url(sha, &format!("sprite/{path}/AnimData.xml"))).await.map_err(SpriteError::Download)?;
    let text = String::from_utf8(xml).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    let table = animdata::parse(&text).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    for source in table.sources() {
        let png = fetch
            .get(&raw_url(sha, &format!("sprite/{path}/{source}-Anim.png")))
            .await
            .map_err(SpriteError::Download)?;
        std::fs::write(dir.join(format!("{source}-Anim.png")), png).map_err(|e| SpriteError::Io(e.to_string()))?;
    }
    std::fs::write(dir.join("AnimData.xml"), text).map_err(|e| SpriteError::Io(e.to_string()))
}

/// Returns a validated variant, using the cache when valid; otherwise downloads (retrying once).
pub async fn ensure_variant<F: Fetch>(fetch: &F, cache_root: &Path, sha: &str, path: &str) -> Result<LoadedVariant, SpriteError> {
    let dir = cache_root.join("sprites").join(sha).join(path);
    if let Ok(v) = load_cached(&dir, path) {
        return Ok(v);
    }
    let mut last = SpriteError::Download("not attempted".into());
    for _ in 0..2 {
        let _ = std::fs::remove_dir_all(&dir);
        match download(fetch, &dir, sha, path).await {
            Ok(()) => match load_cached(&dir, path) {
                Ok(v) => return Ok(v),
                Err(e) => last = e,
            },
            Err(e) => last = e,
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Err(last)
}

/// Returns the cached path of a species' Normal portrait, downloading it if needed.
pub async fn ensure_portrait<F: Fetch>(fetch: &F, cache_root: &Path, sha: &str, dex: &str) -> Result<PathBuf, SpriteError> {
    let file = cache_root.join("portraits").join(sha).join(format!("{dex}.png"));
    if std::fs::read(&file).ok().is_some_and(|b| decode_alpha(&b).is_ok()) {
        return Ok(file);
    }
    let bytes = fetch.get(&raw_url(sha, &format!("portrait/{dex}/Normal.png"))).await.map_err(SpriteError::Download)?;
    decode_alpha(&bytes).map_err(SpriteError::Invalid)?;
    std::fs::create_dir_all(file.parent().expect("has parent")).map_err(|e| SpriteError::Io(e.to_string()))?;
    std::fs::write(&file, bytes).map_err(|e| SpriteError::Io(e.to_string()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounds::tests::png_bytes;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const XML: &str = r#"<AnimData><Anims>
        <Anim><Name>Walk</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>4</Duration><Duration>4</Duration></Durations></Anim>
        <Anim><Name>Idle</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>30</Duration></Durations></Anim>
        <Anim><Name>Strike</Name><CopyOf>Idle</CopyOf></Anim>
        </Anims></AnimData>"#;

    struct FakeFetch {
        files: Mutex<HashMap<String, Vec<u8>>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeFetch {
        fn good() -> Self {
            let mut files = HashMap::new();
            let base = "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/sprite/0202";
            files.insert(format!("{base}/AnimData.xml"), XML.as_bytes().to_vec());
            files.insert(format!("{base}/Walk-Anim.png"), png_bytes(16, 64, |_, _| true));
            files.insert(format!("{base}/Idle-Anim.png"), png_bytes(8, 8, |_, _| true));
            files.insert(
                "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/portrait/0202/Normal.png".into(),
                png_bytes(40, 40, |_, _| true),
            );
            FakeFetch { files: Mutex::new(files), calls: Mutex::new(Vec::new()) }
        }
        fn calls(&self) -> Vec<String> { self.calls.lock().unwrap().clone() }
    }

    impl Fetch for FakeFetch {
        async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.calls.lock().unwrap().push(url.to_string());
            self.files.lock().unwrap().get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
    }

    fn run<T>(f: impl std::future::Future<Output = T>) -> T { tauri::async_runtime::block_on(f) }

    #[test]
    fn downloads_each_source_sheet_once_and_validates() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let v = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(v.table.get("Walk").unwrap().rows, 8);
        assert_eq!(v.table.get("Strike").unwrap().source, "Idle");
        assert!(v.dir.join("Walk-Anim.png").exists());
        assert_eq!(fetch.calls().len(), 3, "AnimData + Walk + Idle (Strike reuses Idle): {:?}", fetch.calls());
    }

    #[test]
    fn second_load_uses_cache() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        let before = fetch.calls().len();
        run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(fetch.calls().len(), before);
    }

    #[test]
    fn corrupt_or_missing_cached_sheet_is_redownloaded() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let v = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        std::fs::write(v.dir.join("Walk-Anim.png"), b"garbage").unwrap();
        run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        std::fs::remove_file(v.dir.join("Idle-Anim.png")).unwrap();
        let v = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert!(v.dir.join("Idle-Anim.png").exists());
    }

    #[test]
    fn network_failure_retries_once_then_errors() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch { files: Mutex::new(HashMap::new()), calls: Mutex::new(Vec::new()) };
        let err = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap_err();
        assert!(matches!(err, SpriteError::Download(_)), "{err}");
        assert_eq!(fetch.calls().len(), 2);
    }

    #[test]
    fn wrong_sheet_size_is_invalid_and_cache_is_cleared() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        fetch.files.lock().unwrap().insert(
            "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/sprite/0202/Walk-Anim.png".into(),
            png_bytes(15, 64, |_, _| true),
        );
        let err = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap_err();
        assert!(matches!(err, SpriteError::Invalid(_)), "{err}");
        assert!(!cache.path().join("sprites/sha1/0202").exists());
    }

    #[test]
    fn portraits_download_once() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let p = run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert!(p.ends_with("portraits/sha1/0202.png") && p.exists());
        run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(fetch.calls().len(), 1);
    }
}
