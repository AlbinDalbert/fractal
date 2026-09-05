use fractal::{
    FolderHtmlExportOptions, FractalErrorCode, HealthIssueCode, HtmlExportOptions, LinkTarget,
    MutationKind, NativeDocumentParts, Page, Project, ProjectEntryKind,
};
use std::fs;
use tempfile::TempDir;

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::fs::symlink;

fn project(name: &str) -> (TempDir, Project) {
    let directory = TempDir::new().unwrap();
    let project = Project::init(directory.path(), name).unwrap();
    (directory, project)
}

#[test]
fn public_data_types_have_native_only_shapes() {
    // Struct literals and exhaustive matches pin the external API shape. A
    // removed field or enum variant returning will make this test fail to compile.
    let page = Page {
        path: "page.fractal.html".into(),
        content_hash: "sha256:page".into(),
        title: Some("Page".into()),
        text: "Page".into(),
        links: vec![],
    };
    let parts = NativeDocumentParts {
        title: "Page".into(),
        title_hash: "sha256:title".into(),
        content_html: "<p>Page</p>".into(),
        content_hash: "sha256:content".into(),
        style_css: String::new(),
        style_hash: "sha256:style".into(),
        metadata_html: String::new(),
        metadata_hash: "sha256:metadata".into(),
        source_hash: "sha256:source".into(),
    };
    assert_eq!(page.path, "page.fractal.html");
    assert_eq!(parts.title, "Page");

    for target in [
        LinkTarget::Resolved("target.fractal.html".into()),
        LinkTarget::Broken("missing.fractal.html".into()),
    ] {
        match target {
            LinkTarget::Resolved(_) | LinkTarget::Broken(_) => {}
        }
    }

    for operation in [
        MutationKind::CreatePage,
        MutationKind::CreateFolder,
        MutationKind::RecreatePage,
        MutationKind::SetPageContent,
        MutationKind::SetPageStyle,
        MutationKind::SetPageMetadata,
        MutationKind::RepairPageStructure,
        MutationKind::SetPageTitle,
        MutationKind::MovePage,
        MutationKind::DeletePages,
        MutationKind::InsertLink,
        MutationKind::SetFolderTitle,
        MutationKind::ReorderFolder,
        MutationKind::MoveFolder,
        MutationKind::DeleteFolder,
        MutationKind::RepairProject,
    ] {
        match operation {
            MutationKind::CreatePage
            | MutationKind::CreateFolder
            | MutationKind::RecreatePage
            | MutationKind::SetPageContent
            | MutationKind::SetPageStyle
            | MutationKind::SetPageMetadata
            | MutationKind::RepairPageStructure
            | MutationKind::SetPageTitle
            | MutationKind::MovePage
            | MutationKind::DeletePages
            | MutationKind::InsertLink
            | MutationKind::SetFolderTitle
            | MutationKind::ReorderFolder
            | MutationKind::MoveFolder
            | MutationKind::DeleteFolder
            | MutationKind::RepairProject => {}
        }
    }
}

#[test]
fn native_only_projects_open_and_inspect_through_the_public_api() {
    let (directory, mut project) = project("Field notes");
    project.create_page("Welcome").unwrap();
    drop(project);

    let inspection = Project::inspect(directory.path()).unwrap();
    assert!(inspection.openable);
    assert!(inspection.healthy);

    let project = Project::open(directory.path()).unwrap();
    assert_eq!(project.manifest().version, 2);
    assert_eq!(project.pages()[0].path, "welcome.fractal.html");
    assert!(project.validate().valid);
}

#[test]
fn ordinary_html_and_opaque_files_are_invisible_to_public_queries() {
    let (directory, mut project) = project("Opaque neighbors");
    project.create_page("Visible").unwrap();
    fs::write(
        directory.path().join("pages/ordinary.html"),
        "<title>Hidden phrase</title><p>needle outside the catalog</p>",
    )
    .unwrap();
    fs::write(
        directory.path().join("pages/notes.txt"),
        "needle outside the catalog",
    )
    .unwrap();
    fs::write(directory.path().join("private.bin"), [0, 1, 2, 3]).unwrap();

    let project = Project::open(directory.path()).unwrap();
    assert_eq!(project.pages().len(), 1);
    assert_eq!(project.pages()[0].path, "visible.fractal.html");
    assert!(project.search("needle").is_empty());
    assert!(project.links("visible").unwrap().is_empty());
    assert!(project.validate().valid);
    assert!(directory.path().join("pages/ordinary.html").is_file());
    assert!(directory.path().join("pages/notes.txt").is_file());
    assert!(directory.path().join("private.bin").is_file());
}

#[test]
fn version_one_is_inspectable_but_cannot_be_opened() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("pages")).unwrap();
    fs::write(
        directory.path().join("fractal.json"),
        r#"{"name":"Old project","version":1}"#,
    )
    .unwrap();

    let error = Project::open(directory.path()).unwrap_err();
    assert_eq!(error.code, FractalErrorCode::UnsupportedVersion);

    let inspection = Project::inspect(directory.path()).unwrap();
    assert!(!inspection.openable);
    assert!(!inspection.healthy);
    assert_eq!(
        inspection.issues[0].code,
        HealthIssueCode::UnsupportedVersion
    );
    assert!(!directory.path().join(".fractal.lock").exists());
}

#[test]
fn folder_creation_is_explicit_and_page_parents_must_exist() {
    let (directory, mut project) = project("Folders");

    project.create_page("Movable").unwrap();
    let source = project.source("movable").unwrap();
    let error = project
        .move_page("movable", "missing-parent/movable")
        .unwrap_err();
    assert_eq!(error.code, FractalErrorCode::NotFound);
    project.delete_page("movable").unwrap();
    let error = project
        .recreate_page_from_source("missing-parent/movable", &source)
        .unwrap_err();
    assert_eq!(error.code, FractalErrorCode::NotFound);
    assert!(!directory.path().join("pages/missing-parent").exists());

    let error = project
        .create_page_at("field-notes/entry", "Entry")
        .unwrap_err();
    assert_eq!(error.code, FractalErrorCode::NotFound);
    assert!(!directory.path().join("pages/field-notes").exists());

    let receipt = project.create_folder(".", "Field Notes").unwrap();
    assert_eq!(receipt.operation, MutationKind::CreateFolder);
    assert!(receipt.changes.iter().any(|change| matches!(
        change,
        fractal::ProjectChange::Created { path, entry, .. }
            if path.as_str() == "pages/field-notes"
                && *entry == ProjectEntryKind::Directory
    )));

    project
        .create_page_at("field-notes/entry", "Entry")
        .unwrap();
    assert_eq!(
        project.page("field-notes/entry").unwrap().path,
        "field-notes/entry.fractal.html"
    );
}

#[test]
fn exports_refuse_destinations_inside_the_project() {
    let (directory, mut project) = project("Exports");
    project.create_page("Source").unwrap();
    let source_path = directory.path().join("pages/source.fractal.html");
    let source_before = fs::read(&source_path).unwrap();

    let error = project
        .export_html("source", &source_path, HtmlExportOptions::default())
        .unwrap_err();
    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert_eq!(fs::read(&source_path).unwrap(), source_before);

    let manifest_path = directory.path().join("fractal.json");
    let manifest_before = fs::read(&manifest_path).unwrap();
    let error = project
        .export_folder_html(".", &manifest_path, FolderHtmlExportOptions::default())
        .unwrap_err();
    assert_eq!(error.code, FractalErrorCode::InvalidInput);
    assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);

    #[cfg(unix)]
    {
        let alias_directory = TempDir::new().unwrap();
        let project_alias = alias_directory.path().join("project-alias");
        symlink(directory.path(), &project_alias).unwrap();
        let error = project
            .export_html(
                "source",
                project_alias.join("pages/alias.html"),
                HtmlExportOptions::default(),
            )
            .unwrap_err();
        assert_eq!(error.code, FractalErrorCode::InvalidInput);
    }

    assert!(Project::open(directory.path()).is_ok());
}

#[cfg(unix)]
#[test]
fn opaque_filesystem_entries_stay_outside_the_catalog() {
    let (directory, mut project) = project("Opaque entries");
    project.create_page("Outside").unwrap();
    let outside = directory.path().join("outside.fractal.html");
    fs::rename(
        directory.path().join("pages/outside.fractal.html"),
        &outside,
    )
    .unwrap();
    symlink(
        &outside,
        directory.path().join("pages/outside.fractal.html"),
    )
    .unwrap();
    fs::write(
        directory
            .path()
            .join("pages")
            .join(OsString::from_vec(vec![0xff])),
        b"opaque",
    )
    .unwrap();
    drop(project);

    let project = Project::open(directory.path()).unwrap();
    assert!(project.pages().is_empty());
    assert!(project.search("Outside").is_empty());
    assert!(project.folder(".").unwrap().children.is_empty());
}

#[test]
fn search_and_link_queries_include_native_documents_only() {
    let (directory, mut project) = project("Links");
    project.create_page("Target").unwrap();
    project.create_page("Source").unwrap();

    let parts = project.native_document_parts("source").unwrap();
    project
        .set_page_content(
            "source",
            concat!(
                "<p>Target remains available as a derived link. ",
                "Connect this marker explicitly.</p>",
                "<p><a href=\"notes.html\">Opaque</a> ",
                "<a href=\"https://example.com/target.fractal.html\">External</a></p>"
            ),
            &parts.content_hash,
        )
        .unwrap();
    project.insert_link("source", "marker", "target").unwrap();
    fs::write(
        directory.path().join("pages/ordinary.html"),
        "<p>exclusive opaque search phrase</p>",
    )
    .unwrap();
    let project = Project::open(directory.path()).unwrap();

    let search_paths: Vec<_> = project
        .search("derived link")
        .into_iter()
        .map(|result| result.path)
        .collect();
    assert_eq!(search_paths, ["source.fractal.html"]);
    assert!(project.search("exclusive opaque").is_empty());

    let links = project.links("source").unwrap();
    assert_eq!(links.len(), 1);
    assert!(matches!(
        &links[0].target,
        LinkTarget::Resolved(path) if path == "target.fractal.html"
    ));
    let backlinks = project.backlinks("target").unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].page, "source.fractal.html");

    let source_before = project.source("source").unwrap();
    let derived = project.derived_links("source").unwrap();
    assert_eq!(derived.len(), 1);
    assert_eq!(derived[0].text, "Target");
    assert_eq!(derived[0].target, "target.fractal.html");
    assert_eq!(project.source("source").unwrap(), source_before);
}

#[test]
fn stale_native_section_hashes_fail_with_a_conflict() {
    let (_directory, mut project) = project("Conflicts");
    project.create_page("Draft").unwrap();
    let stale_hash = project.native_document_parts("draft").unwrap().content_hash;

    project
        .set_page_content("draft", "<p>First edit.</p>", &stale_hash)
        .unwrap();
    let error = project
        .set_page_content("draft", "<p>Stale edit.</p>", &stale_hash)
        .unwrap_err();

    assert_eq!(error.code, FractalErrorCode::Conflict);
    assert!(project.source("draft").unwrap().contains("First edit."));
    assert!(!project.source("draft").unwrap().contains("Stale edit."));
}
