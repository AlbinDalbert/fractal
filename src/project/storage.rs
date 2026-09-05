use super::support::*;
use super::*;

impl Project {
    pub(super) fn stored(&self, path: &Path) -> Result<&StoredPage> {
        let path = path_string(&self.existing_path(path)?);
        self.pages
            .get(&path)
            .ok_or_else(|| FractalError::not_found(format!("page does not exist: {path}")))
    }

    pub(super) fn existing_path(&self, path: &Path) -> Result<PathBuf> {
        let normalized = normalize_native_page_path(path)?;
        if self.pages.contains_key(&path_string(&normalized)) {
            Ok(normalized)
        } else {
            Err(FractalError::not_found(format!(
                "page does not exist: {}",
                path.display()
            )))
        }
    }

    pub(super) fn existing_or_ghost_native_path(&self, path: &Path) -> Result<PathBuf> {
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

    pub(super) fn lock_for_mutation(&self) -> Result<ProjectLock> {
        let lock = ProjectLock::exclusive(&self.root)?;
        ensure_no_pending_transactions(&self.root)?;
        Ok(lock)
    }
    pub(super) fn reload(&mut self) -> Result<()> {
        self.reload_folders()?;
        let mut files = Vec::new();
        collect_native_documents(&self.root.join(PAGES), &self.root.join(PAGES), &mut files)?;
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
                        } else {
                            LinkTarget::Broken(resolved)
                        }
                    } else {
                        LinkTarget::Broken(href.clone())
                    };
                    Link { href, text, target }
                })
                .collect();
            let page = Page {
                path: path.clone(),
                content_hash: content_hash(&html),
                title: document.title(),
                text: document.text(),
                links,
            };
            pages.insert(path, StoredPage { page, html });
        }
        self.pages = pages;
        Ok(())
    }
}
