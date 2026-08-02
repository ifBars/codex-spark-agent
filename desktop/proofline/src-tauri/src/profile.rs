use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub(crate) const PROFILE_ROOT_ENV: &str = "SPARK_PROOFLINE_PROFILE_ROOT";
const MAIN_WEBVIEW_DIRECTORY: &str = "main-webview2";

/// Returns a calibration-only WebView2 user-data directory.
///
/// Production leaves this unset and therefore retains Tauri's normal platform
/// data-directory behavior. Calibration roots must already exist so a typo can
/// never silently create or fall back to a user profile directory.
pub(crate) fn calibration_profile_directory() -> Result<Option<PathBuf>, String> {
    let root = env::var_os(PROFILE_ROOT_ENV).map(PathBuf::from);
    calibration_profile_directory_from_root(root)
}

fn calibration_profile_directory_from_root(
    root: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let Some(root) = root else {
        return Ok(None);
    };
    if !root.is_absolute() {
        return Err("calibration profile root must be an absolute directory".into());
    }
    let metadata =
        fs::metadata(&root).map_err(|_| "calibration profile root is unavailable".to_owned())?;
    if !metadata.is_dir() {
        return Err("calibration profile root must be a directory".into());
    }
    let root = fs::canonicalize(root)
        .map_err(|_| "calibration profile root could not be resolved".to_owned())?;
    let data_directory = root.join(MAIN_WEBVIEW_DIRECTORY);
    if data_directory.exists() {
        if !data_directory.is_dir() {
            return Err("calibration main webview path must be a directory".into());
        }
    } else {
        fs::create_dir(&data_directory)
            .map_err(|_| "calibration main webview directory could not be created".to_owned())?;
    }
    let data_directory = fs::canonicalize(data_directory)
        .map_err(|_| "calibration main webview directory could not be resolved".to_owned())?;
    if !is_strict_descendant(&data_directory, &root) {
        return Err("calibration main webview directory escapes its profile root".into());
    }
    Ok(Some(data_directory))
}

fn is_strict_descendant(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::{calibration_profile_directory_from_root, MAIN_WEBVIEW_DIRECTORY};
    use std::{fs, path::PathBuf};
    use tempfile::TempDir;

    #[test]
    fn absent_profile_root_preserves_normal_platform_behavior() {
        assert_eq!(
            calibration_profile_directory_from_root(None).expect("no root"),
            None
        );
    }

    #[test]
    fn valid_existing_root_creates_and_returns_an_exact_absolute_child_directory() {
        let directory = TempDir::new().expect("temporary profile root");
        let root = fs::canonicalize(directory.path()).expect("canonical root");
        let data_directory = calibration_profile_directory_from_root(Some(root.clone()))
            .expect("isolated profile directory")
            .expect("configured directory");
        assert!(data_directory.is_absolute());
        assert!(data_directory.is_dir());
        assert_eq!(data_directory, root.join(MAIN_WEBVIEW_DIRECTORY));
        assert!(data_directory.starts_with(root));
    }

    #[test]
    fn relative_missing_and_file_roots_fail_closed() {
        assert!(
            calibration_profile_directory_from_root(Some(PathBuf::from("relative-profile")))
                .is_err()
        );
        let directory = TempDir::new().expect("temporary parent");
        assert!(
            calibration_profile_directory_from_root(Some(directory.path().join("missing")))
                .is_err()
        );
        let file = directory.path().join("not-a-directory");
        fs::write(&file, "not a profile directory").expect("profile file");
        assert!(calibration_profile_directory_from_root(Some(file)).is_err());
    }

    #[test]
    fn existing_main_webview_file_fails_closed() {
        let directory = TempDir::new().expect("temporary profile root");
        fs::write(
            directory.path().join(MAIN_WEBVIEW_DIRECTORY),
            "not a directory",
        )
        .expect("main webview file");
        assert!(
            calibration_profile_directory_from_root(Some(directory.path().to_path_buf())).is_err()
        );
    }
}
