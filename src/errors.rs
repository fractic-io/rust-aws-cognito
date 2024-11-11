use fractic_server_error::{
    define_internal_error_type, GenericServerError, GenericServerErrorTrait,
};

define_internal_error_type!(CognitoConnectionError, "Cognito error.");
