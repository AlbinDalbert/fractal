use crate::document::html::escape_html;
use crate::document::notes::{insert_note_into_document, note_id_from_trigger, render_note_aside};
use crate::document::render::{render_page_document, stylesheet_href};
use crate::document::PageDocument;
use crate::graph::{
    graph_neighbors_report, graph_orphans_report, graph_page, graph_page_report, neighbor_pages,
    orphan_pages,
};
use crate::io::markdown::{html_to_markdown, markdown_to_html};
use crate::project::constants::{GRAPH_VERSION, INDEX_VERSION, MANIFEST_VERSION};
use crate::project::paths::{collect_page_paths, resolve_page_destination};
use crate::validation::validate_page_metadata;
use crate::FractalErrorCode;
use crate::{
    add_note, build_index, create_directory, create_page, delete_directory, delete_page,
    editor_page_detail, export_page, extract_page_text, graph_backlinks_report, graph_notes_report,
    graph_outlinks_report, graph_related_report, import_markdown, init_project_at,
    list_editor_pages, load_project_index, load_project_manifest, new_page, page_backlinks,
    page_metadata, page_metadata_report, page_notes, page_outlinks, patch_note,
    preflight_delete_page, preflight_rename_page, preflight_repair_project, project_summary,
    read_page_source, related_pages, remove_note, rename_page, repair_project, reset_page_metadata,
    search_project, search_report, set_page_summary, set_page_tags, set_page_title, sync_project,
    update_editor_page, update_page_body, validate_project, write_page_source, EditorLinkDetail,
    EditorNoteDetail, EditorPageListEntry, EditorPageUpdate, FileEntry, GraphEdge,
    GraphNeighborPage, GraphNode, GraphNoteLink, GraphPageLink, GraphRelatedPage, LinkEntry,
    NoteEntry, OperationEvent, PageCreate, PageEntry, PageGraphEntry, PageRename, ProjectGraph,
    ProjectIndex, ProjectManifest, SearchMatch, SearchResult, Theme,
};
use std::collections::BTreeMap;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tempfile::{Builder, TempDir};

struct TestDir {
    dir: TempDir,
}

impl TestDir {
    fn path(&self) -> &Path {
        self.dir.path()
    }
}

impl AsRef<Path> for TestDir {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl Deref for TestDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

struct TestProject {
    root: TestDir,
}

impl TestProject {
    fn new(name: &str) -> Self {
        let root = temp_dir(name);
        fs::create_dir_all(root.pages_dir()).expect("create pages dir");
        fs::create_dir_all(root.workspace_dir()).expect("create workspace dir");
        fs::write(root.workspace_dir().join("style.css"), "").expect("write stylesheet");
        write_test_manifest(root.path());
        Self { root }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn pages_dir(&self) -> PathBuf {
        self.root.pages_dir()
    }

    fn workspace_dir(&self) -> PathBuf {
        self.root.workspace_dir()
    }

    fn write_page(&self, path: &str, html: impl AsRef<str>) {
        let page = self.pages_dir().join(path);
        if let Some(parent) = page.parent() {
            fs::create_dir_all(parent).expect("create page parent dir");
        }
        fs::write(page, html.as_ref()).expect("write page");
    }
}

trait TestRootExt {
    fn pages_dir(&self) -> PathBuf;
    fn workspace_dir(&self) -> PathBuf;
}

impl<T: AsRef<Path>> TestRootExt for T {
    fn pages_dir(&self) -> PathBuf {
        self.as_ref().join("pages")
    }

    fn workspace_dir(&self) -> PathBuf {
        self.as_ref().join(".fractal")
    }
}

fn temp_dir(name: &str) -> TestDir {
    TestDir {
        dir: Builder::new()
            .prefix(&format!("fractal-{name}-"))
            .tempdir()
            .expect("create temp dir"),
    }
}

fn required_meta() -> BTreeMap<String, String> {
    [
        ("fractal:summary".to_string(), "".to_string()),
        ("fractal:tags".to_string(), "".to_string()),
        ("fractal:version".to_string(), "0.1".to_string()),
    ]
    .into_iter()
    .collect()
}

fn write_test_manifest(root: &Path) {
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: MANIFEST_VERSION,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
}

fn write_test_graph(root: &Path, pages: Vec<PageGraphEntry>) {
    write_test_graph_data(root, Vec::new(), Vec::new(), pages);
}

fn write_test_graph_data(
    root: &Path,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    pages: Vec<PageGraphEntry>,
) {
    fs::create_dir_all(root.join(".fractal")).expect("create workspace dir");
    fs::write(
        root.join(".fractal").join("graph.json"),
        serde_json::to_string_pretty(&ProjectGraph {
            version: GRAPH_VERSION,
            nodes,
            edges,
            pages,
        })
        .expect("serialize graph"),
    )
    .expect("write graph");
}

#[test]
fn init_project_at_uses_explicit_destination_and_project_name() {
    let parent = temp_dir("init-at");
    let root = parent.join("custom-root");

    let report = init_project_at(&root, "Display Name").expect("initialize project");

    assert_eq!(
        report.events,
        vec![OperationEvent::ProjectCreated {
            path: PathBuf::from(".")
        }]
    );
    assert!(!root.join("pages/index.html").exists());
    assert!(root.join(".fractal/style.css").is_file());
    assert_eq!(
        load_project_manifest(&root)
            .expect("load manifest")
            .project_name,
        "Display Name"
    );
}

#[test]
fn escapes_html_sensitive_characters() {
    assert_eq!(escape_html("&<>"), "&amp;&lt;&gt;");
}

#[test]
fn collects_paths_relative_to_pages_root() {
    let root = temp_dir("pages");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("create nested dir");
    fs::write(root.join("index.html"), "").expect("write index");
    fs::write(nested.join("note.html"), "").expect("write nested note");

    let mut pages = Vec::new();
    collect_page_paths(root.path(), root.path(), &mut pages).expect("collect page paths");
    pages.sort();

    assert_eq!(pages, vec!["index.html", "nested/note.html"]);
}

#[test]
fn page_renderer_includes_required_meta_tags() {
    let html = render_page_document(
        "hello",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );

    assert!(html.contains("<meta name=\"fractal:version\" content=\"0.1\" />"));
    assert!(html.contains("<meta name=\"fractal:summary\" content=\"\" />"));
    assert!(html.contains("<meta name=\"fractal:tags\" content=\"\" />"));
    assert!(html.contains("<link rel=\"stylesheet\" href=\"../.fractal/style.css\">"));
    assert!(html.contains("<body data-fractal-theme=\"dark\">"));
    assert!(html.contains("<section data-fractal-notes>"));
}

#[test]
fn markdown_import_converts_basic_blocks_to_html() {
    let (title, body) = markdown_to_html(
        "fallback",
        "# Title\n\nIntro line\ncontinued line\n\n## Section\n\nBody & <more>",
    );

    assert_eq!(title, "Title");
    assert_eq!(
            body,
            "<p>Intro line continued line</p>\n      <h2>Section</h2>\n      <p>Body &amp; &lt;more&gt;</p>"
        );
}

#[test]
fn markdown_import_keeps_non_initial_h1_in_body() {
    let (title, body) = markdown_to_html("fallback", "Intro\n\n# Later");

    assert_eq!(title, "fallback");
    assert_eq!(body, "<p>Intro</p>\n      <h1>Later</h1>");
}

#[test]
fn html_export_converts_basic_blocks_to_markdown() {
    let html = render_page_document(
        "Title",
        "<p>Intro &amp; more</p>\n      <h2>Section</h2>\n      <p>Body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );

    assert_eq!(
        html_to_markdown(&html),
        "# Title\n\nIntro & more\n\n## Section\n\nBody"
    );
}

#[test]
fn html_export_reads_ordinary_html_formatting() {
    let html = r#"<!doctype html>
<HTML>
  <BODY>
    <MAIN class="content">
      <H1>Flexible <em>Title</em></H1>
      <p>
        Intro <a href="topic.html">link</a> &amp; more
      </p>
      <section>
        <h2>Nested Section</h2>
        <p><strong>Body</strong> text</p>
      </section>
    </MAIN>
    <section data-fractal-notes>
      <aside id="note-ignore" data-fractal-note><p>Ignore me</p></aside>
    </section>
  </BODY>
</HTML>
"#;

    assert_eq!(
        html_to_markdown(html),
        "# Flexible Title\n\nIntro link & more\n\n## Nested Section\n\nBody text"
    );
}

#[test]
fn import_and_export_markdown_files() {
    let root = temp_dir("markdown-io");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: MANIFEST_VERSION,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
    let source = root.join("source.md");
    fs::write(&source, "# Source\n\nHello\n\n### Deep").expect("write markdown");

    import_markdown(root.path(), &source).expect("import markdown");
    let page = fs::read_to_string(pages_dir.join("source.html")).expect("read imported page");
    assert!(page.contains("<h1>Source</h1>"));
    assert!(page.contains("<p>Hello</p>"));
    assert!(page.contains("<h3>Deep</h3>"));

    let output = root.join("out.md");
    export_page(root.path(), Path::new("pages/source.html"), &output).expect("export markdown");
    assert_eq!(
        fs::read_to_string(output).expect("read exported markdown"),
        "# Source\n\nHello\n\n### Deep"
    );
}

#[test]
fn markdown_import_rejects_existing_destination() {
    let project = TestProject::new("markdown-overwrite");
    project.write_page(
        "source.html",
        render_page_document(
            "source",
            "<p>existing</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    let source = project.root().join("source.md");
    fs::write(&source, "# Source").expect("write markdown");

    let error = import_markdown(project.root(), &source).expect_err("overwrite should fail");

    assert!(error.to_string().contains("page already exists"));
    assert!(fs::read_to_string(project.pages_dir().join("source.html"))
        .expect("read existing page")
        .contains("existing"));
}

#[test]
fn export_page_accepts_optional_pages_prefix() {
    let project = TestProject::new("export-prefix");
    project.write_page(
        "source.html",
        render_page_document(
            "Source",
            "<p>Hello</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    let output = project.root().join("out.md");

    export_page(project.root(), Path::new("source"), &output).expect("export markdown");
    assert_eq!(
        fs::read_to_string(&output).expect("read export"),
        "# Source\n\nHello"
    );

    export_page(project.root(), Path::new("pages/source.html"), &output)
        .expect("export markdown with pages prefix");
    assert_eq!(
        fs::read_to_string(output).expect("read export"),
        "# Source\n\nHello"
    );
}

#[test]
fn extract_page_text_returns_compact_main_text() {
    let project = TestProject::new("extract-page");
    project.write_page(
        "source.html",
        r#"<!doctype html>
<html>
  <head>
    <title>Source</title>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="Do not include me.">
    <meta name="fractal:tags" content="hidden">
  </head>
  <body>
    <main>
      <h1>Source</h1>
      <p>Alpha <a href="other.html">linked beta</a>.</p>
      <pre>code block</pre>
    </main>
    <section data-fractal-notes>
      <aside id="note-alpha" data-fractal-note><p>Note text</p></aside>
    </section>
  </body>
</html>"#,
    );

    assert_eq!(
        extract_page_text(project.root(), Path::new("source")).expect("extract text"),
        "Source Alpha linked beta. code block"
    );
}

#[test]
fn stylesheet_href_reaches_workspace_from_nested_pages() {
    assert_eq!(
        stylesheet_href(Path::new("index.html")),
        "../.fractal/style.css"
    );
    assert_eq!(
        stylesheet_href(Path::new("folder/subpage.html")),
        "../../.fractal/style.css"
    );
}

#[test]
fn validate_page_metadata_rejects_missing_notes_section() {
    let root = temp_dir("validate-notes");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    let html = render_page_document(
        "page",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    )
    .replace("    <section data-fractal-notes>\n    </section>\n", "");
    fs::write(&page, html).expect("write page");

    let error = validate_page_metadata(&page).expect_err("missing notes section should fail");
    assert!(error.to_string().contains("missing notes section"));
}

#[test]
fn validate_page_metadata_rejects_duplicate_notes_sections() {
    let root = temp_dir("validate-duplicate-notes");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    let html = render_page_document(
        "page",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    )
    .replace(
        "    <section data-fractal-notes>\n    </section>",
        "    <section data-fractal-notes>\n    </section>\n    <section data-fractal-notes></section>",
    );
    fs::write(&page, html).expect("write page");

    let error = validate_page_metadata(&page).expect_err("duplicate notes sections should fail");
    assert!(error.to_string().contains("duplicate notes section"));
}

#[test]
fn validate_page_metadata_rejects_malformed_note_ids() {
    let root = temp_dir("validate-note-id");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    let html = render_page_document(
        "page",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    )
    .replace(
        "    <section data-fractal-notes>\n    </section>",
        "    <section data-fractal-notes>\n      <aside id=\"Rust\" data-fractal-note><p>bad</p></aside>\n    </section>",
    );
    fs::write(&page, html).expect("write page");

    let error = validate_page_metadata(&page).expect_err("malformed note id should fail");
    assert!(error.to_string().contains("malformed note id"));
}

#[test]
fn validate_page_metadata_allows_custom_summary_and_tags() {
    let root = temp_dir("validate-custom-meta");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    let html = render_page_document(
        "page",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    )
    .replace(
        "content=\"Short page summary here.\"",
        "content=\"A custom project summary.\"",
    )
    .replace(
        "content=\"rust, graphs, parsing\"",
        "content=\"personal, custom\"",
    );
    fs::write(&page, html).expect("write page");

    validate_page_metadata(&page).expect("custom metadata should validate");
}

#[test]
fn validate_project_rejects_extra_fractal_meta_tags() {
    let project = TestProject::new("validate-extra-fractal-meta");
    let html = render_page_document(
        "Home",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    )
    .replace(
        "    <link rel=\"stylesheet\" href=\"../.fractal/style.css\">",
        "    <meta name=\"fractal:custom\" content=\"nope\">\n    <link rel=\"stylesheet\" href=\"../.fractal/style.css\">",
    );
    project.write_page("index.html", html);

    let error = validate_project(project.root()).expect_err("extra fractal meta should fail");
    assert_eq!(error.code, FractalErrorCode::InvalidProject);
    assert!(error.to_string().contains("unsupported Fractal meta tag"));
}

#[test]
fn validate_project_rejects_title_heading_mismatch() {
    let project = TestProject::new("validate-title-heading-mismatch");
    let html = render_page_document(
        "Home",
        "<p>body</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    )
    .replace("<h1>Home</h1>", "<h1>Other</h1>");
    project.write_page("index.html", html);

    let error = validate_project(project.root()).expect_err("mismatched title should fail");
    assert!(error.to_string().contains("title and main heading differ"));
}

#[test]
fn validate_project_rejects_unsupported_body_elements_and_manual_links() {
    let project = TestProject::new("validate-body-contract");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><span>bad</span></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = validate_project(project.root()).expect_err("unsupported span should fail");
    assert_eq!(error.code, FractalErrorCode::InvalidProject);
    assert!(error
        .to_string()
        .contains("unsupported Fractal body element"));

    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"missing.html\">Manual</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = validate_project(project.root()).expect_err("manual link should fail");
    assert!(error
        .to_string()
        .contains("manual link is not valid Fractal"));
}

#[test]
fn validate_project_rejects_broken_generated_links() {
    let project = TestProject::new("validate-broken-generated-link");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"missing.html\" data-fractal-link=\"page\">Missing</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = validate_project(project.root()).expect_err("missing generated target should fail");
    assert!(error
        .to_string()
        .contains("generated page link target is missing"));
}

#[test]
fn validate_project_rejects_page_links_whose_text_does_not_match_the_target() {
    let project = TestProject::new("validate-mismatched-page-link-text");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"rust.html\" data-fractal-link=\"page\">the language</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error =
        validate_project(project.root()).expect_err("mismatched generated link text should fail");
    assert!(error
        .to_string()
        .contains("generated page link text does not identify its target"));
    assert!(error.to_string().contains("expected `Rust`"));

    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"rust.html\">the language</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = validate_project(project.root()).expect_err("mismatched manual link should fail");
    assert!(error
        .to_string()
        .contains("manual page link text does not identify its target"));
}

#[test]
fn validate_fix_rewrites_mismatched_internal_links_to_visible_target_labels() {
    let project = TestProject::new("validate-fix-mismatched-page-links");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            r#"<p><a href="rust.html">the language</a></p>
      <p><a href="rust.html" data-fractal-link="page">systems language</a></p>"#,
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    repair_project(project.root()).expect("fix mismatched internal links");

    let fixed =
        fs::read_to_string(project.pages_dir().join("index.html")).expect("read fixed index page");
    assert!(fixed.contains("<a data-fractal-link=\"page\" href=\"rust.html\">Rust</a>"));
    assert!(!fixed.contains("the language (Rust)"));
    assert!(!fixed.contains("systems language (Rust)"));
}

#[test]
fn validate_fix_repairs_simple_contract_drift_and_unwraps_manual_links() {
    let project = TestProject::new("validate-fix-contract-drift");
    let html = render_page_document(
        "Home",
        "<p><a href=\"missing.html\">Manual</a></p>",
        Theme::Light,
        "wrong.css".to_string(),
    )
    .replace("    <title>Home</title>\n", "");
    project.write_page("index.html", html);

    let preflight = preflight_repair_project(project.root()).expect("preflight simple drift");
    assert!(preflight.events.iter().any(|event| matches!(
        event,
        OperationEvent::ProjectRepaired { path, applied: false } if path == Path::new("pages/index.html")
    )));
    let preflight_summary = preflight.summary();
    assert!(!preflight_summary.source_changed);
    assert_eq!(preflight_summary.repaired_paths.len(), 1);

    let preview =
        fs::read_to_string(project.pages_dir().join("index.html")).expect("read preview page");
    assert!(!preview.contains("<title>Home</title>"));
    assert!(preview.contains("href=\"wrong.css\""));

    let repair = repair_project(project.root()).expect("fix simple drift");
    let repair_summary = repair.summary();
    assert!(repair_summary.source_changed);
    assert_eq!(repair_summary.repaired_paths.len(), 1);

    let fixed =
        fs::read_to_string(project.pages_dir().join("index.html")).expect("read fixed page");
    assert!(fixed.contains("<title>Home</title>"));
    assert!(fixed.contains("href=\"../.fractal/style.css\""));
    assert!(fixed.contains("data-fractal-theme=\"dark\""));
    assert!(!fixed.contains("<a href=\"missing.html\""));
    assert!(fixed.contains("Manual"));
}

#[test]
fn write_page_source_rejects_invalid_fractal_html_before_saving() {
    let project = TestProject::new("write-source-validates");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>original</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    let invalid = render_page_document(
        "Home",
        "<p><span>invalid</span></p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );

    let error = write_page_source(project.root(), Path::new("index"), invalid)
        .expect_err("invalid source should fail");
    assert!(error
        .to_string()
        .contains("unsupported Fractal body element"));
    assert!(fs::read_to_string(project.pages_dir().join("index.html"))
        .expect("read original page")
        .contains("original"));
}

#[test]
fn validate_rejects_unsupported_manifest_version() {
    let root = temp_dir("manifest-version");
    fs::create_dir_all(root.path()).expect("create temp dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: MANIFEST_VERSION + 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");

    let error = validate_project(root.path()).expect_err("unsupported manifest should fail");
    assert!(error.to_string().contains("unsupported manifest version"));
}

#[test]
fn validate_fix_repairs_missing_project_scaffold_and_page_markers() {
    let root = temp_dir("validate-fix");
    let pages_dir = root.join("pages");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: MANIFEST_VERSION,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
    fs::write(
            pages_dir.join("index.html"),
            "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <title>index</title>\n  </head>\n  <body>\n    <main>\n      <h1>index</h1>\n      <p>body</p>\n    </main>\n  </body>\n</html>\n",
        )
        .expect("write page");

    repair_project(root.path()).expect("fix and validate project");

    assert!(root.join(".fractal/style.css").is_file());
    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
    let document = PageDocument::parse(&html);
    assert_eq!(document.fractal_meta(), required_meta());
    assert!(document
        .document
        .select_first("link[rel=\"stylesheet\"][href=\"../.fractal/style.css\"]")
        .is_ok());
    assert!(html.contains("<body data-fractal-theme=\"dark\">"));
    assert_eq!(document.notes_section_count(), 1);
}

#[test]
fn validate_fix_merges_duplicate_notes_sections() {
    let root = temp_dir("validate-fix-duplicate-notes");
    let pages_dir = root.join("pages");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    write_test_manifest(root.path());
    fs::write(
        pages_dir.join("index.html"),
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>index</title>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="">
    <meta name="fractal:tags" content="">
  </head>
  <body>
    <main><h1>index</h1></main>
    <section data-fractal-notes>
      <aside id="note-alpha" data-fractal-note><p>alpha</p></aside>
    </section>
    <section data-fractal-notes>
      <aside id="note-beta" data-fractal-note><p>beta</p></aside>
    </section>
  </body>
</html>
"#,
    )
    .expect("write page");

    repair_project(root.path()).expect("fix and validate project");

    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
    let document = PageDocument::parse(&html);
    assert_eq!(document.notes_section_count(), 1);
    assert_eq!(
        document.notes(),
        vec![
            NoteEntry {
                id: "note-alpha".to_string(),
                label: "alpha".to_string(),
            },
            NoteEntry {
                id: "note-beta".to_string(),
                label: "beta".to_string(),
            },
        ]
    );
}

#[test]
fn validate_ignores_non_html_files_under_pages() {
    let root = temp_dir("validate-assets");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(workspace_dir.join("style.css"), "").expect("write stylesheet");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: MANIFEST_VERSION,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
    fs::write(
        pages_dir.join("index.html"),
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    )
    .expect("write index page");
    fs::write(pages_dir.join("asset.txt"), "asset").expect("write asset");

    validate_project(root.path()).expect("validate project");
}

#[test]
fn page_destination_is_relative_to_pages_root() {
    let destination = resolve_page_destination(Path::new("."), Path::new("nested/secondpage"))
        .expect("resolve destination");

    assert_eq!(destination, PathBuf::from("./pages/nested/secondpage.html"));
}

#[test]
fn page_destination_accepts_optional_pages_prefix() {
    let destination = resolve_page_destination(Path::new("."), Path::new("pages/nested/topic"))
        .expect("resolve destination");

    assert_eq!(destination, PathBuf::from("./pages/nested/topic.html"));
}

#[test]
fn page_destination_rejects_parent_traversal() {
    let error = resolve_page_destination(Path::new("."), Path::new("../escape"))
        .expect_err("parent traversal should be rejected");

    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert_eq!(error.to_string(), "page path cannot contain `..`");
}

#[test]
fn page_destination_accepts_lowercase_unicode_and_underscores() {
    let destination = resolve_page_destination(Path::new("."), Path::new("göteborg/my_new_page"))
        .expect("resolve destination");

    assert_eq!(
        destination,
        PathBuf::from("./pages/göteborg/my_new_page.html")
    );
}

#[test]
fn page_destination_rejects_uppercase_unicode() {
    let error = resolve_page_destination(Path::new("."), Path::new("Göteborg"))
        .expect_err("uppercase page path should be rejected");

    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert_eq!(
        error.to_string(),
        "page path component must be a lowercase page slug: Göteborg"
    );
}

#[test]
fn note_id_normalizes_trigger_text() {
    assert_eq!(
        note_id_from_trigger("Andromeda Galaxy").expect("normalize trigger"),
        "note-andromeda-galaxy"
    );
    assert_eq!(
        note_id_from_trigger("JAVA").expect("normalize trigger"),
        "note-java"
    );
}

#[test]
fn new_page_rebuilds_index() {
    let project = TestProject::new("new-page-index");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let report = new_page(project.root(), Path::new("secondpage")).expect("create new page");
    let created_page = PathBuf::from("pages/secondpage.html");
    assert_eq!(
        report.events.first(),
        Some(&OperationEvent::PageCreated {
            path: created_page.clone()
        })
    );
    let summary = report.summary();
    assert!(summary.source_changed);
    assert!(summary.generated_changed);
    assert_eq!(summary.created_paths, vec![created_page.clone()]);
    assert_eq!(summary.pages_changed, vec![created_page]);

    let index: ProjectIndex = serde_json::from_str(
        &fs::read_to_string(project.workspace_dir().join("index.json")).expect("read index"),
    )
    .expect("parse index");

    assert_eq!(index.version, INDEX_VERSION);
    assert_eq!(
        index.files,
        vec![
            FileEntry {
                path: "index.html".to_string(),
                kind: "page".to_string(),
            },
            FileEntry {
                path: "secondpage.html".to_string(),
                kind: "page".to_string(),
            },
        ]
    );
    assert_eq!(
        index.pages,
        vec![
            PageEntry {
                path: "index.html".to_string(),
                title: "index".to_string(),
                meta: required_meta(),
                notes: vec![],
                links: vec![],
            },
            PageEntry {
                path: "secondpage.html".to_string(),
                title: "secondpage".to_string(),
                meta: required_meta(),
                notes: vec![],
                links: vec![],
            }
        ]
    );
}

#[test]
fn create_directory_then_create_page_in_directory() {
    let project = TestProject::new("directory-page-create");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let report =
        create_directory(project.root(), Path::new(""), "research").expect("create directory");
    assert_eq!(
        report.events,
        vec![OperationEvent::DirectoryCreated {
            path: PathBuf::from("pages/research")
        }]
    );

    create_page(
        project.root(),
        PageCreate {
            directory: Some(PathBuf::from("research")),
            title: "Rust Ownership".to_string(),
        },
    )
    .expect("create page in existing directory");

    assert!(project
        .pages_dir()
        .join("research/rust-ownership.html")
        .is_file());
}

#[test]
fn create_page_does_not_create_missing_parent_directories() {
    let project = TestProject::new("page-missing-directory");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = create_page(
        project.root(),
        PageCreate {
            directory: Some(PathBuf::from("missing")),
            title: "Rust Ownership".to_string(),
        },
    )
    .expect_err("missing directory should be rejected");

    assert_eq!(error.code, FractalErrorCode::NotFound);
    assert!(!project.pages_dir().join("missing").exists());
}

#[test]
fn page_source_round_trip_rebuilds_index_and_serializes_report() {
    let project = TestProject::new("page-source");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let source =
        read_page_source(project.root(), Path::new("pages/index")).expect("read page source");
    assert_eq!(source.path, "index.html");
    assert!(source.html.contains("<p>body</p>"));

    let updated = source.html.replace("<p>body</p>", "<p>updated</p>");
    let report =
        write_page_source(project.root(), Path::new("index"), updated).expect("write page source");

    assert!(fs::read_to_string(project.pages_dir().join("index.html"))
        .expect("read updated page")
        .contains("<p>updated</p>"));
    assert_eq!(
        load_project_index(project.root())
            .expect("load generated index")
            .pages[0]
            .title,
        "index"
    );

    let report_json = serde_json::to_value(&report).expect("serialize report");
    assert_eq!(report_json["events"][0]["type"], "page_source_updated");
    assert_eq!(report_json["events"][0]["page"], "pages/index.html");

    let summary = report.summary();
    assert!(!summary.noop);
    assert!(summary.source_changed);
    assert!(summary.source_files_changed);
    assert!(summary.user_content_changed);
    assert!(summary.generated_changed);
    assert!(summary.generated_files_changed);
    assert_eq!(
        summary.pages_changed,
        vec![PathBuf::from("pages/index.html")]
    );
    assert_eq!(
        summary.source_paths_changed,
        vec![PathBuf::from("pages/index.html")]
    );
    assert_eq!(
        summary.generated_paths_changed,
        vec![
            PathBuf::from(".fractal/index.json"),
            PathBuf::from(".fractal/graph.json")
        ]
    );

    let summary_json = serde_json::to_value(summary).expect("serialize summary");
    assert_eq!(summary_json["noop"], false);
    assert_eq!(summary_json["user_content_changed"], true);
    assert_eq!(summary_json["source_paths_changed"][0], "pages/index.html");
    assert_eq!(
        summary_json["generated_paths_changed"],
        serde_json::json!([".fractal/index.json", ".fractal/graph.json"])
    );
}

#[test]
fn same_content_source_write_reports_noop_when_generated_data_is_fresh() {
    let project = TestProject::new("same-content-source-write");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    build_index(project.root()).expect("build initial generated data");
    let generated_report = build_index(project.root()).expect("rebuild unchanged generated data");
    assert_eq!(generated_report.events, Vec::<OperationEvent>::new());
    assert!(generated_report.summary().noop);

    let source = read_page_source(project.root(), Path::new("index")).expect("read page source");
    let report = write_page_source(project.root(), Path::new("index"), source.html)
        .expect("write same page source");

    assert_eq!(report.events, Vec::<OperationEvent>::new());
    let summary = report.summary();
    assert!(summary.noop);
    assert!(!summary.source_files_changed);
    assert!(!summary.user_content_changed);
    assert!(!summary.generated_files_changed);
    assert_eq!(summary.changed_paths, Vec::<PathBuf>::new());

    let summary_json = serde_json::to_value(summary).expect("serialize no-op summary");
    assert_eq!(summary_json["noop"], true);
    assert_eq!(summary_json["changed_paths"], serde_json::json!([]));
}

#[test]
fn editor_page_api_lists_pages_and_returns_detail_data() {
    let project = TestProject::new("editor-page-api");
    let home = render_page_document(
        "Home",
        "<p><a href=\"rust.html\" data-fractal-link=\"page\">Rust</a> and <a href=\"#note-java\" data-fractal-link=\"note\">Java</a>.</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );
    let home = insert_note_into_document(&home, &render_note_aside("note-java", "note body"))
        .expect("insert note");
    project.write_page("index.html", home);
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p><a href=\"index.html\" data-fractal-link=\"page\">Home</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let pages = list_editor_pages(project.root()).expect("list editor pages");
    assert_eq!(
        pages,
        vec![
            EditorPageListEntry {
                path: "index.html".to_string(),
                title: "Home".to_string(),
                summary: None,
                tags: vec![],
                backlink_count: 1,
                outlink_count: 1,
            },
            EditorPageListEntry {
                path: "rust.html".to_string(),
                title: "Rust".to_string(),
                summary: None,
                tags: vec![],
                backlink_count: 1,
                outlink_count: 1,
            },
        ]
    );

    let detail = editor_page_detail(project.root(), Path::new("index")).expect("page detail");
    assert_eq!(detail.source.path, "index.html");
    assert!(detail.source.html.contains("href=\"rust.html\""));
    assert!(detail.source.html.contains("data-fractal-link=\"page\""));
    assert!(detail.body_html.contains("Java"));
    assert_eq!(detail.metadata.title, "Home");
    assert_eq!(
        detail.notes,
        vec![EditorNoteDetail {
            id: "note-java".to_string(),
            label: "java".to_string(),
            text: "note body".to_string(),
        }]
    );
    assert_eq!(
        detail.links,
        vec![
            EditorLinkDetail {
                href: "rust.html".to_string(),
                text: "Rust".to_string(),
                scope: "page".to_string(),
                target_page: Some("rust.html".to_string()),
                target_note: None,
            },
            EditorLinkDetail {
                href: "#note-java".to_string(),
                text: "Java".to_string(),
                scope: "note".to_string(),
                target_page: None,
                target_note: Some("note-java".to_string()),
            },
        ]
    );
    assert_eq!(
        detail.outlinks,
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Rust".to_string(),
        }]
    );
    assert_eq!(
        detail.backlinks,
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Home".to_string(),
        }]
    );
}

#[test]
fn project_summary_reports_validation_counts_and_generated_freshness() {
    let project = TestProject::new("project-summary");
    let home = render_page_document(
        "Home",
        "<p><a href=\"rust.html\" data-fractal-link=\"page\">Rust</a></p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );
    let home = insert_note_into_document(&home, &render_note_aside("note-java", "note body"))
        .expect("insert note");
    project.write_page("index.html", home);
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    fs::write(project.pages_dir().join("asset.txt"), "asset").expect("write asset");
    build_index(project.root()).expect("build generated data");

    let summary = project_summary(project.root()).expect("project summary");

    assert_eq!(summary.project_name, "test");
    assert_eq!(summary.default_page, "pages/index.html");
    assert!(summary.valid);
    assert_eq!(summary.validation_error, None);
    assert_eq!(summary.file_count, 3);
    assert_eq!(summary.page_count, 2);
    assert_eq!(summary.asset_count, 1);
    assert_eq!(summary.note_count, 1);
    assert_eq!(summary.link_count, 1);
    assert_eq!(summary.graph_node_count, 3);
    assert_eq!(summary.graph_edge_count, 2);
    assert_eq!(summary.orphan_page_count, 1);
    assert!(summary.generated_index_exists);
    assert!(summary.generated_graph_exists);
    assert!(summary.generated_index_fresh);
    assert!(summary.generated_graph_fresh);

    project.write_page(
        "java.html",
        render_page_document(
            "Java",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let stale_summary = project_summary(project.root()).expect("stale project summary");

    assert_eq!(stale_summary.page_count, 3);
    assert!(!stale_summary.generated_index_fresh);
    assert!(!stale_summary.generated_graph_fresh);
}

#[test]
fn safe_editor_page_update_mutates_owned_fields_and_rebuilds_index() {
    let project = TestProject::new("safe-editor-update");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>Original body.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>Target page.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let report = update_editor_page(
        project.root(),
        Path::new("index"),
        EditorPageUpdate {
            title: Some("Home Base".to_string()),
            body_html: Some(
                "<p><a href=\"rust.html\" data-fractal-link=\"page\">Rust</a> updated.</p>"
                    .to_string(),
            ),
            summary: Some("Local project home".to_string()),
            tags: Some(vec![
                "Projects".to_string(),
                "rust, projects".to_string(),
                " ".to_string(),
            ]),
        },
    )
    .expect("update editor page");

    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageTitleUpdated { title, .. } if title == "Home Base"
        )
    }));
    assert!(report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::PageContentUpdated { .. })));
    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageMetadataUpdated { name, content, .. }
                if name == "fractal:summary" && content == "Local project home"
        )
    }));
    assert!(report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::GeneratedIndexBuilt { .. })));

    let detail = editor_page_detail(project.root(), Path::new("index")).expect("page detail");
    assert_eq!(detail.metadata.title, "Home Base");
    assert_eq!(
        detail.metadata.summary,
        Some("Local project home".to_string())
    );
    assert_eq!(
        detail.metadata.tags,
        vec!["Projects".to_string(), "rust".to_string()]
    );
    assert!(detail.source.html.contains("<title>Home Base</title>"));
    assert!(detail.source.html.contains("<h1>Home Base</h1>"));
    assert!(detail.body_html.contains("Rust"));
    assert_eq!(
        detail.links,
        vec![EditorLinkDetail {
            href: "rust.html".to_string(),
            text: "Rust".to_string(),
            scope: "page".to_string(),
            target_page: Some("rust.html".to_string()),
            target_note: None,
        }]
    );
}

#[test]
fn safe_editor_page_update_repairs_editor_html_links_before_validating() {
    let project = TestProject::new("safe-editor-repairs-links");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>Original body.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>Target page.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let report = update_editor_page(
        project.root(),
        Path::new("index"),
        EditorPageUpdate {
            body_html: Some("<p><a href=\"rust.html\">Rust</a> stayed readable.</p>".to_string()),
            ..EditorPageUpdate::default()
        },
    )
    .expect("repair editor body links");

    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageLinksRewritten { count, .. } if *count == 1
        )
    }));

    let detail = editor_page_detail(project.root(), Path::new("index")).expect("page detail");
    assert!(detail.body_html.contains("Rust stayed readable."));
    assert!(detail.links.is_empty());

    sync_project(project.root()).expect("sync generated links");
    let detail = editor_page_detail(project.root(), Path::new("index")).expect("synced detail");
    assert_eq!(
        detail.links,
        vec![EditorLinkDetail {
            href: "rust.html".to_string(),
            text: "Rust".to_string(),
            scope: "page".to_string(),
            target_page: Some("rust.html".to_string()),
            target_note: None,
        }]
    );
}

#[test]
fn safe_page_editing_rejects_duplicate_title_before_writing() {
    let project = TestProject::new("safe-editor-duplicate-title");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = set_page_title(project.root(), Path::new("index"), "rust")
        .expect_err("duplicate title should fail");
    assert!(error.to_string().contains("duplicate page label"));
    assert!(fs::read_to_string(project.pages_dir().join("index.html"))
        .expect("read index")
        .contains("<title>Home</title>"));

    update_page_body(project.root(), Path::new("index"), "<p>safe body</p>").expect("update body");
    assert!(fs::read_to_string(project.pages_dir().join("index.html"))
        .expect("read updated index")
        .contains("<p>safe body</p>"));
}

#[test]
fn preflight_rename_page_reports_affected_state_without_writing() {
    let project = TestProject::new("preflight-rename-page");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"rust.html\" data-fractal-link=\"page\">Rust</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "links.html",
        render_page_document(
            "Links",
            "<p><a href=\"index.html\" data-fractal-link=\"page\">Home</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let preflight = preflight_rename_page(
        project.root(),
        Path::new("index"),
        PageRename {
            path: Some(PathBuf::from("folder/home")),
            title: Some("Home Base".to_string()),
        },
    )
    .expect("preflight rename");

    assert_eq!(preflight.source_page, "index.html");
    assert_eq!(preflight.destination_page, "folder/home.html");
    assert_eq!(
        preflight.source_path,
        project.pages_dir().join("index.html")
    );
    assert_eq!(
        preflight.destination_path,
        project.pages_dir().join("folder/home.html")
    );
    assert_eq!(preflight.title, "Home Base");
    assert!(preflight.path_changed);
    assert!(preflight.title_changed);
    assert!(preflight.updates_default_page);
    assert_eq!(
        preflight.backlinks,
        vec![GraphPageLink {
            page: "links.html".to_string(),
            text: "Home".to_string(),
        }]
    );
    assert_eq!(
        preflight.outlinks,
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Rust".to_string(),
        }]
    );
    assert!(project.pages_dir().join("index.html").is_file());
    assert!(!project.pages_dir().join("folder/home.html").exists());
}

#[test]
fn rename_page_moves_page_updates_title_manifest_and_generated_data() {
    let project = TestProject::new("rename-page");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            r#"<p><a href="links.html" data-fractal-link="page">Links</a></p>
      <p><a href="index.html#intro" data-fractal-link="page">Home</a></p>"#,
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "links.html",
        render_page_document(
            "Links",
            r#"<p><a href="index.html#intro" data-fractal-link="page">Home</a></p>
      <p>External</p>"#,
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "folder/topic.html",
        render_page_document(
            "Topic",
            r#"<p><a href="../index.html?view=1" data-fractal-link="page">Home</a></p>"#,
            Theme::Dark,
            "../../.fractal/style.css".to_string(),
        ),
    );

    let report = rename_page(
        project.root(),
        Path::new("index"),
        PageRename {
            path: Some(PathBuf::from("folder/home")),
            title: Some("Home Base".to_string()),
        },
    )
    .expect("rename page");

    let source = project.pages_dir().join("index.html");
    let destination = project.pages_dir().join("folder/home.html");
    let source_report_path = PathBuf::from("pages/index.html");
    let destination_report_path = PathBuf::from("pages/folder/home.html");
    assert!(!source.exists());
    assert!(destination.is_file());
    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageMoved { from, to }
                if from == &source_report_path && to == &destination_report_path
        )
    }));
    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageTitleUpdated { page, title }
                if page == &destination_report_path && title == "Home Base"
        )
    }));
    assert!(report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::ManifestUpdated { .. })));
    assert!(report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::PageLinksRewritten { .. })));
    let summary = report.summary();
    assert!(summary.source_changed);
    assert!(summary.generated_changed);
    assert!(summary.manifest_changed);
    assert_eq!(summary.moved_paths[0].from, source_report_path);
    assert_eq!(summary.moved_paths[0].to, destination_report_path);

    let html = fs::read_to_string(&destination).expect("read moved page");
    assert!(html.contains("<title>Home Base</title>"));
    assert!(html.contains("<h1>Home Base</h1>"));
    assert!(html.contains("href=\"../../.fractal/style.css\""));
    assert!(html.contains("href=\"../links.html\""));
    assert!(html.contains("href=\"home.html#intro\""));
    let links_html =
        fs::read_to_string(project.pages_dir().join("links.html")).expect("read links");
    assert!(links_html.contains("href=\"folder/home.html#intro\""));
    assert!(links_html.contains("<p>External</p>"));
    let topic_html =
        fs::read_to_string(project.pages_dir().join("folder/topic.html")).expect("read topic");
    assert!(topic_html.contains("href=\"home.html?view=1\""));
    assert_eq!(
        load_project_manifest(project.root())
            .expect("load manifest")
            .default_page,
        "pages/folder/home.html"
    );
    assert_eq!(
        load_project_index(project.root())
            .expect("load index")
            .pages[0]
            .path,
        "folder/home.html"
    );
    validate_project(project.root()).expect("renamed project validates");
}

#[test]
fn rename_page_rejects_label_collision_before_writing() {
    let project = TestProject::new("rename-page-collision");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = rename_page(
        project.root(),
        Path::new("index"),
        PageRename {
            path: Some(PathBuf::from("folder/rust")),
            title: Some("Rust".to_string()),
        },
    )
    .expect_err("duplicate future labels should fail");

    assert!(error.to_string().contains("duplicate page label"));
    assert!(project.pages_dir().join("index.html").is_file());
    assert!(!project.pages_dir().join("folder/rust.html").exists());
    assert!(fs::read_to_string(project.pages_dir().join("index.html"))
        .expect("read original page")
        .contains("<title>Home</title>"));
}

#[test]
fn delete_directory_removes_nested_pages_and_rebuilds_index() {
    let project = TestProject::new("delete-directory");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"folder/rust.html\" data-fractal-link=\"page\">Rust</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "folder/rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../../.fractal/style.css".to_string(),
        ),
    );

    let report = delete_directory(project.root(), Path::new("folder"), true)
        .expect("delete directory recursively");

    assert!(!project.pages_dir().join("folder").exists());
    assert!(report.events.iter().any(|event| matches!(
        event,
        OperationEvent::DirectoryDeleted { path } if path == Path::new("pages/folder")
    )));
    assert!(report.events.iter().any(|event| matches!(
        event,
        OperationEvent::PageDeleted { path } if path == Path::new("pages/folder/rust.html")
    )));
    let summary = report.summary();
    assert_eq!(
        summary.deleted_paths,
        vec![
            PathBuf::from("pages/folder"),
            PathBuf::from("pages/folder/rust.html")
        ]
    );
    assert_eq!(
        summary.pages_changed,
        vec![
            PathBuf::from("pages/folder/rust.html"),
            PathBuf::from("pages/index.html")
        ]
    );
    assert!(fs::read_to_string(project.pages_dir().join("index.html"))
        .expect("read index")
        .contains("<p>Rust</p>"));
    let index = load_project_index(project.root()).expect("load index");
    assert_eq!(
        index
            .pages
            .iter()
            .map(|page| page.path.as_str())
            .collect::<Vec<_>>(),
        vec!["index.html"]
    );
}

#[test]
fn preflight_delete_page_reports_default_and_links_without_writing() {
    let project = TestProject::new("preflight-delete-page");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"rust.html\" data-fractal-link=\"page\">Rust</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p><a href=\"java.html\" data-fractal-link=\"page\">Java</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "java.html",
        render_page_document(
            "Java",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let preflight =
        preflight_delete_page(project.root(), Path::new("rust")).expect("preflight delete");

    assert_eq!(preflight.page, "rust.html");
    assert_eq!(preflight.path, project.pages_dir().join("rust.html"));
    assert!(!preflight.deleting_default);
    assert_eq!(preflight.replacement_default_page, None);
    assert_eq!(
        preflight.backlinks,
        vec![GraphPageLink {
            page: "index.html".to_string(),
            text: "Rust".to_string(),
        }]
    );
    assert_eq!(
        preflight.outlinks,
        vec![GraphPageLink {
            page: "java.html".to_string(),
            text: "Java".to_string(),
        }]
    );
    assert!(project.pages_dir().join("rust.html").is_file());
}

#[test]
fn delete_page_removes_file_rebuilds_generated_data_and_reports_links() {
    let project = TestProject::new("delete-page");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p><a href=\"rust.html\" data-fractal-link=\"page\">Rust</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p><a href=\"java.html\" data-fractal-link=\"page\">Java</a></p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "java.html",
        render_page_document(
            "Java",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let report = delete_page(project.root(), Path::new("rust")).expect("delete page");

    assert!(!project.pages_dir().join("rust.html").exists());
    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageLinkImpact {
                page,
                backlinks,
                outlinks,
            } if page == "rust.html"
                && backlinks == &vec![GraphPageLink {
                    page: "index.html".to_string(),
                    text: "Rust".to_string(),
                }]
                && outlinks == &vec![GraphPageLink {
                    page: "java.html".to_string(),
                    text: "Java".to_string(),
                }]
        )
    }));
    let deleted_page = PathBuf::from("pages/rust.html");
    assert!(report.events.iter().any(|event| {
        matches!(
            event,
            OperationEvent::PageDeleted { path }
                if path == &deleted_page
        )
    }));
    let summary = report.summary();
    assert!(summary.source_changed);
    assert!(summary.generated_changed);
    assert_eq!(summary.deleted_paths, vec![deleted_page.clone()]);
    assert!(summary.pages_changed.contains(&deleted_page));
    assert_eq!(summary.links_rewritten_count, 1);
    assert_eq!(summary.warnings, Vec::<String>::new());

    assert_eq!(
        load_project_index(project.root())
            .expect("load generated index")
            .pages
            .into_iter()
            .map(|page| page.path)
            .collect::<Vec<_>>(),
        vec!["index.html".to_string(), "java.html".to_string()]
    );
    validate_project(project.root()).expect("project validates after delete");
}

#[test]
fn delete_page_updates_manifest_default_when_needed() {
    let project = TestProject::new("delete-default-page");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "second.html",
        render_page_document(
            "Second",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let report = delete_page(project.root(), Path::new("index")).expect("delete default page");

    assert!(!project.pages_dir().join("index.html").exists());
    assert_eq!(
        load_project_manifest(project.root())
            .expect("load manifest")
            .default_page,
        "pages/second.html"
    );
    assert!(report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::ManifestUpdated { .. })));
    validate_project(project.root()).expect("project validates after default delete");
}

#[test]
fn delete_page_rejects_removing_only_page() {
    let project = TestProject::new("delete-only-page");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error =
        delete_page(project.root(), Path::new("index")).expect_err("only page delete should fail");

    assert!(error.to_string().contains("cannot delete the only page"));
    assert!(project.pages_dir().join("index.html").is_file());
}

#[test]
fn extracts_all_fractal_meta_tags() {
    let root = temp_dir("extract-meta");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    fs::write(
            &page,
            "<meta name=\"fractal:version\" content=\"0.1\" />\n<meta name=\"fractal:summary\" content=\"Short page summary here.\" />\n<meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" />\n<meta name=\"fractal:extra\" content=\"alpha &amp; beta\" />\n<meta name=\"description\" content=\"ignore me\" />",
        )
        .expect("write page");

    let meta = PageDocument::from_path(&page)
        .expect("parse page")
        .fractal_meta();

    assert_eq!(
        meta,
        [
            ("fractal:extra".to_string(), "alpha & beta".to_string()),
            (
                "fractal:summary".to_string(),
                "Short page summary here.".to_string()
            ),
            (
                "fractal:tags".to_string(),
                "rust, graphs, parsing".to_string()
            ),
            ("fractal:version".to_string(), "0.1".to_string()),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn page_document_extracts_from_ordinary_html_formatting() {
    let document = PageDocument::parse(
        r#"<!doctype html>
<html>
  <head>
    <META NAME='fractal:version' CONTENT='0.1'>
    <meta content='Summary &amp; detail' name='fractal:summary'>
    <title>Flexible   Page</title>
  </head>
  <body>
    <main>
      <p><a HREF='topic.html'><strong>Topic</strong></a></p>
    </main>
    <section data-fractal-notes>
      <aside data-fractal-note id='note-java' data-fractal-trigger='Java language'></aside>
    </section>
  </body>
</html>"#,
    );

    assert!(document.has_notes_section());
    assert_eq!(document.title(), Some("Flexible Page".to_string()));
    assert_eq!(
        document.fractal_meta(),
        [
            (
                "fractal:summary".to_string(),
                "Summary & detail".to_string()
            ),
            ("fractal:version".to_string(), "0.1".to_string()),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        document.notes(),
        vec![NoteEntry {
            id: "note-java".to_string(),
            label: "Java language".to_string(),
        }]
    );
    assert_eq!(
        document.links(),
        vec![LinkEntry {
            href: "topic.html".to_string(),
            text: "Topic".to_string(),
            scope: "page".to_string(),
        }]
    );
}

#[test]
fn add_note_creates_notes_section_in_requested_page() {
    let project = TestProject::new("add-note");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    add_note(project.root(), Path::new("index"), "java", "my note text").expect("add note");
    let error = add_note(project.root(), Path::new("index"), "java", "duplicate")
        .expect_err("duplicate note should fail");
    assert_eq!(error.code, FractalErrorCode::AlreadyExists);

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read page");
    let document = PageDocument::parse(&html);
    assert_eq!(document.notes_section_count(), 1);
    assert_eq!(
        document.notes(),
        vec![NoteEntry {
            id: "note-java".to_string(),
            label: "java".to_string(),
        }]
    );
    assert!(html.contains("<p>my note text</p>"));
}

#[test]
fn patch_note_updates_existing_note() {
    let project = TestProject::new("patch-note");
    let page = insert_note_into_document(
        &render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
        &render_note_aside("note-java", "old text"),
    )
    .expect("insert note");
    project.write_page("index.html", page);

    patch_note(project.root(), Path::new("index"), "java", "new text").expect("patch note");

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read page");
    assert!(html.contains("<p>new text</p>"));
    assert!(!html.contains("<p>old text</p>"));
}

#[test]
fn remove_note_keeps_empty_note_section() {
    let project = TestProject::new("remove-note");
    let page = insert_note_into_document(
        &render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
        &render_note_aside("note-java", "old text"),
    )
    .expect("insert note");
    project.write_page("index.html", page);

    remove_note(project.root(), Path::new("index"), "java").expect("remove note");

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read page");
    let document = PageDocument::parse(&html);
    assert!(!html.contains("note-java"));
    assert_eq!(document.notes_section_count(), 1);
}

#[test]
fn page_metadata_reads_summary_tags_and_extra_fractal_meta() {
    let project = TestProject::new("metadata-read");
    project.write_page(
        "index.html",
        r#"<!doctype html>
<html>
  <head>
    <title>Knowledge Base</title>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="Graph notes">
    <meta name="fractal:tags" content="rust, Graphs, rust">
    <meta name="fractal:extra" content="kept">
  </head>
  <body>
    <main><p>body</p></main>
    <section data-fractal-notes></section>
  </body>
</html>
"#,
    );

    let metadata = page_metadata(project.root(), Path::new("index")).expect("read metadata");

    assert_eq!(metadata.path, "index.html");
    assert_eq!(metadata.title, "Knowledge Base");
    assert_eq!(metadata.summary, Some("Graph notes".to_string()));
    assert_eq!(
        metadata.tags,
        vec!["rust".to_string(), "Graphs".to_string()]
    );
    assert_eq!(
        metadata.meta.get("fractal:extra").map(String::as_str),
        Some("kept")
    );
}

#[test]
fn set_page_metadata_updates_meta_tags_and_rebuilds_index() {
    let project = TestProject::new("metadata-set");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let summary_report = set_page_summary(project.root(), Path::new("index"), "Local graph notes")
        .expect("set summary");
    assert!(summary_report.events.iter().any(|event| matches!(
        event,
        OperationEvent::PageMetadataUpdated { name, content, .. }
            if name == "fractal:summary" && content == "Local graph notes"
    )));

    set_page_tags(
        project.root(),
        Path::new("index"),
        ["rust", "graphs, parsing", "rust"],
    )
    .expect("set tags");

    let metadata = page_metadata(project.root(), Path::new("index")).expect("read metadata");
    assert_eq!(metadata.summary, Some("Local graph notes".to_string()));
    assert_eq!(
        metadata.tags,
        vec![
            "rust".to_string(),
            "graphs".to_string(),
            "parsing".to_string()
        ]
    );

    let index = load_project_index(project.root()).expect("load generated index");
    assert_eq!(
        index.pages[0]
            .meta
            .get("fractal:summary")
            .map(String::as_str),
        Some("Local graph notes")
    );
    assert_eq!(
        index.pages[0].meta.get("fractal:tags").map(String::as_str),
        Some("rust, graphs, parsing")
    );
}

#[test]
fn reset_page_metadata_restores_generated_defaults() {
    let project = TestProject::new("metadata-reset");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    set_page_summary(project.root(), Path::new("index"), "Custom summary").expect("set summary");
    set_page_tags(project.root(), Path::new("index"), ["custom"]).expect("set tags");

    reset_page_metadata(project.root(), Path::new("index")).expect("reset metadata");

    let metadata = page_metadata(project.root(), Path::new("index")).expect("read metadata");
    assert_eq!(metadata.summary, None);
    assert_eq!(metadata.tags, Vec::<String>::new());
}

#[test]
fn page_metadata_report_prints_editor_friendly_summary() {
    let project = TestProject::new("metadata-report");
    project.write_page(
        "index.html",
        render_page_document(
            "index",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    assert_eq!(
        page_metadata_report(project.root(), Path::new("index")).expect("metadata report"),
        "index.html\ntitle: index\nsummary: (none)\ntags: (none)\n"
    );
}

#[test]
fn note_mutation_handles_ordinary_html_formatting() {
    let project = TestProject::new("messy-note-mutation");
    project.write_page(
        "index.html",
        r#"<!doctype html>
<HTML lang="en">
  <HEAD>
    <TITLE>Index</TITLE>
    <meta content="0.1" name="fractal:version">
    <meta content="" name="fractal:summary">
    <meta content="" name="fractal:tags">
  </HEAD>
  <BODY>
    <main><p>Java and Rust are both mentioned.</p></main>
    <section data-fractal-notes>
      <aside data-fractal-note id='note-rust'><p>old rust</p></aside>
    </section>
  </BODY>
</HTML>
"#,
    );

    add_note(project.root(), Path::new("index"), "Java", "java & <note>").expect("add note");
    patch_note(project.root(), Path::new("index"), "rust", "patched <rust>").expect("patch note");
    remove_note(project.root(), Path::new("index"), "Java").expect("remove note");

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read page");
    let document = PageDocument::parse(&html);
    assert_eq!(document.notes_section_count(), 1);
    assert_eq!(
        document.notes(),
        vec![NoteEntry {
            id: "note-rust".to_string(),
            label: "rust".to_string(),
        }]
    );
    assert!(html.contains("patched &lt;rust&gt;"));
    assert!(!html.contains("note-java"));
}

#[test]
fn sync_rebuilds_index_and_links_notes_before_project_pages() {
    let project = TestProject::new("sync-links");

    let index = render_page_document(
        "Home",
        "<p>Java and Rust are mentioned here.</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );
    let index = insert_note_into_document(&index, &render_note_aside("note-java", "note body"))
        .expect("insert note");
    project.write_page("index.html", index);
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>Home mentions Java.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    fs::write(project.pages_dir().join("asset.txt"), "asset").expect("write asset");

    let first_report = sync_project(project.root()).expect("sync project");
    let second_report = sync_project(project.root()).expect("sync project twice");
    assert!(first_report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::SyncCompleted { pages_updated: 2 })));
    assert!(second_report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::SyncCompleted { pages_updated: 0 })));
    let first_summary = first_report.summary();
    assert!(!first_summary.noop);
    assert!(first_summary.source_files_changed);
    assert!(!first_summary.user_content_changed);
    assert!(first_summary.links_rewritten_count > 0);
    assert!(second_report.summary().noop);

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read index page");
    assert_eq!(html.matches("data-fractal-link=\"note\"").count(), 1);
    assert_eq!(html.matches("data-fractal-link=\"page\"").count(), 1);
    let links = PageDocument::parse(&html).links();
    assert!(links.contains(&LinkEntry {
        href: "#note-java".to_string(),
        text: "Java".to_string(),
        scope: "note".to_string(),
    }));
    assert!(links.contains(&LinkEntry {
        href: "rust.html".to_string(),
        text: "Rust".to_string(),
        scope: "page".to_string(),
    }));

    let index: ProjectIndex = serde_json::from_str(
        &fs::read_to_string(project.workspace_dir().join("index.json")).expect("read index"),
    )
    .expect("parse index");
    assert_eq!(index.version, INDEX_VERSION);
    assert_eq!(
        index.files,
        vec![
            FileEntry {
                path: "asset.txt".to_string(),
                kind: "asset".to_string(),
            },
            FileEntry {
                path: "index.html".to_string(),
                kind: "page".to_string(),
            },
            FileEntry {
                path: "rust.html".to_string(),
                kind: "page".to_string(),
            },
        ]
    );
    assert_eq!(
        index.pages[0].notes,
        vec![NoteEntry {
            id: "note-java".to_string(),
            label: "java".to_string(),
        }]
    );
    assert_eq!(
        index.pages[0].links,
        vec![
            LinkEntry {
                href: "#note-java".to_string(),
                text: "Java".to_string(),
                scope: "note".to_string(),
            },
            LinkEntry {
                href: "rust.html".to_string(),
                text: "Rust".to_string(),
                scope: "page".to_string(),
            },
        ]
    );

    let graph: ProjectGraph = serde_json::from_str(
        &fs::read_to_string(project.workspace_dir().join("graph.json")).expect("read graph"),
    )
    .expect("parse graph");
    assert!(graph.nodes.iter().any(|node| node.id == "page:index.html"));
    assert!(graph.nodes.iter().any(|node| node.id == "page:rust.html"));
    assert!(graph
        .nodes
        .iter()
        .any(|node| node.id == "note:index.html#note-java"));
    assert!(graph.edges.iter().any(|edge| {
        edge.from == "page:index.html"
            && edge.to == "note:index.html#note-java"
            && edge.kind == "links_to_note"
    }));

    let index_page_graph = graph
        .pages
        .iter()
        .find(|page| page.path == "index.html")
        .expect("index page graph");
    assert_eq!(
        index_page_graph.outlinks,
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Rust".to_string(),
        }]
    );
    assert_eq!(
        index_page_graph.backlinks,
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Home".to_string(),
        }]
    );
}

#[test]
fn index_rejects_duplicate_page_labels_case_insensitively() {
    let project = TestProject::new("duplicate-page-labels");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "alpha.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "beta.html",
        render_page_document(
            "rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error = build_index(project.root()).expect_err("duplicate page labels should fail");
    assert!(error.to_string().contains("duplicate page label"));

    let report =
        validate_project(project.root()).expect("duplicate project validates with warning");
    assert!(report.events.iter().any(|event| matches!(
        event,
        OperationEvent::Warning { message } if message.contains("duplicate page label")
    )));
}

#[test]
fn new_page_rejects_existing_page_label_before_writing() {
    let project = TestProject::new("new-page-duplicate-label");
    project.write_page(
        "topic.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    let error =
        new_page(project.root(), Path::new("rust")).expect_err("new page label should be rejected");

    assert!(error.to_string().contains("duplicate page label"));
    assert!(!project.pages_dir().join("rust.html").exists());
}

#[test]
fn sync_links_ordinary_html_without_touching_manual_or_code_links() {
    let project = TestProject::new("sync-ordinary-html");
    project.write_page(
        "index.html",
        r#"<!doctype html>
<HTML lang="en">
  <HEAD>
    <TITLE>Home</TITLE>
    <meta content="0.1" name="fractal:version">
    <meta content="" name="fractal:summary">
    <meta content="" name="fractal:tags">
  </HEAD>
  <BODY>
    <MAIN>
      <p><a href="manual.html">Manual Rust</a> mentions Java, <code>Rust</code>, and <span>Rust</span>.</p>
      <p><a href="old.html" data-fractal-link="page">Rust</a> was generated before.</p>
    </MAIN>
    <section data-fractal-notes>
      <aside data-fractal-note id="note-java"><p>note body</p></aside>
    </section>
  </BODY>
</HTML>
"#,
    );
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    sync_project(project.root()).expect("sync project");

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read index page");
    let links = PageDocument::parse(&html).links();
    assert!(links.contains(&LinkEntry {
        href: "manual.html".to_string(),
        text: "Manual Rust".to_string(),
        scope: "page".to_string(),
    }));
    assert!(links.contains(&LinkEntry {
        href: "#note-java".to_string(),
        text: "Java".to_string(),
        scope: "note".to_string(),
    }));
    assert_eq!(
        links
            .iter()
            .filter(|link| link.href == "rust.html" && link.text == "Rust")
            .count(),
        2
    );
    assert!(!links.iter().any(|link| link.href == "old.html"));
    assert!(html.contains("<code>Rust</code>"));
}

#[test]
fn sync_prefers_page_local_notes_over_same_named_pages_case_insensitively() {
    let project = TestProject::new("sync-note-priority");
    let index = render_page_document(
        "Home",
        "<p>RUST appears here.</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );
    let index = insert_note_into_document(&index, &render_note_aside("note-rust", "note body"))
        .expect("insert note");
    project.write_page("index.html", index);
    project.write_page(
        "rust.html",
        render_page_document(
            "Rust",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );

    sync_project(project.root()).expect("sync project");

    let html = fs::read_to_string(project.pages_dir().join("index.html")).expect("read index page");
    let links = PageDocument::parse(&html).links();
    assert!(links.contains(&LinkEntry {
        href: "#note-rust".to_string(),
        text: "RUST".to_string(),
        scope: "note".to_string(),
    }));
    assert!(!links
        .iter()
        .any(|link| link.href == "rust.html" && link.text == "RUST" && link.scope == "page"));
}

#[test]
fn search_project_finds_indexed_page_fields() {
    let project = TestProject::new("search");
    project.write_page(
        "index.html",
        r#"<!doctype html>
<html lang="en">
  <head>
    <title>Knowledge Base</title>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="Andromeda graph notes.">
    <meta name="fractal:tags" content="space, graph">
  </head>
  <body>
    <main><p><a href="rust.html">Rust engine</a> supports quantum snippets.</p></main>
    <section data-fractal-notes>
      <aside id="note-andromeda-galaxy" data-fractal-note data-fractal-trigger="Andromeda Galaxy"></aside>
    </section>
  </body>
</html>"#,
    );
    build_index(project.root()).expect("build index");

    assert_eq!(
        search_project(project.root(), "andromeda graph").expect("search project"),
        vec![SearchResult {
            path: "index.html".to_string(),
            title: "Knowledge Base".to_string(),
            matches: vec![
                SearchMatch {
                    field: "note".to_string(),
                    text: "Andromeda Galaxy".to_string(),
                },
                SearchMatch {
                    field: "summary".to_string(),
                    text: "Andromeda graph notes.".to_string(),
                },
                SearchMatch {
                    field: "tags".to_string(),
                    text: "space, graph".to_string(),
                },
            ],
        }]
    );
    assert_eq!(
        search_report(project.root(), "rust").expect("search report"),
        "search results for `rust`\n  - index.html (Knowledge Base)\n    body: Rust engine supports quantum snippets.\n    link: Rust engine\n"
    );
    assert_eq!(
        search_project(project.root(), "quantum").expect("body search"),
        vec![SearchResult {
            path: "index.html".to_string(),
            title: "Knowledge Base".to_string(),
            matches: vec![SearchMatch {
                field: "body".to_string(),
                text: "Rust engine supports quantum snippets.".to_string(),
            }],
        }]
    );
}

#[test]
fn graph_page_report_shows_backlinks_and_outlinks() {
    let root = temp_dir("graph-page-report");
    fs::create_dir_all(root.path()).expect("create temp dir");
    write_test_manifest(root.path());
    write_test_graph(
        root.path(),
        vec![PageGraphEntry {
            path: "index.html".to_string(),
            outlinks: vec![GraphPageLink {
                page: "rust.html".to_string(),
                text: "Rust".to_string(),
            }],
            backlinks: vec![GraphPageLink {
                page: "folder/topic.html".to_string(),
                text: "Home".to_string(),
            }],
        }],
    );

    assert_eq!(
        graph_page_report(root.path(), Path::new("pages/index")).expect("page report"),
        "index.html\noutlinks:\n  - rust.html (Rust)\nbacklinks:\n  - folder/topic.html (Home)\n"
    );
    assert_eq!(
        graph_page(root.path(), Path::new("pages/index"))
            .expect("structured page graph")
            .outlinks,
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Rust".to_string(),
        }]
    );
}

#[test]
fn graph_focused_reports_show_basic_page_views() {
    let root = temp_dir("graph-focused-reports");
    fs::create_dir_all(root.path()).expect("create temp dir");
    write_test_manifest(root.path());
    write_test_graph_data(
        root.path(),
        vec![
            GraphNode {
                id: "page:index.html".to_string(),
                kind: "page".to_string(),
                label: "Home".to_string(),
                path: Some("index.html".to_string()),
            },
            GraphNode {
                id: "note:index.html#note-rust".to_string(),
                kind: "note".to_string(),
                label: "Rust".to_string(),
                path: Some("index.html".to_string()),
            },
        ],
        vec![GraphEdge {
            from: "page:index.html".to_string(),
            to: "note:index.html#note-rust".to_string(),
            kind: "contains_note".to_string(),
            text: Some("Rust".to_string()),
            href: Some("#note-rust".to_string()),
        }],
        vec![PageGraphEntry {
            path: "index.html".to_string(),
            outlinks: vec![GraphPageLink {
                page: "rust.html".to_string(),
                text: "Rust".to_string(),
            }],
            backlinks: vec![GraphPageLink {
                page: "folder/topic.html".to_string(),
                text: "Home".to_string(),
            }],
        }],
    );

    assert_eq!(
        page_outlinks(root.path(), Path::new("index")).expect("outlinks"),
        vec![GraphPageLink {
            page: "rust.html".to_string(),
            text: "Rust".to_string(),
        }]
    );
    assert_eq!(
        page_backlinks(root.path(), Path::new("index")).expect("backlinks"),
        vec![GraphPageLink {
            page: "folder/topic.html".to_string(),
            text: "Home".to_string(),
        }]
    );
    assert_eq!(
        related_pages(root.path(), Path::new("index")).expect("related pages"),
        vec![
            GraphRelatedPage {
                page: "folder/topic.html".to_string(),
                text: "Home".to_string(),
                direction: "backlink".to_string(),
            },
            GraphRelatedPage {
                page: "rust.html".to_string(),
                text: "Rust".to_string(),
                direction: "outlink".to_string(),
            },
        ]
    );
    assert_eq!(
        page_notes(root.path(), Path::new("index")).expect("page notes"),
        vec![GraphNoteLink {
            id: "note-rust".to_string(),
            label: "Rust".to_string(),
            href: "#note-rust".to_string(),
        }]
    );

    assert_eq!(
        graph_outlinks_report(root.path(), Path::new("pages/index")).expect("outlinks report"),
        "index.html\noutlinks:\n  - rust.html (Rust)\n"
    );
    assert_eq!(
        graph_backlinks_report(root.path(), Path::new("pages/index")).expect("backlinks report"),
        "index.html\nbacklinks:\n  - folder/topic.html (Home)\n"
    );
    assert_eq!(
        graph_related_report(root.path(), Path::new("pages/index")).expect("related report"),
        "index.html\nrelated pages:\n  - folder/topic.html (backlink: Home)\n  - rust.html (outlink: Rust)\n"
    );
    assert_eq!(
        graph_notes_report(root.path(), Path::new("pages/index")).expect("notes report"),
        "index.html\nnotes:\n  - note-rust (Rust)\n"
    );
}

#[test]
fn graph_neighbors_traverse_depth_limited_page_links() {
    let root = temp_dir("graph-neighbors");
    fs::create_dir_all(root.path()).expect("create temp dir");
    write_test_manifest(root.path());
    write_test_graph(
        root.path(),
        vec![
            PageGraphEntry {
                path: "index.html".to_string(),
                outlinks: vec![GraphPageLink {
                    page: "rust.html".to_string(),
                    text: "Rust".to_string(),
                }],
                backlinks: vec![GraphPageLink {
                    page: "topic.html".to_string(),
                    text: "Home".to_string(),
                }],
            },
            PageGraphEntry {
                path: "rust.html".to_string(),
                outlinks: vec![GraphPageLink {
                    page: "borrow.html".to_string(),
                    text: "Borrow".to_string(),
                }],
                backlinks: vec![GraphPageLink {
                    page: "index.html".to_string(),
                    text: "Rust".to_string(),
                }],
            },
            PageGraphEntry {
                path: "topic.html".to_string(),
                outlinks: vec![GraphPageLink {
                    page: "index.html".to_string(),
                    text: "Home".to_string(),
                }],
                backlinks: Vec::new(),
            },
            PageGraphEntry {
                path: "borrow.html".to_string(),
                outlinks: Vec::new(),
                backlinks: vec![GraphPageLink {
                    page: "rust.html".to_string(),
                    text: "Borrow".to_string(),
                }],
            },
        ],
    );

    assert_eq!(
        neighbor_pages(root.path(), Path::new("index"), 1).expect("depth one neighbors"),
        vec![
            GraphNeighborPage {
                page: "rust.html".to_string(),
                distance: 1,
            },
            GraphNeighborPage {
                page: "topic.html".to_string(),
                distance: 1,
            },
        ]
    );
    assert_eq!(
        neighbor_pages(root.path(), Path::new("index"), 2).expect("depth two neighbors"),
        vec![
            GraphNeighborPage {
                page: "borrow.html".to_string(),
                distance: 2,
            },
            GraphNeighborPage {
                page: "rust.html".to_string(),
                distance: 1,
            },
            GraphNeighborPage {
                page: "topic.html".to_string(),
                distance: 1,
            },
        ]
    );
    assert_eq!(
        graph_neighbors_report(root.path(), Path::new("pages/index"), 2).expect("neighbors report"),
        "index.html\nneighbors depth 2:\n  - borrow.html (distance 2)\n  - rust.html (distance 1)\n  - topic.html (distance 1)\n"
    );
}

#[test]
fn graph_orphans_report_lists_pages_with_no_backlinks() {
    let root = temp_dir("graph-orphans-report");
    fs::create_dir_all(root.path()).expect("create temp dir");
    write_test_manifest(root.path());
    write_test_graph(
        root.path(),
        vec![
            PageGraphEntry {
                path: "index.html".to_string(),
                outlinks: vec![GraphPageLink {
                    page: "rust.html".to_string(),
                    text: "Rust".to_string(),
                }],
                backlinks: Vec::new(),
            },
            PageGraphEntry {
                path: "rust.html".to_string(),
                outlinks: Vec::new(),
                backlinks: vec![GraphPageLink {
                    page: "index.html".to_string(),
                    text: "Rust".to_string(),
                }],
            },
        ],
    );

    assert_eq!(
        graph_orphans_report(root.path()).expect("orphans report"),
        "orphan pages\n  - index.html (1 outlink)\n"
    );
    assert_eq!(
        orphan_pages(root.path()).expect("structured orphans"),
        vec![PageGraphEntry {
            path: "index.html".to_string(),
            outlinks: vec![GraphPageLink {
                page: "rust.html".to_string(),
                text: "Rust".to_string(),
            }],
            backlinks: Vec::new(),
        }]
    );
}

#[test]
fn graph_reports_reject_unsupported_graph_version() {
    let root = temp_dir("graph-version");
    fs::create_dir_all(root.path()).expect("create temp dir");
    write_test_manifest(root.path());
    fs::create_dir_all(root.join(".fractal")).expect("create workspace dir");
    fs::write(
        root.join(".fractal").join("graph.json"),
        serde_json::to_string_pretty(&ProjectGraph {
            version: GRAPH_VERSION + 1,
            nodes: Vec::new(),
            edges: Vec::new(),
            pages: Vec::new(),
        })
        .expect("serialize graph"),
    )
    .expect("write graph");

    let error = graph_orphans_report(root.path()).expect_err("unsupported graph should fail");
    assert!(error.to_string().contains("unsupported graph version"));
}

#[test]
fn sync_uses_relative_links_from_nested_pages() {
    let project = TestProject::new("sync-nested-links");
    project.write_page(
        "index.html",
        render_page_document(
            "Home",
            "<p>Nested Topic lives elsewhere.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    );
    project.write_page(
        "folder/topic.html",
        render_page_document(
            "Nested Topic",
            "<p>Home is the default page.</p>",
            Theme::Dark,
            "../../.fractal/style.css".to_string(),
        ),
    );

    sync_project(project.root()).expect("sync project");

    let nested = fs::read_to_string(project.pages_dir().join("folder/topic.html"))
        .expect("read nested page");
    assert!(PageDocument::parse(&nested).links().contains(&LinkEntry {
        href: "../index.html".to_string(),
        text: "Home".to_string(),
        scope: "page".to_string(),
    }));
}
