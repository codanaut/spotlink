# ![spotlink](spotlink-desktop/src-tauri/icons/128x128.png) SpotLink

SpotLink is a lightweight, cross-platform application designed to display your bidirectional matches from PSK Reporter in real time.

By leveraging PSK Reporter's MQTT stream, SpotLink monitors for your callsign in both incoming and outgoing signal reports, displaying bidirectional matches in a clean, easy-to-read list.
## 🚀 Features

   - Real-Time Stream: Uses MQTT to catch spots instantly.

   - Rolling 5-Minute Window: Reports are kept for 5 minutes before being automatically cleared, ensuring you only see active, recent matches.

   - Top Matches: Stations with strong signal reports—indicating a high probability of a successful QSO—are automatically marked with a star (⭐).

   - Ultra-Lightweight: Designed specifically not to slow down or interfere with older machines running WSJT-X. It can be run on your radio PC or any other machine on your network.

   - Note: The goal of SpotLink isn't to replace the PSK Reporter map, but to complement it by giving you a quick, glanceable count of how many stations you hear, how many hear you, and how many matches go both ways.

## 🛠️ Requirements

The only requirement is that you must be actively uploading your spots to PSK Reporter so the system can log the stations you see. Make sure "Enable Spotting" is turned on in your WSJT-X settings!

## 💾 Downloads

Try it out by downloading the latest release for your operating system from the [Releases](https://github.com/codanaut/spotlink/releases/tag/latest) page.

### 💻 Desktop App

Windows has an installer or a portable version available.

Linux has packages for Debian/Ubuntu(.deb) and Fedora(.rpm) along with an AppImage. AUR support can be added if there's further interest.


### 📱 iOS Build (via SideStore)
The iOS builds require [SideStore](https://sidestore.io/) for sideloading on your iOS device. 

Theres currently two iOS options available:

The stable version can be downloaded [here](https://github.com/codanaut/spotlink/releases/tag/latest).

The beta version can be downloaded [here](https://github.com/codanaut/spotlink/releases/tag/ios-latest).


> 💡 An official version for the Apple App Store will hopefully be available down the road once I can justify paying Apple $99 a year for the developer license!

### 🤖 Android Build

Coming soon!