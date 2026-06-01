use super::document::PageDocument;
use super::html::escape_html;
use super::markdown::{html_to_markdown, markdown_to_html};
use super::notes::{insert_note_into_document, note_id_from_trigger, render_note_aside};
use super::paths::{collect_page_paths, resolve_page_destination};
use super::render::{render_page_document, stylesheet_href};
use super::validation::validate_page_metadata;
use super::{
    add_note, export_page, import_markdown, new_page, patch_note, remove_note, sync_project,
    validate_project, FileEntry, GraphPageLink, LinkEntry, NoteEntry, PageEntry, ProjectGraph,
    ProjectIndex, ProjectManifest, Theme,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    std::env::temp_dir().join(format!("fractal-{name}-{unique}"))
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
            version: 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
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
    collect_page_paths(&root, &root, &mut pages).expect("collect page paths");
    pages.sort();

    assert_eq!(pages, vec!["index.html", "nested/note.html"]);

    fs::remove_dir_all(&root).expect("cleanup temp dir");
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
            version: 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
    let source = root.join("source.md");
    fs::write(&source, "# Source\n\nHello\n\n### Deep").expect("write markdown");

    import_markdown(&root, &source).expect("import markdown");
    let page = fs::read_to_string(pages_dir.join("source.html")).expect("read imported page");
    assert!(page.contains("<h1>Source</h1>"));
    assert!(page.contains("<p>Hello</p>"));
    assert!(page.contains("<h3>Deep</h3>"));

    let output = root.join("out.md");
    export_page(&root, Path::new("pages/source.html"), &output).expect("export markdown");
    assert_eq!(
        fs::read_to_string(output).expect("read exported markdown"),
        "# Source\n\nHello\n\n### Deep"
    );

    fs::remove_dir_all(&root).expect("cleanup temp dir");
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
    fs::create_dir_all(&root).expect("create temp dir");
    let page = root.join("page.html");
    fs::write(
            &page,
            "<html><head><meta name=\"fractal:version\" content=\"0.1\" /><meta name=\"fractal:summary\" content=\"Short page summary here.\" /><meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" /></head><body></body></html>",
        )
        .expect("write page");

    let error = validate_page_metadata(&page).expect_err("missing notes section should fail");
    assert!(error.to_string().contains("missing notes section"));

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn validate_page_metadata_rejects_duplicate_notes_sections() {
    let root = temp_dir("validate-duplicate-notes");
    fs::create_dir_all(&root).expect("create temp dir");
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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn validate_page_metadata_rejects_malformed_note_ids() {
    let root = temp_dir("validate-note-id");
    fs::create_dir_all(&root).expect("create temp dir");
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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
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
            version: 1,
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

    validate_project(&root, true).expect("fix and validate project");

    assert!(root.join(".fractal/style.css").is_file());
    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
    assert!(html.contains("<meta name=\"fractal:version\" content=\"0.1\" />"));
    assert!(html.contains("<link rel=\"stylesheet\" href=\"../.fractal/style.css\">"));
    assert!(html.contains("<body data-fractal-theme=\"dark\">"));
    assert!(html.contains("<section data-fractal-notes>"));

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn validate_fix_merges_duplicate_notes_sections() {
    let root = temp_dir("validate-fix-duplicate-notes");
    let pages_dir = root.join("pages");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    write_test_manifest(&root);
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

    validate_project(&root, true).expect("fix and validate project");

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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
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
            version: 1,
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

    validate_project(&root, false).expect("validate project");

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn page_destination_is_relative_to_pages_root() {
    let destination = resolve_page_destination(Path::new("."), Path::new("nested/secondpage"))
        .expect("resolve destination");

    assert_eq!(destination, PathBuf::from("./pages/nested/secondpage.html"));
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
    let root = temp_dir("new-page-index");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: 1,
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

    new_page(&root, Path::new("secondpage")).expect("create new page");

    let index: ProjectIndex = serde_json::from_str(
        &fs::read_to_string(workspace_dir.join("index.json")).expect("read index"),
    )
    .expect("parse index");

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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn extracts_all_fractal_meta_tags() {
    let root = temp_dir("extract-meta");
    fs::create_dir_all(&root).expect("create temp dir");
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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
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
    let root = temp_dir("add-note");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: 1,
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

    add_note(&root, Path::new("index"), "java", "my note text").expect("add note");

    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn patch_note_updates_existing_note() {
    let root = temp_dir("patch-note");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
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
    fs::write(pages_dir.join("index.html"), page).expect("write index page");

    patch_note(&root, Path::new("index"), "java", "new text").expect("patch note");

    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
    assert!(html.contains("<p>new text</p>"));
    assert!(!html.contains("<p>old text</p>"));

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn remove_note_keeps_empty_note_section() {
    let root = temp_dir("remove-note");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
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
    fs::write(pages_dir.join("index.html"), page).expect("write index page");

    remove_note(&root, Path::new("index"), "java").expect("remove note");

    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
    let document = PageDocument::parse(&html);
    assert!(!html.contains("note-java"));
    assert_eq!(document.notes_section_count(), 1);

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn note_mutation_handles_ordinary_html_formatting() {
    let root = temp_dir("messy-note-mutation");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    write_test_manifest(&root);
    fs::write(
        pages_dir.join("index.html"),
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
    )
    .expect("write index page");

    add_note(&root, Path::new("index"), "Java", "java & <note>").expect("add note");
    patch_note(&root, Path::new("index"), "rust", "patched <rust>").expect("patch note");
    remove_note(&root, Path::new("index"), "Java").expect("remove note");

    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn sync_rebuilds_index_and_links_notes_before_project_pages() {
    let root = temp_dir("sync-links");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(&pages_dir).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");

    let index = render_page_document(
        "Home",
        "<p>Java and Rust are mentioned here.</p>",
        Theme::Dark,
        "../.fractal/style.css".to_string(),
    );
    let index = insert_note_into_document(&index, &render_note_aside("note-java", "note body"))
        .expect("insert note");
    fs::write(pages_dir.join("index.html"), index).expect("write index page");
    fs::write(
        pages_dir.join("rust.html"),
        render_page_document(
            "Rust",
            "<p>Home mentions Java.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    )
    .expect("write rust page");
    fs::write(pages_dir.join("asset.txt"), "asset").expect("write asset");

    sync_project(&root).expect("sync project");
    sync_project(&root).expect("sync project twice");

    let html = fs::read_to_string(pages_dir.join("index.html")).expect("read index page");
    assert_eq!(html.matches("data-fractal-link=\"note\"").count(), 1);
    assert_eq!(html.matches("data-fractal-link=\"page\"").count(), 1);
    assert!(html.contains("<a href=\"#note-java\" data-fractal-link=\"note\">Java</a>"));
    assert!(html.contains("<a href=\"rust.html\" data-fractal-link=\"page\">Rust</a>"));

    let index: ProjectIndex = serde_json::from_str(
        &fs::read_to_string(workspace_dir.join("index.json")).expect("read index"),
    )
    .expect("parse index");
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
        &fs::read_to_string(workspace_dir.join("graph.json")).expect("read graph"),
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

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}

#[test]
fn sync_uses_relative_links_from_nested_pages() {
    let root = temp_dir("sync-nested-links");
    let pages_dir = root.join("pages");
    let workspace_dir = root.join(".fractal");
    fs::create_dir_all(pages_dir.join("folder")).expect("create pages dir");
    fs::create_dir_all(&workspace_dir).expect("create workspace dir");
    fs::write(
        root.join("fractal.json"),
        serde_json::to_string_pretty(&ProjectManifest {
            project_name: "test".to_string(),
            version: 1,
            default_page: "pages/index.html".to_string(),
            theme: Theme::Dark,
        })
        .expect("serialize manifest"),
    )
    .expect("write manifest");
    fs::write(
        pages_dir.join("index.html"),
        render_page_document(
            "Home",
            "<p>Nested Topic lives elsewhere.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        ),
    )
    .expect("write index page");
    fs::write(
        pages_dir.join("folder/topic.html"),
        render_page_document(
            "Nested Topic",
            "<p>Home is the default page.</p>",
            Theme::Dark,
            "../../.fractal/style.css".to_string(),
        ),
    )
    .expect("write nested page");

    sync_project(&root).expect("sync project");

    let nested = fs::read_to_string(pages_dir.join("folder/topic.html")).expect("read nested page");
    assert!(nested.contains("<a href=\"../index.html\" data-fractal-link=\"page\">Home</a>"));

    fs::remove_dir_all(&root).expect("cleanup temp dir");
}
