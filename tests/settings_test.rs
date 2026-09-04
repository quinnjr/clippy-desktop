//! Tests for settings resolution and the config-file format. These never touch
//! the real config file (no `save`/`load`), only the pure logic.

use clippy_app::options::{ChatArgs, CliArgs, DEFAULT_SCALE};
use clippy_app::settings::{file_from, resolve_with_env, ChatFile, FileConfig};

fn no_env(_: &str) -> Option<String> {
    None
}

fn cli(scale: Option<u32>, always_on_top: Option<bool>, base_url: Option<&str>) -> CliArgs {
    CliArgs {
        scale,
        always_on_top,
        chat: ChatArgs {
            base_url: base_url.map(String::from),
            model: None,
            api_key: None,
        },
    }
}

#[test]
fn defaults_apply_when_nothing_is_set() {
    let settings = resolve_with_env(&CliArgs::default(), &FileConfig::default(), no_env);
    assert_eq!(settings.scale, DEFAULT_SCALE);
    assert!(settings.always_on_top);
    // With no server anywhere, chat is unconfigured.
    assert!(settings.chat.is_err());
}

#[test]
fn appearance_precedence_is_cli_then_file_then_default() {
    let file = FileConfig {
        scale: Some(5),
        always_on_top: Some(false),
        chat: ChatFile::default(),
    };
    // File wins over the default.
    let settings = resolve_with_env(&CliArgs::default(), &file, no_env);
    assert_eq!(settings.scale, 5);
    assert!(!settings.always_on_top);

    // The command line wins over the file.
    let settings = resolve_with_env(&cli(Some(3), Some(true), None), &file, no_env);
    assert_eq!(settings.scale, 3);
    assert!(settings.always_on_top);
}

#[test]
fn an_out_of_range_scale_is_clamped() {
    let file = FileConfig {
        scale: Some(999),
        ..FileConfig::default()
    };
    let settings = resolve_with_env(&CliArgs::default(), &file, no_env);
    assert!((1..=8).contains(&settings.scale));
}

#[test]
fn chat_precedence_is_cli_then_env_then_file() {
    let file = FileConfig {
        chat: ChatFile {
            base_url: Some("http://file/v1".into()),
            model: Some("file-model".into()),
            api_key: Some("file-key".into()),
        },
        ..FileConfig::default()
    };
    let env = |name: &str| match name {
        "OPENAI_BASE_URL" => Some("http://env/v1".to_string()),
        _ => None,
    };

    // Nothing on the CLI or env for base_url -> file. Model/key only in file.
    let settings = resolve_with_env(&CliArgs::default(), &file, env);
    let config = settings.chat.expect("configured from env + file");
    assert_eq!(config.base_url, "http://env/v1"); // env beats file
    assert_eq!(config.model, "file-model"); // file (no env/cli)
    assert_eq!(config.api_key.as_deref(), Some("file-key"));

    // The CLI base_url beats both env and file.
    let settings = resolve_with_env(&cli(None, None, Some("http://cli/v1")), &file, env);
    assert_eq!(settings.chat.unwrap().base_url, "http://cli/v1");
}

#[test]
fn file_config_round_trips_through_toml() {
    let file = file_from(4, false, "http://h/v1/", "m", "");
    // The API key was empty, so it is omitted.
    assert_eq!(file.chat.api_key, None);
    assert_eq!(file.chat.base_url.as_deref(), Some("http://h/v1/"));

    let text = toml::to_string_pretty(&file).unwrap();
    let parsed: FileConfig = toml::from_str(&text).unwrap();
    assert_eq!(parsed, file);

    // A partial file (only a base URL) loads with everything else absent.
    let partial: FileConfig = toml::from_str("[chat]\nbase_url = \"http://h/v1\"\n").unwrap();
    assert_eq!(partial.scale, None);
    assert_eq!(partial.always_on_top, None);
    assert_eq!(partial.chat.base_url.as_deref(), Some("http://h/v1"));
}
