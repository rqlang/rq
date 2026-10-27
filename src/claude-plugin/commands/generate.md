---
description: Generate rqlang (.rq) requests from a description
argument-hint: <what the requests should do>
---

Generate rqlang requests for: $ARGUMENTS

Follow the `rq-authoring` skill: read both rqlang references, check existing request names with `list_requests`, then write the files and iterate with `validate_rq` and `lint_rq` until both return `ok: true` for every file.
