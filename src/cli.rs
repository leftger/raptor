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
             -h, --help         Print this help message\n",
            config::MUSIC_DEFAULT_VOLUME
        );
    }
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
}
