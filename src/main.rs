mod calc;
mod i18n;
mod theme;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use rust_decimal::Decimal;

use calc::{CalcError, Notation};
use i18n::{Lang, Msg};
use theme::Theme;

const APP_ID: &str = "io.github.HollowPhobos.rechner";

const CSS: &str = "
entry.display { font-size: 20px; }
button.result { padding: 4px 10px; }
button.result label { font-size: 26px; font-weight: bold; }
button.result.preview label { opacity: 0.5; font-weight: normal; }
button.result.error label { font-size: 16px; color: @error_color; }
label.copied { font-size: 11px; opacity: 0.7; }
grid.keys button { min-height: 34px; }
";

/// Characters that continue a calculation from the previous result instead
/// of starting a new one.
const CONTINUES: &[char] = &['+', '-', '−', '*', '×', '/', '÷', ':', '^', '²', '%'];

/// Window opacity choices while another window has focus, in percent.
const OPACITIES: &[u8] = &[20, 50, 80, 100];

struct Config {
    notation: Notation,
    theme: Theme,
    lang: Lang,
    /// Window opacity in percent while another window has focus.
    opacity: u8,
}

impl Config {
    fn path() -> PathBuf {
        glib::user_config_dir().join("rechner").join("config")
    }

    fn load() -> Self {
        let mut cfg = Config {
            notation: Notation::Comma,
            theme: Theme::Light,
            lang: Lang::En,
            opacity: 20,
        };
        let Ok(text) = std::fs::read_to_string(Self::path()) else {
            return cfg;
        };
        for line in text.lines() {
            match line.split_once('=') {
                Some(("notation", v)) => cfg.notation = Notation::from_key(v).unwrap_or(cfg.notation),
                Some(("theme", v)) => cfg.theme = Theme::from_key(v).unwrap_or(cfg.theme),
                Some(("lang", v)) => cfg.lang = Lang::from_key(v).unwrap_or(cfg.lang),
                Some(("opacity", v)) => cfg.opacity = parse_opacity(v).unwrap_or(cfg.opacity),
                _ => {}
            }
        }
        cfg
    }

    fn save(&self) {
        let path = Self::path();
        let text = format!(
            "notation={}\ntheme={}\nlang={}\nopacity={}\n",
            self.notation.key(),
            self.theme.key(),
            self.lang.key(),
            self.opacity
        );
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, text));
        if let Err(e) = result {
            eprintln!("rechner: {}: {e}", path.display());
        }
    }
}

struct Ui {
    entry: gtk::Entry,
    result_button: gtk::Button,
    result_label: gtk::Label,
    copied_label: gtk::Label,
    decimal_button: gtk::Button,
    square_button: gtk::Button,
    undo_button: gtk::Button,
    clear_button: gtk::Button,
    menu: gio::Menu,
    theme_css: gtk::CssProvider,
    config: RefCell<Config>,
    result: Cell<Option<Decimal>>,
    /// Set by `=`; the next keystroke either continues from the result or starts over.
    evaluated: Cell<bool>,
    undo: RefCell<Vec<String>>,
    /// Entry text as of the last recorded change, pushed onto `undo` on the next one.
    shown: RefCell<String>,
    /// Suppresses the `changed` handler while the text is set programmatically.
    silent: Cell<bool>,
}

impl Ui {
    fn notation(&self) -> Notation {
        self.config.borrow().notation
    }

    fn lang(&self) -> Lang {
        self.config.borrow().lang
    }

    fn text(&self) -> String {
        self.entry.text().to_string()
    }

    /// Replaces the entry text as one undo step and places the cursor (in chars).
    fn set_text(&self, text: &str, cursor: usize) {
        let old = self.shown.replace(text.to_string());
        if old != text {
            self.undo.borrow_mut().push(old);
        }
        self.silent.set(true);
        self.entry.set_text(text);
        self.entry.set_position(cursor as i32);
        self.silent.set(false);
        self.refresh();
    }

    fn on_user_edit(&self) {
        if self.silent.get() {
            return;
        }
        let old = self.shown.replace(self.text());
        self.undo.borrow_mut().push(old);
        self.refresh();
    }

    /// Inserts `s` at the cursor, replacing any selection.
    fn type_text(&self, s: &str) {
        if self.evaluated.get() {
            let start = if s.starts_with(CONTINUES) {
                self.result
                    .get()
                    .map(|v| calc::format(v, self.notation(), false))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let text = start + s;
            self.set_text(&text, text.chars().count());
            return;
        }
        let chars: Vec<char> = self.text().chars().collect();
        let (a, b) = match self.entry.selection_bounds() {
            Some((a, b)) => (a.min(b) as usize, a.max(b) as usize),
            None => {
                let p = self.entry.position() as usize;
                (p, p)
            }
        };
        let mut text: String = chars[..a].iter().collect();
        text.push_str(s);
        text.extend(&chars[b..]);
        self.set_text(&text, a + s.chars().count());
    }

    /// Replaces the character just before the cursor with `to` if it is `from`.
    /// Returns whether it did.
    fn retype_previous(&self, from: char, to: char) -> bool {
        let pos = self.entry.position() as usize;
        let mut chars: Vec<char> = self.text().chars().collect();
        if self.evaluated.get()
            || self.entry.selection_bounds().is_some()
            || pos == 0
            || chars.get(pos - 1) != Some(&from)
        {
            return false;
        }
        chars[pos - 1] = to;
        self.set_text(&chars.iter().collect::<String>(), pos);
        true
    }

    /// The numpad decimal key: once types the decimal separator, a second
    /// press right after it turns it into the group separator.
    fn decimal_key(&self) {
        let n = self.notation();
        if !self.retype_previous(n.decimal(), n.group()) {
            self.type_text(&n.decimal().to_string());
        }
    }

    /// The period key: a second `.` right after one turns both into `,`.
    /// Two periods in a row are invalid in either notation, so no valid
    /// input is lost. Returns whether it did.
    fn period_key(&self) -> bool {
        self.retype_previous('.', ',')
    }

    fn undo(&self) {
        let Some(prev) = self.undo.borrow_mut().pop() else {
            return;
        };
        self.shown.replace(prev.clone());
        self.silent.set(true);
        self.entry.set_text(&prev);
        self.entry.set_position(-1);
        self.silent.set(false);
        self.refresh();
    }

    fn clear(&self) {
        self.set_text("", 0);
    }

    /// Recomputes the live preview after any change to the expression.
    fn refresh(&self) {
        self.evaluated.set(false);
        match calc::evaluate(&self.text(), self.notation()) {
            Ok(v) => self.show_result(Some(v), "preview"),
            Err(_) => self.show_result(None, "preview"),
        }
    }

    fn evaluate(&self) {
        match calc::evaluate(&self.text(), self.notation()) {
            Ok(v) => {
                self.show_result(Some(v), "final");
                self.evaluated.set(true);
            }
            Err(CalcError::Empty) => self.show_result(None, "final"),
            Err(e) => {
                self.result.set(None);
                self.result_label.set_text(self.lang().error(e));
                self.set_result_class("error");
            }
        }
    }

    fn show_result(&self, value: Option<Decimal>, class: &str) {
        self.result.set(value);
        let text = value
            .map(|v| format!("= {}", calc::format(v, self.notation(), true)))
            .unwrap_or_default();
        self.result_label.set_text(&text);
        self.set_result_class(class);
    }

    fn set_result_class(&self, class: &str) {
        for c in ["preview", "final", "error"] {
            self.result_button.remove_css_class(c);
        }
        self.result_button.add_css_class(class);
    }

    /// Copies the result without group separators, so it pastes as a plain number.
    fn copy_result(&self) {
        let Some(v) = self.result.get() else {
            return;
        };
        self.entry.clipboard().set_text(&calc::format(v, self.notation(), false));
        self.copied_label.set_visible(true);
        let label = self.copied_label.clone();
        glib::timeout_add_local_once(Duration::from_millis(1200), move || label.set_visible(false));
    }

    fn set_notation(&self, n: Notation) {
        let old = self.notation();
        self.config.borrow_mut().notation = n;
        self.config.borrow().save();
        self.decimal_button.set_label(&n.decimal().to_string());
        let text = calc::convert(&self.text(), old, n);
        let pos = self.entry.position() as usize;
        self.set_text(&text, pos);
    }

    fn set_theme(&self, t: Theme) {
        self.config.borrow_mut().theme = t;
        self.config.borrow().save();
        t.apply(&self.theme_css);
    }

    fn set_opacity(&self, percent: u8) {
        self.config.borrow_mut().opacity = percent;
        self.config.borrow().save();
    }

    fn update_opacity(&self, window: &gtk::ApplicationWindow) {
        let percent = if window.is_active() { 100 } else { self.config.borrow().opacity };
        window.set_opacity(f64::from(percent) / 100.0);
    }

    fn set_lang(&self, l: Lang) {
        self.config.borrow_mut().lang = l;
        self.config.borrow().save();
        self.apply_lang();
    }

    /// Sets every translated text in the window and menu.
    fn apply_lang(&self) {
        let l = self.lang();
        self.result_button.set_tooltip_text(Some(l.tr(Msg::ClickToCopy)));
        self.copied_label.set_text(l.tr(Msg::Copied));
        self.square_button.set_tooltip_text(Some(l.tr(Msg::ShiftClickPower)));
        self.undo_button.set_label(l.tr(Msg::Undo));
        self.clear_button.set_label(l.tr(Msg::Clear));
        fill_menu(&self.menu, l);
        if self.result_button.has_css_class("error") {
            self.evaluate();
        }
    }
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("no display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
    app.connect_activate(|app| match app.active_window() {
        // A second launch (e.g. the calculator key again) raises the open window.
        Some(win) => win.present(),
        None => build_ui(app),
    });
    app.run()
}

fn build_ui(app: &gtk::Application) {
    let config = Config::load();
    let theme_css = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("no display"),
        &theme_css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    config.theme.apply(&theme_css);

    let entry = gtk::Entry::builder().xalign(1.0).css_classes(["display"]).build();
    let result_label = gtk::Label::builder().xalign(1.0).hexpand(true).build();
    let result_button = gtk::Button::builder()
        .child(&result_label)
        .css_classes(["flat", "result", "preview"])
        .focusable(false)
        .build();
    let copied_label = gtk::Label::builder()
        .css_classes(["copied"])
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .margin_start(10)
        .visible(false)
        .can_target(false)
        .build();
    let result_row = gtk::Overlay::builder().child(&result_button).build();
    result_row.add_overlay(&copied_label);

    let keys = gtk::Grid::builder()
        .row_spacing(4)
        .column_spacing(4)
        .row_homogeneous(true)
        .column_homogeneous(true)
        .vexpand(true)
        .css_classes(["keys"])
        .build();
    let key = |label: &str, col: i32, row: i32, width: i32| {
        let b = gtk::Button::builder().label(label).focusable(false).build();
        keys.attach(&b, col, row, width, 1);
        b
    };

    let decimal_button = key(&config.notation.decimal().to_string(), 1, 3, 1);
    let square = key("x²", 4, 2, 1);
    let undo = key("", 4, 0, 1);
    let clear = key("", 5, 0, 1);
    let menu = gio::Menu::new();
    let ui = Rc::new(Ui {
        entry: entry.clone(),
        result_button: result_button.clone(),
        result_label,
        copied_label,
        decimal_button: decimal_button.clone(),
        square_button: square.clone(),
        undo_button: undo.clone(),
        clear_button: clear.clone(),
        menu: menu.clone(),
        theme_css,
        config: RefCell::new(config),
        result: Cell::new(None),
        evaluated: Cell::new(false),
        undo: RefCell::new(Vec::new()),
        shown: RefCell::new(String::new()),
        silent: Cell::new(false),
    });

    let inserts = [
        ("7", 0, 0), ("8", 1, 0), ("9", 2, 0), ("÷", 3, 0),
        ("4", 0, 1), ("5", 1, 1), ("6", 2, 1), ("×", 3, 1), ("(", 4, 1), (")", 5, 1),
        ("1", 0, 2), ("2", 1, 2), ("3", 2, 2), ("−", 3, 2), ("√", 5, 2),
        ("0", 0, 3), ("%", 2, 3), ("+", 3, 3),
    ];
    for (label, col, row) in inserts {
        let ui = ui.clone();
        key(label, col, row, 1).connect_clicked(move |_| ui.type_text(label));
    }
    clear.add_css_class("destructive-action");
    let equals = key("=", 4, 3, 2);
    equals.add_css_class("suggested-action");

    square.connect_clicked(glib::clone!(#[strong] ui, move |b| {
        ui.type_text(if shift_held(b) { "^" } else { "²" })
    }));
    undo.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.undo()));
    clear.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.clear()));
    equals.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.evaluate()));
    decimal_button.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.decimal_key()));
    result_button.connect_clicked(glib::clone!(#[strong] ui, move |_| ui.copy_result()));
    entry.connect_changed(glib::clone!(#[strong] ui, move |_| ui.on_user_edit()));
    entry.connect_activate(glib::clone!(#[strong] ui, move |_| ui.evaluate()));

    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    body.append(&entry);
    body.append(&result_row);
    body.append(&keys);

    ui.apply_lang();
    app.set_menubar(Some(&menu));
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("rechner")
        .default_width(440)
        .default_height(320)
        .show_menubar(true)
        .child(&body)
        .build();

    install_actions(app, &window, &ui);
    install_keys(&window, &ui);
    window.connect_is_active_notify(glib::clone!(#[strong] ui, move |w| ui.update_opacity(w)));
    window.present();
    entry.grab_focus();
}

fn shift_held(widget: &impl IsA<gtk::Widget>) -> bool {
    widget
        .display()
        .default_seat()
        .and_then(|seat| seat.keyboard())
        .is_some_and(|kb| kb.modifier_state().contains(gdk::ModifierType::SHIFT_MASK))
}

/// Rebuilds the menu bar in place, so the open window picks up a language change.
fn fill_menu(bar: &gio::Menu, l: Lang) {
    let calc = gio::Menu::new();
    calc.append(Some(l.tr(Msg::CopyResult)), Some("win.copy"));
    calc.append(Some(l.tr(Msg::Quit)), Some("app.quit"));

    let notation = gio::Menu::new();
    notation.append(Some(l.tr(Msg::DecimalComma)), Some("app.notation::comma"));
    notation.append(Some(l.tr(Msg::DecimalPoint)), Some("app.notation::point"));

    let base_themes = gio::Menu::new();
    base_themes.append(Some(l.tr(Msg::Light)), Some("app.theme::light"));
    base_themes.append(Some(l.tr(Msg::Dark)), Some("app.theme::dark"));
    let palettes = gio::Menu::new();
    for t in theme::PALETTES {
        palettes.append(Some(t.name()), Some(&format!("app.theme::{}", t.key())));
    }
    let themes = gio::Menu::new();
    themes.append_section(None, &base_themes);
    themes.append_section(None, &palettes);

    let opacity = gio::Menu::new();
    for &p in OPACITIES {
        opacity.append(Some(&l.percent(p)), Some(&format!("app.opacity::{p}")));
    }

    let view = gio::Menu::new();
    view.append_submenu(Some(l.tr(Msg::MenuTheme)), &themes);
    view.append_submenu(Some(l.tr(Msg::MenuOpacity)), &opacity);

    let lang = gio::Menu::new();
    for option in [Lang::En, Lang::De] {
        lang.append(Some(option.name()), Some(&format!("app.lang::{}", option.key())));
    }

    bar.remove_all();
    bar.append_submenu(Some(l.tr(Msg::MenuCalculator)), &calc);
    bar.append_submenu(Some(l.tr(Msg::MenuNotation)), &notation);
    bar.append_submenu(Some(l.tr(Msg::MenuView)), &view);
    bar.append_submenu(Some(l.tr(Msg::MenuLanguage)), &lang);
}

fn install_actions(app: &gtk::Application, window: &gtk::ApplicationWindow, ui: &Rc<Ui>) {
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(#[weak] app, move |_, _| app.quit()));
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);

    let copy = gio::SimpleAction::new("copy", None);
    copy.connect_activate(glib::clone!(#[strong] ui, move |_, _| ui.copy_result()));
    window.add_action(&copy);

    let (notation, theme, lang, opacity) = {
        let cfg = ui.config.borrow();
        (cfg.notation, cfg.theme, cfg.lang, cfg.opacity)
    };

    let action = gio::SimpleAction::new_stateful(
        "notation",
        Some(glib::VariantTy::STRING),
        &notation.key().to_variant(),
    );
    action.connect_activate(glib::clone!(#[strong] ui, move |action, param| {
        let Some(n) = param.and_then(|p| p.str()).and_then(Notation::from_key) else { return };
        action.set_state(&n.key().to_variant());
        ui.set_notation(n);
    }));
    app.add_action(&action);

    let action = gio::SimpleAction::new_stateful(
        "theme",
        Some(glib::VariantTy::STRING),
        &theme.key().to_variant(),
    );
    action.connect_activate(glib::clone!(#[strong] ui, move |action, param| {
        let Some(t) = param.and_then(|p| p.str()).and_then(Theme::from_key) else { return };
        action.set_state(&t.key().to_variant());
        ui.set_theme(t);
    }));
    app.add_action(&action);

    let action = gio::SimpleAction::new_stateful(
        "lang",
        Some(glib::VariantTy::STRING),
        &lang.key().to_variant(),
    );
    action.connect_activate(glib::clone!(#[strong] ui, move |action, param| {
        let Some(l) = param.and_then(|p| p.str()).and_then(Lang::from_key) else { return };
        action.set_state(&l.key().to_variant());
        ui.set_lang(l);
    }));
    app.add_action(&action);

    let action = gio::SimpleAction::new_stateful(
        "opacity",
        Some(glib::VariantTy::STRING),
        &opacity.to_string().to_variant(),
    );
    action.connect_activate(glib::clone!(#[strong] ui, move |action, param| {
        let Some(p) = param.and_then(|p| p.str()).and_then(parse_opacity) else { return };
        action.set_state(&p.to_string().to_variant());
        ui.set_opacity(p);
    }));
    app.add_action(&action);
}

fn parse_opacity(s: &str) -> Option<u8> {
    s.parse().ok().filter(|p| OPACITIES.contains(p))
}

fn install_keys(window: &gtk::ApplicationWindow, ui: &Rc<Ui>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let window_ = window.clone();
    keys.connect_key_pressed(glib::clone!(#[strong] ui, move |_, key, _, state| {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::KP_Decimal | gdk::Key::KP_Separator => ui.decimal_key(),
            gdk::Key::period if ui.period_key() => {}
            gdk::Key::Escape if ui.text().is_empty() => window_.close(),
            gdk::Key::Escape => ui.clear(),
            gdk::Key::z if ctrl => ui.undo(),
            gdk::Key::c if ctrl && ui.entry.selection_bounds().is_none() => ui.copy_result(),
            gdk::Key::equal | gdk::Key::KP_Equal => ui.evaluate(),
            _ => match key.to_unicode() {
                // After `=`, typed input continues from or replaces the result.
                Some(c) if ui.evaluated.get() && !ctrl && !c.is_control() => {
                    ui.type_text(&c.to_string())
                }
                _ => return glib::Propagation::Proceed,
            },
        }
        glib::Propagation::Stop
    }));
    window.add_controller(keys);
}
