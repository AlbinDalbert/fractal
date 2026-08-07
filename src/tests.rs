use crate::{LinkTarget, MatchKind, Project};
use std::fs;
use tempfile::TempDir;

fn project() -> (TempDir, Project) {
    let temp = TempDir::new().unwrap();
    let project = Project::init(temp.path(), "Test").unwrap();
    (temp, project)
}

#[test]
fn project_has_no_generated_state() {
    let (temp, project) = project();
    assert!(project.pages().is_empty());
    assert!(temp.path().join("fractal.json").is_file());
    assert!(temp.path().join("pages").is_dir());
    assert!(!temp.path().join(".fractal").exists());
}

#[test]
fn ordinary_html_and_explicit_links_work() {
    let (_temp, mut project) = project();
    project.create_page("Stockholm").unwrap();
    project.create_page("Travel").unwrap();
    project
        .write_page(
            "travel",
            "<!doctype html><title>Travel</title><main><h1>Travel</h1><p>Visit <strong><a href=\"stockholm.html\">Stockholm</a></strong> or <a href=\"https://example.com\">the web</a>.</p></main>",
        )
        .unwrap();

    let links = project.links("travel").unwrap();
    assert!(matches!(&links[0].target, LinkTarget::Internal(path) if path == "stockholm.html"));
    assert!(matches!(&links[1].target, LinkTarget::External(url) if url == "https://example.com"));
    assert_eq!(
        project.backlinks("stockholm").unwrap()[0].page,
        "travel.html"
    );
    assert!(project.validate().valid);
}

#[test]
fn suggestions_group_ambiguous_candidates_without_writing() {
    let (_temp, mut project) = project();
    project.create_page("Stockholm").unwrap();
    project.create_page("Stockholm City").unwrap();
    project.create_page("Sweden").unwrap();
    project
        .write_page(
            "sweden",
            "<title>Sweden</title><h1>Sweden</h1><p>Stockholm is the capital.</p>",
        )
        .unwrap();

    let before = project.source("sweden").unwrap();
    let suggestions = project.suggest_links("sweden").unwrap();
    let stockholm = suggestions
        .iter()
        .find(|suggestion| suggestion.text.eq_ignore_ascii_case("stockholm"))
        .unwrap();
    assert_eq!(stockholm.candidates.len(), 2);
    assert_eq!(stockholm.candidates[0].match_kind, MatchKind::ExactTitle);
    assert_eq!(project.source("sweden").unwrap(), before);
}

#[test]
fn inserting_a_suggested_link_is_explicit() {
    let (_temp, mut project) = project();
    project.create_page("Stockholm").unwrap();
    project.create_page("Sweden").unwrap();
    project
        .write_page("sweden", "<title>Sweden</title><p>Visit Stockholm.</p>")
        .unwrap();
    project
        .insert_link("sweden", "Stockholm", "stockholm")
        .unwrap();
    assert!(project
        .source("sweden")
        .unwrap()
        .contains("<a href=\"stockholm.html\">Stockholm</a>"));
    assert!(project.suggest_links("sweden").unwrap().is_empty());
}

#[test]
fn moving_a_page_updates_explicit_backlinks() {
    let (_temp, mut project) = project();
    project.create_page("Stockholm").unwrap();
    project.create_page_at("trips/sweden", "Sweden").unwrap();
    project
        .write_page(
            "trips/sweden",
            "<title>Sweden</title><a href=\"../stockholm.html\">Stockholm</a>",
        )
        .unwrap();
    project
        .write_page(
            "stockholm",
            "<title>Stockholm</title><a href=\"trips/sweden.html\">Sweden</a>",
        )
        .unwrap();
    project.move_page("stockholm", "places/stockholm").unwrap();
    assert!(project
        .source("trips/sweden")
        .unwrap()
        .contains("../places/stockholm.html"));
    assert!(project
        .source("places/stockholm")
        .unwrap()
        .contains("../trips/sweden.html"));
    assert_eq!(project.backlinks("places/stockholm").unwrap().len(), 1);
}

#[test]
fn validation_reports_missing_titles_and_broken_links() {
    let (temp, _project) = project();
    fs::write(
        temp.path().join("pages/broken.html"),
        "<p><a href=\"missing.html\">Missing</a></p>",
    )
    .unwrap();
    let project = Project::open(temp.path()).unwrap();
    let report = project.validate();
    assert!(!report.valid);
    assert_eq!(report.issues.len(), 2);
}

#[test]
fn search_scans_the_in_memory_catalog() {
    let (_temp, mut project) = project();
    project.create_page("One").unwrap();
    project
        .write_page("one", "<title>One</title><p>The northern lights.</p>")
        .unwrap();
    assert_eq!(project.search("northern lights")[0].path, "one.html");
}
