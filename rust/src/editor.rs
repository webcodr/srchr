use std::process::{Command, ExitStatus};

/// Rewrite paths that begin with `+` or `-` to `./...` so editors don't treat
/// them as commands/options.
pub fn normalize_path(path: &str) -> String {
    if path.starts_with('+') || path.starts_with('-') {
        format!("./{path}")
    } else {
        path.to_string()
    }
}

/// Build the argument vector for the editor. Content hits jump to `+line`.
pub fn editor_args(path: &str, line: Option<usize>) -> Vec<String> {
    let safe = normalize_path(path);
    match line {
        Some(n) => vec![format!("+{n}"), safe],
        None => vec![safe],
    }
}

/// Resolve `$EDITOR`; error (rather than guess) if unset or empty.
pub fn resolve_editor() -> Result<String, String> {
    match std::env::var("EDITOR") {
        Ok(e) if !e.trim().is_empty() => Ok(e),
        _ => Err("$EDITOR is not set".to_string()),
    }
}

/// Spawn the editor as a foreground child inheriting stdio (it owns the tty),
/// and wait for it to exit. The terminal must already be restored by the caller.
pub fn launch(editor: &str, args: &[String]) -> std::io::Result<ExitStatus> {
    Command::new(editor).args(args).status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_leaves_plain_path() {
        assert_eq!(normalize_path("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn normalize_guards_leading_plus() {
        assert_eq!(normalize_path("+weird.rs"), "./+weird.rs");
    }

    #[test]
    fn normalize_guards_leading_dash() {
        assert_eq!(normalize_path("-weird.rs"), "./-weird.rs");
    }

    #[test]
    fn args_with_line_prepend_plus_line() {
        assert_eq!(
            editor_args("src/main.rs", Some(42)),
            vec!["+42", "src/main.rs"]
        );
    }

    #[test]
    fn args_without_line_just_path() {
        assert_eq!(editor_args("src/main.rs", None), vec!["src/main.rs"]);
    }

    #[test]
    fn args_apply_path_guard() {
        assert_eq!(editor_args("-weird.rs", Some(3)), vec!["+3", "./-weird.rs"]);
    }

    #[test]
    fn launch_invokes_editor_with_args() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("args.txt");
        let script = dir.path().join("fakeed.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n", out.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let status = launch(script.to_str().unwrap(), &editor_args("file.rs", Some(7))).unwrap();
        assert!(status.success());

        let mut s = String::new();
        std::fs::File::open(&out)
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert_eq!(s, "+7\nfile.rs\n");
    }
}
