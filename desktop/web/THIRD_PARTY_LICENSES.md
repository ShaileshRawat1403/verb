# Browser workbench third-party licences

Everything below is bundled into `dist/app.js` or `dist/app.css`, or served from `fonts/`, and is
embedded in the Verb Desktop binary. Each notice file is under `LICENSES/` and ships with release
archives. Versions are the ones locked in `package-lock.json`; the font files match the Fontsource
packages byte for byte.

## npm packages

All MIT. The notice file is the package's own licence file, copied unchanged.

| Package | Version | Notice |
| --- | --- | --- |
| `@codemirror/autocomplete` | 6.20.4 | `LICENSES/codemirror-autocomplete.txt` |
| `@codemirror/commands` | 6.11.1 | `LICENSES/codemirror-commands.txt` |
| `@codemirror/lang-css` | 6.3.1 | `LICENSES/codemirror-lang-css.txt` |
| `@codemirror/lang-html` | 6.4.12 | `LICENSES/codemirror-lang-html.txt` |
| `@codemirror/lang-javascript` | 6.2.5 | `LICENSES/codemirror-lang-javascript.txt` |
| `@codemirror/lang-json` | 6.0.2 | `LICENSES/codemirror-lang-json.txt` |
| `@codemirror/lang-python` | 6.2.1 | `LICENSES/codemirror-lang-python.txt` |
| `@codemirror/language` | 6.13.1 | `LICENSES/codemirror-language.txt` |
| `@codemirror/lint` | 6.9.7 | `LICENSES/codemirror-lint.txt` |
| `@codemirror/state` | 6.7.6 | `LICENSES/codemirror-state.txt` |
| `@codemirror/view` | 6.43.15 | `LICENSES/codemirror-view.txt` |
| `@lezer/common` | 1.5.4 | `LICENSES/lezer-common.txt` |
| `@lezer/css` | 1.3.9 | `LICENSES/lezer-css.txt` |
| `@lezer/highlight` | 1.2.5 | `LICENSES/lezer-highlight.txt` |
| `@lezer/html` | 1.3.13 | `LICENSES/lezer-html.txt` |
| `@lezer/javascript` | 1.5.6 | `LICENSES/lezer-javascript.txt` |
| `@lezer/json` | 1.0.3 | `LICENSES/lezer-json.txt` |
| `@lezer/lr` | 1.4.11 | `LICENSES/lezer-lr.txt` |
| `@lezer/python` | 1.1.19 | `LICENSES/lezer-python.txt` |
| `@marijn/find-cluster-break` | 1.0.4 | `LICENSES/marijn-find-cluster-break.txt` |
| `@xterm/xterm` | 5.5.0 | `LICENSES/xterm.txt` |
| `@xterm/addon-fit` | 0.10.0 | `LICENSES/addon-fit.txt` |
| `@xterm/addon-image` | 0.8.0 | `LICENSES/xterm-addon-image.txt` |
| `@xterm/addon-search` | 0.15.0 | `LICENSES/xterm-addon-search.txt` |
| `@xterm/addon-unicode11` | 0.8.0 | `LICENSES/xterm-addon-unicode11.txt` |
| `@xterm/addon-web-links` | 0.11.0 | `LICENSES/xterm-addon-web-links.txt` |
| `@xterm/addon-webgl` | 0.18.0 | `LICENSES/xterm-addon-webgl.txt` |
| `crelt` | 1.0.7 | `LICENSES/crelt.txt` |
| `style-mod` | 4.1.4 | `LICENSES/style-mod.txt` |
| `w3c-keyname` | 2.2.8 | `LICENSES/w3c-keyname.txt` |
| `qrcode` | 1.5.4 | `LICENSES/qrcode.txt` |
| `dijkstrajs` | 1.0.3 | `LICENSES/dijkstrajs.md` |

The editor packages' notices are also preserved inside `dist/app.js` by
`src/studio-code-notices.js`.

## Libraries prebundled in `@xterm/addon-image`

The add-on ships these already bundled into its own code; they have no separate install path.
All MIT. Their copyright lines and the MIT text are in `LICENSES/xterm-addon-image-bundled.txt`.

| Library | Version |
| --- | --- |
| `sixel` | 0.16.0 |
| `xterm-wasm-parts` | 0.1.0 |
| `inwasm` | 0.0.13 |

## Fonts

SIL Open Font License 1.1. The licence travels with the fonts; they may not be sold on their own.

| Files under `fonts/` | Source | Notice |
| --- | --- | --- |
| `barlow-400.woff2`, `barlow-500.woff2`, `barlow-600.woff2` | `@fontsource/barlow` 5.3.0, latin | `LICENSES/barlow-OFL.txt` |
| `barlow-condensed-600.woff2`, `barlow-condensed-700.woff2` | `@fontsource/barlow-condensed` 5.3.0, latin | `LICENSES/barlow-condensed-OFL.txt` |
