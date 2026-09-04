use clippy_app::options::{parse, Command, Options, DEFAULT_SCALE, USAGE};
use clippy_app::renderer::shader_sources;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn no_arguments_means_defaults() {
    assert_eq!(parse(args(&[])), Ok(Command::Run(Options::default())));
    assert_eq!(Options::default().scale, DEFAULT_SCALE);
    assert!(Options::default().always_on_top);
}

#[test]
fn scale_accepts_both_spellings_within_range() {
    let expected = |scale| {
        Ok(Command::Run(Options {
            scale,
            ..Options::default()
        }))
    };
    assert_eq!(parse(args(&["--scale", "3"])), expected(3));
    assert_eq!(parse(args(&["--scale=4"])), expected(4));
    assert_eq!(parse(args(&["--scale", "1"])), expected(1));
    assert_eq!(parse(args(&["--scale", "8"])), expected(8));
    assert!(parse(args(&["--scale", "0"])).is_err());
    assert!(parse(args(&["--scale", "9"])).is_err());
    assert!(parse(args(&["--scale", "two"])).is_err());
    assert!(parse(args(&["--scale"])).is_err());
}

#[test]
fn window_level_flag_and_informational_commands() {
    assert_eq!(
        parse(args(&["--no-always-on-top"])),
        Ok(Command::Run(Options {
            always_on_top: false,
            ..Options::default()
        }))
    );
    assert_eq!(parse(args(&["--help"])), Ok(Command::Help));
    assert_eq!(parse(args(&["-h"])), Ok(Command::Help));
    assert_eq!(parse(args(&["--version"])), Ok(Command::Version));
    assert_eq!(parse(args(&["-V"])), Ok(Command::Version));
    assert_eq!(parse(args(&["--scale", "3", "--help"])), Ok(Command::Help));
}

#[test]
fn unknown_options_are_errors_that_name_the_offender() {
    let err = parse(args(&["--bogus"])).unwrap_err();
    assert!(err.0.contains("--bogus"));
    assert!(USAGE.contains("--scale"));
}

#[test]
fn shader_dialect_matches_the_context_kind() {
    let (vertex, fragment) = shader_sources(true, 2, 0);
    assert!(vertex.starts_with("#version 100") && fragment.contains("precision mediump float"));
    let (vertex, fragment) = shader_sources(true, 3, 1);
    assert!(vertex.starts_with("#version 100") && fragment.contains("gl_FragColor"));
    let (vertex, fragment) = shader_sources(false, 2, 1);
    assert!(vertex.starts_with("#version 110") && fragment.contains("gl_FragColor"));
    let (vertex, fragment) = shader_sources(false, 3, 1);
    assert!(vertex.starts_with("#version 110") && fragment.contains("texture2D"));
    let (vertex, fragment) = shader_sources(false, 3, 2);
    assert!(vertex.starts_with("#version 150") && fragment.contains("out vec4 o_color"));
    let (vertex, _) = shader_sources(false, 4, 6);
    assert!(vertex.starts_with("#version 150"));
}
