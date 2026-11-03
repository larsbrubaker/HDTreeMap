//! Headless UI tests of Delete Permanently, the delete that skips the Trash (WinDirStat's
//! "Delete (no way to undelete!)"): it sits after Move to Trash in the right-click menu of
//! the map and the folder tree and in the Edit menu (Shift+Delete, or Option+Cmd+Backspace
//! on a Mac); it asks first in the same sheet as Move to Trash, worded for a delete that
//! cannot be undone; confirming calls `PlatformServices::delete_permanently` and takes the
//! item out of the tree. A failure is reported; a folder that failed part-way is rescanned
//! so the map shows what is left. The scan root is refused, and the web demo simulates it.

mod common;

use agg_gui::platform::{current_platform, Platform};
use agg_gui::{Key, Modifiers};
use common::{fake, fake_map, fake_map_with, Harness};
use hdtreemap_core::model::NodeId;
use hdtreemap_core::platform::{DemoPlatform, PlatformServices};
use hdtreemap_core::state::AppState;
use hdtreemap_core::ui::commands::{Command, Commands};
use hdtreemap_core::ui::folder_tree::{FolderTree, FOLDER_TREE_ID};
use hdtreemap_core::ui::node_actions::{DELETE_ROOT_MESSAGE, SIMULATED_TRASH_MESSAGE};
use hdtreemap_core::ui::sheets::{
    DELETE_EXPLANATION, REMOVE_CANCEL_ID, REMOVE_CONFIRM_ID, REMOVE_EXPLANATION_ID,
    REMOVE_SHEET_ID, REMOVE_TITLE_ID, TRASH_EXPLANATION,
};
use std::rc::Rc;

const DELETE: &str = "Delete Permanently\u{2026}";
const TRASH: &str = "Move to Trash\u{2026}";

/// The first child of the scan root that `want` accepts.
fn root_child(state: &AppState, want: impl Fn(bool) -> bool) -> NodeId {
    state
        .with_result(|r, _| {
            let tree = &r.tree;
            tree.children(tree.root())
                .find(|&c| tree.node(c).is_some_and(|n| want(n.is_dir())))
        })
        .flatten()
        .expect("a child of the root")
}

fn path_of(state: &AppState, node: NodeId) -> String {
    state
        .with_result(|r, _| r.tree.path(node).display().to_string())
        .expect("path")
}

fn name_of(state: &AppState, node: NodeId) -> String {
    state
        .with_result(|r, _| r.tree.name(node).to_string())
        .expect("name")
}

fn is_live(state: &AppState, node: NodeId) -> bool {
    state
        .with_result(|r, _| r.tree.is_live(node))
        .unwrap_or(false)
}

/// Right-click `node`'s row in the folder tree.
fn right_click_row(h: &mut Harness, node: NodeId) {
    h.frame();
    let rect = h
        .widget::<FolderTree>(FOLDER_TREE_ID)
        .row_rect(node)
        .expect("the row is in view");
    let (x, y) = h.point_at_local(FOLDER_TREE_ID, rect);
    h.right_click_at(x, y);
}

/// Delete Permanently's shortcut as the keyboard sends it.
fn delete_key() -> (Key, Modifiers) {
    if current_platform() == Platform::MacOS {
        (
            Key::Backspace,
            Modifiers {
                meta: true,
                alt: true,
                ..Default::default()
            },
        )
    } else {
        (
            Key::Delete,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        )
    }
}

#[test]
fn delete_permanently_from_the_tree_asks_first_then_removes_the_folder() {
    let (mut h, calls) = fake_map();
    let folder = root_child(&h.state, |dir| dir);
    let name = name_of(&h.state, folder);
    let path = path_of(&h.state, folder);
    let total = h.state.with_result(|r, _| r.tree.total()).expect("total");
    let size = h
        .state
        .with_result(|r, _| r.tree.sizes(folder))
        .expect("sizes");

    right_click_row(&mut h, folder);
    let menu = h.context_menu();
    assert_eq!(menu.node(), Some(folder));
    let items = menu.items();
    let n = items.len();
    assert_eq!(
        items[n - 2..],
        [(TRASH.to_string(), true), (DELETE.to_string(), true)],
        "Delete Permanently follows Move to Trash"
    );

    // Cancel: nothing happens.
    h.choose(DELETE);
    assert!(h.visible(REMOVE_SHEET_ID), "the confirmation opens");
    let title = h.label_text(REMOVE_TITLE_ID);
    assert!(
        title.contains(&name) && title.contains("permanently"),
        "{title}"
    );
    assert_eq!(h.label_text(REMOVE_EXPLANATION_ID), DELETE_EXPLANATION);
    h.click(REMOVE_CANCEL_ID);
    assert!(!h.visible(REMOVE_SHEET_ID));
    assert!(calls.borrow().is_empty());
    assert!(is_live(&h.state, folder));

    // Confirm: deleted through the platform and gone from the tree.
    right_click_row(&mut h, folder);
    h.choose(DELETE);
    h.click(REMOVE_CONFIRM_ID);
    assert!(!h.visible(REMOVE_SHEET_ID));
    assert_eq!(*calls.borrow(), vec![format!("delete {path}")]);
    assert!(!is_live(&h.state, folder));
    let after = h.state.with_result(|r, _| r.tree.total()).expect("total");
    assert_eq!(after.allocated, total.allocated - size.allocated);
    h.frame();
    assert!(h.widget::<FolderTree>(FOLDER_TREE_ID).row(folder).is_none());
    assert!(
        h.toasts()
            .iter()
            .any(|t| t.starts_with(&format!("Deleted {name}"))),
        "{:?}",
        h.toasts()
    );
}

#[test]
fn trash_and_delete_share_the_sheet_but_say_what_they_do() {
    let (mut h, calls) = fake_map();
    let file = root_child(&h.state, |dir| !dir);
    right_click_row(&mut h, file);
    h.choose(TRASH);
    assert!(h.label_text(REMOVE_TITLE_ID).contains("Trash"));
    assert_eq!(h.label_text(REMOVE_EXPLANATION_ID), TRASH_EXPLANATION);
    h.key(Key::Escape);
    assert!(!h.visible(REMOVE_SHEET_ID));

    right_click_row(&mut h, file);
    h.choose(DELETE);
    assert!(h.label_text(REMOVE_TITLE_ID).contains("permanently"));
    assert_eq!(h.label_text(REMOVE_EXPLANATION_ID), DELETE_EXPLANATION);
    // Confirming the delete never trashes.
    h.click(REMOVE_CONFIRM_ID);
    assert_eq!(
        *calls.borrow(),
        vec![format!("delete {}", path_of(&h.state, file))]
    );
}

#[test]
fn a_folder_that_failed_to_delete_is_reported_and_rescanned() {
    let (mut platform, _) = fake(Vec::new(), None);
    platform.pace = hdtreemap_core::demo_fs::DemoPace::Unpaced;
    platform.delete_error = Some("Could not delete it: permission denied".into());
    let (mut h, calls) = fake_map_with(platform);
    let folder = root_child(&h.state, |dir| dir);
    let path = path_of(&h.state, folder);
    right_click_row(&mut h, folder);
    h.choose(DELETE);
    h.click(REMOVE_CONFIRM_ID);
    assert!(h
        .toasts()
        .contains(&"Could not delete it: permission denied".to_string()));
    // Part of it may be gone: the folder is scanned again to show what is left.
    assert_eq!(
        *calls.borrow(),
        vec![format!("delete {path}"), format!("scan {path}")]
    );
    assert!(h.frames_until(500, |h| !h.state.is_rescanning()));
    assert!(is_live(&h.state, folder));
}

#[test]
fn a_file_that_failed_to_delete_changes_nothing() {
    let (mut platform, _) = fake(Vec::new(), None);
    platform.pace = hdtreemap_core::demo_fs::DemoPace::Unpaced;
    platform.delete_error = Some("Could not delete it: in use".into());
    let (mut h, calls) = fake_map_with(platform);
    let file = root_child(&h.state, |dir| !dir);
    right_click_row(&mut h, file);
    h.choose(DELETE);
    h.click(REMOVE_CONFIRM_ID);
    assert_eq!(
        *calls.borrow(),
        vec![format!("delete {}", path_of(&h.state, file))]
    );
    assert!(is_live(&h.state, file));
    assert!(!h.state.is_rescanning());
    assert!(h
        .toasts()
        .contains(&"Could not delete it: in use".to_string()));
}

#[test]
fn the_shortcut_deletes_the_selection_and_the_edit_menu_offers_it() {
    let (mut h, calls) = fake_map();
    let file = root_child(&h.state, |dir| !dir);
    let platform: Rc<dyn PlatformServices> = Rc::new(DemoPlatform::unpaced());
    let commands = Commands::new(&h.state, &platform);
    assert_eq!(
        Command::from_id(&Command::Delete.id()),
        Some(Command::Delete)
    );
    assert!(!commands.enabled(Command::Delete), "nothing is selected");
    h.state.set_selected(Some(file));
    h.frame();
    assert!(commands.enabled(Command::Delete));
    let (key, modifiers) = delete_key();
    h.key_with(key, modifiers);
    assert!(h.visible(REMOVE_SHEET_ID));
    assert!(h.label_text(REMOVE_TITLE_ID).contains("permanently"));
    h.click(REMOVE_CONFIRM_ID);
    assert_eq!(
        *calls.borrow(),
        vec![format!("delete {}", path_of(&h.state, file))]
    );
    assert!(!is_live(&h.state, file));
}

#[test]
fn the_scan_root_cannot_be_deleted() {
    let (mut h, calls) = fake_map();
    h.state.set_selected(Some(NodeId(0)));
    h.frame();
    let (key, modifiers) = delete_key();
    h.key_with(key, modifiers);
    assert!(!h.visible(REMOVE_SHEET_ID));
    assert!(calls.borrow().is_empty());
    assert!(
        h.toasts().contains(&DELETE_ROOT_MESSAGE.to_string()),
        "{:?}",
        h.toasts()
    );
}

#[test]
fn the_demo_delete_removes_from_the_simulated_tree_only() {
    let mut h = common::demo_map("usb-stick");
    let folder = root_child(&h.state, |dir| dir);
    right_click_row(&mut h, folder);
    h.choose(DELETE);
    h.click(REMOVE_CONFIRM_ID);
    assert!(!is_live(&h.state, folder));
    assert!(
        h.toasts().contains(&SIMULATED_TRASH_MESSAGE.to_string()),
        "{:?}",
        h.toasts()
    );
}
