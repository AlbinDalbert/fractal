use super::support::*;
use super::*;

impl Project {
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
}
