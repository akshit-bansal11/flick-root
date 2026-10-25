//! VS Code colour-theme import. The file is untrusted (user disk or the internet): its size is
//! capped before any parsing, JSONC is stripped by a linear, non-recursive state machine,
//! `serde_json` keeps its default recursion limit, and only `name` and the string values under
//! `colors` are read. `include` is never followed.

use serde_json::{Map, Value};

use super::color::{Rgb, blend_over, parse_hex_alpha, to_hex};
use super::derive::{Resolved, resolve};
use super::{Seed, Source, ThemeError};

/// Largest theme file accepted, in bytes.
pub const MAX_BYTES: usize = 1_048_576;
const MAX_NAME: usize = 80;
const MAX_SLUG: usize = 40;
const INCLUDE_NOTE: &str = "this theme includes another file; its colours were not read";

/// How one app role was filled from the VS Code file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingRow {
    /// App role, like `panel`.
    pub role: &'static str,
    /// VS Code key the colour came from, or the first key tried when none was usable.
    pub key: &'static str,
    /// Colour fed to the derivation (fallback or derived value when `used` is false), `#rrggbb`.
    pub hex: String,
    /// True when the theme file supplied a usable colour for this role.
    pub used: bool,
}

/// A parsed import, ready to preview and save.
#[derive(Debug, Clone)]
pub struct VsImport {
    /// The seed to save, id `imported:<slug>`.
    pub seed: Seed,
    /// One row per role, in seed field order.
    pub rows: Vec<MappingRow>,
    /// The derived theme; `resolved.adjusted` lists the colours raised for contrast.
    pub resolved: Resolved,
    /// Things the user should know, like an ignored `include`.
    pub notes: Vec<String>,
}

#[derive(Clone, Copy)]
enum State {
    Code,
    Str,
    StrEscape,
    LineComment,
    BlockComment,
}

/// Turns JSONC into JSON: drops a leading BOM, `//` and `/* */` comments outside strings, and
/// trailing commas before `}` or `]`. One pass, no recursion. Fails on input over [`MAX_BYTES`],
/// an unterminated block comment or an unterminated string.
pub fn strip_jsonc(src: &str) -> Result<String, ThemeError> {
    if src.len() > MAX_BYTES {
        return Err(ThemeError::TooLarge);
    }
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut out = String::with_capacity(src.len());
    let mut state = State::Code;
    // A comma is held back until the next significant character shows it is not trailing.
    let mut comma = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match state {
            State::Code => match c {
                '/' if chars.peek() == Some(&'/') => {
                    chars.next();
                    state = State::LineComment;
                }
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    state = State::BlockComment;
                }
                ',' => {
                    if comma {
                        out.push(',');
                    }
                    comma = true;
                }
                '}' | ']' => {
                    comma = false;
                    out.push(c);
                }
                c if c.is_whitespace() => out.push(c),
                c => {
                    if comma {
                        out.push(',');
                        comma = false;
                    }
                    if c == '"' {
                        state = State::Str;
                    }
                    out.push(c);
                }
            },
            State::Str => {
                match c {
                    '\\' => state = State::StrEscape,
                    '"' => state = State::Code,
                    _ => {}
                }
                out.push(c);
            }
            State::StrEscape => {
                state = State::Str;
                out.push(c);
            }
            State::LineComment => {
                // CR alone also ends a line (classic Mac line endings).
                if c == '\n' || c == '\r' {
                    state = State::Code;
                    out.push(c);
                }
            }
            State::BlockComment => {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    state = State::Code;
                    // Keeps `1/**/2` from fusing into one token.
                    out.push(' ');
                }
            }
        }
    }
    match state {
        State::Str | State::StrEscape => Err(ThemeError::Jsonc("unterminated string")),
        State::BlockComment => Err(ThemeError::Jsonc("unterminated comment")),
        State::Code | State::LineComment => {
            if comma {
                out.push(',');
            }
            Ok(out)
        }
    }
}

/// Lowercase `[a-z0-9-]` slug of `name`: at most 40 bytes, never empty, no dots or separators,
/// never a bare Windows device name (`con`, `com1`, ...; those get `-theme` appended).
pub fn slug(name: &str) -> String {
    slug_opt(name).unwrap_or_else(|| "theme".to_owned())
}

/// [`slug`] without the generic fallback: `None` when `name` has no ASCII letters or digits.
fn slug_opt(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.chars() {
        if out.len() >= MAX_SLUG {
            break;
        }
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        None
    } else if is_device_name(trimmed) {
        // At most 4 bytes + 6, well under MAX_SLUG.
        Some(format!("{trimmed}-theme"))
    } else {
        Some(trimmed.to_owned())
    }
}

/// Windows reserved device names, for a lowercase ASCII slug (it never contains a dot, so the
/// whole slug is the file name part of `<slug>.json`).
fn is_device_name(s: &str) -> bool {
    matches!(s, "con" | "prn" | "aux" | "nul")
        || (s.len() == 4
            && (s.starts_with("com") || s.starts_with("lpt"))
            && s.as_bytes()[3].is_ascii_digit())
}

/// Format and invisible characters that can hide or reorder text (soft hyphen, bidi controls,
/// zero-width spaces and joiners, BOM).
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

/// Control and invisible characters removed, trimmed, at most 80 characters.
fn clean_name(raw: &str) -> String {
    let kept: String = raw
        .chars()
        .filter(|&c| !c.is_control() && !is_invisible(c))
        .collect();
    let short: String = kept.trim().chars().take(MAX_NAME).collect();
    short.trim_end().to_owned()
}

/// A key found with a usable colour: `(key, colour, alpha)`.
type Picked = (&'static str, Rgb, u8);

/// First key in `keys` whose value is a `#rgb`, `#rrggbb` or `#rrggbbaa` string.
fn pick(colors: &Map<String, Value>, keys: &[&'static str]) -> Option<Picked> {
    keys.iter().find_map(|&k| {
        let (c, a) = parse_hex_alpha(colors.get(k)?.as_str()?)?;
        Some((k, c, a))
    })
}

/// Panel and bg are the ground everything blends onto, so their alpha is ignored.
fn opaque(p: Picked, notes: &mut Vec<String>) -> Rgb {
    if p.2 != 255 {
        notes.push(format!("{} has transparency; treated as opaque", p.0));
    }
    p.1
}

fn row(role: &'static str, keys: &[&'static str], p: Option<Picked>, hex: String) -> MappingRow {
    MappingRow {
        role,
        key: p.map_or(keys[0], |p| p.0),
        hex,
        used: p.is_some(),
    }
}

const PANEL: &[&str] = &["editor.background"];
const BG: &[&str] = &["sideBar.background"];
const TEXT: &[&str] = &["editor.foreground"];
const MUTED: &[&str] = &["descriptionForeground"];
const ACCENT: &[&str] = &["button.background", "focusBorder", "textLink.foreground"];
const SEL: &[&str] = &["list.activeSelectionBackground"];
const LINE: &[&str] = &["sideBar.border"];

/// Parses a VS Code colour theme into an imported seed. `file_stem` names the theme when the file
/// has no usable `name`. Alpha colours are blended onto the panel; colours in any other format
/// count as missing. Errors: [`ThemeError::TooLarge`] (checked first), [`ThemeError::Jsonc`],
/// [`ThemeError::Json`], [`ThemeError::NotATheme`], [`ThemeError::Unreadable`].
pub fn parse(src: &str, file_stem: &str) -> Result<VsImport, ThemeError> {
    let json = strip_jsonc(src)?;
    let root: Value = serde_json::from_str(&json)?;
    let root = root
        .as_object()
        .ok_or(ThemeError::NotATheme("top level is not an object"))?;
    let colors = match root.get("colors") {
        Some(Value::Object(m)) => m,
        Some(_) => return Err(ThemeError::NotATheme("colors is not an object")),
        None => return Err(ThemeError::NotATheme("no colors object")),
    };

    let mut notes = Vec::new();
    if root.contains_key("include") {
        notes.push(INCLUDE_NOTE.to_owned());
    }

    let panel_p = pick(colors, PANEL).ok_or(ThemeError::NotATheme(
        "editor.background is missing or not a hex colour",
    ))?;
    let text_p = pick(colors, TEXT).ok_or(ThemeError::NotATheme(
        "editor.foreground is missing or not a hex colour",
    ))?;
    let panel = opaque(panel_p, &mut notes);
    let bg_p = pick(colors, BG);
    let bg = bg_p.map_or(panel, |p| opaque(p, &mut notes));
    let blend = |p: Picked| blend_over(p.1, p.2, panel);
    let text = blend(text_p);
    let muted_p = pick(colors, MUTED);
    let muted = muted_p.map_or(text, blend);
    let accent_p = pick(colors, ACCENT);
    let accent = accent_p.map_or(text, blend);
    let sel_p = pick(colors, SEL);
    let line_p = pick(colors, LINE);

    let raw_name = root.get("name").and_then(Value::as_str).unwrap_or("");
    let name = [raw_name, file_stem]
        .into_iter()
        .map(clean_name)
        .find(|n| !n.is_empty())
        .unwrap_or_else(|| "Imported theme".to_owned());
    let id = slug_opt(&name)
        .or_else(|| slug_opt(file_stem))
        .unwrap_or_else(|| slug(""));

    let seed = Seed {
        id: format!("imported:{id}"),
        name: format!("{name} (imported)"),
        source: Source::Imported,
        bg: to_hex(bg),
        panel: to_hex(panel),
        text: to_hex(text),
        muted: to_hex(muted),
        accent: to_hex(accent),
        sel: sel_p.map(|p| to_hex(blend(p))),
        line: line_p.map(|p| to_hex(blend(p))),
        surface: None,
        upstream: None,
    };
    let resolved = resolve(&seed)?;
    let rows = vec![
        row("bg", BG, bg_p, seed.bg.clone()),
        row("panel", PANEL, Some(panel_p), seed.panel.clone()),
        row("text", TEXT, Some(text_p), seed.text.clone()),
        row("muted", MUTED, muted_p, seed.muted.clone()),
        row("accent", ACCENT, accent_p, seed.accent.clone()),
        row(
            "sel",
            SEL,
            sel_p,
            seed.sel
                .clone()
                .unwrap_or_else(|| resolved.roles.sel.clone()),
        ),
        row(
            "line",
            LINE,
            line_p,
            seed.line
                .clone()
                .unwrap_or_else(|| resolved.roles.line.clone()),
        ),
    ];
    Ok(VsImport {
        seed,
        rows,
        resolved,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(colors: &str) -> String {
        format!(r#"{{"name": "T", "colors": {{{colors}}}}}"#)
    }

    const BASE: &str = r##""editor.background": "#1a1b26", "editor.foreground": "#c0caf5""##;

    fn row_of<'a>(imp: &'a VsImport, role: &str) -> &'a MappingRow {
        imp.rows.iter().find(|r| r.role == role).unwrap()
    }

    #[test]
    fn comments_in_all_positions() {
        let src = r##"// head
/* block */ { // after brace
  /* before key */ "name" /* after key */ : /* after colon */ "C" /* after value */ , // eol
  "colors": { "editor.background": "#000000" /* x */, /* y */ "editor.foreground": "#ffffff" }
  /* before close */ } // tail without newline"##;
        let imp = parse(src, "f").unwrap();
        assert_eq!(imp.seed.name, "C (imported)");
        assert_eq!(imp.seed.panel, "#000000");
        assert_eq!(imp.seed.text, "#ffffff");
    }

    #[test]
    fn double_slash_inside_string_is_kept() {
        let src = r##"{"$schema": "vscode://schemas/color-theme", "name": "a // b \" // c",
            "colors": {"editor.background": "#000000", "editor.foreground": "#ffffff"}}"##;
        let out = strip_jsonc(src).unwrap();
        assert!(out.contains("vscode://schemas/color-theme"));
        assert_eq!(
            parse(src, "f").unwrap().seed.name,
            "a // b \" // c (imported)"
        );
    }

    #[test]
    fn block_comment_inside_key_is_literal() {
        let src = theme(&format!(
            r##"{BASE}, "editor/* x */.background": "#ff0000", "button/**/.background": "#00ff00""##
        ));
        assert!(
            strip_jsonc(&src)
                .unwrap()
                .contains("\"editor/* x */.background\"")
        );
        let imp = parse(&src, "f").unwrap();
        assert_eq!(imp.seed.panel, "#1a1b26");
        assert!(!row_of(&imp, "accent").used);
    }

    #[test]
    fn trailing_commas_removed() {
        let src = r##"{"name": "T", "tokenColors": [1, 2, /* c */ ], "colors": {
            "editor.background": "#000000", "editor.foreground": "#ffffff", // c
        },}"##;
        assert!(parse(src, "f").is_ok());
        assert_eq!(strip_jsonc("[1,]").unwrap(), "[1]");
        assert_eq!(strip_jsonc("{\"a\":1 , }").unwrap(), "{\"a\":1  }");
    }

    #[test]
    fn bom_is_dropped() {
        let src = format!("\u{feff}{}", theme(BASE));
        assert_eq!(parse(&src, "f").unwrap().seed.panel, "#1a1b26");
    }

    #[test]
    fn unterminated_comment_and_string_are_errors() {
        assert!(matches!(
            strip_jsonc("{ /* open"),
            Err(ThemeError::Jsonc("unterminated comment"))
        ));
        assert!(matches!(
            strip_jsonc(r#"{"name": "abc"#),
            Err(ThemeError::Jsonc("unterminated string"))
        ));
        assert!(matches!(
            parse(r#"{"name": "abc\"#, "f"),
            Err(ThemeError::Jsonc(_))
        ));
    }

    #[test]
    fn size_cap_checked_before_parsing() {
        let over = " ".repeat(MAX_BYTES + 1);
        assert!(matches!(parse(&over, "f"), Err(ThemeError::TooLarge)));
        let src = theme(BASE);
        let exact = format!("{src}{}", " ".repeat(MAX_BYTES - src.len()));
        assert_eq!(exact.len(), MAX_BYTES);
        assert!(parse(&exact, "f").is_ok());
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        // Valid JSON throughout, so only serde_json's recursion limit can reject it.
        let is_depth = |r: Result<VsImport, ThemeError>| match r {
            Err(ThemeError::Json(e)) => e.to_string().contains("recursion limit exceeded"),
            _ => false,
        };
        let deep = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
        assert!(is_depth(parse(&deep, "f")));
        let deep_obj = format!(
            r#"{{"colors": {{"x": {}0{}}}}}"#,
            r#"{"a":"#.repeat(10_000),
            "}".repeat(10_000)
        );
        assert!(is_depth(parse(&deep_obj, "f")));
    }

    #[test]
    fn colors_missing_wrong_type_or_not_strings() {
        assert!(matches!(
            parse(r#"{"name": "T"}"#, "f"),
            Err(ThemeError::NotATheme("no colors object"))
        ));
        assert!(matches!(
            parse(r##"{"colors": ["#000000"]}"##, "f"),
            Err(ThemeError::NotATheme("colors is not an object"))
        ));
        assert!(matches!(parse("[]", "f"), Err(ThemeError::NotATheme(_))));
        let src = theme(r##""editor.background": 0, "editor.foreground": {"a": "#fff"}"##);
        assert!(matches!(parse(&src, "f"), Err(ThemeError::NotATheme(_))));
    }

    #[test]
    fn rgba_short_and_rgb_function_rejected() {
        let short = theme(r##""editor.background": "#000f", "editor.foreground": "#ffffff""##);
        assert!(matches!(parse(&short, "f"), Err(ThemeError::NotATheme(_))));
        let func =
            theme(r##""editor.background": "#000000", "editor.foreground": "rgb(255,255,255)""##);
        assert!(matches!(parse(&func, "f"), Err(ThemeError::NotATheme(_))));
        let opt = theme(&format!(
            r##"{BASE}, "button.background": "#f00f", "focusBorder": "#7aa2f7""##
        ));
        let imp = parse(&opt, "f").unwrap();
        assert_eq!(row_of(&imp, "accent").key, "focusBorder");
        assert_eq!(imp.seed.accent, "#7aa2f7");
    }

    #[test]
    fn alpha_is_blended_onto_panel() {
        let src = theme(
            r##""editor.background": "#202020", "editor.foreground": "#ffffff80",
                "list.activeSelectionBackground": "#ffffff33""##,
        );
        let imp = parse(&src, "f").unwrap();
        // 32 + 223 * 128/255 = 143.94 -> 144 (0x90); 32 + 223 * 0.2 = 76.6 -> 77 (0x4d).
        // Onto black these would be #808080 and #333333.
        assert_eq!(imp.seed.text, "#909090");
        assert_eq!(imp.seed.sel.as_deref(), Some("#4d4d4d"));
        assert!(imp.notes.is_empty());
    }

    #[test]
    fn panel_and_bg_alpha_ignored_with_a_note() {
        let src = theme(
            r##""editor.background": "#00000080", "sideBar.background": "#11111100",
                "editor.foreground": "#ffffff""##,
        );
        let imp = parse(&src, "f").unwrap();
        assert_eq!(imp.seed.panel, "#000000");
        assert_eq!(imp.seed.bg, "#111111");
        assert_eq!(imp.notes.len(), 2);
        assert!(imp.notes[0].contains("editor.background"));
    }

    #[test]
    fn missing_optional_keys_fall_back() {
        let imp = parse(&theme(BASE), "f").unwrap();
        assert_eq!(imp.seed.bg, imp.seed.panel);
        assert_eq!(imp.seed.muted, imp.seed.text);
        assert_eq!(imp.seed.accent, imp.seed.text);
        assert_eq!(imp.seed.sel, None);
        assert_eq!(imp.seed.line, None);
        for role in ["bg", "muted", "accent", "sel", "line"] {
            assert!(!row_of(&imp, role).used, "{role}");
        }
        assert_eq!(row_of(&imp, "accent").key, "button.background");
        assert_eq!(row_of(&imp, "sel").hex, imp.resolved.roles.sel);
        let link = parse(
            &theme(&format!(r##"{BASE}, "textLink.foreground": "#7aa2f7""##)),
            "f",
        )
        .unwrap();
        assert_eq!(row_of(&link, "accent").key, "textLink.foreground");
        assert!(row_of(&link, "accent").used);
    }

    #[test]
    fn include_is_ignored_with_a_note() {
        let src = format!(r#"{{"include": "./base.json", "colors": {{{BASE}}}}}"#);
        let imp = parse(&src, "f").unwrap();
        assert_eq!(imp.notes, [INCLUDE_NOTE]);
    }

    #[test]
    fn slug_is_safe() {
        assert_eq!(slug("../..\\x"), "x");
        assert_eq!(slug(""), "theme");
        assert_eq!(slug("..."), "theme");
        assert_eq!(slug("Tokyo Night Storm!"), "tokyo-night-storm");
        assert_eq!(slug(&"ab".repeat(100)).len(), MAX_SLUG);
        let src = format!(r#"{{"name": "../..\\x", "colors": {{{BASE}}}}}"#);
        assert_eq!(parse(&src, "f").unwrap().seed.id, "imported:x");
    }

    #[test]
    fn empty_and_long_names() {
        let empty = format!(r#"{{"name": "", "colors": {{{BASE}}}}}"#);
        let imp = parse(&empty, "my-theme").unwrap();
        assert_eq!(imp.seed.name, "my-theme (imported)");
        assert_eq!(imp.seed.id, "imported:my-theme");
        let ctrl = format!(r#"{{"name": "\u0000\u0007", "colors": {{{BASE}}}}}"#);
        assert_eq!(
            parse(&ctrl, "").unwrap().seed.name,
            "Imported theme (imported)"
        );
        let long = format!(r#"{{"name": "{}", "colors": {{{BASE}}}}}"#, "a".repeat(300));
        let imp = parse(&long, "f").unwrap();
        assert_eq!(
            imp.seed.name.chars().count(),
            MAX_NAME + " (imported)".len()
        );
        assert_eq!(imp.seed.id.len(), "imported:".len() + MAX_SLUG);
        let mixed = format!(r#"{{"name": "Night\nOwl\u001b", "colors": {{{BASE}}}}}"#);
        assert_eq!(parse(&mixed, "f").unwrap().seed.name, "NightOwl (imported)");
    }

    #[test]
    fn device_names_never_bare() {
        for (name, want) in [
            ("COM1", "com1-theme"),
            ("con", "con-theme"),
            ("Lpt9", "lpt9-theme"),
            ("nul", "nul-theme"),
            ("aux!", "aux-theme"),
            ("console", "console"),
            ("com10", "com10"),
        ] {
            assert_eq!(slug(name), want, "{name}");
        }
        let src = format!(r#"{{"name": "PRN", "colors": {{{BASE}}}}}"#);
        assert_eq!(parse(&src, "f").unwrap().seed.id, "imported:prn-theme");
    }

    #[test]
    fn invisible_chars_stripped_from_name() {
        let zw = format!(r#"{{"name": "​", "colors": {{{BASE}}}}}"#);
        let imp = parse(&zw, "my-theme").unwrap();
        assert_eq!(imp.seed.name, "my-theme (imported)");
        let bidi = format!(r#"{{"name": "Nord‮", "colors": {{{BASE}}}}}"#);
        assert_eq!(parse(&bidi, "f").unwrap().seed.name, "Nord (imported)");
    }

    #[test]
    fn non_ascii_name_slugs_from_stem() {
        let src = format!(r#"{{"name": "東京の夜", "colors": {{{BASE}}}}}"#);
        let imp = parse(&src, "tokyo-night").unwrap();
        assert_eq!(imp.seed.id, "imported:tokyo-night");
        assert_eq!(imp.seed.name, "東京の夜 (imported)");
        assert_eq!(parse(&src, "東京").unwrap().seed.id, "imported:theme");
    }

    #[test]
    fn line_comment_ends_at_cr() {
        let src = format!("{{\"name\":\"x\"// c\r,\"colors\":{{{BASE}}}}}");
        assert!(strip_jsonc(&src).unwrap().contains("\r,\"colors\""));
        assert_eq!(parse(&src, "f").unwrap().seed.panel, "#1a1b26");
    }

    #[test]
    fn name_cap_counts_chars_not_bytes() {
        let named = |n: usize| format!(r#"{{"name": "{}", "colors": {{{BASE}}}}}"#, "é".repeat(n));
        let want = format!("{} (imported)", "é".repeat(MAX_NAME));
        assert_eq!(parse(&named(MAX_NAME), "f").unwrap().seed.name, want);
        assert_eq!(parse(&named(MAX_NAME + 1), "f").unwrap().seed.name, want);
    }

    #[test]
    fn size_cap_counts_bytes_not_chars() {
        let base = format!(r#"{{"name": "", "colors": {{{BASE}}}}}"#);
        let fill = MAX_BYTES - base.len();
        let name = format!("{}{}", "é".repeat(fill / 2), " ".repeat(fill % 2));
        let exact = format!(r#"{{"name": "{name}", "colors": {{{BASE}}}}}"#);
        assert_eq!(exact.len(), MAX_BYTES);
        assert!(parse(&exact, "f").is_ok());
        let over = format!("{exact} ");
        assert_eq!(over.len(), MAX_BYTES + 1);
        assert!(over.chars().count() < MAX_BYTES);
        assert!(matches!(parse(&over, "f"), Err(ThemeError::TooLarge)));
    }

    #[test]
    fn tokyo_night_shaped_fixture_is_readable() {
        let src = r##"{
  "$schema": "vscode://schemas/color-theme",
  "name": "Tokyo Night",
  "type": "dark",
  // workbench colours
  "colors": {
    "editor.background": "#1a1b26",
    "editor.foreground": "#a9b1d6",
    "sideBar.background": "#16161e",
    "sideBar.border": "#101014",
    "descriptionForeground": "#515670",
    "button.background": "#3d59a1dd",
    "focusBorder": "#545c7e33",
    "textLink.foreground": "#6183bb",
    "list.activeSelectionBackground": "#202330",
  },
  "tokenColors": [
    { "scope": ["comment"], "settings": { "foreground": "#51597d", "fontStyle": "italic" } },
  ],
}"##;
        let imp = parse(src, "tokyo-night").unwrap();
        assert_eq!(imp.seed.id, "imported:tokyo-night");
        assert_eq!(imp.seed.name, "Tokyo Night (imported)");
        assert_eq!(imp.seed.source, Source::Imported);
        assert!(imp.rows.iter().all(|r| r.used));
        let r = imp.resolved.ratios;
        for v in [
            r.text,
            r.muted,
            r.accent_text,
            r.on_accent,
            r.sel_text,
            r.muted_bg,
            r.muted_sel,
        ] {
            assert!(v >= 4.5, "{v}");
        }
        // The dim descriptionForeground must have been raised.
        assert!(
            imp.resolved
                .adjusted
                .iter()
                .any(|a| a.starts_with("muted "))
        );
    }
}
