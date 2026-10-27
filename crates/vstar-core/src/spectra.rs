//! One-dimensional spectra stored as FITS files, as the AAVSO spectroscopy
//! database (AVSpec) provides them.
//!
//! AVSpec files hold a single 1-D spectrum in the primary image, with the
//! wavelength scale given by the `CRVAL1`, `CDELT1` and `CRPIX1` keywords.

use crate::error::{Error, Result};

const BLOCK: usize = 2880;
const CARD: usize = 80;

/// The address AVSpec serves a spectrum's FITS file from.
pub fn download_url(id: u64) -> String {
    format!("https://apps.aavso.org/avspec/download/{id}")
}

/// The address of a spectrum's page on the AVSpec website.
pub fn page_url(id: u64) -> String {
    format!("https://apps.aavso.org/avspec/obs/{id}")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    /// AVSpec's id for this observation.
    pub id: u64,
    pub object: Option<String>,
    pub observer: Option<String>,
    /// The `DATE-OBS` text from the header.
    pub date_obs: Option<String>,
    pub jd: Option<f64>,
    pub instrument: Option<String>,
    pub resolving_power: Option<f64>,
    /// Wavelengths in angstroms.
    pub wavelengths: Vec<f64>,
    pub flux: Vec<f64>,
}

/// A header keyword's value with any trailing `/ comment` removed.
fn card_value(card: &str) -> Option<String> {
    let rest = card.get(10..)?.trim_start();
    if let Some(quoted) = rest.strip_prefix('\'') {
        // A string: ends at the closing quote ('' is an escaped quote).
        let mut out = String::new();
        let mut chars = quoted.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    out.push('\'');
                    chars.next();
                } else {
                    break;
                }
            } else {
                out.push(c);
            }
        }
        return Some(out.trim_end().to_string());
    }
    let value = rest.split('/').next().unwrap_or("").trim();
    (!value.is_empty()).then(|| value.to_string())
}

struct Header {
    cards: Vec<(String, String)>,
}

impl Header {
    fn get(&self, key: &str) -> Option<&str> {
        self.cards
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn number(&self, key: &str) -> Option<f64> {
        self.get(key)?.replace('D', "E").parse().ok()
    }

    fn text(&self, key: &str) -> Option<String> {
        self.get(key)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
}

/// Read the header of the first HDU. Returns it and the offset where the
/// data begins.
fn read_header(bytes: &[u8]) -> Result<(Header, usize)> {
    let mut cards = Vec::new();
    let mut offset = 0;
    loop {
        let block = bytes
            .get(offset..offset + BLOCK)
            .ok_or_else(|| Error::Parse("the FITS file ends inside its header".into()))?;
        offset += BLOCK;
        for raw in block.chunks(CARD) {
            let card = String::from_utf8_lossy(raw);
            let key = card.get(..8).unwrap_or("").trim().to_string();
            if key == "END" {
                return Ok((Header { cards }, offset));
            }
            if card.get(8..10) == Some("= ")
                && let Some(value) = card_value(&card)
            {
                cards.push((key, value));
            }
        }
    }
}

/// Parse a 1-D spectrum from the bytes of a FITS file.
pub fn parse_fits(bytes: &[u8], id: u64) -> Result<Spectrum> {
    if !bytes.starts_with(b"SIMPLE") {
        return Err(Error::Parse(
            "this isn't a FITS file (AVSpec may have sent a web page instead)".into(),
        ));
    }
    let (header, data_start) = read_header(bytes)?;
    let bitpix = header.number("BITPIX").unwrap_or(0.0) as i32;
    let naxis = header.number("NAXIS").unwrap_or(0.0) as usize;
    let n = header.number("NAXIS1").unwrap_or(0.0) as usize;
    if naxis == 0 || n == 0 {
        return Err(Error::Parse(
            "the FITS file has no spectrum in its primary image".into(),
        ));
    }
    let bytes_per = (bitpix.unsigned_abs() / 8) as usize;
    let data = bytes
        .get(data_start..data_start + n * bytes_per)
        .ok_or_else(|| Error::Parse("the FITS file is shorter than its header says".into()))?;
    let (bzero, bscale) = (
        header.number("BZERO").unwrap_or(0.0),
        header.number("BSCALE").unwrap_or(1.0),
    );
    let raw: Vec<f64> = match bitpix {
        8 => data.iter().map(|&b| b as f64).collect(),
        16 => data
            .chunks_exact(2)
            .map(|c| i16::from_be_bytes([c[0], c[1]]) as f64)
            .collect(),
        32 => data
            .chunks_exact(4)
            .map(|c| i32::from_be_bytes([c[0], c[1], c[2], c[3]]) as f64)
            .collect(),
        -32 => data
            .chunks_exact(4)
            .map(|c| f32::from_be_bytes([c[0], c[1], c[2], c[3]]) as f64)
            .collect(),
        -64 => data
            .chunks_exact(8)
            .map(|c| f64::from_be_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
            .collect(),
        other => {
            return Err(Error::Parse(format!(
                "unsupported FITS data type (BITPIX {other})"
            )));
        }
    };
    let flux: Vec<f64> = raw.into_iter().map(|v| bzero + bscale * v).collect();

    let crval = header.number("CRVAL1").unwrap_or(1.0);
    let cdelt = header
        .number("CDELT1")
        .or_else(|| header.number("CD1_1"))
        .unwrap_or(1.0);
    let crpix = header.number("CRPIX1").unwrap_or(1.0);
    let wavelengths = (0..flux.len())
        .map(|i| crval + (i as f64 + 1.0 - crpix) * cdelt)
        .collect();

    Ok(Spectrum {
        id,
        object: header.text("OBJNAME").or_else(|| header.text("OBJECT")),
        observer: header.text("OBSERVER"),
        date_obs: header.text("DATE-OBS"),
        jd: header.number("JD"),
        instrument: header.text("AAV_INST"),
        resolving_power: header.number("AAV_ITRP"),
        wavelengths,
        flux,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str, value: &str) -> Vec<u8> {
        let mut text = format!("{key:<8}= {value}");
        text.truncate(CARD);
        let mut bytes = text.into_bytes();
        bytes.resize(CARD, b' ');
        bytes
    }

    fn fits(bitpix: i32, flux: &[f32], extra: &[(&str, &str)]) -> Vec<u8> {
        let mut header = Vec::new();
        let n = flux.len().to_string();
        let bp = bitpix.to_string();
        for (k, v) in [
            ("SIMPLE", "T"),
            ("BITPIX", bp.as_str()),
            ("NAXIS", "1"),
            ("NAXIS1", n.as_str()),
            ("CRVAL1", "4000.0 / start"),
            ("CDELT1", "2.5"),
            ("CRPIX1", "1"),
            ("OBJNAME", "'T CrB   '"),
            ("OBSERVER", "'BGAK'"),
            ("JD", "2460000.25"),
        ] {
            header.extend(card(k, v));
        }
        for (k, v) in extra {
            header.extend(card(k, v));
        }
        let mut end = b"END".to_vec();
        end.resize(CARD, b' ');
        header.extend(end);
        header.resize(header.len().div_ceil(BLOCK) * BLOCK, b' ');
        for v in flux {
            if bitpix == -32 {
                header.extend(v.to_be_bytes());
            } else {
                header.extend((*v as i16).to_be_bytes());
            }
        }
        header
    }

    #[test]
    fn reads_a_float_spectrum() {
        let bytes = fits(-32, &[1.0, 2.0, 3.5], &[]);
        let s = parse_fits(&bytes, 7).unwrap();
        assert_eq!(s.id, 7);
        assert_eq!(s.flux, vec![1.0, 2.0, 3.5]);
        assert_eq!(s.wavelengths, vec![4000.0, 4002.5, 4005.0]);
        assert_eq!(s.object.as_deref(), Some("T CrB"));
        assert_eq!(s.observer.as_deref(), Some("BGAK"));
        assert_eq!(s.jd, Some(2460000.25));
    }

    #[test]
    fn reads_scaled_integers() {
        let bytes = fits(16, &[10.0, 20.0], &[("BZERO", "100"), ("BSCALE", "2")]);
        let s = parse_fits(&bytes, 1).unwrap();
        assert_eq!(s.flux, vec![120.0, 140.0]);
    }

    #[test]
    fn rejects_web_pages_and_short_files() {
        assert!(parse_fits(b"<html>", 1).is_err());
        let mut bytes = fits(-32, &[1.0, 2.0], &[]);
        bytes.truncate(BLOCK + 4);
        assert!(parse_fits(&bytes, 1).is_err());
    }

    #[test]
    fn addresses() {
        assert_eq!(
            download_url(19915),
            "https://apps.aavso.org/avspec/download/19915"
        );
        assert_eq!(page_url(19915), "https://apps.aavso.org/avspec/obs/19915");
    }
}
