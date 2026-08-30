use crate::{
    FolderChildKind, FolderChildStatus, FolderHtmlExportOptions, FractalErrorCode,
    HtmlExportOptions, IframeTarget, LinkTarget, PageKind, Project,
};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// Existing behavioral tests use complete native documents as fixtures. This
// helper bypasses the public editing API so those fixtures do not weaken the
// section-only production boundary.
trait NativeFixtureWrite {
    fn write_page(&mut self, path: impl AsRef<Path>, html: &str) -> crate::Result<crate::Mutation>;
    fn write_page_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> crate::Result<crate::Mutation>;
}

impl NativeFixtureWrite for Project {
    fn write_page(&mut self, path: impl AsRef<Path>, html: &str) -> crate::Result<crate::Mutation> {
        let page = self.page(path)?;
        let relative = std::path::PathBuf::from(&page.path);
        let original = self.source(&relative)?;
        fs::write(self.root().join("pages").join(&relative), html)?;
        *self = Project::open(self.root())?;
        if let Some(issue) = self.validate().issues.into_iter().find(|issue| {
            issue.path.as_deref() == Some(page.path.as_str())
                && (issue.message.contains("unsupported elements")
                    || issue.message.contains("needs exactly one"))
        }) {
            fs::write(self.root().join("pages").join(&relative), original)?;
            *self = Project::open(self.root())?;
            return Err(crate::FractalError::invalid_input(format!(
                "invalid native document: {}",
                issue.message
            )));
        }
        Ok(crate::Mutation {
            changed: vec![relative],
            deleted: vec![],
        })
    }

    fn write_page_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> crate::Result<crate::Mutation> {
        let current = Project::open(self.root())?;
        let page = current.page(path.as_ref())?;
        *self = current;
        if page.content_hash != expected_hash {
            return Err(crate::FractalError::conflict(
                "page changed since it was read",
            ));
        }
        self.write_page(path, html)
    }
}

fn project() -> (TempDir, Project) {
    let temp = TempDir::new().unwrap();
    let project = Project::init(temp.path(), "Test").unwrap();
    (temp, project)
}

fn native(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta name=\"fractal-format\" content=\"1\"><title>{title}</title><style data-fractal-style></style></head><body><main data-fractal-document><h1 data-fractal-title>{title}</h1>{body}</main></body></html>"
    )
}

#[test]
fn project_has_no_generated_state() {
    let (temp, project) = project();
    assert!(project.pages().is_empty());
    assert!(temp.path().join("fractal.json").is_file());
    assert!(temp.path().join("pages").is_dir());
    assert!(!temp.path().join(".fractal").exists());
    assert_eq!(project.manifest().version, 2);
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
fn first_folder_metadata_mutation_upgrades_v1_to_v2() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("pages")).unwrap();
    fs::write(
        temp.path().join("fractal.json"),
        r#"{"name":"Version one","version":1}"#,
    )
    .unwrap();
    let mut project = Project::open(temp.path()).unwrap();

    project.set_folder_title(".", "Renamed").unwrap();

    assert_eq!(project.manifest().version, 2);
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(temp.path().join("fractal.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["version"], 2);
    assert!(temp.path().join("pages/fractal.json").is_file());
}

#[test]
fn v1_projects_preserve_nested_fractal_json_as_an_ordinary_asset() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("pages")).unwrap();
    fs::write(
        temp.path().join("fractal.json"),
        r#"{"name":"Version one","version":1}"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("pages/fractal.json"),
        r#"{"title":"Pages"}"#,
    )
    .unwrap();

    let original = fs::read_to_string(temp.path().join("pages/fractal.json")).unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    assert_eq!(project.folder(".").unwrap().title, "Version one");
    let error = project.set_folder_title(".", "Pages").unwrap_err();
    assert_eq!(error.code, FractalErrorCode::Conflict);
    assert_eq!(
        fs::read_to_string(temp.path().join("pages/fractal.json")).unwrap(),
        original
    );
    assert_eq!(project.manifest().version, 1);
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
fn inserting_a_link_is_explicit() {
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
    assert_eq!(report.issues.len(), 4);
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
fn page_hashes_cover_the_exact_source_bytes() {
    let (temp, _project) = project();
    fs::write(temp.path().join("pages/raw.html"), "abc").unwrap();
    let project = Project::open(temp.path()).unwrap();

    let hash = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert_eq!(project.content_hash("raw").unwrap(), hash);
    assert_eq!(project.page("raw").unwrap().content_hash, hash);
}

#[test]
fn conditional_write_rejects_a_stale_editor_revision() {
    let (temp, mut editor) = project();
    editor.create_page("Draft").unwrap();
    let expected_hash = editor.content_hash("draft").unwrap();

    let mut other_process = Project::open(temp.path()).unwrap();
    let winner = native("Draft", "<p>Written elsewhere.</p>");
    other_process.write_page("draft", &winner).unwrap();

    let error = editor
        .write_page_if_unchanged(
            "draft",
            &native("Draft", "<p>Stale editor contents.</p>"),
            &expected_hash,
        )
        .unwrap_err();

    assert_eq!(error.code, FractalErrorCode::Conflict);
    assert_eq!(editor.source("draft").unwrap(), winner);
    assert_eq!(
        fs::read_to_string(temp.path().join("pages/draft.fractal.html")).unwrap(),
        winner
    );
}

#[test]
fn conditional_write_accepts_the_current_hash_and_exposes_the_new_hash() {
    let (_temp, mut project) = project();
    project.create_page("Draft").unwrap();
    let expected_hash = project.content_hash("draft").unwrap();
    let replacement = native("Draft", "<p>Saved.</p>");

    project
        .write_page_if_unchanged("draft", &replacement, &expected_hash)
        .unwrap();

    assert_ne!(project.content_hash("draft").unwrap(), expected_hash);
    assert_eq!(project.source("draft").unwrap(), replacement);
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
    let source = "<!doctype html><html><head><meta name=\"fractal-format\" content=\"1\"><title>Strict</title><style data-fractal-style></style><script>alert(1)</script></head><body><main data-fractal-document><h1 data-fractal-title>Strict</h1><p>Text</p></main></body></html>";

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
fn batch_delete_allows_references_within_the_selection() {
    let (_temp, mut project) = project();
    project.create_page("One").unwrap();
    project.create_page("Two").unwrap();
    project
        .write_page(
            "one",
            &native("One", "<a href=\"two.fractal.html\">Two</a>"),
        )
        .unwrap();

    let mutation = project.delete_pages(["one", "two"]).unwrap();

    assert_eq!(mutation.deleted.len(), 2);
    assert!(project.pages().is_empty());
}

#[test]
fn batch_delete_checks_every_path_before_deleting_any_page() {
    let (_temp, mut project) = project();
    project.create_page("Keep").unwrap();

    let error = project.delete_pages(["keep", "missing"]).unwrap_err();

    assert_eq!(error.code, FractalErrorCode::NotFound);
    assert!(project.page("keep").is_ok());
}

#[test]
fn folder_delete_removes_nested_pages_and_assets() {
    let (temp, mut project) = project();
    project.create_page_at("section/one", "One").unwrap();
    project.create_page_at("section/nested/two", "Two").unwrap();
    project.create_page("Keep").unwrap();
    fs::write(temp.path().join("pages/section/image.png"), "image").unwrap();
    project = Project::open(temp.path()).unwrap();

    let mutation = project.delete_folder("section").unwrap();

    assert_eq!(
        mutation.deleted,
        vec![
            std::path::PathBuf::from("section/image.png"),
            std::path::PathBuf::from("section/nested/two.fractal.html"),
            std::path::PathBuf::from("section/one.fractal.html"),
        ]
    );
    assert!(!temp.path().join("pages/section").exists());
    assert!(project.page("keep").is_ok());
}

#[test]
fn folder_delete_rejects_references_from_surviving_pages() {
    let (_temp, mut project) = project();
    project.create_page_at("section/one", "One").unwrap();
    project.create_page("Keep").unwrap();
    project
        .write_page(
            "keep",
            &native("Keep", "<a href=\"section/one.fractal.html\">One</a>"),
        )
        .unwrap();

    let error = project.delete_folder("section").unwrap_err();

    assert!(error.message.contains("1 link(s)"));
    assert!(project.page("section/one").is_ok());
}

#[test]
fn move_refreshes_the_catalog_before_rewriting_backlinks() {
    let (temp, mut first_process) = project();
    first_process.create_page("Target").unwrap();
    let mut second_process = Project::open(temp.path()).unwrap();
    second_process.create_page("Late backlink").unwrap();
    second_process
        .write_page(
            "late-backlink",
            &native(
                "Late backlink",
                "<a href=\"target.fractal.html\">Target</a>",
            ),
        )
        .unwrap();

    first_process.move_page("target", "moved/target").unwrap();

    assert!(first_process
        .source("late-backlink")
        .unwrap()
        .contains("moved/target.fractal.html"));
}

#[test]
fn opening_a_project_rolls_back_an_interrupted_file_transaction() {
    let (temp, mut project) = project();
    project.create_page("Draft").unwrap();
    let source = project.source("draft").unwrap();
    drop(project);

    let transaction = temp.path().join(".fractal-transaction-test");
    fs::create_dir_all(transaction.join("old")).unwrap();
    fs::rename(
        temp.path().join("pages/draft.fractal.html"),
        transaction.join("old/draft.fractal.html"),
    )
    .unwrap();
    fs::write(
        transaction.join("plan.json"),
        r#"{"affected":["draft.fractal.html"],"originals":["draft.fractal.html"]}"#,
    )
    .unwrap();

    let recovered = Project::open(temp.path()).unwrap();

    assert_eq!(recovered.source("draft").unwrap(), source);
    assert!(!transaction.exists());
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

#[test]
fn html_export_flattens_direct_native_links_into_text_references() {
    let (temp, mut project) = project();
    project.create_page("Source").unwrap();
    project.create_page("Reference").unwrap();
    project.create_page("Nested").unwrap();
    fs::write(temp.path().join("pages/widget.html"), "<p>Raw widget</p>").unwrap();
    project
        .write_page(
            "reference",
            &native(
                "Reference",
                "<p>Reference text. <a href=\"nested.fractal.html\">Nested</a></p>",
            ),
        )
        .unwrap();
    project
        .write_page(
            "source",
            "<!doctype html><html><head><meta name=\"fractal-format\" content=\"1\"><meta name=\"viewport\" content=\"width=device-width\"><link rel=\"stylesheet\" href=\"theme.css\"><style data-fractal-style>body { color: red }</style><title>Source</title></head><body><main data-fractal-document><h1 data-fractal-title>Source</h1><p>Read <strong><a href=\"reference.fractal.html\">Reference</a></strong> and <a href=\"widget.html\">Widget</a>. <a href=\"https://example.com\">External</a>.</p><img src=\"image.png\"><iframe src=\"frame.html\"></iframe></main></body></html>",
        )
        .unwrap();

    let output = temp.path().join("export.html");
    let report = project
        .export_html("source", &output, HtmlExportOptions::default())
        .unwrap();
    let exported = fs::read_to_string(&output).unwrap();

    assert_eq!(report.references, vec!["reference.fractal.html"]);
    assert!(exported.contains(
        "Read <strong><a href=\"#fractal-reference-reference.fractal.html\">Reference</a></strong> and Widget."
    ));
    assert!(exported.contains("<a href=\"https://example.com\">External</a>"));
    assert!(exported.contains("[image]"));
    assert!(exported.contains("[iframe]"));
    assert!(exported.contains("<style data-fractal-style=\"\">body { color: red }</style>"));
    assert!(!exported.contains("theme.css"));
    assert!(!exported.contains("fractal-format"));
    assert!(!exported.contains("data-fractal-document"));
    assert!(exported.contains("<section id=\"fractal-references\">"));
    assert!(exported.contains("<details id=\"fractal-reference-reference.fractal.html\">"));
    let main_start = exported.find("<main").unwrap();
    let references_start = exported
        .find("<section id=\"fractal-references\">")
        .unwrap();
    let main_end = exported.find("</main>").unwrap();
    assert!(main_start < references_start && references_start < main_end);
    assert!(exported.contains("Reference text. Nested"));
    assert!(!exported.contains("Nested text"));
    assert!(!exported.contains("href=\"reference.fractal.html\">Reference"));
}

#[test]
fn html_export_can_include_derived_native_references() {
    let (temp, mut project) = project();
    project.create_page("Target").unwrap();
    project.create_page("Source").unwrap();
    project
        .write_page(
            "source",
            &native("Source", "<p>Target is mentioned here.</p>"),
        )
        .unwrap();

    let output_without = temp.path().join("without.html");
    let without = project
        .export_html("source", &output_without, HtmlExportOptions::default())
        .unwrap();
    assert!(without.references.is_empty());

    let output_with = temp.path().join("with.html");
    let with = project
        .export_html(
            "source",
            &output_with,
            HtmlExportOptions {
                include_derived_links: true,
            },
        )
        .unwrap();
    assert_eq!(with.references, vec!["target.fractal.html"]);
    let exported = fs::read_to_string(output_with).unwrap();
    assert!(exported.contains("<a href=\"#fractal-reference-target.fractal.html\">Target</a>"));
    assert!(exported.contains("<summary>Target</summary>"));
}

#[test]
fn html_export_rejects_raw_html_pages() {
    let (temp, _project) = project();
    fs::write(temp.path().join("pages/raw.html"), "<p>Raw</p>").unwrap();
    let project = Project::open(temp.path()).unwrap();

    let error = project
        .export_html(
            "raw",
            temp.path().join("export.html"),
            HtmlExportOptions::default(),
        )
        .unwrap_err();

    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert!(error
        .message
        .contains("only available for native documents"));
}

#[test]
fn folder_html_export_follows_recursive_order_and_numbers_pages() {
    let (temp, mut project) = project();
    project
        .create_page_at("intro.fractal.html", "Intro")
        .unwrap();
    project
        .create_page_at("part/two.fractal.html", "Two")
        .unwrap();
    project
        .create_page_at("part/one.fractal.html", "One")
        .unwrap();
    project
        .reorder_folder("part", ["two.fractal.html", "one.fractal.html"])
        .unwrap();
    project
        .reorder_folder(".", ["intro.fractal.html", "part"])
        .unwrap();

    let output = temp.path().join("folder.html");
    let report = project
        .export_folder_html(
            ".",
            &output,
            FolderHtmlExportOptions {
                number_sections: true,
                ..Default::default()
            },
        )
        .unwrap();
    let html = fs::read_to_string(output).unwrap();

    assert_eq!(
        report.pages,
        vec![
            "intro.fractal.html",
            "part/two.fractal.html",
            "part/one.fractal.html"
        ]
    );
    assert!(html.find("<h1>1. Intro</h1>").unwrap() < html.find("<h1>2. Two</h1>").unwrap());
    assert!(html.find("<h1>2. Two</h1>").unwrap() < html.find("<h1>3. One</h1>").unwrap());
    assert_eq!(html.matches("<hr>").count(), 2);
    assert_eq!(html.matches("<h1>").count(), 3);
    assert!(html.contains("<title>Test</title>"));
}

#[test]
fn folder_export_selections_expand_only_unqualified_folders() {
    let (temp, mut project) = project();
    project
        .create_page_at("part/one.fractal.html", "One")
        .unwrap();
    project
        .create_page_at("part/two.fractal.html", "Two")
        .unwrap();

    let whole_folder = project
        .export_folder_html(
            ".",
            temp.path().join("whole.html"),
            FolderHtmlExportOptions {
                selections: vec!["part".into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(whole_folder.pages.len(), 2);

    let selected_child = project
        .export_folder_html(
            ".",
            temp.path().join("selected.html"),
            FolderHtmlExportOptions {
                selections: vec!["part".into(), "part/two.fractal.html".into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(selected_child.pages, vec!["part/two.fractal.html"]);
}

#[test]
fn folder_export_force_skips_invalid_and_ghost_pages() {
    let (temp, mut project) = project();
    project.create_page("Good").unwrap();
    project.create_page("Ghost").unwrap();
    project.create_page("Bad").unwrap();
    project
        .reorder_folder(
            ".",
            [
                "good.fractal.html",
                "ghost.fractal.html",
                "bad.fractal.html",
            ],
        )
        .unwrap();
    fs::remove_file(temp.path().join("pages/ghost.fractal.html")).unwrap();
    fs::write(
        temp.path().join("pages/bad.fractal.html"),
        "<html><body>bad</body></html>",
    )
    .unwrap();
    project = Project::open(temp.path()).unwrap();

    assert!(project
        .export_folder_html(
            ".",
            temp.path().join("refused.html"),
            FolderHtmlExportOptions::default(),
        )
        .unwrap_err()
        .message
        .contains("bad.fractal.html"));
    let report = project
        .export_folder_html(
            ".",
            temp.path().join("forced.html"),
            FolderHtmlExportOptions {
                force: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(report.pages, vec!["good.fractal.html"]);
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].path, "bad.fractal.html");
}

#[test]
fn folder_export_links_included_pages_and_places_derived_references_last() {
    let (temp, mut project) = project();
    project.create_page("First").unwrap();
    project.create_page("Second").unwrap();
    project.create_page("Reference").unwrap();
    project
        .write_page(
            "first",
            &native(
                "First",
                "<p><a href=\"second.fractal.html\">Second</a> and Reference.</p>",
            ),
        )
        .unwrap();

    let output = temp.path().join("links.html");
    let report = project
        .export_folder_html(
            ".",
            &output,
            FolderHtmlExportOptions {
                selections: vec!["first.fractal.html".into(), "second.fractal.html".into()],
                include_derived_links: true,
                ..Default::default()
            },
        )
        .unwrap();
    let html = fs::read_to_string(output).unwrap();
    assert!(html.contains("href=\"#fractal-page-"));
    assert!(!html.contains("href=\"#fractal-reference-second.fractal.html\""));
    assert_eq!(report.references, vec!["reference.fractal.html"]);
    assert!(
        html.find("id=\"fractal-references\"").unwrap() > html.find("<h1>Second</h1>").unwrap()
    );
}

#[test]
fn folders_default_to_folders_first_then_native_files() {
    let (temp, mut project) = project();
    fs::create_dir_all(temp.path().join("pages/z-folder")).unwrap();
    fs::create_dir_all(temp.path().join("pages/a-folder")).unwrap();
    fs::write(temp.path().join("pages/raw.html"), "<p>Raw</p>").unwrap();
    project.create_page_at("z.fractal.html", "Z").unwrap();
    project.create_page_at("a.fractal.html", "A").unwrap();

    let project = Project::open(temp.path()).unwrap();
    let folder = project.folder(".").unwrap();
    assert_eq!(folder.title, "Test");
    assert_eq!(folder.order, None);
    assert_eq!(
        folder
            .children
            .iter()
            .map(|child| (child.name.as_str(), child.kind))
            .collect::<Vec<_>>(),
        vec![
            ("a-folder", FolderChildKind::Folder),
            ("z-folder", FolderChildKind::Folder),
            ("a.fractal.html", FolderChildKind::Native),
            ("z.fractal.html", FolderChildKind::Native),
        ]
    );
}

#[test]
fn reorder_is_complete_and_preserves_missing_children_as_ghosts() {
    let (temp, mut project) = project();
    project.create_page("One").unwrap();
    project.create_page("Two").unwrap();
    project
        .reorder_folder(".", ["two.fractal.html", "one.fractal.html"])
        .unwrap();
    fs::remove_file(temp.path().join("pages/one.fractal.html")).unwrap();

    let mut project = Project::open(temp.path()).unwrap();
    let folder = project.folder(".").unwrap();
    assert_eq!(folder.children[1].status, FolderChildStatus::Missing);
    assert!(!project.validate().valid);
    assert!(project
        .reorder_folder(".", ["two.fractal.html"])
        .unwrap_err()
        .message
        .contains("one.fractal.html"));
    project
        .reorder_folder(".", ["one.fractal.html", "two.fractal.html"])
        .unwrap();
}

#[test]
fn external_and_engine_created_children_append_to_explicit_order() {
    let (temp, mut project) = project();
    project.create_page("One").unwrap();
    project.reorder_folder(".", ["one.fractal.html"]).unwrap();
    fs::write(
        temp.path().join("pages/two.fractal.html"),
        native("Two", ""),
    )
    .unwrap();

    let mut project = Project::open(temp.path()).unwrap();
    assert_eq!(
        project.folder(".").unwrap().order.unwrap(),
        vec!["one.fractal.html", "two.fractal.html"]
    );
    project.create_page("Three").unwrap();
    assert_eq!(
        project.folder(".").unwrap().order.unwrap(),
        vec!["one.fractal.html", "two.fractal.html", "three.fractal.html"]
    );
}

#[test]
fn deleting_a_ghost_removes_only_its_order_entry() {
    let (temp, mut project) = project();
    project.create_page("Gone").unwrap();
    project.reorder_folder(".", ["gone.fractal.html"]).unwrap();
    fs::remove_file(temp.path().join("pages/gone.fractal.html")).unwrap();

    let mut project = Project::open(temp.path()).unwrap();
    let mutation = project.delete_page("gone").unwrap();
    assert_eq!(
        mutation.deleted,
        vec![std::path::PathBuf::from("gone.fractal.html")]
    );
    assert!(project.folder(".").unwrap().children.is_empty());
    assert!(temp.path().join("pages/fractal.json").is_file());
}

#[test]
fn deleting_a_missing_ordered_folder_removes_its_ghost() {
    let (temp, _) = project();
    fs::create_dir(temp.path().join("pages/appendix")).unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project.reorder_folder(".", ["appendix"]).unwrap();
    fs::remove_dir(temp.path().join("pages/appendix")).unwrap();

    let mut project = Project::open(temp.path()).unwrap();
    let mutation = project.delete_folder("appendix").unwrap();
    assert_eq!(
        mutation.changed,
        vec![std::path::PathBuf::from("fractal.json")]
    );
    assert!(mutation.deleted.is_empty());
    assert!(project.folder(".").unwrap().children.is_empty());
}

#[test]
fn moving_a_native_page_preserves_its_order_position() {
    let (_temp, mut project) = project();
    project.create_page("One").unwrap();
    project.create_page("Two").unwrap();
    project
        .reorder_folder(".", ["two.fractal.html", "one.fractal.html"])
        .unwrap();

    project.move_page("two", "renamed").unwrap();

    assert_eq!(
        project.folder(".").unwrap().order.unwrap(),
        vec!["renamed.fractal.html", "one.fractal.html"]
    );
}

#[test]
fn setting_a_folder_title_renames_the_directory() {
    let (temp, _) = project();
    fs::create_dir(temp.path().join("pages/draft-name")).unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project
        .set_folder_title("draft-name", "The Glass Garden")
        .unwrap();

    let reopened = Project::open(temp.path()).unwrap();
    assert_eq!(
        reopened.folder("the-glass-garden").unwrap().title,
        "The Glass Garden"
    );
    assert!(!temp.path().join("pages/draft-name").exists());
}

#[test]
fn moving_a_folder_preserves_its_title_and_rewrites_references() {
    let (temp, _) = project();
    fs::create_dir_all(temp.path().join("pages/archive")).unwrap();
    fs::create_dir_all(temp.path().join("pages/section")).unwrap();
    fs::write(
        temp.path().join("pages/section/topic.fractal.html"),
        native("Topic", "<p>Body</p>"),
    )
    .unwrap();
    fs::write(temp.path().join("pages/section/image.png"), "asset").unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    project.create_page("Index").unwrap();
    project
        .write_page(
            "index",
            &native("Index", "<a href=\"section/topic.fractal.html\">topic</a>"),
        )
        .unwrap();

    let mutation = project.move_folder("section", "archive/section").unwrap();

    assert_eq!(project.folder("archive/section").unwrap().title, "section");
    assert!(project
        .source("index")
        .unwrap()
        .contains("archive/section/topic.fractal.html"));
    assert!(temp
        .path()
        .join("pages/archive/section/image.png")
        .is_file());
    assert!(mutation
        .changed
        .contains(&std::path::PathBuf::from("archive/section/image.png")));
    assert!(mutation
        .deleted
        .contains(&std::path::PathBuf::from("section/image.png")));
}

#[test]
fn moving_a_folder_cannot_change_its_title_driven_basename() {
    let (temp, _) = project();
    fs::create_dir_all(temp.path().join("pages/archive")).unwrap();
    fs::create_dir(temp.path().join("pages/section")).unwrap();
    let mut project = Project::open(temp.path()).unwrap();

    let error = project
        .move_folder("section", "archive/renamed")
        .unwrap_err();

    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert!(temp.path().join("pages/section").is_dir());
}

#[test]
fn setting_a_page_title_renames_it_and_rewrites_explicit_links() {
    let (_temp, mut project) = project();
    project.create_page("Old name").unwrap();
    project.create_page("Index").unwrap();
    project
        .write_page(
            "index",
            &native("Index", "<a href=\"old-name.fractal.html\">old</a>"),
        )
        .unwrap();

    project
        .set_page_title("old-name", "Guns Akimbo and other Stuff")
        .unwrap();

    assert_eq!(
        project
            .page("guns-akimbo-and-other-stuff")
            .unwrap()
            .title
            .as_deref(),
        Some("Guns Akimbo and other Stuff")
    );
    assert!(project
        .source("index")
        .unwrap()
        .contains("guns-akimbo-and-other-stuff.fractal.html"));
}

#[test]
fn native_sections_merge_disjoint_concurrent_changes() {
    let (temp, mut content_editor) = project();
    content_editor.create_page("Draft").unwrap();
    let mut style_editor = Project::open(temp.path()).unwrap();
    let content_parts = content_editor.native_document_parts("draft").unwrap();
    let style_parts = style_editor.native_document_parts("draft").unwrap();

    style_editor
        .set_page_style("draft", "body { color: hotpink; }", &style_parts.style_hash)
        .unwrap();
    content_editor
        .set_page_content(
            "draft",
            "<p>Written concurrently.</p>",
            &content_parts.content_hash,
        )
        .unwrap();

    let reopened = Project::open(temp.path()).unwrap();
    let parts = reopened.native_document_parts("draft").unwrap();
    assert_eq!(parts.style_css, "body { color: hotpink; }");
    assert!(parts.content_html.contains("Written concurrently."));
    assert!(!parts.content_html.contains("data-fractal-title"));
}

#[test]
fn native_section_hash_rejects_a_stale_change_to_the_same_section() {
    let (_temp, mut project) = project();
    project.create_page("Draft").unwrap();
    let parts = project.native_document_parts("draft").unwrap();
    project
        .set_page_content("draft", "<p>First</p>", &parts.content_hash)
        .unwrap();
    let error = project
        .set_page_content("draft", "<p>Stale</p>", &parts.content_hash)
        .unwrap_err();
    assert_eq!(error.code, FractalErrorCode::Conflict);
}

#[test]
fn raw_writes_cannot_replace_a_native_document() {
    let (_temp, mut project) = project();
    project.create_page("Native").unwrap();
    let error = project
        .write_raw_page("native", "<p>replacement</p>")
        .unwrap_err();
    assert!(error.message.contains("only available for raw HTML"));
}

#[test]
fn structure_repair_marks_legacy_title_and_style() {
    let (temp, _project) = project();
    fs::write(
        temp.path().join("pages/legacy.fractal.html"),
        "<!doctype html><html><head><meta name=\"fractal-format\" content=\"1\"><title>Legacy</title><style>p { color: red; }</style></head><body><main data-fractal-document><h1>Legacy</h1><p>Text</p></main></body></html>",
    )
    .unwrap();
    let mut project = Project::open(temp.path()).unwrap();
    assert!(project.native_document_parts("legacy").is_err());
    project.repair_page_structure("legacy").unwrap();
    let source = project.source("legacy").unwrap();
    assert!(source.contains("data-fractal-title"));
    assert!(source.contains("data-fractal-style"));
    assert!(source.contains("p { color: red; }"));
}

#[test]
fn metadata_and_head_links_are_contained_sections() {
    let (_temp, mut project) = project();
    project.create_page("Sections").unwrap();
    let parts = project.native_document_parts("sections").unwrap();
    project
        .set_page_metadata(
            "sections",
            "<meta name=\"description\" content=\"Example\">",
            &parts.metadata_hash,
        )
        .unwrap();
    let parts = project.native_document_parts("sections").unwrap();
    project
        .set_page_head_links(
            "sections",
            "<link rel=\"stylesheet\" href=\"theme.css\">",
            &parts.head_links_hash,
        )
        .unwrap();
    let parts = project.native_document_parts("sections").unwrap();
    assert!(parts.metadata_html.contains("description"));
    assert!(parts.head_links_html.contains("theme.css"));
    let error = project
        .set_page_metadata(
            "sections",
            "<meta name=\"fractal-format\" content=\"9\">",
            &parts.metadata_hash,
        )
        .unwrap_err();
    assert!(error.message.contains("cannot be changed"));
}

#[test]
fn opening_repairs_title_path_mismatches() {
    let (temp, mut project) = project();
    project.create_page("Correct title").unwrap();
    fs::rename(
        temp.path().join("pages/correct-title.fractal.html"),
        temp.path().join("pages/wrong.fractal.html"),
    )
    .unwrap();
    drop(project);

    let project = Project::open(temp.path()).unwrap();
    assert_eq!(
        project.page("correct-title").unwrap().title.as_deref(),
        Some("Correct title")
    );
    assert!(!temp.path().join("pages/wrong.fractal.html").exists());
}
