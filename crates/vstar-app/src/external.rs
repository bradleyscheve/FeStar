//! Opening the current star in programs on this computer: Stellarium and
//! N.I.N.A.

use serde::{Deserialize, Serialize};
use vstar_core::http::ReqwestClient;
use vstar_core::planetarium::{
    self, NINA_DEFAULT_PORT, STELLARIUM_DEFAULT_PORT, open_in_nina, open_in_stellarium,
};
use vstar_core::{Error, Result};

use crate::task::Task;

/// Where the other programs are, set in File > Preferences.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalApps {
    pub stellarium_path: String,
    pub stellarium_port: u16,
    pub nina_path: String,
    pub nina_port: u16,
}

impl Default for ExternalApps {
    fn default() -> Self {
        Self {
            stellarium_path: String::new(),
            stellarium_port: STELLARIUM_DEFAULT_PORT,
            nina_path: String::new(),
            nina_port: NINA_DEFAULT_PORT,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Program {
    Stellarium,
    Nina,
}

impl Program {
    fn name(self) -> &'static str {
        match self {
            Program::Stellarium => "Stellarium",
            Program::Nina => "N.I.N.A.",
        }
    }

    fn setup_hint(self) -> &'static str {
        match self {
            Program::Stellarium => {
                "In Stellarium, switch on the Remote Control plugin (Configuration > Plugins) \
                 and restart it."
            }
            Program::Nina => {
                "In N.I.N.A., install the Advanced API plugin and start its server \
                 (Plugins > Advanced API)."
            }
        }
    }
}

/// The server answered, but with an error: the program is running and it's
/// the setup that's wrong, so there is no point starting it again.
fn server_answered(e: &Error) -> bool {
    e.to_string().contains("server answered")
}

/// Send the position on a background thread. The result is a message for
/// the status bar.
pub fn open(
    ctx: &egui::Context,
    program: Program,
    apps: &ExternalApps,
    ra_deg: f64,
    dec_deg: f64,
) -> Task<String> {
    let apps = apps.clone();
    Task::spawn(ctx, format!("Opening in {}", program.name()), move |_| {
        send(program, &apps, ra_deg, dec_deg)
    })
}

fn send(program: Program, apps: &ExternalApps, ra_deg: f64, dec_deg: f64) -> Result<String> {
    let http = ReqwestClient::new()?;
    let result = match program {
        Program::Stellarium => open_in_stellarium(&http, apps.stellarium_port, ra_deg, dec_deg),
        Program::Nina => open_in_nina(&http, apps.nina_port, ra_deg, dec_deg),
    };
    let name = program.name();
    match result {
        Ok(()) => Ok(format!(
            "Sent {} {} to {name}.",
            planetarium::ra_hms(ra_deg),
            planetarium::dec_dms(dec_deg)
        )),
        Err(e) if server_answered(&e) => Err(Error::Analysis(format!(
            "{name} answered with an error ({e}). {}",
            program.setup_hint()
        ))),
        Err(e @ Error::Analysis(_)) => Err(e),
        Err(_) => {
            // Nothing is listening: start the program if we know where it is.
            let path = match program {
                Program::Stellarium => &apps.stellarium_path,
                Program::Nina => &apps.nina_path,
            };
            if path.trim().is_empty() {
                return Err(Error::Analysis(format!(
                    "Couldn't reach {name}. Start it, or set its location in File > Preferences. {}",
                    program.setup_hint()
                )));
            }
            std::process::Command::new(path.trim())
                .spawn()
                .map_err(|e| Error::Analysis(format!("Couldn't start {name} ({path}): {e}")))?;
            Ok(format!(
                "Started {name}. When it has finished opening, click the button again. {}",
                program.setup_hint()
            ))
        }
    }
}
