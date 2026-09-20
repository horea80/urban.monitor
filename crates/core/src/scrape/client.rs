//! Client HTTP politicos (NFR-1): o cerere la un moment dat, pauză minimă între cereri,
//! reîncercare cu backoff la erori tranzitorii, snapshot brut pe disc (FR-1.10).

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::config::Config;
use crate::error::{Error, Result};

const MAX_ATTEMPTS: u32 = 3;

pub struct Client {
    http: reqwest::Client,
    delay: Duration,
    last_request: Mutex<Option<Instant>>,
    raw_dir: Option<PathBuf>,
}

impl Client {
    pub fn new(cfg: &Config) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(cfg.user_agent.clone())
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| Error::Http {
                url: String::new(),
                source: e,
            })?;
        Ok(Client {
            http,
            delay: cfg.request_delay,
            last_request: Mutex::new(None),
            raw_dir: Some(cfg.raw_dir()),
        })
    }

    /// Dezactivează sau schimbă directorul de snapshot-uri.
    pub fn with_raw_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.raw_dir = dir;
        self
    }

    /// Pagină HTML, ca text. Snapshot `.html`.
    pub async fn get_html(&self, url: &str) -> Result<String> {
        let bytes = self.get_bytes(url).await?;
        self.snapshot(url, "html", &bytes);
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Fișier PDF, ca bytes. Snapshot `.pdf`.
    pub async fn get_pdf(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self.get_bytes(url).await?;
        self.snapshot(url, "pdf", &bytes);
        Ok(bytes)
    }

    async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.throttle().await;
            debug!(url, attempt, "GET");
            let result = self.http.get(url).send().await;
            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp.bytes().await.map(|b| b.to_vec()).map_err(|e| Error::Http {
                            url: url.to_owned(),
                            source: e,
                        });
                    }
                    let transient = status.is_server_error() || status.as_u16() == 429;
                    if transient && attempt < MAX_ATTEMPTS {
                        warn!(url, %status, attempt, "răspuns tranzitoriu, reîncerc");
                        self.backoff(attempt).await;
                        continue;
                    }
                    return Err(Error::Status {
                        url: url.to_owned(),
                        status: status.as_u16(),
                    });
                }
                Err(e) => {
                    let transient = e.is_timeout() || e.is_connect() || e.is_request();
                    if transient && attempt < MAX_ATTEMPTS {
                        warn!(url, error = %e, attempt, "eroare tranzitorie, reîncerc");
                        self.backoff(attempt).await;
                        continue;
                    }
                    return Err(Error::Http {
                        url: url.to_owned(),
                        source: e,
                    });
                }
            }
        }
    }

    /// Serializează cererile și impune pauza minimă între ele.
    async fn throttle(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(t) = *last {
            let elapsed = t.elapsed();
            if elapsed < self.delay {
                tokio::time::sleep(self.delay - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    async fn backoff(&self, attempt: u32) {
        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
    }

    fn snapshot(&self, url: &str, ext: &str, bytes: &[u8]) {
        let Some(dir) = &self.raw_dir else { return };
        if let Err(e) = write_snapshot(dir, url, ext, bytes) {
            warn!(url, error = %e, "nu am putut scrie snapshot-ul");
        }
    }
}

fn write_snapshot(dir: &Path, url: &str, ext: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(snapshot_name(url, ext)), bytes)
}

/// Nume de fișier lizibil derivat din URL, cu un hash scurt când e nevoie de trunchiere.
pub fn snapshot_name(url: &str, ext: &str) -> String {
    let stripped = url.trim_start_matches("https://").trim_start_matches("http://");
    let sanitized: String = stripped
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = sanitized.trim_matches('_');
    if name.len() > 150 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        return format!("{}_{:016x}.{ext}", &name[..150], h.finish());
    }
    format!("{name}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_names_are_readable_and_bounded() {
        let n = snapshot_name(
            "https://primariaclujnapoca.ro/urbanism/sedinte-comisie/sedinta-din-16-septembrie-2026/",
            "html",
        );
        assert_eq!(
            n,
            "primariaclujnapoca.ro_urbanism_sedinte-comisie_sedinta-din-16-septembrie-2026.html"
        );
        let long = format!("https://x.ro/{}", "a".repeat(400));
        let n = snapshot_name(&long, "pdf");
        assert!(n.len() < 180);
        assert!(n.ends_with(".pdf"));
    }
}
