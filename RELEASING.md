# Releasing FarmBuddy (Windows, Mac, Android)

GitHub builds every installer for you. You never build on your own computer:
one command pushes the code and a version tag, and about 30–40 minutes later
the release page has the Windows `.exe`/`.msi`, the Mac `.dmg` and the Android `.apk`.

Repository: **https://github.com/treasure155/farmbuddy**
Releases (downloads): **https://github.com/treasure155/farmbuddy/releases**

---

## 1. One time: GitHub

1. Sign in to GitHub as `treasure155` and create an **empty** repository named `farmbuddy`
   (New repository → name `farmbuddy` → Private or Public → do **not** add a README).
   Installed apps download updates from its Releases, so if it is private, make the
   releases reachable (a public repository is the simple choice; the code needs no secrets).
2. Install the GitHub CLI once and sign in (it lets the key tool save the secrets for you):

```bash
# Windows (Git Bash):   winget install GitHub.cli      then close and reopen Git Bash
# Mac:                  brew install gh
gh auth login            # GitHub.com → HTTPS → Login with a web browser
```

## 2. One time: the keys (admin computer only)

```bash
cd ~/Desktop/farmbuddy-v1.0.5/farmbuddy
npm install
npm run setup-keys
```

`setup-keys` makes the keys in `~/.farmbuddy-keys/` (Windows: `C:\Users\<you>\.farmbuddy-keys\`),
writes the public halves into the project, and saves these GitHub secrets for you
(or prints them to paste under GitHub → farmbuddy → Settings → Secrets and variables → Actions → New repository secret):

| Secret | What it is | Required? |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Update signing key (installed apps only accept updates signed with it) | **Yes** |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password | **Yes** |
| `ANDROID_KEYSTORE_BASE64` | Android app signing key (`android-release.p12`, base64) | **Yes** (for the APK) |
| `ANDROID_KEYSTORE_PASSWORD` | Its password | **Yes** (for the APK) |

It never replaces a key that already exists. **Back up `~/.farmbuddy-keys/` to a flash drive.**
If it is lost you can no longer issue licences, update installed apps, or update the Android app.

Check the secrets are there: `gh secret list --repo treasure155/farmbuddy`

## 3. Every release: one command

1. Raise the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` (all three the same, e.g. `1.0.6`).
   Each version can be released once.
2. Run:

```bash
npm run release
```

The first time, this also turns the folder into a git repository linked to
`treasure155/farmbuddy` and pushes everything (Git asks you to sign in to GitHub once).
Then it commits, tags `vX.Y.Z` and pushes. Watch it under GitHub → **Actions** → "Build and release".

When it is green, open **Releases** → `FarmBuddy vX.Y.Z` and download:

| Platform | File |
|---|---|
| Windows | `FarmBuddy_X.Y.Z_x64-setup.exe` (or `FarmBuddy_X.Y.Z_x64_en-US.msi`) |
| Mac (Apple Silicon and Intel) | `FarmBuddy_X.Y.Z_universal.dmg` |
| Android | `FarmBuddy_X.Y.Z_android.apk` (arrives a few minutes after the others) |

If one job fails, open it, fix the cause, then use **Re-run failed jobs** (never "Re-run all jobs" on a published release).
The Android APK alone can be rebuilt from Actions → "Android APK" → Run workflow.

## 4. Licence keys for customers (how to get a key)

Each computer needs its own key. FarmBuddy runs a 14-day trial, then asks for one.

1. The customer opens **Settings → Licence** (or the activation screen) and sends you the
   **Install ID**, e.g. `P0NP-K8GF-6R3E-QB45`.
2. On the admin computer (the one with `~/.farmbuddy-keys/`):

```bash
npm run license -- issue P0NP-K8GF-6R3E-QB45                                   # never expires
npm run license -- issue P0NP-K8GF-6R3E-QB45 --days 365 --note "Mr Okon, Etinan"  # one year
npm run license -- list                                                        # all keys issued
```

3. Send back the `FMBD-…` key (WhatsApp is fine). The customer pastes it into Settings → Licence.
   It only works on the computer with that Install ID. Every key is logged in
   `~/.farmbuddy-keys/issued-licences.csv`.

Keys only work in apps built **after** `setup-keys`, because the app must contain your public key.
Apps from the GitHub release always have it.

## 5. Signing the apps

There are two different kinds of signing:

* **Update and licence signing** (sections 2 and 4): free, done by `setup-keys`, already required.
* **Code signing by the operating system's publisher programme**: optional. It removes the
  "Unknown publisher" warnings. The app works without it. Add it when you are ready; the
  workflow picks it up automatically from the secrets below.

### Android: signed already

`setup-keys` creates `~/.farmbuddy-keys/android-release.p12` and every APK is signed with it.
Phones only install an update over an older copy if both are signed with the **same** key, so
never make a new one. Users tap the APK, allow "Install unknown apps" for their browser or
file manager once, and install. (For the Google Play Store later you would upload an `.aab`;
the same key can be used as the upload key.)

To set the secrets by hand:

```bash
cd ~/.farmbuddy-keys
base64 -w0 android-release.p12 > android.b64        # Mac: base64 -i android-release.p12 -o android.b64
gh secret set ANDROID_KEYSTORE_BASE64   --repo treasure155/farmbuddy < android.b64
gh secret set ANDROID_KEYSTORE_PASSWORD --repo treasure155/farmbuddy < android-keystore-password.txt
rm android.b64
```

### Windows: optional code-signing certificate

Unsigned: Windows SmartScreen shows "Windows protected your PC" the first time.
Users click **More info → Run anyway**. Everything else works.

To sign, you need a code-signing certificate in the name of TechAlpha (from a certificate
authority such as Sectigo, DigiCert, SSL.com or Certum). Since June 2023 new certificates are
usually delivered on a USB token or in the authority's cloud, not as a `.pfx` file you can copy.
So pick one of these:

* **You have a `.pfx` file** (exportable certificate): add two secrets and you are done:

```bash
base64 -w0 techalpha-codesign.pfx > win.b64
gh secret set WINDOWS_CERTIFICATE          --repo treasure155/farmbuddy < win.b64
gh secret set WINDOWS_CERTIFICATE_PASSWORD --repo treasure155/farmbuddy    # type the .pfx password
rm win.b64
```

  The workflow imports it, signs the `.exe` and `.msi` with SHA-256 and a timestamp, and deletes the file.
* **Cloud signing** (e.g. Microsoft Azure Trusted Signing, SSL.com eSigner, or your authority's
  cloud service): check you are eligible in your country first. Tauri can call the signing tool
  through `bundle.windows.signCommand`; send me the service you choose and I will wire it in.
* **USB token only**: download the unsigned installer from the release and sign it on the
  computer with the token: `signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /a FarmBuddy_X.Y.Z_x64-setup.exe`.
  (Signing after release does not change the update files, which are signed separately.)

### Mac: optional Apple Developer ID (signing and notarisation)

Unsigned: the first time, users **right-click the app → Open → Open**
(or System Settings → Privacy & Security → **Open Anyway**).

To remove that step you need the **Apple Developer Program** (developer.apple.com/programs, yearly fee) and a Mac:

1. Xcode → Settings → Accounts → your Apple ID → Manage Certificates → **+** → **Developer ID Application**.
2. Keychain Access → My Certificates → right-click "Developer ID Application: …" → **Export** → `developer-id.p12`, set a password.
3. appleid.apple.com → Sign-In and Security → **App-Specific Passwords** → create one (for notarisation).
4. Your Team ID is on developer.apple.com → Account → Membership.
5. Add the secrets:

```bash
base64 -i developer-id.p12 -o mac.b64
gh secret set APPLE_CERTIFICATE          --repo treasure155/farmbuddy < mac.b64
gh secret set APPLE_CERTIFICATE_PASSWORD --repo treasure155/farmbuddy        # the .p12 password
gh secret set APPLE_SIGNING_IDENTITY     --repo treasure155/farmbuddy        # e.g. Developer ID Application: TechAlpha Global Concept (AB12CD34EF)
gh secret set APPLE_ID                   --repo treasure155/farmbuddy        # your Apple ID email
gh secret set APPLE_PASSWORD             --repo treasure155/farmbuddy        # the app-specific password
gh secret set APPLE_TEAM_ID              --repo treasure155/farmbuddy        # e.g. AB12CD34EF
rm mac.b64
```

The next release is signed and notarised; Mac users then open it with a normal double-click.

## 6. What never goes to GitHub

`~/.farmbuddy-keys/` (all private keys and passwords), `.pfx`/`.p12` files, `secrets/`.
They are listed in `.gitignore`. Only the **public** halves are in the project
(`src-tauri/license_public_key.txt`, `src-tauri/content_public_key.txt`, the updater `pubkey`).
