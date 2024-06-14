use fractic_env_config::{define_env_config, define_env_variable, EnvConfigEnum};

define_env_variable!(COGNITO_REGION);
define_env_variable!(COGNITO_USER_POOL_ID);

define_env_config!(
    CognitoEnvConfig,
    CognitoRegion => COGNITO_REGION,
    CognitoUserPoolId => COGNITO_USER_POOL_ID,
);
