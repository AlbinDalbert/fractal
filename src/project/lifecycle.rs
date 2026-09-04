use super::support::*;
use super::*;

impl Project {
    /// Creates and opens a new project at the specified path.
    ///
    /// The project name must contain at least one non-whitespace character, and the
    /// destination directory must be empty or not yet exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tempfile::tempdir;
    /// # use your_crate::Project;
    /// # fn main() -> your_crate::Result<()> {
    /// let directory = tempdir()?;
    /// let project = Project::init(directory.path(), "Example")?;
    ///
    /// assert_eq!(project.manifest().name, "Example");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `path` - The directory in which to create the project.
    /// * `name` - The project's display name.
    ///
    /// # Errors
    ///
    /// Returns an error if the project name is invalid, the destination contains
    /// entries, or project creation fails.
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

    /// Opens an existing project from the specified directory.
    ///
    /// The directory must contain a project manifest, or a project lock that can
    /// be resolved to a valid manifest. Opening fails if pending transactions are
    /// present.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let project = Project::open("path/to/project")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// The opened project, or an error if the directory is not a valid project.
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

    /// Loads a project from its root directory.
    ///
    /// The directory must contain a valid manifest and pages directory. The project
    /// name and manifest version are validated before the project's pages and folders
    /// are loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read or parsed, the project name
    /// is invalid, the project version is unsupported, the pages directory is
    /// missing, or the project contents cannot be loaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let project = Project::load(std::path::PathBuf::from("my-project"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub(super) fn load(root: PathBuf) -> Result<Self> {
        let manifest_path = root.join(MANIFEST);
        let manifest: ProjectManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        validate_project_name(&manifest.name)
            .map_err(|_| FractalError::invalid_project("project name cannot be empty"))?;
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

    /// Recovers a project from interrupted mutations and removes leftover committed transaction directories.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let recovery = Project::recover("my-project");
    /// assert!(recovery.is_ok() || recovery.is_err());
    /// ```
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

    /// Provides access to the project's root directory.
    ///
    /// # Examples
    ///
    /// ```
    /// # let project: Project = todo!();
    /// let root = project.root();
    /// assert_eq!(root, project.root());
    /// ```
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
}

fn validate_project_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(FractalError::invalid_input("project name cannot be empty"));
    }
    Ok(())
}
