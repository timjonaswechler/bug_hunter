# Explicit timing with no hidden advance or wait

## Status

Superseded by
[ADR-0005](0005-tick-warp-preserves-explicit-simulation-control.md).

This ADR was imported into the standalone `bug_hunter` repository so that its decision history
remains available. It was accepted and implemented in the original `star_sim` integration of
`bug_hunter`.

## Context

Controlled Sessions use a controlled clock. `TimeUpdateStrategy::ManualDuration(ZERO)` is active,
`SimulationScheduleOrder` is parked, and only `control_schedule::Frames` runs on an explicit time
command.

The former host layer introduced implicit behavior:

- `ControllerSession::settle()` combined an input request with `advance(1)`.
- `click` advanced two hidden frames and `activate_target` advanced three.
- `wait_for`, `Session::wait_for_observation`, `FrameLimit`, and `Script::Wait` polled observations
  while advancing hidden frames.
- The Wire protocol used `time.advance`, the host and REPL used `step`, and configuration used
  `frame_nanoseconds` and `startup_frames` for the same concept.

As a result, the clock frame index was not derivable from the command trace, several inputs could
not be batched into one frame, Session Recordings hid when time passed, and replay fidelity
suffered.

The requirement was to queue several Virtual Input commands and apply them together through one
explicit `step`. No dummy input should be needed for ticking, and `wait` should be removed.

## Decision

1. `step` is the only time command. `time::Command::Advance` becomes
   `time::Command::Step { frames, step_nanoseconds }`. The change is breaking and has no Wire alias
   for `advance`.
2. Host operations such as `settle`, `transition`, `click`, and `perform` send requests without
   advancing time. Time advances only through an explicit `step`.
3. Built-in waiting is removed. A caller may implement polling by alternating an observation with
   an explicit `step(1)`.
4. Several Pointer, Keyboard, or Text commands received before one `step(1)` are consumed together
   in the same application update.
5. A Session Script consists of actions, explicit steps, and expectations. Virtual Input remains
   queued until the next `step`.

## Consequences

- Timing is explicit in command traces and Session Recordings.
- Input commands can be batched into one application frame.
- The Wire rename from `advance` to `step` is breaking, and old recordings must be regenerated.
- Existing scripts and examples must add explicit steps after input where application processing is
  required.
- `startup_frames` and `frame_nanoseconds` remain, but affect time only through explicit `step`
  commands.

## Historical references

The original decision referred to the former `star_sim` workspace paths
`crates/bug_hunter`, `sessions/museum.json`, and `docs/worksheet_timing_explicit.md`.
