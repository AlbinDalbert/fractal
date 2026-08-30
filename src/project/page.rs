use super::support::*;
use super::*;

impl Project {
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

    /// Changes a native document title and derives its filename from that title.
    ///
    /// The source update, path change, explicit-link rewrites, and folder order
    /// update commit as one recoverable transaction.
    pub fn set_page_title(&mut self, path: impl AsRef<Path>, title: &str) -> Result<Mutation> {
        self.set_page_title_inner(path.as_ref(), title, None)
    }

    pub fn set_page_title_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        title: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.set_page_title_inner(path.as_ref(), title, Some(expected_hash))
    }

    fn set_page_title_inner(
        &mut self,
        path: &Path,
        title: &str,
        expected_hash: Option<&str>,
    ) -> Result<Mutation> {
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
        self.rename_native_with_title(&from, &to, Some(title))
    }

    pub fn create_page(&mut self, title: &str) -> Result<Mutation> {
        let stem = slug(title)?;
        self.create_page_at(format!("{stem}{NATIVE_SUFFIX}"), title)
    }

    pub fn create_page_at(&mut self, path: impl AsRef<Path>, title: &str) -> Result<Mutation> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        if title.trim().is_empty() {
            return Err(FractalError::invalid_input("title cannot be empty"));
        }
        let relative = normalize_native_page_path(path.as_ref())?;
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
        let mut writes = vec![(relative.clone(), html)];
        let mut changed = vec![relative.clone()];
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
            changed.push(write.0.clone());
            writes.push(write);
        }
        commit_file_transaction(&self.root, writes, vec![])?;
        self.reload()?;
        Ok(Mutation {
            changed,
            deleted: vec![],
        })
    }

    pub fn write_raw_page(&mut self, path: impl AsRef<Path>, html: &str) -> Result<Mutation> {
        self.write_raw_page_inner(path.as_ref(), html, None)
    }

    /// Replaces a page only when its current source hash matches `expected_hash`.
    ///
    /// Fractal holds the project lock while it refreshes the page, compares the
    /// hash, and atomically replaces the file.
    pub fn write_raw_page_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.write_raw_page_inner(path.as_ref(), html, Some(expected_hash))
    }

    fn write_raw_page_inner(
        &mut self,
        path: &Path,
        html: &str,
        expected_hash: Option<&str>,
    ) -> Result<Mutation> {
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
        atomic_write(&self.root.join(PAGES).join(&relative), html)?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![relative],
            deleted: vec![],
        })
    }

    pub fn set_page_content(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.mutate_native_section(path.as_ref(), expected_hash, "content", |document| {
            document.set_content_html(html)
        })
    }

    pub fn set_page_style(
        &mut self,
        path: impl AsRef<Path>,
        css: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.mutate_native_section(path.as_ref(), expected_hash, "style", |document| {
            document.set_managed_style_css(css)
        })
    }

    pub fn restore_default_page_style(
        &mut self,
        path: impl AsRef<Path>,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.set_page_style(path, DEFAULT_STYLE, expected_hash)
    }

    pub fn set_page_metadata(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.mutate_native_section(path.as_ref(), expected_hash, "metadata", |document| {
            document.set_user_metadata_html(html)
        })
    }

    pub fn set_page_head_links(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.mutate_native_section(path.as_ref(), expected_hash, "head links", |document| {
            document.set_head_links_html(html)
        })
    }

    pub fn repair_page_structure(&mut self, path: impl AsRef<Path>) -> Result<Mutation> {
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
        self.commit_native_document(relative, document)
    }

    fn mutate_native_section<F>(
        &mut self,
        path: &Path,
        expected_hash: &str,
        section: &str,
        mutate: F,
    ) -> Result<Mutation>
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
        self.commit_native_document(relative, document)
    }

    fn commit_native_document(
        &mut self,
        relative: PathBuf,
        document: Document,
    ) -> Result<Mutation> {
        let issues = native_document_issues(&document);
        if let Some(issue) = issues.first() {
            return Err(FractalError::invalid_input(format!(
                "invalid native document: {issue}"
            )));
        }
        atomic_write(
            &self.root.join(PAGES).join(&relative),
            &document.serialize()?,
        )?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![relative],
            deleted: vec![],
        })
    }
    pub fn move_page(&mut self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<Mutation> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let from = self.existing_path(from.as_ref())?;
        let kind = page_kind(&from);
        let to = normalize_destination_page_path(to.as_ref(), kind)?;
        if from == to {
            return Ok(Mutation {
                changed: vec![],
                deleted: vec![],
            });
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

        let mut writes = vec![(to.clone(), moved_html)];
        let mut changed = vec![to.clone()];
        for (path, html) in rewrites {
            writes.push((PathBuf::from(&path), html));
            changed.push(PathBuf::from(path));
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
                    changed.push(write.0.clone());
                    writes.push(write);
                }
            } else {
                if let Some(write) =
                    self.folder_metadata_child_change(from_parent, Some(from_name.as_ref()), None)?
                {
                    changed.push(write.0.clone());
                    writes.push(write);
                }
                if let Some(write) =
                    self.folder_metadata_child_change(to_parent, None, Some(to_name.as_ref()))?
                {
                    changed.push(write.0.clone());
                    writes.push(write);
                }
            }
        }
        commit_file_transaction(&self.root, writes, vec![from.clone()])?;
        self.reload()?;
        Ok(Mutation {
            changed,
            deleted: vec![from],
        })
    }
    pub(super) fn rename_native_with_title(
        &mut self,
        from: &Path,
        to: &Path,
        title: Option<&str>,
    ) -> Result<Mutation> {
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
        let mut writes = vec![(to.to_path_buf(), moved_document.serialize()?)];
        let mut changed = vec![to.to_path_buf()];
        if from != to {
            for (path, stored) in &self.pages {
                if path == &from_string || stored.page.kind != PageKind::Native {
                    continue;
                }
                let document = Document::parse(&stored.html);
                if document.rewrite_internal_target(path, &from_string, &to_string) > 0 {
                    writes.push((PathBuf::from(path), document.serialize()?));
                    changed.push(PathBuf::from(path));
                }
            }
            let parent = from.parent().unwrap_or_else(|| Path::new(""));
            let old = from.file_name().expect("page has a name").to_string_lossy();
            let new = to.file_name().expect("page has a name").to_string_lossy();
            if let Some(write) = self.folder_metadata_replace_child(parent, &old, &new)? {
                changed.push(write.0.clone());
                writes.push(write);
            }
        }
        let deletes = (from != to)
            .then(|| from.to_path_buf())
            .into_iter()
            .collect();
        commit_file_transaction(&self.root, writes, deletes)?;
        self.reload()?;
        Ok(Mutation {
            changed,
            deleted: (from != to)
                .then(|| from.to_path_buf())
                .into_iter()
                .collect(),
        })
    }
    pub fn delete_page(&mut self, path: impl AsRef<Path>) -> Result<Mutation> {
        self.delete_pages([path])
    }

    /// Deletes a set of pages as one locked project operation.
    ///
    /// References between pages in the set do not block deletion. References
    /// from pages that survive do block it.
    pub fn delete_pages<I, P>(&mut self, paths: I) -> Result<Mutation>
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
        let deleted: Vec<PathBuf> = relative.iter().cloned().collect();
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
        let changed = writes.iter().map(|(path, _)| path.clone()).collect();
        let physical_deletes: Vec<_> = deleted
            .iter()
            .filter(|path| path_exists(&self.root.join(PAGES).join(path)))
            .cloned()
            .collect();
        commit_file_transaction(&self.root, writes, physical_deletes)?;
        self.reload()?;
        Ok(Mutation { changed, deleted })
    }
}
