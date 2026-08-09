# Verb V0 Device Validation Results

Date: 2026-08-09

## Device and build

| Field | Value |
| --- | --- |
| Device | Vivo I2202 |
| Android version | 14 |
| Package | `com.aistudio.verb.app` |
| Verified source commit | `48616e6` |
| Installation | Fresh debug installation through ADB, followed by an in-place update |

## Verified on the physical device

| Area | Result |
| --- | --- |
| Launch | App launched cleanly to Ask; no fatal exception observed. |
| Storage summary | Real device storage shown as 104.2 GB total, 37.2 GB used, 67.0 GB available. |
| Memory summary | Real Android `ActivityManager` values shown as 7.42 GB total, 4.51 GB used, 2.91 GB available. |
| Result provenance | Derived, Observed, and Explanation fields each rendered separately. |
| Port inspection | `port 3000` reported only local bind availability; no fabricated process/PID owner. |
| Explanation precedence | `what does ps do?` rendered Command Explanation, not Running Processes. |
| Process-stop safety | `stop process 1234` displayed explicit confirmation; the request was cancelled without execution. |
| System screen | Live storage formatting and `/system/bin/sh` runtime identity rendered correctly. |
| Terminal foundation | Real shell prompt, Clear control, and ESC/CTRL/SHIFT/TAB/PASTE controls rendered. |
| Firebase residue | The post-`48616e6` build launched without Firebase/Google Services initialization warnings. |

## Intentionally not exercised

- No process-stop confirmation was approved against a real device process.
- No terminal command was injected by automated validation.
- Semantic Lens selection/Inspect needs an explicit user-driven terminal text selection pass.

Those cases remain governed by the safety contract and should be tested only with deliberate,
non-destructive inputs.
