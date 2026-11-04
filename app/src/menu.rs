//! The macOS menu bar, laid out like the original's MainMenu.nib (Fork, File, Edit, View,
//! Repository, Window, Help) with the actions Fourchette already has and the original's shortcuts.
//!
//! The model (`bar`, `enabled`, `checked`) is plain Rust and tested on any system. On macOS the
//! `mac` part builds the NSMenu once, on the first frame (after winit has put its own minimal
//! menu in place); a click or a key equivalent queues an `Pick` that `App` drains each frame
//! (`take`), and AppKit asks `validateMenuItem:` whether an item is greyed out, answered from the
//! `State` the app hands over each frame (`sync`). The app menu's bold name comes from the
//! bundle name, which `build.rs` links into the binary (`assets/Info.plist`).
use crate::library::{DetailsLayout, DetailsPlace};

/// An item that runs one of Fourchette's own actions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Id {
    About,
    Settings,
    OpenRepository,
    Switcher,
    Palette,
    /// The greyed line of an empty Open Recent menu.
    NoRecent,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    ShowChanges,
    ToggleDetails,
    DetailsRight,
    DetailsBottom,
    ShowHead,
    Refresh,
    Commit,
    Fetch,
    FetchAll,
    Pull,
    Push,
    Stash,
    PopStash,
    CheckoutBranch,
    NewBranch,
    NewTag,
    ShowInFinder,
    OpenInTerminal,
    Website,
    Issues,
}

impl Id {
    pub const ALL: [Id; 32] = [
        Id::About,
        Id::Settings,
        Id::OpenRepository,
        Id::Switcher,
        Id::Palette,
        Id::NoRecent,
        Id::Undo,
        Id::Redo,
        Id::Cut,
        Id::Copy,
        Id::Paste,
        Id::SelectAll,
        Id::ShowChanges,
        Id::ToggleDetails,
        Id::DetailsRight,
        Id::DetailsBottom,
        Id::ShowHead,
        Id::Refresh,
        Id::Commit,
        Id::Fetch,
        Id::FetchAll,
        Id::Pull,
        Id::Push,
        Id::Stash,
        Id::PopStash,
        Id::CheckoutBranch,
        Id::NewBranch,
        Id::NewTag,
        Id::ShowInFinder,
        Id::OpenInTerminal,
        Id::Website,
        Id::Issues,
    ];

    /// Edit actions go to the focused text field when there is one.
    pub fn is_edit(self) -> bool {
        matches!(self, Id::Undo | Id::Redo | Id::Cut | Id::Copy | Id::Paste | Id::SelectAll)
    }
}

/// What a click or a key equivalent asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Item(Id),
    /// The n-th repository of File > Open Recent (the library's `recent()` order).
    Recent(usize),
}

/// The NSMenuItem tag of each pick: 1 + its place in `Id::ALL`, or `RECENT_TAG + n`.
const RECENT_TAG: isize = 1000;

impl Pick {
    pub fn tag(self) -> isize {
        match self {
            Pick::Item(id) => 1 + Id::ALL.iter().position(|&i| i == id).unwrap_or(0) as isize,
            Pick::Recent(n) => RECENT_TAG + n as isize,
        }
    }

    pub fn from_tag(tag: isize) -> Option<Pick> {
        if tag >= RECENT_TAG {
            return Some(Pick::Recent((tag - RECENT_TAG) as usize));
        }
        Id::ALL.get(usize::try_from(tag - 1).ok()?).map(|&id| Pick::Item(id))
    }
}

/// A key equivalent. ⌘ is always part of it, as in every shortcut of the original's menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shortcut {
    pub key: char,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

const fn cmd(key: char) -> Shortcut {
    Shortcut { key, shift: false, alt: false, ctrl: false }
}

impl Shortcut {
    const fn shift(self) -> Self {
        Shortcut { shift: true, ..self }
    }
    const fn alt(self) -> Self {
        Shortcut { alt: true, ..self }
    }
    const fn ctrl(self) -> Self {
        Shortcut { ctrl: true, ..self }
    }

    /// As macOS writes it, modifiers in the order ⌃⌥⇧⌘.
    #[cfg(test)]
    pub fn label(&self) -> String {
        let mut s = String::new();
        for (on, sym) in [(self.ctrl, '⌃'), (self.alt, '⌥'), (self.shift, '⇧'), (true, '⌘')] {
            if on {
                s.push(sym);
            }
        }
        s.extend(self.key.to_uppercase());
        s
    }
}

/// What an item does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// One of Fourchette's actions.
    App(Id),
    /// A standard AppKit action sent to the first responder (hide:, terminate:, performZoom:…).
    System(&'static str),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Entry {
    Item { title: &'static str, action: Action, key: Option<Shortcut> },
    Separator,
    /// The Services submenu, filled by macOS.
    Services,
    /// File > Open Recent, filled from the repository list.
    OpenRecent,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Menu {
    pub title: &'static str,
    pub entries: Vec<Entry>,
}

pub const APP_NAME: &str = "Fourchette";
pub const REPO_URL: &str = "https://github.com/C2-POLY/fourchette";
pub const ISSUES_URL: &str = "https://github.com/C2-POLY/fourchette/issues";

fn app(title: &'static str, id: Id, key: Option<Shortcut>) -> Entry {
    Entry::Item { title, action: Action::App(id), key }
}

fn sys(title: &'static str, selector: &'static str, key: Option<Shortcut>) -> Entry {
    Entry::Item { title, action: Action::System(selector), key }
}

/// The whole menu bar, in the original's order. Shortcuts are the original's where Fourchette has
/// the action, except File > Switch Repository (⌘O) and Command Palette (⌘P), which keep the
/// keys Fourchette already uses (the original's ⌘O opens a folder dialog).
pub fn bar() -> Vec<Menu> {
    use Entry::Separator;
    vec![
        Menu {
            title: APP_NAME,
            entries: vec![
                app("About Fourchette", Id::About, None),
                Separator,
                app("Settings…", Id::Settings, Some(cmd(','))),
                Separator,
                Entry::Services,
                Separator,
                sys("Hide Fourchette", "hide:", Some(cmd('h'))),
                sys("Hide Others", "hideOtherApplications:", Some(cmd('h').alt())),
                sys("Show All", "unhideAllApplications:", None),
                Separator,
                sys("Quit Fourchette", "terminate:", Some(cmd('q'))),
            ],
        },
        Menu {
            title: "File",
            entries: vec![
                app("Open Repository…", Id::OpenRepository, None),
                Entry::OpenRecent,
                app("Switch Repository…", Id::Switcher, Some(cmd('o'))),
                app("Command Palette…", Id::Palette, Some(cmd('p'))),
            ],
        },
        Menu {
            title: "Edit",
            entries: vec![
                app("Undo", Id::Undo, Some(cmd('z'))),
                app("Redo", Id::Redo, Some(cmd('z').shift())),
                Separator,
                app("Cut", Id::Cut, Some(cmd('x'))),
                app("Copy", Id::Copy, Some(cmd('c'))),
                app("Paste", Id::Paste, Some(cmd('v'))),
                app("Select All", Id::SelectAll, Some(cmd('a'))),
            ],
        },
        Menu {
            title: "View",
            entries: vec![
                app("Show Local Changes", Id::ShowChanges, None),
                Separator,
                app("Show Details Panel", Id::ToggleDetails, Some(cmd('j'))),
                app("Details on the Right", Id::DetailsRight, None),
                app("Details at the Bottom", Id::DetailsBottom, None),
                Separator,
                app("Show HEAD", Id::ShowHead, Some(cmd('0'))),
                Separator,
                app("Refresh", Id::Refresh, Some(cmd('r'))),
                sys("Enter Full Screen", "toggleFullScreen:", Some(cmd('f').ctrl())),
            ],
        },
        Menu {
            title: "Repository",
            entries: vec![
                app("Commit", Id::Commit, Some(cmd('c').shift())),
                Separator,
                app("Fetch", Id::Fetch, Some(cmd('f').shift())),
                app("Fetch All Remotes", Id::FetchAll, Some(cmd('f').shift().alt())),
                app("Pull", Id::Pull, Some(cmd('l').shift())),
                app("Push", Id::Push, Some(cmd('p').shift())),
                Separator,
                app("Stash Changes", Id::Stash, Some(cmd('h').shift())),
                app("Pop Stash", Id::PopStash, None),
                Separator,
                app("Checkout Branch…", Id::CheckoutBranch, Some(cmd('b'))),
                Separator,
                app("New Branch…", Id::NewBranch, Some(cmd('b').shift())),
                app("New Tag…", Id::NewTag, Some(cmd('t').shift())),
                Separator,
                app("Show in Finder", Id::ShowInFinder, Some(cmd('o').ctrl())),
                app("Open in Terminal", Id::OpenInTerminal, Some(cmd('t').ctrl())),
            ],
        },
        Menu {
            title: "Window",
            entries: vec![
                sys("Minimize", "performMiniaturize:", Some(cmd('m'))),
                sys("Zoom", "performZoom:", None),
                Separator,
                sys("Bring All to Front", "arrangeInFront:", None),
            ],
        },
        Menu {
            title: "Help",
            entries: vec![app("Fourchette on GitHub", Id::Website, None), app("Report an Issue", Id::Issues, None)],
        },
    ]
}

/// What decides whether an item can run now, read from `App` each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// One of the person's repositories is open (not the sample project).
    pub repo: bool,
    /// A Git job is running (fetch, pull, push).
    pub busy: bool,
    /// A dialog (new branch, delete, confirm) is open and holds the keyboard.
    pub modal: bool,
    /// The Settings window is open.
    pub settings: bool,
    /// A text field has the keyboard (commit message, search, palette).
    pub text_focus: bool,
    pub can_undo: bool,
    /// The working copy has changes.
    pub changes: bool,
    /// The same rule as the Commit button: a subject and something staged (or amend).
    pub can_commit: bool,
    pub upstream: bool,
    pub branch: bool,
    pub stashes: bool,
    /// HEAD is in the loaded history.
    pub head_shown: bool,
    pub details: DetailsLayout,
}

pub fn enabled(id: Id, s: &State) -> bool {
    let free = !s.modal;
    let overlay = free && !s.settings;
    let git = s.repo && !s.busy && free;
    match id {
        Id::About | Id::Copy | Id::Website | Id::Issues => true,
        Id::Settings | Id::OpenRepository | Id::ToggleDetails | Id::DetailsRight | Id::DetailsBottom => free,
        Id::Switcher | Id::Palette => overlay,
        Id::NoRecent => false,
        Id::Undo => s.text_focus || (git && s.can_undo),
        Id::Redo | Id::Cut | Id::Paste | Id::SelectAll => s.text_focus,
        Id::ShowChanges => free && s.changes,
        Id::ShowHead => free && s.head_shown,
        Id::Refresh => free && s.repo,
        Id::Commit => git && s.can_commit,
        Id::Fetch | Id::FetchAll => git,
        Id::Pull => git && s.upstream,
        Id::Push => git && s.branch,
        Id::Stash => git && s.changes,
        Id::PopStash => git && s.stashes,
        Id::CheckoutBranch | Id::NewBranch | Id::NewTag => git && !s.settings,
        Id::ShowInFinder | Id::OpenInTerminal => s.repo,
    }
}

/// Items that show a check mark.
pub fn checked(id: Id, s: &State) -> bool {
    let d = s.details;
    match id {
        Id::ToggleDetails => !d.hidden,
        Id::DetailsRight => !d.hidden && d.place == DetailsPlace::Right,
        Id::DetailsBottom => !d.hidden && d.place == DetailsPlace::Bottom,
        _ => false,
    }
}

/// The titles of File > Open Recent: the folder name, followed by its group when two
/// repositories share a name.
pub fn recent_titles(entries: &[(String, String)]) -> Vec<String> {
    entries
        .iter()
        .map(|(name, group)| {
            if entries.iter().filter(|(n, _)| n == name).count() > 1 {
                format!("{name} — {group}")
            } else {
                name.clone()
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub use mac::{install, pasteboard_text, sync, take};

/// Elsewhere there is no menu bar: nothing to install and no picks.
#[cfg(not(target_os = "macos"))]
mod other {
    pub fn install(_ctx: &egui::Context) {}
    pub fn sync(_s: &super::State, _recent: &[String]) {}
    pub fn take() -> Vec<super::Pick> {
        Vec::new()
    }
    pub fn pasteboard_text() -> Option<String> {
        None
    }
}
#[cfg(not(target_os = "macos"))]
pub use other::{install, pasteboard_text, sync, take};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod mac {
    use super::{Action, Entry, Id, Pick, Shortcut, State, APP_NAME};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
    use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
    use objc2_app_kit::{
        NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionVersion, NSApplication, NSControlStateValueOff, NSControlStateValueOn,
        NSEventModifierFlags, NSMenu, NSMenuItem, NSMenuItemValidation, NSPasteboard, NSPasteboardTypeString,
    };
    use objc2_foundation::{ns_string, MainThreadMarker, NSDictionary, NSString};
    use std::cell::RefCell;

    declare_class!(
        struct Target;

        // SAFETY: NSObject has no subclassing requirements; the target lives on the main thread
        // and has no Drop.
        unsafe impl ClassType for Target {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "FourchetteMenuTarget";
        }

        impl DeclaredClass for Target {}

        unsafe impl Target {
            #[method(menuAction:)]
            fn menu_action(&self, sender: &NSMenuItem) {
                let Some(pick) = Pick::from_tag(unsafe { sender.tag() }) else { return };
                if pick == Pick::Item(Id::About) {
                    about();
                    return;
                }
                UI.with(|u| {
                    let mut u = u.borrow_mut();
                    u.queue.push(pick);
                    if let Some(ctx) = &u.ctx {
                        ctx.request_repaint();
                    }
                });
            }
        }

        unsafe impl NSObjectProtocol for Target {}

        unsafe impl NSMenuItemValidation for Target {
            #[method(validateMenuItem:)]
            unsafe fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
                let pick = Pick::from_tag(unsafe { item.tag() });
                let (on, check) = UI.with(|u| {
                    let u = u.borrow();
                    match pick {
                        Some(Pick::Item(id)) => (super::enabled(id, &u.state), super::checked(id, &u.state)),
                        Some(Pick::Recent(_)) => (!u.state.modal, false),
                        None => (false, false),
                    }
                });
                unsafe { item.setState(if check { NSControlStateValueOn } else { NSControlStateValueOff }) };
                on
            }
        }
    );

    impl Target {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(());
            unsafe { msg_send_id![super(this), init] }
        }
    }

    /// What the menu needs between frames, on the main thread only.
    #[derive(Default)]
    struct Ui {
        ctx: Option<egui::Context>,
        target: Option<Retained<Target>>,
        recent_menu: Option<Retained<NSMenu>>,
        recent: Vec<String>,
        state: State,
        queue: Vec<Pick>,
    }

    thread_local! {
        static UI: RefCell<Ui> = RefCell::default();
    }

    /// Replaces winit's minimal menu with the full one. Call it from the main thread, after
    /// the event loop has started (the first frame); later calls do nothing.
    pub fn install(ctx: &egui::Context) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if UI.with(|u| u.borrow().target.is_some()) {
            return;
        }
        let target = Target::new(mtm);
        let app = NSApplication::sharedApplication(mtm);
        let bar = NSMenu::new(mtm);
        let mut recent_menu = None;
        for m in super::bar() {
            let menu = unsafe { NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(m.title)) };
            for e in &m.entries {
                match e {
                    Entry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                    Entry::Services => {
                        let sub = unsafe { NSMenu::initWithTitle(mtm.alloc(), ns_string!("Services")) };
                        let item = item(mtm, "Services", None, None);
                        item.setSubmenu(Some(&sub));
                        menu.addItem(&item);
                        unsafe { app.setServicesMenu(Some(&sub)) };
                    }
                    Entry::OpenRecent => {
                        let sub = unsafe { NSMenu::initWithTitle(mtm.alloc(), ns_string!("Open Recent")) };
                        let item = item(mtm, "Open Recent", None, None);
                        item.setSubmenu(Some(&sub));
                        menu.addItem(&item);
                        recent_menu = Some(sub);
                    }
                    Entry::Item { title, action: Action::System(selector), key } => {
                        menu.addItem(&item(mtm, title, Some(Sel::register(selector)), *key));
                    }
                    Entry::Item { title, action: Action::App(id), key } => {
                        let it = item(mtm, title, Some(sel!(menuAction:)), *key);
                        unsafe {
                            it.setTarget(Some(&target));
                            it.setTag(Pick::Item(*id).tag());
                        }
                        menu.addItem(&it);
                    }
                }
            }
            let top = item(mtm, m.title, None, None);
            top.setSubmenu(Some(&menu));
            bar.addItem(&top);
            unsafe {
                match m.title {
                    "Window" => app.setWindowsMenu(Some(&menu)),
                    "Help" => app.setHelpMenu(Some(&menu)),
                    _ => {}
                }
            }
        }
        app.setMainMenu(Some(&bar));
        UI.with(|u| {
            let mut u = u.borrow_mut();
            u.ctx = Some(ctx.clone());
            u.target = Some(target);
            u.recent_menu = recent_menu;
            u.recent = vec![String::from("\u{0}")]; // forces the first fill
        });
    }

    /// Hands over this frame's state, and refills Open Recent when the list changed.
    pub fn sync(state: &State, recent: &[String]) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let refill = UI.with(|u| {
            let mut u = u.borrow_mut();
            u.state = state.clone();
            if u.recent == recent {
                return None;
            }
            u.recent = recent.to_vec();
            Some((u.recent_menu.clone()?, u.target.clone()?))
        });
        let Some((menu, target)) = refill else { return };
        unsafe { menu.removeAllItems() };
        if recent.is_empty() {
            let it = item(mtm, "No Recent Repositories", Some(sel!(menuAction:)), None);
            unsafe {
                it.setTarget(Some(&target));
                it.setTag(Pick::Item(Id::NoRecent).tag());
            }
            menu.addItem(&it);
        }
        for (n, title) in recent.iter().enumerate() {
            let it = item(mtm, title, Some(sel!(menuAction:)), None);
            unsafe {
                it.setTarget(Some(&target));
                it.setTag(Pick::Recent(n).tag());
            }
            menu.addItem(&it);
        }
    }

    /// The picks made since the last frame.
    pub fn take() -> Vec<Pick> {
        UI.with(|u| std::mem::take(&mut u.borrow_mut().queue))
    }

    /// The text on the clipboard, for Edit > Paste into a text field.
    pub fn pasteboard_text() -> Option<String> {
        let s = unsafe { NSPasteboard::generalPasteboard().stringForType(NSPasteboardTypeString) }?;
        Some(s.to_string())
    }

    /// The standard About panel, with the name and version an unbundled binary lacks.
    fn about() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
        let keys: [&NSString; 3] = unsafe { [NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionVersion] };
        let values = vec![NSString::from_str(APP_NAME), version, NSString::new()];
        let dict: Retained<NSDictionary<NSString, NSString>> = NSDictionary::from_vec(&keys, values);
        // SAFETY: an NSDictionary of NSString values is an NSDictionary of objects.
        let dict: Retained<NSDictionary<NSString, AnyObject>> = unsafe { Retained::cast(dict) };
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.orderFrontStandardAboutPanelWithOptions(&dict) };
    }

    fn item(mtm: MainThreadMarker, title: &str, action: Option<Sel>, key: Option<Shortcut>) -> Retained<NSMenuItem> {
        let k = key.map(|k| k.key.to_string()).unwrap_or_default();
        let it = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), &NSString::from_str(title), action, &NSString::from_str(&k)) };
        if let Some(k) = key {
            let mut mask = NSEventModifierFlags::NSEventModifierFlagCommand;
            if k.shift {
                mask |= NSEventModifierFlags::NSEventModifierFlagShift;
            }
            if k.alt {
                mask |= NSEventModifierFlags::NSEventModifierFlagOption;
            }
            if k.ctrl {
                mask |= NSEventModifierFlags::NSEventModifierFlagControl;
            }
            it.setKeyEquivalentModifierMask(mask);
        }
        it
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(m: &Menu) -> Vec<&'static str> {
        m.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item { title, .. } => Some(*title),
                Entry::Services => Some("Services"),
                Entry::OpenRecent => Some("Open Recent"),
                Entry::Separator => None,
            })
            .collect()
    }

    fn items() -> Vec<(&'static str, Action, Option<Shortcut>)> {
        bar()
            .into_iter()
            .flat_map(|m| m.entries)
            .filter_map(|e| match e {
                Entry::Item { title, action, key } => Some((title, action, key)),
                _ => None,
            })
            .collect()
    }

    fn key_of(title: &str) -> String {
        items().into_iter().find(|i| i.0 == title).and_then(|i| i.2).map(|k| k.label()).unwrap_or_default()
    }

    #[test]
    fn menus_come_in_the_originals_order() {
        let names: Vec<&str> = bar().iter().map(|m| m.title).collect();
        assert_eq!(names, ["Fourchette", "File", "Edit", "View", "Repository", "Window", "Help"]);
    }

    #[test]
    fn standard_menus_have_the_mac_items() {
        let b = bar();
        assert_eq!(titles(&b[0]), ["About Fourchette", "Settings…", "Services", "Hide Fourchette", "Hide Others", "Show All", "Quit Fourchette"]);
        assert_eq!(titles(&b[2]), ["Undo", "Redo", "Cut", "Copy", "Paste", "Select All"]);
        assert_eq!(titles(&b[5]), ["Minimize", "Zoom", "Bring All to Front"]);
    }

    #[test]
    fn shortcuts_are_the_originals_or_fourchettes() {
        // The original's MainMenu.nib (ev_1c8c0440).
        for (title, key) in [
            ("Settings…", "⌘,"),
            ("Hide Fourchette", "⌘H"),
            ("Hide Others", "⌥⌘H"),
            ("Quit Fourchette", "⌘Q"),
            ("Undo", "⌘Z"),
            ("Redo", "⇧⌘Z"),
            ("Copy", "⌘C"),
            ("Show HEAD", "⌘0"),
            ("Refresh", "⌘R"),
            ("Enter Full Screen", "⌃⌘F"),
            ("Commit", "⇧⌘C"),
            ("Fetch", "⇧⌘F"),
            ("Fetch All Remotes", "⌥⇧⌘F"),
            ("Pull", "⇧⌘L"),
            ("Push", "⇧⌘P"),
            ("Stash Changes", "⇧⌘H"),
            ("Checkout Branch…", "⌘B"),
            ("New Branch…", "⇧⌘B"),
            ("New Tag…", "⇧⌘T"),
            ("Show in Finder", "⌃⌘O"),
            ("Open in Terminal", "⌃⌘T"),
            ("Minimize", "⌘M"),
            // Fourchette's own keys.
            ("Switch Repository…", "⌘O"),
            ("Command Palette…", "⌘P"),
            ("Show Details Panel", "⌘J"),
        ] {
            assert_eq!(key_of(title), key, "{title}");
        }
    }

    #[test]
    fn no_shortcut_is_used_twice() {
        let keys: Vec<Shortcut> = items().into_iter().filter_map(|i| i.2).collect();
        for (n, k) in keys.iter().enumerate() {
            assert!(!keys[n + 1..].contains(k), "{} is used twice", k.label());
        }
    }

    #[test]
    fn every_app_item_appears_once_and_round_trips_its_tag() {
        let ids: Vec<Id> = items()
            .into_iter()
            .filter_map(|i| match i.1 {
                Action::App(id) => Some(id),
                Action::System(_) => None,
            })
            .collect();
        for id in Id::ALL {
            let n = ids.iter().filter(|&&i| i == id).count();
            assert_eq!(n, usize::from(id != Id::NoRecent), "{id:?}");
            assert_eq!(Pick::from_tag(Pick::Item(id).tag()), Some(Pick::Item(id)));
        }
        assert_eq!(Pick::from_tag(Pick::Recent(3).tag()), Some(Pick::Recent(3)));
        assert_eq!(Pick::from_tag(0), None);
    }

    fn open_repo() -> State {
        State { repo: true, changes: true, can_commit: true, can_undo: true, upstream: true, branch: true, stashes: true, head_shown: true, ..State::default() }
    }

    #[test]
    fn repository_actions_are_greyed_without_a_repository() {
        let s = State { changes: true, can_commit: true, head_shown: true, ..State::default() };
        for id in [Id::Commit, Id::Fetch, Id::FetchAll, Id::Pull, Id::Push, Id::Stash, Id::PopStash, Id::CheckoutBranch, Id::NewBranch, Id::NewTag, Id::ShowInFinder, Id::OpenInTerminal, Id::Refresh, Id::Undo] {
            assert!(!enabled(id, &s), "{id:?}");
        }
        // Looking around the sample project still works.
        for id in [Id::About, Id::Settings, Id::OpenRepository, Id::Switcher, Id::Palette, Id::ShowChanges, Id::ToggleDetails, Id::ShowHead, Id::Copy, Id::Website] {
            assert!(enabled(id, &s), "{id:?}");
        }
    }

    #[test]
    fn git_actions_wait_for_the_running_one() {
        let s = State { busy: true, ..open_repo() };
        for id in [Id::Commit, Id::Fetch, Id::Pull, Id::Push, Id::Stash, Id::PopStash, Id::NewBranch, Id::Undo] {
            assert!(!enabled(id, &s), "{id:?}");
        }
        assert!(enabled(Id::ShowInFinder, &s));
        assert!(enabled(Id::Fetch, &open_repo()));
    }

    #[test]
    fn items_follow_the_repository_state() {
        let s = open_repo();
        assert!(enabled(Id::Commit, &s) && enabled(Id::Pull, &s) && enabled(Id::PopStash, &s));
        assert!(!enabled(Id::Commit, &State { can_commit: false, ..s.clone() }));
        assert!(!enabled(Id::Pull, &State { upstream: false, ..s.clone() }));
        assert!(!enabled(Id::Push, &State { branch: false, ..s.clone() }));
        assert!(!enabled(Id::Stash, &State { changes: false, ..s.clone() }));
        assert!(!enabled(Id::PopStash, &State { stashes: false, ..s.clone() }));
        assert!(!enabled(Id::Undo, &State { can_undo: false, ..s.clone() }));
        assert!(!enabled(Id::ShowHead, &State { head_shown: false, ..s }));
    }

    #[test]
    fn edit_items_follow_the_text_field() {
        let s = State::default();
        assert!(!enabled(Id::Cut, &s) && !enabled(Id::Paste, &s) && !enabled(Id::SelectAll, &s) && !enabled(Id::Redo, &s));
        assert!(enabled(Id::Copy, &s), "copies selected label text too");
        let t = State { text_focus: true, ..s };
        for id in [Id::Undo, Id::Redo, Id::Cut, Id::Copy, Id::Paste, Id::SelectAll] {
            assert!(enabled(id, &t), "{id:?}");
            assert!(id.is_edit());
        }
    }

    #[test]
    fn a_dialog_or_settings_blocks_what_would_open_on_top() {
        let d = State { modal: true, ..open_repo() };
        for id in [Id::Settings, Id::Switcher, Id::Palette, Id::Commit, Id::Push, Id::Refresh, Id::ToggleDetails] {
            assert!(!enabled(id, &d), "{id:?}");
        }
        let s = State { settings: true, ..open_repo() };
        for id in [Id::Switcher, Id::Palette, Id::CheckoutBranch, Id::NewBranch, Id::NewTag] {
            assert!(!enabled(id, &s), "{id:?}");
        }
        assert!(enabled(Id::Settings, &s), "⌘, closes it again");
    }

    #[test]
    fn details_items_show_the_current_layout() {
        let mut s = State::default();
        assert!(checked(Id::ToggleDetails, &s) && checked(Id::DetailsRight, &s) && !checked(Id::DetailsBottom, &s));
        s.details = DetailsLayout::shown(DetailsPlace::Bottom);
        assert!(checked(Id::DetailsBottom, &s) && !checked(Id::DetailsRight, &s));
        s.details = s.details.hide();
        assert!(!checked(Id::ToggleDetails, &s) && !checked(Id::DetailsBottom, &s));
    }

    #[test]
    fn recent_titles_tell_same_named_repositories_apart() {
        let e = |n: &str, g: &str| (n.to_string(), g.to_string());
        assert_eq!(recent_titles(&[e("fourchette", "c2-poly"), e("api", "c2-poly"), e("api", "perso")]), ["fourchette", "api — c2-poly", "api — perso"]);
    }
}
