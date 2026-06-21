use crate::io::fs::atomic_write;
use crate::project::constants::WORKSPACE_DIR;
use crate::types::{OperationEvent, OperationReport};
use crate::{FractalError, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const MUTATION_LOCK_FILE: &str = "mutation.lock";

#[derive(Debug, Default)]
pub(crate) struct MutationPlan {
    steps: Vec<MutationStep>,
}

impl MutationPlan {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn event(&mut self, event: OperationEvent) {
        self.steps.push(MutationStep::Event(event));
    }

    pub(crate) fn create_dir(&mut self, path: impl Into<PathBuf>, event: OperationEvent) {
        self.steps.push(MutationStep::CreateDir {
            path: path.into(),
            event,
        });
    }

    pub(crate) fn ensure_dir(&mut self, path: impl Into<PathBuf>) {
        self.steps
            .push(MutationStep::EnsureDir { path: path.into() });
    }

    pub(crate) fn write_if_changed(
        &mut self,
        path: impl Into<PathBuf>,
        contents: impl Into<Vec<u8>>,
        event: OperationEvent,
    ) {
        self.steps.push(MutationStep::WriteFile {
            path: path.into(),
            contents: contents.into(),
            event: WriteEvent::IfChanged(event),
        });
    }

    pub(crate) fn write_always(
        &mut self,
        path: impl Into<PathBuf>,
        contents: impl Into<Vec<u8>>,
        event: OperationEvent,
    ) {
        self.steps.push(MutationStep::WriteFile {
            path: path.into(),
            contents: contents.into(),
            event: WriteEvent::Always(event),
        });
    }

    pub(crate) fn write_silent(&mut self, path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) {
        self.steps.push(MutationStep::WriteFile {
            path: path.into(),
            contents: contents.into(),
            event: WriteEvent::Silent,
        });
    }

    pub(crate) fn move_file(
        &mut self,
        from: impl Into<PathBuf>,
        to: impl Into<PathBuf>,
        event: OperationEvent,
    ) {
        self.steps.push(MutationStep::MoveFile {
            from: from.into(),
            to: to.into(),
            event,
        });
    }

    pub(crate) fn remove_file(&mut self, path: impl Into<PathBuf>, event: OperationEvent) {
        self.steps.push(MutationStep::RemoveFile {
            path: path.into(),
            event,
        });
    }

    pub(crate) fn remove_dir(
        &mut self,
        path: impl Into<PathBuf>,
        recursive: bool,
        event: OperationEvent,
    ) {
        self.steps.push(MutationStep::RemoveDir {
            path: path.into(),
            recursive,
            event,
        });
    }

    pub(crate) fn apply(self, root: &Path) -> Result<OperationReport> {
        if self.steps.is_empty() {
            return Ok(OperationReport::new());
        }

        let _lock = ProjectLock::acquire(root)?;
        self.apply_unlocked()
    }

    fn apply_unlocked(self) -> Result<OperationReport> {
        let mut report = OperationReport::new();

        for step in self.steps {
            match step {
                MutationStep::Event(event) => report.push(event),
                MutationStep::CreateDir { path, event } => {
                    fs::create_dir(&path)?;
                    report.push(event);
                }
                MutationStep::EnsureDir { path } => {
                    fs::create_dir_all(path)?;
                }
                MutationStep::WriteFile {
                    path,
                    contents,
                    event,
                } => {
                    let changed = atomic_write(&path, contents)?;
                    match event {
                        WriteEvent::Silent => {}
                        WriteEvent::Always(event) => report.push(event),
                        WriteEvent::IfChanged(event) if changed => report.push(event),
                        WriteEvent::IfChanged(_) => {}
                    }
                }
                MutationStep::MoveFile { from, to, event } => {
                    fs::rename(from, to)?;
                    report.push(event);
                }
                MutationStep::RemoveFile { path, event } => {
                    fs::remove_file(path)?;
                    report.push(event);
                }
                MutationStep::RemoveDir {
                    path,
                    recursive,
                    event,
                } => {
                    if recursive {
                        fs::remove_dir_all(path)?;
                    } else {
                        fs::remove_dir(path)?;
                    }
                    report.push(event);
                }
            }
        }

        Ok(report)
    }
}

struct ProjectLock {
    path: PathBuf,
}

impl ProjectLock {
    fn acquire(root: &Path) -> Result<Self> {
        let path = root.join(WORKSPACE_DIR).join(MUTATION_LOCK_FILE);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    FractalError::project_locked(format!(
                        "project is locked for mutation: {}",
                        path.display()
                    ))
                } else {
                    error.into()
                }
            })?;

        writeln!(file, "pid={}", std::process::id())?;
        file.sync_all()?;

        Ok(Self { path })
    }
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug)]
enum MutationStep {
    Event(OperationEvent),
    CreateDir {
        path: PathBuf,
        event: OperationEvent,
    },
    EnsureDir {
        path: PathBuf,
    },
    WriteFile {
        path: PathBuf,
        contents: Vec<u8>,
        event: WriteEvent,
    },
    MoveFile {
        from: PathBuf,
        to: PathBuf,
        event: OperationEvent,
    },
    RemoveFile {
        path: PathBuf,
        event: OperationEvent,
    },
    RemoveDir {
        path: PathBuf,
        recursive: bool,
        event: OperationEvent,
    },
}

#[derive(Debug)]
enum WriteEvent {
    Silent,
    Always(OperationEvent),
    IfChanged(OperationEvent),
}
