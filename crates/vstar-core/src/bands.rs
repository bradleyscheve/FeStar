//! Photometric bands (VStar calls these "series").
//!
//! Short names, descriptions and colours follow VStar's `SeriesType`, so the
//! plots look familiar to existing VStar users.

/// Display information about a band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandInfo {
    /// The code databases use, e.g. "V" or "Vis.".
    pub short_name: &'static str,
    pub description: &'static str,
    /// Plot colour. `None` means "use the theme's text colour", which keeps
    /// visual estimates (black in VStar) readable on a dark background.
    pub rgb: Option<[u8; 3]>,
}

const fn band(short_name: &'static str, description: &'static str, rgb: [u8; 3]) -> BandInfo {
    BandInfo {
        short_name,
        description,
        rgb: Some(rgb),
    }
}

pub const VISUAL: &str = "Vis.";
pub const UNKNOWN: &str = "N/A";
pub const GAIA_G: &str = "GG";

pub static BANDS: &[BandInfo] = &[
    BandInfo {
        short_name: VISUAL,
        description: "Visual",
        rgb: None,
    },
    band(UNKNOWN, "Unknown", [255, 255, 0]),
    band("U", "Johnson U", [0, 255, 255]),
    band("B", "Johnson B", [0, 0, 255]),
    band("V", "Johnson V", [0, 255, 0]),
    band("RJ", "Johnson R", [192, 0, 64]),
    band("IJ", "Johnson I", [192, 64, 128]),
    band("HA", "Halpha", [192, 32, 0]),
    band("HAC", "Halpha-continuum", [160, 32, 32]),
    band("Blue-Vis.", "Blue", [0, 0, 75]),
    band("Green-Vis.", "Green", [0, 75, 0]),
    band("Red-Vis.", "Red", [75, 0, 0]),
    band("Yellow-Vis.", "Yellow", [255, 255, 128]),
    band("K", "K NIR 2.2micron", [255, 128, 255]),
    band("H", "H NIR 1.6micron", [128, 128, 128]),
    band("J", "J NIR 1.2micron", [255, 0, 255]),
    band("SZ", "Sloan z", [255, 192, 0]),
    band("STU", "Stromgren u", [0, 192, 255]),
    band("STV", "Stromgren v", [0, 255, 192]),
    band("STB", "Stromgren b", [0, 0, 192]),
    band("STY", "Stromgren y", [192, 255, 0]),
    band("STHBW", "Stromgren Hbw", [0, 128, 255]),
    band("STHBN", "Stromgren Hbn", [0, 128, 192]),
    band("R", "Cousins R", [255, 0, 0]),
    band("SU", "Sloan u", [192, 192, 0]),
    band("SG", "Sloan g", [0, 64, 64]),
    band("SR", "Sloan r", [128, 64, 0]),
    band("SI", "Sloan i", [192, 64, 0]),
    band("ZS", "PanSTARRS Z-short", [255, 64, 32]),
    band("Y", "PanSTARRS Y", [96, 0, 0]),
    band("I", "Cousins I", [255, 64, 0]),
    band("TB", "Tri-Color Blue", [0, 0, 128]),
    band("TG", "Tri-Color Green", [0, 128, 0]),
    band("TR", "Tri-Color Red", [128, 0, 0]),
    band("MA", "Optec Wing A", [128, 64, 255]),
    band("MB", "Optec Wing B", [128, 64, 128]),
    band("MI", "Optec Wing C", [128, 0, 192]),
    band("O", "Orange (Liller)", [255, 128, 0]),
    band("CBB", "Clear Blue Blocking", [255, 220, 32]),
    band("CV", "Unfiltered with V Zeropoint", [0, 192, 0]),
    band("CR", "Unfiltered with R Zeropoint", [192, 0, 0]),
    band(GAIA_G, "GAIA G", [255, 165, 0]),
];

/// Look up a band by its short name (case-sensitive, as in the AID).
pub fn find(short_name: &str) -> Option<&'static BandInfo> {
    BANDS.iter().find(|b| b.short_name == short_name)
}

/// A readable description, falling back to the code itself.
pub fn description(short_name: &str) -> String {
    find(short_name).map_or_else(|| short_name.to_string(), |b| b.description.to_string())
}

/// Plot colour for a band; unknown bands get a stable colour derived from
/// their name. `None` means "theme text colour".
pub fn color(short_name: &str) -> Option<[u8; 3]> {
    match find(short_name) {
        Some(b) => b.rgb,
        None => {
            let h = short_name
                .bytes()
                .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
            Some([
                64 + (h & 0x7f) as u8,
                64 + ((h >> 8) & 0x7f) as u8,
                64 + ((h >> 16) & 0x7f) as u8,
            ])
        }
    }
}

/// Map common alternative band spellings (BAA files, Gaia, user CSVs) onto
/// the AID short names.
pub fn normalize(raw: &str) -> String {
    let t = raw.trim();
    let mapped = match t.to_ascii_lowercase().as_str() {
        "" | "n/a" | "unknown" => UNKNOWN,
        "vis" | "vis." | "visual" => VISUAL,
        "v" | "johnson v" => "V",
        "b" | "johnson b" => "B",
        "u" | "johnson u" => "U",
        "r" | "rc" | "cousins r" => "R",
        "i" | "ic" | "cousins i" => "I",
        "cv" | "c" | "cl" | "clear" | "unfiltered" => "CV",
        "cr" => "CR",
        "tg" => "TG",
        "tb" => "TB",
        "tr" => "TR",
        "g" | "gaia g" | "gg" => GAIA_G,
        _ => return find(t).map_or_else(|| t.to_string(), |b| b.short_name.to_string()),
    };
    mapped.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_bands_resolve() {
        assert_eq!(description("V"), "Johnson V");
        assert_eq!(color("Vis."), None);
        assert_eq!(color("V"), Some([0, 255, 0]));
        assert!(color("WEIRD").is_some());
    }

    #[test]
    fn normalizes_aliases() {
        assert_eq!(normalize("Visual"), "Vis.");
        assert_eq!(normalize(" v "), "V");
        assert_eq!(normalize("Rc"), "R");
        assert_eq!(normalize("STB"), "STB");
        assert_eq!(normalize("Foo"), "Foo");
    }
}
