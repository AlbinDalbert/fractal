use super::constants::{GRAPH_VERSION, INDEX_VERSION, MANIFEST_VERSION};
use super::document::PageDocument;
use super::graph::{graph_orphans_report, graph_page_report};
use super::html::escape_html;
use super::markdown::{html_to_markdown, markdown_to_html};
use super::notes::{insert_note_into_document, note_id_from_trigger, render_note_aside};
use super::paths::{collect_page_paths, resolve_page_destination};
use super::render::{render_page_document, stylesheet_href};
use super::validation::validate_page_metadata;
use super::{
    add_note, build_index, export_page, import_markdown, new_page, patch_note, remove_note,
    sync_project, validate_project, FileEntry, GraphPageLink, LinkEntry, NoteEntry, OperationEvent,
    PageEntry, PageGraphEntry, ProjectGraph, ProjectIndex, ProjectManifest, Theme,
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
        (
            "fractal:summary".to_string(),
            "Short page summary here.".to_string(),
        ),
        (
            "fractal:tags".to_string(),
            "rust, graphs, parsing".to_string(),
        ),
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
    fs::create_dir_all(root.join(".fractal")).expect("create workspace dir");
    fs::write(
        root.join(".fractal").join("graph.json"),
        serde_json::to_string_pretty(&ProjectGraph {
            version: GRAPH_VERSION,
            nodes: Vec::new(),
            edges: Vec::new(),
            pages,
        })
        .expect("serialize graph"),
    )
    .expect("write graph");
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
    assert!(html.contains("<meta name=\"fractal:summary\" content=\"Short page summary here.\" />"));
    assert!(html.contains("<meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" />"));
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
    fs::write(
            &page,
            "<html><head><meta name=\"fractal:version\" content=\"0.1\" /><meta name=\"fractal:summary\" content=\"Short page summary here.\" /><meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" /></head><body></body></html>",
        )
        .expect("write page");

    let error = validate_page_metadata(&page).expect_err("missing notes section should fail");
    assert!(error.to_string().contains("missing notes section"));
}

#[test]
fn validate_page_metadata_rejects_duplicate_notes_sections() {
    let root = temp_dir("validate-duplicate-notes");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    fs::write(
        &page,
        r#"<html>
  <head>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="Short page summary here.">
    <meta name="fractal:tags" content="rust, graphs, parsing">
  </head>
  <body>
    <section data-fractal-notes></section>
    <section data-fractal-notes></section>
  </body>
</html>"#,
    )
    .expect("write page");

    let error = validate_page_metadata(&page).expect_err("duplicate notes sections should fail");
    assert!(error.to_string().contains("duplicate notes section"));
}

#[test]
fn validate_page_metadata_rejects_malformed_note_ids() {
    let root = temp_dir("validate-note-id");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    fs::write(
        &page,
        r#"<html>
  <head>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="Short page summary here.">
    <meta name="fractal:tags" content="rust, graphs, parsing">
  </head>
  <body>
    <section data-fractal-notes>
      <aside id="Rust" data-fractal-note></aside>
    </section>
  </body>
</html>"#,
    )
    .expect("write page");

    let error = validate_page_metadata(&page).expect_err("malformed note id should fail");
    assert!(error.to_string().contains("malformed note id"));
}

#[test]
fn validate_page_metadata_allows_custom_summary_and_tags() {
    let root = temp_dir("validate-custom-meta");
    fs::create_dir_all(root.path()).expect("create temp dir");
    let page = root.join("page.html");
    fs::write(
        &page,
        r#"<html>
  <head>
    <meta name="fractal:version" content="0.1">
    <meta name="fractal:summary" content="A custom project summary.">
    <meta name="fractal:tags" content="personal, custom">
  </head>
  <body>
    <section data-fractal-notes></section>
  </body>
</html>"#,
    )
    .expect("write page");

    validate_page_metadata(&page).expect("custom metadata should validate");
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

    let error = validate_project(root.path(), false).expect_err("unsupported manifest should fail");
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

    validate_project(root.path(), true).expect("fix and validate project");

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
    <meta name="fractal:summary" content="Short page summary here.">
    <meta name="fractal:tags" content="rust, graphs, parsing">
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

    validate_project(root.path(), true).expect("fix and validate project");

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

    validate_project(root.path(), false).expect("validate project");
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

    assert_eq!(error.to_string(), "page path cannot contain `..`");
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
    assert_eq!(
        report.events.first(),
        Some(&OperationEvent::Created {
            path: project.pages_dir().join("secondpage.html")
        })
    );

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
fn note_mutation_handles_ordinary_html_formatting() {
    let project = TestProject::new("messy-note-mutation");
    project.write_page(
        "index.html",
        r#"<!doctype html>
<HTML lang="en">
  <HEAD>
    <TITLE>Index</TITLE>
    <meta content="0.1" name="fractal:version">
    <meta content="Short page summary here." name="fractal:summary">
    <meta content="rust, graphs, parsing" name="fractal:tags">
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
        .any(|event| matches!(event, OperationEvent::SyncComplete { pages_updated: 2 })));
    assert!(second_report
        .events
        .iter()
        .any(|event| matches!(event, OperationEvent::SyncComplete { pages_updated: 0 })));

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

    let error = validate_project(project.root(), false).expect_err("duplicate project should fail");
    assert!(error.to_string().contains("duplicate page label"));
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
    <meta content="Short page summary here." name="fractal:summary">
    <meta content="rust, graphs, parsing" name="fractal:tags">
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
