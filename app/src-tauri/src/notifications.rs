//! Desktop notifications, Linux only: the freedesktop service on the session
//! D-Bus (GNOME, KDE...). Local to the desktop, never the network.

/// Keeps a shown notification on screen: dropping it closes its D-Bus
/// connection, and GNOME then withdraws the notification.
#[cfg(target_os = "linux")]
pub struct Handle(#[allow(dead_code)] notify_rust::NotificationHandle);

/// Nothing to keep where notifications are not supported.
#[cfg(not(target_os = "linux"))]
pub struct Handle;

/// Shows a notification from disphas.
#[cfg(target_os = "linux")]
pub async fn show(title: &str, body: &str) -> Result<Handle, String> {
    notify_rust::Notification::new()
        .appname("disphas")
        .summary(title)
        .body(body)
        .show_async()
        .await
        .map(Handle)
        .map_err(|error| error.to_string())
}

/// Notifications are not supported on this system.
#[cfg(not(target_os = "linux"))]
pub async fn show(_title: &str, _body: &str) -> Result<Handle, String> {
    Err("desktop notifications are only supported on Linux".to_owned())
}
