use clippy_app::options::{parse, ChatArgs, CliArgs, Command, USAGE};
use clippy_app::renderer::shader_sources;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn no_arguments_means_everything_unset() {
    assert_eq!(parse(args(&[])), Ok(Command::Run(CliArgs::default())));
    let default = CliArgs::default();
    assert_eq!(default.scale, None);
    assert_eq!(default.always_on_top, None);
    assert_eq!(default.chat, ChatArgs::default());
}

#[test]
fn scale_accepts_both_spellings_within_range() {
    let expected = |scale: u32| {
        Ok(Command::Run(CliArgs {
            scale: Some(scale),
            ..Default::default()
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
        Ok(Command::Run(CliArgs {
            always_on_top: Some(false),
            ..Default::default()
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
fn chat_flags_default_to_unset() {
    assert_eq!(ChatArgs::default().base_url, None);
    assert_eq!(ChatArgs::default().model, None);
    assert_eq!(ChatArgs::default().api_key, None);
    assert!(USAGE.contains("--base-url") && USAGE.contains("--model"));
    assert!(USAGE.contains("F2"));
}

#[test]
fn chat_flags_parse_in_both_spellings() {
    let expected = |base: &str, model: &str, key: Option<&str>| {
        Ok(Command::Run(CliArgs {
            chat: ChatArgs {
                base_url: Some(base.to_string()),
                model: Some(model.to_string()),
                api_key: key.map(String::from),
            },
            ..Default::default()
        }))
    };
    assert_eq!(
        parse(args(&[
            "--base-url",
            "http://localhost:11434/v1",
            "--model",
            "llama3.2"
        ])),
        expected("http://localhost:11434/v1", "llama3.2", None)
    );
    assert_eq!(
        parse(args(&[
            "--base-url=https://api.openai.com/v1",
            "--model=gpt-4o-mini",
            "--api-key=sk-abc",
        ])),
        expected("https://api.openai.com/v1", "gpt-4o-mini", Some("sk-abc"))
    );
    assert!(parse(args(&["--base-url"])).is_err());
    assert!(parse(args(&["--model"])).is_err());
    assert!(parse(args(&["--api-key"])).is_err());
}

#[test]
fn chat_flags_coexist_with_other_options() {
    let command = parse(args(&[
        "--scale",
        "4",
        "--no-always-on-top",
        "--model",
        "m",
        "--base-url",
        "http://h/v1",
    ]))
    .unwrap();
    let Command::Run(cli) = command else {
        panic!("expected Run");
    };
    assert_eq!(cli.scale, Some(4));
    assert_eq!(cli.always_on_top, Some(false));
    assert_eq!(cli.chat.model.as_deref(), Some("m"));
    assert_eq!(cli.chat.base_url.as_deref(), Some("http://h/v1"));
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
