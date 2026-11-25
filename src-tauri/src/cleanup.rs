//! Ownership-scoped Zana data cleanup.
//!
//! Cleanup moves complete, exact application-owned directory roots to the user's
//! recoverable Trash. It deliberately does not walk shared caches: a model or
//! package in a shared cache cannot be proven to belong only to Zana.

use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const TAURI_APP_IDENTIFIER: &str = "app.zana";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedTarget {
    label: String,
    parent: PathBuf,
    leaf: String,
}

impl OwnedTarget {
    /// Build a target that is exactly one named child of an ownership root.
    pub fn child(label: impl Into<String>, parent: PathBuf, leaf: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            parent,
            leaf: leaf.into(),
        }
    }

    /// Build a target from a platform path while requiring the expected final component.
    pub fn exact_directory(
        label: impl Into<String>,
        path: PathBuf,
        expected_leaf: &str,
    ) -> Result<Self, String> {
        let parent = path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "cleanup target has no parent".to_string())?;
        let leaf = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "cleanup target has no valid final component".to_string())?;
        if leaf != expected_leaf {
            return Err(format!(
                "cleanup target {leaf:?} does not match the expected app-owned name {expected_leaf:?}"
            ));
        }
        Ok(Self::child(label, parent, leaf))
    }

    fn path(&self) -> PathBuf {
        self.parent.join(&self.leaf)
    }

    fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct InventoryEntry {
    pub label: String,
    pub path: String,
    pub ownership: String,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct CleanupReport {
    pub moved: Vec<String>,
    pub absent: Vec<String>,
}

/// Build the exact roots Zana owns on the current platform.
///
/// The Tauri paths are supplied by the caller so configured platform paths are
/// respected. The final `app.zana` component is checked before cleanup.
pub fn targets_from_paths(
    zana_config_dir: PathBuf,
    zana_local_data_dir: PathBuf,
    app_config_dir: PathBuf,
    app_data_dir: PathBuf,
    app_local_data_dir: PathBuf,
    app_cache_dir: PathBuf,
    app_log_dir: PathBuf,
) -> Result<Vec<OwnedTarget>, String> {
    let mut targets = vec![
        OwnedTarget::child("Zana application support", zana_config_dir, "Zana"),
        OwnedTarget::child("Zana local data", zana_local_data_dir, "Zana"),
    ];

    for (label, path) in [
        ("Tauri application config", app_config_dir),
        ("Tauri application data", app_data_dir),
        ("Tauri application local data", app_local_data_dir),
        ("Tauri application cache", app_cache_dir),
        ("Zana application logs", app_log_dir),
    ] {
        targets.push(OwnedTarget::exact_directory(
            label,
            path,
            TAURI_APP_IDENTIFIER,
        )?);
    }

    Ok(targets)
}

pub fn inventory(targets: &[OwnedTarget]) -> Vec<InventoryEntry> {
    targets
        .iter()
        .map(|target| InventoryEntry {
            label: target.label.clone(),
            path: target.path().display().to_string(),
            ownership: "Zana-owned; moved as one directory to Trash".to_string(),
        })
        .collect()
}

/// Return the platform's user Trash directory.
pub fn trash_dir() -> Result<PathBuf, String> {
    let home =
        dirs::home_dir().ok_or_else(|| "could not determine the home directory".to_string())?;

    #[cfg(target_os = "macos")]
    let path = home.join(".Trash");
    #[cfg(not(target_os = "macos"))]
    let path = home.join(".local/share/Trash/files");

    Ok(path)
}

/// Move owned roots to a caller-provided Trash fixture or platform Trash.
///
/// This function never recursively deletes. `rename` moves the root directory
/// itself, so nested symlinks are not followed. A symlink at a root or in its
/// ownership parent is rejected before any target is moved.
pub fn move_targets_to_trash(
    targets: &[OwnedTarget],
    trash_root: &Path,
) -> Result<CleanupReport, String> {
    let mut seen = BTreeSet::new();
    let mut present = Vec::new();
    let mut report = CleanupReport::default();

    for target in targets {
        let path = target.path();
        if !seen.insert(path.clone()) {
            continue;
        }

        match fs::symlink_metadata(&path) {
            Ok(_) => {
                validate_target(target, &path)?;
                present.push((target, path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.absent.push(target.label.clone());
            }
            Err(error) => {
                return Err(format!("could not inspect {}: {error}", target.label()));
            }
        }
    }

    if present.is_empty() {
        return Ok(report);
    }

    prepare_trash_root(trash_root)?;

    for (target, path) in present {
        let destination = unique_destination(trash_root, &target.leaf);
        fs::rename(&path, &destination)
            .map_err(|error| format!("could not move {} to Trash: {error}", target.label()))?;
        report.moved.push(target.label.clone());
    }

    Ok(report)
}

fn validate_target(target: &OwnedTarget, path: &Path) -> Result<(), String> {
    if !matches!(
        Path::new(&target.leaf)
            .components()
            .collect::<Vec<_>>()
            .as_slice(),
        [Component::Normal(_)]
    ) {
        return Err(format!(
            "refusing {}: cleanup root name is not a single path component",
            target.label()
        ));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no ownership parent", target.label()))?;
    if parent != target.parent {
        return Err(format!(
            "refusing {}: target is not the exact owned child",
            target.label()
        ));
    }

    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
        format!(
            "could not inspect the ownership parent for {}: {error}",
            target.label()
        )
    })?;
    if parent_metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing {}: ownership parent is a symlink",
            target.label()
        ));
    }
    if !parent_metadata.is_dir() {
        return Err(format!(
            "refusing {}: ownership parent is not a directory",
            target.label()
        ));
    }

    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", target.label()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing {}: cleanup root is a symlink",
            target.label()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "refusing {}: cleanup root is not a directory",
            target.label()
        ));
    }

    Ok(())
}

fn prepare_trash_root(trash_root: &Path) -> Result<(), String> {
    match fs::symlink_metadata(trash_root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("refusing to use a symlink as the Trash directory".to_string())
        }
        Ok(metadata) if !metadata.is_dir() => Err("the Trash path is not a directory".to_string()),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(trash_root)
                .map_err(|error| format!("could not create the Trash directory: {error}"))
        }
        Err(error) => Err(format!("could not inspect the Trash directory: {error}")),
    }
}

fn unique_destination(trash_root: &Path, leaf: &str) -> PathBuf {
    let base = format!("{leaf} (Zana data)");
    let first = trash_root.join(&base);
    if fs::symlink_metadata(&first).is_err() {
        return first;
    }

    for index in 1.. {
        let candidate = trash_root.join(format!("{base} {index}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }

    unreachable!("the finite filesystem cannot exhaust all destination names")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn moves_owned_roots_to_fixture_trash_and_leaves_siblings() {
        let fixture = tempdir().unwrap();
        let support = fixture.path().join("Application Support");
        let zana = support.join("Zana");
        let other_app = support.join("OtherApp");
        let shared_cache = fixture.path().join("shared-cache");
        let trash = fixture.path().join("Trash");
        fs::create_dir_all(zana.join("voicebot")).unwrap();
        fs::create_dir_all(&other_app).unwrap();
        fs::create_dir_all(&shared_cache).unwrap();
        fs::write(zana.join("voicebot/transcript.md"), "fixture only").unwrap();
        fs::write(other_app.join("keep.txt"), "other app").unwrap();
        fs::write(shared_cache.join("keep.txt"), "shared").unwrap();

        let target = OwnedTarget::child("Zana application support", support, "Zana");
        let report = move_targets_to_trash(&[target], &trash).unwrap();

        assert_eq!(report.moved, vec!["Zana application support"]);
        assert!(!zana.exists());
        assert!(other_app.join("keep.txt").exists());
        assert!(shared_cache.join("keep.txt").exists());
        assert!(trash
            .join("Zana (Zana data)/voicebot/transcript.md")
            .exists());
    }

    #[test]
    fn keeps_existing_trash_entries_and_uses_a_unique_destination() {
        let fixture = tempdir().unwrap();
        let support = fixture.path().join("support");
        let zana = support.join("Zana");
        let trash = fixture.path().join("Trash");
        fs::create_dir_all(&zana).unwrap();
        fs::create_dir_all(trash.join("Zana (Zana data)")).unwrap();
        fs::write(trash.join("Zana (Zana data)/keep.txt"), "keep").unwrap();

        let target = OwnedTarget::child("Zana application support", support, "Zana");
        move_targets_to_trash(&[target], &trash).unwrap();

        assert_eq!(
            fs::read_to_string(trash.join("Zana (Zana data)/keep.txt")).unwrap(),
            "keep"
        );
        assert!(trash.join("Zana (Zana data) 1").is_dir());
    }

    #[test]
    fn absent_roots_are_reported_without_creating_trash() {
        let fixture = tempdir().unwrap();
        let target = OwnedTarget::child("missing", fixture.path().join("support"), "Zana");
        let trash = fixture.path().join("Trash");

        let report = move_targets_to_trash(&[target], &trash).unwrap();

        assert_eq!(report.absent, vec!["missing"]);
        assert!(!trash.exists());
    }

    #[test]
    fn inventory_uses_only_exact_zana_and_tauri_app_roots() {
        let fixture = tempdir().unwrap();
        let zana_config = fixture.path().join("Application Support");
        let zana_local = fixture.path().join("Local Application Support");
        let app_config = zana_config.join(TAURI_APP_IDENTIFIER);
        let app_data = fixture.path().join("Data").join(TAURI_APP_IDENTIFIER);
        let app_local = fixture.path().join("Local").join(TAURI_APP_IDENTIFIER);
        let app_cache = fixture.path().join("Caches").join(TAURI_APP_IDENTIFIER);
        let app_logs = fixture.path().join("Logs").join(TAURI_APP_IDENTIFIER);

        let targets = targets_from_paths(
            zana_config,
            zana_local,
            app_config,
            app_data,
            app_local,
            app_cache,
            app_logs,
        )
        .unwrap();
        let entries = inventory(&targets);

        assert_eq!(entries.len(), 7);
        assert!(entries
            .iter()
            .any(|entry| Path::new(&entry.path).ends_with("Zana")));
        assert!(entries
            .iter()
            .filter(|entry| entry.label.starts_with("Tauri") || entry.label.contains("logs"))
            .all(|entry| Path::new(&entry.path).ends_with(TAURI_APP_IDENTIFIER)));
        assert!(entries
            .iter()
            .all(|entry| !entry.path.contains("huggingface") && !entry.path.contains("uv")));

        let wrong_app = fixture.path().join("Data").join("other-app");
        assert!(targets_from_paths(
            fixture.path().join("Application Support"),
            fixture.path().join("Local Application Support"),
            fixture.path().join("Config").join(TAURI_APP_IDENTIFIER),
            wrong_app,
            fixture.path().join("Local").join(TAURI_APP_IDENTIFIER),
            fixture.path().join("Caches").join(TAURI_APP_IDENTIFIER),
            fixture.path().join("Logs").join(TAURI_APP_IDENTIFIER),
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_cleanup_root_without_touching_its_target() {
        let fixture = tempdir().unwrap();
        let support = fixture.path().join("support");
        let outside = fixture.path().join("outside");
        let trash = fixture.path().join("Trash");
        fs::create_dir_all(&support).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("must-survive.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(&outside, support.join("Zana")).unwrap();

        let target = OwnedTarget::child("Zana application support", support.clone(), "Zana");
        let error = move_targets_to_trash(&[target], &trash).unwrap_err();

        assert!(error.contains("symlink"));
        assert!(outside.join("must-survive.txt").exists());
        assert!(support.join("Zana").exists());
        assert!(!trash.exists());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_ownership_parent_without_touching_outside() {
        let fixture = tempdir().unwrap();
        let parent_link = fixture.path().join("support-link");
        let outside = fixture.path().join("outside");
        let trash = fixture.path().join("Trash");
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir_all(outside.join("Zana")).unwrap();
        fs::write(outside.join("Zana/must-survive.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(&outside, &parent_link).unwrap();

        let target = OwnedTarget::child("Zana application support", parent_link, "Zana");
        let error = move_targets_to_trash(&[target], &trash).unwrap_err();

        assert!(error.contains("ownership parent is a symlink"));
        assert!(outside.join("Zana/must-survive.txt").exists());
        assert!(!trash.exists());
    }

    #[cfg(unix)]
    #[test]
    fn moving_a_root_does_not_follow_nested_symlinks() {
        let fixture = tempdir().unwrap();
        let support = fixture.path().join("support");
        let zana = support.join("Zana");
        let outside = fixture.path().join("outside");
        let trash = fixture.path().join("Trash");
        fs::create_dir_all(&zana).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("must-survive.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(&outside, zana.join("linked")).unwrap();

        let target = OwnedTarget::child("Zana application support", support, "Zana");
        move_targets_to_trash(&[target], &trash).unwrap();

        assert!(outside.join("must-survive.txt").exists());
        assert!(fs::symlink_metadata(trash.join("Zana (Zana data)/linked"))
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
