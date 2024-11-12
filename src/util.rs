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
use fractic_env_config::EnvVariables;
use fractic_server_error::{CriticalError, ServerError};

use crate::{env::CognitoEnvConfig, errors::CognitoCalloutError};

const EMAIL_ATTRIBUTE: &str = "email";
const USER_SUB_ATTRIBUTE: &str = "sub";

// AWS Cognito utils.
// --------------------------------------------------

pub struct CognitoUtil<ClientImpl: CognitoClient> {
    client: ClientImpl,
    env: EnvVariables<CognitoEnvConfig>,
}

impl CognitoUtil<aws_sdk_cognitoidentityprovider::Client> {
    pub async fn new(
        env: EnvVariables<CognitoEnvConfig>,
    ) -> Result<CognitoUtil<aws_sdk_cognitoidentityprovider::Client>, ServerError> {
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
    async fn get_username_from_attribute(
        &self,
        attribute: &str,
        value: &str,
    ) -> Result<Option<String>, ServerError> {
        let user_pool_id = self.env.get(&CognitoEnvConfig::CognitoUserPoolId)?.clone();

        let response = self
            .client
            .list_users(user_pool_id, format!("{} = \"{}\"", attribute, value), 1)
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
        let user_pool_id = self.env.get(&CognitoEnvConfig::CognitoUserPoolId)?.clone();

        if let Some(username) = self
            .get_username_from_attribute(USER_SUB_ATTRIBUTE, user_sub)
            .await?
        {
            self.client
                .admin_delete_user_attributes(
                    user_pool_id,
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
pub trait CognitoClient {
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
impl CognitoClient for aws_sdk_cognitoidentityprovider::Client {
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
