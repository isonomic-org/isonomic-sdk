# Tool kits: deviations

Every place the kit code in this repo differs from its design, "Tool kits:
architecture v0.3": what, and why. References are named by ID only; the
register lives in the design. Newest entries go at the end of their
section.

## Planned deviations

The design's own rules (grants, access before the verdict, flat inputs,
result caps, the three-flag rule) are listed in the harness's
`docs/kits-deviations.md`. They hold here unchanged: this repo carries the
harness's types and adds none of its own to them.

## The permissions interface, remembered approvals and kits (#59)

References opened: none named for this issue. It follows the SDK's own
wire rules: new fixture lines only, tolerant decoding.

1. **The schema revision stays 1.** The design gives the kit fields
   revision 2, or revision 1 if they land with display widgets. The harness
   kept revision 1 (its deviation 10), and the SDK reports what the harness
   ships.
2. **The kit tool specs are not re-exported yet.** `agents::kits`
   re-exports the frame's types (scope, access, remember, the offer
   table). harness-core has no kit specs at the pinned commit; they arrive
   with the harness's files kit and join the re-export when the pin moves.
   Nothing was copied into the SDK.
3. **Three of the six conformance checks wait.** `permissions.shell_only`,
   `permissions.scoped` and `permissions.request_needs_person` are built,
   and fail against the token-stripping proxy. `permissions.revoked`,
   `kits.remember` and `kits.offered` each need a kit tool called in a
   session. The pinned harness has no kit tools, its chat preset names no
   kit, and no conformance check sends input (a check cannot know a real
   model's words). The test engine's own tests cover what can be shown
   without a tool: a revoked grant refuses the host's next check, `remember`
   never exceeds what a request offers and needs the person's token, and a
   kit the manifest does not name is never granted.
4. **The wire shapes are this repo's.** The design gives the four methods
   and what each carries in prose. The field names (`grant`, `permission`,
   `ask`, `access`, `until`), the one reply (`permissions`, for grant and
   list) and the ids are chosen here, and frozen by the new fixture lines.
5. **A no is a revoke.** The design records each answer, yes or no, with
   four methods. A no to a kit's first question is `permissions.revoke` of
   that kit, which reads `off` from then on, whether or not it was on.
6. **`files` has no switch.** The design lists kit switches beside linked
   folders. The files kit's permissions are the folders, so `kits` leaves it
   out and the test engine refuses `grant { kit: "files" }`.
7. **Commands in a folder are not on the wire.** The design's folder sheet
   also asks whether commands may run there. `access` is `read` or
   `read_write` only; it is an open enum, so the shell kit can add to it.
8. **`permissions.request` reuses `missing_user_action`.** The design adds
   one error code, `not_linkable`; a request without a token answers the
   code a decision without one already answers.
9. **How a request reaches the window, and how its token is minted, are
   not built.** The design says the window is asked and that the call needs
   a user-action token, and names no event, call or mint for either. The
   existing mint is bound to a session and an approval. Nothing was
   invented: the test engine records requests for tests to read and mints
   the token from its scripting handle. This needs the owner's decision
   before an engine can complete a request.
10. **The scratch root is the temporary directory, by path alone.** The
    test engine opens nothing: a folder is linkable when its path lies
    strictly inside the temporary directory. It serves no files; the
    in-memory file service belongs to the harness's files kit.
11. **The test engine does not model `session` or `until` on a grant**
    (`unsupported`), and lists no remembered approvals: the host's
    in-memory grants keep neither a session-wide standing grant nor a
    listing, and no tool at this pin can cause a remembered approval.
12. **One more harness crate, `harness-toolkit`,** at the pinned commit,
    for the in-memory grants the host already uses. It is in house and
    comes from the same repository as `harness-host`.
13. **One conformance check makes an empty folder** under the temporary
    directory and removes it, so an engine that opens what it links has
    something to open. The other checks still speak the wire alone.
