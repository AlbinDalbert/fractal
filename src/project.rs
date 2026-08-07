use crate::document::{is_external_href, relative_href, resolve_internal_href, Document};
use crate::types::*;
use crate::{FractalError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MANIFEST: &str = "fractal.json";
const PAGES: &str = "pages";
const VERSION: u32 = 1;

#[derive(Debug)]
pub struct Project {
    root: PathBuf,
    manifest: ProjectManifest,
    pages: BTreeMap<String, StoredPage>,
}

#[derive(Debug, Clone)]
struct StoredPage {
    page: Page,
    html: String,
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
        let manifest: ProjectManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
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

    pub fn page(&self, path: impl AsRef<Path>) -> Result<Page> {
        Ok(self.stored(path.as_ref())?.page.clone())
    }

    pub fn source(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(self.stored(path.as_ref())?.html.clone())
    }

    pub fn create_page(&mut self, title: &str) -> Result<Mutation> {
        let stem = slug(title)?;
        self.create_page_at(format!("{stem}.html"), title)
    }

    pub fn create_page_at(&mut self, path: impl AsRef<Path>, title: &str) -> Result<Mutation> {
        if title.trim().is_empty() {
            return Err(FractalError::invalid_input("title cannot be empty"));
        }
        let relative = normalize_page_path(path.as_ref())?;
        let destination = self.root.join(PAGES).join(&relative);
        if destination.exists() {
            return Err(FractalError::already_exists(format!(
                "page already exists: {}",
                relative.display()
            )));
        }
        let title = title.trim();
        let html = format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>{}</title>\n</head>\n<body>\n  <main>\n    <h1>{}</h1>\n  </main>\n</body>\n</html>\n",
            escape_html(title),
            escape_html(title)
        );
        atomic_write(&destination, &html)?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![relative],
            deleted: vec![],
        })
    }

    pub fn write_page(&mut self, path: impl AsRef<Path>, html: &str) -> Result<Mutation> {
        let relative = self.existing_path(path.as_ref())?;
        let document = Document::parse(html);
        if document.title().is_none() {
            return Err(FractalError::invalid_input(
                "page must have a non-empty <title> or <h1>",
            ));
        }
        atomic_write(&self.root.join(PAGES).join(&relative), html)?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![relative],
            deleted: vec![],
        })
    }

    pub fn move_page(&mut self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<Mutation> {
        let from = self.existing_path(from.as_ref())?;
        let to = normalize_page_path(to.as_ref())?;
        if from == to {
            return Ok(Mutation {
                changed: vec![],
                deleted: vec![],
            });
        }
        let destination = self.root.join(PAGES).join(&to);
        if destination.exists() {
            return Err(FractalError::already_exists(format!(
                "page already exists: {}",
                to.display()
            )));
        }

        let from_string = path_string(&from);
        let to_string = path_string(&to);
        let moved_document = Document::parse(&self.stored(&from)?.html);
        moved_document.rewrite_source_location(&from_string, &to_string);
        let moved_html = moved_document.serialize()?;

        let mut rewrites = Vec::new();
        for (path, stored) in &self.pages {
            if path == &from_string {
                continue;
            }
            let document = Document::parse(&stored.html);
            if document.rewrite_internal_target(path, &from_string, &to_string) > 0 {
                rewrites.push((path.clone(), document.serialize()?));
            }
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(self.root.join(PAGES).join(&from), &destination)?;
        atomic_write(&destination, &moved_html)?;
        let mut changed = vec![to.clone()];
        for (path, html) in rewrites {
            atomic_write(&self.root.join(PAGES).join(&path), &html)?;
            changed.push(PathBuf::from(path));
        }
        self.reload()?;
        Ok(Mutation {
            changed,
            deleted: vec![from],
        })
    }

    pub fn delete_page(&mut self, path: impl AsRef<Path>) -> Result<Mutation> {
        let relative = self.existing_path(path.as_ref())?;
        let backlinks = self.backlinks(&relative)?;
        if !backlinks.is_empty() {
            return Err(FractalError::invalid_input(format!(
                "cannot delete {} while {} explicit link(s) target it",
                relative.display(),
                backlinks.len()
            )));
        }
        fs::remove_file(self.root.join(PAGES).join(&relative))?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![],
            deleted: vec![relative],
        })
    }

    pub fn links(&self, path: impl AsRef<Path>) -> Result<Vec<Link>> {
        Ok(self.stored(path.as_ref())?.page.links.clone())
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

    pub fn suggest_links(&self, path: impl AsRef<Path>) -> Result<Vec<LinkSuggestion>> {
        let source = self.stored(path.as_ref())?;
        let text = Document::parse(&source.html).unlinked_text();
        let text_lower = text.to_lowercase();
        let existing: BTreeSet<_> = source
            .page
            .links
            .iter()
            .filter_map(|link| match &link.target {
                LinkTarget::Internal(path) => Some(path.clone()),
                _ => None,
            })
            .collect();
        let words: BTreeSet<String> = text
            .split(|character: char| !character.is_alphanumeric())
            .filter(|word| word.chars().count() >= 3)
            .map(str::to_lowercase)
            .collect();
        let mut grouped: BTreeMap<String, (String, Vec<LinkCandidate>)> = BTreeMap::new();

        for target in self.pages.values() {
            if target.page.path == source.page.path || existing.contains(&target.page.path) {
                continue;
            }
            let Some(title) = target.page.title.as_deref() else {
                continue;
            };
            let title_lower = title.to_lowercase();
            let stem = Path::new(&target.page.path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .replace(['-', '_'], " ");
            let stem_lower = stem.to_lowercase();

            let (mention, match_kind, score) = if contains_phrase(&text_lower, &title_lower) {
                (title.to_string(), MatchKind::ExactTitle, 100)
            } else if stem_lower != title_lower && contains_phrase(&text_lower, &stem_lower) {
                (stem, MatchKind::ExactStem, 95)
            } else {
                let title_words: Vec<_> = title
                    .split(|character: char| !character.is_alphanumeric())
                    .filter(|word| word.chars().count() >= 3)
                    .collect();
                let Some(matched) = title_words
                    .iter()
                    .find(|word| words.contains(&word.to_lowercase()))
                else {
                    continue;
                };
                let overlap = title_words
                    .iter()
                    .filter(|word| words.contains(&word.to_lowercase()))
                    .count();
                let score = 60 + ((overlap * 20) / title_words.len().max(1)) as u8;
                ((*matched).to_string(), MatchKind::TokenOverlap, score)
            };
            let key = mention.to_lowercase();
            grouped
                .entry(key)
                .or_insert_with(|| (mention, vec![]))
                .1
                .push(LinkCandidate {
                    page: target.page.path.clone(),
                    title: title.to_string(),
                    match_kind,
                    score,
                });
        }

        let mut suggestions: Vec<_> = grouped
            .into_values()
            .map(|(text, mut candidates)| {
                candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.page.cmp(&b.page)));
                LinkSuggestion { text, candidates }
            })
            .collect();
        suggestions.sort_by_key(|suggestion| suggestion.text.to_lowercase());
        Ok(suggestions)
    }

    pub fn insert_link(
        &mut self,
        page: impl AsRef<Path>,
        text: &str,
        target: impl AsRef<Path>,
    ) -> Result<Mutation> {
        let page = self.existing_path(page.as_ref())?;
        let target = self.existing_path(target.as_ref())?;
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
        for stored in self.pages.values() {
            if stored.page.title.is_none() {
                issues.push(ValidationIssue {
                    path: Some(stored.page.path.clone()),
                    message: "page needs a non-empty <title> or <h1>".into(),
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

    fn stored(&self, path: &Path) -> Result<&StoredPage> {
        let path = path_string(&normalize_page_path(path)?);
        self.pages
            .get(&path)
            .ok_or_else(|| FractalError::not_found(format!("page does not exist: {path}")))
    }

    fn existing_path(&self, path: &Path) -> Result<PathBuf> {
        let normalized = normalize_page_path(path)?;
        let key = path_string(&normalized);
        self.pages
            .contains_key(&key)
            .then_some(normalized)
            .ok_or_else(|| FractalError::not_found(format!("page does not exist: {key}")))
    }

    fn reload(&mut self) -> Result<()> {
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
            let page = Page {
                path: path.clone(),
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

fn normalize_page_path(path: &Path) -> Result<PathBuf> {
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
    if output.extension().is_none() {
        output.set_extension("html");
    }
    if output.extension().and_then(|extension| extension.to_str()) != Some("html") {
        return Err(FractalError::invalid_input("page path must end in .html"));
    }
    Ok(output)
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

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("fractal-tmp");
    fs::write(&temp, contents)?;
    fs::rename(temp, path)?;
    Ok(())
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

fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    if phrase.is_empty() {
        return false;
    }
    haystack.match_indices(phrase).any(|(start, value)| {
        let end = start + value.len();
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        before.is_none_or(|c| !c.is_alphanumeric()) && after.is_none_or(|c| !c.is_alphanumeric())
    })
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
