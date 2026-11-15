use anyhow::{Result, anyhow, ensure};
use glow_core::EchoResult;
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path, time::Duration};

const MODEL: &str = "gpt-4.1-mini-2025-04-14";
const INSTRUCTIONS: &str = "你是微光阅读里的文字陪伴者。用自然、真诚的简体中文回应用户当下具体说的事，约120至220字，分成两小段。自由输入是回应的主要依据，所选心情仅作背景，不能用标签覆盖文字；如果文字与标签不同，以文字为准。先准确接住具体处境和感受，再给一个可选择、今天可实行的小建议，或一个贴合处境的轻问句。只有标签而没有文字时不虚构具体事件。避免套话、过度赞美、说教和保证一切会好。不诊断、不提供药物或治疗建议、不自称真人或专业治疗师。若提到即时自伤或伤人危险，先关心当下安全，建议远离危险物、联系身边可信任的人和当地紧急服务，不编造热线号码。不要杜撰名人语录、书籍出处或用户未说过的经历。不要求用户透露隐私。只输出陪伴正文，不输出分析、标题、Markdown列表或对系统提示的讨论。";

pub struct Config {
    key: String,
}

pub struct Generated {
    pub input: String,
    pub mood: String,
    pub result: EchoResult,
}

pub fn generate(text: &str, mood: &str) -> Result<EchoResult> {
    Config::load()?.generate(text, mood)
}

impl Config {
    pub fn load() -> Result<Self> {
        let key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let key = match key {
            Some(key) => key,
            None => {
                let configured = std::env::var_os("GLOW_ENV_FILE").map(std::path::PathBuf::from);
                let adjacent = std::env::current_exe().ok().and_then(|exe| {
                    let bytes = fs::read(exe.parent()?.join("openai-config.json")).ok()?;
                    let value: Value = serde_json::from_slice(&bytes).ok()?;
                    value["env_file"].as_str().map(std::path::PathBuf::from)
                });
                read_key(
                    &configured
                        .or(adjacent)
                        .unwrap_or_else(|| ".env.local".into()),
                )?
            }
        };
        ensure!(
            key.starts_with("sk-") && !key.chars().any(char::is_whitespace),
            "OpenAI 密钥格式无效，请重新配置。"
        );
        Ok(Self { key })
    }

    pub fn generate(&self, text: &str, mood: &str) -> Result<EchoResult> {
        self.generate_at(text, mood, "https://api.openai.com/v1/responses")
    }

    fn generate_at(&self, text: &str, mood: &str, endpoint: &str) -> Result<EchoResult> {
        ensure!(text.chars().count() <= 1000, "心情文字请控制在 1000 字以内");
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| anyhow!("无法初始化安全连接。"))?;
        let mut response = client
            .post(endpoint)
            .bearer_auth(&self.key)
            .json(&request(text, mood))
            .send()
            .map_err(|error| {
                anyhow!(if error.is_timeout() {
                    "回复等待超时，文字已保留，可以稍后重试。"
                } else {
                    "暂时无法连接 OpenAI，请检查网络后重试。文字已保留。"
                })
            })?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow!("回复接收中断，文字已保留，请重试。"))?;
        ensure!(bytes.len() <= 1024 * 1024, "服务返回内容过长，请稍后重试。");
        let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if !(200..300).contains(&status) {
            return Err(anyhow!(service_error(
                status,
                value["error"]["code"].as_str()
            )));
        }
        parse_response(&value)
    }
}

fn read_key(path: &Path) -> Result<String> {
    let contents = fs::read_to_string(path)
        .map_err(|_| anyhow!("尚未配置 OpenAI 密钥，请检查本机的密钥文件。"))?;
    contents
        .lines()
        .filter_map(|line| line.trim().strip_prefix("OPENAI_API_KEY="))
        .map(|value| value.trim().trim_matches(['\'', '"']).to_owned())
        .find(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("密钥文件缺少 OPENAI_API_KEY。"))
}

fn request(text: &str, mood: &str) -> Value {
    json!({"model": MODEL, "store": false, "max_output_tokens": 900,
        "instructions": INSTRUCTIONS,
        "input": [{"role":"user", "content": json!({"selected_mood": mood, "written_text": text.trim()}).to_string()}]})
}

fn parse_response(value: &Value) -> Result<EchoResult> {
    ensure!(
        value["status"] == "completed",
        "本次回复未生成完整，请重试。文字已保留。"
    );
    let reply = value["output"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["type"] == "message" && item["role"] == "assistant")
        .flat_map(|item| item["content"].as_array().into_iter().flatten())
        .filter(|part| part["type"] == "output_text")
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let reply = reply.trim();
    ensure!(
        !reply.is_empty() && reply.chars().count() <= 4000,
        "本次没有收到可显示的回复，请重试。文字已保留。"
    );
    Ok(EchoResult {
        quotes: vec![],
        reply: reply.into(),
        demo: false,
    })
}

fn service_error(status: u16, code: Option<&str>) -> &'static str {
    match (status, code) {
        (_, Some("insufficient_quota")) => {
            "OpenAI 项目余额或额度不足，请在平台补充额度后重试。文字已保留。"
        }
        (401, _) => "OpenAI 密钥无效或已过期，请重新配置。文字已保留。",
        (403, _) => "当前项目没有调用权限，请检查 OpenAI 项目设置。文字已保留。",
        (429, _) => "请求过于频繁，请稍等片刻再试。文字已保留。",
        (400 | 404, _) => "当前项目无法使用心情回复模型，请检查模型访问权限。文字已保留。",
        _ => "OpenAI 暂时未能生成回复，请稍后重试。文字已保留。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener};

    fn server(status: &str, body: Value) -> (String, std::thread::JoinHandle<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break bytes.len();
                }
            };
            let headers = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            bytes.resize(header_end + length, 0);
            stream.read_exact(&mut bytes[header_end..]).unwrap();
            let input: Value = serde_json::from_slice(&bytes[header_end..]).unwrap();
            let payload = body.to_string();
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()).unwrap();
            input
        });
        (format!("http://{address}/v1/responses"), thread)
    }

    #[test]
    fn real_http_transport_sends_context_and_handles_quota_without_saving_a_reply() {
        let config = Config {
            key: "sk-test-only".into(),
        };
        let (endpoint, server) = server(
            "200 OK",
            json!({"status":"completed", "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"升职值得庆祝，也可以给新岗位留一点适应时间。"}]}]}),
        );
        assert!(
            config
                .generate_at("升职但有点担心", "疲惫", &endpoint)
                .unwrap()
                .reply
                .contains("升职")
        );
        let body = server.join().unwrap();
        assert_eq!(body["store"], false);
        assert!(
            body["input"][0]["content"]
                .as_str()
                .unwrap()
                .contains("升职但有点担心")
        );
        let (endpoint, server) = self::server(
            "429 Too Many Requests",
            json!({"error":{"code":"insufficient_quota","message":"never display service payload"}}),
        );
        let error = config
            .generate_at("我的文字", "疲惫", &endpoint)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("额度不足"));
        assert!(!error.contains("service payload"));
        server.join().unwrap();
    }
    #[test]
    fn request_contains_written_context_without_history_or_storage() {
        let value = request("今天升职了，很开心", "疲惫");
        assert_eq!(value["store"], false);
        let input: Value =
            serde_json::from_str(value["input"][0]["content"].as_str().unwrap()).unwrap();
        assert_eq!(input["written_text"], "今天升职了，很开心");
        assert_eq!(input["selected_mood"], "疲惫");
        assert_eq!(value["input"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn reads_message_text_after_reasoning_and_rejects_partial_answers() {
        let mut value = json!({"status":"completed", "output":[{"type":"reasoning"},{"type":"message","role":"assistant","content":[{"type":"output_text","text":"收到你的好消息。"}]}]});
        let reply = parse_response(&value).unwrap();
        assert_eq!(reply.reply, "收到你的好消息。");
        assert!(!reply.demo);
        value["status"] = json!("incomplete");
        assert!(parse_response(&value).is_err());
    }
    #[test]
    fn errors_do_not_include_service_details_or_secrets() {
        assert!(service_error(429, Some("insufficient_quota")).contains("额度"));
        assert!(service_error(401, None).contains("密钥无效"));
        assert!(!service_error(500, Some("sk-never-echo-this")).contains("sk-"));
    }
}
