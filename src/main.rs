//! Screen and video capture script capture script.

mod args;
mod util;

use std::collections::HashMap;
use std::env::var;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use chrono::prelude::*;
use hostname::get_hostname;

use self::args::*;
use self::util::*;

fn main() -> Result<(), clap::Error> {
    let config = Config::from_args();
    let path = filename(config.mode());

    match config.mode() {
        Image => capture_image(&path, config.region()),
        Video(rate) => capture_video(&path, config.region(), rate),
    }

    println!("Capture saved to {:?}", path);

    Ok(())
}

/// Capture video of the screen.
fn capture_video(filename: &Path, region: ScreenRegion, framerate: u64) {
    let filename = filename.to_str().expect("Filename as string");
    let format = find_codec(
        FFMPEGSupport::formats(),
        &["matroska", "mp4"],
        FFMPEGSupport::encode,
    )
    .expect("ffmpeg supports matroska");
    println!("Format: {:#?}", format);

    let x11 = find_codec(
        FFMPEGSupport::formats(),
        &["x11grab"],
        FFMPEGSupport::decode,
    )
    .expect("ffmpeg supports x11 capture");
    println!("X11: {:#?}", x11);

    let pulse = find_codec(FFMPEGSupport::formats(), &["pulse"], FFMPEGSupport::decode)
        .expect("ffmpeg can record from pulseaudio");
    println!("Pulseaudio: {:#?}", pulse);

    let audio = find_codec(
        FFMPEGSupport::audio_encoders(),
        &["aac", "libvo_aac"],
        FFMPEGSupport::encode,
    )
    .expect("ffmpeg can encode audio");
    println!("Audio: {:#?}", audio);

    let video = find_codec(
        FFMPEGSupport::video_encoders(),
        &["h264_nvenc", "h264_qsv", "libx264", "h264"],
        FFMPEGSupport::encode,
    )
    .expect("ffmpeg can encode video");
    println!("Video: {:#?}", video);

    let (resolution, region) = x11_region_string(region);

    // TODO: Add audio output monitor
    let mut command = exec!(ffmpeg
        -hide_banner
        -threads (num_cpus::get())
        -y
        -f (x11)
            -draw_mouse (1)
            -framerate (framerate)
            -show_region (1)
            -video_size (resolution)
            -i (region)
        -f (pulse) -i default
        -f (format)
            -map ("0:0") ("-c:v") (video) ("-preset:v") fast -crf (16)
            -map ("1:0") ("-c:a") (audio) ("-b:a") ("256k")
        (filename)
    );
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Spawn ffmpeg");

    println!("Started 'ffmpeg' with PID #{}", child.id());

    child.wait().expect("Waiting for ffmpeg");
}

/// Get the X11 reference for the capture region.
fn x11_region_string(region: ScreenRegion) -> (String, String) {
    match region {
        Screen => x11_fullscreen(),
        Window => x11_current_window(),
        Select => unreachable!(),
    }
}

/// Get the region for the full screen.
fn x11_fullscreen() -> (String, String) {
    let lines = command_output(exec!(xdpyinfo));
    let (lines, _) = get_line(lines, |line| line.contains("screen #0"));
    let (_lines, dimensions) = get_nth_from_line(lines, |line| line.contains("dimensions:"), 1);

    (dimensions.to_owned(), format!("{}+0,0", x11_screen()))
}

/// Get the region for the current window.
fn x11_current_window() -> (String, String) {
    let window_id = x11_window();
    let lines = command_output(exec!(xwininfo - id(window_id)));
    let (lines, xpos) = get_nth_from_line(lines, |line| line.contains("Absolute upper-left X:"), 3);
    let (lines, ypos) = get_nth_from_line(lines, |line| line.contains("Absolute upper-left Y:"), 3);
    let (lines, width) = get_nth_from_line(lines, |line| line.contains("Width:"), 1);
    let (_lines, height) = get_nth_from_line(lines, |line| line.contains("Height:"), 1);

    (
        format!("{}x{}", width, height),
        format!("{}+{},{}", x11_screen(), xpos, ypos),
    )
}

/// Get the ID of the current window.
fn x11_window() -> String {
    let lines = command_output(exec!(xprop - root));
    let (_, window_id) = get_nth_from_line(lines, |line| line.contains("_NET_ACTIVE_WINDOW"), 4);
    window_id
}

/// Get the current screen.
fn x11_screen() -> String {
    format!(
        "{}.0",
        var("DISPLAY").expect("Get DISPLAY environment variable")
    )
}

/// Capture an image of the screen.
fn capture_image(filename: &Path, region: ScreenRegion) {
    let filename = filename.to_str().expect("Filename as string");
    let mut screenshot = exec!(("gnome-screenshot") - B - f(filename));
    match region {
        Window => screenshot.arg("-w"),
        Select => screenshot.arg("-a"),
        _ => &mut screenshot,
    };
    screenshot.status().expect("Take screenshot");
}

/// Determine the name of the file given the capture mode.
///
/// The file name is based on the current date and time.
///
/// Videos are stored in ~/Videos/Screenshot and are saved in Matroska format.
/// Images are stores in ~/Pictures/Screenshot and are saved in PNG format.
fn filename(mode: CaptureMode) -> PathBuf {
    let home = var("HOME").expect("Get home directory");
    let (subdir, extension) = match mode {
        Image => ("Pictures", "png"),
        Video(_) => ("Videos", "mkv"),
    };
    let now = Local::now().format("%Y-%m-%d.%H%M.%S");
    let hostname = get_hostname().expect("Get hostname");
    let hostname = hostname.split('.').nth(0).unwrap();
    let filename = format!("{}.{}.{}", hostname, now, extension);

    let mut path = Path::new(&home).to_owned();
    path.push(subdir);
    path.push("Screenshot");
    path.push(filename);

    path
}

fn find_codec(
    codecs: impl Iterator<Item = FFMPEGSupport>,
    names: &[&str],
    filter: impl Fn(&FFMPEGSupport) -> bool,
) -> Option<String> {
    let mut found = HashMap::new();

    for codec in codecs {
        for name in names {
            if codec.has_name(name) && filter(&codec) {
                found.insert(name, codec.clone());
            }
        }
    }

    for name in names {
        if let Some(codec) = found.remove(name) {
            return Some(codec.name().to_owned());
        }
    }

    None
}
