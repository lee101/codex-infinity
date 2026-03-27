use dirs::home_dir;
use std::path::PathBuf;

const CODEXINFINITY_DIR: &str = ".codexinfinity";
const LEGACY_CODEX_DIR: &str = ".codex";

/// Auth/credential files to symlink from legacy .codex dir.
const AUTH_FILES: &[&str] = &["auth.json", ".credentials.json"];

/// Returns the path to the Codex Infinity configuration directory.
///
/// Priority: `CODEX_INFINITY_HOME` > `CODEX_HOME` > `~/.codexinfinity`
///
/// On first use, auto-migrates config.toml from `~/.codex/` and symlinks
/// auth files so credentials are shared without re-login.
pub fn find_codex_home() -> std::io::Result<PathBuf> {
    let infinity_env = std::env::var("CODEX_INFINITY_HOME")
        .ok()
        .filter(|val| !val.is_empty());
    let codex_env = std::env::var("CODEX_HOME")
        .ok()
        .filter(|val| !val.is_empty());
    let env_val = infinity_env.as_deref().or(codex_env.as_deref());
    find_codex_home_from_env(env_val)
}

fn find_codex_home_from_env(codex_home_env: Option<&str>) -> std::io::Result<PathBuf> {
    match codex_home_env {
        Some(val) => {
            let path = PathBuf::from(val);
            let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("CODEX_HOME/CODEX_INFINITY_HOME points to {val:?}, but that path does not exist"),
                ),
                _ => std::io::Error::new(
                    err.kind(),
                    format!("failed to read CODEX_HOME/CODEX_INFINITY_HOME {val:?}: {err}"),
                ),
            })?;

            if !metadata.is_dir() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "CODEX_HOME/CODEX_INFINITY_HOME points to {val:?}, but that path is not a directory"
                    ),
                ))
            } else {
                path.canonicalize().map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!(
                            "failed to canonicalize CODEX_HOME/CODEX_INFINITY_HOME {val:?}: {err}"
                        ),
                    )
                })
            }
        }
        None => {
            let home = home_dir().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory",
                )
            })?;
            let new_dir = home.join(CODEXINFINITY_DIR);

            if !new_dir.exists() {
                let legacy_dir = home.join(LEGACY_CODEX_DIR);
                if legacy_dir.is_dir() {
                    auto_migrate(&legacy_dir, &new_dir)?;
                }
            }

            Ok(new_dir)
        }
    }
}

/// Returns the legacy ~/.codex path (for fallback auth lookups).
pub fn find_legacy_codex_home() -> std::io::Result<PathBuf> {
    let mut p = home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;
    p.push(LEGACY_CODEX_DIR);
    Ok(p)
}

/// Migrate from ~/.codex to ~/.codexinfinity:
/// - Copy config.toml (so we get our own independent copy)
/// - Symlink auth files (shared credentials)
fn auto_migrate(legacy: &std::path::Path, new: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(new)?;

    // Copy config.toml
    let legacy_config = legacy.join("config.toml");
    if legacy_config.is_file() {
        let new_config = new.join("config.toml");
        if let Err(e) = std::fs::copy(&legacy_config, &new_config) {
            eprintln!("WARNING: failed to migrate config.toml: {e}");
        }
    }

    // Symlink auth files
    #[cfg(unix)]
    for filename in AUTH_FILES {
        let src = legacy.join(filename);
        if src.exists() {
            let dst = new.join(filename);
            if !dst.exists() {
                if let Err(e) = std::os::unix::fs::symlink(&src, &dst) {
                    eprintln!("WARNING: failed to symlink {filename}: {e}");
                }
            }
        }
    }

    #[cfg(windows)]
    for filename in AUTH_FILES {
        let src = legacy.join(filename);
        if src.is_file() {
            let dst = new.join(filename);
            if !dst.exists() {
                if let Err(e) = std::fs::copy(&src, &dst) {
                    eprintln!("WARNING: failed to copy {filename}: {e}");
                }
            }
        }
    }

    eprintln!("Migrated config from {LEGACY_CODEX_DIR}/ to {CODEXINFINITY_DIR}/");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::find_codex_home_from_env;
    use dirs::home_dir;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[test]
    fn find_codex_home_env_missing_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let missing = temp_home.path().join("missing-codex-home");
        let missing_str = missing
            .to_str()
            .expect("missing codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(missing_str)).expect_err("missing CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn find_codex_home_env_file_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let file_path = temp_home.path().join("codex-home.txt");
        fs::write(&file_path, "not a directory").expect("write temp file");
        let file_str = file_path
            .to_str()
            .expect("file codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(file_str)).expect_err("file CODEX_HOME");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_valid_directory_canonicalizes() {
        let temp_home = TempDir::new().expect("temp home");
        let temp_str = temp_home
            .path()
            .to_str()
            .expect("temp codex home path should be valid utf-8");

        let resolved = find_codex_home_from_env(Some(temp_str)).expect("valid CODEX_HOME");
        let expected = temp_home
            .path()
            .canonicalize()
            .expect("canonicalize temp home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_codexinfinity() {
        let resolved = find_codex_home_from_env(None).expect("default CODEX_HOME");
        let mut expected = home_dir().expect("home dir");
        expected.push(".codexinfinity");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn auto_migrate_copies_config_and_symlinks_auth() {
        let tmp = TempDir::new().expect("temp");
        let legacy = tmp.path().join("legacy");
        let new = tmp.path().join("new");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("config.toml"), "test = true").unwrap();
        fs::write(legacy.join("auth.json"), r#"{"key":"val"}"#).unwrap();

        super::auto_migrate(&legacy, &new).unwrap();

        assert!(new.join("config.toml").is_file());
        assert_eq!(
            fs::read_to_string(new.join("config.toml")).unwrap(),
            "test = true"
        );

        #[cfg(unix)]
        {
            assert!(new.join("auth.json").is_symlink());
            assert_eq!(
                fs::read_to_string(new.join("auth.json")).unwrap(),
                r#"{"key":"val"}"#
            );
        }
    }
}
