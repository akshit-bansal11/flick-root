//! Editor and terminal launch commands. Arguments are passed as an argv list, never a shell line,
//! so a folder name cannot inject commands.

use std::path::Path;
use std::process::{Command, Stdio};

/// Program and arguments, ready to spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// Executable name or path.
    pub program: String,
    /// Arguments, one per element.
    pub args: Vec<String>,
}

/// Which editor `code <name>` opens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Editor {
    /// VS Code (`code`).
    #[default]
    VsCode,
    /// Cursor (`cursor`).
    Cursor,
    /// Zed (`zed`).
    Zed,
    /// User command; the folder is appended as the last argument.
    Custom(String),
}

impl Editor {
    /// Parse the `editor` option; blank means VS Code.
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "" | "code" => Self::VsCode,
            "cursor" => Self::Cursor,
            "zed" => Self::Zed,
            other => Self::Custom(other.to_owned()),
        }
    }

    /// Command that opens `path` in this editor. Errors on a custom command with an unclosed quote.
    pub fn open_spec(&self, path: &Path) -> Result<Spec, String> {
        let dir = path.to_string_lossy().into_owned();
        // VS Code and Cursor ship `.cmd` shims, which CreateProcess only finds with the extension.
        let (program, mut args) = match self {
            Self::VsCode => ("code.cmd".to_owned(), vec![]),
            Self::Cursor => ("cursor.cmd".to_owned(), vec![]),
            Self::Zed => ("zed".to_owned(), vec![]),
            Self::Custom(cmd) => {
                let cmd = cmd.trim();
                let (program, rest) = match cmd.strip_prefix('"') {
                    Some(q) => {
                        let (prog, rest) = q
                            .split_once('"')
                            .filter(|(p, _)| !p.is_empty())
                            .ok_or_else(|| {
                                format!("editor command has an unclosed or empty quote: {cmd}")
                            })?;
                        (Some(prog.to_owned()), rest)
                    }
                    None => (None, cmd),
                };
                let mut parts = rest.split_whitespace().map(str::to_owned);
                let program = match program {
                    Some(p) => p,
                    None => parts.next().unwrap_or_else(|| "code.cmd".to_owned()),
                };
                (program, parts.collect())
            }
        };
        args.push(dir);
        Ok(Spec { program, args })
    }
}

/// Windows Terminal at `path`. `None` when the path holds `;`, which `wt` reads as a command
/// separator and which cannot be escaped safely.
pub fn terminal_spec(path: &Path) -> Option<Spec> {
    let dir = path.to_string_lossy().into_owned();
    if dir.contains(';') {
        return None;
    }
    Some(Spec {
        program: "wt".into(),
        args: vec!["-d".into(), dir],
    })
}

/// Start the process without a console window and without waiting for it.
pub fn spawn(spec: &Spec) -> std::io::Result<()> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_args_with_spaces_and_ampersand() {
        let p = Path::new("D:\\dev\\my app & more");
        let s = Editor::VsCode.open_spec(p).unwrap();
        assert_eq!(s.program, "code.cmd");
        assert_eq!(s.args, ["D:\\dev\\my app & more"]);
        let t = terminal_spec(p).unwrap();
        assert_eq!(t.args, ["-d", "D:\\dev\\my app & more"]);
        assert!(terminal_spec(Path::new("D:\\a;b")).is_none());
    }

    #[test]
    fn parses_editors() {
        assert_eq!(Editor::parse(""), Editor::VsCode);
        assert_eq!(Editor::parse("cursor"), Editor::Cursor);
        assert_eq!(Editor::parse("zed"), Editor::Zed);
        let s = Editor::parse("sub.exe -n")
            .open_spec(Path::new("C:\\x y"))
            .unwrap();
        assert_eq!(s.program, "sub.exe");
        assert_eq!(s.args, ["-n", "C:\\x y"]);
    }

    #[test]
    fn quoted_custom_program() {
        let dir = Path::new(r"C:\x y");
        let s = Editor::parse(r#""C:\Program Files\Sublime Text\subl.exe" -n -a"#)
            .open_spec(dir)
            .unwrap();
        assert_eq!(s.program, r"C:\Program Files\Sublime Text\subl.exe");
        assert_eq!(s.args, ["-n", "-a", r"C:\x y"]);
        assert!(
            Editor::parse(r#""C:\Program Files\subl.exe -n"#)
                .open_spec(dir)
                .is_err()
        );
        assert!(Editor::parse(r#""" -n"#).open_spec(dir).is_err());
        let s = Editor::Custom(String::new()).open_spec(dir).unwrap();
        assert_eq!(s.program, "code.cmd");
        assert_eq!(s.args, [r"C:\x y"]);
    }
}
