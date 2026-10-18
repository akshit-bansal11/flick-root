//! Source-level ADR-011 guards: no command returns a secret, secret commands are a closed set,
//! the launcher window cannot write secrets. The panic hook's redaction is unit-tested in
//! src/logging.rs (the module is private, so it cannot be reached from here).

use std::fs;

const SECRET_CMDS: [&str; 4] = ["set_secret", "delete_secret", "test_secret", "list_secrets"];

fn read(rel: &str) -> String {
    fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// `(name, return type)` of every `#[tauri::command]` fn under src/commands.
fn commands() -> Vec<(String, String)> {
    let dir = format!("{}/src/commands", env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        for chunk in text.split("#[tauri::command]").skip(1) {
            let sig = chunk.split('{').next().unwrap();
            let name = sig
                .split("fn ")
                .nth(1)
                .unwrap()
                .split(['(', '<'])
                .next()
                .unwrap()
                .trim()
                .to_owned();
            let ret = sig.split("->").nth(1).unwrap_or("").trim().to_owned();
            out.push((name, ret));
        }
    }
    out
}

#[test]
fn no_command_returns_a_secret() {
    let cmds = commands();
    assert!(cmds.len() > 20, "parser found too few commands");
    for (name, ret) in &cmds {
        assert!(
            !name.contains("get_secret") && !name.contains("reveal_secret"),
            "{name}"
        );
        assert!(!ret.contains("SecretString"), "{name} returns {ret}");
        assert!(!ret.contains("SecretValue"), "{name} returns {ret}");
    }
    // test_secret and list_secrets return typed metadata, never a bare String.
    for (name, ret) in &cmds {
        if SECRET_CMDS.contains(&name.as_str()) {
            assert!(!ret.contains("Result<String"), "{name} returns {ret}");
        }
    }
}

#[test]
fn secret_touching_commands_are_a_closed_set() {
    let build = read("build.rs");
    for line in build.lines().map(str::trim) {
        let Some(name) = line.strip_prefix('"').and_then(|l| l.strip_suffix("\",")) else {
            continue;
        };
        if ["secret", "token", "credential", "api_key"]
            .iter()
            .any(|w| name.contains(w))
        {
            assert!(
                SECRET_CMDS.contains(&name),
                "unexpected secret command {name}"
            );
        }
    }
    for c in SECRET_CMDS {
        assert!(
            build.contains(&format!("\"{c}\"")),
            "{c} missing from manifest"
        );
    }
}

#[test]
fn launcher_capability_cannot_write_secrets() {
    let default = read("capabilities/default.json");
    for c in SECRET_CMDS {
        let perm = format!("allow-{}", c.replace('_', "-"));
        assert!(!default.contains(&perm), "launcher window has {perm}");
    }
}
