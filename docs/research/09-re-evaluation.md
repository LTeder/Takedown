# 09: Re-evaluation, November 2026

*2026-11-10. The founder asked for "a large-scale re-evaluation of this repo, its contents, design, intention, and ideas. Anticipate what I have not. Re-think from first principles when useful." Twelve Sonnet reviewers each took one lens (product thesis, first-principles redesign, architecture against v0.1, evidence DSP, sound engine, defects in core, defects elsewhere, evaluation, user experience, plan and licences, blind spots, documentation). A second agent per lens tried to refute each finding. Of 92 findings the skeptics confirmed 27 outright, corrected 60, could not check 5 and refuted none. Every finding, with its evidence and the skeptic's correction, is in [09-findings.md](09-findings.md). This page is Claude's synthesis: what to believe, what to do, and where the reviewers are wrong. Claims about other products come from the agents' web searches and were not re-checked here.*

## The verdict

The idea holds, the analysis is good enough, and the project built its foundation before proving its sound. All twelve lenses reached that conclusion independently. About 17,000 of the repository's 20,900 lines of Rust are core types, the render contract and their tests, and until today nothing in the repository made a sound. The one risk that can end the project ("renders sound like a mouth, not a machine") had no code path.

Nothing built is wrong enough to throw away. Most of `core` is simply ahead of anything that uses it. The next month should be spent on sound.

## How this happened

Two calls, both Claude's, are worth naming so they are not repeated:

- **Week 0 was skipped.** The plan's first step was a two-day listening check with a stop rule. The first build session went straight to the foundation, because a foundation is what an unattended session can verify with tests. The process rewarded plumbing.
- **Then the listening test was handed to the founder's own synths** (DECISIONS, 2026-11-09), which tests the founder's patches, not Takedown's sound. The founder reversed this on 2026-11-10: the sounds have to be Claude's.

The rule that follows: an unattended session should end in something the founder can listen to.

## Findings that change what we do

1. **Sound comes first, and it now exists as a sketch.** `research/week0/render.py` plays a take through three synthesised palettes and writes a listening page. The reviewers' landscape survey says detection (onsets, pitch, MIDI out) is what existing tools already sell, and designed sound driven by the mouth's gesture is what none of them do. Effort should follow that.
2. **The onset detector depends on input gain. This is a real bug.** A reviewer measured it on 30 AVP files: recall 0.93 at full level, 0.42 with the audio 20 dB quieter, 0.05 at 30 dB quieter. The onset function takes a logarithm of unnormalised magnitudes and the pick threshold is absolute. The founder's take was loud enough to hide it; a quieter recording loses half its hits. The tests use loud signals over near-silence, so they cannot see it.
3. **One primitive is missing: level above the take's own noise floor.** It explains the fan tone called voiced and the false onsets in quiet stretches, and every other mask should depend on it. It does not explain the noise passage, whose onsets are almost as loud as real hits.
4. **The noise passage is material, not error.** Breath, lip rolls and noise are sounds the palette wants. Evidence has no notion of sustained noise, so they arrive as a burst of drum hits. They should drive a texture voice.
5. **Evidence is less parameter-free than the rule says.** The stored voiced probability is already pushed through pYIN's fixed priors, so the lens cannot re-threshold noisy frames. Brightness in harmonic units divides by a pitch candidate that decoding may later overrule. Both should store the raw observation and leave the opinion to the lens.
6. **The performance has to be audible in the result.** If every kick sounds the same however it was performed, the product is a drum machine with a microphone. The sketch now renders a control (same hit times, each hit wearing another hit's features) so this can be heard directly. Loudness should drive distortion depth, not only level.
7. **The week-4 gate cannot say why it failed.** Sixty blind renders bundle wrong onsets, wrong roles, dull voices and inaudible mapping into one "no". It needs a control condition, a recorded reason for each rejection, and to be judged in a real session of the founder's.
8. **Roles are the weakest link and may be the wrong frame.** Kick, snare and hat is a taxonomy borrowed from drum machines. Finding the kick by low-band punch works (92% of AVP's kicks); snare against hat stays near a coin flip without teaching. Continuous features choosing the sound, with roles kept coarse, is the first-principles alternative.
9. **Freeze `core`; expect three rewrites.** The hit event's fixed 7-bit fields (no pitch, sweep or decay), the six hard-coded sliders (three are tonal and may do nothing for drums), and the seven-operation edit language (no gesture in the v0.1 app uses it) are unlikely to survive a real voice. Meanwhile the things the next step needs have no type at all: a palette, a voice, the lens, and the session glue that mixes, plays and exports.
10. **The offline analyser cannot run live as written.** Its windows look 40 to 60 ms ahead. The planned 25 ms monitor needs its own causal path; decide that before hit and lens work depends on the offline shapes.
11. **Plaits is a shortcut to test, not the engine.** Its drum voices have fixed envelopes and three macros, its internal block may break block-size invariance, and the Rust port is one maintainer's git repository. Compare it blind against a few purpose-built voices before depending on it.
12. **The experience has four gaps:** no way to hear a render in the context of the song; no way to fix one wrong hit; dice that re-roll everything when only the snare is wrong; and a first run with nothing taught. Export should be stems per role, aligned to the take.

Confirmed defects, none urgent: the hit cache loads with no structural check; `SplitAt` can snap to a boundary that does not touch the span and refuse a valid split; `MuteSpan` leaves one-sample slivers; `JoinAt` fills a gap of any length; a take that starts in room tone gets a picked onset at 0; the evidence cache key omits most analyser constants; `score.py --threshold` scores a different detector from the default pick.

Documents: MASTER-PLAN is stale on Week 0, the hit's exciter and the plugin smoke spike (promised four times, never built); `core-types.md` departs from the code in four places; the risk register misses the real bottleneck, which is the founder's listening time.

## What should not change

The reviewers were asked to attack and still agreed on these:

- Evidence is observation and the lens is opinion; candidates and the onset function are stored so decisions re-run without audio.
- Offline-first, analysing the whole take. It is the accuracy advantage over live tools.
- The renderer takes a `ControlTrack` and nothing else; mangle is per voice; polish belongs to the DAW.
- A hit's identity (`anchor`) is separate from when it fires (`at`); seeds derive from identity.
- Strict loading and atomic saves.
- Listening as evaluation, beside the mir_eval numbers; conservative for evaluation, eager for sound.

## What nobody had anticipated

- **The capture chain is unexamined.** Plosives clip laptop mics and flatten the peak that sets velocity. Mic distance changes the low band that identifies a kick. Built-in mics apply their own processing. Headphone bleed from a click becomes false onsets. A stereo file with one live channel is averaged to half level.
- **Hearing the result changes the performance.** Takes recorded deaf may not exercise the mapping the way real use will. Record a second take while the first render plays.
- **The founder's ears are the scarce, serial resource.** Every gate needs them, and they adapt: after an hour with the same fills, the familiar starts to sound good. Schedule listening; keep a held-back set.
- **Dice can decouple sound from performance.** Re-rolled bodies drift from what was meant, and "my fill, designed" becomes "random drums on my rhythm".
- **"Mouth amount" may be the whole product:** the one control no competitor can offer, and the direct test of the main risk.
- **Rendering then re-analysing the render is a free test.** It needs no annotation and checks that audio and events agree.
- **Golden audio files will differ between macOS and Windows** (different maths libraries), so audio tests need a tolerance, not byte equality.
- **Three sample rates meet on day one:** the capture device, the render and the output device.
- **A sustaining mangle (feedback, a ringing filter) will be cut** by the contract's 50 ms gap rule and hard reset, and will sound like an unexplained gate.
- **Per-take percentile normalisation hides level problems:** a take that is mostly fan noise gets stretched to full range.
- **Distribution has costs the plan defers:** an unsigned macOS app is blocked for anyone else, a bundled app needs a microphone permission string, and AI-written code has uncertain copyright in some jurisdictions, which weakens GPL enforcement.

## Where the reviewers are wrong, or where to hold back

- **Do not delete core machinery.** Several lenses recommend removing migrations, the edit language or the exact-float writer. They cost nothing to keep and the founder chose those rules. Freeze them: no new tests, no new reviews, until something uses them.
- **Do not abandon the golden set; reorder it.** Hours of annotation before the listening verdict is the wrong order. Recording the raw material is still worth doing, at three input levels, with a throat-bass item and two seconds of room tone at the start.
- **"Critical" was over-used.** The skeptics downgraded several: the pYIN prior is standard practice, not a contradiction, and the real-time contract is ahead of need but harmless.
- **The name and the competitor landscape are unverified.** The agents' searches are the only source.

## Proposed order of work

1. **The founder listens to the sketch** (about 30 minutes) and gives a verdict per render, plus what excited or bored them.
2. **Fix the gain-dependent onsets and add the noise-floor primitive** in evidence, with tests that vary gain and start in room tone.
3. **Iterate the sketch by ear:** a texture voice for breath and noise, continuous mapping in place of hard roles, the mouth as exciter. Decide Plaits against purpose-built voices blind.
4. **Port what wins to Rust** behind the existing contract, with a thin lens and `cli render`, and let that slice reshape the hit event and the sliders. Add the session glue and stem export then.
5. **Re-baseline MASTER-PLAN** after the verdict: dates, a week-4 gate with a control, a risk register that names founder listening time.
6. **Small fixes** as they come up: the edit defects, hit-cache validation, the cache key, the spec's four departures.
