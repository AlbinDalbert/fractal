use super::support::*;
use super::*;

impl Project {
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

    pub fn set_folder_title(&mut self, path: impl AsRef<Path>, title: &str) -> Result<Mutation> {
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
        let resulting_metadata_path = if path.as_os_str().is_empty() {
            folder_metadata_relative_path(&path)
        } else {
            folder_metadata_relative_path(
                &path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(slug(title)?),
            )
        };
        self.upgrade_contract_for_folder_metadata()?;
        let metadata_path = folder_metadata_relative_path(&path);
        commit_file_transaction(
            &self.root,
            vec![(
                metadata_path.clone(),
                serde_json::to_string_pretty(&metadata)?,
            )],
            vec![],
        )?;
        self.reload()?;
        self.repair_title_paths()?;
        Ok(Mutation {
            changed: vec![resulting_metadata_path],
            deleted: vec![],
        })
    }

    pub fn reorder_folder<I, S>(&mut self, path: impl AsRef<Path>, order: I) -> Result<Mutation>
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
        self.upgrade_contract_for_folder_metadata()?;
        let metadata_path = folder_metadata_relative_path(&path);
        commit_file_transaction(
            &self.root,
            vec![(
                metadata_path.clone(),
                serde_json::to_string_pretty(&metadata)?,
            )],
            vec![],
        )?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![metadata_path],
            deleted: vec![],
        })
    }

    pub fn move_folder(
        &mut self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> Result<Mutation> {
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
            return Ok(Mutation {
                changed: vec![],
                deleted: vec![],
            });
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
        self.rename_folder(&from, &to)
    }
    /// Deletes a folder below `pages/` with a single namespace rename.
    ///
    /// The returned `deleted` list includes every file that was below the
    /// folder, including non-HTML assets.
    pub fn delete_folder(&mut self, path: impl AsRef<Path>) -> Result<Mutation> {
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
        let changed = writes.iter().map(|(path, _)| path.clone()).collect();
        let deletes = exists.then_some(folder.clone()).into_iter().collect();
        commit_file_transaction(&self.root, writes, deletes)?;
        self.reload()?;
        Ok(Mutation { changed, deleted })
    }
    fn upgrade_contract_for_folder_metadata(&mut self) -> Result<()> {
        if self.manifest.version >= 2 {
            return Ok(());
        }
        let pages_root = self.root.join(PAGES);
        let mut folders = Vec::new();
        collect_directories(&pages_root, &pages_root, &mut folders)?;
        folders.insert(0, PathBuf::new());
        if let Some(conflict) = folders
            .into_iter()
            .map(|folder| pages_root.join(folder).join(MANIFEST))
            .find(|path| path_exists(path))
        {
            return Err(FractalError::conflict(format!(
                "cannot upgrade to project format version 2 because the reserved folder metadata path already exists: {}",
                conflict.display()
            )));
        }
        self.manifest.version = VERSION;
        atomic_write(
            &self.root.join(MANIFEST),
            &serde_json::to_string_pretty(&self.manifest)?,
        )
    }

    pub(super) fn reject_references_into(
        &self,
        targets: &BTreeSet<String>,
        deleted_pages: &BTreeSet<String>,
    ) -> Result<()> {
        let mut links = 0;
        let mut iframes = 0;
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
            iframes += stored
                .page
                .iframes
                .iter()
                .filter(|iframe| {
                    iframe_target_path(&iframe.target).is_some_and(|path| targets.contains(path))
                })
                .count();
        }
        if links == 0 && iframes == 0 {
            return Ok(());
        }
        Err(FractalError::invalid_input(format!(
            "cannot delete while {links} link(s) and {iframes} iframe(s) from surviving pages target the selection"
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

    pub(super) fn repair_title_paths(&mut self) -> Result<()> {
        loop {
            let mismatch = self.folders.values().find_map(|stored| {
                let path = Path::new(&stored.folder.path);
                let parent = path.parent()?;
                let desired = parent.join(slug(&stored.folder.title).ok()?);
                (desired != path).then(|| (path.to_path_buf(), desired))
            });
            let Some((from, to)) = mismatch else { break };
            self.rename_folder(&from, &to)?;
        }
        loop {
            let mismatch = self.pages.values().find_map(|stored| {
                if stored.page.kind != PageKind::Native {
                    return None;
                }
                let title = stored.page.title.as_deref()?;
                let path = PathBuf::from(&stored.page.path);
                let desired =
                    path.with_file_name(format!("{}{}", slug(title).ok()?, NATIVE_SUFFIX));
                (desired != path).then_some((path, desired))
            });
            let Some((from, to)) = mismatch else { break };
            self.rename_native_with_title(&from, &to, None)?;
        }
        Ok(())
    }

    fn rename_folder(&mut self, from: &Path, to: &Path) -> Result<Mutation> {
        let pages = self.root.join(PAGES);
        if path_exists(&pages.join(to)) {
            return Err(FractalError::already_exists(format!(
                "folder already exists: {}",
                to.display()
            )));
        }
        let mut old_files = Vec::new();
        collect_files(&pages, &pages.join(from), &mut old_files)?;
        let new_files: Vec<PathBuf> = old_files
            .iter()
            .map(|path| Ok(to.join(path.strip_prefix(from)?)))
            .collect::<Result<_>>()?;
        let old_prefix = path_string(from);
        let new_prefix = path_string(to);
        let mut rewrites = Vec::new();
        for stored in self.pages.values() {
            if stored.page.kind != PageKind::Native {
                continue;
            }
            let old_source = &stored.page.path;
            let new_source = if Path::new(old_source).starts_with(from) {
                path_string(&to.join(Path::new(old_source).strip_prefix(from)?))
            } else {
                old_source.clone()
            };
            let document = Document::parse(&stored.html);
            let mut changes = 0;
            if old_source != &new_source {
                changes += document.rewrite_source_location(old_source, &new_source);
            }
            changes += document.rewrite_internal_prefix(
                &new_source,
                &new_source,
                &old_prefix,
                &new_prefix,
            );
            if changes > 0 {
                rewrites.push((new_source, document.serialize()?));
            }
        }
        fs::rename(pages.join(from), pages.join(to))?;
        let result = (|| -> Result<()> {
            for (path, html) in rewrites {
                atomic_write(&pages.join(path), &html)?;
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
            for (path, html) in metadata_writes {
                atomic_write(&pages.join(path), &html)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::rename(pages.join(to), pages.join(from));
            return Err(error);
        }
        self.reload()?;
        Ok(Mutation {
            changed: new_files,
            deleted: old_files,
        })
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
            let mut metadata: Option<FolderMetadata> =
                if self.manifest.version >= 2 && metadata_path.is_file() {
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
            if let Some(stored) = metadata.as_mut() {
                if let Some(order) = stored.order.as_mut() {
                    validate_stored_order(&relative, order, &present)?;
                    let known: BTreeSet<&str> = order.iter().map(String::as_str).collect();
                    let additions: Vec<String> = present
                        .keys()
                        .filter(|name| !known.contains(name.as_str()))
                        .cloned()
                        .collect();
                    if !additions.is_empty() {
                        order.extend(additions);
                        atomic_write(&metadata_path, &serde_json::to_string_pretty(stored)?)?;
                    }
                }
            }
            let title = metadata
                .as_ref()
                .map(|metadata| metadata.title.clone())
                .unwrap_or_else(|| default_folder_title(&self.manifest.name, &relative));
            let order = metadata
                .as_ref()
                .and_then(|metadata| metadata.order.clone());
            let names = order.clone().unwrap_or_else(|| {
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
