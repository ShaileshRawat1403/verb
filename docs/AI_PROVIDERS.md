# Verb AI providers

## Scope

Verb's **Assistant** is a provider-backed, user-initiated chat surface. It is separate from
Verb's structured system actions and separate from the interactive terminal.

- Assistant requests are sent only after the user saves a provider configuration and presses
  **Ask provider**.
- Assistant text may explain or propose commands, but it has no API that writes to a terminal PTY
  or executes a device action.
- The Terminal remains an explicit, user-controlled interactive session. Credentials configured
  for a terminal CLI are not read by Assistant, and Assistant credentials are never exported to
  Terminal.

## Supported protocols

| Provider | Protocol |
| --- | --- |
| OpenAI | Responses API |
| Anthropic | Messages API |
| Gemini | GenerateContent API |
| OpenAI-compatible | Chat Completions API |

The user chooses a model available to their own account. Verb does not hard-code a model name or
claim that a model is available.

## Credential boundary

Verb is a bring-your-own-key app. Provider API keys are encrypted with an Android Keystore AES-GCM
key before being written to app-private preferences. The UI exposes only whether a key exists; it
never displays, logs, or includes it in an `ActionResult`, history card, semantic analysis, or
terminal payload.

For a distributed, multi-user product, move provider calls behind a user-owned backend or broker
that can enforce authentication, quotas, and abuse controls. Do not ship a shared provider key in
the APK, source tree, or `.env.example`.

## Explicit exclusions

- No model-generated terminal command execution.
- No automatic forwarding of terminal output, selections, files, or device observations to a
  provider.
- No API keys in Git, build output, logs, screenshots, or terminal output.
- No reliance on a ChatGPT, Claude, or Gemini consumer subscription as an API credential.
