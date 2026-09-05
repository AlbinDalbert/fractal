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
const LOCK: &str = ".fractal.lock";
const PAGES: &str = "pages";
const MIN_SUPPORTED_VERSION: u32 = 1;
const VERSION: u32 = 2;
const NATIVE_SUFFIX: &str = ".fractal.html";
const TRANSACTION_PREFIX: &str = ".fractal-transaction-";
const DEFAULT_STYLE: &str = r#"
    :root { color-scheme: dark; }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: #0c0c0a;
      color: #e8e1d5;
      font: 1.125rem/1.65 ui-sans-serif, system-ui, sans-serif;
    }
    main {
      width: min(100% - 2rem, 45rem);
      margin: 0 auto;
      padding: clamp(4rem, 12vh, 8rem) 0;
    }
    h1 {
      margin: 0 0 2.5rem;
      font-size: clamp(2.75rem, 8vw, 4rem);
      line-height: 1;
      letter-spacing: -0.04em;
    }
    h2, h3, h4, h5, h6 { line-height: 1.2; }
    p, ul, ol, blockquote, pre, figure, table { margin: 1.25rem 0; }
    a { color: #e8bb4d; text-underline-offset: 0.18em; }
    img, iframe { max-width: 100%; }
    code, pre { font-family: ui-monospace, monospace; }
  "#;

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

mod export;
mod folder;
mod graph;
mod lifecycle;
mod page;
mod storage;
mod support;
mod validation;

#[cfg(test)]
pub(crate) use support::{inject_transaction_fault, TransactionFaultPoint};
