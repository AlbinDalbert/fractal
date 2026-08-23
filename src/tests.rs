use crate::{IframeTarget, LinkTarget, MatchKind, PageKind, Project};
use std::fs;
use tempfile::TempDir;

fn project() -> (TempDir, Project) {
    let temp = TempDir::new().unwrap();
    let project = Project::init(temp.path(), "Test").unwrap();
    (temp, project)
}

fn native(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta name=\"fractal-format\" content=\"1\"><title>{title}</title></head><body><main data-fractal-document><h1>{title}</h1>{body}</main></body></html>"
    )
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
fn legacy_manifest_opens_without_rewriting_project_files() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("pages")).unwrap();
    let manifest = r#"{
  "project_name": "Legacy project",
  "version": 1,
  "default_page": "pages/index.html",
  "theme": "dark"
}"#;
    let page = "<!doctype html><title>Legacy page</title><main><h1>Legacy page</h1></main>";
    fs::write(temp.path().join("fractal.json"), manifest).unwrap();
    fs::write(temp.path().join("pages/index.html"), page).unwrap();

    let project = Project::open(temp.path()).unwrap();

    assert_eq!(project.manifest().name, "Legacy project");
    assert_eq!(project.page("index").unwrap().kind, PageKind::Raw);
    assert_eq!(project.source("index").unwrap(), page);
    assert_eq!(
        fs::read_to_string(temp.path().join("fractal.json")).unwrap(),
        manifest
    );
}

#[test]
fn ordinary_html_and_explicit_links_work() {
    let (_temp, mut project) = project();
    project.create_page("Stockholm").unwrap();
    project.create_page("Travel").unwrap();
    project
        .write_page(
            "travel",
            &native(
                "Travel",
                "<p>Visit <strong><a href=\"stockholm.fractal.html\">Stockholm</a></strong> or <a href=\"https://example.com\">the web</a>.</p>",
            ),
        )
        .unwrap();

    let links = project.links("travel").unwrap();
    assert!(
        matches!(&links[0].target, LinkTarget::Internal(path) if path == "stockholm.fractal.html")
    );
    assert!(matches!(&links[1].target, LinkTarget::External(url) if url == "https://example.com"));
    assert_eq!(
        project.backlinks("stockholm").unwrap()[0].page,
        "travel.fractal.html"
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
            &native("Sweden", "<p>Stockholm is the capital.</p>"),
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
        .write_page("sweden", &native("Sweden", "<p>Visit Stockholm.</p>"))
        .unwrap();
    project
        .insert_link("sweden", "Stockholm", "stockholm")
        .unwrap();
    assert!(project
        .source("sweden")
        .unwrap()
        .contains("<a href=\"stockholm.fractal.html\">Stockholm</a>"));
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
            &native(
                "Sweden",
                "<a href=\"../stockholm.fractal.html\">Stockholm</a>",
            ),
        )
        .unwrap();
    project
        .write_page(
            "stockholm",
            &native(
                "Stockholm",
                "<a href=\"trips/sweden.fractal.html\">Sweden</a>",
            ),
        )
        .unwrap();
    project.move_page("stockholm", "places/stockholm").unwrap();
    assert!(project
        .source("trips/sweden")
        .unwrap()
        .contains("../places/stockholm.fractal.html"));
    assert!(project
        .source("places/stockholm")
        .unwrap()
        .contains("../trips/sweden.fractal.html"));
    assert_eq!(project.backlinks("places/stockholm").unwrap().len(), 1);
}

#[test]
fn validation_reports_missing_titles_and_broken_links() {
    let (temp, _project) = project();
    fs::write(
        temp.path().join("pages/broken.fractal.html"),
        "<!doctype html><meta name=\"fractal-format\" content=\"1\"><main data-fractal-document><p><a href=\"missing.html\">Missing</a></p></main>",
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
        .write_page("one", &native("One", "<p>The northern lights.</p>"))
        .unwrap();
    assert_eq!(
        project.search("northern lights")[0].path,
        "one.fractal.html"
    );
}

#[test]
fn new_pages_are_native_documents() {
    let (_temp, mut project) = project();
    project.create_page("Native page").unwrap();
    let page = project.page("native-page").unwrap();
    assert_eq!(page.path, "native-page.fractal.html");
    assert_eq!(page.kind, PageKind::Native);
    let source = project.source("native-page").unwrap();
    assert!(source.contains("<meta name=\"fractal-format\" content=\"1\">"));
    assert!(source.contains("<meta name=\"viewport\""));
    assert!(source.contains("background: #0c0c0a"));
    assert!(source.contains("a { color: #e8bb4d"));
    assert!(project.validate().valid);
}

#[test]
fn raw_html_is_readable_without_being_native() {
    let (temp, _project) = project();
    let source = "<article><custom-card>Hand-authored content</custom-card></article>";
    fs::write(temp.path().join("pages/raw.html"), source).unwrap();
    let project = Project::open(temp.path()).unwrap();
    let page = project.page("raw").unwrap();
    assert_eq!(page.kind, PageKind::Raw);
    assert_eq!(page.title, None);
    assert_eq!(project.source("raw").unwrap(), source);
    assert!(project.validate().valid);
}

#[test]
fn explicit_raw_source_write_preserves_the_supplied_bytes() {
    let (temp, _project) = project();
    fs::write(temp.path().join("pages/raw.html"), "<p>Before</p>").unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    let replacement = "  <CUSTOM-ELEMENT data-value='one'>After</CUSTOM-ELEMENT>\n";

    project.write_page("raw", replacement).unwrap();

    assert_eq!(project.source("raw").unwrap(), replacement);
    assert_eq!(
        fs::read_to_string(temp.path().join("pages/raw.html")).unwrap(),
        replacement
    );
}

#[test]
fn semantic_link_insertion_rejects_raw_html() {
    let (temp, _project) = project();
    fs::write(
        temp.path().join("pages/raw.html"),
        "<title>Raw</title><p>Visit Native.</p>",
    )
    .unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project.create_page("Native").unwrap();
    let error = project.insert_link("raw", "Native", "native").unwrap_err();
    assert!(error
        .message
        .contains("only available for native documents"));
}

#[test]
fn moving_a_native_page_does_not_rewrite_raw_html() {
    let (temp, mut project) = project();
    project.create_page("Native").unwrap();
    let raw = "<title>Raw</title><a href=\"native.fractal.html\">Native</a>";
    fs::write(temp.path().join("pages/raw.html"), raw).unwrap();
    project = Project::open(temp.path()).unwrap();
    project.move_page("native", "moved/native").unwrap();
    assert_eq!(project.source("raw").unwrap(), raw);
}

#[test]
fn native_documents_support_standard_text_elements() {
    let (_temp, mut project) = project();
    project.create_page("Elements").unwrap();
    let body = "<p>Text<br><img src=\"image.png\" alt=\"Image\"></p><blockquote>Quote</blockquote><pre><code>let x = 1;</code></pre><table><thead><tr><th>Name</th></tr></thead><tbody><tr><td>Fractal</td></tr></tbody></table>";
    project
        .write_page("elements", &native("Elements", body))
        .unwrap();
    assert!(project.validate().valid);
}

#[test]
fn native_documents_reject_elements_outside_the_profile() {
    let (_temp, mut project) = project();
    project.create_page("Strict").unwrap();
    let error = project
        .write_page("strict", &native("Strict", "<script>alert(1)</script>"))
        .unwrap_err();
    assert!(error.message.contains("unsupported elements: script"));
}

#[test]
fn native_documents_reject_scripts_in_the_head() {
    let (_temp, mut project) = project();
    project.create_page("Strict").unwrap();
    let source = "<!doctype html><html><head><meta name=\"fractal-format\" content=\"1\"><title>Strict</title><script>alert(1)</script></head><body><main data-fractal-document><p>Text</p></main></body></html>";

    let error = project.write_page("strict", source).unwrap_err();

    assert!(error
        .message
        .contains("head contains unsupported elements: script"));
}

#[test]
fn iframe_references_are_typed_and_validated() {
    let (temp, _project) = project();
    fs::write(
        temp.path().join("pages/widget.html"),
        "<style>body { color: tomato }</style><p>Widget</p>",
    )
    .unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project.create_page("Dashboard").unwrap();
    project
        .write_page(
            "dashboard",
            &native(
                "Dashboard",
                "<iframe src=\"widget.html\" title=\"Widget\" sandbox></iframe><iframe srcdoc=\"&lt;p&gt;Inline&lt;/p&gt;\"></iframe><iframe src=\"https://example.com\"></iframe>",
            ),
        )
        .unwrap();

    let page = project.page("dashboard").unwrap();
    assert!(matches!(
        &page.iframes[0].target,
        IframeTarget::Internal(path) if path == "widget.html"
    ));
    assert_eq!(page.iframes[0].sandbox.as_deref(), Some(""));
    assert_eq!(page.iframes[1].target, IframeTarget::Inline);
    assert!(matches!(
        &page.iframes[2].target,
        IframeTarget::External(url) if url == "https://example.com"
    ));
    assert_eq!(
        project.iframe_backlinks("widget").unwrap()[0].page,
        "dashboard.fractal.html"
    );
    assert!(project.validate().valid);
}

#[test]
fn moving_raw_html_updates_native_iframes_without_changing_raw_source() {
    let (temp, _project) = project();
    let raw = "<style>body { color: tomato }</style><p>Widget</p>";
    fs::write(temp.path().join("pages/widget.html"), raw).unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project.create_page("Dashboard").unwrap();
    project
        .write_page(
            "dashboard",
            &native(
                "Dashboard",
                "<iframe src=\"widget.html#preview\" title=\"Widget\"></iframe>",
            ),
        )
        .unwrap();

    project.move_page("widget", "embeds/widget").unwrap();

    assert_eq!(project.source("embeds/widget").unwrap(), raw);
    assert!(project
        .source("dashboard")
        .unwrap()
        .contains("embeds/widget.html#preview"));
    assert!(project.validate().valid);
}

#[test]
fn deleting_an_embedded_page_is_rejected() {
    let (temp, _project) = project();
    fs::write(temp.path().join("pages/widget.html"), "<p>Widget</p>").unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project.create_page("Dashboard").unwrap();
    project
        .write_page(
            "dashboard",
            &native(
                "Dashboard",
                "<iframe src=\"widget.html\" title=\"Widget\"></iframe>",
            ),
        )
        .unwrap();

    let error = project.delete_page("widget").unwrap_err();
    assert!(error.message.contains("1 iframe(s)"));
}

#[test]
fn missing_local_iframe_target_invalidates_a_native_document() {
    let (_temp, mut project) = project();
    project.create_page("Dashboard").unwrap();
    project
        .write_page(
            "dashboard",
            &native("Dashboard", "<iframe src=\"missing.html\"></iframe>"),
        )
        .unwrap();
    let report = project.validate();
    assert!(!report.valid);
    assert!(report.issues[0].message.contains("broken iframe source"));
}

#[test]
fn derived_links_report_exact_title_occurrences_without_writing() {
    let (_temp, mut project) = project();
    project.create_page("Ada Lovelace").unwrap();
    project.create_page("Notes").unwrap();
    project
        .write_page(
            "notes",
            &native(
                "Notes",
                "<p>😀 ADA LOVELACE met Ada Lovelace and Ada LovelaceX.</p><p><a href=\"ada-lovelace.fractal.html\">Ada Lovelace</a> Ada Lovelace.</p>",
            ),
        )
        .unwrap();
    let before = project.source("notes").unwrap();

    let links = project.derived_links("notes").unwrap();

    assert_eq!(links.len(), 3);
    assert_eq!(links[0].text, "ADA LOVELACE");
    assert_eq!(links[0].target, "ada-lovelace.fractal.html");
    assert_eq!(links[0].occurrence.start.text_node, 1);
    assert_eq!(links[0].occurrence.start.offset, 3);
    assert_eq!(links[0].occurrence.end.offset, 15);
    assert_eq!(links[1].text, "Ada Lovelace");
    assert_eq!(links[2].occurrence.start.text_node, 3);
    assert_eq!(project.source("notes").unwrap(), before);
}

#[test]
fn derived_links_skip_ambiguous_titles() {
    let (_temp, mut project) = project();
    project
        .create_page_at("people/ada", "Ada Lovelace")
        .unwrap();
    project
        .create_page_at("scientists/ada", "Ada Lovelace")
        .unwrap();
    project.create_page("Notes").unwrap();
    project
        .write_page("notes", &native("Notes", "<p>Ada Lovelace</p>"))
        .unwrap();

    assert!(project.derived_links("notes").unwrap().is_empty());
}

#[test]
fn derived_links_prefer_the_longest_exact_title() {
    let (_temp, mut project) = project();
    project.create_page("Stockholm").unwrap();
    project.create_page("Stockholm City").unwrap();
    project.create_page("Sweden").unwrap();
    project
        .write_page(
            "sweden",
            &native("Sweden", "<p>Stockholm City and Stockholm.</p>"),
        )
        .unwrap();

    let links = project.derived_links("sweden").unwrap();

    assert_eq!(links.len(), 2);
    assert_eq!(links[0].text, "Stockholm City");
    assert_eq!(links[0].target, "stockholm-city.fractal.html");
    assert_eq!(links[1].text, "Stockholm");
    assert_eq!(links[1].target, "stockholm.fractal.html");
}
