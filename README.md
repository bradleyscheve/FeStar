# FeStar

A redesign of VStar with new AI integrations: a modern, fast desktop app for
variable star light curve analysis, written in Rust. It is a rewrite of [AAVSO VStar](https://github.com/AAVSO/VStar), which
it uses as the reference for its data loaders and analysis methods.

![FeStar showing a light curve (synthetic demo data)](docs/screenshot-dark.png)

## Status

Progress against the four-step plan:

1. **Foundation:** the app window, AAVSO and BAA data loading, and an
   interactive light curve. *Done.*
2. **Analysis:** period search, phase plots and models, favourite stars.
   *Built, in review.*
3. **Gaia:** Gaia DR3 epoch photometry for the selected star. *Built, in
   review.*
4. **AI and packaging:** optional AI help choosing period-search settings and
   explaining results, then a Windows installer. *Not started.*

Windows is the first target. The code is portable, so macOS and Linux builds
can follow.

## What works now

- **AAVSO International Database:** type a star name (e.g. `SS Cyg`) and load
  its observations, optionally limited to a JD range. Star details (type,
  period, spectral type, coordinates) come from VSX.
- **BAA VSS database and Gaia DR3:** download BAA observations as CSV and open
  the file, or load Gaia epoch photometry (G, BP, RP) for variable stars.
- **Other files:** AAVSO download files, VStar "simple format" files, and most
  CSV or tab-separated files with JD (or UT date) and magnitude columns.
- **Light curve:** zoom, pan, hover for details of each observation, per-band
  colours, error bars, fainter-than estimates, discrepant observations on or
  off, and Julian or calendar dates on the axis.
- **Analysis:** period search, phase plots and models.
- **One tab per loaded star**, so you can switch stars without reloading.
- **Star card:** variability type, spectral type and a star icon that reflects
  the type; buttons to open the star's VSP chart, VSX page, Stellarium and
  N.I.N.A. framing, and to view the latest AAVSO spectra.
- Light, dark or follow-Windows theme. Recent stars and settings are
  remembered between runs.

## Getting the Windows app

Every push to `main` builds the Windows app in GitHub Actions. Open the latest
run under the **Actions** tab and download the `festar-windows` artifact. It
contains `festar.exe`, which runs without installing anything.

If the window stays blank on an older PC, set the environment variable
`VSTAR_OPENGL=1` to use the OpenGL renderer instead of DirectX 12.

## Building from source

Install Rust from <https://rustup.rs>, then:

```
cargo run --release -p vstar-app --bin festar
cargo test
```

You can also pass a data file to open at startup:
`festar path\to\observations.csv`.

## Layout

- `crates/vstar-core`: data model, data sources and parsing. No UI code, fully
  unit-tested.
- `crates/vstar-app`: the desktop app (egui / eframe).

## Licence

VStar is licensed under the GNU Affero General Public License v3, and this
project ports code from it, so FeStar is under the same licence. See
[LICENSE](LICENSE).

Please acknowledge the data sources you use: the AAVSO International Database,
the BAA VSS database ("The BAA Photometry Database is acknowledged as the (part)
source of data on which this article was based"), and ESA Gaia.
