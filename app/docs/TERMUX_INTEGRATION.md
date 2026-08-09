# Termux Terminal Engine Integration & Provenance Report

## 1. Provenance Metadata (Real Verified Upstream Source)
- **Upstream Repository**: [ShaileshRawat1403/termux-app](https://github.com/ShaileshRawat1403/termux-app)
- **Upstream Branch**: `master`
- **Verified Commit SHA**: `3df69d1da197dd9bd71a3bafd902dffd720576b4`
- **Verification Method**: Verified against upstream Java sources in the specified commit.
- **Licensing**:
  - Upstream Termux core (`terminal-emulator` / `terminal-view`): **GPLv3-only with exceptions**. As stated in the upstream LICENSE.md, it includes Android Terminal Emulator-derived code used in terminal-view and terminal-emulator which is licensed under the **Apache License 2.0**.
  - Verb runtime adapter layer (`com.example.verb.terminal.*`): **Apache License 2.0**

---

## 2. File Audit & Classification
Every file under `app/src/main/java/com/termux/terminal/` and `app/src/main/java/com/termux/view/` has been audited against upstream Java sources. The Kotlin reimplementations have been removed and replaced with the actual upstream Java implementations.

### EXACT_UPSTREAM (20 files)
These files are byte-identical or functionally identical to the upstream pinned commit:
- `com/termux/terminal/ByteQueue.java`
- `com/termux/terminal/KeyHandler.java`
- `com/termux/terminal/Logger.java`
- `com/termux/terminal/TerminalBuffer.java`
- `com/termux/terminal/TerminalColorScheme.java`
- `com/termux/terminal/TerminalColors.java`
- `com/termux/terminal/TerminalEmulator.java`
- `com/termux/terminal/TerminalOutput.java`
- `com/termux/terminal/TerminalRow.java`
- `com/termux/terminal/TerminalSession.java`
- `com/termux/terminal/TerminalSessionClient.java`
- `com/termux/terminal/TextStyle.java`
- `com/termux/terminal/WcWidth.java`
- `com/termux/terminal/JNI.java`
- `com/termux/view/GestureAndScaleRecognizer.java`
- `com/termux/view/TerminalRenderer.java`
- `com/termux/view/TerminalView.java`
- `com/termux/view/textselection/CursorController.java`
- `com/termux/view/textselection/TextSelectionHandleView.java`
- `com/termux/view/support/PopupWindowCompatGingerbread.java`

### MODIFIED_UPSTREAM (2 files)
These files were imported from upstream but required minor modifications for Verb integration:
- `com/termux/view/TerminalViewClient.java` (Added `onInspectText(String)` for Semantic Lens integration)
- `com/termux/view/textselection/TextSelectionCursorController.java` (Added `ACTION_INSPECT` menu item for Semantic Lens)

---

## 3. Native Version Provenance
- **NATIVE_VERSION_STATUS**: `EMULATOR_VERIFIED` = **VERIFIED** / `PHYSICAL_DEVICE_VERIFIED` = **NOT_RUN**
- **Details**: The previous prebuilt binary approach (`v0.118.3` extraction) was proven defective on physical devices (`unexpected e_version`). The project now uses the exact upstream native source from the pinned Termux commit:
  - `terminal-emulator/src/main/jni/termux.c`
  - `terminal-emulator/src/main/jni/Android.mk`
- **Compatibility Status**: `EXACT_MATCH`. The Java JNI contract (`com.termux.terminal.JNI.java`) and the native source are built from the exact same pinned repository commit.
- **Resolution**: Verb's `build.gradle.kts` incorporates the upstream `Android.mk` via `externalNativeBuild { ndkBuild { ... } }`, producing an authoritative `libtermux.so` for `arm64-v8a`.

---

## 4. Runtime Environment Definition
For Verb P0.2, the runtime is explicitly defined as:
- **Termux terminal engine** (upstream Java + `libtermux.so` native PTY binary)
- **Android system shell** (`/system/bin/sh`)

**Note on Termux Userland**: Verb is not the `com.termux` package. It does not automatically append `/data/data/com.termux/files/usr/bin` to `PATH`. Full Termux bootstrap/package userland integration is outside the scope of this sprint.

---

## 5. Automated Build & Test Evidence
| Command | Environment | Status |
| :--- | :--- | :---: |
| `gradle :app:assembleDebug` | Gradle CLI | **BUILD_VERIFIED** |
| `gradle :app:testDebugUnitTest` | JVM / Robolectric | **BUILD_VERIFIED** |

---

## 6. Physical Device Test Checklist (Truthful Status)
Physical Android tests remain **NOT_RUN** unless the user performs them on a physical device. We do not fabricate physical-device results.

| Test ID | Test Case Scenario | Procedure | Physical Status |
| :---: | :--- | :--- | :---: |
| **1** | ANSI Colour | Execute `printf '\e[31mRed\e[32mGreen\e[0m\n'` | **NOT RUN** |
| **2** | Arrow History | Press `UP` and `DOWN` power strip keys | **NOT RUN** |
| **3** | Ctrl-C Signal | Execute `sleep 30`, tap `CTRL_C` key | **NOT RUN** |
| **4** | Interactive Cat Input | Run `cat`, type input, terminate with `CTRL_C` | **NOT RUN** |
| **5** | UTF-8 Rendering | Execute `printf 'नमस्ते 世界\n'` | **NOT RUN** |
| **6** | Orientation Resize | Rotate device between portrait and landscape | **NOT RUN** |
| **7** | Large Scrollback | Output 10,000 lines of text in session | **NOT RUN** |
| **8** | Exact Selection | Long press line and forward selection to Semantic Lens | **NOT RUN** |
| **9** | Alternate Screen | Run full-screen interactive utility (`top` or `vim`) | **NOT RUN** |
| **10** | Lifecycle Resume | Move app to background and resume foreground state | **NOT RUN** |

---

## 7. Source-Reference Report
At completion state:
- **Termux repo**: `ShaileshRawat1403/termux-app`
- **Termux commit**: `3df69d1da197dd9bd71a3bafd902dffd720576b4`
- **Termux files inspected**: All `com.termux.terminal.*` and `com.termux.view.*` Java files.
- **Exact upstream files reused**: 20 files.
- **Modified upstream files**: 2 files.
- **Verb adapter files modified**: `TermuxTerminalRuntimeAdapter.kt` (fixed selection ownership, clipboard path, and `PATH` assumption).
- **Termix consulted**: NOT REQUIRED FOR THIS INFRASTRUCTURE PATCH
