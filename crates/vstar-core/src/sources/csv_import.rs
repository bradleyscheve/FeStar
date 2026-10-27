//! Load observations from delimited text files.
//!
//! This covers the BAA VSS database's CSV download, the AAVSO "download"
//! format, and VStar's "simple" format (JD, magnitude, optional error and
//! band, no header). Columns are found by name, so files with extra or
//! reordered columns still load.

use std::path::Path;

use crate::bands;
use crate::model::{Dataset, Observation, StarInfo, parse_magnitude};
use crate::sources::aavso::validation_from_flag;
use crate::time::parse_ut_to_jd;
use crate::{Error, Result};

/// Column names we recognise, compared case-insensitively after trimming.
const JD: &[&str] = &[
    "jd",
    "julian date",
    "julian_date",
    "date (jd)",
    "jd (utc)",
    "hjd",
    "bjd",
    "jd/hjd",
    "date_jd",
];
const UT_DATE: &[&str] = &[
    "date",
    "ut",
    "ut date",
    "date (ut)",
    "utc",
    "datetime",
    "date/time",
];
const MAG: &[&str] = &["mag", "magnitude", "vmag", "mag."];
const ERR: &[&str] = &[
    "uncert",
    "uncertainty",
    "error",
    "err",
    "mag error",
    "magerr",
    "mag_err",
    "e_mag",
    "magnitude error",
];
const BAND: &[&str] = &["band", "filter", "fil", "passband"];
const OBSERVER: &[&str] = &[
    "observer code",
    "observer",
    "obscode",
    "observer_code",
    "by",
    "obs code",
    "observer id",
];
const FAINTER: &[&str] = &["fainterthan", "fainter than", "fainter_than"];
const VALIDATION: &[&str] = &["validation flag", "val", "validation"];
const COMMENT: &[&str] = &["comment", "comments", "notes", "remarks"];
const STAR: &[&str] = &["star", "star name", "starname", "name", "object", "target"];
const OBS_TYPE: &[&str] = &["obstype", "obs type", "method", "type"];

/// Load a delimited text file. `source_label` describes where it came from,
/// e.g. "BAA VSS database".
pub fn load_file(path: &Path, source_label: &str) -> Result<Dataset> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut ds = parse(&text, source_label)?;
    if ds.star.name.is_empty() {
        ds.star.name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    Ok(ds)
}

/// Parse delimited text already in memory.
pub fn parse(text: &str, source_label: &str) -> Result<Dataset> {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim_start_matches('\u{feff}'))
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .collect();
    let first = *lines
        .first()
        .ok_or_else(|| Error::Parse("the file is empty".into()))?;
    let delim = detect_delimiter(first);
    let body = lines.join("\n");

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(body.as_bytes());
    let mut records = rdr.records();
    let first_rec = records
        .next()
        .ok_or_else(|| Error::Parse("the file is empty".into()))??;

    let has_header = first_rec.get(0).is_some_and(|f| f.parse::<f64>().is_err());
    let cols = if has_header {
        Columns::from_header(&first_rec)?
    } else {
        Columns::simple(first_rec.len())
    };

    let mut observations = Vec::new();
    let mut star_name = String::new();
    let mut skipped = 0usize;
    let pending = if has_header {
        None
    } else {
        Some(Ok(first_rec))
    };

    for rec in pending.into_iter().chain(records) {
        let rec = rec?;
        match cols.observation(&rec) {
            Some(ob) => observations.push(ob),
            None => skipped += 1,
        }
        if star_name.is_empty()
            && let Some(name) = cols.star.and_then(|i| rec.get(i)).filter(|s| !s.is_empty())
        {
            star_name = name.to_string();
        }
    }

    if observations.is_empty() {
        return Err(Error::Parse(format!(
            "no observations could be read ({skipped} rows skipped); \
             the file needs a JD or date column and a magnitude column"
        )));
    }

    let mut ds = Dataset {
        star: StarInfo {
            name: star_name,
            ..Default::default()
        },
        source: source_label.to_string(),
        observations,
    };
    ds.sort_by_jd();
    Ok(ds)
}

fn detect_delimiter(line: &str) -> u8 {
    const CANDIDATES: [u8; 3] = *b"\t;,";
    CANDIDATES
        .into_iter()
        .max_by_key(|d| line.bytes().filter(|b| b == d).count())
        .filter(|d| line.as_bytes().contains(d))
        .unwrap_or(b',')
}

#[derive(Debug, Default)]
struct Columns {
    jd: Option<usize>,
    ut_date: Option<usize>,
    mag: usize,
    err: Option<usize>,
    band: Option<usize>,
    observer: Option<usize>,
    fainter: Option<usize>,
    validation: Option<usize>,
    comment: Option<usize>,
    star: Option<usize>,
    obs_type: Option<usize>,
}

impl Columns {
    fn from_header(rec: &csv::StringRecord) -> Result<Self> {
        let names: Vec<String> = rec
            .iter()
            .map(|h| h.trim().trim_matches('"').to_ascii_lowercase())
            .collect();
        let find = |aliases: &[&str]| {
            aliases
                .iter()
                .find_map(|a| names.iter().position(|n| n == a))
        };
        let jd = find(JD);
        let ut_date = find(UT_DATE);
        let mag = find(MAG).ok_or_else(|| {
            Error::Parse(format!(
                "no magnitude column found in header: {}",
                names.join(", ")
            ))
        })?;
        if jd.is_none() && ut_date.is_none() {
            return Err(Error::Parse(format!(
                "no JD or date column found in header: {}",
                names.join(", ")
            )));
        }
        Ok(Self {
            jd,
            ut_date,
            mag,
            err: find(ERR),
            band: find(BAND),
            observer: find(OBSERVER),
            fainter: find(FAINTER),
            validation: find(VALIDATION),
            comment: find(COMMENT),
            star: find(STAR),
            obs_type: find(OBS_TYPE),
        })
    }

    /// VStar's simple format: JD, magnitude, [error], [band].
    fn simple(width: usize) -> Self {
        Self {
            jd: Some(0),
            mag: 1,
            err: (width > 2).then_some(2),
            band: (width > 3).then_some(3),
            ..Default::default()
        }
    }

    fn observation(&self, rec: &csv::StringRecord) -> Option<Observation> {
        let get = |i: Option<usize>| i.and_then(|i| rec.get(i)).filter(|s| !s.is_empty());

        let jd = get(self.jd)
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| get(self.ut_date).and_then(parse_ut_to_jd))?;
        let (mag, ft_prefix, uncertain) = parse_magnitude(rec.get(self.mag)?)?;

        let mut ob = Observation::new(
            jd,
            mag,
            bands::normalize(get(self.band).unwrap_or(bands::UNKNOWN)),
        );
        ob.error = get(self.err)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|e| *e >= 0.0)
            .unwrap_or(0.0);
        ob.fainter_than =
            ft_prefix || get(self.fainter).is_some_and(|s| matches!(s, "1" | "yes" | "Y" | "true"));
        ob.uncertain = uncertain;
        if let Some(v) = get(self.validation) {
            ob.validation = validation_from_flag(v);
        }
        ob.observer = get(self.observer).map(str::to_string);
        ob.comment = get(self.comment).map(str::to_string);
        ob.obs_type = get(self.obs_type).map(str::to_string);
        Some(ob)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Validation;

    #[test]
    fn reads_vstar_simple_format() {
        let ds = parse(
            "2459000.5,8.2,0.01,V\n2458999.5\t8.3\n2459001.5,8.1\n",
            "test",
        )
        .unwrap();
        assert_eq!(
            ds.observations.len(),
            2,
            "the tab row is a different shape and is skipped"
        );
        assert_eq!(ds.observations[0].jd, 2459000.5);
        assert_eq!(ds.observations[0].band, "V");
        assert_eq!(ds.observations[0].error, 0.01);
    }

    #[test]
    fn reads_aavso_download_format() {
        let text = "JD,Magnitude,Uncertainty,HQuncertainty,Band,Observer Code,Comment Code(s),Comp Star 1,\
                    Comp Star 2,Charts,Comments,Transfomed,Airmass,Validation Flag,Cmag,Kmag,HJD,Star Name\n\
                    2459000.1234,<11.5,,,Vis.,ABC,,,,,,0,,Z,,,,SS CYG\n\
                    2459000.2000,8.95:,0.02,,V,XYZ,,,,,clouds,1,,T,,,,SS CYG\n";
        let ds = parse(text, "AAVSO file").unwrap();
        assert_eq!(ds.star.name, "SS CYG");
        let [a, b] = &ds.observations[..] else {
            panic!("expected two observations")
        };
        assert!(a.fainter_than && a.band == "Vis." && a.observer.as_deref() == Some("ABC"));
        assert!(b.uncertain && b.error == 0.02 && b.validation == Validation::Discrepant);
        assert_eq!(b.comment.as_deref(), Some("clouds"));
    }

    #[test]
    fn reads_ut_dated_file_with_filter_column() {
        let text = "# exported from somewhere\nObject;Date;Mag;Error;Filter;Observer\n\
                    R Leo;2000-01-01 12:00;6.5;0.1;Visual;JOE\n";
        let ds = parse(text, "BAA VSS database").unwrap();
        assert_eq!(ds.star.name, "R Leo");
        assert_eq!(ds.observations[0].jd, 2451545.0);
        assert_eq!(ds.observations[0].band, "Vis.");
    }

    #[test]
    fn explains_missing_columns() {
        let err = parse("when,brightness\n1,2\n", "x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("magnitude"), "{err}");
    }
}
