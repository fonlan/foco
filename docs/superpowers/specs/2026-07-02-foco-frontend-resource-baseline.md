# Foco Frontend Memory and CPU Baseline

Date: 2026-07-02

Scope: Phase 1 records enough baseline data to decide whether the next optimization should target first-load downloads, idle-time dynamic imports, or streaming render work. No production code changes in this phase.

ponytail: this is a static bundle audit plus one Chrome CDP smoke baseline, not a full performance lab. It will miss machine-specific behavior and real backend variance; upgrade path is an automated trace runner checked into the perf suite.

## Build Checked

Ran `npm run build -w web` successfully before taking the asset numbers below.

## Initial Module Preloads

`web/dist/index.html` currently preloads only:

- `/assets/rolldown-runtime-QTnfLwEv.js`
- `/assets/vendor-react-Dctn_cca.js`

The first HTML document does not modulepreload Monaco, Markdown, charts, terminal, or Mermaid chunks. The CSS loaded from the HTML is `/assets/index-Bfa_viHR.css`, and the module entry is `/assets/index-B09_ySle.js`.

## Raw Asset Sizes

| Asset group | File(s) | Raw size |
| --- | --- | ---: |
| main js | `index-B09_ySle.js` | 655,632 B / 640.27 KiB |
| main css | `index-Bfa_viHR.css` | 161,208 B / 157.43 KiB |
| vendor-react | `vendor-react-Dctn_cca.js` | 190,825 B / 186.35 KiB |
| vendor-monaco | `vendor-monaco-B-ePN0zV.js`, `vendor-monaco-Br_kD0ds.css` | 4,327,799 B / 4,226.37 KiB |
| vendor-markdown | `vendor-markdown-egsBwSRK.js` | 600,704 B / 586.63 KiB |
| StatCharts | `StatCharts-BPMnBlew.js` | 329,851 B / 322.12 KiB |
| vendor-terminal | `vendor-terminal-DQewORP6.js`, `vendor-terminal-BrP-ENHg.css` | 345,497 B / 337.40 KiB |
| Mermaid chunks total | see below | 981,898 B / 958.88 KiB |

Largest Mermaid-related chunks:

| Chunk | Raw size |
| --- | ---: |
| `architectureDiagram-3BPJPVTR-DyQKkdDp.js` | 146,934 B / 143.49 KiB |
| `sequenceDiagram-3UESZ5HK-bETz6YIf.js` | 115,732 B / 113.02 KiB |
| `blockDiagram-GPEHLZMM-O9hv18Wn.js` | 72,689 B / 70.99 KiB |
| `c4Diagram-AAUBKEIU-DIw5eP13.js` | 69,170 B / 67.55 KiB |
| `flowDiagram-I6XJVG4X-Dv8fuAKU.js` | 60,220 B / 58.81 KiB |
| `ganttDiagram-6RSMTGT7-De1g8BS7.js` | 54,562 B / 53.28 KiB |
| `vennDiagram-CIIHVFJN-qDs8LZ3L.js` | 40,628 B / 39.68 KiB |
| `xychartDiagram-2RQKCTM6-Bz8htaFB.js` | 38,763 B / 37.85 KiB |
| `quadrantDiagram-W4KKPZXB-CvplU2P6.js` | 33,522 B / 32.74 KiB |
| `mermaid.core-BS37Q9iJ.js` | 32,415 B / 31.66 KiB |
| Remaining Mermaid chunks | 327,263 B / 319.59 KiB |

## Chrome CDP 60s Baseline

Method: served `web/dist` locally and used headless Google Chrome through the DevTools Protocol with a mocked backend. Each scenario waited 60s, then forced a GC before the final sample. CPU numbers are Chrome `Performance.getMetrics()` deltas across the 60s window.

| Scenario | JS heap before | JS heap after GC | Heap delta | Task CPU delta | Script delta | Layout delta | Recalc style delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Chat page idle 60s | 19,153,133 B | 19,050,135 B | -102,998 B | 113 ms | 2 ms | 0 ms | 9 ms |
| Long message streaming 60s | 20,111,090 B | 20,214,068 B | +102,978 B | 13,185 ms | 6,494 ms | 1,999 ms | 1,415 ms |
| File editor open, idle 60s | 20,884,743 B | 20,151,420 B | -733,323 B | 135 ms | 3 ms | 0 ms | 9 ms |

Loaded dynamic assets during the scenarios:

- Chat idle loaded Markdown and Monaco dynamically after the page was open: `MarkdownRenderer-DznOJKWx.js`, `vendor-markdown-egsBwSRK.js`, `vendor-monaco-B-ePN0zV.js`, `vendor-monaco-Br_kD0ds.css`.
- Long streaming loaded the same Markdown and Monaco chunks, then spent most CPU on the streaming render path.
- File editor loaded Monaco, as expected, and did not load Markdown.
- No chart, terminal, or Mermaid chunks were loaded in these scenarios.

## Attribution

First-load HTML preload is already narrow: runtime plus React only. The stronger idle risk is the dynamic import path: Monaco is still pulled during chat idle before opening the editor, and Markdown loads as soon as chat content renders. Streaming is the main CPU hotspot in this baseline: roughly 13.2s task time over 60s for 1,800 deltas, with about half of that in script and visible layout/style cost. The file editor idle case looks stable once Monaco is loaded.
