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

#[cfg(target_os = "macos")]
const POPOVER_LABEL: &str = "main";

#[cfg(target_os = "macos")]
pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::tray::TrayIconBuilder;
    use tauri::ActivationPolicy;

    app.set_activation_policy(ActivationPolicy::Accessory);

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
fn create_popover(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::utils::config::WindowConfig;
    use tauri::{WebviewWindowBuilder, WindowEvent};

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
    use super::{PopoverAction, PopoverState};

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
    fn configuration_has_no_startup_webview() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();

        assert_eq!(config["app"]["windows"].as_array().unwrap().len(), 0);
    }
}
