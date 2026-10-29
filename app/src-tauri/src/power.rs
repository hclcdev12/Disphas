//! Sleep to thaw a frozen drive (B1, UX review D5). Most firmwares freeze the
//! ATA security of SATA drives at boot; a sleep cuts their power and they
//! come back thawed. The app asks systemd-logind on the local system bus,
//! as the desktop's own sleep menu does, then re-inspects the drives when
//! logind announces the resume. Nothing here reaches the network.
//!
//! Development builds never put the machine to sleep: they simulate a short
//! one, so the demo can be followed without suspending the developer's PC.

use tauri::Emitter;

/// Event sent to the interface when the machine wakes up.
pub const RESUMED: &str = "disphas://resumed";

#[cfg(all(target_os = "linux", not(debug_assertions)))]
mod logind {
    use zbus::blocking::{Connection, Proxy};

    fn manager(connection: &Connection) -> zbus::Result<Proxy<'_>> {
        Proxy::new(
            connection,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
    }

    /// `yes`, or `challenge` (an administrator password is asked), means possible.
    pub fn can_suspend() -> bool {
        let answer = Connection::system()
            .and_then(|connection| manager(&connection)?.call::<_, _, String>("CanSuspend", &()));
        matches!(answer.as_deref(), Ok("yes" | "challenge"))
    }

    pub fn suspend() -> Result<(), String> {
        let connection = Connection::system().map_err(|error| error.to_string())?;
        manager(&connection)
            .and_then(|proxy| proxy.call::<_, _, ()>("Suspend", &(true,)))
            .map_err(|error| error.to_string())
    }

    /// Calls `on_resume` each time logind announces the end of a sleep.
    pub fn watch(on_resume: impl Fn() + Send + 'static) {
        std::thread::spawn(move || {
            let Ok(connection) = Connection::system() else {
                return;
            };
            let Ok(proxy) = manager(&connection) else {
                return;
            };
            let Ok(signals) = proxy.receive_signal("PrepareForSleep") else {
                return;
            };
            for message in signals {
                // `true` before the sleep, `false` after the wake-up.
                if message.body().deserialize::<bool>() == Ok(false) {
                    on_resume();
                }
            }
        });
    }
}

/// Whether this machine can be put to sleep from the app.
pub fn can_suspend() -> bool {
    #[cfg(all(target_os = "linux", not(debug_assertions)))]
    {
        logind::can_suspend()
    }
    #[cfg(debug_assertions)]
    {
        true
    }
    #[cfg(all(not(target_os = "linux"), not(debug_assertions)))]
    {
        false
    }
}

/// Puts the machine to sleep. Returns once the request is accepted; the
/// wake-up arrives later as the [`RESUMED`] event.
pub fn suspend(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(all(target_os = "linux", not(debug_assertions)))]
    {
        let _ = app;
        logind::suspend()
    }
    #[cfg(debug_assertions)]
    {
        // Simulated: a few seconds "asleep", then the wake-up.
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let _ = app.emit(RESUMED, ());
        });
        Ok(())
    }
    #[cfg(all(not(target_os = "linux"), not(debug_assertions)))]
    {
        let _ = app;
        Err("sleep is not available on this system".to_owned())
    }
}

/// Forwards every wake-up announced by the system to the interface.
pub fn watch(app: &tauri::AppHandle) {
    #[cfg(all(target_os = "linux", not(debug_assertions)))]
    {
        let app = app.clone();
        logind::watch(move || {
            let _ = app.emit(RESUMED, ());
        });
    }
    #[cfg(debug_assertions)]
    {
        let _ = app;
    }
    #[cfg(all(not(target_os = "linux"), not(debug_assertions)))]
    {
        let _ = app;
    }
}
