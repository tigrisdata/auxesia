use crate::{
    content::{self, Capture, Failure},
    render::{self, Snapshot},
};
use std::{path::Path, sync::Arc, time::SystemTime};

#[derive(Clone, Debug)]
pub struct ServingState {
    pub snapshot: Arc<Snapshot>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    attempted: Option<[u8; 32]>,
}

pub fn load(root: &Path, previous: Option<&ServingState>) -> Result<ServingState, Failure> {
    // One reference time makes every scan in this load agree on which files are recent.
    let now = SystemTime::now();
    load_with(
        || content::stamp_at(root, now),
        || content::capture_at(root, now),
        previous,
    )
}

// Stamps detect changes from metadata, so only a changed tree has its contents read.
fn load_with(
    mut stamp: impl FnMut() -> [u8; 32],
    mut capture: impl FnMut() -> Capture,
    previous: Option<&ServingState>,
) -> Result<ServingState, Failure> {
    let mut before = stamp();
    if let Some(previous) = previous
        && previous.attempted == Some(before)
    {
        return Ok(previous.clone());
    }
    for _ in 0..2 {
        let fingerprint = before;
        let captured = capture();
        let consistent = captured.fingerprint == fingerprint;
        let candidate = content::from_capture(captured)
            .and_then(|site| render::build_cached(site, previous.map(|p| p.snapshot.as_ref())));
        let after = stamp();
        if consistent && after == fingerprint {
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
    use std::{
        fs,
        time::{Duration, SystemTime},
    };

    #[test]
    fn retries_changed_capture_and_never_publishes_unstable_inputs() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("index.md");
        fs::write(&file, "---\ntitle: Original\n---\nOriginal").unwrap();
        let original = load(root.path(), None).unwrap();
        let now = SystemTime::now();
        let mut captures = 0;
        let stable = load_with(
            || content::stamp_at(root.path(), now),
            || {
                let captured = content::capture_at(root.path(), now);
                captures += 1;
                if captures == 1 {
                    fs::write(&file, "---\ntitle: Changed\n---\nChanged").unwrap();
                }
                captured
            },
            None,
        )
        .unwrap();
        assert_eq!(captures, 2);
        let render::Route::Page { html, .. } = &stable.snapshot.routes["/"] else {
            panic!("page");
        };
        assert!(html.contains("Changed"));
        let mut revision = 0;
        let unstable = load_with(
            || content::stamp_at(root.path(), now),
            || {
                let captured = content::capture_at(root.path(), now);
                revision += 1;
                fs::write(
                    &file,
                    format!("---\ntitle: Revision {revision}\n---\nRevision {revision}"),
                )
                .unwrap();
                captured
            },
            Some(&original),
        )
        .unwrap();
        assert!(Arc::ptr_eq(&unstable.snapshot, &original.snapshot));
        assert!(!unstable.errors.is_empty());
        assert_eq!(revision, 2);
    }

    #[test]
    fn unchanged_tree_is_reused_without_capturing_contents() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("index.md"), "---\ntitle: Home\n---\nHome").unwrap();
        fs::write(root.path().join("image.png"), [0_u8; 4096]).unwrap();
        // Folders make both scans agree on which directories publish a route.
        fs::write(root.path().join(".auxesia.yaml"), "ignore: ['plans/**']\n").unwrap();
        fs::create_dir_all(root.path().join("guide")).unwrap();
        fs::write(
            root.path().join("guide/page.md"),
            "---\ntitle: Page\n---\nPage",
        )
        .unwrap();
        fs::create_dir_all(root.path().join("plans")).unwrap();
        fs::write(root.path().join("plans/draft.md"), "Draft").unwrap();
        // A reference time far after the writes makes every file settled.
        let settled = SystemTime::now() + Duration::from_secs(3600);
        let stamp = || content::stamp_at(root.path(), settled);
        let capture = || content::capture_at(root.path(), settled);
        let original = load_with(stamp, capture, None).unwrap();
        let reused = load_with(
            stamp,
            || panic!("an unchanged tree must not be captured"),
            Some(&original),
        )
        .unwrap();
        assert!(Arc::ptr_eq(&reused.snapshot, &original.snapshot));
        fs::write(root.path().join("new.md"), "---\ntitle: New\n---\nNew").unwrap();
        let mut captures = 0;
        let changed = load_with(
            stamp,
            || {
                captures += 1;
                capture()
            },
            Some(&original),
        )
        .unwrap();
        assert_eq!(captures, 1);
        assert!(changed.snapshot.routes.contains_key("/new.md"));
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
