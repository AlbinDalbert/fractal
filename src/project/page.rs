use super::support::*;
use super::*;

impl NativePageDraft {
    /// Parses complete native source into the sections needed for guarded
    /// recreation. This does not permit whole-source replacement of a live
    /// native document.
    pub fn from_source(source: &str) -> Result<Self> {
        let document = Document::parse(source);
        let issues = native_document_issues(&document);
        if let Some(issue) = issues.first() {
            return Err(FractalError::invalid_input(format!(
                "invalid recovered native document: {issue}"
            )));
        }
        Ok(Self {
            title: document.title().unwrap_or_default(),
            content_html: document.content_html()?,
            style_css: document.managed_style_css()?,
            metadata_html: document.user_metadata_html()?,
        })
    }
}

impl Project {
    /// Returns cataloged metadata for a native document.
    pub fn page(&self, path: impl AsRef<Path>) -> Result<Page> {
        Ok(self.stored(path.as_ref())?.page.clone())
    }

    /// Returns the exact HTML source stored for a native document.
    pub fn source(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.html.clone())
    }

    /// Extracts the editor-owned sections of a native document and hashes each
    /// section for guarded mutation.
    pub fn native_document_parts(&self, path: impl AsRef<Path>) -> Result<NativeDocumentParts> {
        let stored = self.stored(path.as_ref())?;
        let document = Document::parse(&stored.html);
        let title = document.title().unwrap_or_default();
        let content_html = document.content_html()?;
        let style_css = document.managed_style_css()?;
        let metadata_html = document.user_metadata_html()?;
        Ok(NativeDocumentParts {
            title_hash: content_hash(&title),
            title,
            content_hash: content_hash(&content_html),
            content_html,
            style_hash: content_hash(&style_css),
            style_css,
            metadata_hash: content_hash(&metadata_html),
            metadata_html,
            source_hash: stored.page.content_hash.clone(),
        })
    }

    /// Returns the SHA-256 hash of a native document's exact UTF-8 source bytes.
    pub fn content_hash(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.page.content_hash.clone())
    }

    /// Changes a native document title and derives its filename from that title.
    ///
    /// The source update, path change, explicit-link rewrites, and folder order
    /// update commit as one recoverable transaction.
    pub fn set_page_title(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
    ) -> Result<MutationReceipt> {
        self.set_page_title_inner(path.as_ref(), title, None)
    }

    /// Changes a native document title if the title has not changed since it
    /// was read.
    ///
    /// `expected_hash` is the `title_hash` returned by
    /// [`Project::native_document_parts`].
    pub fn set_page_title_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.set_page_title_inner(path.as_ref(), title, Some(expected_hash))
    }

    fn set_page_title_inner(
        &mut self,
        path: &Path,
        title: &str,
        expected_hash: Option<&str>,
    ) -> Result<MutationReceipt> {
        let title = title.trim();
        let stem = slug(title)?;
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let from = self.existing_path(path)?;
        if let Some(expected_hash) = expected_hash {
            let actual_title = self.stored(&from)?.page.title.as_deref().unwrap_or("");
            let actual_hash = content_hash(actual_title);
            if actual_hash != expected_hash {
                return Err(FractalError::conflict(format!(
                    "title changed since it was read (expected {expected_hash}, found {actual_hash})"
                )));
            }
        }
        let to = from.with_file_name(format!("{stem}{NATIVE_SUFFIX}"));
        self.rename_native_with_title(&from, &to, Some(title), MutationKind::SetPageTitle)
    }

    /// Creates a native page in the pages root using a path derived from
    /// `title`.
    pub fn create_page(&mut self, title: &str) -> Result<MutationReceipt> {
        let stem = slug(title)?;
        self.create_page_at(format!("{stem}{NATIVE_SUFFIX}"), title)
    }

    /// Creates a native page at an explicit title-derived project path.
    ///
    /// The filename must match the slug Fractal derives from `title`. Parent
    /// folders are created as needed and stored folder order is updated.
    pub fn create_page_at(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
    ) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        if title.trim().is_empty() {
            return Err(FractalError::invalid_input("title cannot be empty"));
        }
        let relative = normalize_native_page_path(path.as_ref())?;
        validate_title_driven_page_path(&relative, title)?;
        if path_exists(&self.root.join(PAGES).join(&relative)) {
            return Err(FractalError::already_exists(format!(
                "page already exists: {}",
                relative.display()
            )));
        }
        let title = title.trim();
        let html = format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <meta name=\"fractal-format\" content=\"1\">\n  <title>{}</title>\n  <style>\n    :root {{ color-scheme: dark; }}\n    * {{ box-sizing: border-box; }}\n    body {{\n      margin: 0;\n      background: #0c0c0a;\n      color: #e8e1d5;\n      font: 1.125rem/1.65 ui-sans-serif, system-ui, sans-serif;\n    }}\n    main {{\n      width: min(100% - 2rem, 45rem);\n      margin: 0 auto;\n      padding: clamp(4rem, 12vh, 8rem) 0;\n    }}\n    h1 {{\n      margin: 0 0 2.5rem;\n      font-size: clamp(2.75rem, 8vw, 4rem);\n      line-height: 1;\n      letter-spacing: -0.04em;\n    }}\n    h2, h3, h4, h5, h6 {{ line-height: 1.2; }}\n    p, ul, ol, blockquote, pre, figure, table {{ margin: 1.25rem 0; }}\n    a {{ color: #e8bb4d; text-underline-offset: 0.18em; }}\n    code, pre {{ font-family: ui-monospace, monospace; }}\n  </style>\n</head>\n<body>\n  <main data-fractal-document>\n    <h1>{}</h1>\n  </main>\n</body>\n</html>\n",
            escape_html(title),
            escape_html(title)
        )
        .replace("<style>", "<style data-fractal-style>")
        .replace("<h1>", "<h1 data-fractal-title>");
        let mut plan = MutationPlan::new(MutationKind::CreatePage);
        plan.ensure_page_parent_directories(&self.root, &relative);
        plan.write_page(relative.clone(), html);
        if let Some(write) = self.folder_metadata_child_change(
            relative.parent().unwrap_or_else(|| Path::new("")),
            None,
            Some(
                relative
                    .file_name()
                    .expect("page has a name")
                    .to_string_lossy()
                    .as_ref(),
            ),
        )? {
            plan.write_page(write.0, write.1);
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }

    /// Recreates a missing native page from editor-owned recovery data.
    ///
    /// The operation checks absence while holding the project lock and never
    /// overwrites a path that has reappeared.
    pub fn recreate_page_from_draft(
        &mut self,
        path: impl AsRef<Path>,
        draft: &NativePageDraft,
    ) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let relative = normalize_native_page_path(path.as_ref())?;
        validate_title_driven_page_path(&relative, &draft.title)?;
        if path_exists(&self.root.join(PAGES).join(&relative)) {
            return Err(FractalError::conflict(format!(
                "cannot recreate a page that now exists: {}",
                relative.display()
            )));
        }

        let document = Document::parse(&format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"fractal-format\" content=\"1\"><title>{}</title><style data-fractal-style></style></head><body><main data-fractal-document><h1 data-fractal-title>{}</h1></main></body></html>",
            escape_html(draft.title.trim()),
            escape_html(draft.title.trim())
        ));
        document.set_content_html(&draft.content_html)?;
        document.set_managed_style_css(&draft.style_css)?;
        document.set_user_metadata_html(&draft.metadata_html)?;
        let issues = native_document_issues(&document);
        if let Some(issue) = issues.first() {
            return Err(FractalError::invalid_input(format!(
                "invalid recovered native document: {issue}"
            )));
        }

        let mut plan = MutationPlan::new(MutationKind::RecreatePage);
        plan.ensure_page_parent_directories(&self.root, &relative);
        plan.write_page(relative.clone(), document.serialize()?);
        if let Some(write) = self.folder_metadata_child_change(
            relative.parent().unwrap_or_else(|| Path::new("")),
            None,
            relative.file_name().and_then(|name| name.to_str()),
        )? {
            plan.write_page(write.0, write.1);
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }

    /// Recreates an absent native page from a complete recovery source.
    pub fn recreate_page_from_source(
        &mut self,
        path: impl AsRef<Path>,
        source: &str,
    ) -> Result<MutationReceipt> {
        let draft = NativePageDraft::from_source(source)?;
        self.recreate_page_from_draft(path, &draft)
    }

    /// Replaces a native document's content section if it has not changed since
    /// it was read.
    ///
    /// `expected_hash` is the `content_hash` returned by
    /// [`Project::native_document_parts`].
    pub fn set_page_content(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.mutate_native_section(
            path.as_ref(),
            expected_hash,
            "content",
            MutationKind::SetPageContent,
            |document| document.set_content_html(html),
        )
    }

    /// Replaces a native document's managed CSS if it has not changed since it
    /// was read.
    ///
    /// `expected_hash` is the `style_hash` returned by
    /// [`Project::native_document_parts`].
    pub fn set_page_style(
        &mut self,
        path: impl AsRef<Path>,
        css: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.mutate_native_section(
            path.as_ref(),
            expected_hash,
            "style",
            MutationKind::SetPageStyle,
            |document| document.set_managed_style_css(css),
        )
    }

    /// Restores Fractal's default managed CSS if the current CSS matches
    /// `expected_hash`.
    pub fn restore_default_page_style(
        &mut self,
        path: impl AsRef<Path>,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.set_page_style(path, DEFAULT_STYLE, expected_hash)
    }

    /// Replaces a native document's user-owned head metadata if it has not
    /// changed since it was read.
    ///
    /// `expected_hash` is the `metadata_hash` returned by
    /// [`Project::native_document_parts`].
    pub fn set_page_metadata(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.mutate_native_section(
            path.as_ref(),
            expected_hash,
            "metadata",
            MutationKind::SetPageMetadata,
            |document| document.set_user_metadata_html(html),
        )
    }

    /// Restores required native document elements and the default style where
    /// managed structure is missing.
    pub fn repair_page_structure(&mut self, path: impl AsRef<Path>) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let relative = self.existing_path(path.as_ref())?;
        let document = Document::parse(&self.stored(&relative)?.html);
        document.repair_managed_structure(DEFAULT_STYLE)?;
        self.commit_native_document(relative, document, MutationKind::RepairPageStructure)
    }

    fn mutate_native_section<F>(
        &mut self,
        path: &Path,
        expected_hash: &str,
        section: &str,
        operation: MutationKind,
        mutate: F,
    ) -> Result<MutationReceipt>
    where
        F: FnOnce(&Document) -> Result<()>,
    {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let relative = self.existing_path(path)?;
        let document = Document::parse(&self.stored(&relative)?.html);
        let actual_hash = match section {
            "content" => content_hash(&document.content_html()?),
            "style" => content_hash(&document.managed_style_css()?),
            "metadata" => content_hash(&document.user_metadata_html()?),
            _ => unreachable!(),
        };
        if actual_hash != expected_hash {
            return Err(FractalError::conflict(format!(
                "{section} changed since it was read (expected {expected_hash}, found {actual_hash})"
            )));
        }
        mutate(&document)?;
        self.commit_native_document(relative, document, operation)
    }

    fn commit_native_document(
        &mut self,
        relative: PathBuf,
        document: Document,
        operation: MutationKind,
    ) -> Result<MutationReceipt> {
        let issues = native_document_issues(&document);
        if let Some(issue) = issues.first() {
            return Err(FractalError::invalid_input(format!(
                "invalid native document: {issue}"
            )));
        }
        let mut plan = MutationPlan::new(operation);
        plan.write_page(relative, document.serialize()?);
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }
    /// Moves a page and rewrites affected internal references atomically.
    ///
    /// Moving a native page also updates stored folder order. A move to the
    /// current path returns a no-op receipt.
    pub fn move_page(
        &mut self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let from = self.existing_path(from.as_ref())?;
        let to = normalize_native_page_path(to.as_ref())?;
        if from == to {
            return Ok(noop_receipt(MutationKind::MovePage));
        }
        let destination = self.root.join(PAGES).join(&to);
        if path_exists(&destination) {
            return Err(FractalError::already_exists(format!(
                "page already exists: {}",
                to.display()
            )));
        }

        let from_string = path_string(&from);
        let to_string = path_string(&to);
        let native_targets = self.pages.keys().cloned().collect();
        let moved_document = Document::parse(&self.stored(&from)?.html);
        moved_document.rewrite_native_source_location(&from_string, &to_string, &native_targets);
        let moved_html = moved_document.serialize()?;

        let mut rewrites = Vec::new();
        for (path, stored) in &self.pages {
            if path == &from_string {
                continue;
            }
            let document = Document::parse(&stored.html);
            if document.rewrite_internal_target(path, &from_string, &to_string) > 0 {
                rewrites.push((path.clone(), document.serialize()?));
            }
        }

        let mut plan = MutationPlan::new(MutationKind::MovePage);
        plan.ensure_page_parent_directories(&self.root, &to);
        plan.write_page(to.clone(), moved_html);
        plan.delete_page(from.clone());
        plan.move_page(from.clone(), to.clone());
        for (path, html) in rewrites {
            plan.write_page(PathBuf::from(path), html);
        }
        let from_parent = from.parent().unwrap_or_else(|| Path::new(""));
        let to_parent = to.parent().unwrap_or_else(|| Path::new(""));
        let from_name = from.file_name().expect("page has a name").to_string_lossy();
        let to_name = to.file_name().expect("page has a name").to_string_lossy();
        if from_parent == to_parent {
            if let Some(write) = self.folder_metadata_replace_child(
                from_parent,
                from_name.as_ref(),
                to_name.as_ref(),
            )? {
                plan.write_page(write.0, write.1);
            }
        } else {
            if let Some(write) =
                self.folder_metadata_child_change(from_parent, Some(from_name.as_ref()), None)?
            {
                plan.write_page(write.0, write.1);
            }
            if let Some(write) =
                self.folder_metadata_child_change(to_parent, None, Some(to_name.as_ref()))?
            {
                plan.write_page(write.0, write.1);
            }
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }
    pub(super) fn rename_native_with_title(
        &mut self,
        from: &Path,
        to: &Path,
        title: Option<&str>,
        operation: MutationKind,
    ) -> Result<MutationReceipt> {
        if from != to && path_exists(&self.root.join(PAGES).join(to)) {
            return Err(FractalError::already_exists(format!(
                "page already exists: {}",
                to.display()
            )));
        }
        let from_string = path_string(from);
        let to_string = path_string(to);
        let native_targets = self.pages.keys().cloned().collect();
        let moved_document = Document::parse(&self.stored(from)?.html);
        if let Some(title) = title {
            moved_document.set_title(title);
        }
        if from != to {
            moved_document.rewrite_native_source_location(
                &from_string,
                &to_string,
                &native_targets,
            );
        }
        let mut plan = MutationPlan::new(operation);
        plan.write_page(to.to_path_buf(), moved_document.serialize()?);
        if from != to {
            plan.delete_page(from.to_path_buf());
            plan.move_page(from.to_path_buf(), to.to_path_buf());
            for (path, stored) in &self.pages {
                if path == &from_string {
                    continue;
                }
                let document = Document::parse(&stored.html);
                if document.rewrite_internal_target(path, &from_string, &to_string) > 0 {
                    plan.write_page(PathBuf::from(path), document.serialize()?);
                }
            }
            let parent = from.parent().unwrap_or_else(|| Path::new(""));
            let old = from.file_name().expect("page has a name").to_string_lossy();
            let new = to.file_name().expect("page has a name").to_string_lossy();
            if let Some(write) = self.folder_metadata_replace_child(parent, &old, &new)? {
                plan.write_page(write.0, write.1);
            }
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }
    /// Deletes one page if no surviving page links to it.
    pub fn delete_page(&mut self, path: impl AsRef<Path>) -> Result<MutationReceipt> {
        self.delete_pages([path])
    }

    /// Deletes a set of pages as one locked project operation.
    ///
    /// References between pages in the set do not block deletion. References
    /// from pages that survive do block it.
    /// Deletes several pages in one transaction.
    ///
    /// References between pages in the deletion set are allowed. A link from a
    /// surviving page rejects the whole operation.
    pub fn delete_pages<I, P>(&mut self, paths: I) -> Result<MutationReceipt>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let requested: Vec<PathBuf> = paths
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .collect();
        if requested.is_empty() {
            return Err(FractalError::invalid_input(
                "page deletion needs at least one path",
            ));
        }
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let mut relative = BTreeSet::new();
        for path in requested {
            relative.insert(self.existing_or_ghost_native_path(&path)?);
        }
        let targets: BTreeSet<String> = relative.iter().map(|path| path_string(path)).collect();
        self.reject_references_into(&targets, &targets)?;
        let mut metadata = BTreeMap::<PathBuf, FolderMetadata>::new();
        for path in &relative {
            let parent = path.parent().unwrap_or_else(|| Path::new(""));
            let key = parent.to_path_buf();
            if !metadata.contains_key(&key) {
                if let Some(value) = self
                    .folders
                    .get(&path_string(parent))
                    .and_then(|stored| stored.metadata.clone())
                    .filter(|value| value.order.is_some())
                {
                    metadata.insert(key.clone(), value);
                }
            }
            if let Some(order) = metadata
                .get_mut(&key)
                .and_then(|value| value.order.as_mut())
            {
                let name = path.file_name().expect("page has a name").to_string_lossy();
                order.retain(|entry| entry != name.as_ref());
            }
        }
        let writes = metadata
            .into_iter()
            .map(|(folder, metadata)| {
                Ok((
                    folder_metadata_relative_path(&folder),
                    serde_json::to_string_pretty(&metadata)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let physical_deletes: Vec<_> = relative
            .iter()
            .filter(|path| path_exists(&self.root.join(PAGES).join(path)))
            .cloned()
            .collect();
        let mut plan = MutationPlan::new(MutationKind::DeletePages);
        for (path, contents) in writes {
            plan.write_page(path, contents);
        }
        for path in physical_deletes {
            plan.delete_page(path);
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }
}

fn validate_title_driven_page_path(path: &Path, title: &str) -> Result<()> {
    let expected = format!("{}{}", slug(title)?, NATIVE_SUFFIX);
    if path.file_name().and_then(|name| name.to_str()) != Some(expected.as_str()) {
        return Err(FractalError::invalid_input(format!(
            "native page path must end in `{expected}` to match its title"
        )));
    }
    Ok(())
}
