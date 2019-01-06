//! Process command line arguments.

use std::str::FromStr;

use clap::{app_from_crate, crate_authors, crate_description, crate_name, crate_version};
use clap::{App, Arg};

/// Configuration from command line.
#[derive(Debug, Default)]
pub struct Config {
    region: ScreenRegion,
    mode: CaptureMode,
}

impl Config {
    /// Process configuration from command line.
    pub fn from_args() -> Self {
        let matches = Config::args().get_matches();

        let mode = match matches.value_of("mode").unwrap() {
            "image" => Image,
            "video" => Video(matches.value_of("rate").unwrap().parse().unwrap()),
            _ => unreachable!(),
        };

        let region = matches.value_of("region").unwrap().parse().unwrap();

        // Basic validation of particular combinations.
        let (mode, region) = match (mode, region) {
            // TODO: Add proper errors.
            (Video(_), Select) => panic!("Cannot select region for video capture"),
            (mode, region) => (mode, region),
        };

        Config {
            mode: mode,
            region: region,
        }
    }

    pub fn mode(&self) -> CaptureMode {
        self.mode
    }

    pub fn region(&self) -> ScreenRegion {
        self.region
    }

    fn args<'a, 'b>() -> App<'a, 'b> {
        let u64_validator = |value: String| {
            u64::from_str(&value)
                .map_err(|_| format!("{:?} is not an integer", value))
                .map(|_| ())
        };

        let region = Arg::with_name("region")
            .short("r")
            .takes_value(true)
            .help("The region to capture")
            .possible_values(&["screen", "window", "select"])
            .default_value("screen");

        let mode = Arg::with_name("mode")
            .short("m")
            .takes_value(true)
            .help("Whether to capture an image or video")
            .possible_values(&["image", "video"])
            .default_value("image");

        let framerate = Arg::with_name("rate")
            .short("R")
            .takes_value(true)
            .help("Framerate (fps) when capturing video")
            .validator(u64_validator)
            .default_value("30");

        app_from_crate!().arg(region).arg(mode).arg(framerate)
    }
}

/// Possible regions of the screen.
#[derive(Debug, Clone, Copy)]
pub enum ScreenRegion {
    Screen,
    Window,
    Select,
}
pub use self::ScreenRegion::*;

impl Default for ScreenRegion {
    fn default() -> Self {
        Screen
    }
}

impl FromStr for ScreenRegion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "screen" => Ok(Screen),
            "window" => Ok(Window),
            "select" => Ok(Select),
            _ => Err(()),
        }
    }
}

/// Possible capture modes.
#[derive(Debug, Clone, Copy)]
pub enum CaptureMode {
    /// Capture an image
    Image,
    /// Capture a video at a given framerate
    Video(u64),
}
pub use self::CaptureMode::*;

impl Default for CaptureMode {
    fn default() -> Self {
        Image
    }
}
