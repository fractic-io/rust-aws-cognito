use fractic_context::define_ctx_view;

define_ctx_view!(
    name: CognitoCtxView,
    env {
        COGNITO_REGION: String,
        COGNITO_USER_POOL_ID: String,
    },
    secrets {},
    deps_overlay {},
    req_impl {}
);

#[cfg(test)]
pub(crate) mod test_ctx {
    use fractic_context::define_ctx;

    define_ctx!(
        name: TestCtx,
        env {
            COGNITO_REGION: String,
            COGNITO_USER_POOL_ID: String,
        },
        secrets_fetch_region: DUMMY,
        secrets_fetch_id: DUMMY,
        secrets {},
        deps {},
        views {
            crate::CognitoCtxView,
        }
    );
}
