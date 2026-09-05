use super::support::*;
use super::*;

impl Project {
    /// Inspects project health without changing project files.
    ///
    /// Unlike [`Project::open`], inspection reports recovery state, proposed
    /// format repairs, and validation failures as data whenever possible.
    pub fn inspect(path: impl AsRef<Path>) -> Result<ProjectInspection> {
        let root = path.as_ref().to_path_buf();
        let manifest_path = root.join(MANIFEST);
        if !manifest_path.is_file() && !root.join(LOCK).is_file() {
            return Ok(ProjectInspection {
                openable: false,
                healthy: false,
                recovery: vec![],
                proposed_repairs: vec![],
                validation: None,
                issues: vec![HealthIssue {
                    code: HealthIssueCode::InvalidProject,
                    path: None,
                    message: format!("missing {}", manifest_path.display()),
                }],
            });
        }

        let _lock = ProjectLock::shared(&root)?;
        let recovery = inspect_recovery_transactions(&root)?;
        let mut issues = Vec::new();
        let recovery_blocks_open = recovery.iter().any(|transaction| match transaction.status {
            RecoveryTransactionStatus::Pending => {
                issues.push(HealthIssue {
                    code: HealthIssueCode::RecoveryRequired,
                    path: Some(transaction.path.clone()),
                    message: "an interrupted transaction must be recovered before opening".into(),
                });
                true
            }
            RecoveryTransactionStatus::Malformed => {
                issues.push(HealthIssue {
                    code: HealthIssueCode::RecoveryStateMalformed,
                    path: Some(transaction.path.clone()),
                    message: transaction
                        .message
                        .clone()
                        .unwrap_or_else(|| "transaction recovery state is malformed".into()),
                });
                true
            }
            RecoveryTransactionStatus::CommittedCleanupPending => {
                issues.push(HealthIssue {
                    code: HealthIssueCode::CleanupPending,
                    path: Some(transaction.path.clone()),
                    message: "a committed transaction directory still needs cleanup".into(),
                });
                false
            }
        });
        if recovery_blocks_open {
            return Ok(ProjectInspection {
                openable: false,
                healthy: false,
                recovery,
                proposed_repairs: vec![],
                validation: None,
                issues,
            });
        }

        if !manifest_path.is_file() {
            issues.push(HealthIssue {
                code: HealthIssueCode::InvalidProject,
                path: None,
                message: format!("missing {}", manifest_path.display()),
            });
            return Ok(ProjectInspection {
                openable: false,
                healthy: false,
                recovery,
                proposed_repairs: vec![],
                validation: None,
                issues,
            });
        }

        let project = match Self::load(root) {
            Ok(project) => project,
            Err(error) => {
                issues.push(HealthIssue {
                    code: if error.code == crate::FractalErrorCode::UnsupportedVersion {
                        HealthIssueCode::UnsupportedVersion
                    } else {
                        HealthIssueCode::InvalidProject
                    },
                    path: None,
                    message: error.message,
                });
                return Ok(ProjectInspection {
                    openable: false,
                    healthy: false,
                    recovery,
                    proposed_repairs: vec![],
                    validation: None,
                    issues,
                });
            }
        };
        let proposed_repairs = match project.proposed_repairs() {
            Ok(repairs) => repairs,
            Err(error) => {
                issues.push(HealthIssue {
                    code: HealthIssueCode::InvalidProject,
                    path: None,
                    message: error.message,
                });
                vec![]
            }
        };
        for repair in &proposed_repairs {
            let path = match repair {
                ProposedRepair::MovePath { from, .. } => from.clone(),
                ProposedRepair::AppendFolderOrder { metadata, .. } => metadata.clone(),
            };
            issues.push(HealthIssue {
                code: HealthIssueCode::RepairRequired,
                path: Some(path),
                message: "the project has a pending format repair".into(),
            });
        }
        let validation = project.validate();
        if !validation.valid {
            issues.push(HealthIssue {
                code: HealthIssueCode::ValidationFailed,
                path: None,
                message: format!(
                    "project validation found {} issue(s)",
                    validation.issues.len()
                ),
            });
        }
        let healthy = issues.is_empty();
        Ok(ProjectInspection {
            openable: true,
            healthy,
            recovery,
            proposed_repairs,
            validation: Some(validation),
            issues,
        })
    }

    /// Validates the loaded manifest, folder metadata, native documents, and
    /// links without changing project files.
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
        }
        ValidationReport {
            valid: issues.is_empty(),
            issues,
        }
    }

    fn proposed_repairs(&self) -> Result<Vec<ProposedRepair>> {
        let mut repairs = Vec::new();
        let mut moved_paths = Vec::new();
        for stored in self.folders.values() {
            let path = apply_path_moves(PathBuf::from(&stored.folder.path), &moved_paths);
            if let Some(parent) = path.parent() {
                let desired = parent.join(slug(&stored.folder.title)?);
                if desired != path {
                    repairs.push(ProposedRepair::MovePath {
                        from: public_project_path(&Path::new(PAGES).join(&path))?,
                        to: public_project_path(&Path::new(PAGES).join(&desired))?,
                        entry: ProjectEntryKind::Directory,
                    });
                    moved_paths.push((path, desired));
                }
            }
        }
        for stored in self.pages.values() {
            let Some(title) = stored.page.title.as_deref() else {
                continue;
            };
            let path = apply_path_moves(PathBuf::from(&stored.page.path), &moved_paths);
            let desired = path.with_file_name(format!("{}{}", slug(title)?, NATIVE_SUFFIX));
            if desired != path {
                repairs.push(ProposedRepair::MovePath {
                    from: public_project_path(&Path::new(PAGES).join(&path))?,
                    to: public_project_path(&Path::new(PAGES).join(&desired))?,
                    entry: ProjectEntryKind::File,
                });
                moved_paths.push((path, desired));
            }
        }
        for stored in self.folders.values() {
            let path = apply_path_moves(PathBuf::from(&stored.folder.path), &moved_paths);
            if let Some(order) = &stored.folder.order {
                let known: BTreeSet<String> = order.iter().cloned().collect();
                let additions: Vec<String> = stored
                    .folder
                    .children
                    .iter()
                    .map(|child| {
                        apply_path_moves(
                            Path::new(&stored.folder.path).join(&child.name),
                            &moved_paths,
                        )
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| child.name.clone())
                    })
                    .filter(|name| !known.contains(name))
                    .collect();
                if !additions.is_empty() {
                    repairs.push(ProposedRepair::AppendFolderOrder {
                        metadata: public_project_path(
                            &Path::new(PAGES).join(folder_metadata_relative_path(&path)),
                        )?,
                        additions,
                    });
                }
            }
        }
        Ok(repairs)
    }
}

fn apply_path_moves(mut path: PathBuf, moves: &[(PathBuf, PathBuf)]) -> PathBuf {
    for (from, to) in moves {
        if let Ok(suffix) = path.strip_prefix(from) {
            path = to.join(suffix);
        }
    }
    path
}
