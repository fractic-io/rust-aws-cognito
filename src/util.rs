use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_cognitoidentityprovider::{
    config::Region,
    error::SdkError,
    operation::list_users::{ListUsersError, ListUsersOutput},
};
use fractic_env_config::EnvVariables;
use fractic_generic_server_error::{common::CriticalError, GenericServerError};

use crate::{env::CognitoEnvConfig, errors::CognitoConnectionError};

// AWS Cognito utils.
// --------------------------------------------------

pub struct CognitoUtil<ClientImpl: CognitoClient> {
    client: ClientImpl,
    env: EnvVariables<CognitoEnvConfig>,
}

impl CognitoUtil<aws_sdk_cognitoidentityprovider::Client> {
    pub async fn new(
        env: EnvVariables<CognitoEnvConfig>,
    ) -> Result<CognitoUtil<aws_sdk_cognitoidentityprovider::Client>, GenericServerError> {
        let region_str = env.get(&CognitoEnvConfig::CognitoRegion)?;
        let region = Region::new(region_str.clone());
        let shared_config = aws_config::defaults(BehaviorVersion::v2024_03_28())
            .region(region)
            .load()
            .await;
        let client = aws_sdk_cognitoidentityprovider::Client::new(&shared_config);
        Ok(Self { client, env })
    }
}

impl<ClientImpl: CognitoClient> CognitoUtil<ClientImpl> {
    pub async fn get_username_from_email(
        &self,
        email: &str,
    ) -> Result<Option<String>, GenericServerError> {
        let dbg_cxt: &'static str = "get_username_from_email";
        let user_pool_id = self.env.get(&CognitoEnvConfig::CognitoUserPoolId)?;

        let response = self
            .client
            .list_users(&user_pool_id, &format!("email = \"{}\"", email), 1)
            .await
            .map_err(|e| CognitoConnectionError::with_debug(dbg_cxt, "", e.to_string()))?;

        response
            .users
            .unwrap_or_default()
            .pop()
            .map(|user| {
                user.username.ok_or(CriticalError::with_debug(
                    dbg_cxt,
                    "user found but username is missing",
                    email.to_string(),
                ))
            })
            .transpose()
    }
}

// CognitoClient trait implementation.
//
// We wrap the regular cognito client in a custom
// trait so that we can mock it in tests.
// --------------------------------------------------

#[async_trait]
pub trait CognitoClient {
    async fn list_users(
        &self,
        user_pool_id: &str,
        filter: &str,
        limit: i32,
    ) -> Result<ListUsersOutput, SdkError<ListUsersError>>;
}

// Real client implementation.
#[async_trait]
impl CognitoClient for aws_sdk_cognitoidentityprovider::Client {
    async fn list_users(
        &self,
        user_pool_id: &str,
        filter: &str,
        limit: i32,
    ) -> Result<ListUsersOutput, SdkError<ListUsersError>> {
        self.list_users()
            .user_pool_id(user_pool_id)
            .set_filter(Some(filter.to_string()))
            .set_limit(Some(limit))
            .send()
            .await
    }
}

// Tests.
// --------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::env::{COGNITO_REGION, COGNITO_USER_POOL_ID};

    use super::*;
    use aws_sdk_cognitoidentityprovider::types::UserType;
    use fractic_core::collection;
    use fractic_env_config::EnvVariables;

    // Mock client implemenation.
    struct MockCognitoClient {
        should_find_user: bool,
    }
    #[async_trait]
    impl CognitoClient for MockCognitoClient {
        async fn list_users(
            &self,
            _user_pool_id: &str,
            _filter: &str,
            _limit: i32,
        ) -> Result<ListUsersOutput, SdkError<ListUsersError>> {
            let mut builder = ListUsersOutput::builder();
            if self.should_find_user {
                builder = builder.users(UserType::builder().username("username").build());
            };
            Ok(builder.build())
        }
    }

    #[tokio::test]
    async fn test_get_username_from_email_success() {
        let mock_client = MockCognitoClient {
            should_find_user: true,
        };
        let env: EnvVariables<CognitoEnvConfig> = collection! {
            COGNITO_REGION => "us-east-1".to_string(),
            COGNITO_USER_POOL_ID => "us-east-1_123456789".to_string(),
        };
        let cognito = CognitoUtil {
            client: mock_client,
            env,
        };
        let username = cognito
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
        let env: EnvVariables<CognitoEnvConfig> = collection! {
            COGNITO_REGION => "us-east-1".to_string(),
            COGNITO_USER_POOL_ID => "us-east-1_123456789".to_string(),
        };
        let cognito = CognitoUtil {
            client: mock_client,
            env,
        };
        let username = cognito
            .get_username_from_email("abc@example.com")
            .await
            .unwrap();
        assert_eq!(username, None);
    }
}
