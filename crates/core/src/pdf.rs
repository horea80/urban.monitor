//! Extragere de text din PDF, în spatele unui trait (ADR-0007).
//! Implicit `pdf-extract` (pur Rust); fallback la `pdftotext` dacă există în PATH.

use std::path::PathBuf;
use std::process::Command;

use tracing::{debug, warn};

use crate::error::{Error, Result};

pub trait PdfText: Send + Sync {
    fn name(&self) -> &'static str;
    fn extract(&self, bytes: &[u8]) -> Result<String>;
}

/// `pdf-extract`. Biblioteca poate intra în panică pe PDF-uri neobișnuite, așa că
/// panica e transformată în eroare.
pub struct PdfExtract;

impl PdfText for PdfExtract {
    fn name(&self) -> &'static str {
        "pdf-extract"
    }

    fn extract(&self, bytes: &[u8]) -> Result<String> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::extract_text_from_mem(bytes)
        }));
        match result {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(e)) => Err(Error::Pdf(format!("pdf-extract: {e}"))),
            Err(_) => Err(Error::Pdf("pdf-extract a intrat în panică".into())),
        }
    }
}

/// `pdftotext` din poppler, rulat ca proces extern pe un fișier temporar.
pub struct Pdftotext {
    bin: PathBuf,
}

impl Pdftotext {
    /// `Some` doar dacă binarul răspunde la `-v`.
    pub fn find() -> Option<Self> {
        let bin = PathBuf::from("pdftotext");
        Command::new(&bin).arg("-v").output().ok().map(|_| Pdftotext { bin })
    }
}

impl PdfText for Pdftotext {
    fn name(&self) -> &'static str {
        "pdftotext"
    }

    fn extract(&self, bytes: &[u8]) -> Result<String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = std::env::temp_dir().join(format!("urban-{}-{nanos}.pdf", std::process::id()));
        std::fs::write(&tmp, bytes)?;
        let output = Command::new(&self.bin).arg("-layout").arg(&tmp).arg("-").output();
        let _ = std::fs::remove_file(&tmp);
        let output = output?;
        if !output.status.success() {
            return Err(Error::Pdf(format!(
                "pdftotext a ieșit cu {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// Primul care dă text ne-gol câștigă.
pub struct Chain {
    primary: Box<dyn PdfText>,
    fallback: Option<Box<dyn PdfText>>,
}

impl Chain {
    pub fn new(primary: Box<dyn PdfText>, fallback: Option<Box<dyn PdfText>>) -> Self {
        Chain { primary, fallback }
    }

    /// `pdf-extract`, apoi `pdftotext` dacă e instalat.
    pub fn default_chain() -> Self {
        let fallback: Option<Box<dyn PdfText>> = Pdftotext::find().map(|p| Box::new(p) as Box<dyn PdfText>);
        if fallback.is_some() {
            debug!("pdftotext găsit, folosit ca fallback");
        }
        Chain::new(Box::new(PdfExtract), fallback)
    }
}

fn looks_empty(text: &str) -> bool {
    text.trim().len() < 20
}

impl PdfText for Chain {
    fn name(&self) -> &'static str {
        "chain"
    }

    fn extract(&self, bytes: &[u8]) -> Result<String> {
        let first = self.primary.extract(bytes);
        match (&first, &self.fallback) {
            (Ok(text), _) if !looks_empty(text) => first,
            (_, Some(fb)) => {
                if let Err(e) = &first {
                    warn!(error = %e, "{} a eșuat, încerc {}", self.primary.name(), fb.name());
                }
                match fb.extract(bytes) {
                    Ok(text) if !looks_empty(&text) => Ok(text),
                    _ => first,
                }
            }
            (_, None) => first,
        }
    }
}
