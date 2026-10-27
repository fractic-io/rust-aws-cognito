# fractic-aws-cognito

Thin wrapper over the AWS Cognito SDK: `CognitoUtil` (user lookup and
attribute operations), `CognitoCtxView` (the env vars a consumer provides),
typed errors. `src/lib.rs` → `context.rs` (`define_ctx_view!`), `errors.rs`,
`util.rs`.

- Keep the shape: errors in `errors.rs`, context requirements in
  `context.rs`, the public surface `util.rs` (`CognitoUtil`) plus `pub use`s
  in `lib.rs`. A new capability is a method on the util or an `ext/` module,
  not a parallel façade.
- Commit / PR titles: `[util] …`, `[ext] …`.
