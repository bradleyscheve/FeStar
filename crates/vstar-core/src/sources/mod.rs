//! Places observations can be loaded from.

pub mod aavso;
pub mod csv_import;
pub mod gaia;
pub mod vsp;

/// Called as pages of data arrive with `(loaded so far, expected total)`.
/// Return `false` to cancel the load.
pub type Progress<'a> = &'a (dyn Fn(usize, Option<u64>) -> bool + Sync);

/// Percent-encode a query-string value the way VStar does: spaces become
/// "+", and everything but unreserved characters is escaped.
pub(crate) fn encode_query_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.trim().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_star_names() {
        assert_eq!(super::encode_query_value(" SS Cyg "), "SS+Cyg");
        assert_eq!(super::encode_query_value("V0405+Aur"), "V0405%2BAur");
    }
}
