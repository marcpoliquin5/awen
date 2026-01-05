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
