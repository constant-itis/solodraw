//! Right-hand assistant: Inspector / Characters / Snippets tabs (docs/UI_INVENTORY.md §4).

use crate::prelude::*;

/// Built-in palettes. Character sets are plain Unicode ranges, plus user palettes
/// (`{id, name, characters:[{char}]}` JSON) from the characters folder.
pub fn palettes() -> Vec<(String, Vec<char>)> {
    let range = |a: u32, b: u32| (a..=b).filter_map(char::from_u32).collect::<Vec<_>>();
    let mut v = vec![
        ("Box Drawing".to_string(), range(0x2500, 0x257F)),
        ("ASCII".to_string(), range(0x21, 0x7E)),
        ("Block Elements".to_string(), range(0x2580, 0x259F)),
        ("Geometric Shapes".to_string(), range(0x25A0, 0x25FF)),
        ("Arrows".to_string(), range(0x2190, 0x21FF)),
    ];
    if let Ok(dir) = std::fs::read_dir(crate::actions::data_dir("char-palettes")) {
        let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
        files.sort();
        for f in files {
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            let Ok(j) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let name = j["name"].as_str().unwrap_or("Custom").to_string();
            let chars = j["characters"]
                .as_array()
                .map(|a| a.iter().filter_map(|c| c["char"].as_str()?.chars().next()).collect())
                .unwrap_or_default();
            v.push((name, chars));
        }
    }
    v
}

pub struct AssistantParts {
    pub pane: gtk::Box,
    pub stack: gtk::Stack,
    pub inspector: gtk::Box,
    pub pencil_label: gtk::Label,
    pub snippet_search: gtk::SearchEntry,
    pub char_search: gtk::SearchEntry,
}

/// "Animate assistant": switching tabs pushes horizontally in the tab direction for about 0.17 s
/// (Monodraw 1.6.1 capture, golden-gui step 06); off switches instantly.
pub fn set_tab_animation(stack: &gtk::Stack) {
    let on = crate::prefs::get_bool(crate::prefs::ASSISTANT_ANIMATED, true);
    stack.set_transition_type(if on { gtk::StackTransitionType::SlideLeftRight } else { gtk::StackTransitionType::None });
    stack.set_transition_duration(170);
}

pub fn build_assistant(ui_slot: &Rc<RefCell<Option<U>>>, st: &St) -> AssistantParts {
    let pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
    pane.set_width_request(246);
    pane.add_css_class("md-assistant");

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    set_tab_animation(&stack);
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_margin_top(6);
    switcher.set_margin_bottom(6);
    pane.append(&switcher);
    pane.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    pane.append(&stack);

    // Inspector.
    let inspector = gtk::Box::new(gtk::Orientation::Vertical, 4);
    inspector.set_margin_start(12);
    inspector.set_margin_end(12);
    inspector.set_margin_top(6);
    inspector.set_margin_bottom(12);
    let iscroll = gtk::ScrolledWindow::new();
    iscroll.set_child(Some(&inspector));
    iscroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    stack.add_titled(&iscroll, Some("inspector"), "Inspector");

    // Characters.
    let chars = gtk::Box::new(gtk::Orientation::Vertical, 8);
    chars.set_margin_start(12);
    chars.set_margin_end(12);
    chars.set_margin_top(8);
    let pencil_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    pencil_row.append(&gtk::Label::new(Some("Pencil:")));
    let pencil_label = gtk::Label::new(Some(&st.borrow().pencil_char.to_string()));
    pencil_label.add_css_class("md-pencil-char");
    pencil_row.append(&pencil_label);
    chars.append(&pencil_row);

    let pick = {
        let (slot, s) = (ui_slot.clone(), st.clone());
        Rc::new(move |c: char| {
            s.borrow_mut().pencil_char = c;
            if let Some(ui) = slot.borrow().clone() {
                refresh(&ui, &s.borrow());
            }
        })
    };
    let (pal, char_search) = palette_box(pick);
    pal.set_vexpand(true);
    chars.append(&pal);
    stack.add_titled(&chars, Some("characters"), "Characters");

    let snippets = crate::snippets::build_tab(ui_slot, st);
    stack.add_titled(&snippets.page, Some("snippets"), "Snippets");

    AssistantParts { pane, stack, inspector, pencil_label, snippet_search: snippets.search, char_search }
}

/// Characters matching a search: the character itself, its code point (`2550`, `U+2550`,
/// `0x2550`), or every word of the query in its Unicode name. Searches all palettes, in order.
pub fn search_chars(pals: &[(String, Vec<char>)], query: &str) -> Vec<char> {
    let q = query.trim();
    if q.is_empty() {
        return vec![];
    }
    let hex = q.to_ascii_lowercase();
    let hex = hex.trim_start_matches("u+").trim_start_matches("0x");
    let code = u32::from_str_radix(hex, 16).ok().filter(|_| hex.len() >= 2);
    let words: Vec<String> = q.split_whitespace().map(|w| w.to_uppercase()).collect();
    let name = |c: char| crate::charnames::NAMES.iter().find(|(k, _)| *k == c).map(|(_, n)| *n);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for c in pals.iter().flat_map(|p| p.1.iter().copied()) {
        let hit = q.chars().count() == 1 && q.starts_with(c)
            || code == Some(c as u32)
            || name(c).is_some_and(|n| words.iter().all(|w| n.contains(w.as_str())));
        if hit && seen.insert(c) {
            out.push(c);
        }
    }
    out
}

/// Palette popup and search field above a grid of character buttons; `on_pick` gets the
/// clicked character. Shared by the Characters tab and the Character Picker window.
pub fn palette_box(on_pick: Rc<dyn Fn(char)>) -> (gtk::Box, gtk::SearchEntry) {
    let vb = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let pals = palettes();
    let names: Vec<&str> = pals.iter().map(|p| p.0.as_str()).collect();
    let pal_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    pal_row.append(&gtk::Label::new(Some("Palette:")));
    let dd = gtk::DropDown::from_strings(&names);
    dd.set_hexpand(true);
    pal_row.append(&dd);
    vb.append(&pal_row);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search characters"));
    vb.append(&search);

    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_max_children_per_line(8);
    grid.set_min_children_per_line(8);
    grid.set_homogeneous(true);
    // Few results stay packed at the top instead of spreading over the pane.
    grid.set_valign(gtk::Align::Start);
    grid.set_row_spacing(0);
    grid.set_column_spacing(0);
    let gscroll = gtk::ScrolledWindow::new();
    gscroll.set_child(Some(&grid));
    gscroll.set_vexpand(true);
    gscroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    vb.append(&gscroll);

    let fill = Rc::new(move |chars: &[char]| {
        while let Some(c) = grid.first_child() {
            grid.remove(&c);
        }
        for &c in chars {
            let b = gtk::Button::with_label(&c.to_string());
            b.add_css_class("flat");
            b.add_css_class("md-char-button");
            b.set_tooltip_text(Some(&format!("U+{:04X}", c as u32)));
            let f = on_pick.clone();
            b.connect_clicked(move |_| f(c));
            grid.insert(&b, -1);
        }
    });
    let pals = Rc::new(pals);
    // An empty search shows the chosen palette; otherwise matches from every palette.
    let show = {
        let (fill, pals, dd, search) = (fill.clone(), pals.clone(), dd.clone(), search.downgrade());
        Rc::new(move || {
            let q = search.upgrade().map(|s| s.text().to_string()).unwrap_or_default();
            if q.trim().is_empty() {
                fill(&pals[(dd.selected() as usize).min(pals.len().saturating_sub(1))].1);
            } else {
                fill(&search_chars(&pals, &q));
            }
        })
    };
    show();
    let s2 = show.clone();
    dd.connect_selected_notify(move |_| s2());
    search.connect_search_changed(move |_| show());
    (vb, search)
}

/// Window ▸ Character Picker: a floating palette that sets the pencil character.
pub fn character_picker(ui: &U, st: &St) {
    let win = gtk::Window::builder().title("Character Picker").transient_for(&ui.window).default_width(300).default_height(380).build();
    let vb = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vb.set_margin_start(10);
    vb.set_margin_end(10);
    vb.set_margin_top(10);
    vb.set_margin_bottom(10);
    let (u, s) = (ui.clone(), st.clone());
    vb.append(&palette_box(Rc::new(move |c| {
        s.borrow_mut().pencil_char = c;
        refresh(&u, &s.borrow());
        u.status.set_text(&format!("Pencil character: {c}"));
    })).0);
    win.set_child(Some(&vb));
    win.present();
}

#[cfg(test)]
mod tests {
    use super::search_chars;

    #[test]
    fn character_search() {
        let pals = vec![("Box".to_string(), vec!['─', '═', '╬', '┼']), ("ASCII".to_string(), vec!['A', '+', '─'])];
        assert_eq!(search_chars(&pals, "double"), vec!['═', '╬']);
        assert_eq!(search_chars(&pals, "double vertical"), vec!['╬']);
        assert_eq!(search_chars(&pals, "U+2550"), vec!['═']);
        assert_eq!(search_chars(&pals, "253c"), vec!['┼']);
        assert_eq!(search_chars(&pals, "+"), vec!['+']);
        assert_eq!(search_chars(&pals, "light horizontal"), vec!['─', '┼'], "deduplicated across palettes");
        assert!(search_chars(&pals, "  ").is_empty());
    }
}
