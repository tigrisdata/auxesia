use auxesia::content::build;
use std::fs;
use tempfile::TempDir;

fn put(root: &TempDir, path: &str, text: &str) {
    let dest = root.path().join(path);
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    fs::write(dest, text).unwrap();
}

#[test]
fn collects_all_page_errors_and_skips_ignored_documents() {
    let root = TempDir::new().unwrap();
    put(&root, ".auxesia.yaml", "ignore: [plans/**]\n");
    put(&root, "one.md", "# Missing frontmatter");
    put(&root, "two.md", "---\ntitle: ''\n---\nBad");
    put(&root, "plans/draft.md", "No title");
    let failure = build(root.path()).unwrap_err();
    assert!(failure.errors.iter().any(|e| e.contains("one.md")));
    assert!(failure.errors.iter().any(|e| e.contains("two.md")));
    assert!(!failure.errors.iter().any(|e| e.contains("draft.md")));
}

#[test]
fn missing_descriptions_warn_and_source_is_preserved() {
    let root = TempDir::new().unwrap();
    let source = "---\ntitle: Hello\n---\n# Hello\n";
    put(&root, "hello.md", source);
    let site = build(root.path()).unwrap();
    assert!(
        site.warnings
            .iter()
            .any(|w| w.contains("hello.md") && w.contains("description"))
    );
    let page = &site.pages["/hello.md"];
    assert_eq!(page.source.as_ref(), source.as_bytes());
    assert_eq!(page.title, "Hello");
}

#[test]
fn rejects_wrong_metadata_types_and_aggregates_config_errors() {
    let root = TempDir::new().unwrap();
    put(&root, ".auxesia.yaml", "stylesheet: missing.css\n");
    put(&root, "number.md", "---\ntitle: 42\n---\nText");
    put(
        &root,
        "boolean.md",
        "---\ntitle: Good\nindex: 'false'\n---\nText",
    );
    put(
        &root,
        "description.md",
        "---\ntitle: Good\ndescription: 42\n---\nText",
    );
    let errors = build(root.path()).unwrap_err().errors;
    for path in [".auxesia.yaml", "number.md", "boolean.md", "description.md"] {
        assert!(
            errors.iter().any(|e| e.contains(path)),
            "missing diagnostic for {path}: {errors:?}"
        );
    }
}

#[test]
fn skill_entrypoint_must_be_a_file() {
    let root = TempDir::new().unwrap();
    put(
        &root,
        "skills/task/SKILL.md/draft.md",
        "Missing frontmatter",
    );
    let failure = build(root.path()).unwrap_err();
    assert!(
        failure
            .errors
            .iter()
            .any(|e| e.contains("SKILL.md") && e.contains("regular file"))
    );
}

#[test]
fn configuration_rejects_non_string_text_fields() {
    for config in [
        "title: 42",
        "subtitle: false",
        "stylesheet: 42",
        "ignore: [42]",
        "llms: {header: 42}",
    ] {
        let root = TempDir::new().unwrap();
        put(&root, ".auxesia.yaml", config);
        assert!(build(root.path()).is_err(), "accepted {config}");
    }
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_but_does_not_validate_ignored_links() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    put(&outside, "secret.md", "Secret");
    std::os::unix::fs::symlink(outside.path(), root.path().join("outside")).unwrap();
    assert!(
        build(root.path())
            .unwrap_err()
            .errors
            .iter()
            .any(|e| e.contains("symlink"))
    );
    put(&root, ".auxesia.yaml", "ignore: [outside]\n");
    assert!(build(root.path()).is_ok());
}
