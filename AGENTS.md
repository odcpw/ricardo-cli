# Ricardo CLI

This is the standalone Rust Ricardo interface CLI, a sibling of Musicbox V2.
Keep auction logic and browser integration here. Git identity belongs to odcpw.

Read `docs/agent-contract.md` before changing command semantics. Read
`skills/ricardo-cli/SKILL.md` when using this CLI on behalf of a user.

## Implementation rules

- Preserve the versioned JSON envelope, exit code meanings, bounded requests,
  and stdout/stderr separation. Use `capabilities` to disclose real support.
- Retrieve current information from Ricardo through the connected browser
  session. Never substitute indexed search pages or stored observations for a
  failed live request. Unknown status is not active.
- Preserve observed timestamps and separate current bid, next bid, buy-now,
  auction closing time, seller claims, and independently observed facts.
- Implement only routes and response shapes supported by observed evidence.
  Interface drift must produce an actionable error, not invented data.
- Authentication belongs in the browser. Do not store, log, or commit passwords,
  browser profiles, cookies, tokens, account dumps, or private observations.
- Restrict browser requests to approved Ricardo origins. Read snapshots do not
  authorize arbitrary scripts, unrelated navigation, or external messages.
- Mutations must name their action and target, preview by default, and require
  explicit execution. Never claim a mutation succeeded without a verified
  response. Do not add bids, purchases, payments, listing publication, or seller
  messaging as side effects of account or search commands.
- Prefer Rust for CLI and transport hot paths. Use the existing FrankenSuite
  dependencies where they fit; avoid adding runtimes for convenience.
- Keep tests targeted to actual parsing, security, protocol, and CLI contracts.
  Report which tests ran and whether the logged-in live path was exercised.
- Do not commit build outputs or local state. Never infer remote publication
  authorization merely from work in this repository.

## Agent ergonomics

Use the `agent-ergonomics-and-intuitiveness-maximization-for-cli-tools` skill for
CLI changes. Keep help, capabilities, examples, and the agent guide synchronized.
Useful wrong invocations should receive a correction. Noninteractive commands
must not hang waiting for a prompt. Follow this repository's toolchain config.
