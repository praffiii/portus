use std::time::Duration;

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::time::Instant;

pub const TRAY_REOPEN_GUARD: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverAction {
    Create,
    Show,
    Hide,
}

#[derive(Debug, Default)]
pub struct PopoverState;

impl PopoverState {
    pub fn toggle(&self, exists: bool, visible: bool) -> PopoverAction {
        match (exists, visible) {
            (false, _) => PopoverAction::Create,
            (true, true) => PopoverAction::Hide,
            (true, false) => PopoverAction::Show,
        }
    }

    pub fn focus_changed(&self, focused: bool) -> Option<PopoverAction> {
        (!focused).then_some(PopoverAction::Hide)
    }
}

pub fn should_suppress_reopen(
    action: PopoverAction,
    elapsed_since_focus_hide: Option<Duration>,
) -> bool {
    action == PopoverAction::Show
        && elapsed_since_focus_hide.is_some_and(|elapsed| elapsed < TRAY_REOPEN_GUARD)
}

pub fn should_hide_on_focus_loss(dialog_open: bool) -> bool {
    !dialog_open
}

#[cfg(target_os = "macos")]
pub const POPOVER_LABEL: &str = "main";

#[cfg(target_os = "macos")]
#[derive(Default)]
struct FocusHideGuard {
    last_hide: Mutex<Option<Instant>>,
}

#[cfg(target_os = "macos")]
impl FocusHideGuard {
    fn arm(&self) {
        *self
            .last_hide
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Instant::now());
    }

    fn take_elapsed(&self) -> Option<Duration> {
        self.last_hide
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .map(|instant| instant.elapsed())
    }
}

#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct DialogGuard {
    armed: AtomicBool,
}

#[cfg(target_os = "macos")]
impl DialogGuard {
    pub fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }

    pub fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }
}

#[cfg(target_os = "macos")]
pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::tray::TrayIconBuilder;
    use tauri::{ActivationPolicy, Manager};

    app.set_activation_policy(ActivationPolicy::Accessory);
    app.manage(FocusHideGuard::default());
    app.manage(DialogGuard::default());

    let mut tray = TrayIconBuilder::new()
        .tooltip("Portus")
        .icon_as_template(true)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            handle_tray_event(tray.app_handle(), &event);
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn handle_tray_event(app: &tauri::AppHandle, event: &tauri::tray::TrayIconEvent) {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};

    tauri_plugin_positioner::on_tray_event(app, event);

    if matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    ) {
        if let Err(error) = toggle_popover(app) {
            eprintln!("failed to toggle Portus popover: {error}");
        }
    }
}

#[cfg(target_os = "macos")]
fn toggle_popover(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::Manager;

    let window = app.get_webview_window(POPOVER_LABEL);
    let visible = window
        .as_ref()
        .map(|window| window.is_visible())
        .transpose()?
        .unwrap_or(false);
    let action = PopoverState.toggle(window.is_some(), visible);
    let elapsed_since_focus_hide = app.state::<FocusHideGuard>().take_elapsed();
    if should_suppress_reopen(action, elapsed_since_focus_hide) {
        return Ok(());
    }

    match action {
        PopoverAction::Create => create_popover(app),
        PopoverAction::Show => {
            if let Some(window) = window {
                show_popover(app, &window)?;
            }
            Ok(())
        }
        PopoverAction::Hide => {
            if let Some(window) = window {
                hide_popover(app, &window)?;
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn sync_popover_chrome(window: &tauri::WebviewWindow, theme: &str) -> Result<(), String> {
    use tauri::window::Color;
    use window_vibrancy::clear_vibrancy;

    let _ = clear_vibrancy(window);

    let background = match theme {
        "light" => Color(245, 245, 247, 255),
        "dark" => Color(13, 17, 23, 255),
        other => return Err(format!("unknown theme: {other}")),
    };

    window
        .set_background_color(Some(background))
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn sync_popover_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    use tauri::{Manager, Theme};

    let native_theme = match theme.as_str() {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        other => return Err(format!("unknown theme: {other}")),
    };

    app.set_theme(native_theme);

    #[cfg(target_os = "macos")]
    if let Some(window) = app.get_webview_window(POPOVER_LABEL) {
        sync_popover_chrome(&window, &theme)?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn create_popover(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::utils::config::WindowConfig;
    use tauri::{Manager, WebviewWindowBuilder, WindowEvent};

    let config = WindowConfig {
        label: POPOVER_LABEL.to_owned(),
        title: "Portus".to_owned(),
        width: 380.0,
        height: 520.0,
        transparent: true,
        decorations: false,
        resizable: false,
        always_on_top: true,
        skip_taskbar: true,
        visible: false,
        ..WindowConfig::default()
    };
    let window = WebviewWindowBuilder::from_config(app, &config)?.build()?;

    let app_handle = app.clone();
    let focus_window = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Focused(false)) {
            if !should_hide_on_focus_loss(app_handle.state::<DialogGuard>().is_armed()) {
                return;
            }
            app_handle.state::<FocusHideGuard>().arm();
            if let Err(error) = hide_popover(&app_handle, &focus_window) {
                eprintln!("failed to hide Portus popover: {error}");
            }
        }
    });

    show_popover(app, &window)
}

#[cfg(target_os = "macos")]
fn show_popover(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use tauri::Manager;
    use tauri_plugin_positioner::{Position, WindowExt};

    window.move_window_constrained(Position::TrayBottomCenter)?;
    window.show()?;
    window.set_focus()?;
    app.state::<crate::state::PollState>().set_active();
    Ok(())
}

#[cfg(target_os = "macos")]
fn hide_popover(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use tauri::Manager;

    window.hide()?;
    app.state::<crate::state::PollState>().set_idle();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        should_hide_on_focus_loss, should_suppress_reopen, PopoverAction, PopoverState,
        TRAY_REOPEN_GUARD,
    };

    #[test]
    fn first_toggle_creates_the_popover() {
        let state = PopoverState;

        assert_eq!(state.toggle(false, false), PopoverAction::Create);
    }

    #[test]
    fn missing_window_is_recreated_after_a_previous_open() {
        let state = PopoverState;

        assert_eq!(state.toggle(false, false), PopoverAction::Create);
        assert_eq!(state.toggle(false, false), PopoverAction::Create);
    }

    #[test]
    fn toggle_hides_a_visible_created_popover() {
        let state = PopoverState;

        assert_eq!(state.toggle(true, true), PopoverAction::Hide);
    }

    #[test]
    fn toggle_shows_a_hidden_created_popover() {
        let state = PopoverState;

        assert_eq!(state.toggle(true, false), PopoverAction::Show);
    }

    #[test]
    fn losing_focus_hides_the_popover() {
        let state = PopoverState;

        assert_eq!(state.focus_changed(false), Some(PopoverAction::Hide));
        assert_eq!(state.focus_changed(true), None);
    }

    #[test]
    fn focus_loss_does_not_hide_popover_while_dialog_is_open() {
        assert!(!should_hide_on_focus_loss(true));
        assert!(should_hide_on_focus_loss(false));
    }

    #[test]
    fn recent_focus_loss_suppresses_the_tray_reopen() {
        assert!(should_suppress_reopen(
            PopoverAction::Show,
            Some(TRAY_REOPEN_GUARD - Duration::from_millis(1))
        ));
        assert!(!should_suppress_reopen(
            PopoverAction::Show,
            Some(TRAY_REOPEN_GUARD)
        ));
        assert!(!should_suppress_reopen(
            PopoverAction::Hide,
            Some(Duration::ZERO)
        ));
        assert!(!should_suppress_reopen(PopoverAction::Show, None));
    }

    #[test]
    fn configuration_has_no_startup_webview() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();

        assert_eq!(config["app"]["windows"].as_array().unwrap().len(), 0);
    }
}
