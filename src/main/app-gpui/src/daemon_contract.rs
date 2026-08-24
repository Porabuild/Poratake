//! Checks every daemon call this shell makes against the shared contract.
//!
//! `src/types/daemon.ts` declares `DAEMON_METHODS` once, and
//! `tests/unit/daemon-module-parity.test.ts` already fails when either daemon
//! drifts from it. This is the client half: a mistyped module or method here
//! would otherwise fail silently at runtime, on a code path that only a real
//! capture exercises.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        // `src/main/app-gpui` → repository root.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf()
    }

    /// Parses `DAEMON_METHODS` out of `src/types/daemon.ts` so the contract is
    /// read from its single source rather than restated here.
    fn contract() -> BTreeMap<String, Vec<String>> {
        let source = std::fs::read_to_string(repo_root().join("src/types/daemon.ts"))
            .expect("read src/types/daemon.ts");
        let start = source
            .find("DAEMON_METHODS")
            .expect("DAEMON_METHODS declaration");
        let body = &source[start..];
        let end = body.find("} as const;").expect("end of DAEMON_METHODS");
        let body = &body[..end];

        let mut methods = BTreeMap::new();
        let mut module: Option<String> = None;
        let mut pending: Vec<String> = Vec::new();

        for raw in body.lines().skip(1) {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((name, rest)) = line.split_once(':') {
                if let Some(previous) = module.take() {
                    methods.insert(previous, std::mem::take(&mut pending));
                }
                module = Some(name.trim().trim_matches('\'').trim_matches('"').to_string());
                pending.extend(quoted(rest));
                if rest.contains(']') {
                    if let Some(name) = module.take() {
                        methods.insert(name, std::mem::take(&mut pending));
                    }
                }
                continue;
            }
            pending.extend(quoted(line));
            if line.contains(']') {
                if let Some(name) = module.take() {
                    methods.insert(name, std::mem::take(&mut pending));
                }
            }
        }
        if let Some(name) = module {
            methods.insert(name, pending);
        }

        assert!(
            methods.len() > 10,
            "parsed too few modules: {:?}",
            methods.keys().collect::<Vec<_>>()
        );
        methods
    }

    fn quoted(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('\'') {
            rest = &rest[open + 1..];
            let Some(close) = rest.find('\'') else { break };
            out.push(rest[..close].to_string());
            rest = &rest[close + 1..];
        }
        out
    }

    /// Every `daemon.call("module", "method", …)` in this crate, with the file
    /// and line so a failure names the offender.
    fn calls() -> Vec<(String, String, String)> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }

        let mut files = Vec::new();
        walk(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
            &mut files,
        );

        let mut found = Vec::new();
        for path in files {
            if path
                .file_name()
                .is_some_and(|name| name == "daemon_contract.rs" || name == "daemon.rs")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read source");
            let lines: Vec<&str> = source.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let marker = if line.contains(".call_with_timeout(") {
                    ".call_with_timeout("
                } else {
                    ".call("
                };
                let receiver = lines[index.saturating_sub(3)..=index].concat();
                if !receiver.contains("daemon") {
                    continue;
                }
                let Some(rest) = line.split_once(marker).map(|(_, rest)| rest) else {
                    continue;
                };
                let args = quoted_double(rest);
                if args.len() < 2 {
                    let joined = lines
                        .iter()
                        .skip(index)
                        .take(8)
                        .copied()
                        .collect::<String>();
                    let args = quoted_double(&joined);
                    if args.len() >= 2 {
                        found.push((
                            args[0].clone(),
                            args[1].clone(),
                            format!("{}:{}", path.display(), index + 1),
                        ));
                    } else {
                        found.push((
                            String::new(),
                            String::new(),
                            format!("{}:{}", path.display(), index + 1),
                        ));
                    }
                    continue;
                }
                found.push((
                    args[0].clone(),
                    args[1].clone(),
                    format!("{}:{}", path.display(), index + 1),
                ));
            }
        }
        found
    }

    fn quoted_double(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('"') {
            rest = &rest[open + 1..];
            let Some(close) = rest.find('"') else { break };
            out.push(rest[..close].to_string());
            rest = &rest[close + 1..];
        }
        out
    }

    #[test]
    fn every_daemon_call_exists_in_the_shared_contract() {
        let contract = contract();
        let calls = calls();
        assert!(
            calls.len() >= 15,
            "expected to find the daemon calls, found {}",
            calls.len()
        );

        let mut offenders = Vec::new();
        for (module, method, location) in &calls {
            match contract.get(module) {
                None => offenders.push(format!("{location}: unknown module `{module}`")),
                Some(methods) if !methods.contains(method) => offenders.push(format!(
                    "{location}: `{module}` has no method `{method}` (has {methods:?})"
                )),
                Some(_) => {}
            }
        }
        assert!(
            offenders.is_empty(),
            "daemon calls that do not match `DAEMON_METHODS` in \
             src/types/daemon.ts:\n  {}",
            offenders.join("\n  ")
        );
    }

    /// The freeze is what makes the "Freeze screen" setting mean anything:
    /// nothing else populates the daemon's retained frames, so `capture-area`
    /// with `cached: true` would silently fall back to a live capture.
    #[test]
    fn the_shell_drives_the_freeze_lifecycle() {
        let calls = calls();
        let freeze: Vec<&str> = calls
            .iter()
            .filter(|(module, _, _)| module == "freeze-screen")
            .map(|(_, method, _)| method.as_str())
            .collect();
        for method in ["freeze", "release", "prewarm"] {
            assert!(
                freeze.contains(&method),
                "the shell never calls `freeze-screen {method}`; found {freeze:?}"
            );
        }
    }
}
