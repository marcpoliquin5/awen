use anyhow::Result;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// Simple plugin loader that launches plugin binaries as subprocesses and
/// communicates JSON over stdin/stdout. This avoids unsafe dynamic linking and
/// keeps the runtime model simple and auditable.
pub struct PluginLoader;

impl PluginLoader {
    /// Invoke a plugin executable at `path`, sending `input_json` to its stdin.
    /// Returns the stdout of the process as a string if the plugin ran and
    /// produced output. If the path does not exist, returns Ok(None).
    pub fn invoke<P: AsRef<Path>>(path: P, input_json: &str) -> Result<Option<String>> {
        let p = path.as_ref();
        if !p.exists() {
            return Ok(None);
        }

        let mut cmd = Command::new(p);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input_json.as_bytes())?;
        }

        let mut output = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout.read_to_string(&mut output)?;
        }

        let status = child.wait()?;
        if status.success() {
            Ok(Some(output))
        } else {
            Ok(None)
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::PluginLoader;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn script(name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temporary plugin directory");
        let path = dir.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write plugin script");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make plugin executable");
        (dir, path)
    }

    #[test]
    fn missing_plugin_returns_none() {
        let result = PluginLoader::invoke("/nonexistent/awen-plugin", "{}").expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn plugin_receives_stdin_and_returns_stdout() {
        let (_dir, path) = script("echo-plugin", "cat");
        let output = PluginLoader::invoke(&path, r#"{"op":"gemm"}"#)
            .expect("plugin runs")
            .expect("successful plugin returns output");
        assert_eq!(output, r#"{"op":"gemm"}"#);
    }

    #[test]
    fn failing_plugin_returns_none() {
        let (_dir, path) = script("failing-plugin", "cat >/dev/null; exit 3");
        let result = PluginLoader::invoke(&path, "{}").expect("spawn succeeds");
        assert!(result.is_none());
    }
}
