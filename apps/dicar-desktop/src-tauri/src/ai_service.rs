use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

const DEEPSEEK_COMPLETIONS_URL: &str = "https://api.deepseek.com/chat/completions";
const MAX_MODEL_LEN: usize = 64;
const MAX_MESSAGES: usize = 32;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_API_KEY_BYTES: usize = 512;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const CREDENTIAL_SERVICE: &str = "com.dicar.tune";
const CREDENTIAL_USER: &str = "deepseek-api-key";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatMessageDto {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCompletionRequestDto {
    pub request_id: String,
    pub model: String,
    pub messages: Vec<AiChatMessageDto>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiErrorCode {
    AiUnavailable,
    AiKeyMissing,
    AiInvalidRequest,
    AiCancelled,
    AiTimeout,
    AiHttpError,
    AiResponseTooLarge,
    AiInvalidResponse,
    AiCredentialStoreError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiErrorDto {
    pub code: AiErrorCode,
    pub message: String,
}

impl AiErrorDto {
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: AiErrorCode::AiInvalidRequest,
            message: message.into(),
        }
    }

    fn credential_store() -> Self {
        Self {
            code: AiErrorCode::AiCredentialStoreError,
            message: "无法访问 Windows 凭据库".to_owned(),
        }
    }

    fn key_missing() -> Self {
        Self {
            code: AiErrorCode::AiKeyMissing,
            message: "尚未配置 DeepSeek API Key".to_owned(),
        }
    }

    fn http_error(status: Option<u16>) -> Self {
        let message = status.map_or_else(
            || "AI 服务请求失败".to_owned(),
            |status| format!("AI 服务返回 HTTP {status}"),
        );
        Self {
            code: AiErrorCode::AiHttpError,
            message,
        }
    }

    fn cancelled() -> Self {
        Self {
            code: AiErrorCode::AiCancelled,
            message: "AI 请求已取消".to_owned(),
        }
    }

    fn timeout() -> Self {
        Self {
            code: AiErrorCode::AiTimeout,
            message: "AI 请求超时".to_owned(),
        }
    }

    fn response_too_large() -> Self {
        Self {
            code: AiErrorCode::AiResponseTooLarge,
            message: "AI 响应超过 1 MiB 上限".to_owned(),
        }
    }

    fn invalid_response() -> Self {
        Self {
            code: AiErrorCode::AiInvalidResponse,
            message: "AI 服务返回了无法解析的响应".to_owned(),
        }
    }
}

impl AiCompletionRequestDto {
    pub fn validate(&self) -> Result<(), AiErrorDto> {
        if uuid::Uuid::parse_str(&self.request_id).is_err() {
            return Err(AiErrorDto::invalid_request("请求 ID 必须是有效的 UUID"));
        }

        let model = self.model.trim();
        if model.is_empty()
            || model.len() > MAX_MODEL_LEN
            || model.len() != self.model.len()
            || !model
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
        {
            return Err(AiErrorDto::invalid_request(
                "模型名称只能包含字母、数字、点、下划线、冒号或连字符，且最多 64 个字符",
            ));
        }

        if self.messages.is_empty() || self.messages.len() > MAX_MESSAGES {
            return Err(AiErrorDto::invalid_request("消息数量必须介于 1 到 32 之间"));
        }

        if self
            .messages
            .iter()
            .any(|message| !matches!(message.role.as_str(), "system" | "user"))
        {
            return Err(AiErrorDto::invalid_request("消息角色只允许 system 或 user"));
        }

        let total_bytes = self
            .messages
            .iter()
            .try_fold(0usize, |total, message| {
                total.checked_add(message.content.len())
            })
            .ok_or_else(|| AiErrorDto::invalid_request("消息内容过大"))?;
        if total_bytes > MAX_MESSAGE_BYTES {
            return Err(AiErrorDto::invalid_request("消息内容总计不得超过 64 KiB"));
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct CredentialStoreFailure;

trait AiCredentialStore: Send + Sync {
    fn get_api_key(&self) -> Result<Option<String>, CredentialStoreFailure>;
    fn set_api_key(&self, api_key: &str) -> Result<(), CredentialStoreFailure>;
    fn clear_api_key(&self) -> Result<(), CredentialStoreFailure>;
}

#[cfg(target_env = "msvc")]
#[derive(Default)]
struct WindowsCredentialStore;

#[cfg(target_env = "msvc")]
impl WindowsCredentialStore {
    fn entry() -> Result<keyring::Entry, CredentialStoreFailure> {
        keyring::Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER).map_err(|_| CredentialStoreFailure)
    }
}

#[cfg(target_env = "msvc")]
impl AiCredentialStore for WindowsCredentialStore {
    fn get_api_key(&self) -> Result<Option<String>, CredentialStoreFailure> {
        match Self::entry()?.get_password() {
            Ok(api_key) => Ok(Some(api_key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CredentialStoreFailure),
        }
    }

    fn set_api_key(&self, api_key: &str) -> Result<(), CredentialStoreFailure> {
        Self::entry()?
            .set_password(api_key)
            .map_err(|_| CredentialStoreFailure)
    }

    fn clear_api_key(&self) -> Result<(), CredentialStoreFailure> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialStoreFailure),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCredentialStatusDto {
    pub configured: bool,
}

pub struct AiServiceState {
    client: reqwest::Client,
    credential_store: Arc<dyn AiCredentialStore>,
    endpoint: String,
    response_limit: usize,
    active_requests: Mutex<HashMap<String, CancellationToken>>,
}

struct ActiveRequestGuard<'a> {
    request_id: String,
    active_requests: &'a Mutex<HashMap<String, CancellationToken>>,
}

impl Drop for ActiveRequestGuard<'_> {
    fn drop(&mut self) {
        self.active_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.request_id);
    }
}

impl AiServiceState {
    #[cfg(target_env = "msvc")]
    pub fn new() -> Result<Self, AiErrorDto> {
        Self::new_with_components(
            Arc::new(WindowsCredentialStore),
            DEEPSEEK_COMPLETIONS_URL.to_owned(),
            CONNECT_TIMEOUT,
            REQUEST_TIMEOUT,
            MAX_RESPONSE_BYTES,
            true,
        )
    }

    #[cfg(test)]
    fn new_for_test<T>(
        credential_store: Arc<T>,
        endpoint: String,
        request_timeout: Duration,
        response_limit: usize,
    ) -> Result<Self, AiErrorDto>
    where
        T: AiCredentialStore + 'static,
    {
        Self::new_with_components(
            credential_store,
            endpoint,
            request_timeout,
            request_timeout,
            response_limit,
            false,
        )
    }

    fn new_with_components(
        credential_store: Arc<dyn AiCredentialStore>,
        endpoint: String,
        connect_timeout: Duration,
        request_timeout: Duration,
        response_limit: usize,
        https_only: bool,
    ) -> Result<Self, AiErrorDto> {
        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .https_only(https_only)
            .build()
            .map_err(|_| AiErrorDto {
                code: AiErrorCode::AiUnavailable,
                message: "无法初始化 AI 桌面通道".to_owned(),
            })?;
        Ok(Self {
            client,
            credential_store,
            endpoint,
            response_limit,
            active_requests: Mutex::new(HashMap::new()),
        })
    }

    pub fn credential_status(&self) -> Result<AiCredentialStatusDto, AiErrorDto> {
        let api_key = self
            .credential_store
            .get_api_key()
            .map_err(|_| AiErrorDto::credential_store())?;
        Ok(AiCredentialStatusDto {
            configured: api_key.is_some_and(|key| !key.is_empty()),
        })
    }

    pub fn set_api_key(&self, api_key: &str) -> Result<(), AiErrorDto> {
        validate_api_key(api_key)?;
        self.credential_store
            .set_api_key(api_key)
            .map_err(|_| AiErrorDto::credential_store())
    }

    pub fn clear_api_key(&self) -> Result<(), AiErrorDto> {
        self.credential_store
            .clear_api_key()
            .map_err(|_| AiErrorDto::credential_store())
    }

    pub async fn complete(&self, request: AiCompletionRequestDto) -> Result<String, AiErrorDto> {
        request.validate()?;
        let cancellation = CancellationToken::new();
        {
            let mut active = self
                .active_requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match active.entry(request.request_id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(cancellation.clone());
                }
                Entry::Occupied(_) => {
                    return Err(AiErrorDto::invalid_request("请求 ID 正在使用"));
                }
            }
        }

        let _active_request_guard = ActiveRequestGuard {
            request_id: request.request_id.clone(),
            active_requests: &self.active_requests,
        };

        tokio::select! {
            _ = cancellation.cancelled() => Err(AiErrorDto::cancelled()),
            result = self.send_completion(&request) => result,
        }
    }

    pub fn cancel(&self, request_id: &str) {
        if let Some(token) = self
            .active_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(request_id)
            .cloned()
        {
            token.cancel();
        }
    }

    async fn send_completion(
        &self,
        request: &AiCompletionRequestDto,
    ) -> Result<String, AiErrorDto> {
        let api_key = self
            .credential_store
            .get_api_key()
            .map_err(|_| AiErrorDto::credential_store())?
            .ok_or_else(AiErrorDto::key_missing)?;

        #[derive(Serialize)]
        struct DeepSeekRequest<'a> {
            model: &'a str,
            messages: &'a [AiChatMessageDto],
            temperature: u8,
            response_format: DeepSeekResponseFormat,
        }
        #[derive(Serialize)]
        struct DeepSeekResponseFormat {
            r#type: &'static str,
        }

        let mut response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&api_key)
            .json(&DeepSeekRequest {
                model: request.model.trim(),
                messages: &request.messages,
                temperature: 0,
                response_format: DeepSeekResponseFormat {
                    r#type: "json_object",
                },
            })
            .send()
            .await
            .map_err(map_reqwest_error)?;

        if !response.status().is_success() {
            return Err(AiErrorDto::http_error(Some(response.status().as_u16())));
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.response_limit as u64)
        {
            return Err(AiErrorDto::response_too_large());
        }

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
            if body.len().saturating_add(chunk.len()) > self.response_limit {
                return Err(AiErrorDto::response_too_large());
            }
            body.extend_from_slice(&chunk);
        }

        #[derive(Deserialize)]
        struct DeepSeekResponse {
            choices: Vec<DeepSeekChoice>,
        }
        #[derive(Deserialize)]
        struct DeepSeekChoice {
            message: DeepSeekMessage,
        }
        #[derive(Deserialize)]
        struct DeepSeekMessage {
            content: String,
        }

        let response: DeepSeekResponse =
            serde_json::from_slice(&body).map_err(|_| AiErrorDto::invalid_response())?;
        response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(AiErrorDto::invalid_response)
    }
}

fn validate_api_key(api_key: &str) -> Result<(), AiErrorDto> {
    if api_key.is_empty()
        || api_key.len() > MAX_API_KEY_BYTES
        || api_key.trim().is_empty()
        || api_key.chars().any(char::is_control)
    {
        return Err(AiErrorDto::invalid_request(
            "API Key 必须为 1 到 512 字节且不能包含控制字符",
        ));
    }
    Ok(())
}

fn map_reqwest_error(error: reqwest::Error) -> AiErrorDto {
    if error.is_timeout() {
        AiErrorDto::timeout()
    } else {
        AiErrorDto::http_error(None)
    }
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn ai_credential_status(
    state: tauri::State<'_, AiServiceState>,
) -> Result<AiCredentialStatusDto, AiErrorDto> {
    state.credential_status()
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn ai_set_api_key(
    state: tauri::State<'_, AiServiceState>,
    api_key: String,
) -> Result<(), AiErrorDto> {
    state.set_api_key(&api_key)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn ai_clear_api_key(state: tauri::State<'_, AiServiceState>) -> Result<(), AiErrorDto> {
    state.clear_api_key()
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub async fn ai_complete(
    state: tauri::State<'_, AiServiceState>,
    request: AiCompletionRequestDto,
) -> Result<String, AiErrorDto> {
    state.complete(request).await
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn ai_cancel(state: tauri::State<'_, AiServiceState>, request_id: String) {
    state.cancel(&request_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    #[derive(Default)]
    struct MemoryCredentialStore {
        api_key: Mutex<Option<String>>,
        fail_reads: Mutex<bool>,
    }

    impl AiCredentialStore for MemoryCredentialStore {
        fn get_api_key(&self) -> Result<Option<String>, CredentialStoreFailure> {
            if *self.fail_reads.lock().expect("read failure lock") {
                return Err(CredentialStoreFailure);
            }
            Ok(self.api_key.lock().expect("api key lock").clone())
        }

        fn set_api_key(&self, api_key: &str) -> Result<(), CredentialStoreFailure> {
            *self.api_key.lock().expect("api key lock") = Some(api_key.to_owned());
            Ok(())
        }

        fn clear_api_key(&self) -> Result<(), CredentialStoreFailure> {
            *self.api_key.lock().expect("api key lock") = None;
            Ok(())
        }
    }

    fn completion_request() -> AiCompletionRequestDto {
        AiCompletionRequestDto {
            request_id: uuid::Uuid::new_v4().to_string(),
            model: "deepseek-chat".to_owned(),
            messages: vec![AiChatMessageDto {
                role: "user".to_owned(),
                content: "Tune the loop".to_owned(),
            }],
        }
    }

    async fn spawn_http_response(
        status: &str,
        extra_headers: &[(&str, &str)],
        body: Vec<u8>,
        delay: Duration,
    ) -> (String, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let (request_tx, request_rx) = oneshot::channel();
        let status = status.to_owned();
        let headers = extra_headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<Vec<_>>();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept test request");
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let count = stream.read(&mut buffer).await.expect("read test request");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let _ = request_tx.send(request);
            tokio::time::sleep(delay).await;
            let mut response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len()
            );
            for (name, value) in headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            if stream.write_all(response.as_bytes()).await.is_ok() {
                let _ = stream.write_all(&body).await;
            }
        });

        (format!("http://{address}/chat/completions"), request_rx)
    }

    fn service(
        store: Arc<MemoryCredentialStore>,
        endpoint: String,
        timeout: Duration,
        response_limit: usize,
    ) -> AiServiceState {
        AiServiceState::new_for_test(store, endpoint, timeout, response_limit)
            .expect("construct test service")
    }

    #[test]
    fn serializes_stable_error_codes() {
        let error = AiErrorDto {
            code: AiErrorCode::AiInvalidRequest,
            message: "bad request".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(error).unwrap()["code"],
            "aiInvalidRequest"
        );
    }

    #[tokio::test]
    async fn stores_reports_and_clears_credentials_without_returning_the_key() {
        let store = Arc::new(MemoryCredentialStore::default());
        let state = service(
            Arc::clone(&store),
            "http://127.0.0.1:1/chat/completions".to_owned(),
            Duration::from_secs(1),
            1024,
        );

        assert!(!state.credential_status().unwrap().configured);
        state.set_api_key("sk-test-secret").unwrap();
        assert!(state.credential_status().unwrap().configured);
        state.clear_api_key().unwrap();
        assert!(!state.credential_status().unwrap().configured);

        for invalid in ["", "  ", "line\nbreak", &"x".repeat(513)] {
            assert_eq!(
                state.set_api_key(invalid).unwrap_err().code,
                AiErrorCode::AiInvalidRequest
            );
        }
    }

    #[tokio::test]
    async fn completes_a_valid_request_and_sends_bearer_auth_to_the_fixed_client() {
        let body = br#"{"choices":[{"message":{"content":"Use Kp 1.4"}}]}"#.to_vec();
        let (endpoint, request_rx) = spawn_http_response("200 OK", &[], body, Duration::ZERO).await;
        let store = Arc::new(MemoryCredentialStore::default());
        let state = service(store, endpoint, Duration::from_secs(1), 1024);
        state.set_api_key("sk-test-secret").unwrap();

        let answer = state.complete(completion_request()).await.unwrap();
        assert_eq!(answer, "Use Kp 1.4");
        let request = String::from_utf8(request_rx.await.unwrap()).unwrap();
        assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-test-secret"));
    }

    #[tokio::test]
    async fn maps_http_statuses_without_including_response_bodies_or_keys() {
        for status in [
            "401 Unauthorized",
            "429 Too Many Requests",
            "500 Internal Server Error",
        ] {
            let (endpoint, _) = spawn_http_response(
                status,
                &[],
                b"server echoed sk-test-secret".to_vec(),
                Duration::ZERO,
            )
            .await;
            let store = Arc::new(MemoryCredentialStore::default());
            let state = service(store, endpoint, Duration::from_secs(1), 1024);
            state.set_api_key("sk-test-secret").unwrap();

            let error = state.complete(completion_request()).await.unwrap_err();
            assert_eq!(error.code, AiErrorCode::AiHttpError);
            let rendered = format!("{error:?}");
            assert!(!rendered.contains("sk-test-secret"));
            assert!(!rendered.contains("server echoed"));
        }
    }

    #[tokio::test]
    async fn rejects_redirects_without_following_them() {
        let (redirect_target, target_rx) = spawn_http_response(
            "200 OK",
            &[],
            br#"{"choices":[{"message":{"content":"must not arrive"}}]}"#.to_vec(),
            Duration::ZERO,
        )
        .await;
        let (endpoint, _) = spawn_http_response(
            "302 Found",
            &[("Location", redirect_target.as_str())],
            Vec::new(),
            Duration::ZERO,
        )
        .await;
        let store = Arc::new(MemoryCredentialStore::default());
        let state = service(store, endpoint, Duration::from_secs(1), 1024);
        state.set_api_key("sk-test-secret").unwrap();

        assert_eq!(
            state.complete(completion_request()).await.unwrap_err().code,
            AiErrorCode::AiHttpError
        );
        assert!(tokio::time::timeout(Duration::from_millis(50), target_rx)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn enforces_timeout_cancellation_and_response_limit() {
        let (slow_endpoint, _) = spawn_http_response(
            "200 OK",
            &[],
            br#"{"choices":[{"message":{"content":"late"}}]}"#.to_vec(),
            Duration::from_millis(100),
        )
        .await;
        let store = Arc::new(MemoryCredentialStore::default());
        let slow = service(store, slow_endpoint, Duration::from_millis(20), 1024);
        slow.set_api_key("sk-test-secret").unwrap();
        assert_eq!(
            slow.complete(completion_request()).await.unwrap_err().code,
            AiErrorCode::AiTimeout
        );

        let (cancel_endpoint, _) = spawn_http_response(
            "200 OK",
            &[],
            br#"{"choices":[{"message":{"content":"late"}}]}"#.to_vec(),
            Duration::from_millis(500),
        )
        .await;
        let store = Arc::new(MemoryCredentialStore::default());
        let cancelled = Arc::new(service(
            store,
            cancel_endpoint,
            Duration::from_secs(1),
            1024,
        ));
        cancelled.set_api_key("sk-test-secret").unwrap();
        let request = completion_request();
        let request_id = request.request_id.clone();
        let task_state = Arc::clone(&cancelled);
        let task = tokio::spawn(async move { task_state.complete(request).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancelled.cancel(&request_id);
        assert_eq!(
            task.await.unwrap().unwrap_err().code,
            AiErrorCode::AiCancelled
        );

        let (large_endpoint, _) =
            spawn_http_response("200 OK", &[], vec![b'x'; 65], Duration::ZERO).await;
        let store = Arc::new(MemoryCredentialStore::default());
        let large = service(store, large_endpoint, Duration::from_secs(1), 64);
        large.set_api_key("sk-test-secret").unwrap();
        assert_eq!(
            large.complete(completion_request()).await.unwrap_err().code,
            AiErrorCode::AiResponseTooLarge
        );
    }

    #[tokio::test]
    async fn rejects_invalid_json_missing_keys_and_duplicate_request_ids() {
        let (bad_json_endpoint, _) =
            spawn_http_response("200 OK", &[], b"not json".to_vec(), Duration::ZERO).await;
        let store = Arc::new(MemoryCredentialStore::default());
        let invalid_json = service(store, bad_json_endpoint, Duration::from_secs(1), 1024);
        invalid_json.set_api_key("sk-test-secret").unwrap();
        assert_eq!(
            invalid_json
                .complete(completion_request())
                .await
                .unwrap_err()
                .code,
            AiErrorCode::AiInvalidResponse
        );

        let store = Arc::new(MemoryCredentialStore::default());
        let missing_key = service(
            store,
            "http://127.0.0.1:1/chat/completions".to_owned(),
            Duration::from_secs(1),
            1024,
        );
        assert_eq!(
            missing_key
                .complete(completion_request())
                .await
                .unwrap_err()
                .code,
            AiErrorCode::AiKeyMissing
        );

        let (duplicate_endpoint, _) = spawn_http_response(
            "200 OK",
            &[],
            br#"{"choices":[{"message":{"content":"late"}}]}"#.to_vec(),
            Duration::from_millis(200),
        )
        .await;
        let store = Arc::new(MemoryCredentialStore::default());
        let duplicate = Arc::new(service(
            store,
            duplicate_endpoint,
            Duration::from_secs(1),
            1024,
        ));
        duplicate.set_api_key("sk-test-secret").unwrap();
        let request = completion_request();
        let first_state = Arc::clone(&duplicate);
        let first_request = request.clone();
        let first = tokio::spawn(async move { first_state.complete(first_request).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            duplicate.complete(request).await.unwrap_err().code,
            AiErrorCode::AiInvalidRequest
        );
        first.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn credential_store_failures_are_generic_and_never_expose_secret_material() {
        let store = Arc::new(MemoryCredentialStore::default());
        *store.fail_reads.lock().unwrap() = true;
        let state = service(
            store,
            "http://127.0.0.1:1/chat/completions".to_owned(),
            Duration::from_secs(1),
            1024,
        );
        let error = state.complete(completion_request()).await.unwrap_err();
        assert_eq!(error.code, AiErrorCode::AiCredentialStoreError);
        assert_eq!(error.message, "无法访问 Windows 凭据库");
    }

    #[tokio::test]
    async fn dropping_an_inflight_completion_removes_its_request_id() {
        let (endpoint, _) = spawn_http_response(
            "200 OK",
            &[],
            br#"{"choices":[{"message":{"content":"late"}}]}"#.to_vec(),
            Duration::from_millis(500),
        )
        .await;
        let store = Arc::new(MemoryCredentialStore::default());
        let state = Arc::new(service(store, endpoint, Duration::from_secs(1), 1024));
        state.set_api_key("sk-test-secret").unwrap();
        let request = completion_request();
        let request_id = request.request_id.clone();
        let task_state = Arc::clone(&state);
        let task = tokio::spawn(async move { task_state.complete(request).await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        task.abort();
        let _ = task.await;

        assert!(!state
            .active_requests
            .lock()
            .unwrap()
            .contains_key(&request_id));
    }
}
