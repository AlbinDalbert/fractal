use super::support::*;
use super::*;

impl Project {
    /// Returns resolved and broken native-document links in document order.
    ///
    /// External URLs, fragments, mail links, and local non-native targets are
    /// not part of the native link index.
    pub fn links(&self, path: impl AsRef<Path>) -> Result<Vec<Link>> {
        Ok(self.stored(path.as_ref())?.page.links.clone())
    }

    /// Returns native links that resolve to `path`.
    pub fn backlinks(&self, path: impl AsRef<Path>) -> Result<Vec<Backlink>> {
        let target = path_string(&self.existing_path(path.as_ref())?);
        let mut backlinks = Vec::new();
        for page in self.pages.values() {
            for link in &page.page.links {
                if matches!(&link.target, LinkTarget::Resolved(value) if value == &target) {
                    backlinks.push(Backlink {
                        page: page.page.path.clone(),
                        text: link.text.clone(),
                    });
                }
            }
        }
        Ok(backlinks)
    }

    /// Inserts a link around the first matching unlinked text in a native page.
    ///
    /// The source and target must be different existing pages. The operation
    /// fails if `text` does not occur outside an existing link.
    pub fn insert_link(
        &mut self,
        page: impl AsRef<Path>,
        text: &str,
        target: impl AsRef<Path>,
    ) -> Result<MutationReceipt> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
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
        let mut plan = MutationPlan::new(MutationKind::InsertLink);
        plan.write_page(page, document.serialize()?);
        let receipt = plan.commit(&self.root)?;
        self.finish_mutation(receipt)
    }
}
