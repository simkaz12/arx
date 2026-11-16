# Roster wave 2 verification

**PASS — 2026-09-11.** The applicable agreement in
[the brief](../specs/2026-09-11-roster-wave-2-codex-main.md) passed independent review and runtime verification.
The owner deferred the real Claude ↔ Codex live handoff until Sunday, 2026-09-13 after 16:00
Europe/Vilnius. That check is excluded from this gate and has not passed.

## Provenance

- Branch: `roster-wave-2-codex-main`; base revision: `c0567aa3f69029dc3aeb8b57551c30ea41dd7a79`.
- Frozen product/config/test fingerprint: `1c107c765c51a343c8a13732b755d07b5b05572a0c96e525351596e1355e12de`.
- Local manifest: `/tmp/arx-wave2-input-fingerprint-pending-fix.json`; all 132 files matched at final recheck.
  Planning and verification documents are excluded. `git diff --check` passed.
- Environment: Codex CLI 0.153.4, Claude Code 2.1.263, rustc 1.98.0, Node 25.2.1.
- Intended Codex identity: the configured account in `~/.codex/auth.json`.
  `/tmp/arx-wave2-live-_lhdzl1t/identity.json` confirms the isolated copy matched; credentials are not recorded here.
- Runner PASS; independent reviewer PASS with empty `must_fix`; independent `hand-ex` verification PASS.
  The external Codex gate is not applicable to this Codex-native run.

The delivered diff covers `main_body`, `codex_main`, `codex_auth` and `main_tools`; their callers in
`main`, `claude_code`, `config`, `container_hand`, `roster` and `terminal`; the Claude/Codex and layer-close
tests and fixtures; sidecar tool schemas and parity tests; the standing prompt, vocabulary, position,
brief, source findings and map. No hand-interface redesign or provider main is included.

## Acceptance evidence

| Criterion | Observed result | Local evidence |
| --- | --- | --- |
| Real Codex main and mounted tools | `arx run` returned `MOUNTED_TOOL_ROUNDTRIP_OK` through `read_file`; transcript recorded the mounted-tool allow gate. | `/tmp/arx-wave2-live-_lhdzl1t` |
| Stop, follow-up and same-body selection | Current-input installed-Codex acceptance passed interruption, a subsequent turn on the same thread, and selection of `gpt-6-astra`. | `cargo test -p arx --test codex_main installed_codex_tool_interrupt_followup_and_model_change -- --ignored --exact --nocapture` |
| Ordinary checked reset | Real Codex authored and truth-checked checkpoint 1; `/reset` created a fresh home, oriented from that checkpoint and answered `RESET_HANDOFF_OK`. | `/tmp/arx-wave2-live-_lhdzl1t` |
| Usage | Latest prompt occupancy remained separate from cumulative usage. An observed row had input 9713, cached input 9472 and cumulative input 19364; cached input was not added twice. | Same live transcript; matching usage regressions |
| Modal cross-body selection and clear | Actual PTY with deterministic bodies completed Claude fixture → Codex and clear back, each with checkpoint, body-switch reset, orientation and model event. Esc preserved pin bytes. | `/tmp/arx-wave2-modal-jywsl5yo` |
| Typed cross-body selection and surviving hands | Exit 0; `h1` spawned before the switch delivered after destination orientation, then Codex answered the next operator turn. | `/tmp/arx-wave2-handswitch-eof-a77cq3jq` |
| Compaction and exhaustion | Five compaction checks and two exhaustion checks passed against current inputs: abandoned tools suppressed, fresh incarnation, checked-object fallback, unfinished truth-check rejected, and an in-flight hand delivered. Canonical hand-survival reset also passed. | Current targeted runtime checks and matching workspace log |
| Model refusal and pending choice | Failed validation/launch of a replacement preserved the pending choice. Its own later refusal restored the original usable model/pin. Independent reproduction confirmed one activation event on the accepted turn. | `/tmp/arx-wave2-pending-fixed-5mc9uh6s`; `/tmp/arx-wave2-review-pending-fixed-gr_07ap0` |
| Persistence and failure consistency | Regressions passed for listed-but-refused models, no replay after a started turn, refused clear, final roster-save failure, select → quit → restart, and direct startup from a Codex pin. | Matching workspace log |
| Prompt isolation and cleanup | Initialization checks verified distinct homes/thread IDs, authentication-only copying, no ambient instruction/config files, `project_doc_max_bytes=0`, and arx-owned `baseInstructions`. Verification-owned Codex homes and hand containers were gone afterward. | Current adapter checks and live probe |

## Regressions and review corrections

The final workspace run passed **735 tests, zero failed, two ignored**. The measured baseline was 708
passed and one ignored; no tests were removed. The added ignored installed-Codex acceptance test was run
separately and passed. The other ignored test is the existing live Ollama recall test.

Sidecar tests passed **14/14**; configuration validation exited 0. Matching logs were reused:

- `/tmp/arx-wave2-workspace-pending-fix.log`
- `/tmp/arx-wave2-sidecar-pending-fix.log`
- `/tmp/arx-wave2-check-config-pending-fix.log`
- `/tmp/arx-wave2-pending-fix-final-evidence.json`

Review found and rechecked two corrections: a catalogue-listed model refused at `turn/start` must restore
the prior selection, and a failed replacement must preserve an already-saved pending choice. A desired
same-body pin is saved immediately with a pending notice; accepted `turn/start` confirms activation.
This preserves select → quit → restart without claiming that an unaccepted model ran.

The layer-close fixture was updated from removed SDK approval RPCs to real mounted `list_files` allow
and `decisions_query` deny calls, preserving deterministic admission coverage.

## Red classifications and deferred check

Two exploratory hand-switch probes timed out: **broken fixture/driver**. They held a non-TTY `LineFront`
pipe open after their final input. Its blocking read prevented the loop from pumping a later delivery.
The EOF-driven plain-front probe observed the delivery after cross-body orientation. These timeouts do
not establish a wave 2 defect.

**OWNER-DEFERRED:** real Claude ↔ Codex live handoff, Sunday 2026-09-13 after 16:00 Europe/Vilnius.
Claude's weekly quota prevented that earlier observation. No further live Claude call was made during
verification. The deterministic typed and PTY cross-body checks passed; they do not replace this deferred
live check in the record.

Evidence paths above are local scratch artifacts, not repository attachments. The observed outcomes and
provenance are retained here; credentials are excluded.

## Post-landing check — 2026-09-12

**Verdict: PASS with caveats for the existing Wave 2 acceptance scope.** No product defect reproduced.
The real Claude ↔ Codex handoff remains owner-deferred as recorded above. This check does not establish
live-provider readiness beyond the earlier matching evidence.

Tested `main` at `3c92977302a28a8accec18b79c5434a74a82b299`, matching `origin/main`, with a clean worktree
before and after execution. Its content tree equals the landed feature tip `95f6c5d`. All 132 entries in
the existing manifest matched fingerprint `1c107c765c51a343c8a13732b755d07b5b05572a0c96e525351596e1355e12de`.
Environment: rustc/cargo 1.98.0, Node 25.2.1, Codex 0.153.4, Claude Code 2.1.263, Docker 29.7.2.
This report update is excluded from the product fingerprint.

| Check | Current observation |
| --- | --- |
| `cargo test --workspace` | Exit 101 at `a_fake_dispatch_delivered_event_carries_elapsed_and_lag`; all 435 unit tests had passed. The exact failing test passed on rerun. |
| `cargo test --workspace --no-fail-fast` | 734 passed, one failed, two ignored. Only `l2_close_loop_in_containers` failed, on a stale hand directory left by the preceding panic. |
| Layer-close rerun | One passed after confirming the stale directory's owning PID was dead and removing only that verification-created directory. |
| `npm test` in `sidecar/` | Exit 0; 14 passed. |
| `cargo run -p arx -- check-config ./arx.toml` | Exit 0; expected configured roster and tool-coverage notices. |
| Optional `cargo fmt --all -- --check` | Exit 1 at both current HEAD and pre-Wave-2 base `c0567aa`; pre-existing, outside the documented gate. |

All **735 workspace tests passed across these runs, with two ignored**. There was no single uninterrupted
green workspace invocation in this check. The earlier landing log records its own uninterrupted green run;
the scratch log paths above refer to that earlier evidence, not to new logs from this check.

The current `codex_main` integration run passed 27 tests, with its paid live test ignored. These tests drive
actual isolated arx CLI subprocesses with deterministic bodies and temporary config/store/roster. They
observed typed Claude → Codex → Claude handoff, checked objects and orientation, restart from a Codex pin,
reset and compaction recovery, hand delivery after recovery, and model-refusal/persistence recovery.
The successful layer-close rerun exercised the container-backed end-to-end CLI session. No separate
interactive smoke or new paid-provider call ran; the intended identities here were local test fixtures.

The delivery-lag failure is classified as **broken fixture/timing flake**: the test sleeps four seconds
but requires at least 1500 ms delivery lag. The fake container took 2737 ms under this run's load, leaving
1221 ms lag. Its exact rerun passed. The layer-close failure is **broken fixture/environment contamination**:
the first panic left `/tmp/arx-hands/h2-3287866-1789161907184736291`; the cleanup assertion passed after
scoped removal. Neither result establishes a Wave 2 product regression, but both expose test reliability
work. Formatting drift is a **pre-existing failure**, reproduced at the base revision.

Final recheck found no labeled hand containers, `/tmp/arx-hands` entries, verification processes, or
product changes. The remaining live-handoff gap is unchanged.
