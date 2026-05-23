# Pit2SOP North Star

Pit2SOP turns repeated mistakes into pre-action checklists.

中文定义：Pit2SOP 把踩过的坑变成下次行动前的检查清单。

## Core Loop

```text
Capture a pit
→ extract the prevention rule
→ update an SOP
→ before similar work, surface the checklist
→ reduce repeated mistakes
```

The product has value only if this loop works in real work.

## Non-Goals

Pit2SOP is not:

- a general note app
- a task manager
- a chat assistant
- an Obsidian replacement
- a mobile-first product yet
- a generic AI knowledge base

## Product Metrics

| Metric | Target |
|---|---|
| Capture friction | A real pit can be captured within 30 seconds. |
| Extraction quality | Most generated Pit/SOP output needs little editing. |
| Reminder accuracy | `doing` / `check` surfaces genuinely related SOPs. |
| Noise control | Unrelated checklist reminders stay rare. |
| Reuse value | A Pit becomes at least one useful future checklist item. |

## Dogfood Gate

Before adding large new capabilities, dogfood the product for 14 days:

```text
20 real pits captured
5 useful pre-action reminders
0 critical review-loop bugs
```

If this gate fails, improve the core reminder loop before adding more surfaces.

## Feature Filter

Any new feature must answer yes to at least one question:

1. Does it make Pit capture faster?
2. Does it make a Pit easier to convert into an executable checklist?
3. Does it make pre-action reminders more accurate?
4. Does it reduce review/pending friction?

If not, defer it.

## Current Focus

V0.2 beta is a macOS desktop shell for the existing local loop. Do not expand into phone, voice, browser extension, background agent, Git hook, multi-vault, or vector search until dogfood proves the loop is useful.
