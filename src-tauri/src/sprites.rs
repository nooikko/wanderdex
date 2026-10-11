//! Downloads a variant's AnimData.xml and sheets from SpriteCollab (pinned SHA), caches and validates them.

use crate::animdata::{self, AnimTable};
use crate::bounds::{decode_alpha, AlphaSheet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::async_runtime::Mutex as AsyncMutex;

/// One async lock per cache path, so concurrent loads of the same variant (picker preview + spawn,
/// quick Shiny toggles) don't delete each other's half-written files.
static PATH_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> = LazyLock::new(Default::default);

fn path_lock(path: &Path) -> Arc<AsyncMutex<()>> {
    PATH_LOCKS.lock().unwrap().entry(path.to_path_buf()).or_default().clone()
}

pub trait Fetch {
    fn get(&self, url: &str) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + Send;
}

pub struct HttpFetch(reqwest::Client);

impl HttpFetch {
    pub fn new() -> Self {
        Self::with_timeouts(std::time::Duration::from_secs(10), std::time::Duration::from_secs(30))
    }
}

impl HttpFetch {
    /// A stalled connection must fail (and show Retry) rather than hang the picker or restore forever.
    pub fn with_timeouts(connect: std::time::Duration, total: std::time::Duration) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Wanderdex")
            .connect_timeout(connect)
            .timeout(total)
            .build()
            .expect("HTTP client");
        HttpFetch(client)
    }
}

impl Default for HttpFetch {
    fn default() -> Self { Self::new() }
}

impl Fetch for HttpFetch {
    async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut resp = self.0.get(url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {url}", resp.status()));
        }
        // Sprite sheets are a few hundred KB; refuse anything absurd instead of buffering it.
        const MAX_BYTES: usize = 16 << 20;
        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
            if body.len() + chunk.len() > MAX_BYTES {
                return Err(format!("{url} is larger than {} MB", MAX_BYTES >> 20));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
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
    let lock = path_lock(&dir);
    let _guard = lock.lock().await;
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
    let lock = path_lock(&file);
    let _guard = lock.lock().await;
    // Validated before it was written, and written atomically: if it exists, it's good.
    if file.is_file() {
        return Ok(file);
    }
    let bytes = fetch.get(&raw_url(sha, &format!("portrait/{dex}/Normal.png"))).await.map_err(SpriteError::Download)?;
    decode_alpha(&bytes).map_err(SpriteError::Invalid)?;
    std::fs::create_dir_all(file.parent().expect("has parent")).map_err(|e| SpriteError::Io(e.to_string()))?;
    let tmp = file.with_extension("png.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| SpriteError::Io(e.to_string()))?;
    std::fs::rename(&tmp, &file).map_err(|e| SpriteError::Io(e.to_string()))?;
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

    /// Returns Pending once, so concurrent downloads interleave at every fetch.
    struct YieldOnce(bool);
    impl std::future::Future for YieldOnce {
        type Output = ();
        fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
            if self.0 {
                std::task::Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }

    struct SlowFetch(FakeFetch);
    impl Fetch for SlowFetch {
        async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            for _ in 0..3 {
                YieldOnce(false).await;
            }
            self.0.get(url).await
        }
    }

    #[test]
    fn concurrent_loads_of_one_variant_both_succeed() {
        let fetch = std::sync::Arc::new(SlowFetch(FakeFetch::good()));
        for round in 0..30 {
            let cache = tempfile::tempdir().unwrap();
            let root = std::sync::Arc::new(cache.path().to_path_buf());
            let tasks: Vec<_> = (0..2)
                .map(|_| {
                    let (fetch, root) = (fetch.clone(), root.clone());
                    tauri::async_runtime::spawn(async move { ensure_variant(&*fetch, &root, "sha1", "0202").await.map(|_| ()) })
                })
                .collect();
            let before = fetch.0.calls().len();
            for t in tasks {
                let r = run(t).unwrap();
                assert!(r.is_ok(), "round {round}: {r:?}");
            }
            // The second load waits for the first and reuses its files: one download, not two racing ones.
            assert_eq!(fetch.0.calls().len() - before, 3, "round {round}: loads raced (each deletes the other's files)");
        }
    }

    #[test]
    fn stalled_server_times_out_instead_of_hanging() {
        // Accepts the connection but never answers (captive portal, dead Wi-Fi mid-transfer).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _hold = std::thread::spawn(move || {
            let conns: Vec<_> = listener.incoming().take(1).collect();
            std::thread::sleep(std::time::Duration::from_secs(10));
            drop(conns);
        });
        let fetch = HttpFetch::with_timeouts(std::time::Duration::from_secs(1), std::time::Duration::from_secs(1));
        let started = std::time::Instant::now();
        let result = run(fetch.get(&format!("http://{addr}/AnimData.xml")));
        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "took {:?}", started.elapsed());
    }

    #[test]
    fn portraits_download_once() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let p = run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert!(p.ends_with("portraits/sha1/0202.png") && p.exists());
        run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(fetch.calls().len(), 1);
        assert!(!p.with_extension("png.tmp").exists(), "written via a temp file, then renamed");
    }

    #[test]
    fn invalid_portrait_is_never_cached() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        fetch.files.lock().unwrap().insert(
            "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/portrait/0202/Normal.png".into(),
            b"not a png".to_vec(),
        );
        assert!(matches!(run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")), Err(SpriteError::Invalid(_))));
        assert!(!cache.path().join("portraits/sha1/0202.png").exists());
    }
}
