use std::sync::Mutex;

use vstar_core::http::HttpClient;
use vstar_core::sources::aavso;
use vstar_core::{Error, Result, StarQuery, Validation};

/// Serves fixture files and records the URLs it was asked for.
struct FakeHttp {
    responses: Vec<(&'static str, &'static str)>,
    requested: Mutex<Vec<String>>,
}

impl HttpClient for FakeHttp {
    fn get_text(&self, url: &str) -> Result<String> {
        self.requested.lock().unwrap().push(url.to_string());
        self.responses
            .iter()
            .find(|(needle, _)| url.contains(needle))
            .map(|(_, body)| body.to_string())
            .ok_or_else(|| Error::Network(format!("unexpected URL {url}")))
    }
}

const INFO: &str = include_str!("fixtures/vsx_ss_cyg_info.xml");
const PAGE1: &str = include_str!("fixtures/vsx_ss_cyg_page1.xml");

#[test]
fn parses_star_info() {
    let info = aavso::parse_star_info(INFO).unwrap().unwrap();
    assert_eq!(info.name, "SS Cyg");
    assert_eq!(info.auid.as_deref(), Some("000-BCP-220"));
    assert_eq!(info.period_days, Some(0.27512973));
    assert_eq!(info.epoch_jd, None);
    assert_eq!(info.obs_count, Some(3));
}

#[test]
fn unknown_star_is_none() {
    assert_eq!(
        aavso::parse_star_info("<VSXObjects></VSXObjects>").unwrap(),
        None
    );
}

#[test]
fn loads_observations_using_auid() {
    let http = FakeHttp {
        responses: vec![("data=0", INFO), ("ident=000-BCP-220", PAGE1)],
        requested: Mutex::new(Vec::new()),
    };
    let query = StarQuery {
        name: "SS Cyg".into(),
        min_jd: Some(2460000.0),
        max_jd: None,
        bands: vec!["V".into()],
    };
    let ds = aavso::load(&http, &query, &|_, _| true).unwrap();

    assert_eq!(ds.observations.len(), 3, "the row without a JD is skipped");
    let o = &ds.observations;
    assert_eq!(
        (o[0].band.as_str(), o[0].observer.as_deref()),
        ("Vis.", Some("ABC"))
    );
    assert_eq!(o[1].error, 0.012);
    assert!(o[2].fainter_than);
    assert_eq!(o[2].validation, Validation::Discrepant);

    let urls = http.requested.lock().unwrap();
    assert_eq!(
        urls[0],
        "https://vsx.aavso.org/index.php?view=api.object&ident=SS+Cyg&data=0"
    );
    assert_eq!(
        urls[1],
        "https://vsx.aavso.org/index.php?view=api.object&ident=000-BCP-220&data=50000&fromjd=2460000&csv&band=V&mtype=std&page=1"
    );
    assert_eq!(
        urls.len(),
        2,
        "a short page means there is nothing more to fetch"
    );
}

#[test]
fn progress_callback_can_cancel() {
    let http = FakeHttp {
        responses: vec![("data=0", INFO), ("ident=000-BCP-220", PAGE1)],
        requested: Mutex::new(Vec::new()),
    };
    let err = aavso::load(&http, &StarQuery::named("SS Cyg"), &|_, _| false).unwrap_err();
    assert!(matches!(err, Error::Cancelled));
}
