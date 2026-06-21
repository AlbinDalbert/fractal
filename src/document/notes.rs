use crate::document::html::escape_html;
use crate::document::PageDocument;
use crate::index::build_index;
use crate::ops::mutation::MutationPlan;
use crate::project::paths::resolve_existing_page;
use crate::types::{OperationEvent, OperationReport};
use crate::{FractalError, Result};
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
        return Err(FractalError::already_exists(format!(
            "note already exists: {note_id}"
        )));
    }

    let html = insert_note_into_document(&html, &render_note_aside(&note_id, content))?;
    let mut plan = MutationPlan::new();
    plan.write_always(
        page.clone(),
        html.into_bytes(),
        OperationEvent::NoteAdded { page, note_id },
    );
    let mut report = plan.apply(root)?;
    report.extend(build_index(root)?);
    Ok(report.relative_to(root))
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
    let mut plan = MutationPlan::new();
    plan.write_always(
        page.clone(),
        html.into_bytes(),
        OperationEvent::NoteRemoved { page, note_id },
    );
    let mut report = plan.apply(root)?;
    report.extend(build_index(root)?);
    Ok(report.relative_to(root))
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
    let mut plan = MutationPlan::new();
    plan.write_always(
        page.clone(),
        html.into_bytes(),
        OperationEvent::NoteUpdated { page, note_id },
    );
    let mut report = plan.apply(root)?;
    report.extend(build_index(root)?);
    Ok(report.relative_to(root))
}

pub(crate) fn note_id_from_trigger(trigger: &str) -> Result<String> {
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
        return Err(FractalError::invalid_input(
            "trigger must contain at least one letter or number",
        ));
    }

    Ok(format!("note-{slug}"))
}

pub(crate) fn is_valid_note_id(note_id: &str) -> bool {
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

pub(crate) fn insert_note_into_document(html: &str, note: &str) -> Result<String> {
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
        .ok_or_else(|| FractalError::not_found(format!("note does not exist: {note_id}")))?;
    note.detach();

    document.to_html()
}

fn patch_note_in_document(html: &str, note_id: &str, content: &str) -> Result<String> {
    let document = PageDocument::parse(html);
    document.single_notes_section()?;
    let note = document
        .note_node(note_id)
        .ok_or_else(|| FractalError::not_found(format!("note does not exist: {note_id}")))?;
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
        .map_err(|_| {
            FractalError::invalid_input("note markup must contain a data-fractal-note aside")
        })?
        .as_node()
        .clone();
    aside.detach();
    Ok(aside)
}

pub(crate) fn render_note_aside(note_id: &str, content: &str) -> String {
    format!(
        "    <aside id=\"{note_id}\" data-fractal-note>\n      <p>{}</p>\n    </aside>\n",
        escape_html(content)
    )
}
