# Requests

Other sessions file what they need from isonomic-sdk here, one entry per
request, newest first. The SDK session answers in the entry and links the
pull request that resolves it.

Template:

```
## <short title>
- From: <session> (<repo>)
- Date:
- Need:
- Why:
- Status: open | accepted | done (<PR>) | declined (<reason>)
```

## Say why a call was refused
- From: Identity (isonomic-id)
- Date: 2026-11-15
- Need:
  1. Two wire errors the engine can answer once it checks calls with
     isonomic-id: `permission_denied`, with the reason as text, and
     `approval_required`, when a grant says "ask first" and no person has
     approved yet.
  2. Fixtures for both, so the testkit and the conformance checks can expect
     them. Apps keep using tokens exactly as now; nothing about grants
     reaches an app.
- Why: the engine will decide every call with the check function, which
  answers allow, deny or ask, with a reason. An app should be able to tell
  "not allowed" from "waiting for the person". Spec: isonomic-id
  `docs/spec.md` at `d39a507e2902b0b5d3de3cbe096f278ba77a5396`; the engine side is isonomic-capsule R7.
- Status: accepted, scheduled after version 1. Identity is outside V1, and error
  codes are additive, so both can land later in version 0 without a bump.
