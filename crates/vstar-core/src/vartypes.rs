//! Variable-star type descriptions and the star "look" derived from them.
//!
//! The descriptions follow the AAVSO VSX / GCVS variability-type list. They
//! are built into the program, so tooltips work offline and instantly. (They
//! were written from the published type list, not downloaded from VSX.)

/// (code, description). Codes are matched case-insensitively.
const TYPES: &[(&str, &str)] = &[
    // Eruptive
    (
        "FU",
        "FU Orionis: eruptive young star that brightened by about 6 magnitudes and stays bright for decades.",
    ),
    (
        "FUOR",
        "FU Orionis: eruptive young star that brightened by about 6 magnitudes and stays bright for decades.",
    ),
    (
        "GCAS",
        "Gamma Cassiopeiae: rapidly rotating early-type (B) star that sheds a disc of matter, causing irregular outbursts of up to 1.5 mag.",
    ),
    (
        "I",
        "Irregular variable with no definite pattern, spectral type not well known.",
    ),
    (
        "IA",
        "Irregular variable of early spectral type (O to A) with no definite pattern.",
    ),
    (
        "IB",
        "Irregular variable of intermediate to late spectral type (F to M) with no definite pattern.",
    ),
    (
        "IN",
        "Orion variable: irregular young star still associated with nebulosity.",
    ),
    (
        "INA",
        "Orion variable of early type (B to A), sometimes with Algol-like fadings.",
    ),
    (
        "INB",
        "Orion variable of intermediate to late type (F to M).",
    ),
    (
        "INT",
        "T Tauri-type Orion variable: young star with emission lines, irregular, of type F to M.",
    ),
    (
        "IT",
        "T Tauri-type Orion variable: young star with emission lines, irregular, of type F to M.",
    ),
    (
        "IS",
        "Rapid irregular variable (changes of 0.5 to 1 mag within hours to days).",
    ),
    ("ISA", "Rapid irregular variable of early type (O to A)."),
    (
        "ISB",
        "Rapid irregular variable of intermediate to late type (F to M).",
    ),
    (
        "RCB",
        "R Coronae Borealis: hydrogen-poor, carbon-rich supergiant that suddenly fades by 1 to 9 mag as soot clouds form, then slowly recovers.",
    ),
    (
        "SDOR",
        "S Doradus: luminous blue variable, a very massive and unstable hot star with eruptions.",
    ),
    (
        "UV",
        "UV Ceti flare star: red dwarf with sudden flares of up to several magnitudes lasting minutes.",
    ),
    (
        "UVN",
        "Orion flare star: young flare star associated with nebulosity.",
    ),
    (
        "WTTS",
        "Weak-line T Tauri star: young star without strong emission lines.",
    ),
    (
        "CTTS",
        "Classical T Tauri star: young star still accreting from its disc.",
    ),
    (
        "YSO",
        "Young stellar object: star still forming or newly formed, often irregular.",
    ),
    (
        "ZAND",
        "Z Andromedae: symbiotic binary (a hot star and a red giant) with outbursts of up to 4 mag.",
    ),
    (
        "N",
        "Nova: close binary in which a white dwarf ignited a thermonuclear explosion, brightening by 7 to 16 mag within days, then fading over months to years.",
    ),
    ("NA", "Fast nova: fades by 3 mag within 100 days or less."),
    ("NB", "Slow nova: fades by 3 mag in more than 100 days."),
    (
        "NC",
        "Very slow nova: stays near maximum for years before fading.",
    ),
    (
        "NL",
        "Nova-like variable: cataclysmic binary with a bright accretion disc that shows few or no outbursts.",
    ),
    (
        "NR",
        "Recurrent nova: nova seen to erupt more than once, typically every few decades.",
    ),
    (
        "SN",
        "Supernova: a star destroyed in a cataclysmic explosion, brightening by 20 mag or more.",
    ),
    (
        "SNI",
        "Type I supernova: explosion with no hydrogen lines in its spectrum.",
    ),
    (
        "SNIA",
        "Type Ia supernova: thermonuclear explosion of a white dwarf.",
    ),
    (
        "SNII",
        "Type II supernova: core collapse of a massive star, with hydrogen lines.",
    ),
    (
        "SNIB",
        "Type Ib supernova: core collapse of a stripped star, no hydrogen.",
    ),
    (
        "SNIC",
        "Type Ic supernova: core collapse of a stripped star, no hydrogen or helium.",
    ),
    (
        "UG",
        "U Geminorum (dwarf nova): cataclysmic binary with outbursts of 2 to 6 mag that repeat every weeks to months.",
    ),
    (
        "UGSS",
        "SS Cygni dwarf nova: outbursts of 2 to 6 mag at fairly regular intervals.",
    ),
    (
        "UGSU",
        "SU Ursae Majoris dwarf nova: normal outbursts plus brighter, longer superoutbursts with superhumps.",
    ),
    (
        "UGZ",
        "Z Camelopardalis dwarf nova: outbursts interrupted by standstills at intermediate brightness.",
    ),
    (
        "UGER",
        "ER Ursae Majoris dwarf nova: SU UMa type with very frequent outbursts (every 20 to 50 days).",
    ),
    (
        "UGWZ",
        "WZ Sagittae dwarf nova: very rare, large outbursts (6 to 9 mag) every decade or more.",
    ),
    (
        "WZ",
        "WZ Sagittae dwarf nova: very rare, large outbursts (6 to 9 mag) every decade or more.",
    ),
    (
        "AM",
        "AM Herculis: polar, a cataclysmic binary with a strongly magnetic white dwarf and no accretion disc.",
    ),
    (
        "DQ",
        "DQ Herculis: intermediate polar, a cataclysmic binary with a magnetic white dwarf that spins fast.",
    ),
    (
        "IP",
        "Intermediate polar: cataclysmic binary with a magnetic white dwarf spinning faster than the orbit.",
    ),
    (
        "CV",
        "Cataclysmic variable: close binary in which a white dwarf pulls matter from a companion.",
    ),
    (
        "BE",
        "Be star: rapidly rotating B star with a gaseous disc, showing emission lines and slow changes.",
    ),
    // Pulsating
    (
        "ACEP",
        "Anomalous Cepheid: metal-poor pulsating giant with periods of 0.4 to 2 days, brighter than RR Lyrae stars.",
    ),
    (
        "BCEP",
        "Beta Cephei: hot B-type pulsator with periods of 0.1 to 0.3 days and amplitudes of 0.01 to 0.3 mag.",
    ),
    (
        "BCEPS",
        "Short-period Beta Cephei: B-type pulsator with periods under 0.1 days.",
    ),
    (
        "BLAP",
        "Blue large-amplitude pulsator: hot, low-mass star with periods of 3 to 75 minutes.",
    ),
    (
        "CEP",
        "Cepheid: yellow supergiant that pulsates regularly with periods of 1 to 135 days; period and luminosity are linked.",
    ),
    (
        "DCEP",
        "Classical (Delta) Cepheid: young, massive pulsating supergiant, period 1 to 135 days, used as a distance indicator.",
    ),
    (
        "DCEPS",
        "Delta Cepheid with small amplitude (under 0.5 mag) and nearly symmetric light curve.",
    ),
    (
        "CW",
        "W Virginis: Population II Cepheid with periods of 0.8 to 35 days, fainter than classical Cepheids.",
    ),
    ("CWA", "W Virginis variable with period longer than 8 days."),
    (
        "CWB",
        "W Virginis variable with period shorter than 8 days.",
    ),
    (
        "DSCT",
        "Delta Scuti: A to F star pulsating with periods of 0.01 to 0.2 days and amplitudes up to 0.9 mag.",
    ),
    ("DSCTC", "Low-amplitude Delta Scuti star (under 0.1 mag)."),
    (
        "HADS",
        "High-amplitude Delta Scuti star (over 0.1 mag), usually pulsating in one or two modes.",
    ),
    (
        "GDOR",
        "Gamma Doradus: F-type star with non-radial pulsations, periods of 0.4 to 3 days.",
    ),
    (
        "L",
        "Slow irregular variable: late-type giant with no clear periodicity.",
    ),
    (
        "LB",
        "Slow irregular variable of late type (K, M, C or S), usually a giant.",
    ),
    ("LC", "Slow irregular supergiant variable of late type."),
    (
        "M",
        "Mira: long-period (80 to 1000 days) red giant with large visual amplitude (2.5 to 11 mag).",
    ),
    (
        "PVTEL",
        "PV Telescopii: helium-rich supergiant pulsating with periods of about 0.1 to 1 day.",
    ),
    (
        "RPHS",
        "Rapidly pulsating hot subdwarf, with periods of a few minutes.",
    ),
    (
        "RR",
        "RR Lyrae: old horizontal-branch pulsator with periods of 0.2 to 1.2 days and amplitudes of 0.2 to 2 mag.",
    ),
    (
        "RRAB",
        "RR Lyrae with asymmetric light curve and steep rise, fundamental-mode pulsator (period 0.3 to 1.2 days).",
    ),
    (
        "RRC",
        "RR Lyrae with nearly sinusoidal light curve, first-overtone pulsator (period 0.2 to 0.5 days).",
    ),
    (
        "RRD",
        "Double-mode RR Lyrae, pulsating in the fundamental mode and the first overtone together.",
    ),
    (
        "RV",
        "RV Tauri: yellow supergiant with alternating deep and shallow minima, period 30 to 150 days.",
    ),
    ("RVA", "RV Tauri star whose mean brightness stays constant."),
    (
        "RVB",
        "RV Tauri star whose mean brightness varies with a long secondary period.",
    ),
    (
        "SR",
        "Semiregular variable: giant or supergiant with noticeable but imperfect periodicity (20 to 2000 days).",
    ),
    (
        "SRA",
        "Semiregular giant of late type with persistent periodicity and small amplitude (under 2.5 mag).",
    ),
    (
        "SRB",
        "Semiregular giant of late type with poorly defined periodicity or alternating periods.",
    ),
    (
        "SRC",
        "Semiregular supergiant of late type, periods from 30 to thousands of days.",
    ),
    ("SRD", "Semiregular giant or supergiant of type F, G or K."),
    (
        "SRS",
        "Semiregular pulsating red giant with a short period (tens of days).",
    ),
    (
        "SXPHE",
        "SX Phoenicis: Population II pulsator like a Delta Scuti star, periods of 0.04 to 0.08 days.",
    ),
    (
        "ZZ",
        "ZZ Ceti: pulsating white dwarf, periods of 30 seconds to 25 minutes.",
    ),
    ("ZZA", "ZZ Ceti white dwarf with hydrogen atmosphere (DA)."),
    ("ZZB", "Pulsating white dwarf with helium atmosphere (DB)."),
    (
        "ZZO",
        "Pulsating white dwarf with a carbon-oxygen atmosphere (DO).",
    ),
    ("PUL", "Pulsating variable of unspecified type."),
    (
        "V361HYA",
        "V361 Hydrae: pulsating hot subdwarf B star with periods of minutes.",
    ),
    (
        "V1093HER",
        "V1093 Herculis: pulsating subdwarf B star with periods of hours.",
    ),
    // Rotating
    (
        "ACV",
        "Alpha Canum Venaticorum: magnetic A-type star with spots; brightness changes as it rotates (0.01 to 0.1 mag).",
    ),
    ("ACVO", "Alpha CVn star that also shows rapid oscillations."),
    (
        "BY",
        "BY Draconis: spotted K or M dwarf whose brightness changes as it rotates.",
    ),
    (
        "ELL",
        "Rotating ellipsoidal variable: close binary distorted by tides, with no eclipses.",
    ),
    (
        "FKCOM",
        "FK Comae Berenices: rapidly rotating, spotted giant.",
    ),
    (
        "PSR",
        "Pulsar: rotating neutron star, seen in optical light with periods of milliseconds to seconds.",
    ),
    (
        "R",
        "Reflection variable: close binary in which one star lights up the other.",
    ),
    (
        "ROT",
        "Rotating variable: brightness changes due to spots or other features, as the star spins.",
    ),
    (
        "RS",
        "RS Canum Venaticorum: close binary with a very active, spotted component.",
    ),
    (
        "SXARI",
        "SX Arietis: magnetic helium-rich B-type star changing as it rotates.",
    ),
    // Eclipsing and binary
    (
        "E",
        "Eclipsing binary: two stars that pass in front of each other as seen from Earth.",
    ),
    (
        "EA",
        "Algol-type eclipsing binary: nearly constant light between sharply defined eclipses.",
    ),
    (
        "EB",
        "Beta Lyrae-type eclipsing binary: continuously changing light with unequal minima.",
    ),
    (
        "EW",
        "W Ursae Majoris-type eclipsing binary: two stars in contact, minima of nearly equal depth, period under a day.",
    ),
    (
        "D",
        "Detached binary: two stars well inside their Roche lobes.",
    ),
    ("DM", "Detached main-sequence binary."),
    ("DS", "Detached binary with a subgiant."),
    (
        "DW",
        "Detached binary similar to W UMa stars but not in contact.",
    ),
    ("K", "Contact (or near-contact) binary."),
    ("KE", "Contact binary of early spectral type (O to A)."),
    ("KW", "Contact binary of W UMa type."),
    ("SD", "Semi-detached binary: one star fills its Roche lobe."),
    ("GS", "Binary with a giant or supergiant component."),
    ("PN", "Binary with a planetary-nebula nucleus."),
    ("WD", "Binary with a white dwarf component."),
    ("WR", "Binary with a Wolf-Rayet component."),
    (
        "EC",
        "Eclipsing binary showing an extra component of variability.",
    ),
    ("EP", "Eclipsing system with a planet transiting its star."),
    (
        "DIP",
        "Dipper: star dimmed by passing dust clouds in its disc.",
    ),
    // X-ray and compact
    (
        "X",
        "X-ray source: strong X-ray emitter with optical variability.",
    ),
    (
        "XB",
        "X-ray binary: a neutron star or black hole pulling matter from a companion.",
    ),
    ("XF", "Fluctuating X-ray source."),
    ("XI", "X-ray irregular variable."),
    ("XJ", "X-ray binary with relativistic jets."),
    ("XND", "X-ray nova with a dwarf companion."),
    ("XNG", "X-ray nova with a giant companion."),
    ("XP", "X-ray pulsar: magnetic neutron star in a binary."),
    ("XPR", "X-ray pulsar with reflection effect."),
    ("XM", "X-ray burster with a long burst period."),
    // Other
    (
        "AGN",
        "Active galactic nucleus: a supermassive black hole feeding on gas, brightness varies irregularly.",
    ),
    (
        "BLLAC",
        "BL Lacertae object: blazar with rapid, irregular variations.",
    ),
    (
        "BLAZ",
        "Blazar: active galaxy with a jet pointed toward us.",
    ),
    ("QSO", "Quasar: very distant, luminous active galaxy."),
    ("CST", "Constant star, not known to vary."),
    ("MISC", "Miscellaneous variable that fits no other class."),
    (
        "S",
        "Rapid irregular variable (changes in minutes to hours).",
    ),
    ("VAR", "Variable star of unspecified type."),
    ("VAR?", "Variable star of unspecified type."),
    (
        "TTS",
        "T Tauri star: young, still contracting star with irregular changes.",
    ),
    ("GRB", "Gamma-ray burst afterglow."),
    (
        "LPV",
        "Long-period variable: cool giant with a pulsation period of weeks to years.",
    ),
];

/// Description of one variability-type code (such as `EA` or `DCEP`).
pub fn describe(code: &str) -> Option<&'static str> {
    let code = code.trim().trim_matches(|c| c == ':' || c == '*');
    TYPES
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(_, d)| *d)
}

/// Splits a VSX type string such as `EA/SD`, `DCEP:` or `RRAB|RRC` into its
/// codes, with a flag that says the type is uncertain (a trailing colon).
pub fn split_codes(var_type: &str) -> Vec<(String, bool)> {
    var_type
        .split(['/', '|', '+', ',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let uncertain = s.ends_with(':');
            // Drop qualifiers like "(B)" or "(M)" when they are not in the list.
            let code = s.trim_end_matches(':').trim();
            (code.to_string(), uncertain)
        })
        .collect()
}

/// Tooltip text for a type string: one line per code.
pub fn explain(var_type: &str) -> String {
    let mut lines = Vec::new();
    for (code, uncertain) in split_codes(var_type) {
        let base = code
            .split('(')
            .next()
            .unwrap_or(&code)
            .trim()
            .to_ascii_uppercase();
        let text = describe(&code).or_else(|| describe(&base));
        let mut line = match text {
            Some(t) => format!("{code}: {t}"),
            None => format!("{code}: not in the built-in type list."),
        };
        if uncertain {
            line.push_str(" (Type is uncertain.)");
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// How a star is drawn next to its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarLook {
    Single,
    Binary,
    /// Nova and other accretion-disc systems: a star inside a disc.
    Nova,
    /// Pulsating stars: a star with a halo and arrows showing it swell.
    Pulsator,
}

const NOVA_CODES: &[&str] = &[
    "N", "NA", "NB", "NC", "NL", "NR", "UG", "UGSS", "UGSU", "UGZ", "UGER", "UGWZ", "WZ", "ZAND",
    "AM", "DQ", "IP", "CV",
];
const BINARY_CODES: &[&str] = &[
    "E", "EA", "EB", "EW", "EC", "ELL", "D", "DM", "DS", "DW", "K", "KE", "KW", "SD", "GS", "WD",
    "WR", "XB", "RS", "R",
];

const PULSATOR_CODES: &[&str] = &[
    "ACEP", "BCEP", "BCEPS", "BLAP", "CEP", "DCEP", "DCEPS", "CW", "CWA", "CWB", "DSCT", "DSCTC",
    "HADS", "GDOR", "LPV", "M", "PUL", "PVTEL", "RPHS", "RR", "RRAB", "RRC", "RRD", "RV", "RVA",
    "RVB", "SR", "SRA", "SRB", "SRC", "SRD", "SRS", "SXPHE", "V361HYA", "V1093HER", "ZZ", "ZZA",
    "ZZB", "ZZO",
];

/// Picks the look from the first code of a type string.
pub fn star_look(var_type: Option<&str>) -> StarLook {
    let Some(first) = var_type.and_then(|t| split_codes(t).into_iter().next()) else {
        return StarLook::Single;
    };
    let code = first
        .0
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    if NOVA_CODES.contains(&code.as_str()) {
        StarLook::Nova
    } else if BINARY_CODES.contains(&code.as_str()) {
        StarLook::Binary
    } else if PULSATOR_CODES.contains(&code.as_str()) {
        StarLook::Pulsator
    } else {
        StarLook::Single
    }
}

/// Approximate colour of a star as seen by eye, from its spectral type
/// (`G2V`, `M3-4III`, `B0Ve+...`). `None` when the type isn't recognised.
pub fn spectral_color(spectral: Option<&str>) -> Option<[u8; 3]> {
    let s = spectral?.trim().trim_start_matches(['d', 'g', 's', 'k']);
    let mut chars = s.chars();
    let c = chars.next()?.to_ascii_uppercase();
    if c == 'W' {
        // Wolf-Rayet (WN, WC) are hot and blue.
        return Some([155, 176, 255]);
    }
    if c == 'D' {
        // White dwarf.
        return Some([220, 230, 255]);
    }
    Some(match c {
        'O' => [155, 176, 255],
        'B' => [170, 191, 255],
        'A' => [202, 215, 255],
        'F' => [248, 247, 255],
        'G' => [255, 244, 160],
        'K' => [255, 190, 110],
        'M' => [255, 130, 80],
        'C' | 'R' | 'N' => [235, 90, 60],
        'S' => [255, 150, 90],
        'L' | 'T' => [200, 80, 60],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_common_types() {
        assert!(describe("EA").unwrap().contains("Algol"));
        assert!(describe("dcep").unwrap().contains("Cepheid"));
        assert!(describe("ZZZ").is_none());
    }

    #[test]
    fn composite_and_uncertain_types() {
        let text = explain("EA/SD");
        assert!(text.contains("EA:") && text.contains("SD:"));
        assert!(explain("DCEP:").contains("uncertain"));
        assert!(explain("QQ").contains("not in the built-in"));
    }

    #[test]
    fn looks() {
        assert_eq!(star_look(Some("EA/SD")), StarLook::Binary);
        assert_eq!(star_look(Some("NA")), StarLook::Nova);
        assert_eq!(star_look(Some("NR")), StarLook::Nova);
        assert_eq!(star_look(Some("M")), StarLook::Pulsator);
        assert_eq!(star_look(Some("RRC/BL:")), StarLook::Pulsator);
        assert_eq!(star_look(Some("DCEP")), StarLook::Pulsator);
        assert_eq!(star_look(Some("UV")), StarLook::Single);
        assert_eq!(star_look(None), StarLook::Single);
    }

    #[test]
    fn spectral_colours() {
        assert_eq!(spectral_color(Some("M3-4III")), Some([255, 130, 80]));
        assert_eq!(spectral_color(Some("B0Ve")), Some([170, 191, 255]));
        assert_eq!(spectral_color(Some("Pec")), None);
        assert_eq!(spectral_color(None), None);
    }
}
