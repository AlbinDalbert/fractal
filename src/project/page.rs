use super::support::*;
use super::*;

impl NativePageDraft {
    /// Parses a complete native document and extracts the sections required to recreate it.
    ///
    /// # Errors
    ///
    /// Returns an error when the source does not contain a valid native-document structure
    /// or when a required section cannot be extracted.
    ///
    /// # Examples
    ///
    /// ```
    /// # let source = include_str!("fixtures/native-page.html");
    /// let draft = NativePageDraft::from_source(source)?;
    /// assert!(!draft.title.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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
            head_links_html: document.head_links_html()?,
        })
    }
}

impl Project {
    /// Retrieves the stored page for a path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn example(project: &Project) -> Result<()> {
    /// let page = project.page("docs/index.html")?;
    /// # let _ = page;
    /// # Ok(())
    /// # }
    /// ```
    pub fn page(&self, path: impl AsRef<Path>) -> Result<Page> {
        Ok(self.stored(path.as_ref())?.page.clone())
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.html.clone())
    }

    pub fn native_document_parts(&self, path: impl AsRef<Path>) -> Result<NativeDocumentParts> {
        let stored = self.stored(path.as_ref())?;
        if stored.page.kind != PageKind::Native {
            return Err(FractalError::invalid_input(
                "document parts are only available for native documents",
            ));
        }
        let document = Document::parse(&stored.html);
        let title = document.title().unwrap_or_default();
        let content_html = document.content_html()?;
        let style_css = document.managed_style_css()?;
        let metadata_html = document.user_metadata_html()?;
        let head_links_html = document.head_links_html()?;
        Ok(NativeDocumentParts {
            title_hash: content_hash(&title),
            title,
            content_hash: content_hash(&content_html),
            content_html,
            style_hash: content_hash(&style_css),
            style_css,
            metadata_hash: content_hash(&metadata_html),
            metadata_html,
            head_links_hash: content_hash(&head_links_html),
            head_links_html,
            source_hash: stored.page.content_hash.clone(),
        })
    }

    pub fn content_hash(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.page.content_hash.clone())
    }

    /// Renames a native page using a title-derived path and updates its document title.
    
    ///
    
    /// References to the page and folder ordering are updated as part of the same mutation.
    
    ///
    
    /// # Examples
    
    ///
    
    /// ```rust,ignore
    
    /// let receipt = project.set_page_title("docs/old-page.html", "New Page")?;
    
    /// # Ok::<(), YourErrorType>(())
    
    /// ```
    pub fn set_page_title(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
    ) -> Result<MutationReceipt> {
        self.set_page_title_inner(path.as_ref(), title, None)
    }

    /// Sets a page title when its current source matches the expected hash.
    ///
    /// The title is normalized and used to derive the page path. Returns an error if
    /// the page has changed since the expected hash was recorded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let receipt = project.set_page_title_if_unchanged(
    ///     "docs/old-title.html",
    ///     "New Title",
    ///     expected_hash,
    /// )?;
    /// # Ok::<(), YourError>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the native page to rename.
    /// * `title` - New page title.
    /// * `expected_hash` - Source hash that must match the current page.
    ///
    /// # Returns
    ///
    /// A receipt describing the committed mutation.
    pub fn set_page_title_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.set_page_title_inner(path.as_ref(), title, Some(expected_hash))
    }

    /// Renames a native page and updates its document title.
    ///
    /// The destination filename is derived from the trimmed, slugified title. When
    /// provided, `expected_hash` must match the current title hash.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the native page to rename.
    /// * `title` - New title for the page.
    /// * `expected_hash` - Optional hash of the title previously read by the caller.
    ///
    /// # Returns
    ///
    /// A receipt describing the committed mutation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let receipt = project.set_page_title_inner(
    ///     Path::new("guide/page.html"),
    ///     "Updated Page",
    ///     None,
    /// )?;
    /// ```
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
        if page_kind(&from) != PageKind::Native {
            return Err(FractalError::invalid_input(
                "titles can only be changed on native documents",
            ));
        }
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

    /// Creates a native page at a title-derived path.
    ///
    /// The title is converted to a slug and combined with the native page suffix.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let receipt = project.create_page("Getting Started")?;
    /// # let _ = receipt;
    /// # Ok::<(), Error>(())
    /// ```
    pub fn create_page(&mut self, title: &str) -> Result<MutationReceipt> {
        let stem = slug(title)?;
        self.create_page_at(format!("{stem}{NATIVE_SUFFIX}"), title)
    }

    /// Creates a native page at a title-derived path.
    ///
    /// The path must end with the native filename generated from `title`, and the
    /// destination must not already exist. The page is initialized with the
    /// default document structure and style, and its containing folder metadata is
    /// updated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let mut project: Project = todo!();
    /// let receipt = project.create_page_at(
    ///     "guides/getting-started.html",
    ///     "Getting Started",
    /// )?;
    /// # let _ = receipt;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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
            "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <meta name=\"fractal-format\" content=\"1\">\n  <title>{}</title>\n  <style>\n    :root {{ color-scheme: dark; }}\n    * {{ box-sizing: border-box; }}\n    body {{\n      margin: 0;\n      background: #0c0c0a;\n      color: #e8e1d5;\n      font: 1.125rem/1.65 ui-sans-serif, system-ui, sans-serif;\n    }}\n    main {{\n      width: min(100% - 2rem, 45rem);\n      margin: 0 auto;\n      padding: clamp(4rem, 12vh, 8rem) 0;\n    }}\n    h1 {{\n      margin: 0 0 2.5rem;\n      font-size: clamp(2.75rem, 8vw, 4rem);\n      line-height: 1;\n      letter-spacing: -0.04em;\n    }}\n    h2, h3, h4, h5, h6 {{ line-height: 1.2; }}\n    p, ul, ol, blockquote, pre, figure, table {{ margin: 1.25rem 0; }}\n    a {{ color: #e8bb4d; text-underline-offset: 0.18em; }}\n    img, iframe {{ max-width: 100%; }}\n    code, pre {{ font-family: ui-monospace, monospace; }}\n  </style>\n</head>\n<body>\n  <main data-fractal-document>\n    <h1>{}</h1>\n  </main>\n</body>\n</html>\n",
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
    /// The path must match the title-derived native filename. The operation rejects
    /// paths that have reappeared and validates the reconstructed document before
    /// committing it.
    ///
    /// # Errors
    ///
    /// Returns an error if the path or recovered document is invalid, or if a page
    /// already exists at the path.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn example(
    /// #     project: &mut Project,
    /// #     draft: &NativePageDraft,
    /// # ) -> Result<MutationReceipt> {
    /// project.recreate_page_from_draft("guide.html", draft)?;
    /// # Ok(())
    /// # }
    /// ```
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
        document.set_head_links_html(&draft.head_links_html)?;
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

    /// Recreates an absent native page from complete native HTML source.
    ///
    /// The source must contain a valid native document with the required managed
    /// sections.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let receipt = project.recreate_page_from_source("docs/page.html", source)?;
    /// # Ok::<(), project::Error>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// * `path` - Path at which to recreate the page.
    /// * `source` - Complete native HTML source for the page.
    ///
    /// # Returns
    ///
    /// A receipt describing the committed page recreation.
    pub fn recreate_page_from_source(
        &mut self,
        path: impl AsRef<Path>,
        source: &str,
    ) -> Result<MutationReceipt> {
        let draft = NativePageDraft::from_source(source)?;
        self.recreate_page_from_draft(path, &draft)
    }

    /// Replaces the raw HTML page at the specified path.
    
    ///
    
    /// # Examples
    
    ///
    
    /// ```rust,ignore
    
    /// let receipt = project.write_raw_page("about.html", "<h1>About</h1>")?;
    
    /// ```
    pub fn write_raw_page(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
    ) -> Result<MutationReceipt> {
        self.write_raw_page_inner(path.as_ref(), html, None)
    }

    /// Replaces a raw page when its current source matches the expected hash.
    ///
    /// # Parameters
    ///
    /// - `path`: Path of the page to replace.
    /// - `html`: Replacement HTML source.
    /// - `expected_hash`: Source hash expected for the current page.
    ///
    /// # Errors
    ///
    /// Returns an error if the page has changed since `expected_hash` was obtained
    /// or if the replacement cannot be committed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let receipt = project.write_raw_page_if_unchanged(
    ///     "about.html",
    ///     "<h1>About</h1>",
    ///     expected_hash,
    /// )?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn write_raw_page_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.write_raw_page_inner(path.as_ref(), html, Some(expected_hash))
    }

    /// Replaces a raw HTML page, optionally requiring its content hash to match.
    ///
    /// # Errors
    ///
    /// Returns an error if the page does not exist, the expected hash does not match,
    /// or the page is a native document.
    ///
    /// # Examples
    ///
    /// ```
    /// let receipt = project.write_raw_page("about.html", "<h1>About</h1>")?;
    /// # Ok::<(), fractal::FractalError>(())
    /// ```
    fn write_raw_page_inner(
        &mut self,
        path: &Path,
        html: &str,
        expected_hash: Option<&str>,
    ) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let relative = self.existing_path(path.as_ref())?;
        if let Some(expected_hash) = expected_hash {
            let actual_hash = &self.stored(&relative)?.page.content_hash;
            if actual_hash != expected_hash {
                return Err(FractalError::conflict(format!(
                    "page changed since it was read: {} (expected {expected_hash}, found {actual_hash})",
                    relative.display()
                )));
            }
        }
        if page_kind(&relative) == PageKind::Native {
            return Err(FractalError::invalid_input(
                "whole-source writes are only available for raw HTML; use a native section mutation",
            ));
        }
        let mut plan = MutationPlan::new(MutationKind::WriteRawPage);
        plan.write_page(relative, html);
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }

    /// Updates the HTML content section of a native page after verifying its expected hash.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let receipt = project.set_page_content("docs/guide.html", "<p>Updated</p>", expected_hash)?;
    /// # Ok::<(), project::Error>(())
    /// ```
    ///
    /// `expected_hash` must match the current content section hash.
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

    /// Updates the managed CSS style of a native page.
    
    ///
    
    /// The operation succeeds only when `expected_hash` matches the page's current
    
    /// style hash.
    
    ///
    
    /// # Examples
    
    ///
    
    /// ```ignore
    
    /// let receipt = project.set_page_style("docs/guide.html", "body { color: red; }", expected_hash)?;
    
    /// # Ok::<(), Error>(())
    
    /// ```
    
    ///
    
    /// # Returns
    
    ///
    
    /// A receipt describing the committed mutation.
    
    ///
    
    /// # Errors
    
    ///
    
    /// Returns an error if the page is not a native page or if its current style
    
    /// hash differs from `expected_hash`.
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

    /// Restores the default style of a native page after verifying its current style hash.
    ///
    /// # Examples
    ///
    /// ```
    /// # let mut project = /* an existing project */ todo!();
    /// # let expected_hash = /* the current style hash */ "";
    /// let receipt = project.restore_default_page_style("index.html", expected_hash)?;
    /// # Ok::<(), _>(())
    /// ```
    ///
    /// `expected_hash` prevents the update from overwriting a style changed since it was read.
    pub fn restore_default_page_style(
        &mut self,
        path: impl AsRef<Path>,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.set_page_style(path, DEFAULT_STYLE, expected_hash)
    }

    /// Replaces the user-defined metadata section of a native page.
    ///
    /// The update succeeds only when `expected_hash` matches the current metadata
    /// section hash.
    ///
    /// # Parameters
    ///
    /// * `html` — The replacement metadata HTML.
    /// * `expected_hash` — The hash of the metadata section expected before the update.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let mut project = todo!();
    /// let receipt = project.set_page_metadata(
    ///     "docs/page.html",
    ///     "<meta name=\"description\" content=\"Example\">",
    ///     "current-metadata-hash",
    /// )?;
    /// # let _ = receipt;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the page is not a native page, the expected hash is
    /// stale, or the metadata cannot be committed.
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

    /// Replaces the managed head-links section of a native page.
    ///
    /// `expected_hash` must match the current hash of the head-links section.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn example(mut project: Project) -> Result<MutationReceipt> {
    /// let receipt = project.set_page_head_links(
    ///     "docs/example.html",
    ///     r#"<link rel="stylesheet" href="styles.css">"#,
    ///     "current-head-links-hash",
    /// )?;
    /// # Ok(receipt)
    /// # }
    /// ```
    pub fn set_page_head_links(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<MutationReceipt> {
        self.mutate_native_section(
            path.as_ref(),
            expected_hash,
            "head links",
            MutationKind::SetPageHeadLinks,
            |document| document.set_head_links_html(html),
        )
    }

    /// Repairs the managed structure of a native page and saves the corrected document.
    ///
    /// # Errors
    ///
    /// Returns an error if the path does not identify a native page or if the document
    /// cannot be parsed, repaired, validated, or saved.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// project.repair_page_structure("docs/page.html")?;
    /// ```
    pub fn repair_page_structure(&mut self, path: impl AsRef<Path>) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let relative = self.existing_path(path.as_ref())?;
        if page_kind(&relative) != PageKind::Native {
            return Err(FractalError::invalid_input(
                "structure repair is only available for native documents",
            ));
        }
        let document = Document::parse(&self.stored(&relative)?.html);
        document.repair_managed_structure(DEFAULT_STYLE)?;
        self.commit_native_document(relative, document, MutationKind::RepairPageStructure)
    }

    /// Applies a mutation to a section of a native page after verifying its expected content hash.
    ///
    /// Returns a mutation receipt on success. Fails if the page is not native or if the section
    /// changed since the expected hash was obtained.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// project.set_page_content(
    ///     Path::new("docs/example.html"),
    ///     expected_hash,
    ///     MutationKind::SetPageContent,
    ///     |document| document.set_content("<p>Updated content</p>"),
    /// )?;
    /// ```
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
        if page_kind(&relative) != PageKind::Native {
            return Err(FractalError::invalid_input(
                "native section mutations require a native document",
            ));
        }
        let document = Document::parse(&self.stored(&relative)?.html);
        let actual_hash = match section {
            "content" => content_hash(&document.content_html()?),
            "style" => content_hash(&document.managed_style_css()?),
            "metadata" => content_hash(&document.user_metadata_html()?),
            "head links" => content_hash(&document.head_links_html()?),
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

    /// Commits a validated native document and reloads the project state.
    ///
    /// Returns a receipt describing the committed mutation. Rejects documents containing
    /// native-structure issues.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let receipt = project.commit_native_document(relative, document, operation)?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the document is invalid, serialization fails, the mutation
    /// cannot be committed, or the project cannot be reloaded.
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
    /// Moves a page to a new path and updates affected native-page references and folder metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let mut project = /* an open Project */;
    /// project.move_page("draft.html", "archive/draft.html")?;
    /// # Ok::<(), _>(())
    /// ```
    pub fn move_page(
        &mut self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let from = self.existing_path(from.as_ref())?;
        let kind = page_kind(&from);
        let to = normalize_destination_page_path(to.as_ref(), kind)?;
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
        let source_html = self.stored(&from)?.html.clone();
        let moved_html = if kind == PageKind::Native {
            let moved_document = Document::parse(&source_html);
            moved_document.rewrite_source_location(&from_string, &to_string);
            moved_document.serialize()?
        } else {
            source_html
        };

        let mut rewrites = Vec::new();
        for (path, stored) in &self.pages {
            if path == &from_string || stored.page.kind != PageKind::Native {
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
        if kind == PageKind::Native {
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
        }
        let receipt = plan.commit(&self.root)?;
        self.reload()?;
        Ok(receipt)
    }
    /// Renames a native page and optionally updates its title.
    ///
    /// When the path changes, internal references and folder ordering metadata are
    /// updated to point to the new path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let receipt = project.rename_native_with_title(
    ///     Path::new("old-page.html"),
    ///     Path::new("new-page.html"),
    ///     Some("New Page"),
    ///     MutationKind::Rename,
    /// )?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the destination already exists, the source page cannot
    /// be loaded, the updated document cannot be serialized, or the mutation fails.
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
        let moved_document = Document::parse(&self.stored(from)?.html);
        if let Some(title) = title {
            moved_document.set_title(title);
        }
        if from != to {
            moved_document.rewrite_source_location(&from_string, &to_string);
        }
        let mut plan = MutationPlan::new(operation);
        plan.write_page(to.to_path_buf(), moved_document.serialize()?);
        if from != to {
            plan.delete_page(from.to_path_buf());
            plan.move_page(from.to_path_buf(), to.to_path_buf());
            for (path, stored) in &self.pages {
                if path == &from_string || stored.page.kind != PageKind::Native {
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
    /// Deletes the page at the specified path.
    ///
    /// # Examples
    ///
    /// ```
    /// # let mut project = project;
    /// project.delete_page("notes.html")?;
    /// # Ok::<(), YourError>(())
    /// ```
    pub fn delete_page(&mut self, path: impl AsRef<Path>) -> Result<MutationReceipt> {
        self.delete_pages([path])
    }

    /// Deletes the specified pages in a single project mutation.
    ///
    /// References from surviving pages prevent deletion, while references between
    /// pages being deleted are allowed. Folder ordering metadata is updated, and
    /// only files that currently exist are physically removed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// project.delete_pages(["docs/old-page.html"])?;
    /// # Ok::<(), fractal::FractalError>(())
    /// ```
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

/// Validates that a native page path uses the filename derived from its title.
///
/// # Errors
///
/// Returns an error if the title cannot be converted to a slug or if the path
/// does not end with the title-derived native filename.
///
/// # Examples
///
/// ```
/// validate_title_driven_page_path(
///     std::path::Path::new("docs/my-page.html"),
///     "My Page",
/// )
/// .unwrap();
/// ```
fn validate_title_driven_page_path(path: &Path, title: &str) -> Result<()> {
    let expected = format!("{}{}", slug(title)?, NATIVE_SUFFIX);
    if path.file_name().and_then(|name| name.to_str()) != Some(expected.as_str()) {
        return Err(FractalError::invalid_input(format!(
            "native page path must end in `{expected}` to match its title"
        )));
    }
    Ok(())
}
