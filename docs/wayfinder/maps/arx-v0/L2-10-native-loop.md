---
id: L2-10
layer: 2
labels: wayfinder:task
status: split
assignee: none
blocked-by: L1-close
ruled-by: D02 (`975f65cd`, `48af6ac7`), D14 (`3a5aefb6`, `5fadfcc0`), D07 (`7dae481c`, `e65b2a35`), T01 (`../arx-design/findings/T01-findings.md`), `6bbde714` as restated by `af68d429`
---

# L2-10 — the native loop

## Deliverable

The Goose lift on top of L1-10: the Anthropic adapter beside the OpenAI-compatible one (OpenRouter is a base_url, not a third adapter); the error enum by continuation class with Transient retried by the stream wrapper before the first item, Fatal surfaced, ContextWindowExceeded standing alone; the window gauge; append and rewind as the op-set with no splice; multi-turn. Admission in the loop is L3-01. Built for the hand role; the main seat is fog. Held off the frontier by `needs-split`; cut into session-sized cards at claim time (S-SOL-7).

## Done when

The loop completes a tool-using conversation against a live OpenAI-compatible endpoint and against Anthropic from one agent definition each; a stubbed 429 is retried before the first stream item; a stubbed 400 surfaces as ContextWindowExceeded with the overshoot parsed; L1-10's trait is unchanged.

## Ruled by

D02 (`975f65cd`, `48af6ac7`), D14 (`3a5aefb6`, `5fadfcc0`), D07 (`7dae481c`, `e65b2a35`), T01 (`../arx-design/findings/T01-findings.md`), `6bbde714` as restated by `af68d429` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Carried from L1-10

L1-10's reviewer (2026-09-03) found the kill path on a native hand cannot interrupt a stalled request: `reqwest::blocking` has no cancellation, so a vendor stall made of keep-alive lines holds the SSE reader with nothing yielded and the per-chunk `control_rx` check never runs until a real chunk, an error, or the 60 s request timeout. Kill is safe (no stale verdict lands) but not live (thread and socket outlive the kill for up to the timeout). The layer-1 brief locked both "check after every line" and the blocking client, which cannot both hold. This card picks the mechanism — a short read timeout with a resume loop, an async client on a private runtime behind the unchanged synchronous trait, or a cancel flag polled between reads — without moving L1-10's trait.

## Split

Cut on 2026-09-04 by main under the owner's overnight clearance into [L2-10a — the native loop: multi-turn, the window's op-set, a live kill](L2-10a-native-loop-multi-turn.md), [L2-10b — provider errors by continuation class, and the gauge](L2-10b-provider-errors-and-gauge.md) and [L2-10c — the Anthropic adapter](L2-10c-anthropic-adapter.md), in that order. The deliverable and done-when above are met when all three land; this card is not claimed on its own. The kill-path stall carried from L1-10 goes to L2-10a.
