use crate::config;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub initial_path: Option<PathBuf>,
    pub show_hidden: bool,
    pub show_labels: bool,
    pub show_fps: bool,
    /// Procedural music starts on by default.
    pub music: bool,
    pub music_volume: f32,
    /// Frame-time benchmark: seconds to run before exiting, zero for off.
    pub bench_seconds: f32,
    /// Whether the benchmark drives the game as it measures.
    pub bench_ride: bool,
    /// Raster settings that trade looks for frame time.
    pub render: RenderOptions,
    /// Start the window at this size instead of the default.
    pub window: Option<WindowSize>,
}

/// The render knobs, as parsed from the command line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    pub msaa: u32,
    pub bloom: bool,
    pub scanlines: bool,
    pub vignette: bool,
}

/// Window size override, in physical pixels.
pub type WindowSize = (u32, u32);

impl RenderOptions {
    /// Everything that measured expensive, turned down.
    pub fn fast() -> Self {
        Self {
            msaa: 1,
            bloom: false,
            scanlines: true,
            vignette: true,
        }
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            msaa: config::MSAA_SAMPLES,
            bloom: true,
            scanlines: true,
            vignette: true,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            initial_path: None,
            show_hidden: false,
            show_labels: true,
            show_fps: true,
            music: true,
            music_volume: config::MUSIC_DEFAULT_VOLUME,
            bench_seconds: 0.0,
            bench_ride: true,
            render: RenderOptions::default(),
            window: None,
        }
    }
}

impl Options {
    /// Parse command-line arguments from the process environment.
    ///
    /// Prints help or an error and exits when the arguments cannot be used.
    pub fn parse() -> Self {
        match Self::try_parse(std::env::args().skip(1)) {
            Ok(options) => options,
            Err(TryParseError::Help) => {
                Self::print_help();
                std::process::exit(0);
            }
            Err(TryParseError::Invalid(message)) => {
                eprintln!("raptor: {message}\n");
                Self::print_help();
                std::process::exit(2);
            }
        }
    }

    fn try_parse<I>(args: I) -> Result<Self, TryParseError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut options = Options::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Err(TryParseError::Help),
                "--hidden" => options.show_hidden = true,
                "--no-labels" => options.show_labels = false,
                "--no-fps" => options.show_fps = false,
                // Benchmarks turn music off so the numbers are about the frame,
                // not about the synth thread sharing the CPU.
                "--bench" => {
                    options.bench_seconds = BENCH_DEFAULT_SECONDS;
                    options.music = false;
                }
                "--bench-seconds" => {
                    let Some(value) = args.next() else {
                        return Err(TryParseError::Invalid(
                            "--bench-seconds needs a number of seconds".to_string(),
                        ));
                    };
                    let seconds: f32 = value.parse().map_err(|_| {
                        TryParseError::Invalid(format!(
                            "--bench-seconds expects a number, got '{value}'"
                        ))
                    })?;
                    if !(seconds.is_finite() && seconds > 0.0) {
                        return Err(TryParseError::Invalid(
                            "--bench-seconds must be greater than zero".to_string(),
                        ));
                    }
                    options.bench_seconds = seconds;
                    options.music = false;
                }
                "--bench-static" => options.bench_ride = false,
                "--msaa" => {
                    let Some(value) = args.next() else {
                        return Err(TryParseError::Invalid(
                            "--msaa needs a sample count: 1, 2, 4 or 8".to_string(),
                        ));
                    };
                    let samples: u32 = value.parse().map_err(|_| {
                        TryParseError::Invalid(format!("--msaa expects a number, got '{value}'"))
                    })?;
                    if !matches!(samples, 1 | 2 | 4 | 8) {
                        return Err(TryParseError::Invalid(format!(
                            "--msaa supports 1, 2, 4 or 8 samples, got '{value}'"
                        )));
                    }
                    options.render.msaa = samples;
                }
                "--no-bloom" => options.render.bloom = false,
                "--fast" => {
                    // The preset for weak integrated graphics, measured on an
                    // Intel Iris 6100: about 3x the frame rate of the defaults.
                    options.render = RenderOptions::fast();
                }
                "--no-scanlines" => options.render.scanlines = false,
                "--no-vignette" => options.render.vignette = false,
                "--window" => {
                    let Some(value) = args.next() else {
                        return Err(TryParseError::Invalid(
                            "--window needs a size like 960x540".to_string(),
                        ));
                    };
                    options.window = Some(parse_window_size(&value)?);
                }
                "--no-music" => options.music = false,
                "--music-volume" => {
                    let Some(value) = args.next() else {
                        return Err(TryParseError::Invalid(
                            "--music-volume needs a value between 0 and 1".to_string(),
                        ));
                    };
                    options.music_volume = parse_volume(&value)?;
                }
                _ if arg.starts_with('-') => {
                    return Err(TryParseError::Invalid(format!("unknown option '{arg}'")));
                }
                _ if options.initial_path.is_none() => {
                    options.initial_path = Some(PathBuf::from(arg));
                }
                _ => {
                    return Err(TryParseError::Invalid(format!(
                        "unexpected argument '{arg}'"
                    )));
                }
            }
        }

        Ok(options)
    }

    fn print_help() {
        println!(
            "RAPTOR - Realtime Abstracted Path Tree Observer\n\n\
             Usage: raptor [OPTIONS] [DIRECTORY]\n\n\
             Arguments:\n  \
             [DIRECTORY]        Start in this directory instead of your home directory\n\n\
             Options:\n  \
             --hidden           Show hidden files on startup\n  \
             --no-labels        Hide file labels on startup\n  \
             --no-fps           Hide the FPS counter in the status bar\n  \
             --no-music         Start with procedural music disabled (toggle with N)\n  \
             --music-volume V   Music volume 0.0-1.0 (default {})\n  \
             --bench            Ride for {}s, print frame-time stats, then exit\n  \
             --bench-seconds N  Benchmark for N seconds instead\n  \
             --bench-static     Benchmark a settled ride without steering\n  \
             --msaa N           Multisampling: 1, 2, 4 or 8 (default {})\n  \
             --fast             Preset for weak GPUs: no MSAA, no bloom\n  \
             --no-scanlines     Disable the scanline overlay\n  \
             --no-vignette      Disable the vignette post-process\n  \
             --window WxH       Start with a window of this size, e.g. 960x540\n  \
             -h, --help         Print this help message\n",
            config::MUSIC_DEFAULT_VOLUME,
            BENCH_DEFAULT_SECONDS as u32,
            config::MSAA_SAMPLES,
        );
    }
}

/// How long `--bench` rides when no duration is given.
const BENCH_DEFAULT_SECONDS: f32 = 12.0;

/// Parses a `WIDTHxHEIGHT` window size, e.g. `960x540`.
fn parse_window_size(value: &str) -> Result<WindowSize, TryParseError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(TryParseError::Invalid(format!(
            "window size '{value}' should look like 960x540"
        )));
    };
    let parse = |part: &str, axis: &str| {
        part.parse::<u32>()
            .ok()
            .filter(|size| *size >= 320)
            .ok_or_else(|| TryParseError::Invalid(format!("window {axis} '{part}' is too small")))
    };
    Ok((parse(width, "width")?, parse(height, "height")?))
}

fn parse_volume(value: &str) -> Result<f32, TryParseError> {
    let Ok(volume) = value.parse::<f32>() else {
        return Err(TryParseError::Invalid(format!(
            "invalid music volume '{value}'"
        )));
    };
    if !(0.0..=1.0).contains(&volume) {
        return Err(TryParseError::Invalid(format!(
            "music volume '{value}' is out of range 0.0-1.0"
        )));
    }
    Ok(volume)
}

#[derive(Debug)]
enum TryParseError {
    Help,
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::Options;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::try_parse(args.iter().map(|s| s.to_string()))
            .map_err(|_| "failed to parse".to_string())
    }

    #[test]
    fn defaults_are_used_when_no_args_are_given() {
        let options = parse(&[]).unwrap();
        assert_eq!(options, Options::default());
    }

    #[test]
    fn positional_path_is_first_non_option_argument() {
        let options = parse(&["--hidden", "/tmp"]).unwrap();
        assert_eq!(options.initial_path.as_deref().unwrap(), "/tmp");
        assert!(options.show_hidden);
    }

    #[test]
    fn toggles_are_applied() {
        let options = parse(&["--no-labels", "--no-fps", "--hidden"]).unwrap();
        assert!(!options.show_labels);
        assert!(!options.show_fps);
        assert!(options.show_hidden);
    }

    #[test]
    fn unknown_options_are_rejected() {
        assert!(parse(&["--wat"]).is_err());
    }

    #[test]
    fn multiple_positional_arguments_are_rejected() {
        assert!(parse(&["/tmp", "/var"]).is_err());
    }

    #[test]
    fn music_is_on_by_default_and_can_be_disabled() {
        assert!(parse(&[]).unwrap().music);
        assert!(!parse(&["--no-music"]).unwrap().music);
    }

    #[test]
    fn music_volume_accepts_a_value_in_range() {
        let options = parse(&["--music-volume", "0.7"]).unwrap();
        assert!((options.music_volume - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn music_volume_rejects_missing_or_out_of_range_values() {
        assert!(parse(&["--music-volume"]).is_err());
        assert!(parse(&["--music-volume", "loud"]).is_err());
        assert!(parse(&["--music-volume", "1.5"]).is_err());
    }

    #[test]
    fn msaa_accepts_known_sample_counts_only() {
        assert_eq!(parse(&["--msaa", "1"]).unwrap().render.msaa, 1);
        assert_eq!(parse(&["--msaa", "8"]).unwrap().render.msaa, 8);
        assert!(parse(&["--msaa", "3"]).is_err());
        assert!(parse(&["--msaa", "lots"]).is_err());
        assert!(parse(&["--msaa"]).is_err());
    }

    #[test]
    fn the_fast_preset_turns_down_the_expensive_settings() {
        let render = parse(&["--fast"]).unwrap().render;
        assert_eq!(render.msaa, 1, "no multisampling");
        assert!(!render.bloom, "no HDR bloom");
        assert_eq!(render, super::RenderOptions::fast());
    }

    #[test]
    fn render_toggles_are_independent() {
        let options = parse(&["--no-bloom"]).unwrap();
        assert!(!options.render.bloom);
        assert_eq!(options.render.msaa, crate::config::MSAA_SAMPLES);

        let options = parse(&["--no-scanlines", "--no-vignette"]).unwrap();
        assert!(!options.render.scanlines);
        assert!(!options.render.vignette);
        assert!(options.render.bloom);
    }

    #[test]
    fn window_size_parses_a_width_by_height_pair() {
        assert_eq!(
            parse(&["--window", "960x540"]).unwrap().window,
            Some((960, 540))
        );
        assert!(parse(&["--window", "960"]).is_err());
        assert!(parse(&["--window", "100x540"]).is_err());
        assert!(parse(&["--window"]).is_err());
    }

    #[test]
    fn the_benchmark_flag_selects_a_duration_and_drops_music() {
        let options = parse(&["--bench"]).unwrap();
        assert!(options.bench_seconds > 0.0);
        assert!(!options.music, "the synth thread would skew the numbers");

        let options = parse(&["--bench-seconds", "5"]).unwrap();
        assert!((options.bench_seconds - 5.0).abs() < f32::EPSILON);
        assert!(parse(&["--bench-seconds", "0"]).is_err());

        assert!(!parse(&["--bench-static"]).unwrap().bench_ride);
    }
}
