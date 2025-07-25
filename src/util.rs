use std::sync::Arc;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_cognitoidentityprovider::{
    config::Region,
    error::SdkError,
    operation::{
        admin_delete_user_attributes::{
            AdminDeleteUserAttributesError, AdminDeleteUserAttributesOutput,
        },
        list_users::{ListUsersError, ListUsersOutput},
    },
};
use fractic_context::register_ctx_singleton;
use fractic_server_error::{CriticalError, ServerError};

use crate::{errors::CognitoCalloutError, CognitoCtxView};

const EMAIL_ATTRIBUTE: &str = "email";
const USER_SUB_ATTRIBUTE: &str = "sub";

// AWS Cognito utils.
// --------------------------------------------------

pub struct CognitoUtil {
    backend: Arc<dyn CognitoBackend>,
    user_pool_id: String,
}

impl CognitoUtil {
    pub async fn new(ctx: &dyn CognitoCtxView) -> Result<CognitoUtil, ServerError> {
        Ok(Self {
            backend: ctx.cognito_backend().await?,
            user_pool_id: ctx.cognito_user_pool_id().clone(),
        })
    }
}

impl CognitoUtil {
    async fn get_username_from_attribute(
        &self,
        attribute: &str,
        value: &str,
    ) -> Result<Option<String>, ServerError> {
        let response = self
            .backend
            .list_users(
                self.user_pool_id.clone(),
                format!("{} = \"{}\"", attribute, value),
                1,
            )
            .await
            .map_err(|e| CognitoCalloutError::with_debug(&e))?;

        response
            .users
            .unwrap_or_default()
            .pop()
            .map(|user| {
                user.username.ok_or(CriticalError::new(&format!(
                    "User found but username is missing (attribute: '{}', value: '{}').",
                    attribute, value
                )))
            })
            .transpose()
    }

    pub async fn get_username_from_email(
        &self,
        email: &str,
    ) -> Result<Option<String>, ServerError> {
        self.get_username_from_attribute(EMAIL_ATTRIBUTE, email)
            .await
    }

    pub async fn delete_email_for_user(&self, user_sub: &str) -> Result<(), ServerError> {
        if let Some(username) = self
            .get_username_from_attribute(USER_SUB_ATTRIBUTE, user_sub)
            .await?
        {
            self.backend
                .admin_delete_user_attributes(
                    self.user_pool_id.clone(),
                    username,
                    vec![EMAIL_ATTRIBUTE.to_string()],
                )
                .await
                .map_err(|e| CognitoCalloutError::with_debug(&e))?;
        }

        Ok(())
    }
}

// CognitoClient trait implementation.
//
// We wrap the regular cognito client in a custom
// trait so that we can mock it in tests.
// --------------------------------------------------

#[async_trait]
pub trait CognitoBackend: Send + Sync {
    async fn list_users(
        &self,
        user_pool_id: String,
        filter: String,
        limit: i32,
    ) -> Result<ListUsersOutput, SdkError<ListUsersError>>;

    async fn admin_delete_user_attributes(
        &self,
        user_pool_id: String,
        username: String,
        attributes: Vec<String>,
    ) -> Result<AdminDeleteUserAttributesOutput, SdkError<AdminDeleteUserAttributesError>>;
}

// Real client implementation.
#[async_trait]
impl CognitoBackend for aws_sdk_cognitoidentityprovider::Client {
    async fn list_users(
        &self,
        user_pool_id: String,
        filter: String,
        limit: i32,
    ) -> Result<ListUsersOutput, SdkError<ListUsersError>> {
        self.list_users()
            .user_pool_id(user_pool_id)
            .set_filter(Some(filter.to_string()))
            .set_limit(Some(limit))
            .send()
            .await
    }

    async fn admin_delete_user_attributes(
        &self,
        user_pool_id: String,
        username: String,
        attributes: Vec<String>,
    ) -> Result<AdminDeleteUserAttributesOutput, SdkError<AdminDeleteUserAttributesError>> {
        self.admin_delete_user_attributes()
            .user_pool_id(user_pool_id)
            .username(username)
            .set_user_attribute_names(Some(attributes))
            .send()
            .await
    }
}

// Register dependency, default to real AWS backend.
// --------------------------------------------------

register_ctx_singleton!(dyn CognitoCtxView, dyn CognitoBackend, |ctx: Arc<
    dyn CognitoCtxView,
>| async move {
    let region = Region::new(ctx.cognito_region().clone());
    let shared_config = aws_config::defaults(BehaviorVersion::v2025_01_17())
        .region(region)
        .load()
        .await;
    Ok(aws_sdk_cognitoidentityprovider::Client::new(&shared_config))
});

// Tests.
// --------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::test_ctx::TestCtx;

    use super::*;
    use aws_sdk_cognitoidentityprovider::types::UserType;

    // Mock client implemenation.
    struct MockCognitoClient {
        should_find_user: bool,
    }
    #[async_trait]
    impl CognitoBackend for MockCognitoClient {
        async fn list_users(
            &self,
            _user_pool_id: String,
            _filter: String,
            _limit: i32,
        ) -> Result<ListUsersOutput, SdkError<ListUsersError>> {
            let mut builder = ListUsersOutput::builder();
            if self.should_find_user {
                builder = builder.users(UserType::builder().username("username").build());
            };
            Ok(builder.build())
        }

        async fn admin_delete_user_attributes(
            &self,
            _user_pool_id: String,
            _username: String,
            _attributes: Vec<String>,
        ) -> Result<AdminDeleteUserAttributesOutput, SdkError<AdminDeleteUserAttributesError>>
        {
            let builder = AdminDeleteUserAttributesOutput::builder();
            Ok(builder.build())
        }
    }

    async fn build_util(mock_backend: MockCognitoClient) -> CognitoUtil {
        let ctx = TestCtx::init_test("mock-region".to_string(), "mock-user-pool-id".to_string());
        ctx.override_cognito_backend(Arc::new(mock_backend)).await;
        CognitoUtil::new(&*ctx).await.unwrap()
    }

    #[tokio::test]
    async fn test_get_username_from_email_success() {
        let mock_client = MockCognitoClient {
            should_find_user: true,
        };
        let util = build_util(mock_client).await;
        let username = util
            .get_username_from_email("abc@example.com")
            .await
            .unwrap();
        assert_eq!(username, Some("username".to_string()));
    }

    #[tokio::test]
    async fn test_get_username_from_email_not_found() {
        let mock_client = MockCognitoClient {
            should_find_user: false,
        };
        let util = build_util(mock_client).await;
        let username = util
            .get_username_from_email("abc@example.com")
            .await
            .unwrap();
        assert_eq!(username, None);
    }
}
