use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub initial_path: Option<PathBuf>,
    pub show_hidden: bool,
    pub show_labels: bool,
    pub show_fps: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            initial_path: None,
            show_hidden: false,
            show_labels: true,
            show_fps: true,
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

        for arg in args {
            match arg.as_str() {
                "--help" | "-h" => return Err(TryParseError::Help),
                "--hidden" => options.show_hidden = true,
                "--no-labels" => options.show_labels = false,
                "--no-fps" => options.show_fps = false,
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
             [DIRECTORY]  Start in this directory instead of your home directory\n\n\
             Options:\n  \
             --hidden      Show hidden files on startup\n  \
             --no-labels   Hide file labels on startup\n  \
             --no-fps      Hide the FPS counter in the status bar\n  \
             -h, --help    Print this help message\n"
        );
    }
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
}
