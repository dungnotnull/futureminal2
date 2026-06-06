// Futureminal — Open-source terminal built on Warp's core engine.
// This binary uses Warp's OSS channel configuration with all cloud
// features disabled.

#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig};
use warp_core::AppId;

fn main() -> Result<()> {
    // Futureminal uses the OSS channel with ALL cloud/telemetry disabled.
    let state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("dev", "futureminal", "Futureminal"),
            logfile_name: "futureminal.log".into(),
            server_config: WarpServerConfig::production(),
            oz_config: OzConfig::production(),
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    );
    ChannelState::set(state);

    // Initialize Futureminal-specific features
    futureminal_init()?;

    warp::run()
}

fn futureminal_init() -> Result<()> {
    // TODO: Initialize blockchain audit logger
    // TODO: Initialize multi-provider AI router
    // TODO: Initialize plugin host
    Ok(())
}

#[cfg(all(not(feature = "extern_plist"), target_os = "macos"))]
embed_plist::embed_info_plist_bytes!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>futureminal</string>
    <key>CFBundleIdentifier</key>
    <string>dev.futureminal.app</string>
    <key>CFBundleName</key>
    <string>Futureminal</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    </dict>
    </plist>
"#);
