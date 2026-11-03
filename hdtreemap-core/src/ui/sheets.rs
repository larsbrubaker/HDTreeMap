//! The app's two modal sheets (agg-gui `ModalSheet`s, placed late in the root `Stack` so
//! they cover everything while open):
//!
//! - the **removal** confirmation, for Move to Trash and Delete Permanently: the item's
//!   name, size and item count, and what will happen ("This moves it to the Trash/Recycle
//!   Bin. You can restore it from there." or, for a delete, that it skips the trash and
//!   cannot be undone). Cancel (or Escape) closes it; "Move to Trash" / "Delete
//!   Permanently" (or Enter) runs `NodeActions::confirm_removal`. Its content rebuilds for
//!   each request (`AppState::sheet_version`), so it is worded for the kind asked for.
//! - **About HDTreeMap**: version, licence and a link to the project on GitHub.

use std::rc::Rc;
use std::sync::Arc;

use agg_gui::{
    Button, FlexColumn, FlexRow, HAnchor, Hyperlink, Insets, Label, ModalSheet, Rebuilder, Size,
    Spacer, VAnchor, Widget,
};

use super::commands::{Commands, GITHUB_URL};
use crate::fonts::{fa, Fonts};
use crate::format::{format_bytes, format_count};
use crate::model::NodeId;
use crate::platform::ActionOutcome;
use crate::state::{AppState, Removal};
use crate::APP_TITLE;

/// Widget id of the removal confirmation's content.
pub const REMOVE_SHEET_ID: &str = "remove-sheet";
/// Widget id of the removal confirmation's title (the question).
pub const REMOVE_TITLE_ID: &str = "remove-title";
/// Widget id of the removal confirmation's "Move to Trash" / "Delete Permanently" button.
pub const REMOVE_CONFIRM_ID: &str = "remove-confirm";
/// Widget id of the removal confirmation's Cancel button.
pub const REMOVE_CANCEL_ID: &str = "remove-cancel";
/// Widget id of the removal confirmation's description of the item.
pub const REMOVE_DETAILS_ID: &str = "remove-details";
/// Widget id of the removal confirmation's explanation of what will happen.
pub const REMOVE_EXPLANATION_ID: &str = "remove-explanation";
/// Widget id of the About sheet's content.
pub const ABOUT_SHEET_ID: &str = "about-sheet";
/// Widget id of the About sheet's Close button.
pub const ABOUT_CLOSE_ID: &str = "about-close";

/// What the confirmation says Move to Trash will do.
pub const TRASH_EXPLANATION: &str =
    "This moves it to the Trash/Recycle Bin. You can restore it from there.";
/// What the confirmation says Delete Permanently will do.
pub const DELETE_EXPLANATION: &str =
    "This deletes it from the disk without using the Trash/Recycle Bin. It cannot be undone.";

/// The removal confirmation sheet (Move to Trash or Delete Permanently).
pub fn removal_sheet(commands: &Commands, fonts: &Fonts) -> Box<dyn Widget> {
    let state = commands.state().clone();
    let version_state = state.clone();
    let build_commands = commands.clone();
    let build_fonts = fonts.clone();
    let content = Rebuilder::new(
        move || version_state.sheet_version(),
        move || removal_content(&build_commands, &build_fonts),
    );
    let cancel_state = state.clone();
    Box::new(
        ModalSheet::new(state.removal_sheet(), Box::new(content))
            .with_panel_size(Size::new(460.0, 250.0))
            .with_on_close(move || cancel_state.cancel_removal()),
    )
}

/// `"1 file"`, or `"1,234 files in 56 folders"` for a folder.
fn item_count(state: &AppState, node: NodeId) -> String {
    state
        .with_result(|r, _| {
            let tree = &r.tree;
            let Some(n) = tree.node(node) else {
                return String::new();
            };
            if !n.is_dir() {
                return "1 file".to_string();
            }
            let mut folders = 0u64;
            let mut stack: Vec<NodeId> = tree.children(node).collect();
            while let Some(id) = stack.pop() {
                if tree.node(id).is_some_and(|c| c.is_dir()) {
                    folders += 1;
                    stack.extend(tree.children(id));
                }
            }
            let files = u64::from(n.file_count());
            let files = if files == 1 {
                "1 file".to_string()
            } else {
                format!("{} files", format_count(files))
            };
            match folders {
                0 => files,
                1 => format!("{files} in 1 folder"),
                n => format!("{files} in {} folders", format_count(n)),
            }
        })
        .unwrap_or_default()
}

fn removal_content(commands: &Commands, fonts: &Fonts) -> Box<dyn Widget> {
    let state = commands.state().clone();
    let request = state.removal_request();
    let node = request.map(|(node, _)| node);
    let how = request.map_or(Removal::Trash, |(_, how)| how);
    let (name, size) = node
        .and_then(|n| state.with_result(|r, _| (r.tree.name(n).to_string(), r.tree.size(n))))
        .unwrap_or_default();
    let details = node.map_or_else(String::new, |n| {
        format!("{} \u{b7} {}", format_bytes(size), item_count(&state, n))
    });

    let cancel_state = state.clone();
    let cancel = Button::new("Cancel", Arc::clone(&fonts.ui))
        .with_cancel_action()
        .on_click(move || cancel_state.cancel_removal())
        .with_name(REMOVE_CANCEL_ID);
    let (icon, title, button, explanation) = match how {
        Removal::Trash => (
            fa::TRASH,
            format!("Move \u{201c}{name}\u{201d} to the Trash?"),
            "Move to Trash",
            TRASH_EXPLANATION,
        ),
        Removal::Delete => (
            fa::TIMES_CIRCLE,
            format!("Delete \u{201c}{name}\u{201d} permanently?"),
            "Delete Permanently",
            DELETE_EXPLANATION,
        ),
    };
    let confirm_commands = commands.clone();
    let confirm = Button::new(button, Arc::clone(&fonts.ui))
        .with_icon(icon, Arc::clone(&fonts.icons))
        .with_default_action()
        .on_click(move || confirm_commands.nodes().confirm_removal())
        .with_name(REMOVE_CONFIRM_ID);
    let mut column = FlexColumn::new()
        .with_gap(10.0)
        .with_inner_padding(Insets::all(20.0))
        .with_top_anchor(true)
        .add(Box::new(
            Label::new(format!("{icon}  {title}"), Arc::clone(&fonts.ui))
                .with_font_size(16.0)
                .with_strong(true)
                .with_name(REMOVE_TITLE_ID),
        ))
        .add(Box::new(
            Label::new(details, Arc::clone(&fonts.ui))
                .with_font_size(14.0)
                .with_name(REMOVE_DETAILS_ID),
        ))
        .add(Box::new(
            Label::new(explanation, Arc::clone(&fonts.ui))
                .with_font_size(13.0)
                .with_wrap(true)
                .with_h_anchor(HAnchor::STRETCH)
                .with_name(REMOVE_EXPLANATION_ID),
        ));
    if commands.platform().is_demo() {
        column = column.add(Box::new(
            Label::new(
                "Browser demo: nothing on your computer is deleted.",
                Arc::clone(&fonts.ui),
            )
            .with_font_size(13.0)
            .with_dim(true),
        ));
    }
    let buttons = FlexRow::new()
        .with_gap(10.0)
        .with_h_anchor(HAnchor::STRETCH)
        .add_flex(
            Box::new(Spacer::new().with_max_size(Size::new(f64::MAX, 0.0))),
            1.0,
        )
        .add(Box::new(cancel))
        .add(Box::new(confirm));
    Box::new(
        column
            .add_flex(Box::new(Spacer::new()), 1.0)
            .add(Box::new(buttons))
            .with_name(REMOVE_SHEET_ID),
    )
}

/// The About sheet.
pub fn about_sheet(commands: &Commands, fonts: &Fonts) -> Box<dyn Widget> {
    let state = commands.state().clone();
    let visible = state.about_sheet();
    let close_visible = Rc::clone(&visible);
    let close = Button::new("Close", Arc::clone(&fonts.ui))
        .with_default_action()
        .with_cancel_action()
        .on_click(move || close_visible.set(false))
        .with_name(ABOUT_CLOSE_ID);
    let link_commands = commands.clone();
    let link = Hyperlink::new("github.com/larsbrubaker/HDTreeMap", Arc::clone(&fonts.ui)).on_click(
        move || {
            let state = link_commands.state();
            match link_commands.platform().open_url(GITHUB_URL) {
                Ok(ActionOutcome::Done) => {}
                Ok(ActionOutcome::Simulated(note)) => state.notify_info(note),
                Err(err) => state.notify_error(err.message),
            }
        },
    );
    let content = FlexColumn::new()
        .with_gap(8.0)
        .with_inner_padding(Insets::all(20.0))
        .with_top_anchor(true)
        .add(Box::new(
            Label::new(
                format!("{}  {APP_TITLE}", fa::TH_LARGE),
                Arc::clone(&fonts.ui),
            )
            .with_font_size(22.0)
            .with_strong(true),
        ))
        .add(Box::new(
            Label::new(
                format!("Version {}", env!("CARGO_PKG_VERSION")),
                Arc::clone(&fonts.ui),
            )
            .with_font_size(14.0),
        ))
        .add(Box::new(
            Label::new(
                "A small, fast disk-usage treemap viewer, built on agg-gui.",
                Arc::clone(&fonts.ui),
            )
            .with_font_size(13.0)
            .with_wrap(true)
            .with_h_anchor(HAnchor::STRETCH),
        ))
        .add(Box::new(
            Label::new(
                "MIT License \u{b7} \u{a9} Lars Brubaker",
                Arc::clone(&fonts.ui),
            )
            .with_font_size(13.0)
            .with_dim(true),
        ))
        .add(Box::new(link))
        .add_flex(Box::new(Spacer::new()), 1.0)
        .add(Box::new(
            FlexRow::new()
                .with_h_anchor(HAnchor::STRETCH)
                .add_flex(
                    Box::new(Spacer::new().with_max_size(Size::new(f64::MAX, 0.0))),
                    1.0,
                )
                .add(Box::new(close.with_v_anchor(VAnchor::CENTER))),
        ))
        .with_name(ABOUT_SHEET_ID);
    Box::new(ModalSheet::new(visible, Box::new(content)).with_panel_size(Size::new(420.0, 250.0)))
}
