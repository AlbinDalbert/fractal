use crate::project::document::PageDocument;
use crate::project::html::escape_html;
use crate::project::index::build_index;
use crate::project::paths::resolve_existing_page;
use crate::project::types::{OperationEvent, OperationReport};
use crate::Result;
use kuchiki::NodeRef;
use std::fs;
use std::path::Path;

pub fn add_note(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    trigger: &str,
    content: &str,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let note_id = note_id_from_trigger(trigger)?;
    let html = fs::read_to_string(&page)?;

    let document = PageDocument::parse(&html);
    if document.note_node(&note_id).is_some() {
        return Err(format!("note already exists: {note_id}").into());
    }

    let html = insert_note_into_document(&html, &render_note_aside(&note_id, content))?;
    fs::write(&page, html)?;
    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::AddedNote { page, note_id });
    report.extend(generated);
    Ok(report)
}

pub fn remove_note(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    trigger: &str,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let note_id = note_id_from_trigger(trigger)?;
    let html = fs::read_to_string(&page)?;
    let html = remove_note_from_document(&html, &note_id)?;
    fs::write(&page, html)?;
    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::RemovedNote { page, note_id });
    report.extend(generated);
    Ok(report)
}

pub fn patch_note(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    trigger: &str,
    content: &str,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let note_id = note_id_from_trigger(trigger)?;
    let html = fs::read_to_string(&page)?;
    let html = patch_note_in_document(&html, &note_id, content)?;
    fs::write(&page, html)?;
    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::PatchedNote { page, note_id });
    report.extend(generated);
    Ok(report)
}

pub(super) fn note_id_from_trigger(trigger: &str) -> Result<String> {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for character in trigger
        .chars()
        .flat_map(|character| character.to_lowercase())
    {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_was_dash = false;
            continue;
        }

        if (character.is_whitespace() || character == '-' || character == '_')
            && !previous_was_dash
            && !slug.is_empty()
        {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        return Err("trigger must contain at least one letter or number".into());
    }

    Ok(format!("note-{slug}"))
}

pub(super) fn is_valid_note_id(note_id: &str) -> bool {
    let Some(slug) = note_id.strip_prefix("note-") else {
        return false;
    };

    !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

pub(super) fn insert_note_into_document(html: &str, note: &str) -> Result<String> {
    let document = PageDocument::parse(html);
    let section = document.single_notes_section()?;
    let note = parse_note_aside(note)?;

    section.append(NodeRef::new_text("\n    "));
    section.append(note);
    section.append(NodeRef::new_text("\n  "));

    document.to_html()
}

fn remove_note_from_document(html: &str, note_id: &str) -> Result<String> {
    let document = PageDocument::parse(html);
    document.single_notes_section()?;
    let note = document
        .note_node(note_id)
        .ok_or_else(|| format!("note does not exist: {note_id}"))?;
    note.detach();

    document.to_html()
}

fn patch_note_in_document(html: &str, note_id: &str, content: &str) -> Result<String> {
    let document = PageDocument::parse(html);
    document.single_notes_section()?;
    let note = document
        .note_node(note_id)
        .ok_or_else(|| format!("note does not exist: {note_id}"))?;
    let replacement = parse_note_aside(&render_note_aside(note_id, content))?;

    note.insert_before(replacement);
    note.detach();

    document.to_html()
}

fn parse_note_aside(note: &str) -> Result<NodeRef> {
    let document = PageDocument::parse(note);
    let aside = document
        .document
        .select_first("aside[data-fractal-note]")
        .map_err(|_| "note markup must contain a data-fractal-note aside")?
        .as_node()
        .clone();
    aside.detach();
    Ok(aside)
}

pub(super) fn render_note_aside(note_id: &str, content: &str) -> String {
    format!(
        "    <aside id=\"{note_id}\" data-fractal-note>\n      <p>{}</p>\n    </aside>\n",
        escape_html(content)
    )
}
