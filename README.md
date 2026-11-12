# FarmBuddy

**Your Farm. Your Business. Your Assistant.**

FarmBuddy is an offline agricultural management and advisory system by **TechAlpha Hub / TechAlpha Global Concept**, for Nigerian farmers and agricultural businesses. It runs on Windows and macOS (Android build workflow included) and keeps every record on the device, so it works without internet.

It follows the eStock Manager v3.4.0 architecture (as the specification requires): **Tauri 2 + Rust + SQLite** for the backend and **TypeScript + Vite** for the interface. It has its own database file (`farmbuddy.sqlite3`), app ID (`online.techalphahub.farmbuddy`), first-run farm setup and visual design; there are no shop, eatery or laundry business types. All database work, validation, permissions, stock changes and money calculations happen in Rust; money is stored as whole kobo.

## Status: version 1.0.5

### What changed in 1.0.5
- **Releases:** `RELEASING.md` (GitHub setup, keys, licence keys, signing). `npm run release` now creates the git repository and links it to `treasure155/farmbuddy` the first time; the release workflow signs Windows and Mac builds when `WINDOWS_CERTIFICATE` / `APPLE_*` secrets are set.
- **Windows fix:** `npm run tauri dev` no longer stops with `EBUSY: resource busy or locked … src-tauri\target\…` (Vite now ignores `src-tauri/`; `vite.config.ts`).
- **Runs and sales are two separate flows** (Processing page, `palm_oil.rs`, migration 010). The page opens with three plain choices: **Start a processing run**, **Record a harvest**, **Record a sale**, and an **In your barn** list of open runs with what is left.
- **Processing run questions:** bought / from my farm / both; how you buy it (bag, kg, basin…); price of one; how many; transport; which mill and the fee; **workers by name and pay**; **other expenses by name and price**; garri, fufu, flour or palm oil; how you sell it; how much you got (any measure).
- **Harvest runs** (short form, no plots needed) for cucumber, pepper, vegetables (ugu, waterleaf…), melon (egusi), water yam (ete) and yam: date, where, transport, workers, other expenses, how you sell it, how much you harvested.
- **Selling measures entered once:** tick every way you sell (bag, half bag, quarter bag, paint bucket, custard bucket, derica, cup, kg, keg, litre, basket, heap, tuber… or your own), today's price, and how many make one stock unit (e.g. 120 cups = 1 bag). Saved per product (`sell_measures`) and filled in next time. Only sizes that are true by definition (half bag = 2, quarter bag = 4, 25 litres = 1 keg) are pre-filled; market sizes are left to the farmer.
- **Sales form:** which run (with what is left), which product (oil or kernel), measure, how many, price of one (pre-filled), buyer and phone, how paid (cash, transfer, POS, part or not paid: the rest is owed), transportation and miscellaneous. Each sale makes a **receipt** (print or WhatsApp), takes the right fraction out of stock and counts against its run (`run_sales`). You cannot sell more than the run has left.
- **The barn balances:** a run puts everything it made into Farm stock at its cost per unit; sales take it out; own raw material is taken out of stock; editing moves only the difference. When a run is sold out it closes and its **profit or loss is final** (sale transport and misc count as run costs). Voiding a sale's receipt gives the stock back and reopens the run. A run with sales cannot be deleted until its receipts are voided.
- **Reports without double counting:** run sales are counted once, on receipts, with their cost of goods; run costs (including selling costs) are production costs; the value carried into stock is taken back out. Sales recorded inside 1.0.4 runs are moved to `run_sales` and still count.

### What changed in 1.0.4
- **Processing is now one simple form** (Processing page, `palm_oil.rs`, migration 009) for palm oil, garri, fufu and cassava flour. Plain questions in order: how you got the fruit or cassava (bought, from your farm, both), the measure (basin, bag, bunch, heap…), how many and the price of one, transport to processing, whose mill and the fee, workers and pay each, other costs, how much you got (keg, litre, cup, custard bucket, paint bucket, bag, wrap…), the price now, how much sold and at what price, transport to market. The result (spent, received, profit or loss, unsold value, cost per unit) updates as you type. Saving checks plain mistakes ("you sold more palm kernel than you got").
- **Stock balances:** unsold products go into Farm stock at the run's cost per unit; raw material from your own stored harvest is taken out; editing moves only the difference; deleting a run takes it all back. Run costs and direct sales appear in Farm reports (profit and loss, cash flow, profit by activity).
- The line-by-line tiles and the run planner from 1.0.3 are removed; the older "processing batches" page is no longer in the menu (its records stay in the database and reports).

### What changed in 1.0.3
- **Palm oil business** (new page, `palm_oil.rs`, migration 008): runs for people who buy palm fruit (basin, bag, bunch, head pan, kg, tonne, truck), harvest their own palms, or both. Each run records fruit, transport of fruit, loading, processing (mill) fee, workers (number × days × pay), fuel, water, kegs, transport to market, levies; oil, kernel and by-products out; and every sale. It works out total cost, total sales, profit or loss, unsold stock at your expected price, cost per keg, break-even oil price and oil per basin. **Plan a run** does the same sums before buying. Planting stays in Crop farming. Setup has a new choice, "Palm oil: I buy fruit and process it".
- **Three new suggestion rules** for palm oil runs (heading for a loss, oil unsold, no outputs recorded): 20 rules in all.
- **More cards and tiles:** pill-tile tabs, action tiles, run cards, suggestion cards in a grid, KPI tiles, module switches as tiles, and a "how advice works" strip on Suggestions.
- **Own licence keys:** `~/.farmbuddy-keys/`, keys start with `FMBD-`; the content signing key moves there too (see Keys and releases).

### What changed in 1.0.2
- **First-run setup is the farm setup from the specification (section 5):** language, farm profile (state, LGA, community, phone, address, business type), what you manage, size, how you want to work, then the owner account and account recovery. One call (`setup_farm`) creates the owner, the farm, the first batches and plots, and the matching farm template's starter items (names only, no prices or stock).
- **Shop leftovers removed:** the Supermarket, Eatery, Dry cleaning and "Any other business" types, their ~330 sample items with prices, the Settings → Business type page, and the shop pages of the user guide. Existing databases are moved to `business_type = farm` (migration 007); no records are deleted.
- **Farm wording:** Today, Sell produce, Buyers, Farm stock, Purchases, Sales overview; receipts titled "Farm sales receipt".
- **New design:** deep-forest sidebar with every page grouped (Farm, Advice, Sell, Stock, Money, Business, Help), farm switcher at the top, Today page with greeting and quick actions, KPI cards, Fraunces + Plus Jakarta Sans type, emerald and harvest-gold colours, new logo and app icons, new sign-in and setup screens. Phones keep a bottom bar.
- **Ibibio:** your approved words (58 labels, 112 screen words); "Today" uses *Mfịn*.

Every feature below has a working screen, a Rust command with permission checks, database logic and automated tests (86 Rust tests, 18 UI tests, and a UI walk-through of 52 screens as owner, farm worker, in Ibibio, first-run setup and phone size).

| Area | What works |
|---|---|
| **Guided setup and templates** | Conditional question engine (`onboarding.rs`), all 36 states + FCT and **all 774 LGAs**. **14 farm templates** (broiler, layer, cockerel, catfish, cassava, yam, maize, oil palm, mixed crop, palm oil processing, cassava processing, produce store, input shop, mixed business): categories, units, starter item names **without prices or stock**, modules, home cards, check-ups and reports. Templates only add. |
| **Farms and access** | Several farms; archive instead of delete. **Farm switcher** in the top bar. **Per-farm access**: workers, advisers and cashiers can be limited to chosen farms (enforced in Rust). |
| **Livestock** | Batches for poultry, goats, sheep, pigs, cattle, rabbits, snails, catfish, tilapia. Daily deaths, culls, feed (and **feed wasted**), water, eggs, weights; sales; costs. **Individual animals** (tag, sex, date of birth, dam/sire, weight history, sold/died). **Breeding, pregnancy and births** (offspring added to the batch and optionally tagged). **Animals received** into a batch and **transfers between batches**. **Egg stock**: collected, damaged, sold, used, lost, remaining, laying %. Mortality ÷ all animals that came in. |
| **Fish** | **Pond register**, pond chosen on the batch, **water-quality readings** (empty = "not measured", never "safe"), sample weighing for FCR. |
| **Feed** | **Feed guides** you enter with their source and review status; **expected vs actual** feed per day; **wastage**; **comparison across batches** (feed per animal, FCR, cost per animal); **feed mix calculator** (cost per kg, weighted protein and energy only when every value is entered). |
| **Health and vaccination** | Health records, vaccination statuses including *Needs vet review*. **Reusable schedule templates** with review (owner or adviser approves); applying one plans each vaccination on placement date + day. |
| **Crops** | Plots and crop cycles, field work, harvests, yield and loss. **Calendar** (planting, harvest windows, field work, harvests, crop problems, reminders, rain onset/end from outlooks). **Crop health** records (product use requires an advice source; confirmation by an adviser). **Variety catalogue**. **Fields above plots** and **plot history**. |
| **Processing** | Palm oil, garri, flour, milling, feed; costs, stages, products/by-products/waste, cost split, closing into stock. |
| **Storage** | **Stores** (store, warehouse, cold room, crib, shop…), **put away, issue, transfer, spoilage, damage, loss, count**, **quality and moisture checks**, **balance by location** with "not in any store". Loss types reduce stock and need a manager. |
| **Weather and markets** | **Weather and rainfall records** with monthly totals; **NiMet seasonal outlooks** you enter or import (never invented); **market prices with history** by product and market (demo prices are labelled illustrative). |
| **Finance and reports** | **Farm profit and loss**, **cash flow**, **profit by activity** (each batch, crop, processing run), **production records** (livestock, feed, weights, vaccinations, health, crops, processing). **Each naira counted once**: purchases → stock → cost when sold or used; processing cost carried into stock counted on sale; credit sales are revenue but not cash; demo excluded by default. **Save PDF, CSV, Print**. **Finance summary** for loans and grants with assets and liabilities. **Home money cards** for managers: estimated profit, expenses this month, inventory value, owed to suppliers, owed to you. |
| **Advisory** | Suggestions from your records, now driven by **rules stored in the database** (owner switches rules on/off, sets priority and day thresholds). **Ask FarmBuddy** guided check-ups (poultry, crop, fish, processing) with follow-up questions, explained results, source and safety level, saved history. No diagnosis, medicine names, doses or chemical rates. |
| **Knowledge updates** | **Content packages** (articles, sources, schedules, feed guides, outlooks) imported from a file, **Ed25519-signed**, versioned (older or same version refused), never overwriting farm-written articles; unsigned content arrives *not yet reviewed*. Update history. |
| **Users and security** | Roles: **Farm worker, Adviser**, Cashier, Manager, Owner. Workers and advisers never see sales, prices, costs or profit. **Activity log** (staff, settings, resets, voids, stock adjustments, restores, deletions, content imports, template use). |
| **Operations** | **Restore workflow**: file check (SQLite header, integrity, owner exists, version), upgrade of older backups, automatic **safety backup** first, password + typed confirmation, everyone signed out. **Online/offline indicator**. |
| **Processing runs** | Palm oil, garri, fufu and cassava flour, from buying or harvesting to selling, in one simple form; full profit and loss; stock kept in balance. |
| **Language** | `t()` keys (354 English) plus a screen translator for whole interface words. Ibibio: the owner's approved terms (58 keys, 112 screen words, `src/lib/i18n/ib.ts` and `ib-text.ts`); everything else falls back to English (`npm run translation:check`). |
| **Demo data** | Broilers, layers with egg sales, goats with tags, mating and twins, catfish pond with water readings, cassava and maize with a harvest in store, palm oil batch, a purchase with ₦30,000 owed, a part-credit sale, two stores with transfer, spoilage and a moisture check, weather, illustrative market prices. All marked Demo and removed in one step. |
| **Selling and stock (spec sections 11–12)** | Farm stock, purchases and suppliers, Sell produce with split/credit payments, receipts, returns, shifts, expenses, sales reports, backups, licensing, auto-update, offline account recovery. |

### Honest limits

- The Windows and macOS installers were not built or run on those systems during development; build them with the commands below (or the release workflow) and test on each before rollout.
- Ibibio wording needs fluent reviewers; English shows until each Ibibio term is approved.
- Built-in guides and the example content package are drafts marked *pending expert review*. Feed guides, vaccination schedules, outlooks and prices are entered or imported by you with their source; FarmBuddy ships none as fact.

## Requirements

- **Rust** stable: install from https://rustup.rs
- **Node.js** 20 LTS or newer, with npm
- **Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and the WebView2 runtime (already on Windows 10/11)
- **macOS:** Xcode Command Line Tools (`xcode-select --install`)

## Run commands

From the project folder (`farmbuddy/`), in Terminal on Mac or Git Bash on Windows:

```bash
# 1. Install frontend dependencies (first time, and after updates)
npm install

# 2. Run the desktop app in development mode (hot reload)
npm run tauri dev

# 3. Build the installer for this computer's OS
npm run tauri build
```

Checks:

```bash
npm test                       # UI tests (vitest) + all Rust backend tests
npm run test:ui                # UI tests only
npm run test:rust              # Rust tests only (cargo test --lib)
npm run typecheck              # TypeScript type check
npm run content:check -- content/example-package.json   # validate a content package
npm run content:sign -- content/example-package.json    # sign it (uses secrets/content_signing_key.txt)
npm run seed:demo              # load the demo farm into this computer's FarmBuddy database (close the app first)
npm run seed:demo -- --db ./demo.sqlite3                # …or into any file (FB_OWNER_PASSWORD=… creates the owner)
npm run build                  # type-check and build the frontend only
npm run translation:check      # English vs Ibibio keys (add -- --strict before a release)
cargo check --manifest-path src-tauri/Cargo.toml
npm run icons                  # regenerate every icon from src-tauri/icons/app-icon.png
```

Installers are written to `src-tauri/target/release/bundle/`:

- Windows: `msi/FarmBuddy_1.0.0_x64_en-US.msi` and `nsis/FarmBuddy_1.0.0_x64-setup.exe`
- macOS: `dmg/FarmBuddy_1.0.0_aarch64.dmg` (or `_x64.dmg` on Intel Macs) and `macos/FarmBuddy.app`

A local build without the licence public key cannot be activated (it shows a 14-day trial, then locks). Official installers come from GitHub Actions, see **Keys and releases** below.

## First run

1. Open the app and create the **owner account** (the setup screen is the same as eStock's: business name, owner, password, security questions, recovery key).
2. FarmBuddy opens **Set up your farm**. Answer the questions; only those that apply to your answers appear.
3. Look at **Home → Your farm today**. To explore first, open **Farm → My Farm → Load demo data** (and remove it later from the same place).
4. Add staff in **Business → Staff and users**. Managers run the farm records and stock; cashiers (farm workers and sales staff) enter daily records, field work, harvests and sales.

### Role permissions

| | Worker | Adviser | Cashier | Manager | Owner |
|---|:-:|:-:|:-:|:-:|:-:|
| Daily livestock records, eggs, field work, harvests, weather, water readings, storage put-away/transfer | ✓ | ✓ | ✓ | ✓ | ✓ |
| Plan vaccinations, approve schedules / feed guides / articles, confirm crop problems | | ✓ | | (plan) | ✓ |
| Make sales, receipts, customers, shifts | | | ✓ | ✓ | ✓ |
| Costs, prices, profit, farm reports with money, storage losses | | | | ✓ | ✓ |
| Inventory, purchases, sales reports, exports | | | | ✓ | ✓ |
| Staff, farm access, settings, backups and restore, templates, content import, suggestion rules, finance summary, activity log | | | | | ✓ |

Workers, advisers and cashiers can be limited to chosen farms (Staff → Farms). Every rule is enforced in the Rust backend, not just hidden in the UI.

## Keys and releases (admin only)

**Step-by-step release, licence keys and signing for Windows, Mac and Android: see [`RELEASING.md`](RELEASING.md).**

FarmBuddy has **its own key folder**, `~/.farmbuddy-keys/` (Windows: `C:\Users\<you>\.farmbuddy-keys\`), separate from eStock Manager. A FarmBuddy licence key starts with `FMBD-` and is signed only by FarmBuddy's private key, so eStock keys never unlock FarmBuddy and FarmBuddy keys never unlock eStock. Set `FARMBUDDY_KEY_DIR` to keep the folder somewhere else (a flash drive, say).

Before the first release, create the GitHub repository `treasure155/farmbuddy` (the updater, Android update check and workflows point there).

The **private halves live only on the admin's computer**. Never put that folder in the project or on GitHub, and back it up: if it is lost you can no longer issue licences or ship updates to existing installs.

| Key | Private file (admin computer) | Public half (in the project) | Used for |
|---|---|---|---|
| Licence signing (Ed25519) | `~/.farmbuddy-keys/license-private.pem` | `src-tauri/license_public_key.txt` | Making licence keys |
| Update signing (minisign) | `~/.farmbuddy-keys/updater.key` + `updater-password.txt` | `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` | Signing installers so installed apps accept updates |
| Android app signing (PKCS12) | `~/.farmbuddy-keys/android-release.p12` + `android-keystore-password.txt` | none (GitHub secrets) | Signing the APK; phones only accept updates signed with the same key |
| Knowledge packages (Ed25519) | `~/.farmbuddy-keys/content-signing-key.txt` | `src-tauri/content_public_key.txt` | Signing guide and schedule updates |

### One-time setup (admin computer)

```bash
npm install
npm run setup-keys          # creates the keys, writes the public halves into the project
npm run tauri build         # rebuild: the app must contain YOUR public key
```

`setup-keys` never overwrites a key that already exists. It also copies `secrets/content_signing_key.txt` into the key folder (then delete the project's `secrets/` folder), and stores the update and Android secrets on GitHub if the GitHub CLI (`gh`) is signed in; otherwise it prints them to paste under GitHub → repo → Settings → Secrets and variables → Actions.

**Rebuild after `setup-keys`.** An app built before it checks keys against the old public key, so keys you issue will be refused by that old build.

### Issue a licence (one per computer)

1. The customer opens **Settings → Licence** (or the activation screen) and sends you the Install ID, for example `P0NP-K8GF-6R3E-QB45`.
2. On the admin computer:

```bash
npm run license -- issue P0NP-K8GF-6R3E-QB45                                  # never expires
npm run license -- issue P0NP-K8GF-6R3E-QB45 --days 365 --note "Customer, Etinan"
npm run license -- list                                                       # everything issued
```

3. Send back the `FMBD-…` key. The customer pastes it into Settings → Licence. It only works on the computer with that Install ID. Every key issued is logged in `~/.farmbuddy-keys/issued-licences.csv`.

### Release a version (one push)

1. Raise the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` (all three must match).
2. Run:

```bash
npm run release
```

It checks the versions and keys, commits, tags `vX.Y.Z` and pushes the branch and tag together. GitHub Actions builds the Windows (`.exe`, `.msi`) and macOS (universal `.dmg`) installers, signs the update files and **publishes the release** (not a draft) with `latest.json`. Installed copies offer the update automatically. The workflow stops with a clear error if a key or secret is missing.

Without Apple Developer ID secrets the macOS app is not notarised, so first-time Mac users right-click the app → Open (or System Settings → Privacy & Security → Open Anyway). Windows and Mac code signing are optional and switch on when their secrets are added (see `RELEASING.md`, section 5).

## Email setup

Open **Settings → Email**, pick the provider and click **Send test** after saving.

| Provider | Server | Port | Security | Password to use |
|---|---|---|---|---|
| **Gmail / Google Workspace** | smtp.gmail.com | 587 | STARTTLS | A 16-letter **App Password**: turn on 2-Step Verification, then create one at myaccount.google.com/apppasswords. Your normal Gmail password will be rejected. |
| Outlook / Microsoft 365 | smtp.office365.com | 587 | STARTTLS | Mailbox password (the admin must allow Authenticated SMTP) |
| Outlook.com (personal) | smtp-mail.outlook.com | 587 | STARTTLS | Account password or app password |
| Yahoo Mail | smtp.mail.yahoo.com | 465 | SSL/TLS | Yahoo app password |
| Zoho Mail | smtp.zoho.com (.eu / .in) | 465 | SSL/TLS | Password or application-specific password |
| Domain email (cPanel etc.) | mail.yourdomain.com | 465 or 587 | SSL/TLS or STARTTLS | Mailbox password |

Notes:
- Gmail app passwords can be pasted with or without spaces.
- Gmail sends from the signed-in address or an alias added under Gmail → Settings → Accounts → "Send mail as". The limit is about 500 emails a day.
- Passwords are never sent over an unencrypted connection (security "None" is allowed only for a mail server on `localhost`).
- The saved SMTP password is never sent back to the screen. Leave the field blank to keep it.

## Where files go

This is a desktop app, so nothing is uploaded to a server. Place the project files like this:

| File or folder | Destination |
|---|---|
| Whole project | Any folder on the build computer, e.g. `C:\Projects\farmbuddy` or `~/Projects/farmbuddy` |
| `src-tauri/icons/*` | Stay where they are. Tauri reads them during `npm run tauri build`. |
| Built installer (`.msi` / `-setup.exe` / `.dmg`) | Copy to each shop computer and run it |
| Database (created automatically) | Windows: `%APPDATA%\online.techalphahub.farmbuddy\farmbuddy.sqlite3`. macOS: `~/Library/Application Support/online.techalphahub.farmbuddy/farmbuddy.sqlite3` |
| Automatic backups | `Documents/FarmBuddy Backups/farmbuddy-backup-YYYYMMDD-HHMMSS.sqlite3` (or the folder chosen in Settings → Backup) |
| Trial marker | `.farmbuddy-install` in the app config folder (same folder as the database) |
| Admin signing keys | `~/.farmbuddy-keys/` on the admin's computer only |
| CSV exports | Saved to **Downloads** |

**Separate from eStock Manager:** FarmBuddy has its own app identifier and database file, so it installs side by side with eStock Manager and never touches its data.

**Restoring a backup:** close the app, rename the backup file to `farmbuddy.sqlite3` and replace the file in the database folder above.

## Environment variables

None are required to run the app. All configuration (business details, SMTP, theme) lives in Settings and the local database. Optional, for tools only:

| Variable | Used by | Purpose |
|---|---|---|
| `FB_CONTENT_KEY` | `npm run content:sign` | Content signing key (base64). Otherwise read from `~/.farmbuddy-keys/content-signing-key.txt`, then `secrets/content_signing_key.txt` |
| `FARMBUDDY_KEY_DIR` | all admin tools | Where the private keys live (default `~/.farmbuddy-keys`) |
| `FB_OWNER_PASSWORD` | `npm run seed:demo` | Creates the owner account in a new demo database file |

**Keep the content signing key private.** After `npm run setup-keys` it lives in `~/.farmbuddy-keys/`; delete the project's `secrets/` folder. Its public half is `src-tauri/content_public_key.txt`, built into the app.

## Project structure

```
farmbuddy/
├── index.html, package.json, vite.config.ts, tsconfig.json
├── public/logo-mark.svg              FarmBuddy logo (also the source of the app icons)
├── src/
│   ├── main.ts                       Boot; a new owner is sent to the guided setup
│   ├── api.ts, store.ts, types.ts    Command wrapper, cached settings, shared types
│   ├── lib/i18n/                     en.ts (reference), ib.ts (Ibibio, reviewer-approved only), index.ts (t())
│   ├── lib/content/                  nigeria.ts (states, Akwa Ibom LGAs), farm.ts (species, crops, activities, processing templates)
│   ├── views/onboarding.ts           Guided setup wizard
│   ├── views/farm.ts                 My Farm: farms, modules, language, demo data
│   ├── views/livestock.ts            Livestock batches (replaces eStock's flocks page)
│   ├── views/health.ts               Vaccinations and health records
│   ├── views/crops.ts                Plots, crops, field work, harvests
│   ├── views/processing.ts           Processing batches
│   ├── views/advisory.ts             Suggestions, reminders, knowledge library
│   ├── views/home.ts                 Home with "Your farm today" and money cards
│   ├── views/livestock-extra.ts      Animals, breeding, births, transfers, eggs, ponds and water
│   ├── views/feed.ts                 Feed vs guide, comparison, guides, feed mix calculator
│   ├── views/storage.ts              Stores, transfers, spoilage, quality checks
│   ├── views/weather.ts              Weather, NiMet outlooks, market prices
│   ├── views/farm-reports.ts         P&L, cash flow, by activity, production, finance summary
│   ├── views/crops-extra.ts          Crop calendar, crop health, varieties, fields, plot history
│   ├── views/advisory-flows.ts       Ask FarmBuddy check-ups, suggestion rules editor
│   ├── views/audit.ts                Activity log
│   ├── lib/ui.ts, lib/report-pdf.ts  Form/table helpers, PDF report layout
│   ├── views/…                       POS, receipts, inventory, purchases, expenses, reports, staff, settings (from eStock)
│   └── styles/farm.css               Styles for the farm screens
├── src-tauri/
│   ├── migrations/005_farm.sql       Farm schema (farms, livestock fields, health, vaccinations, crops, processing, library, reminders, demo flags)
│   ├── migrations/006_complete.sql   1.0 schema: animals, breeding, transfers, eggs, ponds, water, feed, schedules, fields, varieties, crop health, storage, weather, outlooks, prices, rules, content versions, farm access, audit, finance items
│   ├── content_public_key.txt        Public key that verifies content packages
│   └── src/
│       ├── farms.rs                  Farms, shared validation, farm overview
│       ├── onboarding.rs             Conditional question engine
│       ├── flocks.rs                 Livestock batches (table name kept so eStock data upgrades)
│       ├── health.rs                 Health records and vaccination statuses
│       ├── crops.rs                  Plots, crop cycles, activities, harvests
│       ├── processing.rs             Processing batches and cost allocation
│       ├── advisory.rs               Suggestions and reminders
│       ├── knowledge.rs              Source register and guides
│       ├── demo.rs                   Load / clear demo data
│       ├── livestock_ext.rs          Animals, breeding, births, transfers, eggs, ponds, water
│       ├── feed.rs                   Feed guides, expected vs actual, formulation calculator
│       ├── crops_ext.rs              Fields, varieties, crop health, calendar
│       ├── storage.rs                Stores and storage movements
│       ├── weather_market.rs         Weather, outlooks, market prices
│       ├── finance.rs                P&L, cash flow, by activity, production report, finance summary
│       ├── advisory_flows.rs         Ask FarmBuddy question flows
│       ├── content.rs                Signed content package import
│       ├── farm_templates.rs         The 14 farm templates
│       ├── farm_access.rs            Per-farm access
│       ├── audit.rs                  Activity log
│       ├── ui_fixtures.rs            Test-only JSON fixtures; seed_demo_db for npm run seed:demo
│       └── …                         auth, inventory, sales, purchases, reports, backup, license (from eStock)
├── tests/                            Vitest UI tests
├── content/example-package.json      Example content package (draft, pending review)
├── secrets/content_signing_key.txt   PRIVATE content signing key (not in git)
├── tools/                            translation-check, content-check, sign-content, seed-demo-data, release tools
└── .github/workflows/                test.yml (tests on every push), release (Windows, macOS), Android
```

## Security notes

- Money is stored as integer kobo, so there are no rounding errors.
- Every sale, stock change, void and payment runs in a single SQLite transaction.
- All SQL uses bound parameters. All user text is HTML-escaped before display, and CSV exports neutralise spreadsheet formulas.
- The window runs under a strict Content Security Policy. Fonts are bundled, so nothing loads from the internet.
- Licence keys are verified offline with an embedded Ed25519 public key; the private key never ships. The trial start is stored in two places and a clock-rollback guard stops trial extension by changing the date.
- Updates are accepted only when signed with the release key (minisign), and are downloaded over HTTPS from GitHub Releases.
- WhatsApp links are the only URLs the app may open (`https://wa.me/*`), plus the project's GitHub page.
- The only network request the app makes on its own is the usage notice to `https://api.techalphahub.online/api/products/telemetry` (Rust side, 20-second timeout, retried every 15 minutes while offline).
- SMTP credentials are stored in the local database, readable only through the Owner role in the app. Protect the Windows/macOS user account with a password, use a dedicated sending mailbox, and keep backups somewhere safe, since they contain your data.

## Account recovery (offline)

- At setup the owner picks three security questions and gets a **recovery key** like `K7QM-4XPA-9TRB-2HWN`. Only scrambled (Argon2) copies of the key and answers are stored.
- **Forgot password?** on the sign-in screen resets the password of any account, after the recovery key or all three answers are given. A deactivated account is switched back on.
- **Create a new owner account** on the sign-in screen adds an Owner after the same check, for when no owner can sign in.
- Using the key replaces it with a new one, which is shown once. Answers are not case-sensitive.
- After 5 wrong recovery attempts, recovery is locked for 10 minutes. Every attempt is listed under **Settings → Security & recovery**, where the owner can also create a new key or change the questions (both need the current password).
- For databases upgraded from an earlier version, the owner sees a "Set up account recovery" notice on the Overview page until both are set.

## Android app

The same code builds an Android app (APK) in the release workflow. Differences on Android:

- **Layout:** home screen with tiles (Sell, Stock, Money, Business, Help, and Farm when enabled). Tapping a tile opens its pages as a bottom sheet. A tab bar at the bottom (Home, Sell, Receipts, Customers, Menu) and the phone's back button move between pages.
- **Android versions:** Android 7.0 (API 24) and newer, on phones and tablets (ARM 64/32-bit and x86). The Android build ships an extra compatibility bundle so it runs on old system WebViews too.
- **Barcodes:** a **Scan** button on New sale uses the phone camera (USB/Bluetooth scanners also work).
- **Receipt printing:** through the free **RawBT Print Service** app (Play Store), which drives 58/80 mm Bluetooth and USB thermal printers. Other pages (labels, statements, Z reports) print from the desktop app; statements can be sent on WhatsApp.
- **Updates:** from 3.4.0 the app finds new versions itself (`check_apk_update` reads the latest release), downloads the APK with Android's DownloadManager and opens the package installer (`REQUEST_INSTALL_PACKAGES`; bridge in `tools/android-native.mjs`). Data is kept because every APK is signed with the same key. Phones on 3.3.0 or older install 3.4.0 once by hand.
- **Licence:** the Install ID is created on first run and stays the same until the app is uninstalled.
- **Backups:** saved to the app's Documents folder on the phone.

About & contact (Menu → Help → About & contact, or tap your initials) shows the version, licence state and TechAlpha Hub contact details.

Install on a phone: download `FarmBuddy_X.Y.Z_android.apk` from Releases on the phone, open it and allow installing from the browser when Android asks.

`tools/check-update.mjs` (`node tools/check-update.mjs`) checks that the published desktop update matches your signing key.
