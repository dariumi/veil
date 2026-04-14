#!/usr/bin/env bash
# Apply Android-specific patches after `cargo tauri android init`.
# Run from the repo root: bash veil-app/android/patch.sh
set -euo pipefail

ANDROID_DIR="veil-app/src-tauri/gen/android"
APP_SRC="$ANDROID_DIR/app/src/main"

echo "→ Copying VeilVpnService.kt"
mkdir -p "$APP_SRC/java/com/veilproject/veil"
cp veil-app/android/VeilVpnService.kt \
   "$APP_SRC/java/com/veilproject/veil/VeilVpnService.kt"

echo "→ Patching AndroidManifest.xml"
MANIFEST="$APP_SRC/AndroidManifest.xml"

# Add permissions before </manifest>
PERMISSIONS='    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />\n    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />'
sed -i "s|</manifest>|$PERMISSIONS\n</manifest>|" "$MANIFEST"

# Add VeilVpnService before </application>
SERVICE='        <service\n            android:name="com.veilproject.veil.VeilVpnService"\n            android:permission="android.permission.BIND_VPN_SERVICE"\n            android:exported="true"\n            android:foregroundServiceType="specialUse">\n            <intent-filter>\n                <action android:name="android.net.VpnService" \/>\n            <\/intent-filter>\n            <property\n                android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"\n                android:value="VPN tunnel" \/>\n        <\/service>'
sed -i "s|</application>|$SERVICE\n    </application>|" "$MANIFEST"

echo "→ Ensuring androidx-core dependency in app/build.gradle.kts"
GRADLE="$ANDROID_DIR/app/build.gradle.kts"
if ! grep -q "androidx.core" "$GRADLE"; then
    sed -i 's/dependencies {/dependencies {\n    implementation("androidx.core:core-ktx:1.12.0")/' "$GRADLE"
fi

echo "✓ Android patches applied"
