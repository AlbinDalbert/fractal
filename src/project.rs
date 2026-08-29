use crate::document::{
    escape_attribute, export_reference_id, is_external_href, relative_href, resolve_internal_href,
    Document,
};
use crate::types::*;
use crate::{FractalError, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const MANIFEST: &str = "fractal.json";
const PAGES: &str = "pages";
const MIN_SUPPORTED_VERSION: u32 = 1;
const VERSION: u32 = 2;
const NATIVE_SUFFIX: &str = ".fractal.html";
const TRANSACTION_PREFIX: &str = ".fractal-transaction-";

#[derive(Debug)]
pub struct Project {
    root: PathBuf,
    manifest: ProjectManifest,
    pages: BTreeMap<String, StoredPage>,
    folders: BTreeMap<String, StoredFolder>,
}

#[derive(Debug, Clone)]
struct StoredPage {
    page: Page,
    html: String,
}

#[derive(Debug, Clone)]
struct StoredFolder {
    folder: Folder,
    metadata: Option<FolderMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FolderMetadata {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<Vec<String>>,
}

impl Project {
    pub fn init(path: impl AsRef<Path>, name: impl Into<String>) -> Result<Self> {
        let root = path.as_ref();
        if root.exists() && root.read_dir()?.next().is_some() {
            return Err(FractalError::already_exists(format!(
                "directory is not empty: {}",
                root.display()
            )));
        }
        fs::create_dir_all(root.join(PAGES))?;
        let manifest = ProjectManifest {
            name: name.into(),
            version: VERSION,
        };
        atomic_write(
            &root.join(MANIFEST),
            &serde_json::to_string_pretty(&manifest)?,
        )?;
        Self::open(root)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        let manifest_path = root.join(MANIFEST);
        if !manifest_path.is_file() {
            return Err(FractalError::invalid_project(format!(
                "missing {}",
                manifest_path.display()
            )));
        }
        let _lock = ProjectLock::exclusive(&manifest_path)?;
        recover_transactions(&root)?;
        let manifest: ProjectManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        if !(MIN_SUPPORTED_VERSION..=VERSION).contains(&manifest.version) {
            return Err(FractalError::unsupported_version(format!(
                "unsupported project version {}",
                manifest.version
            )));
        }
        if !root.join(PAGES).is_dir() {
            return Err(FractalError::invalid_project("missing pages directory"));
        }
        let mut project = Self {
            root,
            manifest,
            pages: BTreeMap::new(),
            folders: BTreeMap::new(),
        };
        project.reload()?;
        Ok(project)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn pages(&self) -> Vec<Page> {
        self.pages
            .values()
            .map(|stored| stored.page.clone())
            .collect()
    }

    pub fn folders(&self) -> Vec<Folder> {
        self.folders
            .values()
            .map(|stored| stored.folder.clone())
            .collect()
    }

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
        let metadata = FolderMetadata {
            title: title.trim().to_owned(),
            order: stored
                .metadata
                .as_ref()
                .and_then(|value| value.order.clone()),
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

    pub fn page(&self, path: impl AsRef<Path>) -> Result<Page> {
        Ok(self.stored(path.as_ref())?.page.clone())
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.html.clone())
    }

    pub fn export_html(
        &self,
        path: impl AsRef<Path>,
        output: impl AsRef<Path>,
        options: HtmlExportOptions,
    ) -> Result<HtmlExportReport> {
        let page_path = self.existing_path(path.as_ref())?;
        let page_path_string = path_string(&page_path);
        let stored = self.stored(&page_path)?;
        if stored.page.kind != PageKind::Native {
            return Err(FractalError::invalid_input(
                "HTML export is only available for native documents",
            ));
        }
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
                if self
                    .pages
                    .get(target)
                    .is_some_and(|page| page.page.kind == PageKind::Native)
                {
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
                if self
                    .pages
                    .get(&link.target)
                    .is_some_and(|page| page.page.kind == PageKind::Native)
                {
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

    pub fn content_hash(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.page.content_hash.clone())
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
        );
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

    pub fn write_page(&mut self, path: impl AsRef<Path>, html: &str) -> Result<Mutation> {
        self.write_page_inner(path.as_ref(), html, None)
    }

    /// Replaces a page only when its current source hash matches `expected_hash`.
    ///
    /// Fractal holds the project lock while it refreshes the page, compares the
    /// hash, and atomically replaces the file.
    pub fn write_page_if_unchanged(
        &mut self,
        path: impl AsRef<Path>,
        html: &str,
        expected_hash: &str,
    ) -> Result<Mutation> {
        self.write_page_inner(path.as_ref(), html, Some(expected_hash))
    }

    fn write_page_inner(
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
            let issues = native_document_issues(&Document::parse(html));
            if let Some(issue) = issues.first() {
                return Err(FractalError::invalid_input(format!(
                    "invalid native document: {issue}"
                )));
            }
        }
        atomic_write(&self.root.join(PAGES).join(&relative), html)?;
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

    pub fn links(&self, path: impl AsRef<Path>) -> Result<Vec<Link>> {
        Ok(self.stored(path.as_ref())?.page.links.clone())
    }

    pub fn iframes(&self, path: impl AsRef<Path>) -> Result<Vec<Iframe>> {
        Ok(self.stored(path.as_ref())?.page.iframes.clone())
    }

    pub fn backlinks(&self, path: impl AsRef<Path>) -> Result<Vec<Backlink>> {
        let target = path_string(&self.existing_path(path.as_ref())?);
        let mut backlinks = Vec::new();
        for page in self.pages.values() {
            for link in &page.page.links {
                if matches!(&link.target, LinkTarget::Internal(value) if value == &target) {
                    backlinks.push(Backlink {
                        page: page.page.path.clone(),
                        text: link.text.clone(),
                    });
                }
            }
        }
        Ok(backlinks)
    }

    pub fn iframe_backlinks(&self, path: impl AsRef<Path>) -> Result<Vec<IframeBacklink>> {
        let target = path_string(&self.existing_path(path.as_ref())?);
        let mut backlinks = Vec::new();
        for page in self.pages.values() {
            for iframe in &page.page.iframes {
                if matches!(&iframe.target, IframeTarget::Internal(value) if value == &target) {
                    backlinks.push(IframeBacklink {
                        page: page.page.path.clone(),
                        title: iframe.title.clone(),
                    });
                }
            }
        }
        Ok(backlinks)
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let words: Vec<String> = query
            .split_whitespace()
            .map(|word| word.to_lowercase())
            .collect();
        if words.is_empty() {
            return vec![];
        }
        self.pages
            .values()
            .filter_map(|stored| {
                let haystack = format!(
                    "{} {}",
                    stored.page.title.as_deref().unwrap_or(""),
                    stored.page.text
                )
                .to_lowercase();
                words
                    .iter()
                    .all(|word| haystack.contains(word))
                    .then(|| SearchResult {
                        path: stored.page.path.clone(),
                        title: stored.page.title.clone(),
                        snippet: snippet(&stored.page.text, &words[0]),
                    })
            })
            .collect()
    }

    /// Finds unambiguous, case-insensitive exact-title matches without changing source.
    pub fn derived_links(&self, path: impl AsRef<Path>) -> Result<Vec<DerivedLink>> {
        let source = self.stored(path.as_ref())?;
        let mut titles: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
        for target in self.pages.values() {
            if target.page.path == source.page.path {
                continue;
            }
            let Some(title) = target.page.title.as_deref() else {
                continue;
            };
            titles
                .entry(title.to_lowercase())
                .or_default()
                .push((&target.page.path, title));
        }
        let mut titles: Vec<_> = titles
            .into_values()
            .filter_map(|targets| match targets.as_slice() {
                [(path, title)] => Some((*path, *title)),
                _ => None,
            })
            .collect();
        titles.sort_by(|(left_path, left_title), (right_path, right_title)| {
            right_title
                .chars()
                .count()
                .cmp(&left_title.chars().count())
                .then_with(|| left_path.cmp(right_path))
        });

        let document = Document::parse(&source.html);
        let mut links = Vec::new();
        for node in document.unlinked_text_nodes() {
            let mut matches = Vec::new();
            for (target, title) in &titles {
                for (start, end) in exact_case_insensitive_matches(&node.text, title) {
                    matches.push((start, end, *target));
                }
            }
            matches.sort_by(
                |(left_start, left_end, left_target), (right_start, right_end, right_target)| {
                    left_start
                        .cmp(right_start)
                        .then_with(|| (right_end - right_start).cmp(&(left_end - left_start)))
                        .then_with(|| left_target.cmp(right_target))
                },
            );

            let mut claimed_until = 0;
            for (start, end, target) in matches {
                if start < claimed_until {
                    continue;
                }
                links.push(DerivedLink {
                    text: node.text[start..end].to_string(),
                    target: target.to_string(),
                    occurrence: TextOccurrence {
                        start: TextPosition {
                            text_node: node.index,
                            offset: node.text[..start].encode_utf16().count(),
                        },
                        end: TextPosition {
                            text_node: node.index,
                            offset: node.text[..end].encode_utf16().count(),
                        },
                    },
                });
                claimed_until = end;
            }
        }
        Ok(links)
    }

    pub fn insert_link(
        &mut self,
        page: impl AsRef<Path>,
        text: &str,
        target: impl AsRef<Path>,
    ) -> Result<Mutation> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let page = self.existing_path(page.as_ref())?;
        let target = self.existing_path(target.as_ref())?;
        if page_kind(&page) != PageKind::Native {
            return Err(FractalError::invalid_input(
                "semantic link insertion is only available for native documents",
            ));
        }
        if page == target {
            return Err(FractalError::invalid_input("cannot link a page to itself"));
        }
        let stored = self.stored(&page)?.clone();
        let document = Document::parse(&stored.html);
        let href = relative_href(&path_string(&page), &path_string(&target));
        if !document.insert_link(text, &href)? {
            return Err(FractalError::not_found(format!(
                "unlinked text not found: {text}"
            )));
        }
        atomic_write(&self.root.join(PAGES).join(&page), &document.serialize()?)?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![page],
            deleted: vec![],
        })
    }

    pub fn validate(&self) -> ValidationReport {
        let mut issues = Vec::new();
        if self.manifest.name.trim().is_empty() {
            issues.push(ValidationIssue {
                path: None,
                message: "project name is empty".into(),
            });
        }
        for stored in self.folders.values() {
            for issue in &stored.folder.issues {
                issues.push(ValidationIssue {
                    path: Some(if stored.folder.path.is_empty() {
                        format!("{PAGES}/{MANIFEST}")
                    } else {
                        format!("{}/{MANIFEST}", stored.folder.path)
                    }),
                    message: format!("{}: {}", issue.name, issue.message),
                });
            }
        }
        for stored in self.pages.values() {
            if stored.page.kind != PageKind::Native {
                continue;
            }
            let document = Document::parse(&stored.html);
            for message in native_document_issues(&document) {
                issues.push(ValidationIssue {
                    path: Some(stored.page.path.clone()),
                    message,
                });
            }
            for link in &stored.page.links {
                if let LinkTarget::Broken(target) = &link.target {
                    issues.push(ValidationIssue {
                        path: Some(stored.page.path.clone()),
                        message: format!(
                            "broken internal link `{}` resolves to `{target}`",
                            link.href
                        ),
                    });
                }
            }
            for iframe in &stored.page.iframes {
                match &iframe.target {
                    IframeTarget::Broken(target) => issues.push(ValidationIssue {
                        path: Some(stored.page.path.clone()),
                        message: format!(
                            "broken iframe source `{}` resolves to `{target}`",
                            iframe.src.as_deref().unwrap_or("")
                        ),
                    }),
                    IframeTarget::Missing => issues.push(ValidationIssue {
                        path: Some(stored.page.path.clone()),
                        message: "iframe needs a non-empty `src` or a `srcdoc` attribute".into(),
                    }),
                    _ => {}
                }
            }
        }
        ValidationReport {
            valid: issues.is_empty(),
            issues,
        }
    }

    fn lock_for_mutation(&self) -> Result<ProjectLock> {
        let lock = ProjectLock::exclusive(&self.root.join(MANIFEST))?;
        recover_transactions(&self.root)?;
        Ok(lock)
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

    fn reject_references_into(
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

    fn folder_metadata_child_change(
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

    fn folder_metadata_replace_child(
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

    fn stored(&self, path: &Path) -> Result<&StoredPage> {
        let path = path_string(&self.existing_path(path)?);
        self.pages
            .get(&path)
            .ok_or_else(|| FractalError::not_found(format!("page does not exist: {path}")))
    }

    fn existing_path(&self, path: &Path) -> Result<PathBuf> {
        let normalized = normalize_relative_path(path)?;
        let candidates = if normalized.extension().is_some() {
            validate_html_path(&normalized)?;
            vec![normalized]
        } else {
            vec![
                append_native_suffix(&normalized)?,
                normalized.with_extension("html"),
            ]
        };
        let found: Vec<_> = candidates
            .into_iter()
            .filter(|candidate| self.pages.contains_key(&path_string(candidate)))
            .collect();
        match found.as_slice() {
            [path] => Ok(path.clone()),
            [] => Err(FractalError::not_found(format!(
                "page does not exist: {}",
                path.display()
            ))),
            _ => Err(FractalError::invalid_input(format!(
                "page path is ambiguous, include the suffix: {}",
                path.display()
            ))),
        }
    }

    fn existing_or_ghost_native_path(&self, path: &Path) -> Result<PathBuf> {
        if let Ok(path) = self.existing_path(path) {
            return Ok(path);
        }
        let normalized = normalize_native_page_path(path)?;
        let parent = normalized.parent().unwrap_or_else(|| Path::new(""));
        let name = normalized
            .file_name()
            .expect("native path has a name")
            .to_string_lossy();
        if self
            .folders
            .get(&path_string(parent))
            .is_some_and(|stored| {
                stored.folder.children.iter().any(|child| {
                    child.name == name
                        && child.kind == FolderChildKind::Native
                        && child.status == FolderChildStatus::Missing
                })
            })
        {
            Ok(normalized)
        } else {
            Err(FractalError::not_found(format!(
                "page does not exist: {}",
                path.display()
            )))
        }
    }

    fn reload(&mut self) -> Result<()> {
        self.reload_folders()?;
        let mut files = Vec::new();
        collect_html(&self.root.join(PAGES), &self.root.join(PAGES), &mut files)?;
        let known: BTreeSet<String> = files.iter().map(|path| path_string(path)).collect();
        let mut pages = BTreeMap::new();
        for relative in files {
            let path = path_string(&relative);
            let html = fs::read_to_string(self.root.join(PAGES).join(&relative))?;
            let document = Document::parse(&html);
            let links = document
                .raw_links()
                .into_iter()
                .map(|(href, text)| {
                    let target = if href.starts_with('#') {
                        LinkTarget::Fragment(href.clone())
                    } else if is_external_href(&href) {
                        LinkTarget::External(href.clone())
                    } else if let Some(resolved) = resolve_internal_href(&path, &href) {
                        if known.contains(&resolved) {
                            LinkTarget::Internal(resolved)
                        } else if self.root.join(PAGES).join(&resolved).is_file() {
                            LinkTarget::InternalFile(resolved)
                        } else {
                            LinkTarget::Broken(resolved)
                        }
                    } else {
                        LinkTarget::Broken(href.clone())
                    };
                    Link { href, text, target }
                })
                .collect();
            let iframes = document
                .raw_iframes()
                .into_iter()
                .map(|iframe| {
                    let target = if iframe.has_srcdoc {
                        IframeTarget::Inline
                    } else if let Some(src) =
                        iframe.src.as_deref().filter(|value| !value.is_empty())
                    {
                        if is_external_href(src) {
                            IframeTarget::External(src.to_string())
                        } else if let Some(resolved) = resolve_internal_href(&path, src) {
                            if known.contains(&resolved) {
                                IframeTarget::Internal(resolved)
                            } else if self.root.join(PAGES).join(&resolved).is_file() {
                                IframeTarget::InternalFile(resolved)
                            } else {
                                IframeTarget::Broken(resolved)
                            }
                        } else {
                            IframeTarget::Broken(src.to_string())
                        }
                    } else {
                        IframeTarget::Missing
                    };
                    Iframe {
                        src: iframe.src,
                        title: iframe.title,
                        sandbox: iframe.sandbox,
                        target,
                    }
                })
                .collect();
            let page = Page {
                path: path.clone(),
                content_hash: content_hash(&html),
                kind: page_kind(&relative),
                title: document.title(),
                text: document.text(),
                links,
                iframes,
            };
            pages.insert(path, StoredPage { page, html });
        }
        self.pages = pages;
        Ok(())
    }

    fn reload_folders(&mut self) -> Result<()> {
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

fn normalize_folder_path(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() || path == Path::new(".") || path == Path::new(PAGES) {
        return Ok(PathBuf::new());
    }
    normalize_relative_path(path)
}

fn display_folder_path(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        PAGES.into()
    } else {
        path.display().to_string()
    }
}

fn folder_metadata_relative_path(folder: &Path) -> PathBuf {
    folder.join(MANIFEST)
}

fn default_folder_title(project_name: &str, path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| project_name.to_owned())
}

fn ordered_name_kind(name: &str) -> FolderChildKind {
    if name.ends_with(NATIVE_SUFFIX) {
        FolderChildKind::Native
    } else {
        FolderChildKind::Folder
    }
}

fn direct_orderable_children(directory: &Path) -> Result<BTreeMap<String, FolderChildKind>> {
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

fn validate_stored_order(
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

fn validate_order_name(name: &str) -> Result<()> {
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

fn collect_directories(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
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

fn normalize_relative_path(path: &Path) -> Result<PathBuf> {
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

fn validate_html_path(path: &Path) -> Result<()> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("html") {
        return Err(FractalError::invalid_input("page path must end in .html"));
    }
    Ok(())
}

fn append_native_suffix(path: &Path) -> Result<PathBuf> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(FractalError::invalid_input("invalid native page path"));
    };
    let mut output = path.to_path_buf();
    output.set_file_name(format!("{name}{NATIVE_SUFFIX}"));
    Ok(output)
}

fn normalize_native_page_path(path: &Path) -> Result<PathBuf> {
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

fn normalize_destination_page_path(path: &Path, kind: PageKind) -> Result<PathBuf> {
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

fn page_kind(path: &Path) -> PageKind {
    if path_string(path).ends_with(NATIVE_SUFFIX) {
        PageKind::Native
    } else {
        PageKind::Raw
    }
}

fn native_document_issues(document: &Document) -> Vec<String> {
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

fn collect_html(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
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

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
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

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
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
struct TransactionPlan {
    affected: Vec<PathBuf>,
    originals: BTreeSet<PathBuf>,
}

struct ProjectLock {
    _file: File,
}

impl ProjectLock {
    fn exclusive(path: &Path) -> Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        FileExt::lock_exclusive(&file)?;
        Ok(Self { _file: file })
    }
}

fn commit_file_transaction(
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

fn recover_transactions(root: &Path) -> Result<()> {
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

fn recover_transaction(transaction_root: &Path) -> Result<()> {
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

fn reject_overlapping_transaction_paths(paths: &BTreeSet<PathBuf>) -> Result<()> {
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

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn remove_path_if_present(path: &Path) -> Result<()> {
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

fn path_starts_with(path: &Path, parent: &Path) -> bool {
    path.starts_with(parent)
}

fn link_target_path(target: &LinkTarget) -> Option<&str> {
    match target {
        LinkTarget::Internal(path) | LinkTarget::InternalFile(path) => Some(path),
        _ => None,
    }
}

fn iframe_target_path(target: &IframeTarget) -> Option<&str> {
    match target {
        IframeTarget::Internal(path) | IframeTarget::InternalFile(path) => Some(path),
        _ => None,
    }
}

fn content_hash(contents: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(contents.as_bytes()))
}

fn slug(title: &str) -> Result<String> {
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

fn exact_case_insensitive_matches(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
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

fn snippet(text: &str, query: &str) -> String {
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

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
