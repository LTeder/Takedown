# Platform and Architecture Study: Voice-to-Synth/Drum/MIDI Desktop Tool

*Research report from the platform/engineering pass, 2026-10-20. Prices and versions were checked against the sources linked inline. Items marked "verify" rest on forum or secondary sources.*

## Corrections to the brief

Two premises in the brief are out of date and change the analysis.

1. **VST3 is no longer dual-licensed.** Steinberg moved the VST3 SDK to the **MIT license** with VST 3.8 on 2025-10-29. A closed-source commercial product needs no Steinberg agreement any more, only the MIT notice ([Steinberg press release](https://ocl-steinberg-live.steinberg.net/_storage/asset/819253/storage/master/Press%20Release%20-%202025-10-29%20-%20VST%203.8%20-%20EN.pdf), [Sonicstate](https://sonicstate.com/news/2025/10/30/vst-3-now-available-under-mit-license)). ASIO went the other way: it is now **dual GPLv3 or proprietary** ([LibreArts](https://librearts.org/2025/11/steinberg-relicenses-vst3-and-asio/)).
2. **JUCE 9 shipped on 2026-07-21.** Native CLAP authoring was promised for JUCE 9 ([roadmap](https://juce.com/blog/juce-roadmap-update-q3-2025/)), but the release notes list CLAP-specific features as part of the in-progress "AudioProcessor v2" ([JUCE 9 announcement](https://forum.juce.com/t/juce-9-is-available-now/69175), [KVR](https://www.kvraudio.com/news/juce-9-now-available-67802)). Plan on **clap-juce-extensions** for CLAP for now ([repo](https://github.com/free-audio/clap-juce-extensions)).

Also note that **nih-plug is in maintenance mode**. Its README points users to the community fork **nice-plug** under the RustAudio organisation on Codeberg ([nih-plug](https://github.com/robbert-vdh/nih-plug), [nice-plug](https://codeberg.org/RustAudio/nice-plug)).

---

## 1. Framework comparison

| Framework | Lang | VST3 | AU | AAX | CLAP | Standalone | License / cost |
|---|---|---|---|---|---|---|---|
| **JUCE 9** | C++ | yes | yes (v2/v3) | yes | via clap-juce-extensions (native pending) | yes (rich device manager) | AGPLv3, or commercial: Starter free (≤$20k revenue), Indie $40/mo or $800 perpetual (≤$300k), Pro $175/mo (12-mo minimum) or $3,500 perpetual ([juce.com/get-juce](https://juce.com/get-juce/)) |
| **iPlug2** | C++ | yes | v2/v3 | yes (Native) | yes (recent) | yes | zlib-style, free ([ecosyste.ms mirror](https://awesome.ecosyste.ms/projects/github.com%2FiPlug2%2FiPlug2)) |
| **nih-plug / nice-plug** | Rust | yes | no | no | yes | yes (JACK/CPAL) | ISC. nih-plug's VST3 bindings are **GPLv3** (README), and an open issue tracks moving to permissive bindings now that the SDK is MIT |
| **DPF** | C++ | yes | no (per README) | no | yes | JACK/standalone (for testing) | ISC ([DPF](https://github.com/DISTRHO/DPF)) |
| **Dplug** | D | yes | v2 | yes | yes | no | Mixed permissive licenses ([Dplug](https://github.com/AuburnSounds/Dplug)) |
| **Cmajor** | DSL + C++ | via generated JUCE/CLAP projects | via JUCE | via JUCE | yes | patch player | Own license/EULA. A DSP language, not an app framework |
| **HISE** | C++/JS on JUCE | yes | yes | yes | not stated | yes | GPLv3, or HISE commercial **plus** JUCE commercial ([HISE](https://github.com/christophhart/HISE)) |
| **Tracktion Engine** | C++ on JUCE | n/a (DAW engine) | | | | | GPLv3 or commercial. Useful for clip/edit/render plumbing, too heavy for this tool |
| **clap-wrapper** | C/C++ | from CLAP | v2/v3 | **yes (since 0.15)** | native | yes (0.17: native audio/MIDI on all three OSes) | MIT. AAX/ASIO SDK terms still apply ([repo](https://github.com/free-audio/clap-wrapper), [releases](https://github.com/free-audio/clap-wrapper/releases)) |

**Which give VST3 + AU + AAX + CLAP + standalone from one codebase:**
- **JUCE**, with the CLAP extension.
- **iPlug2**.
- **Any CLAP-native plugin plus clap-wrapper.** This now includes Rust plugins in principle, because clap-wrapper only needs a CLAP entry point. That is untested here, so verify it.
- **Dplug** covers VST3, AU, AAX and CLAP but has no standalone.
- **nih-plug, nice-plug and DPF** stop at VST3 and CLAP (plus LV2 for DPF).
- **HISE and Cmajor** are domain tools for sample-based instruments and DSP. They are a poor fit for an ML analysis app with a piano-roll editor.

**C++ vs Rust tradeoffs**
- **Rust strengths:** memory safety in a codebase that mixes threads, ML and file I/O; good tooling (cargo, `xtask` bundling); clean CLAP bindings (clack).
- **Rust weaknesses for this product:**
  - no AU or AAX without a wrapper;
  - plugin GUI toolkits are less mature (below);
  - a smaller hiring pool;
  - the leading framework just changed maintainers;
  - the ML runtime is still C++ underneath anyway (`ort` 2.0 is still a release candidate, rc.13, wrapping ONNX Runtime 1.28: [docs.rs/ort](https://docs.rs/crate/ort/latest)).
- **Pure-Rust inference** (tract, candle, burn) avoids C++ linking, but its operator coverage and performance on CNN/RNN pitch models must be benchmarked per model.

**Rust GUI for plugins in 2026**
- The options are egui (immediate mode), iced, vizia (declarative, Skia-based), and Slint (WIP adapter), all on **baseview** ([nice-plug](https://codeberg.org/RustAudio/nice-plug), [egui-baseview](https://zff.dev/prokopyl/egui-baseview)).
- Developers report keyboard and text-input problems in some hosts and macOS crashes that needed a patched baseview ([GlicolVerb write-up](https://www.vmunix.com/posts/glicol-verb-first-release)).
- Webview UIs (wry) are possible but not standardised for plugins.
- A dense piano-roll editor is achievable in egui or vizia, but expect to own more of the windowing and host-integration bugs yourself.

---

## 2. AAX specifics

- **SDK licensing.** The SDK sits behind a click-through on [developer.avid.com/aax](https://developer.avid.com/aax/). For a commercial release you email audiosdk@avid.com for the "necessary tools and license". Since SDK 2.8, the source headers describe dual commercial/GPLv3 terms, and clap-wrapper documents AAX as "GPL3 or commercial Avid agreement" ([clap-wrapper](https://github.com/free-audio/clap-wrapper), [JUCE forum](https://forum.juce.com/t/aax-open-source-release/44017)). A closed product takes the commercial route.
- **Signing.** A release AAX must be signed with PACE **Eden/wraptool** and an iLok, or it won't load in retail Pro Tools. Unsigned builds run only in the Pro Tools Developer build.
- **Cost.**
  - Avid currently covers the PACE Eden/Central licences for registered developers. They renew annually; one March 2026 account shows them valid to 2027-04-01.
  - You need an iLok USB key (about $50–70).
  - PACE cloud signing (for CI without a dongle) is a paid enterprise add-on, reportedly around $1k.
  - Sources: [note.com March 2026 memo](https://note.com/kawato3/n/ne11473420ad5?hl=en), [Moonbase 2025 round-up](https://moonbase.sh/articles/code-signing-audio-plugins-in-2025-a-round-up/).
- **Timeline.** One developer got a Developer-build code the same day and PACE tools in about a week. Commercial agreements can take longer. Budget 2–6 weeks, plus Pro Tools-specific QA.
- **CI.** You need a self-hosted runner with the iLok attached, or PACE cloud.
- **Recommendation: defer AAX to phase 3.** Pro Tools is also the weakest host for this workflow (MIDI routing is awkward; see §4), and the drag-out workflow serves Pro Tools users without AAX at all. Start the paperwork early (week 1) so it doesn't gate the release.

---

## 3. VST3 licensing in 2026

- Under MIT, a closed-source product can bundle VST3 freely. You keep the copyright and licence notice, and the "VST" logo/trademark guidelines still apply separately.
- The old GPLv3-or-proprietary agreement was the reason nih-plug's VST3 bindings were GPL and the reason CLAP-first developers used to ship CLAP-only builds or write their own bindings.
- That concern is now largely historical. The remaining trap is **GPL-licensed bindings** such as nih-plug's `vst3-sys`, not the SDK. Swap them for MIT bindings, or ship VST3 via clap-wrapper (MIT).
- CLAP itself is MIT with no membership required.

---

## 4. Plugin design and host behaviour

**Two plugin shapes**
- **(A) Instrument with an audio sidechain.** It sits on an instrument track, receives the voice on a sidechain bus, renders synth or drum audio, and exposes a MIDI/note output. It is one plugin that does both jobs.
- **(B) Audio effect with MIDI output.** It sits on the vocal track. Most hosts **won't route MIDI out of an audio-effect slot**.
- VST3 has no "MIDI effect" category. A MIDI generator is built as an instrument with an event output bus ([JUCE forum](https://forum.juce.com/t/midi-generator-plugin-im-distributing-a-vst3i-could-i-distribute-a-vst3-midi-effect-too-for-ableton-cubase-fl-studio/65814)).
- **Choose (A) for real time, and make the primary workflow capture-based:** the plugin records the sidechain into a take buffer, analyses offline, then renders or drags out.

**Host behaviour**

| Host | Audio into an instrument | Plugin MIDI out to other tracks | Drag MIDI/audio out of plugin window |
|---|---|---|---|
| **Ableton Live 12** | VST3/AU sidechain on plugins (verify per instrument) | Yes for VST: a second MIDI track sets "MIDI From" to the plugin. **Channels are merged**, and **AU has no MIDI out** ([Ableton help](https://help.ableton.com/hc/en-us/articles/209070189-Accessing-the-MIDI-output-of-a-VST-plug-in)) | Works, with historical VST3 file-dialog quirks ([JUCE forum](https://forum.juce.com/t/performexternaldragdropoffiles-not-working-in-ableton/42675)) |
| **Logic Pro 11** | AU instrument sidechain menu. Effect-type AUs in the instrument slot get audio only via mono/stereo sidechain ([JUCE forum](https://forum.juce.com/t/input-bus-layout-in-aumf-au-effects-in-logic/59902)) | Yes since 10.8/11: Internal MIDI In → "Instrument Output" records an instrument's MIDI on another track ([Apple guide](https://support.apple.com/guide/logicpro/route-midi-internally-software-instrument-lgcp1efa7c4d/mac), [VI-Control](https://vi-control.net/community/threads/falcon-using-midi-output-for-logic.149934/)). Earlier versions were buggy. AU MIDI FX (`aumi`) cannot receive audio | Works (AU) |
| **Cubase/Nuendo** | VST3 instrument sidechain | VST3 event output; reports of selecting a plugin as a MIDI source are mixed, so verify. No CLAP yet (announced) | Works |
| **Pro Tools** | AAX instrument key input | AAX MIDI output nodes routed into MIDI tracks; clunky multi-track setup ([Blue Cat](https://www.bluecataudio.com/Blog/tip-of-the-day/using-midi-fx-in-pro-tools/)). No CLAP | Dropping files onto the timeline works; test from the plugin window |
| **Bitwig** | Sidechain on any device | First-class: note output plus Note Receiver; CLAP note ports native | Works (some reports of failures) |
| **Reaper** | Any routing | Full: MIDI sends to any track or hardware | Works |
| **FL Studio** | Mixer sidechain to generator | Via the wrapper's MIDI output port number ([IL manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/MIDI%20Out.htm)). CLAP support reported since FL 2024 (sources conflict) | Works |
| **Studio One** | Instrument sidechain | VST3 MIDI out routable to other instruments (user reports); CLAP since v7 | Works |

CLAP hosts are Bitwig, Reaper, Studio One and (reportedly) FL Studio. Not Ableton, Logic, Pro Tools or Cubase as of 2026 ([cleveraudio.org hosts](https://cleveraudio.org/hosts-and-plug-ins/), [Spectral Colors](https://spectral-colors.com/news/clap-vs-vst3-2026/)). **That means VST3 and AU, not CLAP, are needed to reach the biggest DAWs.**

**Drag-out implementation.** Write a temporary `.mid` or `.wav` file, then call the OS drag (`performExternalDragDropOfFiles` in JUCE). Behaviour varies by host, format and OS, so keep a host test matrix ([JUCE forum](https://forum.juce.com/t/external-drag-and-drop-problem-in-plugin-with-some-daws/31380)).

**Why Dubler and Imitone are standalone apps with virtual MIDI.** In-plugin MIDI routing is inconsistent across hosts (table above). A standalone app owns the microphone, ASIO and latency settings once, and a virtual MIDI port works in every DAW, including Logic and Pro Tools, with no plugin format at all. Dubler 2 does exactly this, with an optional "MIDI Capture" plugin as the bridge ([MusicRadar](https://www.musicradar.com/reviews/vochlea-dubler-2), [Vochlea support](https://vochlea.com/learn/dubler-not-showing-as-a-midi-input-2)).

**Virtual MIDI port by platform**
- **macOS:** `MIDISourceCreate`/`MIDIDestinationCreate` in CoreMIDI create app-owned endpoints, no driver needed. The IAC bus is the user-configured fallback.
- **Linux:** ALSA sequencer ports, or JACK MIDI ports. Both are trivial.
- **Windows:**
  - Classic WinMM has no app-created ports. Options are bundling or recommending loopMIDI/virtualMIDI (Tobias Erichsen's driver is licensable for commercial redistribution), TE's virtual driver, or writing a kernel/UMDF driver (signing burden).
  - **Windows MIDI Services** (Windows 11) adds in-box loopback endpoints that also surface as MIDI 1.0 ports. They are in developer preview now, with consumer rollout stated as November 2026 to early 2027 (Microsoft's page now says 2027) ([MIDI loopback docs](https://microsoft.github.io/MIDI/tools/midiloopbacksetup/), [Windows blog](https://blogs.windows.com/windowsexperience/2026/02/17/making-music-with-midi-just-got-a-real-boost-in-windows-11/)).
  - **Plan:** use Windows MIDI Services loopbacks where present, fall back to a licensed virtualMIDI driver, and defer this to v0.3.

---

## 5. Standalone audio I/O

- **JUCE AudioDeviceManager** covers CoreAudio, WASAPI (shared, exclusive and low-latency), DirectSound, ASIO, ALSA and JACK. JUCE 9 improves CoreAudio aggregate devices.
- **RtAudio/RtMidi** (MIT-style), **PortAudio** (MIT) and **miniaudio** (public domain/MIT-0; WASAPI including exclusive mode) are alternatives for a non-JUCE shell. clap-wrapper's standalone now uses native backends, with RtAudio behind a switch.
- **CPAL** (Rust 0.18) defaults to WASAPI and has an opt-in `asio` feature ([docs.rs/cpal](https://docs.rs/crate/cpal/latest)).
- **ASIO licensing:** GPLv3, or Steinberg's proprietary agreement. A closed product signs the (free) proprietary agreement and does not redistribute the SDK headers.
- **WASAPI exclusive mode or IAudioClient3 low-latency shared mode** gets about 3–10 ms without ASIO on most interfaces. Offer both.
- **macOS microphone permission:** `NSMicrophoneUsageDescription` in Info.plist, plus the `com.apple.security.device.audio-input` entitlement under the hardened runtime (`device.microphone` if App Store sandboxed). Request access explicitly via AVCaptureDevice so the prompt appears ([Apple forum](https://developer.apple.com/forums/thread/768950), [JUCE forum](https://forum.juce.com/t/issue-notarizing-standalone-macos-microphone-access/55117)). Plugins inherit the host's permission.
- **Rates and buffers:**
  - Capture at the device rate and resample to the model rate (typically 16 kHz) on the analysis thread.
  - Render at the host or device rate.
  - Never assume a fixed block size; handle `prepareToPlay` changes and variable blocks.

---

## 6. ML inference in a plugin

- **ONNX Runtime (recommended default).** Build from source as a static minimal build, with ORT-format models and operator reduction ([ORT custom builds](https://onnxruntime.ai/docs/build/custom.html)). Expect a few MB per platform.
  - Hide symbols (`-fvisibility=hidden`, version scripts): hosts may load their own ORT and cause ODR clashes ([ort-builder warning](https://awesome.ecosyste.ms/projects/github.com%2Folilarkin%2Fort-builder)).
  - **Execution providers:** the CPU EP is safe everywhere. CoreML is acceptable for offline analysis on macOS. Avoid CUDA/TensorRT (driver and size). DirectML is optional for offline jobs only.
  - Set `intra_op_num_threads` small so you don't starve the host.
- **RTNeural** (header-only, BSD) suits small real-time RNN/CNN models, such as a low-latency monitoring pitch tracker.
- **LibTorch** is hundreds of MB, so avoid it in a plugin.
- **ggml** fits transformer-style models but is overkill here.
- **anira** (C++) wraps ORT, LibTorch and TFLite on a dedicated thread pool with latency management, and has JUCE and CLAP examples ([paper](https://arxiv.org/html/2506.12665v1)).
- A DAFx-2026 study warns that isolated benchmarks mislead under DAW load ([DAFx](https://dafx.de/paper-archive/details/rBxXewYH3YxRTiGXqSabwQ)).
- **Rust options:** `ort` (2.0 RC), tract (pure Rust, ONNX subset), candle, burn.

**Threading model**
- The audio thread only pushes samples into a lock-free single-producer/single-consumer (SPSC) ring and reads pre-rendered buffers.
- Analysis runs on a worker pool.
- Jobs carry an atomic cancel token and a progress counter that the UI polls on a timer.
- Results are published by atomic pointer swap, with immutable take snapshots.
- Check real-time safety with RealtimeSanitizer (rtsan).

**How shipping products do it**
- **Neutone:** JUCE host plus TorchScript models downloaded on demand ([arXiv](https://arxiv.org/pdf/2508.09126)).
- **DDSP-VST:** embeds TFLite.
- **Neural Amp Modeler:** a small Eigen-based MIT core that loads `.nam` JSON weights ([NAM core](https://github.com/sdatkinson/NeuralAmpModelerCore)).
- **Lesson:** keep weights as versioned data files with a hash, not compiled into the binary, so models can update without a plugin release.

**Candidate models**
- **Spotify basic-pitch** (Apache-2.0; ONNX/CoreML/TFLite provided, polyphonic) ([repo](https://github.com/spotify/basic-pitch)).
- **CREPE/torchcrepe** (MIT).
- Verify the weight licences separately (RMVPE's licence is unclear).

---

## 7. GUI

The editor needs a waveform with the f0 overlay, a piano roll with note drag, split, merge and quantise, a drum-hit lane, and transport.

- **JUCE Graphics** (CPU or Direct2D on Windows, CoreGraphics on macOS) handles this well. Its accessibility hooks map to UIA and NSAccessibility, HiDPI works, and it is the lowest-risk option inside hosts.
- **JUCE 8/9 WebView** (WebBrowserComponent with a resource provider, parameter relays and native-function calls) lets a web team build the editor in TS/Canvas or WebGL ([JUCE docs](https://docs.juce.com/master/juce__WebBrowserComponent_8h.html)). Costs:
  - WebView2 runtime dependency on Windows;
  - WebKitGTK problems on Linux (resource-provider crashes reported in 8.0.8) ([forum](https://forum.juce.com/t/webbrowsercomponent-very-poor-linux-experience/66681));
  - per-instance memory;
  - occasional focus and keyboard issues.
- **Dear ImGui** is fast to build but weak on accessibility and polished visuals.
- **Rust** options are covered in §1.
- **Recommendation:** JUCE Graphics for the piano roll and waveform, where performance and drag-out matter. WebView remains an option for settings and onboarding panels later.

---

## 8. Project structure, build, CI

- **Template.** Start from **Pamplejuce**: JUCE 9 submodule, C++23, CMake ≥3.25, CPM, Catch2 3.8, pluginval, GitHub Actions on Windows, macOS and Linux, macOS notarization, and Windows signing via Azure Trusted Signing ([Pamplejuce](https://github.com/sudara/pamplejuce)). Add `clap_juce_extensions_plugin()` for CLAP.
- **macOS.** Build universal (arm64 + x86_64). Apple Developer Program ($99/yr), Developer ID signing, `notarytool`, and stapling for the app and every plugin bundle.
- **Windows.**
  - **Azure Artifact Signing** costs $9.99/mo Basic ([pricing](https://azure.microsoft.com/en-us/products/artifact-signing)). Eligibility is limited to organisations in the US, Canada, EU and UK, and individuals in the US and Canada (secondary source, verify).
  - Otherwise use an OV or EV certificate held in a cloud KMS (about $200–300/yr).
- **Installers:** pkgbuild/productbuild on macOS, Inno Setup or WiX on Windows, and a tarball (optionally Flatpak) on Linux.
- **Rust equivalent:** `cargo xtask bundle` from nih-plug.
- **Validation in CI:** pluginval (strictness 10), clap-validator, `auval -a`, and `validator` (VST3 SDK). Also host smoke tests in Reaper (scriptable).
- **Unit tests:**
  - Golden-file tests for analysis (WAV in, notes JSON out, with tolerance);
  - null tests for the renderer;
  - property tests on the note editor's undo history;
  - Python-side regression notebooks driven through the bindings.

---

## 9. Licensing of dependencies and content

- **Avoid GPL/AGPL code in the shipped binary** unless you buy the commercial licence. That covers JUCE under AGPL, HISE, GPL synth engines such as Surge XT and Vital's source, and GPL VST3 bindings. LGPL is workable only with dynamic linking and relinking rights, which is awkward inside plugin bundles.
- **Permissive dependencies are fine:** MIT, BSD, ISC, Apache-2.0, zlib, Boost. Ship a third-party notices file.
- **Model weights are licensed separately from code.** Many research checkpoints are CC-BY-NC or "research only" and cannot ship commercially. Train your own on licensed data, or use Apache/MIT weights (basic-pitch, torchcrepe). Record dataset provenance; datasets such as NSynth, MAESTRO and VocalSet each have their own terms.
- **Samples and drum kits:**
  - CC0 and CC-BY can be bundled (with attribution for CC-BY).
  - CC-BY-NC cannot be bundled.
  - CC-BY-SA forces share-alike on the derived sample pack.
  - Commercial sample libraries usually forbid redistributing raw samples inside another instrument. Record or synthesise your own kits, or license OEM content explicitly.
- **SDK obligations:**
  - VST3: MIT notice.
  - ASIO: proprietary agreement for closed source.
  - AAX: Avid agreement plus PACE.
  - AU: Apple's licence, no fee.

---

## 10. Recommended architecture

```
voxsynth/
  core/            # C++20 library, NO JUCE, NO UI: analysis, rendering, take model, serialization
    analysis/      # f0 (ORT/RTNeural), onset/drum classifier, note segmentation
    render/        # synth + drum sampler engines, offline + realtime render
    model/         # Take, NoteEvent, DrumHit, RenderSettings, command/undo
    io/            # project format, MIDI/MPE export, WAV/FLAC export
  bindings/python/ # nanobind -> pip-installable wheel for research/regression tests
  app/             # JUCE shell: standalone (device mgr, virtual MIDI, file browser)
  plugin/          # JUCE AudioProcessor shell: VST3/AU/CLAP(/AAX later)
  ui/              # shared JUCE editor components (piano roll, waveform, inspector)
  tests/  tools/  models/ (versioned, hashed weight files)
```

- **The core never includes JUCE.** That keeps it licence-clean, reusable from Python, and portable to a future Rust or CLAP-native shell. It depends only on std, a small DSP library, ONNX Runtime/RTNeural and a serialisation library.
- **The plugin and the standalone** share `core/` and `ui/`. JUCE's standalone wrapper can host the same `AudioProcessor`. The app target adds the device manager, virtual MIDI and a richer file workflow.
- **Take data model (immutable snapshots):**
  - `Take { id, audio: ref to WAV/FLAC + sampleRate, f0: {hop, freqHz[], confidence[]}, onsets[], notes: [{start, dur, pitch(float), velocity, pitchBend curve, pressure/timbre curves for MPE}], drumHits: [{time, class, velocity, confidence}], renderSettings: {engine, preset, quantize, scale, mpeEnabled, …}, analysisVersion, modelHashes }`.
  - Re-analysis is triggered when `analysisVersion` or `modelHashes` change.
- **Project file:** a zip container (`.vxs`) with `project.json` (schema-versioned, with migrations), the audio files and a cached analysis. In the plugin, store a compact state (take ids plus settings) in host state, and keep audio in a project-relative folder or embed it below a size threshold.
- **Undo:** a command pattern over immutable take snapshots, with structural sharing for note arrays and coalescing for drag gestures. The UI thread owns the history; the audio thread sees only atomically swapped render snapshots.
- **MIDI/MPE export:** standard MIDI file for notes. MPE (per-note channel, pitch-bend ±48 semitones, CC74, channel pressure) from the f0 and timbre curves. Same data for live MIDI out.

---

## Decision matrix

| | Formats covered | AAX path | ML integration | GUI effort (piano roll) | Licensing cost | Hiring / community | Risk |
|---|---|---|---|---|---|---|---|
| **JUCE C++** | VST3, AU, AAX, standalone native; CLAP via extension (native pending) | Built-in; PACE signing only | Native C++ ORT/RTNeural/anira; reference products (Neutone) | Moderate: mature 2D, a11y, HiDPI, drag-out | $800 perpetual (Indie ≤$300k) or $3,500 (Pro); AGPL otherwise | Largest pool, forum, templates | **Low.** CLAP via extension is a small, known risk |
| **nih-plug/nice-plug Rust** | VST3, CLAP, standalone; AU/AAX only via clap-wrapper (unproven from Rust) | Indirect, via clap-wrapper | `ort` RC / tract / candle; C++ underneath | High: baseview toolkits less mature, host input bugs | Free (ISC); check GPL bindings | Small; framework just changed hands | **High** |
| **iPlug2** | VST3, AU, AAX, CLAP, standalone, WAM | Built-in | C++ (NAM plugin is the reference) | Moderate–high: thinner widget set, smaller docs | Free | Small but expert | Medium |
| **Hybrid Rust core + JUCE shell** | Same as JUCE | Same as JUCE | Rust `ort`/tract in core via C ABI (cxx/cbindgen) | Same as JUCE | JUCE cost | Two-language team; harder hiring | Medium: FFI, two build systems, duplicate allocators and threading |

## Recommendation

**Use JUCE 9 (C++20) shells over a JUCE-free C++ core library, with ONNX Runtime (static, CPU EP, hidden symbols) plus RTNeural, and a JUCE Graphics editor.** Buy JUCE Indie ($800 perpetual, or $40/mo) and upgrade to Pro past $300k revenue.

**Rationale**
- It is the only option that covers every required format, including AAX and AU, with a mature standalone and device layer, from one codebase today.
- The ML runtimes are native C++.
- Drag-out, sidechain bus layouts and host quirks are well trodden in JUCE.
- Hiring is easiest.
- The JUCE-free core keeps the door open: if JUCE's cost or CLAP plans disappoint, the same core can go behind a CLAP-native shell plus clap-wrapper (MIT, now with VST3, AU, AAX and standalone) without rewriting analysis or rendering.
- Rust is attractive for safety, but today it adds risk on GUI, format reach and maintenance without solving the ML runtime question.

## Phase plan

**v0.1 (about 12–16 weeks): standalone + VST3 + CLAP, one OS**
- **OS choice:** Windows x64 if the team develops on Windows (as here), since that is the largest hobbyist base and VST3/CLAP are the native formats. Keep macOS CI builds compiling from day 1.
- **Features:** record a take, then offline analysis (monophonic f0 plus note segmentation, plus beatbox onsets with 3–5 drum classes).
- **Editing:** a basic piano-roll edit with undo.
- **Rendering:** one subtractive synth plus one drum kit, all your own content.
- **Export:** WAV and SMF export, and drag-out to the DAW.
- **Plugin:** an instrument with a sidechain input, capture-based.
- **Standalone I/O:** WASAPI shared and exclusive; ASIO only after signing Steinberg's proprietary agreement.
- **CI:** pluginval and clap-validator.
- **Paperwork:** start Avid/PACE, Apple Developer, D-U-N-S and Windows signing now.

**v0.2: macOS**
- Universal build, AU, notarization, CoreMIDI virtual output from the standalone.
- Low-latency monitoring mode (RTNeural pitch tracker, block-based note-on).
- Plugin MIDI output (event bus), with host guides for Live, Logic, Cubase, Bitwig and Reaper.

**v0.3**
- MPE export and live MPE out.
- Windows virtual MIDI (Windows MIDI Services loopback, or a licensed virtualMIDI driver).
- More synth engines and presets; model updater.
- Linux builds (ALSA/JACK) as beta.

**v1.0**
- **AAX** (PACE-signed, iLok runner or PACE cloud), Pro Tools QA.
- Native JUCE CLAP when it lands.
- Polyphonic and chord detection (basic-pitch-class models), accessibility pass, installer polish.

**Deferred or not planned:** AUv3/iOS, a WebView-based editor, GPU execution providers, cloud rendering.
