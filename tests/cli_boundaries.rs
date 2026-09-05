#![cfg(feature = "cli")]

use fractal::{FractalError, FractalErrorCode, ProjectManifest};
use std::process::{Command, Output};
use tempfile::TempDir;

fn fractal(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fractal"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn json_success_and_error_output_are_machine_readable() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("project");
    let root_text = root.to_str().unwrap();

    let success = fractal(&["--json", "init", root_text, "--name", "CLI project"]);
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert!(success.stderr.is_empty());
    let manifest: ProjectManifest = serde_json::from_slice(&success.stdout).unwrap();
    assert_eq!(manifest.name, "CLI project");
    assert_eq!(manifest.version, 2);

    let failure = fractal(&["--project", root_text, "--json", "read", "missing"]);
    assert!(!failure.status.success());
    assert!(failure.stdout.is_empty());
    let error: FractalError = serde_json::from_slice(&failure.stderr).unwrap();
    assert_eq!(error.code, FractalErrorCode::NotFound);
    assert!(error.message.contains("page does not exist"));
}

#[test]
fn help_lists_the_complete_native_only_command_set() {
    let output = fractal(&["--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    let commands = help
        .split("Commands:\n")
        .nth(1)
        .unwrap()
        .split("\n\nOptions:")
        .next()
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();

    assert_eq!(
        commands,
        [
            "init",
            "inspect",
            "recover",
            "list",
            "folders",
            "folder",
            "new-folder",
            "set-folder-title",
            "set-page-title",
            "reorder-folder",
            "read",
            "parts",
            "set-content",
            "set-style",
            "restore-style",
            "set-metadata",
            "repair-page",
            "repair-project",
            "new",
            "recreate",
            "move",
            "move-folder",
            "delete",
            "delete-pages",
            "delete-folder",
            "search",
            "links",
            "backlinks",
            "derived-links",
            "link",
            "export-html",
            "export-folder-html",
            "check",
            "help",
        ]
    );
    for removed in ["write", "iframes", "embedded-by", "set-head-links"] {
        assert!(!commands.contains(&removed));
    }
}

#[test]
fn json_parse_errors_are_fractal_errors() {
    let output = fractal(&["--json", "write", "page"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: FractalError = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert!(error.message.contains("unrecognized subcommand 'write'"));
}
