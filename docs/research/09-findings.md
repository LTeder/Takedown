# 09 appendix: the reviewers' findings, as returned

*2026-11-10. Raw material for [09-re-evaluation.md](09-re-evaluation.md): twelve Sonnet reviewers, one lens each, and one skeptic per lens who tried to refute each finding. Generated from the agents' structured output and not edited. Treat it as claims, not conclusions: the skeptics confirmed 27 of 92 findings outright, corrected 60, could not check 5 and refuted none. Claims about other products and the outside world were checked only by the agents' own web searches.*

## Product thesis and landscape

The thesis is sound but the repo is built around it, not toward it. The differentiator is the designed sound: a whole-take, offline voice-to-machine-sound renderer with in-voice mangle, which no competitor does. Competitors (Dubler 2, DubBox, Humanoid, Synplant 2) all trigger samples or resynthesise live. Yet about 21,000 of the repo's lines are types, contract and evidence, and zero are sound. The Week-0 test that decides whether the thesis holds has been delayed twice, and the plan treats the MIDI half as equal to the audio half. The smallest proof is one fill through AI-synthesised hits with mangle, judged blind in Reaper. Build that before any more infrastructure.

### 1. The differentiator is untested, and the repo has built infrastructure around it

*critical, rethink, reviewer confidence high; skeptic: **partly***

- **Claim.** The real differentiator is designed sound from voice gesture (voice as control, mangle as character), not detection. Detection is the commodity that Dubler, DubBox, Ableton and Melodyne already sell. About 21,000 lines are core, render contract and evidence, 320 are CLI, and nothing synthesises. The week-4 gate was the plan's first sound test, and the Week-0 sound check has slipped twice.
- **Evidence.** wc -l: core 14,561, render 3,828 (contract only), evidence 2,140. MASTER-PLAN §5 Week 0 and Week-4 gate; STATUS.md Next 1 and 6; DECISIONS 2026-11-09 (Week 0 handed to the founder's own synths).
- **Recommendation.** Stop infrastructure. Build one throwaway end-to-end path, onset to loudness, brightness and duration to a synthesised hit plus distortion, as a Python or small Rust offline render. Run the blind test on it before the lens, midi or app crates grow.
- **Skeptic.** Line counts check out: core 14,561, render 3,828, evidence 2,140, cli 320, drums 2, midi 1. But the plan already puts a throwaway Python Week 0 first, and precheck.py exists. The 'slipped twice' claim has no support in STATUS or DECISIONS.
- **Corrected claim.** About 20,500 lines of core, render contract and evidence exist and nothing synthesises. The plan's own Week 0 sound check was reframed on 2026-11-09 to use the founder's synths and has not been run; the real gap is that it was not completed before the infrastructure grew.

### 2. The plan never says who it is for

*major, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** No document names a user beyond the founder. The plan is silent on whether this is a personal instrument or a product. That decides whether MIDI, the app and plugin, golden-set generalisation and 'Mouth amount' dice matter. The 'audience of one' phrasing appears only in the devil's-advocate review (05), not in the plan. The hard-rule list reads as product architecture for a user base that does not exist yet.
- **Evidence.** MASTER-PLAN §1 and §2 name only the founder's workflow and palette; docs/research/05 line 212 ('an audience of one'); CLAUDE.md 'Built by Logan'.
- **Recommendation.** Write one paragraph: 'v0.1 is a personal instrument for Logan in Reaper; a public release is decided after the week-4 gate.' Defer plugin, notarisation, strict licence machinery and migrations until then.
- **Skeptic.** The plan calls it an open-source tool with a GPL licence ('Founder wants libre') and uses the founder's workflow and palette. 'Audience of one' does appear only in research/05 line 212. But no paragraph states personal instrument versus product, so the gap holds in a weaker form.
- **Corrected claim.** The plan says open-source and founder-driven but never states whether v0.1 is a personal instrument or a public product, and the public-release architecture (strict licences, migrations, plugin in v0.2) is built regardless.

### 3. Dual output (audio plus MPE) is essential in the architecture but a distraction as a proof

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** One canonical track serialising to both audio and MIDI is a good internal discipline. It is also exactly what DubBox, Ableton Convert Drums and Dubler already deliver on the MIDI side. MIDI-to-a-user's-own-synth is the commodity, while the audio render is the differentiator. The plan's definition of done leads with MIDI triggering a drum rack, and the Week-0 script spent 392 lines on MIDI plus a deferred MIDI projection.
- **Evidence.** MASTER-PLAN §3 principle 2 and 'v0.1 definition of done'; research/week0/precheck.py; 01-market-survey §2 (DubBox exports MIDI and audio).
- **Recommendation.** Keep the ControlTrack discipline but demote MPE round-trip tests below hearing a good render. Ship audio-first; treat MIDI as the cheap export that ControlTrack already makes available, not a proof point.
- **Skeptic.** The principle and the definition of done are as cited, but the definition of done ends with MIDI triggering a rack; it does not lead with it. precheck.py is 392 lines in total, not 392 lines of MIDI. DECISIONS says MIDI was chosen to avoid a Python synth binding.
- **Corrected claim.** MIDI is half of the v0.1 definition of done and the Week 0 script is built around MIDI output. It is the commodity half, and the audio render is what the thesis needs proved first.

### 4. Landscape gap holds, but the nearest threat is a competitor adding offline mangle, not another voice-to-MIDI tool

*major, opportunity, reviewer confidence medium; skeptic: **partly***

- **Claim.** As of late 2026, DubBox (July 2026, $39-69) owns capture, quantise and sample trigger for beatbox. Dubler 2 and Imitone are stagnant. Ableton Live 12 converts drums and melody to MIDI, and 12.3 added stem separation. Humanoid is a live voice-to-wavetable synth. Synplant 2 Genopatch matches audio to its own engine and can handle voice poorly. Nobody offers whole-take, hit-by-hit synthesis with mangle from voice gesture. The window is real but gets narrower as DubBox adds pitch and kits.
- **Evidence.** https://mixdownmag.com.au/news/dubbox-a-standalone-app-that-plays-real-drums-from-beatboxing/ (22 Jul 2026); https://www.soundonsound.com/reviews/sonic-charge-synplant-2?amp ; docs/research/01 §1-2; docs/research/05 line 16 (Humanoid).
- **Recommendation.** Frame the pitch as 'my mouth is the gesture, the sound is designed', not as better detection. Position against DubBox's sample triggering, and put time-to-first-good-sound ahead of onset F1 gains.
- **Skeptic.** research/01 documents DubBox (July 2026, MIDI and audio export), a stagnant Dubler, and the open-source Beatbox app that copies the DubBox workflow. I did not re-check the web, the pricing, the Synplant 2 claim or Live 12.3 stem separation. 'Nobody offers' is an absence claim, and 'the nearest threat' is speculation.
- **Corrected claim.** The repo's own survey shows a gap in whole-take designed synthesis, but the threat ranking and the pricing and feature claims are not verified here.

### 5. Evaluation effort is aimed at the commodity half

*major, contradiction, reviewer confidence high; skeptic: **partly***

- **Claim.** Gate E1 (onset F1 at least 0.9, pitch RPA at least 95) and weeks of voicing and octave work measure whether detection works, which the founder already judged 'accurate by ear'. Nothing in the gates measures the thing the product sells: how a designed sound responds to the voice, and whether the output sounds like a machine and not a mouth. Remaining STATUS Next items 2-5 are mostly more detection.
- **Evidence.** STATUS.md Numbers and Next 2-5; MASTER-PLAN §5 Gate E1 versus Week-4 gate; memory 'evaluate by listening'.
- **Recommendation.** Treat detection as good enough and freeze it, with only a level floor and a rise/strength filter for onset false positives. Move effort to a listening protocol for rendered sound: blind, shuffled, with a mouth-amount control.
- **Skeptic.** STATUS Next 2-5 (golden set, voicing in noise, octave errors, drums crate) are detection work, and gate E1 is detection metrics. But the founder-driven listening checks and the Week-4 gate do target the rendered sound. The finding is right on effort allocation but wrong that nothing measures the differentiator.
- **Corrected claim.** Most of the near-term queue is detection, while the one gate that measures designed sound (Week-4, blind and shuffled) is defined in the plan but sits behind lens and render work.

### 6. The new decision (AI-designed sounds) is the actual product, but the plan still treats sound design as palette preset craft the founder tunes

*minor, contradiction, reviewer confidence medium; skeptic: **unverifiable***

- **Claim.** MASTER-PLAN §2 assigns palettes to the founder ('the founder writes their own') and 'Serum-ness is preset craft'. The 2026-11-10 decision says the AI's sound creation is what is required. That moves sound design, the one capability that cannot be tested by unit tests, from Logan to the AI, and from a late task to the first one. The plan has no way to evaluate AI-written patches other than Logan's ears, so the iteration loop (render, listen, change) is the bottleneck.
- **Evidence.** MASTER-PLAN §2 Palettes row, §5 Week-4 ('hand-tuned by the founder with Claude'); task brief (founder decision 2026-11-10).
- **Recommendation.** Update the plan so the AI authors the first palettes. Build a fast render-and-audition loop (a folder of 10 s WAVs per variant, shuffled) before choosing engines. Use Plaits only if it speeds that loop.
- **Skeptic.** MASTER-PLAN §2 'The founder writes their own' and §5 'hand-tuned by the founder with Claude' are confirmed. No 2026-11-10 decision appears in DECISIONS.md, and the cited task brief is not in the repo. The contradiction can't be checked, and Claude is already named as co-tuner in the plan.

### 7. Smallest proof of the thesis is one fill, not a library of crates

*minor, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** The thesis claims that a beatbox fill driven through a designed voice produces something the founder could not make another way. That needs one take, onset detection (already done), per-hit loudness, brightness and duration, one synthesised kick, snare and hat, one distortion, and ten shuffled renders. This takes days, not the plan's four weeks. The repo has the onsets and features, so it is one step from the test, but Week 0 was handed to the founder and a Python sketch instead of being completed.
- **Evidence.** MASTER-PLAN §5 Week 0; STATUS.md Next 1; research/week0/precheck.py; crates/evidence/src/onset.rs exists, crates/drums/src/lib.rs and render voices do not.
- **Recommendation.** Define the proof now as 'one fill, ten renders, six the founder would use, one unmakeable another way'. Write it as the only open item at the top of STATUS.md and defer everything else behind it.
- **Skeptic.** Onset code exists, drums and midi are stubs, and the Week-4 gate needs ten fills. But the plan's Week 0 already is the days-long minimal proof (two days, ten fills, a distortion patch). So 'days, not the plan's four weeks' misreads the plan; the failure is execution, not the plan's design.
- **Corrected claim.** The plan already specifies a two-day proof in Week 0; it was not completed, and the repo went on to build the weeks 1-4 foundation instead.

### Not anticipated

- The founder's own take (fan, window, noise) is a better demo than a clean one: the fact that noise and breath can become sound is what DubBox's sample triggering cannot do. Fix the voicing-in-noise problem for sound (eager), not for evaluation, and let the artefacts be part of the sound.
- A 'mouth amount' control may be the whole product: crossfading from the raw voice onto the designed sound is the one knob competitors cannot offer, and it tests risk #1 directly.
- DubBox exports audio and MIDI, and Ableton exports MIDI. The founder's Reaper drag-out is a weak differentiator, so the useful export might be a rendered stem the founder keeps as audio in the project, since Reaper's own undo, not the plugin, then carries it.
- The fixed-seed reproducibility machinery (14,000 lines of core) only pays off if the founder rerolls and wants to return to a roll. If the first listening test shows one deterministic sound is enough, that machinery was premature.
- A beatbox performance is also a gesture library. Teaching a role (kick, snare) per user by kNN is what Dubler and DubBox already do; the unclaimed option is mapping continuous timbre (brightness, breath) of every hit to sound parameters, not classes.

### Keep

- The ControlTrack discipline (renderer never sees notes, one object two outputs) keeps audio and MIDI from disagreeing, and costs little if the audio path comes first.
- Capture-first offline analysis is a real accuracy advantage over causal competitors, and the founder's by-ear check supports it.
- Mangle as per-voice synthesis, with reverb, compression and OTT left to the DAW, is a good scope boundary for a two-person project.
- Listening-based evaluation next to mir_eval numbers, and keeping 'eager for sound, conservative for evaluation' as a lens setting, is the right split.

### Missed by the reviewer, added by the skeptic

- **Week 0 as rerouted tests the founder's synths, not Takedown's designed sound** (major). DECISIONS 2026-11-09 replaced the Python Plaits or Surge patch with MIDI into the founder's own Reaper synths. That tests whether voice-driven triggers excite him. It does not test the plan's differentiator (a synthesised body plus mangle shaped by voice loudness, brightness and breath), which is what the Week-4 gate targets, so the cheapest check of the thesis was swapped for one that cannot fail on it. Evidence: docs/DECISIONS.md 2026-11-09 'The Week 0 pre-check plays the founder's own synths through MIDI'; MASTER-PLAN §5 Week 0; STATUS Next 1 Recommendation: Add a minimal audio render of per-hit loudness, brightness and decay into one synthesised voice plus distortion, offline in Python, and run the shuffled listen on it before the lens work.
- **Product thesis already names the differentiator and the pivot; the reviewer ignores this** (minor). MASTER-PLAN §1 states the difference from DDSP-VST, Neutone and Humanoid, and the Week-4 gate has explicit continue and pivot rules, including falling back to 'hits as pure trigger plus parameters, which is still a product'. The real weakness is sequencing, not an absent thesis. Recommendations should reference this existing gate rather than propose a new one. Evidence: MASTER-PLAN lines 11-15 and 161 Recommendation: Move the existing Week-4 gate forward by building only the minimum render path needed to run it.
- **An open-source copy of the capture workflow already exists and is not discussed as a threat** (minor). research/01 lists Beatbox by Ala Shiban, open source, copying the DubBox workflow with trained triggers, quantise, layers and WAV or MIDI export. It is a free alternative on the MIDI-and-trigger half, which weakens any pitch resting on 'GPL, open drum trigger' and strengthens the argument that the audio-render half must be the differentiator. Evidence: docs/research/01-market-survey.md line 81 Recommendation: Record it in the positioning section and drop MIDI export from the headline claim.

## First-principles redesign

My from-scratch design was sound-first: sketch voices, listen, then a thin wav to events to render to MIDI slice, with persistence, edits and GUI deferred. The repo is the opposite. About 20k lines exist and none makes sound, while the product thesis, that designed sounds driven by a mouth are good, is untested. The 2026-11-10 decision to synthesise sounds first is correct, and the evidence crate and its eval harness are real. Core, render contract and persistence are far ahead of any consumer. My design was naive on block-size invariance for offline rendering, on content-addressing, and on the 200 Hz canonical track, which make a later live plugin possible.

### 1. Infrastructure outran the unproven sound thesis

*critical, rethink, reviewer confidence high; skeptic: **partly***

- **Claim.** Three weeks produced ~20k lines of types, contract, persistence and tests, and zero audible output. Test files outweigh source in core (persistence_props 2,971 lines, edit_props 1,968). The only existential risk (renders sound like a mouth) has no code path yet; Week 0 is still unrun.
- **Evidence.** find crates -name '*.rs' | wc: 20,857 lines; STATUS.md Next 1 and 6; MASTER-PLAN section 6 risk 1; git log shows no sound commit
- **Recommendation.** Freeze core and render-contract work. Next commits: sketch voices, a thin wav-to-events-to-audio path, a listening session. Resume infrastructure only for what that slice needs.
- **Skeptic.** Line counts check out (20,857 total; core tests 7,863 vs src 6,111; persistence_props 2,971), no sound commit in git log. But total includes the real evidence DSP and CLI, which the plan wants first, and Week 0 gate is already sequenced. Corrected: core/render-contract work outran sound; freeze is reasonable.
- **Corrected claim.** About 17k of the 20.9k lines are core and render-contract plus tests; evidence DSP and CLI are legitimate early work. No audio path exists, and the Week 0 listening step is still the founder's to do.

### 2. Core types serve consumers that do not exist

*major, overbuilt, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Edit, Layer, Interpretation, migrations, the musical/canonical MIDI projection, HostAnchor, seed-tree locks and strict-JSON machinery are specified and heavily tested with no UI, project load path, or midi crate. A v0.1 that records, rerolls, plays and exports needs a take, evidence, hit list, seed and settings.
- **Evidence.** crates/core/src/{edit,layer,interp,migrate,musical,canonical}.rs; docs/design/core-types.md sections 2.9-2.14; STATUS.md Deferred; DECISIONS 2026-10-21 JSON f32 entries
- **Recommendation.** Defer: leave built modules untouched, add no more tests or reviews to them. Let the first backend consume a plain event list plus curves; promote to ControlTrack only when the slice proves the shape.
- **Skeptic.** STATUS Deferred says the MIDI projection waits for the midi crate; midi/drums/app are 1-5 line stubs. edit.rs 509, migrate 177, layer 196, interp 217, musical/canonical 566 lines exist and are heavily tested. Defer recommendation matches built state; HostAnchor exists in take.rs.

### 3. Round-trip rule is expensive and mostly unneeded for drums

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** MPE .mid to ControlTrack import, quantise lattice and canonical forms drive much of core's complexity. Drum hits need plain notes with velocity; only hums need MPE bends. Round-trip is a nice property, not a product need, yet it constrains every type.
- **Evidence.** CLAUDE.md Hard rules 'round-trip'; MASTER-PLAN section 3.2; core-types sections 2.9 and 3.7; research/week0/precheck.py writes both MIDI kinds in a short script
- **Recommendation.** Downgrade to export-only with one invariant: MIDI note times and velocities equal the hit list the audio used. Defer MIDI import until someone edits a .mid and wants to hear it.
- **Skeptic.** MASTER-PLAN section 3.2 does state MPE .mid to ControlTrack round-trip, and CLAUDE.md makes it a hard rule. But the projection is explicitly deferred and only about 566 lines of types exist; 'drives much of core's complexity' is overstated. Export-only downgrade is a fair product call.
- **Corrected claim.** Round-trip is a hard rule that so far costs musical.rs/canonical.rs and MidiBinding persistence; the projection itself is unbuilt. Downgrading to export-only with a hit-list-equality invariant is a sensible scope option.

### 4. Sound engine plan is a platform, not a palette

*major, overbuilt, reviewer confidence medium; skeptic: **partly***

- **Claim.** v0.1 plans Plaits, VoiceMono, HarmonicNoise, NoiseTexture, VoiceMaterial, mod matrix, conditioning layer, reorderable mangle chain and four palettes. The target sound (SOPHIE metals, pitch-swept kicks, distorted noise) comes from ~6 voices plus drive/fold/crush. VoiceMaterial (the take itself) is the likeliest to sound like a mouth.
- **Evidence.** MASTER-PLAN section 4.5 and section 5 weeks 3-7; research/week0 holds only precheck.py; founder decision 2026-11-10
- **Recommendation.** Cut v0.1 to 6-8 hand-designed voices with 4-6 macros each and one fixed mangle order. Decide Plaits-vs-own after the founder rates the Python sketch. Defer VoiceMono, HarmonicNoise, mod matrix, VoiceMaterial.
- **Skeptic.** MASTER-PLAN week 3-4 and crates list do include conditioning, mod matrix, mangle chain, Plaits, VoiceMono, HarmonicNoise, NoiseTexture, VoiceMaterial. But render has no engine code yet, and the Week-4 go/no-go gate and Week 0 sketch already exist to prune. The 'six voices suffices' claim is unevidenced opinion.
- **Corrected claim.** The planned v0.1 engine is broad relative to the palette, but nothing is built and the plan has a go/no-go gate; trimming scope before Week 4 is a judgment call, not a verified defect.

### 5. Role classification sits well below its own gate

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** Gate E1 requires 85% four-class accuracy at 10 shots; measured kNN agreement is ~73-74% on AVP improvisations, and AVP is not this user. Forcing hits into kick/snare/hat imports a GM taxonomy the palette may not need; brightness, decay and loudness can drive timbre continuously.
- **Evidence.** MASTER-PLAN section 5 Gate E1; DECISIONS 2026-11-09 Week 0 entry (74.4%, ~73% from 10 per role); precheck.py docstring (54% brightness only, 41% baseline)
- **Recommendation.** Keep roles soft and coarse (two or three low/mid/high classes) and let continuous features choose body parameters. Judge by listening to misclassified fills, not AVP accuracy. Revise the 85% gate.
- **Skeptic.** Gate E1 85% at 10 shots and ~73-74% kNN on AVP are in MASTER-PLAN/DECISIONS. But the plan says the 73% is a brightness/tilt/decay kNN and the MFCC-based drums head, where 85% must be earned, is unbuilt. So the gap is not yet a measured failure of the gate.
- **Corrected claim.** A simple feature kNN scores about 73-74% on AVP versus the 85% gate; the intended MFCC head is unbuilt, so the gate is untested, though it is at risk.

### 6. Noise floor should be one evidence primitive, not three patches

*major, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** The founder's take shows three failures with one cause, no per-take noise floor: quiet false onsets, 90 noise onsets, and a 59 Hz fan called voiced. All come from absolute thresholds, yet STATUS plans separate fixes per curve.
- **Evidence.** STATUS.md Numbers 'Founder's first take' and Next 3, 5; DECISIONS 2026-11-09 eagerness entry
- **Recommendation.** Add one parameter-free observation: per-frame level above a take floor (low percentile or room-tone prefix). Feed onset pick, voicing and a steady-tone flag from it. Add a 2 s room-tone capture to the recording protocol.
- **Skeptic.** STATUS shows the fan voicing (-71 dB median) and quiet onsets (13 peak under 20 dB above floor) are floor-relative. But the 90 noise onsets peak at 28 dB above floor vs 32 for hits, so a floor does not catch them; STATUS says they need rise and strength. Not one cause.
- **Corrected claim.** A per-take floor would fix quiet false onsets and the 59 Hz voicing, but the loud noise-passage onsets need rise/strength, so there are two causes, not one.

### 7. No fast ear-loop tooling for sound

*minor, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** The founder judges by ear, but evaluation ships analysis stems (click, pitch sine). There is no one-command render-and-compare loop (take in, several renders out, blind-shuffled, Reaper-ready). Without it the Week-4 gate is costly and the AI iterates blind.
- **Evidence.** research/eval/listen.py; MASTER-PLAN section 5 Week-4 gate; STATUS Next 1
- **Recommendation.** Make the Python sketch emit a folder of shuffled unlabeled WAVs plus a key file, and reuse it for every later backend. Add automatic sanity checks (clipping, DC, silence) before the founder listens.
- **Skeptic.** research/eval/listen.py indeed emits only click and pitch-sine stems for analysis, and no render-compare tooling exists. But no sound backend exists yet, so tooling for blind shuffled renders is premature; the Week-4 gate is already defined as blind. Worth adding with the first backend.
- **Corrected claim.** Listening tooling covers analysis stems only; a shuffled blind render folder is a sensible addition once a backend exists.

### 8. Offline-only first may miss that hearing the sound changes the performance

*idea, opportunity, reviewer confidence low; skeptic: **confirmed***

- **Claim.** The plan defers monitoring to v0.2 and records dry. A beatboxer who hears the result adapts their hits; takes made deaf may not exercise the mapping as real use will.
- **Evidence.** MASTER-PLAN section 2 'Monitoring: Dry voice only in v0.1', section 5 v0.2
- **Recommendation.** Keep offline-first, but add a protocol step: perform a second take while the first render plays in headphones, and compare which take sounds better.
- **Skeptic.** MASTER-PLAN section 2 monitoring row says dry voice only in v0.1 and itself acknowledges that hearing the instrument changes performance. The suggested second-take protocol step is cheap and doesn't contradict the plan. Low-confidence idea, accurately described.

### Not anticipated

- Reaper already splits items at transients and Ableton and Logic ship audio-to-drum-MIDI. The only moat is sound and mapping quality, which argues for spending the next month on sound.
- Latency alignment matters in the DAW: the dry take and the render must be sample-aligned, including any pre-roll the render adds, or layering voice and synth gives audible flams.
- Mic proximity changes low-band energy heavily, so brightness and role features may need calibration per mic position, not just per room.
- Deterministic cross-platform renders (Windows vs macOS float differences) can make golden-file tests flaky; decide a tolerance early since CI runs both.
- An AI writing both the sounds and the checks has no ear; define objective sanity metrics (peak, DC, spectral balance, silence ratio) so obviously broken renders never reach the founder.

### Keep

- The evidence/lens split with stored, parameter-free Evidence lets sliders change lens work without re-analysis.
- Offline-first with a take analysed whole (lookahead for onsets, take-relative normalisation) is the right v0.1 scope and the real differentiator over live resynth tools.
- The research/eval harness (mir_eval on Vocadito and AVP, listening stems from the founder's own take) and the blind Week-4 go/no-go gate are the right tests of the thesis.
- Mangle as per-voice synthesis, with reverb, delay, compression and OTT left to the DAW, keeps scope small and fits the palette.

### Missed by the reviewer, added by the skeptic

- **The unattended-session protocol drove the infrastructure drift** (major). Core and contract were built mostly in unattended sessions with test-then-commit-then-push steps, so the process rewarded plumbing the AI could verify by tests over sound work only the founder can judge. Gate the next unattended sessions on a listenable deliverable. Evidence: MEMORY.md unattended-session-protocol; STATUS.md 'after the first unattended session (steps 1 to 6 of the kickoff list)'; STATUS Next 1 needs founder recordings Recommendation: Make the next unattended session build the lens-free sketch voices and a render of precheck.py's hit list to WAV, so the founder has something to hear when they return.
- **Founder-side Week 0 recordings block everything, with no fallback** (minor). The pre-check and golden set both depend on founder recordings that do not exist yet; the only recorded material is one 179 s take. The plan has no fallback for proceeding on that take or AVP while waiting. Evidence: STATUS.md Next 1-2 and 'Founder's first take'; MASTER-PLAN section 5 Week 0 text Recommendation: Run the sound sketch on the existing first take and AVP fills now; treat new recordings as refinement.
- **Seven-crate workspace with three empty stubs adds CI cost before use** (idea). midi, drums and app are 1-5 line stubs yet are compiled, linted and CI-tested on two OSes. The seven-crate split is premature structure for a two-person project. Evidence: crates/midi/src/lib.rs 1 line, crates/drums/src/lib.rs 2 lines, crates/app/src/main.rs 5 lines; commit 429a25a and dd11299 Recommendation: Keep the stubs (they are cheap) but do not add cross-crate abstraction for them until the first backend exists.

## Architecture against what v0.1 needs

About 20k lines exist, but the unproven part is the sound: palette, lens and first voice. Core is 6.1k source lines carrying 8.5k test lines; the render contract with its testkit is about 2.8k lines. Most of that guards things v0.1 never exercises: cross-version key stability, a 7-op edit language with no UI, region rolls, configurable MPE profiles, and real-time block splitting. The three likeliest rewrites are the fixed HitEvent/BoundKind::Hit schema, the hard-coded Sliders/LensSettings/PaletteMeta triple, and the Edit enum. Recommendation: stop adding core tests and build one vertical slice of kick, snare, hat and hum voices through a bare lens to WAV, then let it reshape the types.

### 1. Palette, voice and lens are unspecified; everything built is a seam

*critical, missing, reviewer confidence high; skeptic: **confirmed***

- **Claim.** The only palette type is PaletteMeta{name}. No type exists for a voice, its route, body preset, mangle chain, conditioning or the lens itself (lens_version is a number with no lens). 20k lines specify the interfaces of the three things the founder asked the AI to design, and none of their contents.
- **Evidence.** crates/core/src/project.rs:36-39 (PaletteMeta); lens.rs references palettes only by PaletteId; docs/STATUS.md 'Nothing makes sound'; MASTER-PLAN §4.1 'a palette is a set of voices with ranges'
- **Recommendation.** Define Palette as a TOML-loaded struct in render, with the id as the hash of the file. Write a bare lens that turns Hits into a ControlTrack, plus three hit voices and a cli render to WAV. Let that slice dictate core changes.
- **Skeptic.** project.rs has only PaletteMeta{name}; render/src has only contract/ and a 4-line lib.rs; no lens or palette types. Plan §5 weeks 3-4 schedule palettes and render, so absence is expected sequencing, but the observation holds.

### 2. HitEvent's fixed 8-field MIDI-shaped schema will not survive sound design

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** A hit carries only role, vel, bright, breath, mouth (all 7-bit) and dur. There is no pitch, tune, sweep, decay or click, which a SOPHIE-style kick or hat needs. Any added field touches canonical_key, serde, BoundKind::Hit, the toy backend, key tests and the MIDI mapping. Quantising timbre to 7 bits is an export concern applied to the lens/render path.
- **Evidence.** crates/core/src/control.rs:251-285 (HitEvent, canonical_key); crates/render/src/contract/bound.rs BoundKind::Hit; MASTER-PLAN §4.1 lists 'decay slope, class scores' that the type lacks
- **Recommendation.** Do the vertical slice first. Then make hit params a small set of named slots declared by the palette, held as f32 in the track and quantised only in the MIDI projection. Expect this type to change within weeks; do not write more property tests on it yet.
- **Skeptic.** control.rs:251-269 confirms the fields (at, anchor, dur, role, vel, bright, breath, mouth; U7 values). The field list is right, but 'will not survive' is speculation. Plan §4.1 and the Plaits macros may map from bright/breath/mouth. The risk of churn is fair; the 'rewritten within weeks' prediction is unproven.
- **Corrected claim.** HitEvent carries three 7-bit timbre fields plus vel and dur; if the palette needs more per-hit parameters the type, key and MIDI mapping all change. This is a plausible churn risk, not a certainty.

### 3. Edit language, region rolls and Imported source have no v0.1 consumer

*major, overbuilt, reviewer confidence high; skeptic: **partly***

- **Claim.** Seven edit ops with dormancy, rebasing and tolerance logic (509 lines) carry the repo's largest test file (1,968 lines). The v0.1 app list has no edit gesture. The notes lens that needs ForcePitch, SplitAt, JoinAt and MuteSpan is v0.2, and §4.3 says edit semantics are written after the app exists. RegionRoll (UI v0.3) and ControlSource::Imported, with its deferred anchor re-attachment, are in the same position.
- **Evidence.** crates/core/src/edit.rs:44-117; crates/core/tests/edit_props.rs (1,968 lines); crates/core/src/seed.rs:178-195, 338-360; MASTER-PLAN §5 weeks 5-7 and v0.2; interp.rs ControlSource
- **Recommendation.** Park edits, regions and Imported behind a feature or delete them, keeping only DeleteHit and ForceRole for misclassified hits if the app needs them. Revisit when the app's hit-correction gesture exists. Saves about 3k lines, and the time-anchor principle survives in HitAnchor.
- **Skeptic.** edit.rs is 509 lines, edit_props.rs 1968, Imported exists in interp.rs and RegionRoll in seed.rs. The §5 v0.1 app list has no edit gesture, and the notes lens is v0.2. But DeleteHit/ForceRole may be needed, and risk #9 explicitly wants edit semantics tested. The '3k lines saved' figure is approximate.
- **Corrected claim.** The edit ops, region rolls and Imported are speculative for v0.1 (about 2.5k lines including tests); only DeleteHit and ForceRole plausibly have a near-term consumer.

### 4. Cross-version key and persistence stability protects users who do not exist

*major, overbuilt, reviewer confidence medium; skeptic: **partly***

- **Claim.** Cross-version guarantees cover canonical KeyWriter with frozen labels, reproducing-value migrations (all STEPS arrays empty), the .tkev container with bit-flip tests, and a custom JSON writer for two f32 midpoint values (±7.038531e-26). A render key already includes lens, backend and contract versions, so any sound change invalidates it. A cache miss costs under a second for a fill and about 2.5 s of analysis for 3 minutes.
- **Evidence.** crates/core/src/key.rs (450), tests/key_props.rs (665), persistence_props.rs (2,971), migrate.rs (177), project.rs ~330-450, layer.rs RenderInputs; STATUS.md analysis cost 8.3 s per 10 min
- **Recommendation.** Quantise persisted parameters to integers (per-mille, centi-dB), key by blake3 of canonical JSON plus a schema number, and refuse unknown or newer files as now. Drop migrations and the f32 writer until the first release. Keep TakeId and the atomic save. This removes a hard rule from CLAUDE.md and about 4k lines.
- **Skeptic.** migrate.rs STEPS is empty (line 29) and project.rs:331 documents the f32 writer for two midpoint values, as claimed. But these are CLAUDE.md hard rules the founder chose, so deleting them is a policy call. Also, saved projects and take caches need to be readable across builds. The sizes cited are roughly right.
- **Corrected claim.** The migration, writer and key machinery have no real users yet and cost about 4k lines with tests, but they implement an explicit founder rule; the recommendation is to renegotiate it, not an oversight.

### 5. Render contract is real-time machinery ahead of an offline v0.1

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** The cell grid, latency pre-roll, split law, no-alloc tests and EventCursor serve the plugin and monitor (v0.2). v0.1 renders offline, and every slider or dice change invalidates all segments, so segment resume does not give sub-second re-render; speed comes from a roughly 50x-realtime backend and per-voice parallelism. The first real backend (Plaits in blocks of 24, rubato, an oversampled ladder) must buffer internally to satisfy the split law. Law 6 plus Silence writing +0.0 will click when fold or crush leaves DC, or a self-oscillating filter rings past tail_frames.
- **Evidence.** crates/render/src/contract/mod.rs:60-110 (laws), driver.rs, bound.rs segment_plan; testkit.rs (1,023 lines) + render/tests (1,780); MASTER-PLAN §5 DoD
- **Recommendation.** Keep it (founder hard rule, cheap to hold) but freeze the testkit. Build the Plaits voice next and treat contract friction as the signal. Add a DC-offset and ringing-filter case before the mangle chain lands. Parallelise voices, not segments.
- **Skeptic.** Contract, driver, bound and testkit (1023 lines) exist and CLAUDE.md makes block-size invariance a hard rule, as the reviewer concedes. The Silence DC/click concern is plausible: driver.rs has DriverEvent::Silence, but I did not verify the sample value written. The 'every change invalidates all segments' claim is unchecked.
- **Corrected claim.** The contract is built for the v0.2 plugin ahead of need but is mandated by a hard rule. The DC-click risk of Silence after a nonzero tail is plausible and untested; the segment-invalidation claim is not shown.

### 6. Six hard-coded sliders in core are the wrong abstraction for a palette-driven product

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** Sliders is a frozen struct of six named fields, with migration tests (add_glide, swing_to_timing_feel) built around it. For a drum palette Register and Glide/Retrigger are inert, and Mangle and Mouth mean different things per palette. Palettes will want their own macros, so this struct, its keys and the Imported key-subset logic will be rewritten.
- **Evidence.** crates/core/src/lens.rs:16-90; interp.rs ImportedLens; persistence_props.rs migration tests ~1787-2060
- **Recommendation.** Have the palette declare its macros as a list of (name, range, default), with values stored as a name-to-integer map in LensSettings. Keep Mangle and Mouth as conventional names.
- **Skeptic.** Sliders at lens.rs:37-50 has six fixed fields, matching MASTER-PLAN §5 exactly (mouth, fidelity, mangle, timing, register, glide/retrigger). The plan fixes these as the v0.1 UI, so this is a deliberate design. Per-palette macros are a reasonable v0.2 direction, but the 'inert for drums' point is unproven.
- **Corrected claim.** The six sliders are the plan's specified v0.1 controls; making them palette-declared is a plausible later change, and Register and Glide may be inert for drum palettes.

### 7. MPE profile is configurable but there is no plain-MIDI export for Reaper

*minor, missing, reviewer confidence low; skeptic: **partly***

- **Claim.** MpeProfile validates channels, CC maps, reserved CCs and bend range (about 12 error variants) with no UI to change any of it. The default is a ±48 bend over rotating channels 2-16, which on a non-MPE synth or drum rack gives wrong pitch or ignored channels. The DoD says 'drum rack or Serum', and Reaper does not route MPE itself. Plain SMF is mentioned only as a crate bullet.
- **Evidence.** crates/core/src/canonical.rs:56-70, 142-190, MpeProfile::default; MASTER-PLAN §5 DoD and §4.3 midi crate
- **Recommendation.** Reduce MpeProfile to constants. Add an export flavour 'plain': hits on GM ch10, hum on one channel with ±2 or ±12 bend. Test the real targets (Serum version, Surge XT, ReaSamplOmatic) before fixing the layout.
- **Skeptic.** canonical.rs:56-68 shows MpeProfile with configurable channels, CCs, roles and bend; hits go on channels 4-15 and the phrase lane on 1-3, so the doc comment contradicts the reviewer's 'rotating channels 2-16'. Plain SMF export is not planned. The DoD says drum rack or Serum, so an MPE-ignoring target is a fair concern.
- **Corrected claim.** MpeProfile is highly configurable with no UI; the default uses hit channels 4-15 and phrase channels 1-3 (zero-based). Whether a drum rack reads these correctly is untested, and no plain GM export exists.

### 8. Day-one glue has no home: orchestration, mixing, playback, jobs

*major, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Nothing owns interp, lens, per-voice bind, driver, mix, WAV and cache. There is no per-role stem export, and 'play' needs an output stream while §4.4 says cpal is input only. The app needs a cancellable worker for analysis and render. Core's Layer (gain, mute, per-rate renders) sits oddly against CLAUDE.md's 'no looper/layers' and is really a render-cache slot.
- **Evidence.** MASTER-PLAN §4.4 vs §5 'play it'; crates/render/src/lib.rs (4 lines); crates/core/src/layer.rs; render_offline is per voice only; crates/app is 5 lines
- **Recommendation.** Add a `session` module in render: Interpretation plus Evidence plus Palette to per-voice stems plus mix, with a cancel token. Add output to the cpal plan. Rename Layer to RenderSlot, or drop gain and mute. Export stems per role, since Reaper wants them.
- **Skeptic.** render/src/lib.rs is 4 lines, crates/app is a stub, and MASTER-PLAN line 112 and 121 say cpal is input only while §5 requires 'play it'. layer.rs has gain_db and mute fields; the Layer naming does sit awkwardly with the no-layers rule. Orchestration and mixing are unassigned.

### Not anticipated

- Dice and locks are per aspect (Body, Mangle...), not per voice or role. The first real complaint will be 'the kick is perfect, reroll only the snare'. Seeds already derive per role, so a (role, aspect) lock is cheap to add now and expensive after projects exist.
- The ratio of 8.5k test lines to 6.1k core source lines makes every type change cascade through thousands of generated test lines. The founder reviews by ear and cannot realistically review them, so the review budget should move to listening, and the test volume should shrink.
- Golden-file audio tests on both macOS and Windows CI will fail on libm differences in sin, exp and tanh. render keys record platform class as provenance, which admits non-bit-exact output. Goldens need a tolerance such as spectral or per-sample epsilon, not byte equality.
- MIN_GAP_FRAMES of 10 (50 ms) with a hard reset after tail_frames cuts any mangle that sustains, such as feedback, a screaming filter or a long reverse tail. Sound designers will hear this as a gate and not know why.
- The take is captured at the device rate, but output_rate, the render slots per rate and the cpal output device are three separate rates. 44.1 vs 48 kHz mismatches between record, render and playback will arrive on day one on Windows.

### Keep

- Hits and spans carry an anchor (identity, seeds, slices) separate from their firing time `at`, which lets timing feel and perceptual-centre pre-roll move audio without re-rolling it.
- Strict loading (unknown fields, repeated keys and newer schemas refused) and the atomic temp-file-and-rename save are cheap and correct.
- The renderer takes only a ControlTrack, and hit-only voices read no curves, which keeps MIDI round-trip and the lens boundary honest.
- Seed derivation by anchor plus per-role ordinal group makes a moved hit a shifted copy of itself; keep it, with locks and counter-based noise.

### Missed by the reviewer, added by the skeptic

- **Bounded verification of the plan's performance claims is absent from the contract** (minor). The DoD requires sub-second re-render and a 10-minute take under 600 MB RSS. Nothing in core or the contract measures or budgets this: no benchmark, no memory accounting in the driver, and RenderInputs/segment keys do not carry a cost model. Evidence: MASTER-PLAN §5 weeks 5-7 and the DoD; crates/render/src has only contract/ with no bench or budget tests. Recommendation: Add a benchmark rendering a 10-minute ControlTrack with a toy backend to establish a baseline before real DSP lands.
- **Test weight is concentrated where the design is least proven** (idea). Property-test volume (persistence 2971, edit 1968, key 665 lines) dwarfs the code it protects and far exceeds that for hits/palette/lens behaviour, so refactors of HitEvent or Sliders will force large test rewrites. Evidence: wc -l: persistence_props.rs 2971, edit_props.rs 1968, key_props.rs 665, versus the largest src file control.rs at 848. Recommendation: Hold new property tests on fast-changing types and prefer a few golden fixtures until the first backend settles.

## Evidence DSP

The evidence DSP is competent and fast (about 70 us per frame). Its main design claim does not hold in three places. Pitch evidence stores post-prior probabilities, not observations. Brightness in harmonic units bakes in an undecoded f0 candidate. The onset function has a hidden absolute-level constant. Voicing in noise, the room tone and the onset false positives come from one cause: nothing in evidence models the noise floor. The 25 ms live monitor would need different windows from the centred ones used offline, so decide that before the Hit and lens work. I read the code only and could not run it, since the rules forbid cargo.

### 1. Stored voiced_p is a saturated function of a clean-speech prior, not an observation

*critical, contradiction, reviewer confidence medium; skeptic: **partly***

- **Claim.** voiced_p and cand_p are the CMND trough depth pushed through Beta(2,18) and Boltzmann priors, which are fixed constants in evidence. P(threshold > d) is 0.42 at d=0.10, 0.08 at 0.20 and 0.01 at 0.30. At 10 dB SNR dips sit near 0.1 to 0.35, so voiced_p collapses to near 0. That is the VR 21.7 result. A per-take threshold on a saturated number recovers only half. Fix: store raw trough depths.
- **Evidence.** crates/evidence/src/pitch.rs:25-32, 211-235. Beta tail computed from the closed form in the tests (pitch.rs:287-290). docs/STATUS.md 'Numbers', noise rows.
- **Recommendation.** Store the top-K trough (hz, CMND depth) and the global minimum depth per frame. Move the Beta and Boltzmann mapping into decode, so the lens can swap it for a likelihood ratio against the take's own noise-only depth distribution.
- **Skeptic.** pitch.rs:25-32,211-235 confirm voiced_p is the sum of trough probabilities from a fixed Beta(2,18) tail; the 0.42/0.08/0.01 numbers are right. But this is standard pYIN, not a contradiction. The 10 dB depth range and 'recovers only half' are not checked. 'Critical' is overstated.
- **Corrected claim.** voiced_p and cand_p are a fixed Beta(2,18)/Boltzmann transform of CMND trough depth, so noise-degraded dips (above ~0.2) are compressed toward 0 and the lens cannot re-threshold them. Storing raw depths would restore that freedom. Severity is major, not critical.

### 2. A periodic hum is called voiced because nothing models the noise floor

*major, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** YIN is level-blind above -100 dBFS and any stationary periodic tone dips CMND to near 0. The 59 Hz fan therefore looks like a perfect hum. A range prior (fmin 55 to 70 Hz) would be a patch: 100 or 120 Hz mains hum sits inside the voice range. The root fix is a stationary-noise model by minimum statistics. Voicing then becomes level above the floor plus a depth likelihood ratio.
- **Evidence.** pitch.rs:152 (silence gate -100 dB mean square); decode.rs:244-252 (voicing reads voiced_p only, never loudness); docs/STATUS.md 'Voicing on a room tone', 1,453 frames at -71 dB.
- **Recommendation.** Add columns for a minimum-statistics noise floor (dB) and for level above that floor. Add a tone-stationarity cue: f0 variance over about 0.5 s, since a fan holds cents-level steadiness and a human hum does not. Feed both into the voicing state of decode. Causal in live use.
- **Skeptic.** pitch.rs silence gate is mean square below 1e-10 (-100 dB). decode_voicing (decode.rs:244-252) uses only grid.voiced_p, never loudness. STATUS.md records 1,453 voiced frames at -71 dB. The fan-steadiness cue is speculation, but the missing floor model is real.

### 3. BrightnessHarm freezes an undecoded f0 candidate into evidence

*major, contradiction, reviewer confidence high; skeptic: **confirmed***

- **Claim.** centroid_harm divides by cand_hz[0], the top raw candidate. An octave error, or noise taken as a pitch, scales it by 2 or by a random factor. The decode that fixes f0 is a lens call, so the lens cannot repair a stored curve that depended on it. This breaks 'evidence is parameter-free, decode is pure'.
- **Evidence.** crates/evidence/src/lib.rs:78-79; timbre.rs:189-193; store.rs column BrightnessHarm; docs/design/core-types.md evidence rule in CLAUDE.md.
- **Recommendation.** Store only the linear centroid in Hz. Compute harmonic units in the lens from the decoded f0 and the voicing mask, which costs one division per frame.
- **Skeptic.** lib.rs passes pf.cand_hz[0] into timbre.analyze, and timbre.rs:189-193 divides centroid by it. store.rs:90 stores it as Column::BrightnessHarm. The top raw candidate is not the decoded f0, so the column depends on an undecoded choice.

### 4. The onset function has a hidden absolute-level constant, and strength has no noise normalisation

*major, bug, reviewer confidence medium; skeptic: **partly***

- **Claim.** log10(1 + band) in the SuperFlux code is applied to unnormalised FFT magnitudes. The '1' is a gate that moves with input gain and with FFT length, so it differs between 44.1 and 48 kHz. Above that gate log flux is scale-invariant, so noise gives the same ODF statistics at any level. Strength is the ODF minus a trailing mean, not divided by the ODF's own spread. Noise therefore passes a fixed 1.1 threshold.
- **Evidence.** onset.rs:123 (log10(1+band)), 145-171 (strength = v - avg), 40 (threshold 1.1); docs/STATUS.md onset findings.
- **Recommendation.** Normalise magnitudes to FFT length and window gain. Fix the constant relative to the estimated noise floor, not to full scale. In the drums crate, decide on z = (v - median)/MAD of the stored Odf over about 1 to 2 s, plus a level floor from the noise-floor column.
- **Skeptic.** onset.rs:123 uses log10(1+band) on unnormalised Hann-windowed FFT magnitudes (frame.rs:101-115 applies no length or window scaling). The filters are unit-sum, so the gate sits near -55 dBFS for a tone. Strength is v - trailing mean (onset.rs:166-171). However, 1.1 is only the cli default; drums owns the real decision.
- **Corrected claim.** The +1 in the log is an absolute-level gate (about -55 dBFS, varying with FFT length) that depends on gain and sample rate. Strength has no spread normalisation. The fixed 1.1 threshold is only the cli's default decision, not the drums decision.

### 5. The offline windows are centred, so live monitoring would be a different analyser

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** YIN integrates 25 ms plus tau_max lag, which is 50 ms at 40 Hz, centred on the frame. The spectrum is a 46 ms Hann centred on the frame. Backtracking looks 23 ms ahead, and the peak filter looks 20 ms ahead. Live use would need about 40 to 60 ms of delay, not the 20 to 25 ms that research 08 section 6 measured. Viterbi is whole-take, so live would use fixed-lag decode. Offline and live evidence would differ.
- **Evidence.** pitch.rs:21,103-106,140-146; frame.rs:102-103; onset.rs:245-247, 22-23; decode.rs:189-239; docs/research/08 section 6.
- **Recommendation.** Define evidence as trailing-window with a declared fixed delay per column (a stored offset). Live then equals offline minus lookahead. Use a separate 128-sample, short-window HF-energy onset path for the monitor, and keep SuperFlux for offline refinement.
- **Skeptic.** The core point holds: the 46 ms Hann spectrum is centred (frame.rs:102), backtracking and the peak filter look ahead (onset.rs POST_MAX 20 ms), and Viterbi is whole-take. But the YIN window is not centred. pitch.rs:140-146 starts at centre - w/2 and reaches tau_max further, so lookahead is about 37 ms.
- **Corrected claim.** Offline evidence uses forward-looking windows: YIN reaches about 37 ms past the frame centre, the spectrum is a 46 ms centred Hann, and onset peak-picking and backtracking use about 20 ms lookahead. Live use would need a delay of 40 to 60 ms, or a different analyser.

### 6. Octave-down errors are a decision problem the stored evidence cannot settle

*minor, opportunity, reviewer confidence low; skeptic: **partly***

- **Claim.** CMND depth cannot separate f from f/2 on vocal fry or amplitude-modulated frames, because a sub-multiple trough is equally deep. The Boltzmann prior and the 1e-3 octave jump penalty only break ties. The long spectrum and the CPP cepstrum are already computed per frame and give an independent check. At f/2, odd multiples of f/2 are empty if f is the real period, and the cepstrum shows a peak at 2tau.
- **Evidence.** pitch.rs:94-97 and 229-232 (Boltzmann); decode.rs:49,164-186; timbre.rs:47-64; docs/STATUS.md vocadito_34.
- **Recommendation.** Add one column, the ratio of spectral energy at odd multiples of candidate f/2 to the energy at multiples of f. The decoder uses it as a candidate likelihood term, with no extra FFT. Do this after the raw depths are stored.
- **Skeptic.** The Boltzmann prior (pitch.rs:94-97) and the octave jump penalty (decode.rs:49) are as described, and sub-multiple troughs are plausibly equally deep. The reviewer rated it low confidence. The odd-multiple energy ratio and the claim that it needs no extra FFT are untested, and the long spectrum exists per frame.
- **Corrected claim.** CMND depth can leave f and f/2 nearly tied, so only priors choose between them. A spectral harmonic-ratio cue from the existing long spectrum is a plausible extra candidate term. It is untested and should be judged on the vocadito_34 failure.

### 7. One A-weighted scalar is the wrong loudness for drums, and the 200 Hz grid is too coarse for hit features

*minor, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** A-weighting is -19 dB at 100 Hz, so a 50 to 100 Hz beatbox kick reads 15 to 20 dB quieter than a hat. The same weighting is what makes the 59 Hz fan quiet. The 23 ms window and 5 ms hop also smear a 1 to 3 ms attack. Hit velocity and brightness should come from audio at the refined onset, not from nearby grid frames.
- **Evidence.** timbre.rs:35-44, 142-159, 20; DECISIONS item 8 (each hit carries velocity, brightness, breath); onset.rs hit position refined to sample.
- **Recommendation.** Keep the 200 Hz rate for continuous controls. Add low, mid and high band energies (not A-weighted) to evidence for drum velocity. Compute per-hit features in a 2 to 30 ms window anchored on the onset sample. Re-test the 60 ms refractory against 20 Hz rolls.
- **Skeptic.** timbre.rs:142-159 applies A-weighting; the roughly -19 dB at 100 Hz figure is standard. Whether a kick reads 15 to 20 dB below a hat depends on spectra and was not measured. I did not verify that drums takes velocity from grid frames rather than the refined onset sample.
- **Corrected claim.** A-weighted loudness strongly attenuates sub-100 Hz content, so kick and fan energy are under-represented. Per-hit features anchored on the refined onset sample would avoid grid smearing. The size of the effect and the current drums behaviour are unverified.

### 8. Whistles above 1,100 Hz are cut by two hard constants, not by one source model

*minor, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** The decode range is 55 to 1,100 Hz and the analysis range is 40 to 2,000 Hz. A whistle is a near-sinusoid at roughly 700 to 3,500 Hz. Raising the decode fmax alone would invite octave-up errors on voice. Range belongs to the instrument being performed.
- **Evidence.** decode.rs:45-46; pitch.rs:17-19, 103-104; docs/STATUS.md 'Whistle' and Next item 7.
- **Recommendation.** Raise ANALYSIS_FMAX to about 4 kHz (cost is trivial). Make the decode range a per-performer or per-source setting from range calibration, with a whistle mode (700 to 3,500 Hz).
- **Skeptic.** decode.rs:45-46 default fmin 55 / fmax 1100; pitch.rs:17-19 analysis range 40 to 2000 Hz. STATUS.md lists whistle voicing above 1,100 Hz as an open item. Raising decode fmax alone invites octave-up errors, which is a sound inference.

### Not anticipated

- Because evidence is level-blind, a quiet ADC-noise or mains-hum frame gets pitch, brightness and CPP values exactly like a performance frame. The lens needs a performer-present mask (level above noise floor) as a first-class column, which every other mask depends on.
- Whole-take Viterbi uses a back-pointer table of t_len x n_bins x 2 bytes: 62 MB for 10 minutes, but about 370 MB for an hour-long session. Evidence and decode need checkpoints or a fixed-lag decode, which also serves live.
- Tilt regresses band dB equally weighted over 100 Hz to 8 kHz, so a flat mic or room noise floor gives tilt near 0 on any quiet frame. A quiet breath and a quiet hum read alike unless the lens masks by level.
- Mics with a high-pass (laptop built-in, headsets) remove the fundamental of low hums, so YIN can lock onto the second harmonic. Octave-up errors are then likely on real hardware, which Vocadito's studio recordings do not exercise.
- The noise passage may not be a false-positive problem. Mouth noise that bursts at 25 dB rise is an onset by every signal measure. Whether it is a drum or a texture is a class decision, and the founder needs a per-event decay or burstiness feature for it, not a tighter threshold.

### Keep

- The 200 Hz fixed control grid with sample-accurate onset refinement and exact integer frame centres.
- Storing candidates and the ODF so decode and onset picking stay re-runnable without audio.
- Separate pitch and voicing Viterbi, with a single un-normalised kernel normaliser so the range edges are not attractors.
- The FFT-based YIN with f64 accumulation, at about 70 microseconds per frame, leaves ample headroom for the live monitor.

### Missed by the reviewer, added by the skeptic

- **Onset gate sits near room-noise level and the filterbank is unit-sum** (major). Flux filters are normalised to unit sum, so each band is a weighted mean of unnormalised magnitudes. The +1 in log10 corresponds to a sine of about 0.002 amplitude (roughly -54 dBFS). A quiet room, or a laptop mic at low gain, straddles that gate. Flux there is either compressed or amplified, which fits the observed onset false positives in noise and near-silence. Evidence: onset.rs:84-96 (weights /= sum), 123 (log10(1+band)); frame.rs:101-115 (no 1/L or window-gain scaling); timbre.rs:22 (46.4 ms Hann, L about 2227). Recommendation: Scale magnitudes by 2/sum(window) so the unit is amplitude, and set the log offset from the take's measured noise floor. Alternatively use a relative floor, so the onset function does not depend on the input gain.
- **Boltzmann prior structurally biases candidates toward octave-up** (minor). Boltzmann(2.0) weights the first trough below each threshold by about 0.86 and later ones exponentially less. Candidates are ordered by lag, so the shortest lag (highest frequency) wins ties. For a periodic signal, the 2-tau trough is similar in depth, so this prior pushes toward octave-up for any frame with sub-multiple troughs. The decode octave penalty (1e-3) then has to fight it. Evidence: pitch.rs:94-97 boltzmann_pmf and the per-threshold loop at 211-235; tau ordering of self.troughs from tau_min. Recommendation: Check whether the fix for octave errors should drop or lessen the prior, and evaluate it separately from the depth storage change, because it influences which direction the errors go.
- **Filterbank collapses at low bands because the 46 ms window has coarse bin spacing** (minor). With 24 bands per octave from 30 Hz and bin spacing of about 21.5 Hz (46.4 ms window), the lowest bands round to the same bin and are deduplicated. The sub-100 Hz region, where kicks live, has only a few filters, so the flux there is coarse. This is a second reason why a kick onset can be missed or smeared besides A-weighting. Evidence: onset.rs:60-85 (bins dedup via bins.last() != Some(&b)); timbre.rs:22 SPECTRAL_SECONDS = 0.0464. Recommendation: Count how many filters fall below 100 Hz at 44.1 and 48 kHz and test kick onsets separately. A shorter window for the low band, or a separate band-energy onset path, may be needed.

## Sound engine and aesthetic

The engine plan is sound as architecture but unproven as sound. Nothing yet shows the planned voices can make a sound that is not a generic drum machine, and the plan has no stage for the techniques these artists rely on most: resampling, feedback and bus clipping. Plaits is a prototyping shortcut, not the core; four purpose-built voices matter more. The largest week-4 risk is that the performance is inaudible in the result, so a render sounds like the same few drums however the mouth varies. Run listening experiments on the Python sketch before building the Rust engine. Outside research was thin: search returned no technical interviews, so artist-technique claims are low confidence.

### 1. No stage makes the sounds 'designed'; the plan only varies a drum voice

*critical, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** Mangle and per-hit variation make drums vary, but SOPHIE, Death Grips and JPEGMAFIA drums are mostly selected, resampled, clipped and layered sounds. The plan keeps one voice per hit, no resampling of synth output, no feedback paths, no bus clip. Likely outcome: clean drum machine plus distortion.
- **Evidence.** MASTER-PLAN §4.5 (Later list, Honest limits); research/06 §8; CLAUDE.md 'No global effects bus'. External: no technical interviews found by search (low confidence).
- **Recommendation.** In the Python sketch, listen to a voice with and without three things: bounce-and-resample (pitch, reverse), a feedback comb or delay inside the voice, and a hard clipper summing the layers. Keep whichever the founder hears as the difference.
- **Skeptic.** MASTER-PLAN §4.5 and the §2 table show no resampling or feedback of synth output and no bus clip, so clean-drum-machine risk is real. But 'one voice per hit' is overstated: VoiceMaterial (pitch, crush, fold, stutter, reverse) is a parallel layer under bodies, and layers exist.
- **Corrected claim.** Synth voices get no resample, feedback or bus clip, but take-derived VoiceMaterial supplies pitched, reversed, stuttered layers; the real gap is that synth output itself cannot be resampled or layered-clipped. The listening experiment is still sound.

### 2. Mouth amount: the performance may be inaudible in the render

*critical, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** A 5 to 30 ms transient from a laptop mic in a fan-noise room is a click or pop. It will sound like a mouth at high settings and add nothing at low ones. Per-hit brightness and loudness are mic and room dependent, so percentile-normalised values map to small variations. The audible parts of a performance are rhythm, accent-to-distortion, role choice, noise-gated envelope, and sustained curves.
- **Evidence.** MASTER-PLAN §4.5 Hits, §5 week-4 pivot clause; STATUS.md first-take review; research/06 §5.
- **Recommendation.** Map loudness to drive and fold depth, not only gain. Map duration to decay, relative centroid to body pitch, breath to noise mix. Test with a shuffled-features control: same hit times, features permuted. If the founder cannot tell it apart, the mapping is inaudible.
- **Skeptic.** The plan (§4.5 Hits) already maps loudness to accent, brightness to tone and decay, duration to decay, and the §5 pivot clause names the mouth-like failure. The reviewer is right about transient fragility and that drive/fold mapping and a shuffled-feature control are missing; 'adds nothing at low settings' is speculation.
- **Corrected claim.** Hit mapping covers accent, tone, decay and exciter; it does not say loudness drives distortion depth. The shuffled-feature control is a good, absent test.

### 3. Plaits is a prototype shortcut; purpose-built voices matter more

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** Roughly six of 24 engines serve this palette (kick, snare, hat, FM, modal, noise or particle). Plaits drum voices carry fixed internal envelopes and a 3-macro interface, giving 808-style bodies that mangle cannot make unique. It needs 48 kHz and a resampler. Its internal block, about 24 samples, differs from the 32-sample mod rate (from memory, medium). A 150 to 300 line pitch-swept kick, FM metal, noise plus modal snare, comb or Karplus, and wavefolder gives direct per-sample access to pitch envelope into fold, FM or drive.
- **Evidence.** research/06 §1 and the Plaits fit section; MASTER-PLAN §4.3 (render voices list); §4.5 mod matrix at 32-sample rate.
- **Recommendation.** Build the four purpose-built voices first and compare them blind against Plaits kick, snare and FM. Keep Plaits for FM, modal and particle engines if it wins. Do not let its macros define the dice space.
- **Skeptic.** Facts check out: Plaits needs 48 kHz plus rubato (research/06, §4.3), and its block is 24 samples versus the 32-sample mod block (my recollection of Plaits). The 'six of 24' count and 'fixed envelopes' are opinion. The plan already builds VoiceMono and treats Plaits as one source.
- **Corrected claim.** Plaits is one body family among several, not the sole engine; its fixed-envelope macros and 24-sample block are real constraints. A blind comparison against a small own kick/snare/FM set is reasonable but not proven necessary.

### 4. Cross-synthesis is the most audible voice-to-sound link and is scheduled last

*major, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** A vocoder or talkbox-style imposition of the voice's spectral envelope, plus an envelope-follower on noise, is what makes the result sound like the performer without sounding like a mouth. Warps-style cross-modulation and Clouds-style granular on the voice are in the 'Later' list. Ring-mod against the voice is in mangle but needs the voice audio, which the ControlTrack renderer boundary hides.
- **Evidence.** MASTER-PLAN §4.5 Later list; CLAUDE.md 'renderer never sees notes'; research/06 §6 Timbre Follow.
- **Recommendation.** Prototype a voice-to-noise vocoder and a ring-mod of the voice into a synth body in the Python sketch. If it wins, move the voice-audio path (VoiceMaterial, ring-mod) into the week-3 contract design now.
- **Skeptic.** Warps-style vocoder and Clouds-style granular are indeed in the Later list (§4.5), and Timbre Follow is in research/06 only. But ring-mod/XOR against the voice is in the v0.1 mangle chain, and core-types gives backends a take-audio slice reader, so the 'boundary hides voice audio' claim is wrong. 'Most audible' is unsupported.
- **Corrected claim.** Vocoder/spectral-envelope imposition is deferred, but voice audio is reachable through the take-slice reader and ring-mod is already planned for v0.1.

### 5. Bus dynamics: judging isolated renders will understate the sound

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** Heavy clipped and limited masters define Death Grips and SOPHIE loudness. Per-voice mangle cannot make kick and bass intermodulate in a shared clipper. The gate is judged 'in a Reaper session with the founder's master chain', but blind fill renders played alone will sound thin.
- **Evidence.** MASTER-PLAN §6 risk 5, §3 principle 11, §5 gate; CLAUDE.md effects boundary.
- **Recommendation.** For the gate, run every render through the same fixed Reaper chain (clipper, OTT, limiter) and the same chain on a reference track. Consider a documented JSFX or chain preset rather than an engine bus.
- **Skeptic.** MASTER-PLAN risk 5 states the same concern and says gate renders are judged in a Reaper session with the founder's usual master chain. The remaining valid point is specifying that the same fixed chain be applied to renders and the reference track, and that kick and bass intermodulation cannot happen per voice.
- **Corrected claim.** The plan already judges gate renders through the master chain; it should additionally fix that chain and apply it to a reference track for comparison.

### 6. Rhythm transformation is absent from the lens; Aphex-style glitch is mostly pattern edits

*minor, opportunity, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Aphex-type drums come from ratchets, rolls, stutters, odd subdivisions and microedits of the pattern, not from the voice. The lens plan has timing feel and quantise only. A founder fill replayed one-to-one gives straight rhythms in a new timbre.
- **Evidence.** MASTER-PLAN §2 Randomness (dice aspects: body, mangle, timing feel, mouth, pitch) and §5 weeks 5-7 sliders; no ratchet or stutter operator in §4.1 Lens.
- **Recommendation.** Add a ratchet and roll operator to the lens as a seeded edit (per-hit subdivisions with velocity ramps) before the gate. It is cheap and may move the gate more than another body.
- **Skeptic.** The MASTER-PLAN §2 dice aspects and §5 sliders list timing feel, quantise and swing only. The only stutter is in VoiceMaterial slices, not hit-pattern ratchets or rolls. The Aphex-pattern-edit claim is plausible, and I did not search externally.

### 7. Gate design measures sounds alone and bundles several failures

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** 60 renders (10 fills, 2 palettes, 3 rolls) blind is a fatiguing test, and 'track-ready' is judged without a track. Failures from false-positive onsets, wrong roles, generic bodies and inaudible mapping would all read as 'no'.
- **Evidence.** MASTER-PLAN §5 week-4 gate; STATUS.md first-take onset false positives (90 noise onsets).
- **Recommendation.** Split the gate into controlled A/Bs: hand-cleaned onsets vs automatic, shuffled-feature control, with and without the master chain, and drop renders into a real project of the founder's. Record the reason for each rejection.
- **Skeptic.** The 10 fills x 2 palettes x 3 rolls = 60 renders arithmetic matches §5, and the STATUS.md first take has 90 noise-onset false positives. But the plan says renders are judged in a Reaper session, so 'judged without a track' is overstated. Bundling of failure causes is a fair point.
- **Corrected claim.** The gate is blind and bundles onset, role, body and mapping failures; it is judged in Reaper, but it has no controlled A/B isolating the causes and no recorded rejection reasons.

### 8. AI-designed palettes carry the whole 'Serum-ness' bet with no proxy metric

*minor, missing, reviewer confidence low; skeptic: **unverifiable***

- **Claim.** The plan says Serum-ness is preset craft. The founder's 2026-11-10 decision puts that craft on the AI. There is no objective check; sounds can only be judged by ear, which is slow.
- **Evidence.** MASTER-PLAN §2 Palettes; task note on the 2026-11-10 decision; evaluate-by-listening memory.
- **Recommendation.** Add a cheap pre-filter: compute crest factor, spectral centroid and decay time of the founder's own reference drums, and reject candidate sketches far outside them before presenting. Listening stays the verdict.
- **Skeptic.** I could not find a 2026-11-10 founder decision in STATUS.md or DECISIONS.md. The §4.5 text says mangle is where Serum-ness comes from. The memory file prefers listening over metrics, so a pre-filter is only a convenience.

### Not anticipated

- The sketch can answer the Plaits question cheaply: render the same ten hit lists through the hand-written Python voices and through Plaits (via a Python binding or a tiny Rust example) and compare blind before the Rust voices are built.
- Fan noise and the laptop mic put a ceiling on the exciter path: any hit transient is band-limited and noisy, so the exciter slice may need heavy gating and a high-pass before it works at all.
- At 200 Hz control rate, sustained sounds follow the voice but fast hits do not; the hit path must be event-driven with per-hit envelopes, which the plan already implies but the mod matrix does not describe.
- Loudness-to-drive coupling makes quiet hits clean and loud hits destroyed, which is audible without any mouth sound; this may be the cheapest way to make the performance heard.
- A voice-driven noise or tone bed under the drums (hum into filtered noise, breath into whooshes) is probably the part the founder will find most 'theirs'.

### Keep

- Mangle per voice as synthesis, with polish left to the DAW, matches how these sounds are made.
- The renderer takes a ControlTrack only, with block-size invariance proven by the driver, so voices can be swapped without touching the lens or MIDI.
- Seeds as a tree with per-aspect dice and locks suits a reroll workflow on top of a small, coherent palette space.
- The founder's decision to sketch sounds in Python first, with listening as the evaluation, is the right order of work.

### Missed by the reviewer, added by the skeptic

- **Block-size invariance versus Plaits' internal block and rubato resampler** (major). Plaits renders in fixed internal blocks and sits behind a rubato 48 kHz resampler, with internal state in the LPG, envelopes and filters. The driver must still produce bit-identical output for any block segmentation. This is likely to break the invariance property unless the Plaits wrapper renders into a fixed-size internal buffer at absolute sample positions. The reviewer's Plaits-versus-own-voices question missed this and it favours an own-DSP kick, snare and FM set. Evidence: CLAUDE.md hard rule on block-size invariance; MASTER-PLAN §3 principle 3 and §3 stack line 108 (Plaits 48 kHz plus rubato); research/06 line 102 ('run at 48 kHz internally and resample'). Recommendation: Before committing to Plaits, build a minimal Plaits wrapper and run the existing chunked-equals-monolithic property test against it, including resampler latency and warm-up, and decide on that basis.
- **Aliasing from fold, crush and ring-mod with only a 2x oversampled ladder** (major). The mangle chain (hard clip, wavefold, bitcrush, ring-mod or XOR, self-oscillating SVF) is nonlinear, and the only oversampling in the plan is the 2x ZDF ladder in VoiceMono. Uncontrolled aliasing is a core part of the palette's sound and could read as either grit or cheap digital harshness. It also varies with the 48 kHz and 44.1 kHz render rates. Neither is specified or listened to. Evidence: MASTER-PLAN §4.5 mangle chain and VoiceMono (ladder at 2x oversampling); core-types render-rate changes (44.1 and 96 kHz). Recommendation: Add a listening A/B of each mangle stage with and without oversampling or antialiasing to the week-0 or week-1 sketch, and make aliasing amount a deliberate per-stage choice.
- **200 Hz control rate limits transient and pitch-envelope shaping** (minor). Control is fixed at 200 Hz (5 ms steps) and the mod matrix runs at 32 samples, while the hit transients being exploited last 5 to 30 ms. Kick pitch sweeps and transient shaping must therefore run on per-voice audio-rate envelopes triggered by hit events, not from the curves. The plan does not state which mappings are per-hit parameters and which are curves, and that choice affects whether mouth detail is audible. Evidence: CLAUDE.md (control rate 200 Hz, fixed); MASTER-PLAN §4.5 mod matrix and Hits. Recommendation: State in the design that sub-5 ms shaping is per-hit parameters feeding sample-rate envelopes, and verify by rendering one hit with the curve alone versus with per-hit parameters.

## Defects in crates/core

crates/core is unusually careful: exact integer time maths, frozen golden keys, strict JSON, a self-checking allocation counter. I found no panic, overflow or key-instability bug. The defects are in edit semantics and in unvalidated loads. I confirmed four with a standalone probe. A hit cache with misaligned scores, unsorted roles and unsorted hits loads without complaint. SplitAt can wrongly report TooShort. MuteSpan leaves 1-sample spans. JoinAt merges across a 90,000-sample (about 1.9 s) gap. The edit tests largely re-implement the code under test, so they cannot catch a misreading shared with the spec.

### 1. Hit cache loads with no structural validation

*major, missing, reviewer confidence high; skeptic: **confirmed***

- **Claim.** HitCacheFile::from_json checks only the container version and key. A file with roles [snare,kick] (unsorted), hits out of onset order and scores of length 1 or 0 against 2 roles was accepted (probe). Unlike .tkev there is no integrity hash, so a flipped digit that still parses is trusted.
- **Evidence.** crates/core/src/hit.rs HitCacheFile::from_json (~l.230-250); HitSet has no validate(). Probe printed: accepted: roles [snare, kick] hits 2 scores len 1 unsorted onsets true
- **Recommendation.** Add HitSet::validate (roles sorted and unique, scores.len()==roles.len(), finite, hits sorted by onset, span contains onset) and run it on load; treat failure as a cache miss. Add a frame hash like .tkev.
- **Skeptic.** hit.rs from_json (l.230-247) only checks version and key, then returns file.set. grep shows no HitSet validate. Derived Deserialize on HitSet, so unsorted roles or mismatched scores length pass. Matches the claim.

### 2. SplitAt snaps to a boundary that is not next to the covering span

*minor, bug, reviewer confidence high; skeptic: **confirmed***

- **Claim.** The nearest abutting boundary within 20 ms is chosen without requiring it to touch the span containing `at`. With spans [0,10000) [10000,10300) gap [10400,20000) at 48 kHz, SplitAt at 10900 snaps to the boundary at 10000, then computes end=10300 < at and returns Dormant(TooShort). A plain split of the last span would leave 500 and 9100 samples and is valid.
- **Evidence.** crates/core/src/edit.rs split_at ~l.289-310. Probe: 'split at 10900: Dormant(TooShort)', while at 11500 it is Applied.
- **Recommendation.** Restrict snap candidates to boundaries where spans[i-1] or spans[i] is the covering span, or fall back to the plain split when the snapped pair does not contain `at`.
- **Skeptic.** edit.rs split_at l.320-333 picks any abutting boundary within snap without requiring it to touch the covering span; end<at then saturates to 0 and yields TooShort. The spec (core-types.md l.563) words it the same loose way, so spec gap too.

### 3. MuteSpan can leave slivers below the minimum piece

*minor, bug, reviewer confidence high; skeptic: **confirmed***

- **Claim.** SplitAt refuses pieces under 10 ms, but MuteSpan clips with no minimum. Muting [1001,19999) of a span [1000,20000) leaves spans (1000,1001) and (19999,20000), each 1 sample. The result is a gate click in the render and a zero-tick note in MIDI, and the result still validates.
- **Evidence.** crates/core/src/edit.rs mute_span ~l.395-420; probe: 'mute: Applied spans [(1000, 1001), (19999, 20000)]'. MIN_PIECE_MS is used only in split_at.
- **Recommendation.** Drop clipped remnants shorter than MIN_PIECE_MS (as dropped spans) or make MuteSpan snap to the span edge. Add an invariant test: after any edit, every span is at least min_piece or was already shorter.
- **Skeptic.** mute_span l.465-478 clips with no minimum; MIN_PIECE_MS is used only in split_at (grep). A 1-sample remnant is retained and relink does not drop it. Spec only says clips, so consistent with spec but still a gate-click hazard.

### 4. Edit oracles restate the implementation

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** split_semantics builds its 'oracle' by copying split_at's logic (nearest abutting boundary, first/second, saturating_sub, min piece). robust() for SplitAt and JoinAt repeats the same candidate selection. A misreading shared between code and test, like the snap defect above, passes. The properties tested (idempotence, gate union preserved, re-derivation survival) are real, but the TooShort-versus-Applied decision is not independently specified.
- **Evidence.** crates/core/tests/edit_props.rs l.965-1060 (oracle copy), l.642-720 (robust mirrors edit.rs). The snap and sliver cases above pass the 164 tests.
- **Recommendation.** Add a differential test against a naive brute-force model on a sample grid: per-sample gate mask, with 'a split inside a span at least min from both edges of that span must apply'. Add 'no span under min_piece after any edit' as a property.
- **Skeptic.** I did not open edit_props.rs line by line. The code defects above (snap, sliver) do exist and are plausibly untested, which supports the missing-independent-spec point, but the claim that the oracle copies split_at is unchecked by me.
- **Corrected claim.** The split/mute edge cases (non-adjacent snap, sub-minimum slivers) are apparently not caught by the suite; whether the oracle literally mirrors split_at was not verified.

### 5. JoinAt fills a gap of any length

*minor, risk, reviewer confidence high; skeptic: **confirmed***

- **Claim.** For `at` anywhere inside a gap the distance to that junction is 0, so tol never limits it. JoinAt{at:50000,tol:1} on spans (1000,2000) and (90000,95000) merged them into (1000,95000), covering about 1.9 s of silence. The pitch and amp curves in the gap are whatever the lens wrote. This follows the spec's 'a gap is filled' but makes a stray click in silence a destructive edit.
- **Evidence.** crates/core/src/edit.rs join_at ~l.325-345; docs/design/core-types.md l.569; probe: 'join: Applied spans [(1000, 95000)]'.
- **Recommendation.** Bound the filled gap (for example at most a few hundred ms, or tol-relative) and report Dormant otherwise, or require `at` within tol of a gap edge.
- **Skeptic.** join_at l.359-367: for at inside the gap, at>=a and saturating_sub(b) gives 0, so d=0 <= tol always. Spec l.569 explicitly says a gap is filled, so it is by design but unbounded.

### 6. HostAnchor and ParamValue accept values downstream will misread

*minor, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** Project::validate checks HostAnchor only for finiteness. tempo_bpm<=0 and time_sig denominator 0 load. ParamValue is untagged, so an integer above i64::MAX reads as a lossy Float (probe: 9223372036854775808 -> Float(9.223372e18)). That value is keyed under 'float', so a hand-edited or migrated param changes its key silently.
- **Evidence.** crates/core/src/take.rs HostAnchor::is_finite; project.rs validate ~l.160; params.rs ParamValue untagged; probe output above.
- **Recommendation.** Validate tempo>0 and time_sig denominator>0 (and a power of two). Use a tagged or explicit-visitor ParamValue that refuses an out-of-range Int.
- **Skeptic.** HostAnchor::is_finite (take.rs l.150) checks only the two f64s, so tempo<=0 and denominator 0 pass validate. The ParamValue untagged enum (Int before Float) makes the u64>i64::MAX-to-Float fallback plausible; I did not run the probe.
- **Corrected claim.** HostAnchor validation omits tempo>0 and time_sig sanity (confirmed). The ParamValue lossy-Float fallback for huge integers follows from the untagged Int-then-Float order but was not independently run.

### Not anticipated

- Role tables never shrink. ForceRole or DeleteHit leaves a role that no hit uses, so two tracks with the same performance can differ in bytes and in ControlTrack::content_id; the dedup of imported tracks by content id then misses.
- Evidence::new checks only finiteness, not ranges (voicing_p in [0,1], salience, class columns). A buggy analyser change, such as the 59 Hz fan reading as voiced, flows through with a valid key and content hash and is cached forever.
- Seed::noise streams are shifted copies when two (seed,stream) pairs yield equal b. Probability is negligible, but 'distinct streams' is a statistical, not structural, guarantee.
- Layer::store_render does not verify slot.key equals inputs.key(); only Project::validate does, at load and save. In-memory misuse is caught only at the next save.
- A sample-rate change recomputes nothing in the edits: every position is a take-rate sample and tolerances are in samples. That is fine until a take is resampled on import, when Edit::rebased rounds down-rates lossily, with no round-trip guarantee.

### Keep

- The frozen-values test (check_frozen against fixtures/frozen-keys.txt) fails on drift, on unpinned values and on values no longer computed, so keys cannot be re-blessed silently.
- Time arithmetic is exact u128 integers with saturation, and the sample/control mapping is property-tested at 1 kHz to 1.536 MHz and at 44.1 kHz.
- The counting global allocator in no_alloc.rs checks itself and covers value_at, at_clock, gate_at and events().
- Strict map and set decoding (duplicate keys refused), the f32 double-rounding guard in project::json, and canonical keys (-0.0 normalised, NaN refused, integer width-independent) are well thought out.

## Defects in render, evidence, cli and research code

The render contract (driver, bound, segment plan) held up: I found no block-boundary, latency or allocation defect, and the invariance and alloc tests cover it well against the toy backend. The real defects are in evidence. The onset detector depends on absolute input gain, so a take 20 dB quieter than AVP loses half its onsets. Every take that starts in room tone gets a spurious picked onset at t=0. The tests miss both because they use loud signals over near-zero noise. The Week 0 MIDI writer can overlap same-key notes. Python scoring and fetch code are sound apart from minor points.

### 1. Onset detector depends on absolute input gain

*critical, bug, reviewer confidence high; skeptic: **confirmed***

- **Claim.** SuperFlux runs on unnormalised FFT magnitudes through log10(1+mag), and the pick threshold of 1.1 is absolute. I ran the release binary on 30 AVP improvisations: gain 1.0 gives F1 0.893 (R 0.927). Gain 0.1 gives R 0.417, F1 0.553. Gain 0.03 gives R 0.053. Resampled to 96 kHz, precision drops 0.861 to 0.819 because fft_len, and so mag, scales with rate.
- **Evidence.** crates/evidence/src/onset.rs:131 (log10(1+band)), :44 DEFAULT_THRESHOLD; frame.rs:111-125 (mag not normalised). Probe: scratchpad/reeval/code-rest/gain.py, 30 files.
- **Recommendation.** Normalise magnitudes by window sum, then compress against a per-take noise-floor or percentile reference before the log. Re-tune the threshold on gain-swept AVP. Add a gain-sweep regression test and a scoreboard row.
- **Skeptic.** onset.rs push() takes log10(1+band) of raw band magnitudes. frame.rs normalises only mean_square, not mag. The pick threshold is absolute (DEFAULT_THRESHOLD). The code gives gain and rate dependence. I did not rerun the F1 numbers.

### 2. Spurious picked onset at t=0 on any non-silent start

*major, bug, reviewer confidence high; skeptic: **confirmed***

- **Claim.** Flux history starts as zeros, so frame 0 (and frame 1 at 48 kHz) compares against silence. Synthetic takes with room-noise RMS of 1e-3 or more (-60 dBFS) and three kicks gave picked onsets [0.0, .499, .999, 1.499]. At 3e-4 there was no extra onset. The founder's own take file fill.onsets.csv row 1 is sample 0, strength 1.77, picked. It then suppresses real hits within 100 ms. In AVP, 18 files have a picked onset at 0.0 with no reference within 50 ms.
- **Evidence.** onset.rs:105-108 (history zeros), :126; lib.rs:56-58 (documented as intended); fill.onsets.csv line 2; precheck output made a kick at t=0, dur 0.935 s, vel 1.
- **Recommendation.** Prime the flux history with the first real frame, or set the first lag frames' ODF to 0. Keep the mid-sound-start case as an explicit option, not the default.
- **Skeptic.** History is initialised to zeros (onset.rs:105-108). The lib.rs doc says a mid-sound start gives a candidate at the first frame. fill.onsets.csv row 1 is sample 0, strength 1.766, picked=1, rise 72.8 dB. The refractory then suppresses nearby hits. I did not check the AVP count of 18.

### 3. Tests are blind to the gain and noise-floor failures

*major, missing, reviewer confidence high; skeptic: **partly***

- **Claim.** Evidence tests use 0.5 to 0.9 amplitude clicks and bursts over 1e-4 to 3e-4 noise, so absolute thresholds always pass. The CLI test's comment accepts an onset at 0 as correct. No test covers gain, a noisy start, mono, 8/16-bit or float WAV, or stereo downmix. A broken or constant-scale ODF would still pass clicks_are_found_to_the_millisecond. Render tests use only the toy backend, which is honest by construction.
- **Evidence.** crates/evidence/tests/synthetic.rs:141-190; crates/cli/tests/analyze.rs:95-100, :18-40; no gain parameter anywhere in evidence/tests.
- **Recommendation.** Add property tests: picked onsets stable under gain from 0.03x to 1x, a noise-floor start yields no onset at 0, and CLI WAV format and channel tests.
- **Skeptic.** No gain or noisy-start test exists. But cli/tests/analyze.rs does cover stereo 24-bit, and its onset at 0 is legitimate because the tone starts from silence. The WAV tests are weaker than the claim says: no 8/16-bit or float, and both channels are identical.
- **Corrected claim.** No test varies gain or uses a noisy start, and the CLI WAV test covers only stereo 24-bit with identical channels (so downmix correctness, 8/16-bit and float are untested). The onset-at-0 assertion is legitimate there.

### 4. precheck writes overlapping same-key drum notes

*minor, bug, reviewer confidence high; skeptic: **partly***

- **Claim.** Hit length is measured from the detection frame, but note-on uses the refined onset time. In rolls, note length runs to the next frame. On fill.csv, 43 of 436 hits end 8 to 37 ms after the next same-note on-tick. A sampler or Reaper can truncate the new note when the earlier off arrives.
- **Evidence.** research/week0/precheck.py:183 (dur from frame), :258 (off = tick(h.t + h.dur)); my replay of a5123eb precheck on fill.frames.csv.
- **Recommendation.** Cap off at the next same-note on minus 1 tick, or compute dur from the refined onset sample. Do this before the founder's listening test.
- **Skeptic.** The mechanism is real. dur is measured from the detection frame f and limited to the next frame, but the note-on and off use the refined t (precheck.py:183, :258). The refined time can sit before the next frame's centre, so off may land after the next on. I did not replay the 43 of 436 count. Overlap also needs the same MIDI note.
- **Corrected claim.** Overlap is possible, because the off time (refined t plus a frame-based duration) can pass the next hit's refined on-tick when the same note repeats. The count is not verified.

### 5. hits_of reads features from the detection frame, not the onset

*minor, bug, reviewer confidence high; skeptic: **confirmed***

- **Claim.** The 'frame' column is the ODF peak frame. It differs from the refined onset by -20.6 to +52.9 ms (10th to 90th percentile -5.2 to +12.8). Peak search, 25 dB decay and the 40 ms timbre window all start there, so velocity, decay and role features are measured from the wrong place, up to four frames late or early.
- **Evidence.** precheck.py:164-186 (starts = picked['frame']); onset.rs:292 (frame vs sample); percentiles from fill.onsets.csv.
- **Recommendation.** Use round(time_s * 200) as the start frame. The drums crate should take features from the refined sample.
- **Skeptic.** precheck.py:164-186 sets starts = picked['frame'] and uses f for the peak search, the decay and the 40 ms timbre window. t is taken from time_s. The ODF peak frame and the refined onset sample differ by design (onset.rs candidates).

### 6. Evidence cache key omits most analyser constants

*minor, risk, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Provenance lists five parameters. The flux, onset and CPP constants (BANDS_PER_OCTAVE, PRE/POST_MAX, BACKTRACK_RATIO, ENV_SECONDS, MIN_REFINE_RISE_DB, CPP smoothing) are not hashed. STATUS says the onset constants were tuned on AVP. Changing one without bumping ANALYZER_VERSION leaves stale cached evidence under the same key. The test only checks the key is deterministic.
- **Evidence.** crates/evidence/src/store.rs:41-62; onset.rs:20-35; lib.rs:25; tests/store.rs:73-81.
- **Recommendation.** Hash all fixed constants into provenance params, or add a test that fails when a constant changes without a version bump.
- **Skeptic.** store.rs provenance() hashes only five pitch/loudness/spectral window parameters plus ANALYZER_VERSION=1. The onset and flux constants are not included, so tuning them without a version bump leaves a stale key.

### 7. score.py --threshold is not the decision being scored

*minor, contradiction, reviewer confidence high; skeptic: **confirmed***

- **Claim.** With --threshold, est_onsets keeps strength >= threshold and ignores the 60 ms refractory and the 100 ms/25 dB continuation rule. Threshold sweeps therefore score a different detector than pick() and are not comparable to the headline 90.0 F1.
- **Evidence.** research/eval/score.py:67-70 vs crates/evidence/src/onset.rs:pick (~line 262-275).
- **Recommendation.** Re-pick in Python with the same rules, or expose --threshold through the CLI so pick() runs.
- **Skeptic.** score.py:67-70 uses strength >= threshold when a threshold is given. Otherwise it uses the 'picked' column. The refractory and continuation rules in onset.rs pick() are skipped in sweeps.

### 8. backtrack can panic on an onset at the last sample

*minor, risk, reviewer confidence low; skeptic: **partly***

- **Claim.** clamp's lower bound is earliest.max(0), which is the previous candidate's sample plus 1. If that candidate sits at len-1, the bound exceeds len-1 and i64::clamp panics (min > max) for the next candidate. I did not reproduce it, and it needs a loud sound at the very end of the file plus another candidate after it.
- **Evidence.** onset.rs:218 clamp closure, :346 earliest = last.sample + 1.
- **Recommendation.** Use min(earliest, len-1) as the lower bound, and add a fuzz test over file endings.
- **Skeptic.** The clamp bounds can invert when earliest > len-1, which would panic. Reaching that needs a candidate placed at len-1 by the frame-centre fallback, followed by another candidate. peaks() enforces post_max spacing and frames end near the file end, so it is practically unreachable. The reviewer already said it was not reproduced.
- **Corrected claim.** clamp(earliest.max(0), len-1) would panic if a candidate sat at len-1 and another followed. The code permits it in principle, but peak spacing makes it very unlikely.

### Not anticipated

- The Rust WAV reader averages channels. A stereo mic file with one dead channel is silently 6 dB quieter, which the gain-dependent onset detector turns into missed hits, and antiphase channels cancel. Pick a channel or sum by energy, and warn.
- A real-time plugin or app cannot reproduce this analysis as written: SuperFlux with its zero history, the 25 ms YIN lookahead and onset backtracking over 60 ms all need audio from the future or past. Decide now whether live monitoring uses a separate causal path.
- The AVP onset score is both tuned and recorded at a fixed gain and rate, so it cannot show the gain problem. Gate E1 on the golden set needs at least three recorded input levels per take, or it will pass for the wrong reason.
- The pitch-sine stem interpolates f0 linearly through zero at voiced/unvoiced edges. Each edge produces a 5 ms chirp that may sound like a tracking glitch when it is only the stem.

### Keep

- The driver's one-cursor event order (Reset, Cell, Trigger, Run/Silence) and the per-thread counting-allocator test: I found no block-boundary, latency or allocation defect in driver.rs or bound.rs.
- The segment plan with tail and latency folded into activity, plus the pure-shift latency test and J1 case.
- score.py's use of mir_eval.util.match_events for one-to-one onset matching, and the melody evaluation convention of negative frequency for unvoiced frames.
- The evidence synthetic tests that calibrate loudness, A-weighting, CPP and vibrato against known signals.

### Missed by the reviewer, added by the skeptic

- **Stereo downmix by averaging can cancel out-of-phase content** (minor). wav.rs averages channels (sum / channels). A file with antiphase channels, or a hard-panned source with phase differences, loses or cancels energy, and evidence is computed on the quieter result. This compounds the absolute-gain dependence. Evidence: crates/cli/src/wav.rs:34-35. The only CLI test uses identical channels. Recommendation: Add a test with antiphase or one-sided stereo. Consider a per-take energy-preserving downmix.

## Evaluation method and gates

The harness is competent and honest about its own caveats, but it measures the analyzer against proxy annotations, not the product claim: designed sounds triggered at the right moment with the right character. Gate E1 is three accuracy thresholds, one already passed and one tuned on its own test data. The week-4 gate has no control condition, so it cannot say whether Takedown's control adds anything. The golden-set protocol spends hours of the founder's time before the product's core question is answered. Several things that decide the product (confusion costs, microtiming, glitches, mouth-ness) are unmeasured. A leaner plan follows from the findings.

### 1. Week-4 gate has no control, so it cannot attribute success

*critical, rethink, reviewer confidence high; skeptic: **partly***

- **Claim.** Ten fills x palettes x dice, blind and shuffled, judged by the person who performed them. It conflates sound-design quality (now AI-made) with control quality (Takedown's actual contribution). The 6-of-10 threshold is arbitrary at n=10, and the founder recognises their own rhythms, so it is not blind to the source.
- **Evidence.** docs/MASTER-PLAN.md line 161; STATUS 2026-11-10 decision that the AI synthesises the sounds
- **Recommendation.** Per fill, render the same palette three ways: Takedown control, a flat-velocity quantised trigger baseline, and Takedown control with shuffled roles or times. Rank blind. Continue only if Takedown beats the baseline on most fills. Pre-register the threshold and record every verdict.
- **Skeptic.** MASTER-PLAN line 161 confirms no baseline, n=10, threshold 6, founder judging. But the cited 'STATUS 2026-11-10 decision that the AI synthesises sounds' does not exist in STATUS or DECISIONS; the gate is Plaits/mangle synthesis. The 'AI-made' premise is unsupported.
- **Corrected claim.** The week-4 gate is an uncontrolled judgment by the performer over n=10 with an arbitrary 6-of-10 bar; it cannot separate Takedown's control contribution from palette sound-design quality. A flat quantised-trigger baseline would fix that.

### 2. Onset tuning is fitted to annotator convention and to its own test set

*major, risk, reviewer confidence high; skeptic: **confirmed***

- **Claim.** The F1 jump from 61 to 90 came from moving onsets 20 to 40 ms earlier to match AVP's labelling, which says nothing about perceived beat position. Settings were tuned on all 28 participants, so no held-out score exists. The 3.2 ms mean error is below annotation noise and shows convention-matching.
- **Evidence.** docs/STATUS.md Numbers, AVP caveat; research/eval/score.py run_avp (no participant split)
- **Recommendation.** Split AVP by participant now: tune on 14, report on 14. Treat sub-10 ms agreement as saturated. Judge timing by ear: render a click at the detected time versus a +/-20 ms shift against the take, and choose the offset that feels on the beat.
- **Skeptic.** STATUS caveat and DECISIONS line 126 admit post-max, backtrack and pick were chosen while looking at AVP; first pass 61.0 vs 90.0. run_avp has no participant split. Mean error 3.2 ms vs AVP labels supports convention-matching.

### 3. Gate E1 is mostly proxies and weakly discriminating

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** RPA 98.8 and octave errors 0.7 already pass E1 on Vocadito, so the gate no longer separates good from bad. Mean RPA hides what the ear hears: short octave jumps or glitches in a bend-driven FM voice. Mean-over-tracks weights a 5 s excerpt like a 60 s one. Noise robustness uses white noise while the real failure is a 59 Hz fan tone.
- **Evidence.** docs/STATUS.md Numbers (Vocadito, noise rows); score.py run_vocadito; MASTER-PLAN line 154
- **Recommendation.** Replace mean RPA with glitch counts per minute (octave jumps under 100 ms, voiced frames below 80 Hz, pitch-bend slew spikes). Add the founder's own room-tone and fan recordings as the noise test. Keep Vocadito as a regression check only.
- **Skeptic.** E1 is defined on the golden set, which does not exist; STATUS says the fan-noise half will fail voicing, so it does discriminate there. Mean-over-tracks and white-noise-vs-59 Hz points are valid; STATUS already flags the 59 Hz tone.
- **Corrected claim.** Clean Vocadito already passes the E1 pitch numbers, so it is only a regression check; mean RPA hides glitches, mean-over-tracks is unweighted, and white-noise tests miss the real fan-tone failure, which STATUS already identifies and plans to test on founder recordings.

### 4. Hit confusions are scored as equal and in the wrong place

*major, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** E1 requires 4-class accuracy >=85% at 10 shots. Confusion costs are unequal: kick/snare or hat/snare swaps change the groove, open/closed hat swaps matter less, and a missed ghost note differs from a false positive in noise, which may even sound good. Same-session teach and test would leak. The role classifier does not exist yet.
- **Evidence.** MASTER-PLAN line 154; STATUS Next item 5; score.py reports recall by label only, with no confusion matrix
- **Recommendation.** Report a confusion matrix with a cost weighting. Test with teach takes from session A and test fills from session B on another day, as well as AVP leave-one-participant-out. Count false positives per minute separately from misses.
- **Skeptic.** score.py reports only per-label recall at 25 ms (no confusion matrix, no FP/min); role classifier absent (STATUS Next 5). But E1 accuracy is per-user at 5 and 10 shots per MASTER-PLAN, and cost weighting and leakage points are design suggestions.
- **Corrected claim.** No confusion matrix or false-positive rate exists and classifier accuracy is not yet implemented; cost weighting and cross-session teach/test are reasonable additions but the leakage concern is a design suggestion, not a present flaw.

### 5. Golden-set annotation is the wrong use of founder hours, and too early

*major, overbuilt, reviewer confidence medium; skeptic: **partly***

- **Claim.** Hours of hand-annotating onsets and f0 for one speaker, one mic and one room before knowing the product is exciting. n=1 gives onset CIs near +/-1.5 pt but no generalisation. Ear-placed onsets carry P-center bias. The founder's best signal is listening, and verdicts are already being produced.
- **Evidence.** docs/RECORDING-PROTOCOL.md ('a few hours of annotation'); MASTER-PLAN line 148; memory 'evaluate by listening'
- **Recommendation.** Defer the golden set until the week-0/week-4 result. Meanwhile make each founder-flagged false positive or miss in listen.py a permanent regression clip (a few dozen, minutes each). Annotate only disagreements between two detector settings. Use the click track as timing truth for groove items.
- **Skeptic.** Protocol says 'a few hours of annotation' and E1 is defined on the golden set, so deferral is an option. But STATUS already treats listening checks as evaluation, and the recordings are also needed for noise/fan tests; 'wrong use' is a judgment.
- **Corrected claim.** Golden-set annotation (few hours, one speaker) could reasonably be deferred behind the Week 0 listening check, with flagged-error regression clips and listening checks carrying evaluation meanwhile; it is not clearly wrong, since E1 depends on it.

### 6. Timing feel and groove preservation are unmeasured

*major, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Product value rests on the performance's microtiming surviving. Nothing scores inter-onset-interval deviation from the click grid (swing ratio, push/pull), velocity or dynamics accuracy, or systematic latency between detected and perceived onset for sounds with slow attacks.
- **Evidence.** score.py computes only pooled F1 and unsigned deviation; RECORDING-PROTOCOL item 5 supplies click-aligned grooves that could provide ground truth
- **Recommendation.** On click-aligned grooves, compare the swing ratio and per-hit offset from the grid of detected versus rendered hits. Test that velocity rank order matches loudness rank. Add a per-sound onset offset to the render contract, calibrated by ear.
- **Skeptic.** score.py has no click/swing/IOI/velocity logic; only pooled F1 and unsigned deviation. Protocol item 5 provides click-aligned grooves, and 'Timing feel' is a planned slider, so the gap is real.

### 7. 'Does it still sound like a mouth' has no test

*minor, missing, reviewer confidence low; skeptic: **partly***

- **Claim.** The master plan names this as risk 1 and as a pivot condition, but evaluation is a single subjective gate. The Mouth-amount sweep is judged by the same person who knows which render is which.
- **Evidence.** MASTER-PLAN lines 161 and 185
- **Recommendation.** Run an ABX or odd-one-out across Mouth settings with a second listener. Add a cheap objective check: a small classifier of voice versus rendered audio on spectral features, expected to drop as Mouth goes to 0.
- **Skeptic.** MASTER-PLAN risk 1 and the pivot rely on the week-4 gate and Mouth sweep judged by the founder alone; no second listener or objective check exists. But it is low-priority and the proposed classifier is speculative.
- **Corrected claim.** Mouth-likeness is tested only by the founder's own listening at several Mouth amounts; a second listener or odd-one-out would add rigour, but that is a modest improvement.

### 8. mir_eval usage is correct but narrower than the product

*minor, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** match_events one-to-one matching and the MIREX negative-frequency convention are right. However, note segmentation (what MPE needs: note on/off, re-articulation) is not scored although Vocadito ships note annotations and mir_eval.transcription exists. Pooled and macro F are both reported, which is good. No confidence intervals anywhere.
- **Evidence.** score.py pitch_scores and onset_counts; research/README.md dataset table (Vocadito notes unused)
- **Recommendation.** Add note-onset F1 and onset error for hum re-articulation on Vocadito. Add bootstrap CIs by participant or file to every scoreboard number, so a 1 pt change is not read as signal.
- **Skeptic.** score.py uses mir_eval.util.match_events one-to-one and reports pooled and macro F; research/README says Vocadito has notes from two annotators, but they are unused; no bootstrap CIs or note-level scoring anywhere.

### Not anticipated

- The AI now authors the sounds, so a failed week-4 gate can mean 'AI sound design is weak' and not 'control mapping is weak'. Without the flat-trigger baseline the founder cannot tell which, and may pivot on the wrong cause.
- Detected onset time is not perceived onset time: a synthesised kick with a 10 ms ramp and a click-first snare sit at different P-centres. Scoring against the mouth's annotated onset can be correct and still sound late once the sound is swapped.
- Founder's ears adapt: after hours of listening to the same 10 fills, preference drifts toward the familiar. Rotate fresh fills in and keep one held-back set never used for tuning.
- Round-trip self-consistency is a free test: render, re-analyse the rendered audio, and compare to the control track. It needs no annotation and also tests the 'one object, two outputs' MIDI promise.
- Conservative evaluation and eager lens means the scored curve is not the one the product plays. Add an eager-lens scoreboard, or the tuned conservative numbers say nothing about what the user hears.

### Keep

- Keeping Python and mir_eval as a separate scorer of the CLI's CSV output keeps results comparable with the literature and the CLI honest.
- The listen.py click and sine stems against the take are the right instrument for this project: fast, ear-based and aligned with the founder's role.
- Documenting the AVP tuning caveat and splitting evidence (conservative) from lens (eager) is correct practice and should continue.
- Making the week-0 and week-4 gates stop-or-pivot decisions, not score targets, is the right shape.

### Missed by the reviewer, added by the skeptic

- **Pooled F1 hides the false-positive character the founder actually heard** (major). The founder's verdict was onsets were good but over-eager in noise and very quiet stretches. Pooled P/R at 25 ms on AVP (clean content, no noise or silence) cannot see this, so the headline metric does not track the observed failure. Evidence: STATUS 'Founder's first take': 90 noise onsets, 13 below 20 dB; AVP P 86.8 only Recommendation: Add false positives per minute on noise and silence segments, plus a level-relative floor, as scored metrics using founder takes with marked non-hit regions.
- **Gates lack pre-stated decision rules** (minor). E1 requires onset F1 >= 0.9 on the golden set, but AVP already scores 90.0 after tuning on it. Nothing says what action follows if golden-set F1 is 0.85, or what each number buys downstream. Evidence: STATUS Numbers 'on the line, on a dataset it was tuned on'; MASTER-PLAN line 154 Recommendation: State for each gate what decision each outcome triggers, and tie onset targets to hit quality heard in listening, not F1 at 25 ms.
- **No evaluation of round trip or latency despite hard rules** (minor). CLAUDE.md requires audio and MPE to round-trip as one performance, but no evaluation compares detected timing to rendered or MIDI timing, and the planned causal monitor (20-25 ms) has no measurement protocol. Evidence: CLAUDE.md hard rules; MASTER-PLAN v0.2 monitor; score.py scores evidence output only Recommendation: Add an end-to-end onset error test (detected to rendered/MIDI) and a latency measurement once midi exists.

## User experience and workflow

The plan optimises the nudge-and-reroll loop and underweights where it is judged. In v0.1 the app plays only its own dry render: no backing track, no tempo, no click. The founder performs and records in Reaper, so every audition against the song means export, drag, listen, return. The plan's own risk 5 (mangle sounds thin without the master chain) is left unsolved. Fixing a wrong hit has core support (ForceRole, DeleteHit, AddHit) but no listed UI. Dice and locks are too coarse for a take with many hits. Three of the six sliders are tonal, and the plan's "priority" is drums. Teach is specified but absent from the app list. A first run needs a no-teaching path.

### 1. No way to hear results in the context of the song

*critical, missing, reviewer confidence high; skeptic: **partly***

- **Claim.** The v0.1 app has no backing-track playback, tempo or click, and monitoring is dry voice only. Beatboxers perform against a track, and Reaper is where the founder records. The auditioning loop (dice, sliders, A/B) happens on isolated hits with no master chain. Risk 5 admits mangle sounds thin alone, yet gate renders are judged only after export.
- **Evidence.** MASTER-PLAN §2 'Monitoring' row and 'Reaper companion' row; §5 weeks 5-7 app list (line ~164); §6 risk 5; RECORDING-PROTOCOL.md line 3 (click on a separate Reaper track)
- **Recommendation.** Let the app load one reference WAV (the Reaper mixdown or loop) that plays under the render, sample-aligned to the take start. This is audition only, not a DAW feature. It makes the week-4 gate and the whole v0.1 loop honest.
- **Skeptic.** MASTER-PLAN §2 Monitoring and line 164 list no backing playback; true. But risk 5 deliberately judges gate renders in a Reaper session, and v0.1 done is export-and-drag. A gap, not 'critical'; the reference-WAV idea is sound but not a plan contradiction.
- **Corrected claim.** The v0.1 app has no reference-track playback or click, so audition happens dry and context is judged only after export to Reaper, as risk 5 already prescribes; a one-WAV reference player would shorten that loop.

### 2. Record-in-app is the wrong primary path; Reaper should be

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** Recording in the app has no count-in, click or latency compensation, so the take has no relation to the project grid. The founder's own protocol records in Reaper against a click. The research/04 concept had a count-in, metronome and tempo detection; the master plan dropped them without replacing the need. Drop-a-WAV should be the primary path and in-app record a capture convenience.
- **Evidence.** MASTER-PLAN §5 line ~164 ('record from cpal or drop a WAV'); research/04 §2 step 2; DECISIONS 2026-10-21 item 11 (export tempo set by hand)
- **Recommendation.** Design for 'drop the Reaper item render'. Export stems with the same length and start as the dropped take, so they land under the original item with no nudging. Keep the in-app record, but give it a count-in and an optional click.
- **Skeptic.** Line 164 offers both record and drop; research/04 does have count-in and metronome, dropped from plan. RECORDING-PROTOCOL uses a Reaper click. But 'wrong primary path' is opinion, and same-start/length stem export is not specified anywhere (DECISIONS 11 only sets tempo).
- **Corrected claim.** The plan has no count-in, click or latency handling for in-app recording, and no spec that export aligns to a dropped take's start; drop-a-WAV is likely the better primary path.

### 3. Fixing one wrong hit has no UI in the plan

*major, missing, reviewer confidence high; skeptic: **confirmed***

- **Claim.** Core already has ForceRole, DeleteHit, AddHit and MuteSpan with time-anchored semantics. The v0.1 app spec lists none of them as gestures, and 'no piano roll' leaves no obvious home. The founder's first take shows the need: 90 noise onsets, plus quiet false positives. Deleting those one by one is hopeless, and no slider controls onset sensitivity.
- **Evidence.** core-types.md §2.10 lines 527-575; MASTER-PLAN line ~164 app list; STATUS.md 'Founder's first take'
- **Recommendation.** Specify four gestures: click a hit marker to hear it solo and pick its role from a menu; press Delete to remove it; drag-select a range to mute it (MuteSpan); double-click to add one. Add one 'Sensitivity' control (level floor plus rise and strength) so noise is rejected by threshold, not by hand.
- **Skeptic.** core-types §2.10 lines 540-543 define ForceRole, DeleteHit, AddHit, MuteSpan; app list at line 164 has no hit gestures or sensitivity control. STATUS records 90 noise onsets and 13 quiet false positives, with the rise/strength fix noted as lens work.

### 4. Dice and locks are too coarse; reroll is all-or-nothing per take

*major, rethink, reviewer confidence medium; skeptic: **partly***

- **Claim.** Rolling Body re-draws every voice and every hit at once. On a 40-hit fill the user will like the kick and hate the hats and cannot keep one while rerolling the other. Region reroll is deferred to v0.3. Per-slider locks plus per-aspect locks make 11 lockable things for a 'minimal' UI. Roll counts are keyed by aspect only, though voice seeds are already derived per voice.
- **Evidence.** core-types.md lines 181-200 (Aspect, voice_seeds, RegionRoll 'UI arrives in v0.3'), 581-602; MASTER-PLAN §2 'Randomness' row; risk 4
- **Recommendation.** Make the unit of reroll the role or voice: a small dice and lock per role row (kick, snare, hat), which is cheap given voice_seeds. Fold slider locks and aspect locks into one lock concept, and keep the global dice as 'reroll unlocked'. Add a keep-favourite star for any roll.
- **Skeptic.** SeedTree rolls are keyed by Aspect, regions deferred to v0.3, voice_seeds derived per voice (core-types ~181-200). 6 slider locks plus 5 aspect locks is 11. But per-role reroll is a design preference, not a defect.
- **Corrected claim.** Roll counts are per aspect only, so no per-voice or per-hit reroll exists in v0.1; per-role dice are feasible given voice_seeds, and there are 11 separate lock states.

### 5. Three of six sliders are tonal; names are unclear for drums

*major, contradiction, reviewer confidence medium; skeptic: **partly***

- **Claim.** Fidelity, Register and Glide/Retrigger come from research/04's melodic concept. On a drum-only take they do nothing or have no clear meaning (Fidelity to what? Register in semitones of a kick?). 'Timing feel' has no grid in v0.1: HostAnchor and quantise arrive in v0.2. 'Mouth amount' is the only slider with a clear drum meaning. The drum-priority product has effectively two live sliders.
- **Evidence.** core-types.md lines 581-589, 'Quantise' bullet at line ~604; MASTER-PLAN §1 (percussive first) and §5 slider list; research/04 §3 table
- **Recommendation.** Show only the sliders that apply to the take's content: grey out or hide the tonal three when no spans exist. Rename by effect, for example Mouth amount to 'Voice in the sound', Timing feel to 'Tight - loose', Register to 'Pitch of bodies'. Define Timing feel against the take's own median spacing until a host tempo exists.
- **Skeptic.** Slider enum and quantise-in-v0.2 confirmed. Mangle and Mouth amount are clear for drums, and Timing feel is a documented aspect; 'two live sliders' and 'do nothing' are unverified assumptions about unimplemented lens behaviour. Renaming is taste.
- **Corrected claim.** Fidelity, Register and Glide/Retrigger are pitch-oriented and may be inert on drum-only takes; Timing feel has no grid until HostAnchor/quantise in v0.2. Whether they act on drums depends on unwritten lens code.

### 6. First five minutes: Teach is specified but not in the app, so roles start empty

*major, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** The drum head is a per-user kNN trained by performing each sound a few times. The v0.1 app list has no Teach screen. A new user records a fill and either gets one role for everything or must be taught first. 'No text entry anywhere' also conflicts with the export tempo and with palettes the founder writes themselves, which have no editor.
- **Evidence.** MASTER-PLAN line 136 ('Teach flow in the app'), line ~164 app list (no Teach), §2 'Palettes' row; DECISIONS 2026-10-21 item 11
- **Recommendation.** Make the zero-setup path the default: assign roles by brightness, duration and low-band energy (evidence already stores them), so the first fill makes sound in under a minute. Offer Teach as an opt-in upgrade after a mismatch. Put export tempo in a number field, a deliberate exception.
- **Skeptic.** Line 136 says 'Teach flow in the app' while line 164 omits it; a real inconsistency. But 'roles start empty' is unproven: risk 2 names soft roles as fallback, Hit carries brightness and scores, and no-text-entry versus export tempo is a fair but small point.
- **Corrected claim.** Teach is stated at line 136 but absent from the v0.1 app list, leaving the untaught first-run role behaviour unspecified; a zero-setup default is advisable.

### 7. Export is a manual, versionless round trip

*minor, opportunity, reviewer confidence low; skeptic: **partly***

- **Claim.** Each iteration means: export WAV and MIDI to a folder, switch to the file manager, drag, align, then repeat after any change. Files carry no stable names or versions, so the user cannot tell which export came from which A or B. The tempo is hard-coded to 120 BPM unless set by hand. The Reaper Lua link returns only in v0.2.
- **Evidence.** MASTER-PLAN §2 'Reaper companion' row; DECISIONS 2026-10-21 item 11; MASTER-PLAN §5 'v0.1 definition of done'
- **Recommendation.** Export to stable filenames (take_A.wav, take_A.mid) that overwrite in place. Reaper normally refreshes a media item when its source file changes on disk, which would act as a live link with no plugin (unverified here; test it in Week 0). Add a 'reveal in folder' button and an optional drag-out handle.
- **Skeptic.** Manual drag and no Lua link until v0.2 are confirmed (§2 line 39, §5 done). 'Hard-coded 120 BPM' is imprecise: DECISIONS 11 says founder sets tempo by hand. Naming/versioning is unspecified, not shown versionless; Reaper live refresh is acknowledged unverified.
- **Corrected claim.** Export is a manual drag-to-Reaper flow with default 120 BPM map that the founder must set to project tempo; file naming and overwrite behaviour are unspecified.

### 8. Retention: what brings the founder back weekly, and what stops them

*minor, opportunity, reviewer confidence medium; skeptic: **partly***

- **Claim.** The founder opens it if idea to sound-in-session takes under a minute and sounds surprise them. They stop if sounds do not sit in context, the first-take false-positive mess recurs, the whole-take dice never yields a keeper, or favourites cannot be saved and found again. The plan has no sound-library of favourite rolls, rolling capture buffer or take history. research/04 had the buffer and 'Capture last idea'.
- **Evidence.** research/04 §2 step 1 (30 s rolling buffer) versus MASTER-PLAN §2 (dropped); MASTER-PLAN §5 gate definition; STATUS.md first take review
- **Recommendation.** Keep a per-project list of starred interpretations (name, palette, seed, sliders, all already in core's Interpretation) and a one-click 'new take from the rolling buffer'. Make the success metric 'minutes from drop to a keeper in Reaper', not slider count.
- **Skeptic.** research/04 line 35 has the 30 s rolling buffer and the master plan omits it; Interpretation fields exist in core. The rest (what the founder would or would not do) is speculation; no favourites list is planned, but Interpretations are persisted per project.
- **Corrected claim.** The rolling buffer from research/04 was dropped and no favourites/starring UI is planned, though core already persists named Interpretations; retention drivers are conjecture.

### Not anticipated

- Perceptual-centre pre-roll and timing feel move hits earlier than the take. Exported stems and MIDI need a lead-in or a stated start offset, otherwise Reaper clips or misplaces the first hit.
- Hearing one hit solo (click a marker, hear its render beside the original mouth sound) is the cheapest way to judge classification and mangle together; the plan has no audition-one-hit action.
- The founder's own 'rein it in' verdict means sensitivity is a per-take, per-room setting. A level floor derived from the take's noise floor, shown as a draggable line on the waveform, will be touched more than any slider.
- Judging sounds with the dry original voice playing alongside will bias the founder toward the 'Mouth amount' end. The gate should include a voice-muted comparison, or the AI-synthesised first listening test will be rated against a different reference than the later app.
- A pianist will want to audition a palette by playing it on a keyboard. MIDI-in to audition a palette's voices would cost little through the render contract, and it is not a DAW feature.

### Keep

- Offline look-ahead analysis with parameter-free evidence and a lens, so that sliders are millisecond re-renders: this is the real advantage over live tools.
- No looper, layers, piano roll or in-app effects: the Reaper division of labour is right once the in-context audition gap is closed.
- Time-anchored edits that survive re-derivation, and seeds keyed so one reroll never disturbs another, are exactly what a fix-one-hit workflow needs.
- The sub-second render goal and the 8 s for 10 minutes analysis cost keep the iterate loop viable.

### Missed by the reviewer, added by the skeptic

- **A/B and undo semantics are undefined** (minor). A/B is listed in the app but nothing defines what A and B hold (full Interpretation snapshot vs sliders), and how hit edits interact with dice rolls or undo. Without this, nudge-and-reroll can lose a good roll. Evidence: MASTER-PLAN line 164 and line 112 list A/B with no spec; core-types §2.12 Interpretation exists but no app-level history. Recommendation: Specify A/B as two Interpretation snapshots with one-step revert of the last dice roll.
- **Export is a single WAV, not per-role stems** (major). The plan exports one WAV plus MIDI. A drum fill rendered as one mixed file cannot be level- or EQ-balanced per voice in Reaper, though the stated goal is that the MIDI triggers a drum rack and the DAW does the polish. Evidence: MASTER-PLAN line 164 'export WAV + MPE'; §2 effects boundary puts EQ and compression in the DAW; no stem export mentioned. Recommendation: Add optional per-voice stem export to the same folder; voices are already separate per palette.
- **No latency compensation or take-to-grid alignment for in-app capture** (minor). Input latency on cpal capture shifts hits relative to a click and to export tempo, and the plan's 200 Hz control rate and sample-index timing say nothing of recording offset. Beatbox feel depends on tens of milliseconds. Evidence: MASTER-PLAN line 164 record from cpal; no calibration step, unlike research/04 line 35 which had a calibration. Recommendation: Add a loopback or tap calibration offset, or make dropped WAV the timing-trusted path.

## Plan realism, dependencies, licences

The plan's riskiest assumption (risk 1: renders sound like a mouth) is still untested 3 weeks in. Meanwhile about 20k lines of infrastructure exist and the week-4 gate falls due around 17 November with no audio. The plan was never re-baselined after Week 0 was skipped. Dependency and licence health is better than the plan feared: VST3 is MIT, the nice-plug stack is current, and Plaits is MIT and live. The real external risks are social and procedural: Codeberg-hosted deps with an anti-AI contribution policy, a few register gaps, operating rules living only in private memory, and no distribution or signing story. Name and signing are minor.

### 1. Existential test skipped, plan not re-baselined

*critical, contradiction, reviewer confidence high; skeptic: **partly***

- **Claim.** MASTER-PLAN §5 put a 2-day Week 0 listening check first, with 'if none of it excites, stop'. Three weeks (2026-10-20 to 11-10) produced core, evidence, cli and the render contract instead, and no sound. The week-4 gate (~11-17) cannot be met, and §5 and the kickoff brief still read as current. The gate has no new date or written kill criterion.
- **Evidence.** docs/MASTER-PLAN.md §5 Week 0 and Weeks 3-4; docs/STATUS.md Next item 1; crates/render has only contract/testkit
- **Recommendation.** Add a dated v1.3 note: Week 0 is now the AI-synthesised sketch. Set a hard date by which the founder hears it, and state the stop/pivot rule. Freeze new core/render abstraction work until then.
- **Skeptic.** Checked MASTER-PLAN §5, STATUS Next 1, DECISIONS 2026-11-09. Week 0 is not skipped: precheck.py was built 11-09 and awaits the founder's listening. The week-4 gate is unreachable (no backend, drums or lens) and §5 has no new dates, but the sketch is MIDI through the founder's synths, not AI-synthesised.
- **Corrected claim.** The Week 0 pre-check tooling exists (MIDI for the founder's own synths) but the founder's listening is pending and unscheduled; the week-4 gate (~11-17) is unreachable and §5 carries no re-baselined dates or stop date.

### 2. The Python sketch may not de-risk the Rust engine

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** The new Week 0 sketch is Python, but the plan's sound is Plaits engines plus a per-voice mangle chain in Rust. A good sketch built from different primitives proves the sound designer, not the planned architecture. A bad one is ambiguous: it could be a sketch limit or a plan limit. The plan's sound premise (Plaits macros plus mangle reaching SOPHIE or Death Grips territory) has no listening evidence at all.
- **Evidence.** docs/MASTER-PLAN.md §4.5; docs/DECISIONS.md Week 0 entry (precheck writes MIDI only); research/week0/precheck.py
- **Recommendation.** Build the sketch only from primitives that exist in the Rust plan (Plaits-equivalent engines, fold, crush, pitch envelope), or write down explicitly what a pass or fail will and will not license. Judge blind, as the week-4 gate specifies.
- **Skeptic.** The premise is wrong: DECISIONS 2026-11-09 says the pre-check emits MIDI played through the founder's synths, not a Python synth. The underlying point holds, since a pass proves hit extraction and the founder's patches, not Plaits plus mangle. The week-4 gate remains the real test of that.
- **Corrected claim.** The Week 0 pre-check drives the founder's own synths, so it tests the performance-to-hits pipeline and the idea, not the planned Plaits plus per-voice mangle engine; the plan should say what a pass or fail licenses.

### 3. Plugin-stack risk is upstream hostility, not licensing

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** Verified: nice-plug 0.4.2 (ISC, 2026-09-14), vst3 0.3.0 (MIT/Apache, last release Dec 2025), egui-baseview 0.9.0 (2026-10-17). VST3 is MIT since 3.8, so the old GPL-bindings worry is stale. But the stack lives on Codeberg, and the nice-plug README applies the RustAudio AI policy. Codeberg's July 2026 terms also restrict mostly-LLM repositories. Risk 8's mitigation, 'founder-reviewed patch forks', likely means permanent forks.
- **Evidence.** crates.io API nice-plug and vst3; https://codeberg.org/BillyDM/nice-plug; https://www.omgubuntu.co.uk/2026/07/codeberg-bans-ai-generated-code; MASTER-PLAN §6 risk 8, §4.4
- **Recommendation.** Read the RustAudio AI policy before filing any upstream patch. Budget for vendoring or forking in the v0.2 estimate. Re-label risk 7 and 8 around Codeberg availability and AI contribution policy, and run the smoke spike before committing to v0.2's shape.
- **Skeptic.** MASTER-PLAN line 41 already chose GitHub because Codeberg's 2026 terms reject mostly-LLM repos, so the hosting risk is known. Risk 8's wording is vague about forks. I could not verify the version and policy details without web access; the licence point is plausible.
- **Corrected claim.** The plan already records the Codeberg/LLM constraint for Takedown's own hosting, but risks 7 and 8 do not address upstream AI-contribution policy or the cost of carrying forks of Codeberg-hosted dependencies.

### 4. Licence register is accurate today but has gaps and a contradiction

*major, contradiction, reviewer confidence medium; skeptic: **partly***

- **Claim.** No row exists for cpal, eframe/egui, rubato, symphonia (MPL-2.0, file-level copyleft), midir, rtrb or egui's bundled fonts, all planned in §4.4. Neither does the ASIO SDK, which is GPLv3 or proprietary. §7 says the training-data rule is pragmatic, but §3.12 and the §8 kickoff still say 'no non-commercial weights'. The Vocadito note says mirdata lists BY-NC-SA, but mirdata and the paper now say CC BY 4.0. Authorship of AI-written code is not recorded.
- **Evidence.** LICENSES.md; docs/MASTER-PLAN.md §3.12, §7, §8; https://arxiv.org/pdf/2110.05580; https://mirdata.readthedocs.io/en/stable/_modules/mirdata/datasets/vocadito.html
- **Recommendation.** Reconcile §3.12/§8 with §7. Add rows when deps are added, or adopt cargo-deny/cargo-about in CI. Fix the Vocadito note. Add a short third-party NOTICE step before any binary is shared. Plaits and its MIT notice are fine.
- **Skeptic.** Contradiction is real: §3.12 and §8 say no non-commercial weights while §7 and the register allow pretrained checkpoints. Missing rows are not violations: no crate yet depends on cpal, eframe, rubato, symphonia or midir, and the register says rows come before use. Vocadito and AI-authorship claims unverified.
- **Corrected claim.** §3.12 and §8 contradict §7 on non-commercial weights; rows for planned-but-unused dependencies are not yet required, though a cargo-deny or NOTICE step is advisable before distribution.

### 5. Risk register §6 is stale and misses real risks

*major, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Risk 11 ('two-person bandwidth') now understates the real bottleneck: founder listening and recording time. The golden set is still unrecorded, so gate E1 is unmeasurable, and AVP numbers are tuned. Missing risks: Plaits sounds like Plaits rather than SOPHIE; macOS microphone permission (TCC) for a bundled app; dependence on AI sessions and the founder's private memory; distribution and signing; sub-second re-render of a 3-minute take is unmeasured; AI-written code's copyright weakening copyleft enforcement.
- **Evidence.** docs/MASTER-PLAN.md §6; docs/STATUS.md Numbers caveat and Next 2; docs/RECORDING-PROTOCOL.md
- **Recommendation.** Rewrite §6 with dated status per risk. Add the missing items above, each with its cheapest test. Add an explicit 'founder hours needed this week' line to STATUS.
- **Skeptic.** §6 has 11 undated risks and risk 11 says only 'two-person bandwidth'. STATUS Next 2 confirms the golden set is unrecorded, so E1 is unmeasured. Nothing in §6 covers macOS microphone permission, signing, AI-code copyright, or re-render latency. Risk 7's smoke spike also does not exist.

### 6. A fresh session cannot get the operating rules from the repo

*minor, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** Rules that steer behaviour live only in the founder's private memory: unattended-session protocol, evaluate by listening, stay-current-on-versions. STATUS.md heads 2026-10-21 and does not record the 11-10 decision on AI-synthesised sounds. The plan names `spikes/plugin-smoke/`, which does not exist, and untracked fill.*.csv sit in the repo root. The tiny docs/RECORDING-PROTOCOL.md and core-types.md (1,217 lines) are the only deep specs.
- **Evidence.** CLAUDE.md (memory listing in the session context, outside the repo); docs/STATUS.md header and Next; ls repo root; MASTER-PLAN §4.3
- **Recommendation.** Copy the three memory rules into CLAUDE.md or DECISIONS.md. Add the 11-10 decision and the current week-0 state to STATUS. Gitignore scratch CSVs. Otherwise the docs are good enough for a fresh session to continue.
- **Skeptic.** CLAUDE.md does not contain the unattended protocol or the evaluate-by-listening rule; grep finds 'unattended' only in a STATUS header. Evaluate-by-listening is partly recorded in DECISIONS 11-09. spikes/ is absent and the fill CSVs are untracked and not gitignored. The 11-10 decision is not in the repo, which I cannot confirm.
- **Corrected claim.** The unattended-session protocol and the stay-current rule live only outside the repo (the listening rule is partly in DECISIONS); STATUS is not updated past 11-09; the scratch CSVs are not gitignored.

### 7. Signing and distribution are deferred, but v0.1 sharing hits them

*minor, missing, reviewer confidence low; skeptic: **unverifiable***

- **Claim.** Notarisation is placed at v0.3+, but any macOS app or plugin handed to another person is quarantined and blocked unnotarised. Apple Developer Program is $99/yr (not re-verified here). Windows: Azure Artifact Signing is about $10/month, but public individual validation is limited to US and Canada, and organisations need history. SmartScreen warnings otherwise. Irrelevant for the founder's own machine; relevant at first outside tester.
- **Evidence.** https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/; learn.microsoft.com Q&A on individual validation; MASTER-PLAN §5 v0.3
- **Recommendation.** Decide before first outside tester: source-only release plus build instructions, or pay for Apple and one Windows route (SignPath for OSS is unverified). Record it in DECISIONS.
- **Skeptic.** Notarisation is listed under v0.3 in §5 and v0.1 is a standalone app, so the gap is real in the plan. The Apple and Azure prices and eligibility were not rechecked; I had no web access.

### 8. The name 'Takedown' is a search and connotation liability

*idea, risk, reviewer confidence low; skeptic: **unverifiable***

- **Claim.** 'Takedown' is the DMCA term for content removal. For a tool that also promises on-palette sounds, that invites jokes and noisy search results. A search found a drum transcription of 'Takedown' from KPop Demon Hunters. I found no audio product of that name, but no trademark search was done. Renaming is cheapest now: the crates are internal and nothing is published.
- **Evidence.** https://www.sheetmusicplus.com/en/product/takedown-23831821.html; Cargo.toml repository LTeder/Takedown; WebSearch for audio products (no match)
- **Recommendation.** Before first public release, run a USPTO/EUIPO class 9 and 41 check and a GitHub/crates.io search. Pick a name or keep this one deliberately.
- **Skeptic.** No trademark or product search was possible here. 'Takedown' as the DMCA term is common knowledge, but the severity is a judgement call, and the reviewer admits finding no audio product of that name.

### Not anticipated

- Codeberg's mostly-LLM-repository rule and the RustAudio AI policy mean your GitHub choice is right, but every dependency you may need to patch (nice-plug, egui-baseview) sits where AI-authored patches are unwelcome.
- Because most of the code is AI-written, its copyright is legally uncertain in several jurisdictions. That weakens GPL enforcement and makes contributor licensing, DCO and a clear human-authorship statement worth deciding early.
- A bundled macOS app that records the mic needs an Info.plist microphone usage string and TCC consent. A terminal-run CLI inherits the terminal's permission, so v0.1's first mic test can pass and still fail as a double-clicked app.
- Founder ears are the scarce, serial resource: every gate (week 0, golden set, week 4, dice usability) needs them. Schedule listening sessions as a calendar item, not a background task.
- The 'sounds like it came from Serum' goal is partly preset craft, which the plan assigns to the founder with Claude. Today's decision shifts that craft to the AI, so the first real test is whether the AI can design on-palette sounds at all.

### Keep

- The licence discipline (Surge as uncopied reference, register from commit one, dataset provenance with MD5/SHA-256 pinning) is sound and ahead of most projects.
- Choosing GPL-3.0-or-later with MIT/Apache/ISC dependencies is coherent, and VST3 going MIT in 3.8 removes the old plugin-licence friction.
- Evaluating by listening with conservative evidence and eager lens, and recording the AVP tuning caveat in STATUS.md, is honest and should continue.
- The decision log plus STATUS.md with reproducible numbers is a strong handoff base. The master-plan-wins rule is clear.

### Missed by the reviewer, added by the skeptic

- **Plaits dependency is a single-maintainer git pin** (major). The sound engine depends on a git-pinned, non-crates.io port (mi-plaits-dsp-rs) by one outside maintainer. A force-push or deletion breaks the build, and publishing to crates.io needs every dependency to be on crates.io. The risk register has no row for it and no vendoring plan. Evidence: LICENSES.md says 'Not on crates.io; pin to a commit'; crates.io API returns 'crate mi-plaits-dsp does not exist'; MASTER-PLAN §4.4 and CLAUDE.md pin it to a git commit. Recommendation: Add a risk row and vendor a copy under the licence's terms (MIT) before the render backend depends on it; record the commit and a mirror.
- **Weeks 3-4 gate depends on unbuilt crates** (major). The week-4 gate needs drums, the lens, a backend and Plaits. STATUS Next lists drums (5) and lens plus first backend (6) as not started, and the plan's 7-week v0.1 estimate has not been re-costed after core and render consumed the first three weeks. Evidence: docs/STATUS.md Next items 5 and 6; MASTER-PLAN §5 Weeks 3-4 and 5-7; render holds only contract and testkit. Recommendation: Re-estimate the remaining path to the first audible render and put a date on it in §5.
- **Founder time is the critical path and is unplanned** (minor). Week 0 listening, the golden-set recording (two mics, two rooms), teach takes and palette tuning all need founder hours. None is scheduled, and the pre-check roles accuracy (about 73%) falls well short of E1's 85% target. Evidence: docs/STATUS.md Next 1-2; docs/RECORDING-PROTOCOL.md; DECISIONS 2026-11-09 (73% with teach takes versus 85% gate). Recommendation: Add a 'founder hours needed' line to STATUS and a dated recording session.

## Blind spots nobody has anticipated

The largest blind spot is the sound loop itself. The AI writing the synthesis cannot hear it, so every sound-design iteration costs the founder's ear time, and no tooling anticipates that. Second, the false positives the founder flagged (noise, chaotic stretches, hums under hits) are the input the palette prizes (lip rolls, breath, whooshes), so reining them in may tune the product away from its target. Third, the whole pipeline assumes one clean, well-gained, monophonic mouth with no capture-chain checks. Fourth, grid and key are absolute while the product lives inside a DAW project. Three weeks and 13 commits have produced no sound. Experiments below fit in a week.

### 1. The AI sound designer cannot hear its own output

*critical, missing, reviewer confidence high; skeptic: **partly***

- **Claim.** The founder's 2026-11-10 decision makes AI-synthesised sound the first test, but the AI judges only by numbers. Each tweak needs the founder to listen, so the loop is limited by the founder's time and attention. Nothing in the plan provides batching, blind ranking or automated sanity checks (clipping, DC, aliasing, level).
- **Evidence.** Task brief (founder quote); docs/STATUS.md Next 1 puts every judgement on the founder's ear; memory evaluate-by-listening.md
- **Recommendation.** This week: one script renders 24 labelled, seeded variants of one kick and one snare into a single WAV with spoken or beeped index tones, plus a printed table of peak, DC, crest factor and energy above 16 kHz. The founder ranks them in five minutes and the picks are logged as the first taste data. Refuse variants that clip or alias.
- **Skeptic.** Core logic is sound, but the 2026-11-10 decision is not in docs (grep of docs finds no such entry), so it rests on the task brief only. STATUS Next 6 and the week-4 gate do make the founder's ear the judge. The batched-variants idea is reasonable but 'critical' is overstated.
- **Corrected claim.** Sound judgement depends entirely on the founder's ear and time; no batched blind-ranking or automated sanity checks (clip, DC, alias, level) are planned. The cited 11-10 decision is not recorded in the repo.

### 2. The false positives are the palette's raw material

*major, contradiction, reviewer confidence medium; skeptic: **partly***

- **Claim.** The founder's false-positive complaint concerns noise, chaotic stretches and hums mixed with hits. Breath whooshes, lip rolls and growls are exactly the texture input the product promises. Filtering by rise and strength would delete them. The 14 s noise passage should become a texture voice, not 90 rejected onsets. Evidence has no continuous-noise or sustained-texture class.
- **Evidence.** docs/STATUS.md Numbers (90 onsets in 14 s of noise; Next 5 proposes rise/strength floors); MASTER-PLAN §1 ('breathe a whoosh'), §4.5 NoiseTexture
- **Recommendation.** Before tuning thresholds, classify each stretch as hit, tonal or noise/texture (zero-crossing rate, loudness variance, onset density) and route noise to NoiseTexture. Test: record 10 s each of lip roll, breath whoosh and kick-plus-hum, then play the click stem and the noise segmentation against each in listen.py.
- **Skeptic.** STATUS line 49 does show 90 onsets in 14 s of noise and Next 5 proposes rise/strength floors. But MASTER-PLAN already makes NoiseTexture a voice driven by loudness and breath, and the founder called the noise detections false positives. Missing noise/texture class routing is real; 'contradiction' is overstated.
- **Corrected claim.** Evidence has no sustained-noise class, so noise passages are treated as rejected onsets rather than routed to the NoiseTexture voice, which the plan already defines as a voice.

### 3. Fixing the 59 Hz fan tone can break throat bass and voiced kicks

*major, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** The obvious fix for the fan tone (raise the range floor, high-pass, gate by relative loudness) also removes sub-bass vocalisations: throat bass, voiced kicks, pitched 'oom'. The default prior is 55 to 1100 Hz and tests are Vocadito singing and AVP, which hold none of these. Pitch was verified only on hums.
- **Evidence.** docs/DECISIONS.md 2026-10-21 (range prior 55 to 1100 Hz); STATUS.md voicing on a room tone; research/README.md datasets
- **Recommendation.** Record 20 s of throat bass, voiced kick and fan alone. Check that the voicing fix separates them using CPP, harmonic structure and loudness modulation, not frequency. Do this before the adaptive-threshold work.
- **Skeptic.** STATUS Next 3 proposes loudness-relative and CPP fusion for voicing, so a frequency-floor fix is not what is planned; the risk is real only for the loudness gate. Sub-bass vocalisations are untested: protocol items cover hums, sung vowels, hits, not throat bass. Speculative but plausible.
- **Corrected claim.** The planned voicing fix (relative loudness plus CPP) could suppress quiet low voiced sounds such as throat bass; the golden-set protocol has no item covering them.

### 4. No capture-chain checks: clipping, mic distance, bleed, built-in mic processing

*major, missing, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Plosives overload laptop mics. Clipping flattens the peak that drives velocity and loudness ranking. Brightness and tilt shift with distance and proximity effect, which breaks per-user kNN taught at one distance. Click or headphone bleed adds false onsets. The built-in macOS mic may apply voice processing that shaped the founder's noise results. The code detects none of this.
- **Evidence.** grep of crates/evidence for clip, DC or high-pass finds nothing; crates/cli/src/wav.rs averages channels with no level check; RECORDING-PROTOCOL.md plays a click during takes; MASTER-PLAN §6 R10 covers only two mics and two rooms
- **Recommendation.** Today: count samples at or above 0.99 and the peak of the existing 179 s take. This week: record 10 kicks at 3 distances and 2 angles, teach on one setting and classify the others. Add a clip and level warning to analyze and a recording-health line to the app.
- **Skeptic.** Grep of crates/evidence and crates/cli/src/wav.rs finds no clip, DC or level handling; wav.rs only downmixes. RECORDING-PROTOCOL covers two mics and fan, but not distance, plosives or bleed from the click. Plan has per-take percentile normalisation, which hides level problems.

### 5. Tempo, grid and key are absolute; the product lives in a DAW project

*major, missing, reviewer confidence medium; skeptic: **partly***

- **Claim.** Exports are fixed at 120 BPM absolute time, with the founder expected to match the Reaper tempo by hand. Beatbox is rhythmic, but there is no tempo or groove detection until v0.2, and a MIDI item aligned in beats drifts against audio in any other project tempo. Pitched drums (808 kick, tonal snare) have no key reference. A SOPHIE-style kick out of key with the bass is the first thing a producer hears.
- **Evidence.** research/week0/precheck.py docstring (120 BPM, 960 PPQ); DECISIONS.md 2026-10-21 assumption 11; MASTER-PLAN §5 v0.2 (notes lens, HostAnchor, swing)
- **Recommendation.** Test: record a fill against a 90 BPM click, import audio and MIDI into a 90 BPM Reaper project, and measure drift. Spec a 'tempo and key' field on Take (typed in by the founder) now, and let the kick body pitch snap to it.
- **Skeptic.** DECISIONS assumption 11 confirms 120 BPM/960 PPQ export with the founder matching tempo by hand until v0.2 HostAnchor. But the protocol already has 90 and 120 BPM click grooves incl. swung, and the plan has HostAnchor and swing in v0.2. Key snapping is genuinely unspecified.
- **Corrected claim.** Export is fixed at 120 BPM with manual tempo matching until v0.2; tempo handling is planned, while key/pitch tuning of drums to the song has no spec.

### 6. Three weeks, 13 commits and zero sound: the loop has no deadline

*major, risk, reviewer confidence high; skeptic: **partly***

- **Claim.** The plan budgeted Week 0 at 2 days. Three weeks later nothing is audible. Commits fall on three days (Oct 20, Oct 21, Nov 9), and the founder's recording has been the blocker since Oct 21. About 8,000 lines of src, a 1,200-line spec and 164 tests protect an engine that does not exist. Momentum, not code, is the main risk.
- **Evidence.** git log dates; STATUS.md Next 1 and 2 ('founder's part remains'); MASTER-PLAN §5 Week 0
- **Recommendation.** Adopt a rule: no new crate, spec or evidence tuning until a WAV of AI-synthesised sound driven by fill.onsets.csv exists and has been heard. The founder's one decision this week: keep going or stop on that WAV. Keep the Python sketch disposable.
- **Skeptic.** git log confirms 13 commits on Oct 20, Oct 21 and Nov 9 only, and plan Week 0 is two days. But research/week0 now yields drum MIDI and MPE from the take, so not literally zero output. Also 'about 8,000 lines of src' is wrong: crates src is about 10,000 lines (20,857 incl. tests).
- **Corrected claim.** 13 commits on three days across three weeks, with no synthesised audio yet, only MIDI exports and listening stems; about 10k lines of crate src and 164 tests precede the engine.

### 7. Review by the same kind of author compounds unchecked assumptions

*minor, risk, reviewer confidence medium; skeptic: **partly***

- **Claim.** The founder cannot review 20,000 lines in depth. Review so far is by AI panels, and 33 assumptions sit in one entry that the founder approved as a block. Some of them bind sound design, for example monophonic spans, mono voices and role-routed hits. A bad one will surface only after the render layer is built on it.
- **Evidence.** DECISIONS.md 2026-10-21 'Core types' entry (items 4, 8, 26); STATUS.md 'Two independent reviews'; CLAUDE.md
- **Recommendation.** Have the founder read only the 8 or so assumptions that constrain sound (4, 7, 8, 9, 14, 26, 27, 28) and veto or defer by number. Add a 'revisit when first sound exists' tag to the rest.
- **Skeptic.** DECISIONS 2026-10-21 lists 33 assumptions in one entry; items 4, 8 and 26 (monophonic gate, role-routed hits, mono voices) are as cited. STATUS says the founder was pointed to 'the ones most worth a look', so approval was not entirely blind. Self-review bias is plausible but unevidenced.
- **Corrected claim.** 33 assumptions sit in one entry; STATUS highlights a few for the founder, but several that constrain sound design are not individually tagged for revisit.

### 8. Heavy mangle makes output level, aliasing and export format open questions

*minor, missing, reviewer confidence low; skeptic: **confirmed***

- **Claim.** Drive, fold and crush push peaks over 0 dBFS and alias. The plan applies per-take percentile normalisation to inputs but has no output gain staging or export format decision. Aliasing from crush may be wanted, but accidental aliasing from fold and clip is not, and only the ladder filter is oversampled. The DAW's compressor and OTT assume sane levels.
- **Evidence.** MASTER-PLAN §4.5 (2x oversampling for the ladder only; mangle chain), §3 principle 11; no export format decision in DECISIONS.md
- **Recommendation.** Decide on 32-bit float WAV export with a peak-normalise option, and measure alias energy above 16 kHz in the sound sketch before choosing oversampling per mangle stage.
- **Skeptic.** MASTER-PLAN line 125 and 127 show percentile normalisation of inputs and ladder at 2x oversampling only; grep of docs finds no output gain staging or export format decision. Low confidence fits: this is a downstream concern, not yet blocking.

### Not anticipated

- Headphone monitoring while recording: whatever plays the click or a previous render bleeds into the mic and becomes false onsets and false ground truth. Recording on open-back headphones or speakers will worsen it.
- Beatbox technique has inward sounds, clicks and simultaneous sounds that AVP lacks. Inward snares and lip buzzes may have no standard onset signature, and detection tuned on AVP says nothing about them.
- Stereo or interface input: a mic on one channel of a stereo file is averaged to half level, and out-of-phase channels would cancel. Both cases pass the current code silently.
- The dice decouple sound from performance: rerolled bodies may drift from what the founder meant, which turns 'my fill, designed' into 'random drums on my rhythm'. The Week-4 gate should include a fixed-seed repeatability and 'does it still feel like my performance' check.
- The founder is both the only golden-set source and the only judge; a new mic, a bad day or another performer may break everything. Even one friend's 3-minute take would show how personal the tuning is.

### Keep

- Keep the evaluate-by-listening practice and the conservative-evaluation versus eager-lens split; it caught a problem that metrics missed.
- Keep the founder's 2026-11-10 decision to ship AI-made sounds in the first test; it targets the right existential risk.
- Keep the Week-0 and Week-4 kill gates, and treat them as binding.
- Keep the property-test discipline on the contract (block invariance, seed tree); it is cheap insurance once sound exists.

### Missed by the reviewer, added by the skeptic

- **Per-take percentile normalisation hides level and noise-floor problems** (major). Loudness, brightness and breathiness are rescaled p5-p95 per take, so a take that is mostly silence, fan noise or clipped gets stretched to full range, amplifying the room tone into expressive control. The 59 Hz fan tone and noise-floor onsets may be partly an artefact of this. Evidence: MASTER-PLAN line 125 (per-take percentile normalisation p5-p95); STATUS line 50 (voiced frames at -71 dB median loudness). Recommendation: Add an absolute-level floor before normalisation and test a take that is 80 percent room tone.
- **Founder's own beatbox is the only training and evaluation voice** (minor). Per-user kNN, thresholds and listening judgements all come from one person, one laptop mic and one room. Generalisation is untestable and the Week-4 gate is ten fills from the same source; a regression in a different room or mic would go unnoticed. Evidence: RECORDING-PROTOCOL.md (founder's CC0 recordings, two mics, two rooms); STATUS line 47 (single 179 s take); MASTER-PLAN Week-4 gate. Recommendation: Record the same 10 fills on a second mic and room this week and rerun onsets to see the delta.
- **Latency and sync of a click-led take are never measured** (minor). The protocol plays a click during takes, but no step measures input and output latency between click and mic, so onset times against the click, and the MIDI aligned in Reaper, carry an unknown fixed offset of tens of ms. Hit timing feel and perceptual-centre pre-roll edits (assumption 28) assume exact times. Evidence: RECORDING-PROTOCOL.md item 5 (click on separate track); DECISIONS 2026-10-21 assumption 28 (25 ms timing and pre-roll); no latency calibration found in docs. Recommendation: Record a loopback click through the same interface and measure the offset before annotating the golden set.

## Documentation coherence

The Rust code mostly matches what the docs say about it. I checked 17 claims: crate list, test counts, the evidence and onset constants, the scoreboard numbers, the CI shape, the pinned toolchain and the render hook names. All held except small drifts. The trouble is direction. The authoritative MASTER-PLAN still describes the Week 0 premise (the founder's own synths) that the founder reversed on 2026-11-10. That reversal is in no document. core-types.md, which CLAUDE.md says to read before editing, has several stale signatures and a stale invariant. The plugin smoke spike is promised in four places and missing. About 1,340 lines of superseded research plus a plan kickoff brief carry no current information.

### 1. The 2026-11-10 'AI synthesises the sounds' decision is unrecorded; four documents still say the founder's synths

*major, contradiction, reviewer confidence high; skeptic: **unverifiable***

- **Claim.** DECISIONS 2026-11-09 chooses the founder's own Reaper synths for Week 0. STATUS Next #1, research/README 'Listening, in Reaper' and MASTER-PLAN §5 Week 0 and §6 risk 1 all assume it. The founder reversed this today, and nothing supersedes it. The week-4 gate ('a sound they could not have made another way') and the 'Assemble: Plaits' engine decision are also now open.
- **Evidence.** docs/DECISIONS.md last entry (2026-11-09, 'plays the founder's own synths'); docs/STATUS.md Next 1; research/README.md 'Week 0 pre-check'; docs/MASTER-PLAN.md lines 147-149, 185
- **Recommendation.** Append a dated DECISIONS entry that says it supersedes the 11-09 one and states what it changes in the Week 0 stop rule, the week-4 gate wording and the engine sourcing. Then rewrite STATUS Next #1 and the §5 Week 0 paragraph.
- **Skeptic.** Repo half holds: last DECISIONS entry is 2026-11-09 (founder's own synths) and STATUS/plan Week 0 text matches. Last commit is 2026-11-09, so no repo evidence of a reversal today exists. The reversal itself can't be checked here.

### 2. core-types.md departs from the code in at least four places and is not patched

*major, contradiction, reviewer confidence high; skeptic: **confirmed***

- **Claim.** The doc says all Evidence values are finite; the code allows NaN in four columns. It gives trigger_seeds a Lane argument; the code takes TriggerGroup. It gives alloc_id -> u32; the code returns Result<u32, ProjectError>. It places quantise and the MPE lattice in core; they are absent until the midi step. DECISIONS records each change, but the spec's header says to follow it 'in one pass', and its §7 deviations list lacks them.
- **Evidence.** docs/design/core-types.md:265, 201, 698, §2.9; crates/core/src/evidence.rs:9-15; crates/core/src/seed.rs:380; crates/core/src/project.rs:131; crates/core/src/canonical.rs:1-3
- **Recommendation.** Patch the four sections in place, or add a 'Departures since 2026-10-21' block at the top. CLAUDE.md sends sessions here before they change core or render, so a stale signature costs the most here.
- **Skeptic.** evidence.rs header says NaN allowed in four columns, a deviation from §2.5; core-types.md:265 still says all finite. DECISIONS 10-21 items 1 and 5 record trigger_seeds taking TriggerGroup and alloc_id returning Result. Spec header still says follow in one pass. I did not check §7 contents.

### 3. The plugin smoke spike is promised four times and does not exist

*minor, missing, reviewer confidence high; skeptic: **confirmed***

- **Claim.** MASTER-PLAN lists spikes/plugin-smoke in §4.3, schedules it for weeks 1-2, makes it the only cheap test for risk 7, and repeats it in the §8 kickoff brief. There is no spikes/ directory and STATUS does not mention it. The v0.2 plugin shape (nice-plug, egui-baseview, drag-out) is therefore untested, and nothing says it was dropped.
- **Evidence.** docs/MASTER-PLAN.md lines 62, 150, 191, 212; ls spikes -> no such directory; docs/STATUS.md Next has no spike
- **Recommendation.** Either schedule it in STATUS Next, or log a decision to defer it to v0.2 and strike it from §4.3, §5 and §8.
- **Skeptic.** MASTER-PLAN lines 114, 155, 191 and 212 mention it; `ls spikes` fails; grep of STATUS finds no spike. Line 62 was not checked, so the count of four is approximate.

### 4. Gate E1 is described as passing on proxies while its target criteria are unmet or unmeasurable

*major, contradiction, reviewer confidence medium; skeptic: **partly***

- **Claim.** The plan puts E1 (golden-set onset F1 ≥ 0.9, 4-class hit accuracy ≥ 85% at 10 shots) before render work. STATUS reports 'passes on clean Vocadito' and 'on the line' for onsets, which were tuned on AVP. The golden set does not exist. The only role number is about 73%, from a throwaway Python script, not the drums crate. STATUS then queues drums at #5 and lens/backend at #6. The slip from 'record the golden set the same week as Week 0' has no decision.
- **Evidence.** docs/MASTER-PLAN.md:154 (E1); docs/STATUS.md Numbers and Next 2, 5, 6; docs/DECISIONS.md 11-09 pre-check entry (73.4% / 53.8%)
- **Recommendation.** State in STATUS that E1 is unmeasured, not passed. Record whether sound work now deliberately runs ahead of E1, and what E1 must still pass before the week-4 gate counts.
- **Skeptic.** STATUS lines 30 and 43 do say the golden set doesn't exist and numbers are proxies (line 64 'every number above is a proxy'). So it isn't claimed as passed. The missing recorded decision to run sound work ahead of E1 is plausible but not checked.
- **Corrected claim.** STATUS hedges E1 as proxy-only but wording like 'passes on clean Vocadito' could be read as a pass; no decision records proceeding to sound work before the golden set exists.

### 5. About 1,340 lines of superseded research and the plan's §8 brief can go with no loss

*minor, overbuilt, reviewer confidence high; skeptic: **partly***

- **Claim.** Research 00-04 are labelled superseded and describe another product: 'Voice-to-Synth Tool', names Murmur and Lilt, a note-centred concept. MASTER-PLAN §8 is a ~12-line kickoff brief that duplicates CLAUDE.md and already contains the stale Week 0 and spike lines. DECISIONS 2026-10-20 expected the plan to shrink into ARCHITECTURE and ROADMAP; that has not happened.
- **Evidence.** wc -l docs/research/00-04 = 172+259+333+294+280; docs/research/00-summary-and-plan.md:1-3, 123, 144; docs/MASTER-PLAN.md §8; docs/DECISIONS.md 2026-10-20 'Research docs...'
- **Recommendation.** Delete 00-04 (git keeps them) and MASTER-PLAN §8. Keep 05-08, which CLAUDE.md cites for detail. Do the plan split once the render step lands.
- **Skeptic.** Line counts are exact (1338). §8 exists at line 206 and overlaps CLAUDE.md. But CLAUDE.md says research docs may rot, and the plan header cites 01-04 as provenance. 'No loss' is a judgment call; git history keeps them anyway.
- **Corrected claim.** Research 00-04 (1,338 lines) and plan §8 are low-value, deletable with history preserved; provenance references in the plan header would need adjusting.

### 6. MASTER-PLAN is the document most likely to mislead a fresh session

*major, contradiction, reviewer confidence high; skeptic: **partly***

- **Claim.** CLAUDE.md says the plan wins every conflict, yet it is stale on Week 0, the spike and the Hit shape. §4.1 still lists 'exciter slice ref' on Hit and Hit{... exciter} in the ControlTrack line, but core::hit stores no slice (decision 7). §4.2 and §4.6 list vowel PCA and class hints that analyser v1 does not produce. A session obeying the plan over the code or DECISIONS would build the wrong thing.
- **Evidence.** docs/MASTER-PLAN.md lines 70, 74-75, §4.2 pipeline, §4.6; crates/core/src/hit.rs:109-122; docs/DECISIONS.md 2026-10-21 assumption 7 and 32
- **Recommendation.** Add a 'known drift' list to the top of the plan, or fix the lines. Say in CLAUDE.md that DECISIONS entries after the plan's date override it.
- **Skeptic.** Plan lines 70 and 75 do list 'exciter' on Hit, and the Week 0 and spike text is stale. But the plan header line 3 already says decisions after 2026-10-20 go in DECISIONS.md, and CLAUDE.md points to DECISIONS. I did not open hit.rs to confirm the Hit shape.
- **Corrected claim.** The plan is stale on Hit exciter, Week 0 and the spike. Its header already points to DECISIONS for later calls, but CLAUDE.md's 'plan wins' rule doesn't say later DECISIONS override it.

### 7. Small rule and count slips

*minor, bug, reviewer confidence medium; skeptic: **confirmed***

- **Claim.** Two untracked CSVs, fill.frames.csv and fill.onsets.csv, sit at the repo root and are not gitignored; CLAUDE.md says evaluation output goes to research/out/. STATUS says 164 tests; I count 165 #[test] functions, with core at 102 not 101 (one is #[ignore]). 'All JSON writes go through project::json' has one in-memory exception, migrate.rs:171, which re-prints a migrated Value with serde_json::to_vec. It is commented and sound, but the rule reads as absolute.
- **Evidence.** git status; crates/core/src/migrate.rs:155-173; grep -rE '#\[test\]' crates; docs/STATUS.md Tests
- **Recommendation.** Delete or move the CSVs. Make the test count a command, not a hand-typed number. Add a one-line exception to the CLAUDE.md JSON rule.
- **Skeptic.** git status shows fill.*.csv untracked and .gitignore lacks them. STATUS says 164 (core 101); grep gives core 102 (one #[ignore]), 165 total. migrate.rs:171 uses serde_json::to_vec on a Value, an in-memory exception to the JSON rule.

### Not anticipated

- Moving the sound design to the AI makes the Python sketch the first sound-making code. It will either be ported to a Rust backend or discarded, and the plan's 'Assemble from Plaits' engine decision and the LICENSES 'planned' rows may no longer describe the real path. Record which.
- Each reversal needs an explicit 'supersedes' entry. The 2026-11-09 toolchain entry does this well and the Week 0 one does not. The unattended-session protocol (DECISIONS per assumption) is only as good as the rule that a reversal counts as a decision.
- Nothing checks the docs against the code, so counts and signatures drift by hand. A short script that regenerates the test count and lists pub signatures named in core-types.md would catch the drift this review found.
- research/README and STATUS tell the founder to record ten fills, hums and teach takes for a test whose premise has changed. Without an update, the founder may spend an evening on a protocol that is obsolete.

### Keep

- DECISIONS.md as an append-only log with numbered assumptions and rationale: it let me trace every code departure from core-types.
- STATUS.md's honest caveats (onset numbers tuned on AVP, golden set absent, the one failing curve named), which match the scoreboards and constants in the code.
- The licence register's habit of marking components as 'planned' versus used, and keeping Surge as reference only.
- CLAUDE.md's hard rules, which are short and mostly checkable. The property-test, no-alloc and JSON round-trip rules all have matching tests.

### Missed by the reviewer, added by the skeptic

- **CLAUDE.md 'plan wins' rule has no carve-out for later DECISIONS** (minor). CLAUDE.md says MASTER-PLAN is authoritative on any conflict, while the plan header says later decisions live in DECISIONS.md and core-types.md has its own deviation clause. Three precedence rules point different ways. Evidence: CLAUDE.md 'Read first'; docs/MASTER-PLAN.md:3; docs/design/core-types.md:3 Recommendation: State one precedence order in CLAUDE.md: DECISIONS entries after a doc's date override that doc.
