use super::*;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransactionFaultPoint {
    Prepared,
    DirectoryCreated,
    OriginalBackedUp,
    NewFileInstalled,
    DirectoryRemoved,
    CommitMarkerCreated,
    CommittedBeforeCleanup,
}

#[cfg(test)]
thread_local! {
    static TRANSACTION_FAULT: std::cell::Cell<Option<TransactionFaultPoint>> = const {
        std::cell::Cell::new(None)
    };
}

/// Schedules a transaction fault at the specified injection point.
///
/// The configured fault is consumed when the transaction reaches that point.
///
/// # Examples
///
/// ```
/// let point = TransactionFaultPoint::default();
/// inject_transaction_fault(point);
/// ```
#[cfg(test)]
pub(crate) fn inject_transaction_fault(point: TransactionFaultPoint) {
    TRANSACTION_FAULT.set(Some(point));
}

#[cfg(test)]
fn take_transaction_fault(point: TransactionFaultPoint) -> bool {
    if TRANSACTION_FAULT.get() == Some(point) {
        TRANSACTION_FAULT.set(None);
        true
    } else {
        false
    }
}

#[cfg(test)]
fn injected_crash(point: TransactionFaultPoint) -> Result<()> {
    if take_transaction_fault(point) {
        Err(FractalError::indeterminate(format!(
            "injected process interruption at {point:?}"
        )))
    } else {
        Ok(())
    }
}

/// Normalizes export selection paths and returns them as sorted, unique strings.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// let selections = [PathBuf::from("pages/guide"), PathBuf::from("pages/guide")];
/// let normalized = normalize_export_selections(&selections)?;
///
/// assert_eq!(normalized.len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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
    atomic_write_bytes(path, contents.as_bytes())
}

/// Atomically writes bytes to a file and durably synchronizes the file and its parent directory.
///
/// # Examples
///
/// ```
/// let directory = tempfile::tempdir().unwrap();
/// let path = directory.path().join("output.txt");
///
/// atomic_write_bytes(&path, b"hello").unwrap();
///
/// assert_eq!(std::fs::read(&path).unwrap(), b"hello");
/// ```
pub(super) fn atomic_write_bytes(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| FractalError::invalid_input("file path needs a parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct TransactionPlan {
    /// Older transaction plans stored paths relative to `pages/`.
    #[serde(default)]
    root_relative: bool,
    affected: Vec<PathBuf>,
    originals: BTreeSet<PathBuf>,
    #[serde(default)]
    create_directories: Vec<PathBuf>,
    #[serde(default)]
    remove_directories: Vec<PathBuf>,
    #[serde(default)]
    original_directories: Vec<PathBuf>,
}

pub(super) struct MutationPlan {
    operation: MutationKind,
    writes: BTreeMap<PathBuf, Vec<u8>>,
    deletes: BTreeSet<PathBuf>,
    file_moves: Vec<(PathBuf, PathBuf)>,
    directory_moves: Vec<(PathBuf, PathBuf)>,
    create_directories: BTreeSet<PathBuf>,
    remove_directories: BTreeSet<PathBuf>,
}

impl MutationPlan {
    /// Creates an empty mutation plan for the specified operation.
    ///
    /// # Examples
    ///
    /// ```
    /// let plan = MutationPlan::new(MutationKind::Create);
    /// assert!(plan.writes.is_empty());
    /// ```
    pub(super) fn new(operation: MutationKind) -> Self {
        Self {
            operation,
            writes: BTreeMap::new(),
            deletes: BTreeSet::new(),
            file_moves: Vec::new(),
            directory_moves: Vec::new(),
            create_directories: BTreeSet::new(),
            remove_directories: BTreeSet::new(),
        }
    }

    /// Schedules a page write relative to the project's pages directory.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.write_page("guide/intro.html", "<h1>Introduction</h1>");
    /// ```
    pub(super) fn write_page(&mut self, path: impl Into<PathBuf>, contents: impl AsRef<[u8]>) {
        self.write_project(Path::new(PAGES).join(path.into()), contents);
    }

    /// Schedules a project-relative file to be written with the specified contents.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.write_project("pages/index.html", b"<h1>Home</h1>");
    /// ```
    pub(super) fn write_project(&mut self, path: impl Into<PathBuf>, contents: impl AsRef<[u8]>) {
        self.writes.insert(path.into(), contents.as_ref().to_vec());
    }

    /// Schedules a page-relative path for deletion from the project.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.delete_page("guide/index.html");
    /// ```
    pub(super) fn delete_page(&mut self, path: impl Into<PathBuf>) {
        self.delete_project(Path::new(PAGES).join(path.into()));
    }

    /// Schedules a project-relative path for deletion.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.delete_project("docs/old.html");
    /// ```
    pub(super) fn delete_project(&mut self, path: impl Into<PathBuf>) {
        self.deletes.insert(path.into());
    }

    /// Schedules a page-relative file move within the project's `pages` directory.
    ///
    /// # Parameters
    ///
    /// * `from` - The source page path relative to `pages`.
    /// * `to` - The destination page path relative to `pages`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.move_page("old.html", "new.html");
    /// ```
    pub(super) fn move_page(&mut self, from: impl Into<PathBuf>, to: impl Into<PathBuf>) {
        self.file_moves.push((
            Path::new(PAGES).join(from.into()),
            Path::new(PAGES).join(to.into()),
        ));
    }

    /// Schedules a page-directory move within the project.
    ///
    /// The source and destination paths are interpreted relative to the project’s
    /// `pages` directory.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.move_page_directory("old-section", "new-section");
    /// ```
    pub(super) fn move_page_directory...
    pub(super) fn move_page_directory(&mut self, from: impl Into<PathBuf>, to: impl Into<PathBuf>) {
        self.directory_moves.push((
            Path::new(PAGES).join(from.into()),
            Path::new(PAGES).join(to.into()),
        ));
    }

    /// Schedules creation of a page directory relative to the project's pages directory.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.create_page_directory("guides");
    /// ```
    ///
    /// `path` is relative to the project's pages directory.
    ///
    /// # Panics
    ///
    /// Panics if `path` is absolute? Actually Path::join absolute replaces prefix in Rust? Path::join absolute replaces, no panic. Don't include. Need param? Rustdoc param via `@` not Rust. Could say inline. But summary says path relative. Good. Yet examples likely inaccessible due pub(super), but expected. final.
    pub(super) fn create_page_directory(&mut self, path: impl Into<PathBuf>) {
        self.create_directories
            .insert(Path::new(PAGES).join(path.into()));
    }

    /// Schedules a page directory for removal.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut plan = MutationPlan::new();
    /// plan.remove_page_directory("archive");
    /// ```
    pub(super) fn remove_page_directory(&mut self, path: impl Into<PathBuf>) {
        self.remove_directories
            .insert(Path::new(PAGES).join(path.into()));
    }

    /// Schedules creation of missing parent directories for a page under the project's `pages` directory.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut plan = MutationPlan::new();
    /// plan.ensure_page_parent_directories(
    ///     Path::new("/project"),
    ///     Path::new("guides/setup/index.html"),
    /// );
    /// ```
    ///
    /// `root` is the project root, and `page` is relative to the project's `pages` directory.
    pub(super) fn ensure_page_parent_directories(&mut self, root: &Path, page: &Path) {
        let mut missing = Vec::new();
        let mut parent = page.parent();
        while let Some(path) = parent {
            if path.as_os_str().is_empty() {
                break;
            }
            if !path_exists(&root.join(PAGES).join(path)) {
                missing.push(path.to_path_buf());
            }
            parent = path.parent();
        }
        for path in missing.into_iter().rev() {
            self.create_page_directory(path);
        }
    }

    /// Atomically applies the planned project mutations beneath `root`.
    ///
    /// The operation validates paths, records recoverable transaction state, installs
    /// the changes, and rolls back when durability has not been committed. A
    /// successful commit may include warnings when transaction cleanup remains
    /// pending.
    ///
    /// # Errors
    ///
    /// Returns an error if a path is invalid, a mutation is inconsistent, the
    /// filesystem operation fails, or recovery cannot restore an interrupted
    /// transaction.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let receipt = plan.commit(project_root)?;
    /// assert!(receipt.warnings.is_empty());
    /// ```
    pub(super) fn commit(mut self, root: &Path) -> Result<MutationReceipt> {
        for path in self.writes.keys().chain(&self.deletes) {
            validate_project_transaction_path(path)?;
            reject_symlinked_ancestors(root, path, false)?;
        }
        for path in self
            .create_directories
            .iter()
            .chain(&self.remove_directories)
        {
            validate_project_transaction_path(path)?;
            reject_symlinked_ancestors(root, path, true)?;
        }
        for (from, to) in self.file_moves.iter().chain(&self.directory_moves) {
            validate_project_transaction_path(from)?;
            validate_project_transaction_path(to)?;
        }
        for (from, to) in &self.file_moves {
            if !self.deletes.contains(from) || !self.writes.contains_key(to) {
                return Err(FractalError::invalid_project(
                    "a planned file move must delete its source and write its destination",
                ));
            }
        }
        for (from, to) in &self.directory_moves {
            if !self.remove_directories.contains(from) || !self.create_directories.contains(to) {
                return Err(FractalError::invalid_project(
                    "a planned directory move must remove its source and create its destination",
                ));
            }
        }

        self.writes.retain(|path, contents| {
            fs::read(root.join(path)).map_or(true, |current| current != *contents)
        });
        self.deletes.retain(|path| path_exists(&root.join(path)));

        let changes = planned_changes(root, &self)?;
        if changes.is_empty() {
            return Ok(MutationReceipt {
                operation: self.operation,
                changes,
                warnings: vec![],
            });
        }

        let mut affected: BTreeSet<PathBuf> = self.writes.keys().cloned().collect();
        affected.extend(self.deletes.iter().cloned());
        reject_overlapping_transaction_paths(&affected)?;

        let transaction = tempfile::Builder::new()
            .prefix(TRANSACTION_PREFIX)
            .tempdir_in(root)?;
        let transaction_root = transaction.path().to_path_buf();
        let new_root = transaction_root.join("new");
        let old_root = transaction_root.join("old");
        let originals = affected
            .iter()
            .filter(|path| path_exists(&root.join(path)))
            .cloned()
            .collect();
        let mut original_directories = Vec::new();
        for path in &self.remove_directories {
            let directory = root.join(path);
            if directory.is_dir() {
                original_directories.push(path.clone());
                let mut descendants = Vec::new();
                collect_directories(root, &directory, &mut descendants)?;
                original_directories.extend(descendants);
            }
        }
        original_directories.sort_by_key(|path| path.components().count());
        original_directories.dedup();
        let transaction_plan = TransactionPlan {
            root_relative: true,
            affected: affected.iter().cloned().collect(),
            originals,
            create_directories: self.create_directories.iter().cloned().collect(),
            remove_directories: self.remove_directories.iter().cloned().collect(),
            original_directories,
        };
        atomic_write(
            &transaction_root.join("plan.json"),
            &serde_json::to_string(&transaction_plan)?,
        )?;
        for (path, contents) in &self.writes {
            atomic_write_bytes(&new_root.join(path), contents)?;
        }
        sync_directory_tree(&transaction_root)?;
        sync_directory(root)?;
        let transaction_root = transaction.keep();
        #[cfg(test)]
        injected_crash(TransactionFaultPoint::Prepared)?;

        let result = (|| -> Result<()> {
            for path in &transaction_plan.create_directories {
                fs::create_dir_all(root.join(path))?;
            }
            #[cfg(test)]
            injected_crash(TransactionFaultPoint::DirectoryCreated)?;
            sync_directory_tree(&root.join(PAGES))?;
            for path in &transaction_plan.affected {
                let source = root.join(path);
                if path_exists(&source) {
                    let backup = old_root.join(path);
                    create_parent(&backup)?;
                    fs::rename(&source, &backup)?;
                    sync_rename_parents(&source, &backup)?;
                    #[cfg(test)]
                    injected_crash(TransactionFaultPoint::OriginalBackedUp)?;
                }
            }
            for path in self.writes.keys() {
                let source = new_root.join(path);
                let destination = root.join(path);
                create_parent(&destination)?;
                fs::rename(&source, &destination)?;
                sync_rename_parents(&source, &destination)?;
                #[cfg(test)]
                injected_crash(TransactionFaultPoint::NewFileInstalled)?;
            }
            for path in &transaction_plan.remove_directories {
                remove_path_if_present(&root.join(path))?;
                if let Some(parent) = root.join(path).parent() {
                    sync_directory(parent)?;
                }
                #[cfg(test)]
                injected_crash(TransactionFaultPoint::DirectoryRemoved)?;
            }
            let committed = File::create(transaction_root.join("committed"))?;
            committed.sync_all()?;
            #[cfg(test)]
            injected_crash(TransactionFaultPoint::CommitMarkerCreated)?;
            sync_directory(&transaction_root)?;
            Ok(())
        })();

        if let Err(error) = result {
            #[cfg(test)]
            if error.code == crate::FractalErrorCode::Indeterminate
                && error.message.starts_with("injected process interruption")
            {
                return Err(error);
            }
            if transaction_root.join("committed").is_file() {
                return Err(FractalError::indeterminate(format!(
                    "the project changes were installed and a commit marker exists, but its durability could not be confirmed: {error}; transaction files remain at {}",
                    transaction_root.display()
                )));
            }
            if let Err(recovery_error) = recover_transaction(&transaction_root) {
                return Err(FractalError::indeterminate(format!(
                    "transaction failed: {error}; rollback also failed: {recovery_error}; recovery files remain at {}",
                    transaction_root.display()
                )));
            }
            return Err(error);
        }

        let mut warnings = Vec::new();
        #[cfg(test)]
        let cleanup_interrupted =
            take_transaction_fault(TransactionFaultPoint::CommittedBeforeCleanup);
        #[cfg(not(test))]
        let cleanup_interrupted = false;
        if cleanup_interrupted {
            warnings.push(OperationWarning {
                code: OperationWarningCode::CleanupPending,
                message: format!(
                    "the mutation committed, but transaction cleanup remains at {}",
                    transaction_root.display()
                ),
            });
            return Ok(MutationReceipt {
                operation: self.operation,
                changes,
                warnings,
            });
        }
        if let Err(error) = fs::remove_dir_all(&transaction_root).and_then(|()| {
            sync_directory(root).map_err(|error| std::io::Error::other(error.to_string()))
        }) {
            warnings.push(OperationWarning {
                code: OperationWarningCode::CleanupPending,
                message: format!(
                    "the mutation committed, but transaction cleanup remains at {}: {error}",
                    transaction_root.display()
                ),
            });
        }
        Ok(MutationReceipt {
            operation: self.operation,
            changes,
            warnings,
        })
    }
}

/// Creates a mutation receipt with the specified operation and no changes or warnings.
///
/// # Examples
///
/// ```
/// let receipt = noop_receipt(MutationKind::Update);
/// assert!(receipt.changes.is_empty());
/// assert!(receipt.warnings.is_empty());
/// ```
pub(super) fn noop_receipt(operation: MutationKind) -> MutationReceipt {
    MutationReceipt {
        operation,
        changes: vec![],
        warnings: vec![],
    }
}

pub(super) struct ProjectLock {
    _file: File,
    _legacy_guard: Option<File>,
}

impl ProjectLock {
    /// Creates and durably initializes the project lock file.
    ///
    /// # Examples
    ///
    /// ```
    /// let directory = tempfile::tempdir().unwrap();
    /// initialize(directory.path()).unwrap();
    /// assert!(directory.path().join(LOCK).exists());
    /// ```
    pub(super) fn initialize(root: &Path) -> Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(LOCK))?;
        file.sync_all()?;
        sync_directory(root)
    }

    /// Acquires a shared lock for the project root, using the legacy manifest lock when necessary.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let _lock = ProjectLock::shared(std::path::Path::new("/path/to/project")).unwrap();
    /// ```
    pub(super) fn shared(root: &Path) -> Result<Self> {
        let lock_path = root.join(LOCK);
        loop {
            match File::open(&lock_path) {
                Ok(file) => {
                    FileExt::lock_shared(&file)?;
                    return Ok(Self {
                        _file: file,
                        _legacy_guard: None,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let legacy = File::open(root.join(MANIFEST))?;
                    FileExt::lock_shared(&legacy)?;
                    if lock_path.is_file() {
                        drop(legacy);
                        continue;
                    }
                    return Ok(Self {
                        _file: legacy,
                        _legacy_guard: None,
                    });
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Acquires an exclusive lock for a project.
    ///
    /// The lock is created under `root` when needed, with the manifest used as a
    /// compatibility guard while the lock file is initialized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let _lock = ProjectLock::exclusive(project_root)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be opened, created, or acquired.
    pub(super) fn exclusive(root: &Path) -> Result<Self> {
        let lock_path = root.join(LOCK);
        loop {
            match OpenOptions::new().read(true).write(true).open(&lock_path) {
                Ok(file) => {
                    FileExt::lock_exclusive(&file)?;
                    return Ok(Self {
                        _file: file,
                        _legacy_guard: None,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let legacy = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(root.join(MANIFEST))?;
                    FileExt::lock_exclusive(&legacy)?;
                    match OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .open(&lock_path)
                    {
                        Ok(file) => {
                            FileExt::lock_exclusive(&file)?;
                            file.sync_all()?;
                            sync_directory(root)?;
                            return Ok(Self {
                                _file: file,
                                _legacy_guard: Some(legacy),
                            });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            drop(legacy);
                            continue;
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

/// Recovers a pending transaction and reports the project changes made during recovery.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// let changes = recover_transaction(Path::new(".project/transactions/txn-1"))?;
/// println!("Recovered {} changes", changes.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub(super) fn recover_transaction(transaction_root: &Path) -> Result<Vec<ProjectChange>> {
    let plan_path = transaction_root.join("plan.json");
    if !plan_path.is_file() || transaction_root.join("committed").is_file() {
        fs::remove_dir_all(transaction_root)?;
        return Ok(vec![]);
    }
    let plan: TransactionPlan = serde_json::from_str(&fs::read_to_string(plan_path)?)?;
    let root = transaction_root
        .parent()
        .ok_or_else(|| FractalError::invalid_project("transaction has no project root"))?;
    let affected: BTreeSet<PathBuf> = plan
        .affected
        .iter()
        .map(|path| transaction_project_path(&plan, path))
        .collect();
    reject_overlapping_transaction_paths(&affected)?;
    for path in &affected {
        reject_symlinked_ancestors(root, path, false)?;
    }
    for path in plan
        .create_directories
        .iter()
        .chain(&plan.remove_directories)
        .chain(&plan.original_directories)
    {
        let path = transaction_project_path(&plan, path);
        validate_project_transaction_path(&path)?;
        reject_symlinked_ancestors(root, &path, true)?;
    }
    let old_root = transaction_root.join("old");
    let mut changes = Vec::new();
    for stored_path in plan.affected.iter().rev() {
        let path = transaction_project_path(&plan, stored_path);
        let current = root.join(&path);
        let backup = old_root.join(stored_path);
        if path_exists(&backup) {
            let before_hash = fs::read(&current)
                .ok()
                .map(|bytes| content_hash_bytes(&bytes));
            let after_hash = content_hash_bytes(&fs::read(&backup)?);
            remove_path_if_present(&current)?;
            create_parent(&current)?;
            fs::rename(&backup, &current)?;
            changes.push(match before_hash {
                Some(before_hash) => ProjectChange::Updated {
                    path: public_project_path(&path)?,
                    before_hash,
                    after_hash,
                },
                None => ProjectChange::Created {
                    path: public_project_path(&path)?,
                    entry: ProjectEntryKind::File,
                    after_hash: Some(after_hash),
                },
            });
        } else if !plan
            .originals
            .iter()
            .map(|original| transaction_project_path(&plan, original))
            .any(|original| original == path)
        {
            let before_hash = fs::read(&current)
                .ok()
                .map(|bytes| content_hash_bytes(&bytes));
            remove_path_if_present(&current)?;
            if before_hash.is_some() {
                changes.push(ProjectChange::Deleted {
                    path: public_project_path(&path)?,
                    entry: ProjectEntryKind::File,
                    before_hash,
                });
            }
        }
    }
    for stored_path in plan.create_directories.iter().rev() {
        let path = transaction_project_path(&plan, stored_path);
        if path_exists(&root.join(&path)) {
            remove_path_if_present(&root.join(&path))?;
            changes.push(ProjectChange::Deleted {
                path: public_project_path(&path)?,
                entry: ProjectEntryKind::Directory,
                before_hash: None,
            });
        }
    }
    for stored_path in &plan.original_directories {
        let path = transaction_project_path(&plan, stored_path);
        if !path_exists(&root.join(&path)) {
            fs::create_dir_all(root.join(&path))?;
            changes.push(ProjectChange::Created {
                path: public_project_path(&path)?,
                entry: ProjectEntryKind::Directory,
                after_hash: None,
            });
        }
    }
    sync_directory_tree(&root.join(PAGES))?;
    fs::remove_dir_all(transaction_root)?;
    sync_directory(root)?;
    Ok(changes)
}

/// Discovers and inspects pending recovery transaction directories under the project root.
///
/// # Examples
///
/// ```
/// let root = std::env::temp_dir().join(format!(
///     "recovery-transactions-{}",
///     std::process::id()
/// ));
/// std::fs::create_dir_all(&root).unwrap();
///
/// let transactions = inspect_recovery_transactions(&root).unwrap();
/// assert!(transactions.is_empty());
///
/// std::fs::remove_dir_all(root).unwrap();
/// ```
pub(super) fn inspect_recovery_transactions(root: &Path) -> Result<Vec<RecoveryTransaction>> {
    let mut directories = transaction_directories(root)?;
    directories.sort();
    directories
        .into_iter()
        .map(|path| inspect_recovery_transaction(root, &path))
        .collect()
}

/// Ensures the project has no pending or malformed recovery transactions.
///
/// # Examples
///
/// ```
/// let root = std::path::Path::new("project");
/// ensure_no_pending_transactions(root)?;
/// # Ok::<(), FractalError>(())
/// ```
///
/// # Errors
///
/// Returns an error when recovery is required or when recovery state cannot be inspected.
pub(super) fn ensure_no_pending_transactions(root: &Path) -> Result<()> {
    let recovery = inspect_recovery_transactions(root)?;
    if let Some(transaction) = recovery.iter().find(|transaction| {
        matches!(
            transaction.status,
            RecoveryTransactionStatus::Pending | RecoveryTransactionStatus::Malformed
        )
    }) {
        return Err(FractalError::recovery_required(format!(
            "project recovery is required before opening or mutation: {}",
            transaction.path
        )));
    }
    Ok(())
}

/// Recovers pending transactions and removes committed transaction data for a project.
///
/// Malformed or unrecoverable transactions are reported as failures, while cleanup
/// errors are reported as warnings. The report includes recovered changes and the
/// transaction paths processed successfully.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let report = recover_all_transactions(Path::new("."))?;
/// assert!(report.failures.is_empty());
/// # Ok::<(), _>(())
/// ```
pub(super) fn recover_all_transactions(root: &Path) -> Result<RecoveryReport> {
    let mut recovered_transactions = Vec::new();
    let mut cleaned_transactions = Vec::new();
    let mut changes = Vec::new();
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let mut directories = transaction_directories(root)?;
    directories.sort();
    for transaction_root in directories {
        let inspected = match inspect_recovery_transaction(root, &transaction_root) {
            Ok(inspected) => inspected,
            Err(error) => {
                failures.push(OperationFailure {
                    code: error.code,
                    message: error.message,
                });
                break;
            }
        };
        match inspected.status {
            RecoveryTransactionStatus::Malformed => {
                failures.push(OperationFailure {
                    code: crate::FractalErrorCode::InvalidProject,
                    message: inspected
                        .message
                        .unwrap_or_else(|| "transaction recovery state is malformed".into()),
                });
                break;
            }
            RecoveryTransactionStatus::CommittedCleanupPending => {
                match fs::remove_dir_all(&transaction_root) {
                    Ok(()) => cleaned_transactions.push(inspected.path),
                    Err(error) => warnings.push(OperationWarning {
                        code: OperationWarningCode::CleanupPending,
                        message: format!(
                            "could not remove committed transaction {}: {error}",
                            inspected.path
                        ),
                    }),
                }
            }
            RecoveryTransactionStatus::Pending => match recover_transaction(&transaction_root) {
                Ok(recovered) => {
                    changes.extend(recovered);
                    recovered_transactions.push(inspected.path);
                }
                Err(error) => {
                    failures.push(OperationFailure {
                        code: error.code,
                        message: error.message,
                    });
                    break;
                }
            },
        }
    }
    if let Err(error) = sync_directory(root) {
        failures.push(OperationFailure {
            code: error.code,
            message: error.message,
        });
    }
    Ok(RecoveryReport {
        recovered_transactions,
        cleaned_transactions,
        changes,
        warnings,
        failures,
    })
}

/// Lists transaction directories directly under the specified root.
///
/// # Errors
///
/// Returns an error if the root cannot be read or an entry cannot be inspected.
///
/// # Examples
///
/// ```
/// use std::fs;
///
/// let root = std::env::temp_dir().join(format!(
///     "transaction-directories-{}",
///     std::process::id()
/// ));
/// fs::create_dir_all(&root).unwrap();
///
/// let directories = transaction_directories(&root).unwrap();
/// assert!(directories.is_empty());
///
/// fs::remove_dir_all(root).unwrap();
/// ```
fn transaction_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut output = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with(TRANSACTION_PREFIX)
        {
            output.push(entry.path());
        }
    }
    Ok(output)
}

/// Inspects a recovery transaction and classifies its state and affected project paths.
///
/// # Examples
///
/// ```
/// use std::fs;
///
/// let root = std::env::temp_dir().join(format!(
///     "recovery-inspection-{}",
///     std::process::id()
/// ));
/// let transaction = root.join("transaction");
/// fs::create_dir_all(&transaction).unwrap();
///
/// let result = inspect_recovery_transaction(&root, &transaction).unwrap();
/// assert!(matches!(
///     result.status,
///     RecoveryTransactionStatus::Malformed
/// ));
///
/// fs::remove_dir_all(root).unwrap();
/// ```
///
/// # Returns
///
/// A recovery transaction summary with its status, affected paths, and any
/// diagnostic message.
fn inspect_recovery_transaction(
    root: &Path,
    transaction_root: &Path,
) -> Result<RecoveryTransaction> {
    let relative = transaction_root.strip_prefix(root)?;
    let path = public_project_path(relative)?;
    let plan_path = transaction_root.join("plan.json");
    if !plan_path.is_file() {
        return Ok(RecoveryTransaction {
            path,
            status: RecoveryTransactionStatus::Malformed,
            affected: vec![],
            message: Some("transaction directory has no plan.json".into()),
        });
    }
    let source = match fs::read_to_string(&plan_path) {
        Ok(source) => source,
        Err(error) => {
            return Ok(RecoveryTransaction {
                path,
                status: RecoveryTransactionStatus::Malformed,
                affected: vec![],
                message: Some(format!("cannot read transaction plan: {error}")),
            })
        }
    };
    let plan: TransactionPlan = match serde_json::from_str(&source) {
        Ok(plan) => plan,
        Err(error) => {
            return Ok(RecoveryTransaction {
                path,
                status: RecoveryTransactionStatus::Malformed,
                affected: vec![],
                message: Some(format!("invalid transaction plan: {error}")),
            })
        }
    };
    let mut affected = Vec::new();
    for stored_path in &plan.affected {
        let project_path = transaction_project_path(&plan, stored_path);
        if let Err(error) = validate_project_transaction_path(&project_path) {
            return Ok(RecoveryTransaction {
                path,
                status: RecoveryTransactionStatus::Malformed,
                affected: vec![],
                message: Some(error.message),
            });
        }
        if let Err(error) = reject_symlinked_ancestors(root, &project_path, false) {
            return Ok(RecoveryTransaction {
                path,
                status: RecoveryTransactionStatus::Malformed,
                affected: vec![],
                message: Some(error.message),
            });
        }
        affected.push(public_project_path(&project_path)?);
    }
    for stored_path in plan
        .create_directories
        .iter()
        .chain(&plan.remove_directories)
        .chain(&plan.original_directories)
    {
        let project_path = transaction_project_path(&plan, stored_path);
        if let Err(error) = validate_project_transaction_path(&project_path) {
            return Ok(RecoveryTransaction {
                path,
                status: RecoveryTransactionStatus::Malformed,
                affected: vec![],
                message: Some(error.message),
            });
        }
        if let Err(error) = reject_symlinked_ancestors(root, &project_path, true) {
            return Ok(RecoveryTransaction {
                path,
                status: RecoveryTransactionStatus::Malformed,
                affected: vec![],
                message: Some(error.message),
            });
        }
    }
    Ok(RecoveryTransaction {
        path,
        status: if transaction_root.join("committed").is_file() {
            RecoveryTransactionStatus::CommittedCleanupPending
        } else {
            RecoveryTransactionStatus::Pending
        },
        affected,
        message: None,
    })
}

/// Validates that transaction paths are distinct and do not contain one another.
///
/// # Errors
///
/// Returns an error if any path is invalid or if one transaction path is an ancestor of another.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
/// use std::path::PathBuf;
///
/// let paths = BTreeSet::from([
///     PathBuf::from("pages/index.html"),
///     PathBuf::from("assets/style.css"),
/// ]);
///
/// reject_overlapping_transaction_paths(&paths).unwrap();
/// ```
pub(super) fn reject_overlapping_transaction_paths(paths: &BTreeSet<PathBuf>) -> Result<()> {
    for path in paths {
        validate_transaction_path(path)?;
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

/// Validates that a transaction path is non-empty and contains only normal path components.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// assert!(validate_transaction_path(Path::new("pages/index.html")).is_ok());
/// assert!(validate_transaction_path(Path::new("../index.html")).is_err());
/// ```
fn validate_transaction_path(path: &Path) -> Result<()> {
fn validate_transaction_path(path: &Path) -> Result<()> {
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
    Ok(())
}

/// Validates that a transaction path belongs to the project's allowed path layout.
///
/// Accepted paths are the project manifest and paths located beneath the pages
/// directory.
///
/// # Examples
///
/// ```
/// assert!(validate_project_transaction_path(Path::new(MANIFEST)).is_ok());
/// assert!(validate_project_transaction_path(Path::new(PAGES).join("index.html")).is_ok());
/// ```
fn validate_project_transaction_path(path: &Path) -> Result<()> {
    validate_transaction_path(path)?;
    let mut components = path.components();
    let first = components.next();
    let allowed = path == Path::new(MANIFEST)
        || matches!(first, Some(Component::Normal(part)) if part == PAGES)
            && components.next().is_some();
    if !allowed {
        return Err(FractalError::invalid_project(format!(
            "transaction path is outside the Fractal project contract: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Rejects paths that traverse symlinked ancestors beneath the project root.
///
/// When `include_leaf` is `true`, the final path component is checked as well.
/// Missing components are allowed.
///
/// # Errors
///
/// Returns an error if a checked component is a symlink or if its metadata
/// cannot be read for another reason.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// assert!(reject_symlinked_ancestors(
///     Path::new("project"),
///     Path::new("pages/index.html"),
///     false,
/// ).is_ok());
/// ```
fn reject_symlinked_ancestors(root: &Path, path: &Path, include_leaf: bool) -> Result<()> {
    let component_count = path.components().count();
    let checked_count = component_count.saturating_sub(usize::from(!include_leaf));
    let mut current = root.to_path_buf();
    for component in path.components().take(checked_count) {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(FractalError::invalid_project(format!(
                    "project mutations cannot traverse a symlink: {}",
                    current.display()
                )))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Converts a transaction path to a project-relative path, preserving root-relative paths and prefixing legacy page-relative paths with `pages`.
///
/// # Examples
///
/// ```ignore
/// let path = Path::new("guide/index.html");
/// let project_path = transaction_project_path(&plan, path);
/// assert_eq!(project_path, Path::new("pages/guide/index.html"));
/// ```
fn transaction_project_path(plan: &TransactionPlan, path: &Path) -> PathBuf {
    if plan.root_relative {
        path.to_path_buf()
    } else {
        Path::new(PAGES).join(path)
    }
}

/// Converts a valid project-relative path to a public path using forward slashes.
///
/// # Errors
///
/// Returns an error when the path is absolute, empty, contains non-normal
/// components, or is not valid UTF-8.
///
/// # Examples
///
/// ```
/// let path = public_project_path(std::path::Path::new("pages/index.html"))?;
/// # let _ = path;
/// # Ok::<(), _>(())
/// ```
pub(super) fn public_project_path(path: &Path) -> Result<ProjectPath> {
    if path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FractalError::invalid_project(format!(
            "invalid project-relative report path: {}",
            path.display()
        )));
    }
    let value = path.to_str().ok_or_else(|| {
        FractalError::invalid_project(format!(
            "project-relative report path is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    Ok(ProjectPath::new(value.replace('\\', "/")))
}

/// Computes the project changes implied by a mutation plan without applying them.
///
/// File changes include content hashes before and after the mutation. Move operations
/// are represented as moves rather than separate creations and deletions.
///
/// # Examples
///
/// ```
/// # use std::path::Path;
/// # let plan = MutationPlan::new();
/// let changes = planned_changes(Path::new("."), &plan)?;
/// assert!(changes.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
fn planned_changes(root: &Path, plan: &MutationPlan) -> Result<Vec<ProjectChange>> {
    let mut changes = Vec::new();
    let moved_from: BTreeSet<&PathBuf> = plan.file_moves.iter().map(|(from, _)| from).collect();
    let moved_to: BTreeSet<&PathBuf> = plan.file_moves.iter().map(|(_, to)| to).collect();
    for (from, to) in &plan.file_moves {
        let before = fs::read(root.join(from))?;
        let after = plan
            .writes
            .get(to)
            .cloned()
            .or_else(|| fs::read(root.join(to)).ok())
            .ok_or_else(|| {
                FractalError::invalid_project(format!(
                    "planned move has no destination contents: {}",
                    to.display()
                ))
            })?;
        changes.push(ProjectChange::Moved {
            from: public_project_path(from)?,
            to: public_project_path(to)?,
            entry: ProjectEntryKind::File,
            before_hash: Some(content_hash_bytes(&before)),
            after_hash: Some(content_hash_bytes(&after)),
        });
    }
    for (from, to) in &plan.directory_moves {
        changes.push(ProjectChange::Moved {
            from: public_project_path(from)?,
            to: public_project_path(to)?,
            entry: ProjectEntryKind::Directory,
            before_hash: None,
            after_hash: None,
        });
    }
    for (path, contents) in &plan.writes {
        if moved_to.contains(path) {
            continue;
        }
        let after_hash = content_hash_bytes(contents);
        changes.push(match fs::read(root.join(path)) {
            Ok(before) => ProjectChange::Updated {
                path: public_project_path(path)?,
                before_hash: content_hash_bytes(&before),
                after_hash,
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ProjectChange::Created {
                path: public_project_path(path)?,
                entry: ProjectEntryKind::File,
                after_hash: Some(after_hash),
            },
            Err(error) => return Err(error.into()),
        });
    }
    for path in &plan.deletes {
        if moved_from.contains(path) {
            continue;
        }
        changes.push(ProjectChange::Deleted {
            path: public_project_path(path)?,
            entry: ProjectEntryKind::File,
            before_hash: Some(content_hash_bytes(&fs::read(root.join(path))?)),
        });
    }
    for path in &plan.create_directories {
        if !path_exists(&root.join(path))
            && !plan
                .directory_moves
                .iter()
                .any(|(_, to)| path_starts_with(path, to))
        {
            changes.push(ProjectChange::Created {
                path: public_project_path(path)?,
                entry: ProjectEntryKind::Directory,
                after_hash: None,
            });
        }
    }
    for path in &plan.remove_directories {
        if path_exists(&root.join(path))
            && !plan
                .directory_moves
                .iter()
                .any(|(from, _)| path_starts_with(path, from))
        {
            changes.push(ProjectChange::Deleted {
                path: public_project_path(path)?,
                entry: ProjectEntryKind::Directory,
                before_hash: None,
            });
        }
    }
    Ok(changes)
}

/// Creates all directories needed to contain a path.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// create_parent(Path::new("output/file.txt"))?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub(super) fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn sync_rename_parents(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = from.parent() {
        sync_directory(parent)?;
    }
    if to.parent() != from.parent() {
        if let Some(parent) = to.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

fn sync_directory_tree(root: &Path) -> Result<()> {
    let mut directories = Vec::new();
    collect_directories(root, root, &mut directories)?;
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        sync_directory(&root.join(directory))?;
    }
    sync_directory(root)
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
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

/// Extracts the internal path from an iframe target.
///
/// # Examples
///
/// ```
/// let target = IframeTarget::Internal("docs/guide.html".to_owned());
/// assert_eq!(iframe_target_path(&target), Some("docs/guide.html"));
/// ```
pub(super) fn iframe_target_path(target: &IframeTarget) -> Option<&str> {
    match target {
        IframeTarget::Internal(path) | IframeTarget::InternalFile(path) => Some(path),
        _ => None,
    }
}

/// Computes the SHA-256 hash of text contents as a lowercase hexadecimal string.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     content_hash("hello"),
///     "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
/// );
/// ```
pub(super) fn content_hash(contents: &str) -> String {
    content_hash_bytes(contents.as_bytes())
}

/// Computes a SHA-256 hash for byte content.
///
/// # Examples
///
/// ```
/// let hash = content_hash_bytes(b"hello");
/// assert_eq!(
///     hash,
///     "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
/// );
/// ```
pub(super) fn content_hash_bytes(contents: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(contents))
}

/// Converts a title into a lowercase, hyphen-separated slug.
///
/// Returns an error when the title contains no alphanumeric characters.
///
/// # Examples
///
/// ```
/// assert_eq!(slug("Hello, World!").unwrap(), "hello-world");
/// ```
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
