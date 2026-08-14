use dicar_desktop_lib::{AiChatMessageDto, AiCompletionRequestDto, AiErrorCode};

fn request(model: &str, content: &str) -> AiCompletionRequestDto {
    AiCompletionRequestDto {
        request_id: "f9bc6db0-2705-4a8b-87a4-a61edf9735b3".to_owned(),
        model: model.to_owned(),
        messages: vec![AiChatMessageDto {
            role: "user".to_owned(),
            content: content.to_owned(),
        }],
    }
}

#[test]
fn accepts_a_safe_deepseek_request() {
    request("deepseek-chat", "Tune this controller")
        .validate()
        .expect("safe request should pass validation");
}

#[test]
fn rejects_unsafe_or_empty_model_names() {
    for model in [
        "",
        "../deepseek",
        "deepseek chat",
        " deepseek-chat",
        &"a".repeat(65),
    ] {
        let error = request(model, "hello")
            .validate()
            .expect_err("unsafe model must be rejected");
        assert_eq!(error.code, AiErrorCode::AiInvalidRequest);
    }
}

#[test]
fn rejects_invalid_request_ids_roles_and_message_limits() {
    let mut invalid_id = request("deepseek-chat", "hello");
    invalid_id.request_id = "not-a-uuid".to_owned();
    assert_eq!(
        invalid_id.validate().unwrap_err().code,
        AiErrorCode::AiInvalidRequest
    );

    let mut invalid_role = request("deepseek-chat", "hello");
    invalid_role.messages[0].role = "assistant".to_owned();
    assert_eq!(
        invalid_role.validate().unwrap_err().code,
        AiErrorCode::AiInvalidRequest
    );

    let too_many_messages = AiCompletionRequestDto {
        messages: (0..33)
            .map(|_| AiChatMessageDto {
                role: "user".to_owned(),
                content: "x".to_owned(),
            })
            .collect(),
        ..request("deepseek-chat", "hello")
    };
    assert_eq!(
        too_many_messages.validate().unwrap_err().code,
        AiErrorCode::AiInvalidRequest
    );

    let oversized = request("deepseek-chat", &"x".repeat(64 * 1024 + 1));
    assert_eq!(
        oversized.validate().unwrap_err().code,
        AiErrorCode::AiInvalidRequest
    );
}
