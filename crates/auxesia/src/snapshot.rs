use crate::{
    content::{self, Capture, Failure},
    render::{self, Snapshot},
};
use std::{path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct ServingState {
    pub snapshot: Arc<Snapshot>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    attempted: Option<[u8; 32]>,
}

pub fn load(root: &Path, previous: Option<&ServingState>) -> Result<ServingState, Failure> {
    load_with(|| content::capture(root), previous)
}

fn load_with(
    mut capture: impl FnMut() -> Capture,
    previous: Option<&ServingState>,
) -> Result<ServingState, Failure> {
    let mut before = capture();
    if let Some(previous) = previous
        && previous.attempted == Some(before.fingerprint)
    {
        return Ok(previous.clone());
    }
    for _ in 0..2 {
        let fingerprint = before.fingerprint;
        let candidate = content::from_capture(before)
            .and_then(|site| render::build_cached(site, previous.map(|p| p.snapshot.as_ref())));
        let after = capture();
        if after.fingerprint == fingerprint {
            return match candidate {
                Ok(snapshot) => Ok(ServingState {
                    warnings: snapshot.warnings.clone(),
                    snapshot: Arc::new(snapshot),
                    errors: vec![],
                    attempted: Some(fingerprint),
                }),
                Err(failure) => retain(previous, failure, Some(fingerprint)),
            };
        }
        before = after;
    }
    retain(previous, Failure { errors: vec!["The documentation changed during validation twice. Finish the edit and reload to retry.".into()], warnings: vec![] }, None)
}

fn retain(
    previous: Option<&ServingState>,
    failure: Failure,
    attempted: Option<[u8; 32]>,
) -> Result<ServingState, Failure> {
    match previous {
        Some(previous) => Ok(ServingState {
            snapshot: previous.snapshot.clone(),
            errors: failure.errors,
            warnings: failure.warnings,
            attempted,
        }),
        None => Err(failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn retries_changed_capture_and_never_publishes_unstable_inputs() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("index.md");
        fs::write(&file, "---\ntitle: Original\n---\nOriginal").unwrap();
        let original = load(root.path(), None).unwrap();
        let mut scans = 0;
        let stable = load_with(
            || {
                let captured = content::capture(root.path());
                scans += 1;
                if scans == 1 {
                    fs::write(&file, "---\ntitle: Changed\n---\nChanged").unwrap();
                }
                captured
            },
            None,
        )
        .unwrap();
        assert_eq!(scans, 3);
        let render::Route::Page { html, .. } = &stable.snapshot.routes["/"] else {
            panic!("page");
        };
        assert!(html.contains("Changed"));
        let mut revision = 0;
        let unstable = load_with(
            || {
                revision += 1;
                fs::write(
                    &file,
                    format!("---\ntitle: Revision {revision}\n---\nRevision {revision}"),
                )
                .unwrap();
                content::capture(root.path())
            },
            Some(&original),
        )
        .unwrap();
        assert!(Arc::ptr_eq(&unstable.snapshot, &original.snapshot));
        assert!(!unstable.errors.is_empty());
        assert_eq!(revision, 3);
    }

    #[test]
    fn unchanged_skill_archive_is_reused_after_unrelated_edit() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("skills")).unwrap();
        fs::write(
            root.path().join("skills/task.md"),
            "---\nname: Task\n---\nTask",
        )
        .unwrap();
        let original = load(root.path(), None).unwrap();
        fs::write(root.path().join("new.md"), "---\ntitle: New\n---\nNew").unwrap();
        let changed = load(root.path(), Some(&original)).unwrap();
        assert!(Arc::ptr_eq(
            &original.snapshot.archives["task"].bytes,
            &changed.snapshot.archives["task"].bytes
        ));
    }
}
