# fractic-aws-cognito

Thin wrapper over the AWS Cognito SDK: `CognitoUtil` (user lookup and
attribute operations), `CognitoCtxView` (the env vars a consumer provides),
typed errors. `src/lib.rs` → `context.rs` (`define_ctx_view!`), `errors.rs`,
`util.rs`. Consumer side: `/efs/fractic/_/rust.md` §2.

- Commit / PR titles: `[util] …`, `[ext] …`.
