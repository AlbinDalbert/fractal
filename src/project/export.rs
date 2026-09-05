use super::support::*;
use super::*;

impl Project {
    /// Exports one native document as standalone HTML.
    ///
    /// Internal native-document references are appended to the output so the
    /// exported file remains self-contained. Derived references are included
    /// when requested by `options`.
    pub fn export_html(
        &self,
        path: impl AsRef<Path>,
        output: impl AsRef<Path>,
        options: HtmlExportOptions,
    ) -> Result<HtmlExportReport> {
        let page_path = self.existing_path(path.as_ref())?;
        let page_path_string = path_string(&page_path);
        let stored = self.stored(&page_path)?;
        if let Some(issue) = native_document_issues(&Document::parse(&stored.html)).first() {
            return Err(FractalError::invalid_input(format!(
                "cannot export invalid native document: {issue}"
            )));
        }

        let mut references = Vec::new();
        let mut seen = BTreeSet::new();
        let mut derived_references = Vec::new();
        let add_reference =
            |target: &str, references: &mut Vec<String>, seen: &mut BTreeSet<String>| {
                if target == page_path_string || !seen.insert(target.to_string()) {
                    return;
                }
                if self.pages.contains_key(target) {
                    references.push(target.to_string());
                }
            };

        let document = Document::parse(&stored.html);
        for (href, _) in document.raw_links() {
            if let Some(target) = resolve_internal_href(&page_path_string, &href) {
                add_reference(&target, &mut references, &mut seen);
            }
        }
        if options.include_derived_links {
            for link in self.derived_links(&page_path)? {
                if self.pages.contains_key(&link.target) {
                    add_reference(&link.target, &mut references, &mut seen);
                    derived_references.push(link);
                }
            }
        }

        let export = Document::parse(&stored.html);
        let native_targets: BTreeSet<_> = references.iter().cloned().collect();
        export.flatten_for_html(&page_path_string, &native_targets, &derived_references)?;
        if !references.is_empty() {
            let mut section = String::from(
                r#"<section id="fractal-references">
  <h2>References</h2>
"#,
            );
            for reference in &references {
                let referenced = self
                    .pages
                    .get(reference)
                    .expect("reference was collected from the project");
                let title = referenced
                    .page
                    .title
                    .clone()
                    .unwrap_or_else(|| reference.clone());
                let text = Document::parse(&referenced.html).export_text();
                section.push_str(&format!(
                    "  <details id=\"{}\">\n    <summary>{}</summary>\n    <p>{}</p>\n  </details>\n",
                    escape_attribute(&export_reference_id(reference)),
                    escape_html(&title),
                    escape_html(&text),
                ));
            }
            section.push_str("</section>");
            export.append_to_main(&section)?;
        }

        let output = output.as_ref().to_path_buf();
        atomic_write(&output, &export.serialize()?)?;
        Ok(HtmlExportReport { output, references })
    }

    /// Exports a folder or selected descendants as one ordered HTML document.
    ///
    /// Selection paths are relative to `folder`. Invalid selected pages reject
    /// the export unless [`FolderHtmlExportOptions::force`] is set.
    pub fn export_folder_html(
        &self,
        folder: impl AsRef<Path>,
        output: impl AsRef<Path>,
        options: FolderHtmlExportOptions,
    ) -> Result<FolderHtmlExportReport> {
        let folder = normalize_folder_path(folder.as_ref())?;
        let stored_folder = self.folders.get(&path_string(&folder)).ok_or_else(|| {
            FractalError::not_found(format!(
                "folder does not exist: {}",
                display_folder_path(&folder)
            ))
        })?;
        let selections = normalize_export_selections(&options.selections)?;
        for selection in &selections {
            if !self.export_selection_exists(&folder, selection) {
                return Err(FractalError::not_found(format!(
                    "export selection does not exist: {selection}"
                )));
            }
        }

        let mut candidates = Vec::new();
        self.collect_folder_export_pages(
            &folder,
            Path::new(""),
            selections.is_empty(),
            &selections,
            &mut candidates,
        )?;
        let mut pages = Vec::new();
        let mut skipped = Vec::new();
        for path in candidates {
            let stored = self
                .pages
                .get(&path)
                .expect("folder traversal returned a present native page");
            if let Some(issue) = native_document_issues(&Document::parse(&stored.html)).first() {
                let reason = format!("invalid native document: {issue}");
                if !options.force {
                    return Err(FractalError::invalid_input(format!(
                        "cannot export invalid native document `{path}`: {issue}"
                    )));
                }
                skipped.push(SkippedExportPage { path, reason });
            } else {
                pages.push(path);
            }
        }

        let included_targets: BTreeMap<String, String> = pages
            .iter()
            .map(|path| (path.clone(), folder_export_page_id(path)))
            .collect();
        let included: BTreeSet<String> = pages.iter().cloned().collect();
        let mut references = Vec::new();
        let mut seen_references = BTreeSet::new();
        let mut derived_by_page = BTreeMap::<String, Vec<DerivedLink>>::new();
        for path in &pages {
            let stored = self.pages.get(path).expect("included page exists");
            for link in &stored.page.links {
                if let LinkTarget::Internal(target) = &link.target {
                    if !included.contains(target)
                        && self.pages.contains_key(target)
                        && seen_references.insert(target.clone())
                    {
                        references.push(target.clone());
                    }
                }
            }
            if options.include_derived_links {
                let derived = self.derived_links(path)?;
                for link in &derived {
                    if !included.contains(&link.target)
                        && seen_references.insert(link.target.clone())
                    {
                        references.push(link.target.clone());
                    }
                }
                derived_by_page.insert(path.clone(), derived);
            }
        }
        let reference_targets: BTreeSet<String> = references.iter().cloned().collect();

        let mut main = String::new();
        for (index, path) in pages.iter().enumerate() {
            if index > 0 {
                main.push_str("\n<hr>\n");
            }
            let stored = self.pages.get(path).expect("included page exists");
            let title = stored.page.title.as_deref().unwrap_or(path);
            let heading = if options.number_sections {
                format!("{}. {title}", index + 1)
            } else {
                title.to_owned()
            };
            let document = Document::parse(&stored.html);
            let content = document.folder_export_content(
                path,
                &included_targets,
                &reference_targets,
                derived_by_page.get(path).map(Vec::as_slice).unwrap_or(&[]),
            )?;
            main.push_str(&format!(
                "<section id=\"{}\" data-fractal-source=\"{}\">\n  <h1>{}</h1>{}\n</section>",
                escape_attribute(included_targets.get(path).expect("included page has an id")),
                escape_attribute(path),
                escape_html(&heading),
                content
            ));
        }
        if !references.is_empty() {
            main.push_str("\n<section id=\"fractal-references\">\n  <h2>References</h2>\n");
            for reference in &references {
                let referenced = self.pages.get(reference).expect("reference target exists");
                let title = referenced.page.title.as_deref().unwrap_or(reference);
                let text = Document::parse(&referenced.html).export_text();
                main.push_str(&format!(
                    "  <details id=\"{}\">\n    <summary>{}</summary>\n    <p>{}</p>\n  </details>\n",
                    escape_attribute(&export_reference_id(reference)),
                    escape_html(title),
                    escape_html(&text),
                ));
            }
            main.push_str("</section>");
        }

        let html = folder_export_shell(&stored_folder.folder.title, &main);
        let output = output.as_ref().to_path_buf();
        atomic_write(&output, &html)?;
        Ok(FolderHtmlExportReport {
            output,
            pages,
            skipped,
            references,
        })
    }

    fn collect_folder_export_pages(
        &self,
        folder: &Path,
        relative: &Path,
        include_all: bool,
        selections: &BTreeSet<String>,
        output: &mut Vec<String>,
    ) -> Result<()> {
        let stored = self.folders.get(&path_string(folder)).ok_or_else(|| {
            FractalError::not_found(format!(
                "folder does not exist: {}",
                display_folder_path(folder)
            ))
        })?;
        for child in &stored.folder.children {
            if child.status == FolderChildStatus::Missing {
                continue;
            }
            let child_relative = relative.join(&child.name);
            let child_selection = path_string(&child_relative);
            match child.kind {
                FolderChildKind::Native => {
                    if include_all || selections.contains(&child_selection) {
                        output.push(path_string(&folder.join(&child.name)));
                    }
                }
                FolderChildKind::Folder => {
                    let has_exact = selections.contains(&child_selection);
                    let prefix = format!("{child_selection}/");
                    let has_descendant = selections
                        .iter()
                        .any(|selection| selection.starts_with(&prefix));
                    if include_all || has_exact || has_descendant {
                        self.collect_folder_export_pages(
                            &folder.join(&child.name),
                            &child_relative,
                            include_all || (has_exact && !has_descendant),
                            selections,
                            output,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn export_selection_exists(&self, folder: &Path, selection: &str) -> bool {
        let absolute = folder.join(selection);
        if self.pages.contains_key(&path_string(&absolute))
            || self.folders.contains_key(&path_string(&absolute))
        {
            return true;
        }
        let Some(parent) = absolute.parent() else {
            return false;
        };
        let Some(name) = absolute.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        self.folders
            .get(&path_string(parent))
            .is_some_and(|stored| {
                stored
                    .folder
                    .children
                    .iter()
                    .any(|child| child.name == name)
            })
    }
}
