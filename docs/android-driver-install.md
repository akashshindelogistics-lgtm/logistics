# Installing the driver app (Android)

The Logistics Driver app lets a driver's phone report its location, so the
vehicle assigned to them moves on the dispatch map without a GPS tracker. It
isn't on the Play Store: the backend serves the APK at a fixed link, and the
driver installs it directly. This guide is for the office and the driver together; the first
install takes about ten minutes.

Needs Android 8.0 or newer, and location (GPS) switched on.

## Before you start (office)

1. **The download:** open the driver's page on the dashboard (from the
   organization's driver list). The **Driver app (Android)** card has a QR
   code the driver scans with the phone's camera, the download link to send
   over WhatsApp or SMS instead, and the server address. The link is the same
   for every release. If the card says nothing is published, build and publish
   a release first ("Building a release" below).
2. **The server address:** the `https://` address of the backend, the same one
   the dashboard uses for its API.
3. **A pairing code for the driver:** the driver must exist, be active and
   have a vehicle assigned (Vehicles > assign driver). On the driver's page,
   an Admin or Dispatcher taps **Pair a phone** in the **Driver's phone**
   panel. It shows a QR code for the app to scan, and the server address and
   device token (a code like `1b4e28ba-2fa1-11d2-883f-0016d3cca427`) to type
   instead. They are shown only once. Treat the token like a password: anyone
   with it can move that driver's vehicle on the map.
   - **Lost or changed phone:** tap **Pair a new phone**; the old phone stops
     reporting at once.
   - **Driver left:** tap **Unpair**.
   - **Has it worked?** The panel shows when the phone last reported.

## Install (driver)

1. Scan the QR code or open the link; the browser downloads
   `logistics-driver-<version>.apk`. Open it from the download notification,
   the browser's downloads or the Files app.
2. Android asks to allow installing apps from that app ("Install unknown
   apps"). Tap **Settings**, switch on **Allow from this source**, go back and
   tap **Install**.
3. If **Play Protect** warns that the app is unknown or wasn't scanned, tap
   **More details > Install anyway**. This happens because the app is not from
   the Play Store. If it offers to scan the app, scanning is fine.
4. Open **Logistics Driver**: the white location pin on a black icon.

## Pair the phone

1. Tap **Scan QR code** and scan the code on the office's screen. The app
   fills in the server address and device token. Check the server address is
   your company's.
   - The first scan may take a moment while Google Play services sets up the
     scanner.
   - If the phone's camera app opens the code instead, it offers to open it
     in Logistics Driver; that fills in the form the same way.
   - No scanner? Type both: the token's capital letters and a pasted
     `Bearer ` in front don't matter.
2. Tap **Pair**. The app never pairs from a code by itself. The app checks both with the server and shows your name,
   company and vehicle. If it says the token wasn't recognised, ask the office
   for a new one.

## Share your location

1. Switch on **Share my location**.
2. Android asks for location access: choose **While using the app** (and
   **Precise**). Then allow notifications: the app shows one while sharing,
   with a **Stop** button.
3. The app explains battery optimisation the first time. Tap **Open settings**
   and set Logistics Driver to **Unrestricted** / **Don't optimise**.
   Otherwise the phone may stop sharing a few minutes after the screen goes off.
4. Many phones need one more setting, also shown in the app:

   | Phone | Setting |
   | --- | --- |
   | Xiaomi, Redmi, Poco | Settings > Apps > Manage apps > Logistics Driver: turn on **Autostart**, and set **Battery saver** to "No restrictions" |
   | Vivo, iQOO | Settings > Battery > Background power consumption management: allow Logistics Driver **high background power use** |
   | Oppo, Realme, OnePlus | Settings > Apps > Logistics Driver > Battery usage: turn on **Allow background activity** and **Allow auto launch** |
   | Samsung | Settings > Battery > Background usage limits: add Logistics Driver to **Never sleeping apps** |
   | Huawei, Honor | Settings > Battery > App launch: find Logistics Driver, switch to manage manually and allow all three options |

   Menu names vary a little between phone models and Android versions.

While sharing, the app sends a position about every 30 seconds while the phone
moves, also with the screen off. With no signal, positions are kept on the
phone (up to about a day's worth) and sent when the network is back.

## Checking it works

On the app's status screen:

- **Queued fixes** is 0, or falls back to 0, when there's signal.
- **Last sync** is recent and says "Reported to <vehicle>".
- **Last position** under the vehicle updates.

On the dashboard, the vehicle's position moves with the phone.

If **Last sync** shows a message in red:

| Message | What to do |
| --- | --- |
| No internet connection | Nothing; positions upload when the network is back |
| The device token was rejected | The office issued a new token. Unpair, and pair with the new one |
| You're marked inactive | The office reactivates the driver |
| No vehicle is assigned to you yet | The office assigns a vehicle |

**After the phone restarts**, a notification says "Tap to resume". Tap it and
then **Resume sharing**. Android doesn't let the app start sharing location by
itself after a restart.

## Updating the app

Once the office publishes a newer release, the app shows **Update available**
at the top of the status screen. It checks when opened, when brought back to
the screen, and on **Refresh**. Tap **Download update**, then open the
download and tap **Update**. It installs over the old app the same way as the
first time. Pairing and queued positions are kept, so there's no need to
uninstall. The status
screen shows the **App version**, so the office can see who needs the update.
If Android says the app "conflicts with an existing package", the APK was
signed with a different key. Don't uninstall: ask whoever built it to use the
release keystore.

## Building a release (office / developer)

Once only, create the signing key outside the repo and back it up, with its
password, somewhere other than this machine:

```sh
android/scripts/make-release-keystore.sh   # ~/.config/logistics-driver/release.jks
```

Losing it means every driver has to uninstall and pair again, because Android
installs an update only if it is signed with the same key.

For each release:

1. Raise `versionCode` by one, and set `versionName`, in
   `android/app/build.gradle.kts`.
2. Build it, and publish it into a local folder:

   ```sh
   DRIVER_KEYSTORE=~/.config/logistics-driver/release.jks PUBLISH_DIR=/tmp/driver-app \
       android/scripts/build-release.sh
   ```

   It runs the unit tests and lint and builds the signed APK. It also checks
   the signing certificate against `android/release-cert.sha256`; the first
   release creates that file, which you commit. The APK is written to
   `android/build/release/logistics-driver-<version>.apk`, and
   `/tmp/driver-app` gets the APK and its `latest.json`.
3. Put it on the server's uploads volume (the backend reads
   `DRIVER_APP_DIR`, by default `driver-app` under `UPLOAD_DIR`):

   ```sh
   # on the server, after copying /tmp/driver-app there (e.g. with scp)
   cd ~/logistics-deploy && docker compose --env-file .env cp /tmp/driver-app/. api:/app/uploads/driver-app/
   ```

   Check it with `curl https://<server>/api/driver-app/latest`. Phones offer
   the update the next time the app is opened. A backend run with
   `cargo run` reads `./uploads/driver-app`, so locally publish straight
   there: `PUBLISH_DIR=./uploads/driver-app`.

`android/scripts/publish-apk.sh <apk> <dir>` does step 2's publishing on its
own, for an APK that's already built. It refuses an APK whose `versionCode`
isn't above the one already published, because phones wouldn't install it as
an update.
