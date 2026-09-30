use crate::client::translate;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
use crate::ipc::Data;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
use hbb_common::tokio;
use hbb_common::{allow_err, log};
use std::sync::{Arc, Mutex};
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
use std::time::Duration;

pub fn start_tray() {
    if crate::ui_interface::get_builtin_option(hbb_common::config::keys::OPTION_HIDE_TRAY) == "Y" {
        #[cfg(not(target_os = "macos"))]
        {
            return;
        }
    }

    #[cfg(target_os = "linux")]
    crate::server::check_zombie();

    allow_err!(make_tray());
}

fn make_tray() -> hbb_common::ResultType<()> {
    // https://github.com/tauri-apps/tray-icon/blob/dev/examples/tao.rs
    use hbb_common::anyhow::Context;
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        TrayIcon, TrayIconBuilder, TrayIconEvent as TrayEvent,
    };
    // Tether: see `no_metal`.
    #[cfg(target_os = "macos")]
    let window_less = !crate::platform::macos::has_metal();
    #[cfg(not(target_os = "macos"))]
    let window_less = false;
    let icon;
    #[cfg(target_os = "macos")]
    {
        icon = include_bytes!("../res/mac-tray-dark-x2.png"); // use as template, so color is not important
    }
    #[cfg(not(target_os = "macos"))]
    {
        icon = include_bytes!("../res/tray-icon.ico");
    }

    let (icon_rgba, icon_width, icon_height) = {
        // macOS renders the tray icon as a template (alpha-only), so a full-color
        // Flutter asset would show as a solid blob. Use the dedicated monochrome
        // Tether template PNG instead. Other platforms prefer the bundled asset.
        #[cfg(target_os = "macos")]
        let image = image::load_from_memory(icon)
            .context("Failed to open icon path")?
            .into_rgba8();
        #[cfg(not(target_os = "macos"))]
        let image = load_icon_from_asset()
            .unwrap_or(image::load_from_memory(icon).context("Failed to open icon path")?)
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let icon = tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .context("Failed to open icon")?;

    let mut event_loop = EventLoopBuilder::new().build();

    let tray_menu = Menu::new();
    let hide_stop_service = crate::ui_interface::get_builtin_option(
        hbb_common::config::keys::OPTION_HIDE_STOP_SERVICE,
    ) == "Y";
    // The tray icon is only shown when the service is running, so we don't need to check
    // the `stop-service` option here.
    let quit_i = if !hide_stop_service {
        Some(MenuItem::new(translate("Stop service".to_owned()), true, None))
    } else {
        None
    };
    let open_i = if window_less {
        MenuItem::new("Show ID and password…", true, None)
    } else {
        MenuItem::new(translate("Open".to_owned()), true, None)
    };
    let id_i = MenuItem::new("ID: …", false, None);
    let password_i = MenuItem::new("One-time password: …", false, None);
    let set_password_i = MenuItem::new("Set permanent password…", true, None);
    let set_server_i = MenuItem::new("Set ID server…", true, None);
    if window_less {
        tray_menu
            .append_items(&[&id_i, &password_i, &PredefinedMenuItem::separator()])
            .ok();
    }
    tray_menu.append(&open_i).ok();
    if window_less {
        tray_menu.append_items(&[&set_password_i, &set_server_i]).ok();
    }
    if let Some(quit_i) = &quit_i {
        tray_menu.append(quit_i).ok();
    }
    let tooltip = |count: usize| {
        if count == 0 {
            format!(
                "{} {}",
                crate::get_app_name(),
                translate("Service is running".to_owned()),
            )
        } else {
            format!(
                "{} - {}\n{}",
                crate::get_app_name(),
                translate("Ready".to_owned()),
                translate("{".to_string() + &format!("{count}") + "} sessions"),
            )
        }
    };
    let mut _tray_icon: Arc<Mutex<Option<TrayIcon>>> = Default::default();

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayEvent::receiver();
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    let (ipc_sender, ipc_receiver) = std::sync::mpsc::channel::<Data>();
    // Tether: event-loop -> ipc-thread channel carrying "show this connection's CM
    // window" clicks, plus the dynamic per-connection menu items (MenuItem, conn_id)
    // and the last-seen list (to avoid rebuilding the menu every poll).
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    let (showcm_sender, showcm_receiver) = std::sync::mpsc::channel::<i32>();
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    let mut conn_items: Vec<(MenuItem, i32)> = Vec::new();
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    let mut last_conn: Vec<(i32, String, String)> = Vec::new();

    let open_func = move || {
        #[cfg(target_os = "macos")]
        if window_less {
            std::thread::spawn(no_metal::show_info);
            return;
        }
        if cfg!(not(feature = "flutter")) {
            crate::run_me::<&str>(vec![]).ok();
            return;
        }
        #[cfg(target_os = "macos")]
        crate::platform::macos::handle_application_should_open_untitled_file();
        #[cfg(target_os = "windows")]
        {
            // Do not use "start uni link" way, it may not work on some Windows, and pop out error
            // dialog, I found on one user's desktop, but no idea why, Windows is shit.
            // Use `run_me` instead.
            // `allow_multiple_instances` in `flutter/windows/runner/main.cpp` allows only one instance without args.
            crate::run_me::<&str>(vec![]).ok();
        }
        #[cfg(target_os = "linux")]
        {
            // Do not use "xdg-open", it won't read the config.
            if crate::dbus::invoke_new_connection(crate::get_uri_prefix()).is_err() {
                if let Ok(task) = crate::run_me::<&str>(vec![]) {
                    crate::server::CHILD_PROCESS.lock().unwrap().push(task);
                }
            }
        }
    };

    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    std::thread::spawn(move || {
        start_query_session_count(ipc_sender.clone(), showcm_receiver);
    });
    // Tether: keep the window-less menu's ID / one-time password current.
    #[cfg(target_os = "macos")]
    let (info_sender, info_receiver) = std::sync::mpsc::channel::<(String, String)>();
    #[cfg(target_os = "macos")]
    if window_less {
        std::thread::spawn(move || loop {
            if info_sender.send(no_metal::id_and_password()).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_secs(3));
        });
    }
    #[cfg(windows)]
    let mut last_click = std::time::Instant::now();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::EventLoopExtMacOS;
        event_loop.set_activation_policy(tao::platform::macos::ActivationPolicy::Accessory);
    }
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        );

        if let tao::event::Event::NewEvents(tao::event::StartCause::Init) = event {
            // for fixing https://github.com/rustdesk/rustdesk/discussions/10210#discussioncomment-14600745
            // so we start tray, but not to show it
            if crate::ui_interface::get_builtin_option(hbb_common::config::keys::OPTION_HIDE_TRAY) == "Y" {
                return;
            }
            // We create the icon once the event loop is actually running
            // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
            let mut builder = TrayIconBuilder::new()
                .with_menu(Box::new(tray_menu.clone()))
                .with_tooltip(tooltip(0))
                .with_icon(icon.clone());
            #[cfg(target_os = "macos")]
            {
                builder = builder.with_icon_as_template(true);
            }
            #[cfg(target_os = "windows")]
            {
                // Required since tray-icon 0.17
                // Fixes #15215, #15222, #15410
                builder = builder.with_menu_on_left_click(false);
            }
            let tray = builder.build();
            match tray {
                Ok(tray) => _tray_icon = Arc::new(Mutex::new(Some(tray))),
                Err(err) => {
                    log::error!("Failed to create tray icon: {}", err);
                }
            };

            // We have to request a redraw here to have the icon actually show up.
            // Tao only exposes a redraw method on the Window so we use core-foundation directly.
            #[cfg(target_os = "macos")]
            unsafe {
                use core_foundation::runloop::{CFRunLoopGetMain, CFRunLoopWakeUp};

                let rl = CFRunLoopGetMain();
                CFRunLoopWakeUp(rl);
            }
        }

        if let Ok(event) = menu_channel.try_recv() {
            if let Some(quit_i) = &quit_i {
                if event.id == quit_i.id() {
                    /* failed in windows, seems no permission to check system process
                    if !crate::check_process("--server", false) {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    */
                    if !crate::platform::uninstall_service(false, false) {
                        *control_flow = ControlFlow::Exit;
                    }
                } else if event.id == open_i.id() {
                    open_func();
                }
            } else if event.id == open_i.id() {
                open_func();
            }
            #[cfg(target_os = "macos")]
            if window_less {
                if event.id == set_password_i.id() {
                    std::thread::spawn(no_metal::set_permanent_password);
                } else if event.id == set_server_i.id() {
                    std::thread::spawn(no_metal::set_id_server);
                }
            }
            // Tether: clicking a per-connection entry asks the service to show that
            // connection's info (CM) window.
            #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
            if let Some((_, conn_id)) = conn_items.iter().find(|(mi, _)| event.id == *mi.id()) {
                log::info!("[tether-showcm] tray session entry clicked, conn_id={}", conn_id);
                let _ = showcm_sender.send(*conn_id);
            }
        }

        if let Ok(_event) = tray_channel.try_recv() {
            #[cfg(target_os = "windows")]
            match _event {
                TrayEvent::Click {
                    button,
                    button_state,
                    ..
                } => {
                    if button == tray_icon::MouseButton::Left
                        && button_state == tray_icon::MouseButtonState::Up
                    {
                        if last_click.elapsed() < std::time::Duration::from_secs(1) {
                            return;
                        }
                        open_func();
                        last_click = std::time::Instant::now();
                    }
                }
                _ => {}
            }
        }

        #[cfg(target_os = "macos")]
        if let Ok((id, password)) = info_receiver.try_recv() {
            id_i.set_text(format!("ID: {id}"));
            let password = if password.is_empty() { "-" } else { &password };
            password_i.set_text(format!("One-time password: {password}"));
        }

        #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
        if let Ok(data) = ipc_receiver.try_recv() {
            match data {
                Data::ControlledSessionCount(count) => {
                    _tray_icon
                        .lock()
                        .unwrap()
                        .as_mut()
                        .map(|t| t.set_tooltip(Some(tooltip(count))));
                }
                // Tether: rebuild the per-connection menu entries labeled "name(peer_id)".
                Data::ControlledSessions(list) => {
                    if list != last_conn {
                        last_conn = list.clone();
                        for (mi, _) in conn_items.drain(..) {
                            let _ = tray_menu.remove(&mi);
                        }
                        for (conn_id, name, peer_id) in list {
                            let item = MenuItem::new(format!("{}({})", name, peer_id), true, None);
                            let _ = tray_menu.append(&item);
                            conn_items.push((item, conn_id));
                        }
                    }
                }
                _ => {}
            }
        }
    });
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
#[tokio::main(flavor = "current_thread")]
async fn start_query_session_count(
    sender: std::sync::mpsc::Sender<Data>,
    showcm_receiver: std::sync::mpsc::Receiver<i32>,
) {
    let mut last_count = 0;
    loop {
        if let Ok(mut c) = crate::ipc::connect(1000, "").await {
            let mut timer = crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
            loop {
                tokio::select! {
                    res = c.next() => {
                        match res {
                            Err(err) => {
                                log::error!("ipc connection closed: {}", err);
                                break;
                            }

                            Ok(Some(Data::ControlledSessionCount(count))) => {
                                if count != last_count {
                                    last_count = count;
                                    sender.send(Data::ControlledSessionCount(count)).ok();
                                }
                            }
                            // Tether: forward the labeled connection list to the tray menu.
                            Ok(Some(Data::ControlledSessions(list))) => {
                                sender.send(Data::ControlledSessions(list)).ok();
                            }
                            _ => {}
                        }
                    }

                    _ = timer.tick() => {
                        // Tether: forward pending "show CM window" clicks to the service.
                        while let Ok(conn_id) = showcm_receiver.try_recv() {
                            log::info!("[tether-showcm] tray sending ShowCM({}) over ipc", conn_id);
                            c.send(&Data::ShowCM(conn_id)).await.ok();
                        }
                        c.send(&Data::ControlledSessionCount(0)).await.ok();
                    }
                }
            }
        }
        hbb_common::sleep(1.).await;
    }
}

/// Tether: window-less mode for Macs without Metal (see `platform::macos::has_metal`).
/// The Flutter window can't be created there, so the menu-bar icon offers the ID,
/// password and basic settings through AppleScript dialogs (no Metal needed).
/// Everything goes over IPC to the `--server` process, like the Flutter UI does.
#[cfg(target_os = "macos")]
pub mod no_metal {
    use hbb_common::log;
    use std::process::{Command, Stdio};

    const MIN_PASSWORD_LEN: usize = 6;

    /// Shows a `display dialog` and returns the clicked button, plus the typed
    /// text when `answer` is `Some((default, hidden))`. A button named "Cancel"
    /// makes AppleScript fail with -128, which returns `None`. Texts are passed
    /// as arguments so nothing needs escaping.
    fn dialog(
        message: &str,
        answer: Option<(&str, bool)>,
        buttons: &[&str],
    ) -> Option<(String, String)> {
        let button_list = (0..buttons.len())
            .map(|i| format!("item {} of argv", i + 3))
            .collect::<Vec<_>>()
            .join(", ");
        let mut show = format!(
            "set r to display dialog (item 1 of argv) with title \"Tether\" \
             buttons {{{button_list}}} default button {}",
            buttons.len()
        );
        if buttons.contains(&"Cancel") {
            show += " cancel button \"Cancel\"";
        }
        if let Some((_, hidden)) = answer {
            show += " default answer (item 2 of argv)";
            if hidden {
                show += " with hidden answer";
            }
        }
        let ret = if answer.is_some() {
            "return (button returned of r) & linefeed & (text returned of r)"
        } else {
            "return button returned of r"
        };
        let out = Command::new("osascript")
            .args(["-e", "on run argv", "-e", "tell me to activate", "-e"])
            .arg(show)
            .args(["-e", ret, "-e", "end run"])
            .arg(message)
            .arg(answer.map(|a| a.0).unwrap_or(""))
            .args(buttons)
            .output()
            .map_err(|e| log::error!("osascript failed: {e}"))
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let out = String::from_utf8_lossy(&out.stdout);
        let out: &str = &out;
        let out = out.strip_suffix('\n').unwrap_or(out);
        let (button, text) = out.split_once('\n').unwrap_or((out, ""));
        if button == "Cancel" {
            return None;
        }
        Some((button.to_owned(), text.to_owned()))
    }

    fn message(text: &str) {
        dialog(text, None, &["OK"]);
    }

    fn copy(text: &str) {
        let child = Command::new("pbcopy").stdin(Stdio::piped()).spawn();
        if let Ok(mut child) = child {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                stdin.write_all(text.as_bytes()).ok();
            }
            child.wait().ok();
        }
    }

    /// (ID, one-time password) from the running server.
    pub fn id_and_password() -> (String, String) {
        let id = crate::ipc::get_id();
        let password = crate::ipc::get_config("temporary-password")
            .ok()
            .flatten()
            .unwrap_or_default();
        (id, password)
    }

    pub fn show_info() {
        let (id, password) = id_and_password();
        let password = if password.is_empty() { "-".to_owned() } else { password };
        let text = format!(
            "Tether is running in the menu bar. This Mac's graphics can't show \
             the Tether window, but it can be controlled remotely.\n\n\
             ID: {id}\nOne-time password: {password}\n\n\
             Use the Tether icon in the menu bar to set a permanent password \
             or the ID server."
        );
        match dialog(&text, None, &["Copy password", "Copy ID", "OK"]) {
            Some((b, _)) if b == "Copy ID" => copy(&id),
            Some((b, _)) if b == "Copy password" => copy(&password),
            _ => {}
        }
    }

    pub fn set_permanent_password() {
        let Some((_, first)) = dialog(
            &format!(
                "New permanent password (at least {MIN_PASSWORD_LEN} characters). \
                 Leave empty to remove it and use only the one-time password."
            ),
            Some(("", true)),
            &["Cancel", "OK"],
        ) else {
            return;
        };
        if !first.is_empty() {
            if first.chars().count() < MIN_PASSWORD_LEN {
                message(&format!(
                    "The password must be at least {MIN_PASSWORD_LEN} characters."
                ));
                return;
            }
            let Some((_, second)) =
                dialog("Enter the password again.", Some(("", true)), &["Cancel", "OK"])
            else {
                return;
            };
            if first != second {
                message("The passwords don't match. Nothing was changed.");
                return;
            }
        }
        match crate::ipc::set_permanent_password(first.clone()) {
            Ok(()) if first.is_empty() => message("The permanent password was removed."),
            Ok(()) => message("The permanent password was set."),
            Err(err) => message(&format!("Failed to set the password: {err}")),
        }
    }

    pub fn set_id_server() {
        let mut options = crate::ipc::get_options();
        let current = |k: &str| options.get(k).cloned().unwrap_or_default();
        let (host, key) = (current("custom-rendezvous-server"), current("key"));
        let Some((_, host)) = dialog(
            "ID server (host name or IP address). Leave empty to use the default server.",
            Some((host.as_str(), false)),
            &["Cancel", "Next"],
        ) else {
            return;
        };
        let host = host.trim().to_owned();
        let key = if host.is_empty() {
            String::new()
        } else {
            let Some((_, key)) = dialog(
                "Key: the contents of id_ed25519.pub on the server.",
                Some((key.as_str(), false)),
                &["Cancel", "Save"],
            ) else {
                return;
            };
            key.trim().to_owned()
        };
        // The relay defaults to the ID server's host when left empty.
        for (k, v) in [
            ("custom-rendezvous-server", host.clone()),
            ("key", key),
            ("relay-server", String::new()),
        ] {
            if v.is_empty() {
                options.remove(k);
            } else {
                options.insert(k.to_owned(), v);
            }
        }
        match crate::ipc::set_options(options) {
            Ok(()) if host.is_empty() => message("Tether now uses the default server."),
            Ok(()) => message(&format!("Tether now uses the ID server {host}.")),
            Err(err) => message(&format!("Failed to save the server: {err}")),
        }
    }
}

fn load_icon_from_asset() -> Option<image::DynamicImage> {
    let Some(path) = std::env::current_exe().map_or(None, |x| x.parent().map(|x| x.to_path_buf()))
    else {
        return None;
    };
    #[cfg(target_os = "macos")]
    let path = path.join("../Frameworks/App.framework/Resources/flutter_assets/assets/icon.png");
    #[cfg(windows)]
    let path = path.join(r"data\flutter_assets\assets\icon.png");
    #[cfg(target_os = "linux")]
    let path = path.join(r"data/flutter_assets/assets/icon.png");
    if path.exists() {
        if let Ok(image) = image::open(path) {
            return Some(image);
        }
    }
    None
}
