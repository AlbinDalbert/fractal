use super::support::*;
use super::*;

impl Project {
    /// Returns folder metadata and its children in effective display order.
    ///
    /// An empty path addresses the pages root.
    pub fn folder(&self, path: impl AsRef<Path>) -> Result<Folder> {
        let path = normalize_folder_path(path.as_ref())?;
        self.folders
            .get(&path_string(&path))
            .map(|stored| stored.folder.clone())
            .ok_or_else(|| {
                FractalError::not_found(format!(
                    "folder does not exist: {}",
                    display_folder_path(&path)
                ))
            })
    }

    /// Creates a folder below an existing parent.
    ///
    /// Fractal derives the directory name from `title`. It creates the
    /// directory and its metadata, then updates explicit parent order in the
    /// same recoverable transaction.
    pub fn create_folder(
        &mut self,
        parent: impl AsRef<Path>,
        title: &str,
    ) -> Result<MutationReceipt> {
        let parent = normalize_folder_path(parent.as_ref())?;
        let title = title.trim();
        if title.is_empty() {
            return Err(FractalError::invalid_input("folder title cannot be empty"));
        }
        let name = slug(title)?;
        let folder = parent.join(&name);

        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        if !self.folders.contains_key(&path_string(&parent)) {
            return Err(FractalError::not_found(format!(
                "parent folder does not exist: {}",
                display_folder_path(&parent)
            )));
        }
        if path_exists(&self.root.join(PAGES).join(&folder)) {
            return Err(FractalError::already_exists(format!(
                "folder already exists: {}",
                folder.display()
            )));
        }

        let metadata = FolderMetadata {
            title: title.to_owned(),
            order: None,
        };
        let mut plan = MutationPlan::new(MutationKind::CreateFolder);
        plan.create_page_directory(folder.clone());
        plan.write_page(
            folder_metadata_relative_path(&folder),
            serde_json::to_string_pretty(&metadata)?,
        );
        if let Some((path, contents)) =
            self.folder_metadata_child_change(&parent, None, Some(&name))?
        {
            plan.write_page(path, contents);
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }

    /// Changes a folder title and, outside the pages root, renames the folder to
    /// the title-derived path.
    ///
    /// The rename, parent ordering update, and native reference rewrites commit
    /// as one recoverable mutation.
    pub fn set_folder_title(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
    ) -> Result<MutationReceipt> {
        if title.trim().is_empty() {
            return Err(FractalError::invalid_input("folder title cannot be empty"));
        }
        let path = normalize_folder_path(path.as_ref())?;
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let stored = self.folders.get(&path_string(&path)).ok_or_else(|| {
            FractalError::not_found(format!(
                "folder does not exist: {}",
                display_folder_path(&path)
            ))
        })?;
        if !path.as_os_str().is_empty() {
            let destination = path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(slug(title)?);
            if destination != path && path_exists(&self.root.join(PAGES).join(&destination)) {
                return Err(FractalError::already_exists(format!(
                    "folder already exists: {}",
                    destination.display()
                )));
            }
        }
        let metadata = FolderMetadata {
            title: title.trim().to_owned(),
            order: stored
                .metadata
                .as_ref()
                .and_then(|value| value.order.clone()),
        };
        let metadata_contents = serde_json::to_string_pretty(&metadata)?;
        let receipt = if path.as_os_str().is_empty() {
            let mut plan = MutationPlan::new(MutationKind::SetFolderTitle);
            plan.write_page(folder_metadata_relative_path(&path), metadata_contents);
            plan.commit(&self.root)?
        } else {
            let destination = path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(slug(title)?);
            if destination == path {
                let mut plan = MutationPlan::new(MutationKind::SetFolderTitle);
                plan.write_page(folder_metadata_relative_path(&path), metadata_contents);
                plan.commit(&self.root)?
            } else {
                self.rename_folder(
                    &path,
                    &destination,
                    MutationKind::SetFolderTitle,
                    Some((
                        folder_metadata_relative_path(&destination),
                        metadata_contents,
                    )),
                )?
            }
        };
        self.reload()?;
        Ok(receipt)
    }

    /// Stores an explicit order for every present and missing child of a folder.
    ///
    /// `order` must contain each child name exactly once.
    pub fn reorder_folder<I, S>(
        &mut self,
        path: impl AsRef<Path>,
        order: I,
    ) -> Result<MutationReceipt>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let path = normalize_folder_path(path.as_ref())?;
        let order: Vec<String> = order.into_iter().map(Into::into).collect();
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let stored = self.folders.get(&path_string(&path)).ok_or_else(|| {
            FractalError::not_found(format!(
                "folder does not exist: {}",
                display_folder_path(&path)
            ))
        })?;
        let expected: BTreeSet<String> = stored
            .folder
            .children
            .iter()
            .map(|child| child.name.clone())
            .collect();
        let mut provided = BTreeSet::new();
        for name in &order {
            validate_order_name(name)?;
            if !provided.insert(name.clone()) {
                return Err(FractalError::invalid_input(format!(
                    "ordered child appears more than once: {name}"
                )));
            }
        }
        if provided != expected {
            let missing: Vec<_> = expected.difference(&provided).cloned().collect();
            let unknown: Vec<_> = provided.difference(&expected).cloned().collect();
            return Err(FractalError::invalid_input(format!(
                "order must contain every present and missing child exactly once; missing: [{}]; unknown: [{}]",
                missing.join(", "),
                unknown.join(", ")
            )));
        }
        let metadata = FolderMetadata {
            title: stored.folder.title.clone(),
            order: Some(order),
        };
        let metadata_path = folder_metadata_relative_path(&path);
        let mut plan = MutationPlan::new(MutationKind::ReorderFolder);
        plan.write_page(metadata_path, serde_json::to_string_pretty(&metadata)?);
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }

    /// Moves a folder and rewrites internal native document references to its
    /// descendants.
    ///
    /// The destination parent must exist and the destination name must remain
    /// consistent with the folder title.
    pub fn move_folder(
        &mut self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> Result<MutationReceipt> {
        let from = normalize_relative_path(from.as_ref())?;
        let to = normalize_relative_path(to.as_ref())?;
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let stored = self.folders.get(&path_string(&from)).ok_or_else(|| {
            FractalError::not_found(format!("folder does not exist: {}", from.display()))
        })?;
        let expected_name = slug(&stored.folder.title)?;
        if to.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
            return Err(FractalError::invalid_input(format!(
                "folder destination must end in `{expected_name}` to match its title"
            )));
        }
        if from == to {
            return Ok(noop_receipt(MutationKind::MoveFolder));
        }
        if to.starts_with(&from) {
            return Err(FractalError::invalid_input(
                "a folder cannot be moved inside itself",
            ));
        }
        let destination_parent = to.parent().unwrap_or_else(|| Path::new(""));
        if !self.folders.contains_key(&path_string(destination_parent)) {
            return Err(FractalError::not_found(format!(
                "destination folder does not exist: {}",
                display_folder_path(destination_parent)
            )));
        }
        self.rename_folder(&from, &to, MutationKind::MoveFolder, None)
    }
    /// Deletes a folder below `pages/`. Materialized folders use a single
    /// namespace rename.
    ///
    /// Deleting an ordered but missing folder only removes its ghost entry;
    /// references to nonexistent descendants are not checked.
    pub fn delete_folder(&mut self, path: impl AsRef<Path>) -> Result<MutationReceipt> {
        let folder = normalize_relative_path(path.as_ref())?;
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let absolute = self.root.join(PAGES).join(&folder);
        let exists =
            fs::symlink_metadata(&absolute).is_ok_and(|metadata| metadata.file_type().is_dir());
        let parent = folder.parent().unwrap_or_else(|| Path::new(""));
        let name = folder
            .file_name()
            .expect("folder has a name")
            .to_string_lossy();
        let is_ghost = self
            .folders
            .get(&path_string(parent))
            .is_some_and(|stored| {
                stored.folder.children.iter().any(|child| {
                    child.name == name
                        && child.kind == FolderChildKind::Folder
                        && child.status == FolderChildStatus::Missing
                })
            });
        if !exists && !is_ghost {
            return Err(FractalError::not_found(format!(
                "folder does not exist: {}",
                folder.display()
            )));
        }
        let mut deleted = Vec::new();
        if exists {
            collect_files(&self.root.join(PAGES), &absolute, &mut deleted)?;
            if deleted.iter().any(|path| !is_managed_folder_file(path)) {
                return Err(FractalError::invalid_input(
                    "folder contains unsupported content that Fractal does not manage",
                ));
            }
        }
        let targets: BTreeSet<String> = deleted.iter().map(|path| path_string(path)).collect();
        let deleted_pages: BTreeSet<String> = self
            .pages
            .keys()
            .filter(|path| path_starts_with(Path::new(path), &folder))
            .cloned()
            .collect();
        self.reject_references_into(&targets, &deleted_pages)?;
        let writes: Vec<_> = self
            .folder_metadata_child_change(parent, Some(name.as_ref()), None)?
            .into_iter()
            .collect();
        let mut plan = MutationPlan::new(MutationKind::DeleteFolder);
        for (path, contents) in writes {
            plan.write_page(path, contents);
        }
        if exists {
            for path in &deleted {
                plan.delete_page(path.clone());
            }
            plan.remove_page_directory(folder.clone());
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }
    pub(super) fn reject_references_into(
        &self,
        targets: &BTreeSet<String>,
        deleted_pages: &BTreeSet<String>,
    ) -> Result<()> {
        let mut links = 0;
        for stored in self.pages.values() {
            if deleted_pages.contains(&stored.page.path) {
                continue;
            }
            links += stored
                .page
                .links
                .iter()
                .filter(|link| {
                    link_target_path(&link.target).is_some_and(|path| targets.contains(path))
                })
                .count();
        }
        if links == 0 {
            return Ok(());
        }
        Err(FractalError::invalid_input(format!(
            "cannot delete while {links} link(s) from surviving pages target the selection"
        )))
    }

    pub(super) fn folder_metadata_child_change(
        &self,
        folder: &Path,
        remove: Option<&str>,
        append: Option<&str>,
    ) -> Result<Option<(PathBuf, String)>> {
        let Some(stored) = self.folders.get(&path_string(folder)) else {
            return Ok(None);
        };
        let Some(mut metadata) = stored.metadata.clone() else {
            return Ok(None);
        };
        let Some(order) = metadata.order.as_mut() else {
            return Ok(None);
        };
        if let Some(remove) = remove {
            order.retain(|name| name != remove);
        }
        if let Some(append) = append {
            if !order.iter().any(|name| name == append) {
                order.push(append.to_owned());
            }
        }
        Ok(Some((
            folder_metadata_relative_path(folder),
            serde_json::to_string_pretty(&metadata)?,
        )))
    }

    pub(super) fn folder_metadata_replace_child(
        &self,
        folder: &Path,
        old: &str,
        new: &str,
    ) -> Result<Option<(PathBuf, String)>> {
        let Some(stored) = self.folders.get(&path_string(folder)) else {
            return Ok(None);
        };
        let Some(mut metadata) = stored.metadata.clone() else {
            return Ok(None);
        };
        let Some(order) = metadata.order.as_mut() else {
            return Ok(None);
        };
        if let Some(name) = order.iter_mut().find(|name| name.as_str() == old) {
            *name = new.to_owned();
        } else if !order.iter().any(|name| name == new) {
            order.push(new.to_owned());
        }
        Ok(Some((
            folder_metadata_relative_path(folder),
            serde_json::to_string_pretty(&metadata)?,
        )))
    }

    /// Applies title-driven path repairs and pending folder-order additions.
    ///
    /// Repair stops at the first failed operation and records the failure in the
    /// report. Changes committed before that failure remain applied.
    pub fn repair(&mut self) -> Result<RepairReport> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let mut report = RepairReport {
            changes: vec![],
            warnings: vec![],
            failures: vec![],
        };

        loop {
            let mismatch = self.folders.values().find_map(|stored| {
                let path = Path::new(&stored.folder.path);
                let parent = path.parent()?;
                let desired = parent.join(slug(&stored.folder.title).ok()?);
                (desired != path).then(|| (path.to_path_buf(), desired))
            });
            let Some((from, to)) = mismatch else { break };
            let receipt = match self.rename_folder(&from, &to, MutationKind::RepairProject, None) {
                Ok(receipt) => receipt,
                Err(error) => {
                    report.failures.push(OperationFailure {
                        code: error.code,
                        message: error.message,
                    });
                    return Ok(report);
                }
            };
            report.changes.extend(receipt.changes);
            report.warnings.extend(receipt.warnings);
        }
        loop {
            let mismatch = self.pages.values().find_map(|stored| {
                let title = stored.page.title.as_deref()?;
                let path = PathBuf::from(&stored.page.path);
                let desired =
                    path.with_file_name(format!("{}{}", slug(title).ok()?, NATIVE_SUFFIX));
                (desired != path).then_some((path, desired))
            });
            let Some((from, to)) = mismatch else { break };
            let receipt = match self.rename_native_with_title(
                &from,
                &to,
                None,
                MutationKind::RepairProject,
            ) {
                Ok(receipt) => receipt,
                Err(error) => {
                    report.failures.push(OperationFailure {
                        code: error.code,
                        message: error.message,
                    });
                    return Ok(report);
                }
            };
            report.changes.extend(receipt.changes);
            report.warnings.extend(receipt.warnings);
        }

        let mut order_plan = MutationPlan::new(MutationKind::RepairProject);
        for stored in self.folders.values() {
            let Some(mut metadata) = stored.metadata.clone() else {
                continue;
            };
            let Some(order) = metadata.order.as_mut() else {
                continue;
            };
            let known: BTreeSet<String> = order.iter().cloned().collect();
            let additions: Vec<String> = stored
                .folder
                .children
                .iter()
                .map(|child| child.name.clone())
                .filter(|name| !known.contains(name))
                .collect();
            if !additions.is_empty() {
                order.extend(additions);
                order_plan.write_page(
                    folder_metadata_relative_path(Path::new(&stored.folder.path)),
                    serde_json::to_string_pretty(&metadata)?,
                );
            }
        }
        let receipt = match order_plan.commit(&self.root) {
            Ok(receipt) => receipt,
            Err(error) => {
                report.failures.push(OperationFailure {
                    code: error.code,
                    message: error.message,
                });
                return Ok(report);
            }
        };
        report.changes.extend(receipt.changes);
        report.warnings.extend(receipt.warnings);
        if let Err(error) = self.reload() {
            report.failures.push(OperationFailure {
                code: error.code,
                message: error.message,
            });
        }
        Ok(report)
    }

    fn rename_folder(
        &mut self,
        from: &Path,
        to: &Path,
        operation: MutationKind,
        metadata_override: Option<(PathBuf, String)>,
    ) -> Result<MutationReceipt> {
        let pages = self.root.join(PAGES);
        if path_exists(&pages.join(to)) {
            return Err(FractalError::already_exists(format!(
                "folder already exists: {}",
                to.display()
            )));
        }
        let mut old_files = Vec::new();
        collect_files(&pages, &pages.join(from), &mut old_files)?;
        if old_files.iter().any(|path| !is_managed_folder_file(path)) {
            return Err(FractalError::invalid_input(
                "folder contains unsupported content that Fractal does not manage",
            ));
        }
        let mut old_directories = Vec::new();
        collect_directories(&pages, &pages.join(from), &mut old_directories)?;
        let new_files: Vec<PathBuf> = old_files
            .iter()
            .map(|path| Ok(to.join(path.strip_prefix(from)?)))
            .collect::<Result<_>>()?;
        let mut new_directories = vec![to.to_path_buf()];
        new_directories.extend(old_directories.iter().map(|path| {
            to.join(
                path.strip_prefix(from)
                    .expect("collected directory is below renamed folder"),
            )
        }));
        let old_prefix = path_string(from);
        let new_prefix = path_string(to);
        let native_targets: BTreeSet<String> = self.pages.keys().cloned().collect();
        let mut rewrites = BTreeMap::new();
        for stored in self.pages.values() {
            let old_source = &stored.page.path;
            let new_source = if Path::new(old_source).starts_with(from) {
                path_string(&to.join(Path::new(old_source).strip_prefix(from)?))
            } else {
                old_source.clone()
            };
            let document = Document::parse(&stored.html);
            let changes = document.rewrite_native_paths(
                old_source,
                &new_source,
                &old_prefix,
                &new_prefix,
                &native_targets,
            );
            if changes > 0 {
                rewrites.insert(
                    PathBuf::from(new_source),
                    document.serialize()?.into_bytes(),
                );
            }
        }
        for (old, new) in old_files.iter().zip(&new_files) {
            if !rewrites.contains_key(new) {
                rewrites.insert(new.clone(), fs::read(pages.join(old))?);
            }
        }
        let from_parent = from.parent().unwrap_or_else(|| Path::new(""));
        let to_parent = to.parent().unwrap_or_else(|| Path::new(""));
        let from_name = from
            .file_name()
            .expect("folder has a name")
            .to_string_lossy();
        let to_name = to.file_name().expect("folder has a name").to_string_lossy();
        let metadata_writes: Vec<_> = if from_parent == to_parent {
            self.folder_metadata_replace_child(from_parent, &from_name, &to_name)?
                .into_iter()
                .collect()
        } else {
            self.folder_metadata_child_change(from_parent, Some(&from_name), None)?
                .into_iter()
                .chain(self.folder_metadata_child_change(to_parent, None, Some(&to_name))?)
                .collect()
        };
        rewrites.extend(
            metadata_writes
                .into_iter()
                .map(|(path, contents)| (path, contents.into_bytes())),
        );
        if let Some((path, contents)) = metadata_override {
            rewrites.insert(path, contents.into_bytes());
        }
        let mut plan = MutationPlan::new(operation);
        for (path, contents) in rewrites {
            plan.write_page(path, contents);
        }
        for (old, new) in old_files.iter().zip(&new_files) {
            plan.delete_page(old.clone());
            plan.move_page(old.clone(), new.clone());
        }
        for directory in new_directories {
            plan.create_page_directory(directory);
        }
        plan.remove_page_directory(from.to_path_buf());
        plan.move_page_directory(from.to_path_buf(), to.to_path_buf());
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }

    pub(super) fn reload_folders(&mut self) -> Result<()> {
        let pages_root = self.root.join(PAGES);
        let mut paths = Vec::new();
        collect_directories(&pages_root, &pages_root, &mut paths)?;
        paths.insert(0, PathBuf::new());
        let mut folders = BTreeMap::new();
        for relative in paths {
            let absolute = pages_root.join(&relative);
            let metadata_path = absolute.join(MANIFEST);
            let metadata: Option<FolderMetadata> = if metadata_path.is_file() {
                Some(
                    serde_json::from_str(&fs::read_to_string(&metadata_path)?).map_err(
                        |error| {
                            FractalError::invalid_project(format!(
                                "invalid folder metadata at {}: {error}",
                                metadata_path.display()
                            ))
                        },
                    )?,
                )
            } else {
                None
            };
            if metadata
                .as_ref()
                .is_some_and(|metadata| metadata.title.trim().is_empty())
            {
                return Err(FractalError::invalid_project(format!(
                    "folder title is empty: {}",
                    display_folder_path(&relative)
                )));
            }
            let present = direct_orderable_children(&absolute)?;
            if let Some(stored) = metadata.as_ref() {
                if let Some(order) = stored.order.as_ref() {
                    validate_stored_order(&relative, order, &present)?;
                }
            }
            let title = metadata
                .as_ref()
                .map(|metadata| metadata.title.clone())
                .unwrap_or_else(|| default_folder_title(&self.manifest.name, &relative));
            let order = metadata
                .as_ref()
                .and_then(|metadata| metadata.order.clone());
            let mut names = order.clone().unwrap_or_else(|| {
                present
                    .iter()
                    .filter(|(_, kind)| **kind == FolderChildKind::Folder)
                    .chain(
                        present
                            .iter()
                            .filter(|(_, kind)| **kind == FolderChildKind::Native),
                    )
                    .map(|(name, _)| name.clone())
                    .collect()
            });
            if order.is_some() {
                let known: BTreeSet<String> = names.iter().cloned().collect();
                names.extend(
                    present
                        .keys()
                        .filter(|name| !known.contains(*name))
                        .cloned(),
                );
            }
            let mut issues = Vec::new();
            let children = names
                .into_iter()
                .map(|name| {
                    let (kind, status) = match present.get(&name) {
                        Some(kind) => (*kind, FolderChildStatus::Present),
                        None => {
                            issues.push(FolderIssue {
                                name: name.clone(),
                                message: "ordered child is missing".into(),
                            });
                            (ordered_name_kind(&name), FolderChildStatus::Missing)
                        }
                    };
                    FolderChild { name, kind, status }
                })
                .collect();
            let key = path_string(&relative);
            folders.insert(
                key.clone(),
                StoredFolder {
                    folder: Folder {
                        path: key,
                        title,
                        order,
                        children,
                        issues,
                    },
                    metadata,
                },
            );
        }
        self.folders = folders;
        Ok(())
    }
}
