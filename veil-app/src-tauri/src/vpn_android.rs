// Android VPN bridge: Kotlin VeilVpnService → Rust via JNI
//
// Flow:
//   1. JS calls `start_vpn` Tauri command
//   2. Rust starts VeilConnection + SOCKS5 relay
//   3. VeilVpnService.kt creates TUN via VpnService.Builder
//   4. Kotlin calls JNI onTunReady(fd) → Rust receives raw fd
//   5. Rust runs IP packet pump: TUN fd → VeilConnection relay

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub static VPN_RUNNING: AtomicBool = AtomicBool::new(false);

/// Channel: Kotlin delivers the TUN fd here after VpnService.Builder.establish().
pub static TUN_FD_SENDER: Mutex<Option<std::sync::mpsc::SyncSender<i32>>> = Mutex::new(None);

pub async fn start(server: String, token: String) -> anyhow::Result<()> {
    use veil_client::transport::VeilConnection;

    if VPN_RUNNING.swap(true, Ordering::SeqCst) {
        anyhow::bail!("VPN already running");
    }

    let conn = Arc::new(VeilConnection::connect(&server, &token, "default").await?);
    tracing::info!("Connected to Veil server");

    // Create channel and wait for Kotlin to deliver TUN fd
    let (tx, rx) = std::sync::mpsc::sync_channel::<i32>(1);
    *TUN_FD_SENDER.lock().unwrap() = Some(tx);

    let fd = tokio::task::spawn_blocking(move || {
        rx.recv().map_err(|_| anyhow::anyhow!("TUN fd channel closed"))
    })
    .await??;

    tracing::info!(fd = fd, "TUN fd received, starting packet pump");

    veil_client::tunnel::android::run(fd, conn).await?;

    VPN_RUNNING.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn stop() {
    VPN_RUNNING.store(false, Ordering::SeqCst);
    *TUN_FD_SENDER.lock().unwrap() = None;
}

// ── JNI callbacks (called from Kotlin) ───────────────────────────────────────

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_com_veilproject_veil_VeilVpnService_onTunReady(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    fd: jni::sys::jint,
) {
    tracing::info!(fd = fd, "JNI: TUN fd received from Kotlin");
    if let Some(tx) = TUN_FD_SENDER.lock().unwrap().as_ref() {
        let _ = tx.try_send(fd);
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_com_veilproject_veil_VeilVpnService_onVpnRevoked(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    tracing::warn!("JNI: VPN revoked by Android system");
    stop();
}
