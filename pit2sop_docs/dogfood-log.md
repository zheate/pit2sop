# Dogfood Log

Purpose: verify whether Pit2SOP prevents repeated mistakes in real work. This is not a feature wishlist.

## Daily Template

```markdown
## YYYY-MM-DD

### Context

What real work was happening?

### Pit captured

What went wrong, almost went wrong, or caused rework?

### Did Pit2SOP help?

Yes / no / partially. Be concrete.

### What should improve?

Only list changes that strengthen the core loop.

### Product decision

Capture faster, improve extraction, improve reminder quality, reduce review friction, or defer.
```

## 2026-05-23

### Context

Preparing `v0.2.0-beta.2` after `v0.2.0-beta.1` shipped.

### Pit captured

The project drifted toward more engineering polish after the desktop beta shipped. The core product question was getting weaker: will Pit2SOP actually remind before repeated mistakes?

### Did Pit2SOP help?

No. The correction came from product review, not from a pre-action reminder.

### What should improve?

Release and planning checks should surface the North Star before starting new platform or packaging work.

### Product decision

Write the product North Star and use this log for 14 days before adding broad new features.

### Pit2SOP result

Captured with `sop pit`. Generated:

- `01_Pits/2026/2026-05-23 开发中偏离核心指标，堆砌外围功能 7a7e00ff.md`
- `02_SOPs/Release/SOP - 规划新功能前检查核心指标.md`

Validation: `sop check 我要规划下一个 Pit2SOP 功能` matched the SOP and produced a checklist about North Star and dogfood alignment.

## 2026-05-23

### Context

Reviewing beta release readiness.

### Pit captured

Earlier beta smoke testing covered the Pending empty state first, but initially missed the more important apply/reject UI path.

### Did Pit2SOP help?

Partially. The core and CLI had coverage, but the desktop UI path needed a seeded pending patch smoke.

### What should improve?

Before release work, `sop check` should remind to seed Pending patches and verify apply/reject refresh, source Pit closure, and `doing` after apply.

### Product decision

Strengthen release SOP reminders. Do not add unrelated functionality.

### Pit2SOP result

Captured with `sop pit`. Generated:

- `01_Pits/2026/2026-05-23 Beta发布前漏测apply reject UI a5e61403.md`
- `02_SOPs/Release/SOP - Beta发布前UI交互测试清单.md`

Validation: `sop check 我要发布 beta 版本` matched the SOP and produced a checklist covering seeded pending patch data, apply/reject refresh, source Pit closure, and doing match.
