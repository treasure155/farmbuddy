#!/usr/bin/env node
// Used by the release workflow after `tauri android init`: writes the Android
// keystore from GitHub secrets and makes the release APK signed with it.
// The keystore never changes, so phones accept every new APK as an update.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { ROOT } from './common.mjs';
import './android-native.mjs';

const b64 = process.env.ANDROID_KEYSTORE_BASE64, pw = process.env.ANDROID_KEYSTORE_PASSWORD;
if (!b64 || !pw) { console.error('✘ GitHub secrets ANDROID_KEYSTORE_BASE64 / ANDROID_KEYSTORE_PASSWORD are missing. Run npm run setup-keys and add them.'); process.exit(1); }
const androidDir = join(ROOT, 'src-tauri', 'gen', 'android');
const gradle = join(androidDir, 'app', 'build.gradle.kts');
if (!existsSync(gradle)) { console.error(`✘ ${gradle} not found. Run "npx tauri android init" first.`); process.exit(1); }

writeFileSync(join(androidDir, 'farmbuddy-release.p12'), Buffer.from(b64.trim(), 'base64'));
writeFileSync(join(androidDir, 'keystore.properties'), `password=${pw.trim()}\nkeyAlias=farmbuddy\nstoreFile=${join(androidDir, 'farmbuddy-release.p12').replace(/\\/g, '/')}\n`);

let g = readFileSync(gradle, 'utf8');
if (!g.includes('signingConfigs')) {
  const block = `    signingConfigs {
        create("release") {
            val ksFile = rootProject.file("keystore.properties")
            val ks = Properties()
            if (ksFile.exists()) ksFile.inputStream().use { ks.load(it) }
            keyAlias = ks.getProperty("keyAlias")
            keyPassword = ks.getProperty("password")
            storePassword = ks.getProperty("password")
            storeFile = file(ks.getProperty("storeFile"))
            storeType = "pkcs12"
        }
    }
    buildTypes {`;
  if (!/\n\s*buildTypes \{/.test(g)) { console.error('✘ Unexpected build.gradle.kts layout (no buildTypes).'); process.exit(1); }
  g = g.replace(/\n\s*buildTypes \{/, `\n${block}`);
  if (!/getByName\("release"\) \{/.test(g)) { console.error('✘ Unexpected build.gradle.kts layout (no release build type).'); process.exit(1); }
  g = g.replace(/getByName\("release"\) \{/, 'getByName("release") {\n            signingConfig = signingConfigs.getByName("release")');
}
// The block above uses Properties(); inside android { } a bare `java.util...` is
// resolved against the Gradle `java` extension and fails to compile, so import it.
if (!/^import java\.util\.Properties\s*$/m.test(g)) g = 'import java.util.Properties\n' + g;
writeFileSync(gradle, g);
console.log('✔ Android release signing configured');
