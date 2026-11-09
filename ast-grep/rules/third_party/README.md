# Third-party ast-grep rules

The Rust security rules in this folder, and their tests in
`ast-grep/rule-tests/third_party/`, come from
[coderabbitai/ast-grep-essentials](https://github.com/coderabbitai/ast-grep-essentials)
at commit `73120109bf45`, under the Apache License 2.0: a copy of the
licence is in [`LICENSE`](LICENSE) next to this file.

Copied from that repository's `rules/rust/security/` and `tests/rust/`:

- `postgres-empty-password-rust`
- `reqwest-accept-invalid-rust`
- `secrets-reqwest-hardcoded-auth-rust`
- `ssl-verify-none-rust`
- `tokio-postgres-empty-password-rust`
- `tokio-postgres-hardcoded-password-rust`

**Changes:** the top-level `ast-grep-essentials: true` key is removed from
each rule, because ast-grep 0.51 rejects unknown keys. Nothing else is
changed. The rules keep their `warning` severity; CI runs
`ast-grep scan --error`, so a finding fails the step.

**Left out:** `empty-password-rust` and `hardcoded-password-rust`, which
ast-grep 0.51 cannot parse.

To update, copy the files again from a newer upstream commit, drop the same
key, and change the commit above.
