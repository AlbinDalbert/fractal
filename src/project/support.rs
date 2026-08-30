use super::*;

pub(super) fn normalize_export_selections(selections: &[PathBuf]) -> Result<BTreeSet<String>> {
    selections
        .iter()
        .map(|selection| normalize_relative_path(selection).map(|path| path_string(&path)))
        .collect()
}

pub(super) fn folder_export_page_id(path: &str) -> String {
    let mut id = String::from("fractal-page-");
    for byte in path.as_bytes() {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

pub(super) fn folder_export_shell(title: &str, main: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{}</title>\n  <style>\n    * {{ box-sizing: border-box; }}\n    body {{ margin: 0; color: #222; background: #fff; font: 1.05rem/1.65 ui-serif, Georgia, serif; }}\n    main {{ width: min(100% - 2rem, 48rem); margin: 0 auto; padding: 4rem 0; }}\n    section[data-fractal-source] {{ break-before: page; }}\n    section[data-fractal-source]:first-child {{ break-before: auto; }}\n    h1 {{ font-size: 2.25rem; line-height: 1.15; margin: 0 0 2rem; }}\n    h2, h3, h4, h5, h6 {{ line-height: 1.25; }}\n    p, ul, ol, blockquote, pre, figure, table {{ margin: 1.25rem 0; }}\n    hr {{ border: 0; border-top: 1px solid #bbb; margin: 4rem 0; }}\n    a {{ color: #174ea6; text-underline-offset: 0.15em; }}\n    code, pre {{ font-family: ui-monospace, monospace; }}\n  </style>\n</head>\n<body>\n<main>\n{}\n</main>\n</body>\n</html>\n",
        escape_html(title),
        main
    )
}

pub(super) fn normalize_folder_path(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() || path == Path::new(".") || path == Path::new(PAGES) {
        return Ok(PathBuf::new());
    }
    normalize_relative_path(path)
}

pub(super) fn display_folder_path(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        PAGES.into()
    } else {
        path.display().to_string()
    }
}

pub(super) fn folder_metadata_relative_path(folder: &Path) -> PathBuf {
    folder.join(MANIFEST)
}

pub(super) fn default_folder_title(project_name: &str, path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| project_name.to_owned())
}

pub(super) fn ordered_name_kind(name: &str) -> FolderChildKind {
    if name.ends_with(NATIVE_SUFFIX) {
        FolderChildKind::Native
    } else {
        FolderChildKind::Folder
    }
}

pub(super) fn direct_orderable_children(
    directory: &Path,
) -> Result<BTreeMap<String, FolderChildKind>> {
    let mut children = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| FractalError::invalid_project("folder child name is not valid UTF-8"))?;
        let kind = if file_type.is_dir() {
            if name.ends_with(NATIVE_SUFFIX) {
                return Err(FractalError::invalid_project(format!(
                    "folder name uses the reserved native document suffix: {name}"
                )));
            }
            Some(FolderChildKind::Folder)
        } else if file_type.is_file() && name.ends_with(NATIVE_SUFFIX) {
            Some(FolderChildKind::Native)
        } else {
            None
        };
        if let Some(kind) = kind {
            children.insert(name, kind);
        }
    }
    Ok(children)
}

pub(super) fn validate_stored_order(
    folder: &Path,
    order: &[String],
    present: &BTreeMap<String, FolderChildKind>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for name in order {
        validate_order_name(name).map_err(|error| {
            FractalError::invalid_project(format!(
                "invalid order in {}: {}",
                display_folder_path(folder),
                error.message
            ))
        })?;
        if !seen.insert(name) {
            return Err(FractalError::invalid_project(format!(
                "duplicate ordered child `{name}` in {}",
                display_folder_path(folder)
            )));
        }
        if let Some(actual) = present.get(name) {
            let expected = ordered_name_kind(name);
            if *actual != expected {
                return Err(FractalError::invalid_project(format!(
                    "ordered child `{name}` has the wrong type in {}",
                    display_folder_path(folder)
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_order_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if name.is_empty()
        || name == MANIFEST
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(FractalError::invalid_input(format!(
            "invalid ordered child name: {name}"
        )));
    }
    Ok(())
}

pub(super) fn collect_directories(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let path = entry.path();
            output.push(path.strip_prefix(root)?.to_path_buf());
            collect_directories(root, &path, output)?;
        }
    }
    output.sort();
    Ok(())
}

pub(super) fn normalize_relative_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Err(FractalError::invalid_input("page path must be relative"));
    }
    let mut output = PathBuf::new();
    let mut components = path.components().peekable();
    if matches!(components.peek(), Some(Component::Normal(part)) if *part == PAGES) {
        components.next();
    }
    for component in components {
        match component {
            Component::Normal(part) => output.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(FractalError::invalid_input("page path cannot contain `..`"))
            }
            _ => return Err(FractalError::invalid_input("invalid page path")),
        }
    }
    if output.as_os_str().is_empty() {
        return Err(FractalError::invalid_input("page path cannot be empty"));
    }
    Ok(output)
}

pub(super) fn validate_html_path(path: &Path) -> Result<()> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("html") {
        return Err(FractalError::invalid_input("page path must end in .html"));
    }
    Ok(())
}

pub(super) fn append_native_suffix(path: &Path) -> Result<PathBuf> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(FractalError::invalid_input("invalid native page path"));
    };
    let mut output = path.to_path_buf();
    output.set_file_name(format!("{name}{NATIVE_SUFFIX}"));
    Ok(output)
}

pub(super) fn normalize_native_page_path(path: &Path) -> Result<PathBuf> {
    let path = normalize_relative_path(path)?;
    let path = if path.extension().is_none() {
        append_native_suffix(&path)?
    } else {
        path
    };
    if page_kind(&path) != PageKind::Native {
        return Err(FractalError::invalid_input(format!(
            "native page path must end in {NATIVE_SUFFIX}"
        )));
    }
    Ok(path)
}

pub(super) fn normalize_destination_page_path(path: &Path, kind: PageKind) -> Result<PathBuf> {
    let path = normalize_relative_path(path)?;
    let path = if path.extension().is_none() {
        match kind {
            PageKind::Native => append_native_suffix(&path)?,
            PageKind::Raw => path.with_extension("html"),
        }
    } else {
        path
    };
    validate_html_path(&path)?;
    if page_kind(&path) != kind {
        return Err(FractalError::invalid_input(
            "moving a page cannot change whether it is native or raw",
        ));
    }
    Ok(path)
}

pub(super) fn page_kind(path: &Path) -> PageKind {
    if path_string(path).ends_with(NATIVE_SUFFIX) {
        PageKind::Native
    } else {
        PageKind::Raw
    }
}

pub(super) fn native_document_issues(document: &Document) -> Vec<String> {
    let mut issues = Vec::new();
    if !document.has_html_doctype() {
        issues.push("native document needs `<!doctype html>`".into());
    }
    if !document.has_native_marker() {
        issues.push("native document needs `<meta name=\"fractal-format\" content=\"1\">`".into());
    }
    if document.title().is_none() {
        issues.push("native document needs a non-empty `<title>` or `<h1>`".into());
    }
    if document.native_root_count() != 1 {
        issues.push("native document needs exactly one `<main data-fractal-document>`".into());
    }
    if document.managed_title_count() != 1 {
        issues.push(
            "native document needs exactly one `<h1 data-fractal-title>` directly inside its document root"
                .into(),
        );
    }
    if document.managed_style_count() != 1 {
        issues.push(
            "native document needs exactly one `<style data-fractal-style>` in its head".into(),
        );
    }
    let outside = document.body_elements_outside_native_root();
    if !outside.is_empty() {
        issues.push(format!(
            "native document body contains elements outside its document root: {}",
            outside.join(", ")
        ));
    }
    let unsupported = document.unsupported_native_elements();
    if !unsupported.is_empty() {
        issues.push(format!(
            "native document contains unsupported elements: {}",
            unsupported.join(", ")
        ));
    }
    let unsupported = document.unsupported_native_head_elements();
    if !unsupported.is_empty() {
        issues.push(format!(
            "native document head contains unsupported elements: {}",
            unsupported.join(", ")
        ));
    }
    issues
}

pub(super) fn collect_html(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_html(root, &path, output)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("html") {
            output.push(path.strip_prefix(root)?.to_path_buf());
        }
    }
    output.sort();
    Ok(())
}

pub(super) fn collect_files(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, output)?;
        } else {
            output.push(path.strip_prefix(root)?.to_path_buf());
        }
    }
    output.sort();
    Ok(())
}

pub(super) fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| FractalError::invalid_input("file path needs a parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(contents.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct TransactionPlan {
    affected: Vec<PathBuf>,
    originals: BTreeSet<PathBuf>,
}

pub(super) struct ProjectLock {
    _file: File,
}

impl ProjectLock {
    pub(super) fn exclusive(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        FileExt::lock_exclusive(&file)?;
        Ok(Self { _file: file })
    }
}

pub(super) fn commit_file_transaction(
    root: &Path,
    writes: Vec<(PathBuf, String)>,
    deletes: Vec<PathBuf>,
) -> Result<()> {
    let writes: BTreeMap<PathBuf, String> = writes.into_iter().collect();
    let mut affected: BTreeSet<PathBuf> = writes.keys().cloned().collect();
    affected.extend(deletes);
    if affected.is_empty() {
        return Ok(());
    }
    reject_overlapping_transaction_paths(&affected)?;

    let pages = root.join(PAGES);
    let transaction = tempfile::Builder::new()
        .prefix(TRANSACTION_PREFIX)
        .tempdir_in(root)?;
    let transaction_root = transaction.path();
    let new_root = transaction_root.join("new");
    let old_root = transaction_root.join("old");
    let originals = affected
        .iter()
        .filter(|path| path_exists(&pages.join(path)))
        .cloned()
        .collect();
    let plan = TransactionPlan {
        affected: affected.iter().cloned().collect(),
        originals,
    };
    atomic_write(
        &transaction_root.join("plan.json"),
        &serde_json::to_string(&plan)?,
    )?;

    for (path, contents) in &writes {
        atomic_write(&new_root.join(path), contents)?;
    }

    let result = (|| -> Result<()> {
        for path in &plan.affected {
            let source = pages.join(path);
            if path_exists(&source) {
                let backup = old_root.join(path);
                create_parent(&backup)?;
                fs::rename(source, backup)?;
            }
        }
        for path in writes.keys() {
            let source = new_root.join(path);
            let destination = pages.join(path);
            create_parent(&destination)?;
            fs::rename(source, destination)?;
        }
        let committed = File::create(transaction_root.join("committed"))?;
        committed.sync_all()?;
        Ok(())
    })();

    if let Err(error) = result {
        if let Err(recovery_error) = recover_transaction(transaction_root) {
            let preserved = transaction.keep();
            return Err(FractalError::new(
                crate::FractalErrorCode::Io,
                format!(
                    "transaction failed: {error}; rollback also failed: {recovery_error}; recovery files remain at {}",
                    preserved.display()
                ),
            ));
        }
        return Err(error);
    }

    drop(transaction);
    Ok(())
}

pub(super) fn recover_transactions(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir()
            || !entry
                .file_name()
                .to_string_lossy()
                .starts_with(TRANSACTION_PREFIX)
        {
            continue;
        }
        if !entry.path().join("plan.json").is_file() {
            continue;
        }
        recover_transaction(&entry.path())?;
    }
    Ok(())
}

pub(super) fn recover_transaction(transaction_root: &Path) -> Result<()> {
    let plan_path = transaction_root.join("plan.json");
    if !plan_path.is_file() || transaction_root.join("committed").is_file() {
        fs::remove_dir_all(transaction_root)?;
        return Ok(());
    }
    let plan: TransactionPlan = serde_json::from_str(&fs::read_to_string(plan_path)?)?;
    let affected: BTreeSet<PathBuf> = plan.affected.iter().cloned().collect();
    reject_overlapping_transaction_paths(&affected)?;
    let root = transaction_root
        .parent()
        .ok_or_else(|| FractalError::invalid_project("transaction has no project root"))?;
    let pages = root.join(PAGES);
    let old_root = transaction_root.join("old");
    for path in plan.affected.iter().rev() {
        let current = pages.join(path);
        let backup = old_root.join(path);
        if path_exists(&backup) {
            remove_path_if_present(&current)?;
            create_parent(&current)?;
            fs::rename(backup, current)?;
        } else if !plan.originals.contains(path) {
            remove_path_if_present(&current)?;
        }
    }
    fs::remove_dir_all(transaction_root)?;
    Ok(())
}

pub(super) fn reject_overlapping_transaction_paths(paths: &BTreeSet<PathBuf>) -> Result<()> {
    for path in paths {
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(FractalError::invalid_project(format!(
                "transaction contains an invalid path: {}",
                path.display()
            )));
        }
        if paths
            .iter()
            .any(|other| other != path && path_starts_with(path, other))
        {
            return Err(FractalError::invalid_input(
                "transaction paths cannot contain one another",
            ));
        }
    }
    Ok(())
}

pub(super) fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(super) fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

pub(super) fn remove_path_if_present(path: &Path) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub(super) fn path_starts_with(path: &Path, parent: &Path) -> bool {
    path.starts_with(parent)
}

pub(super) fn link_target_path(target: &LinkTarget) -> Option<&str> {
    match target {
        LinkTarget::Internal(path) | LinkTarget::InternalFile(path) => Some(path),
        _ => None,
    }
}

pub(super) fn iframe_target_path(target: &IframeTarget) -> Option<&str> {
    match target {
        IframeTarget::Internal(path) | IframeTarget::InternalFile(path) => Some(path),
        _ => None,
    }
}

pub(super) fn content_hash(contents: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(contents.as_bytes()))
}

pub(super) fn slug(title: &str) -> Result<String> {
    let mut output = String::new();
    let mut separator = false;
    for character in title.trim().to_lowercase().chars() {
        if character.is_alphanumeric() {
            output.push(character);
            separator = false;
        } else if !output.is_empty() && !separator {
            output.push('-');
            separator = true;
        }
    }
    let output = output.trim_matches('-').to_string();
    if output.is_empty() {
        Err(FractalError::invalid_input(
            "title cannot be converted to a filename",
        ))
    } else {
        Ok(output)
    }
}

pub(super) fn exact_case_insensitive_matches(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    let needle_chars = needle.chars().count();
    if needle_chars == 0 {
        return Vec::new();
    }
    let needle_lower = needle.to_lowercase();
    let mut boundaries: Vec<_> = haystack.char_indices().map(|(index, _)| index).collect();
    boundaries.push(haystack.len());
    let mut matches = Vec::new();
    for window in boundaries.windows(needle_chars + 1) {
        let start = window[0];
        let end = window[needle_chars];
        if haystack[start..end].to_lowercase() != needle_lower {
            continue;
        }
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        if before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
        {
            matches.push((start, end));
        }
    }
    matches
}

pub(super) fn snippet(text: &str, query: &str) -> String {
    let lower = text.to_lowercase();
    let start = lower.find(query).unwrap_or(0).saturating_sub(50);
    let start = (start..=lower.len())
        .find(|index| text.is_char_boundary(*index))
        .unwrap_or(0);
    let desired_end = (start + 180).min(text.len());
    let end = (start..=desired_end)
        .rev()
        .find(|index| text.is_char_boundary(*index))
        .unwrap_or(text.len());
    text[start..end].trim().to_string()
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
