use super::support::*;
use super::*;

impl Project {
    /// Creates a project and opens it.
    ///
    /// `path` must be absent or an empty directory. The name must contain at
    /// least one non-whitespace character.
    pub fn init(path: impl AsRef<Path>, name: impl Into<String>) -> Result<Self> {
        let root = path.as_ref();
        let name = name.into();
        validate_project_name(&name)?;
        if root.exists() && root.read_dir()?.next().is_some() {
            return Err(FractalError::already_exists(format!(
                "directory is not empty: {}",
                root.display()
            )));
        }
        fs::create_dir_all(root.join(PAGES))?;
        let manifest = ProjectManifest {
            name,
            version: VERSION,
        };
        atomic_write(
            &root.join(MANIFEST),
            &serde_json::to_string_pretty(&manifest)?,
        )?;
        ProjectLock::initialize(root)?;
        Self::open(root)
    }

    /// Opens and indexes an existing project.
    ///
    /// Opening fails while an interrupted transaction requires recovery. Use
    /// [`Project::inspect`] to distinguish that state from other project errors
    /// and [`Project::recover`] to resolve it.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        let manifest_path = root.join(MANIFEST);
        if !manifest_path.is_file() && !root.join(LOCK).is_file() {
            return Err(FractalError::invalid_project(format!(
                "missing {}",
                manifest_path.display()
            )));
        }
        let _lock = ProjectLock::shared(&root)?;
        ensure_no_pending_transactions(&root)?;
        if !manifest_path.is_file() {
            return Err(FractalError::invalid_project(format!(
                "missing {}",
                manifest_path.display()
            )));
        }
        Self::load(root)
    }

    pub(super) fn load(root: PathBuf) -> Result<Self> {
        let manifest_path = root.join(MANIFEST);
        let manifest: ProjectManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        validate_project_name(&manifest.name)
            .map_err(|_| FractalError::invalid_project("project name cannot be empty"))?;
        if manifest.version != VERSION {
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

    /// Rolls back interrupted mutations and removes committed transaction
    /// directories left behind by failed cleanup.
    pub fn recover(path: impl AsRef<Path>) -> Result<RecoveryReport> {
        let root = path.as_ref();
        let manifest_path = root.join(MANIFEST);
        if !manifest_path.is_file() && !root.join(LOCK).is_file() {
            return Err(FractalError::invalid_project(format!(
                "missing {}",
                manifest_path.display()
            )));
        }
        let _lock = ProjectLock::exclusive(root)?;
        recover_all_transactions(root)
    }

    /// Returns the project root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the loaded project manifest.
    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    /// Returns all indexed pages in project-path order.
    pub fn pages(&self) -> Vec<Page> {
        self.pages
            .values()
            .map(|stored| stored.page.clone())
            .collect()
    }

    /// Returns all indexed folders in project-path order.
    pub fn folders(&self) -> Vec<Folder> {
        self.folders
            .values()
            .map(|stored| stored.folder.clone())
            .collect()
    }
}

fn validate_project_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(FractalError::invalid_input("project name cannot be empty"));
    }
    Ok(())
}
