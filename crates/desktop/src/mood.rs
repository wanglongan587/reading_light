use anyhow::{Result, anyhow, ensure};
use glow_core::{
    AiSettings, BookCompanionCache, CharacterNode, CharacterRelation, EchoResult, LocalSecrets,
    book_context::SpoilerSafeContext, model::now,
};
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path, time::Duration};

const MODEL: &str = "gpt-4.1-mini-2025-04-14";
const INSTRUCTIONS: &str = "你是微光阅读里的文字陪伴者。用自然、真诚的简体中文回应用户当下具体说的事，约120至220字，分成两小段。自由输入是回应的主要依据，所选心情仅作背景，不能用标签覆盖文字；如果文字与标签不同，以文字为准。先准确接住具体处境和感受，再给一个可选择、今天可实行的小建议，或一个贴合处境的轻问句。只有标签而没有文字时不虚构具体事件。避免套话、过度赞美、说教和保证一切会好。不诊断、不提供药物或治疗建议、不自称真人或专业治疗师。若提到即时自伤或伤人危险，先关心当下安全，建议远离危险物、联系身边可信任的人和当地紧急服务，不编造热线号码。不要杜撰名人语录、书籍出处或用户未说过的经历。不要求用户透露隐私。只输出陪伴正文，不输出分析、标题、Markdown列表或对系统提示的讨论。";
const PASSAGE_INSTRUCTIONS: &str = "你是微光阅读里的段落共读陪伴者。用自然、细腻、有文学温度的简体中文，围绕读者刚在书中划出的这段文字展开一次安静的共读，约120至220字，分成两小段。第一段感受这段文字里的意象、节奏或情绪留白；第二段给出一句温柔的延伸体会或一个适合停下来想一想的轻问句。不剧透后续情节，不编造原书未写的内容或虚构名人名言，避免说教和套话。只输出共读正文，不输出标题或Markdown列表。";
const COMPANION_INSTRUCTIONS: &str = "你是微光阅读里的防剧透文学共读助手。你只被允许依据用户提供的【已读正文摘录】来生成前情提要与动态人物关系图谱。\n严格铁律：\n1. 绝对禁止剧透任何未出现在已读摘录里的后续情节、隐藏身份、死亡或大结局！哪怕你原本读过这本书，也必须假装你只看到这一页为止。\n2. 严禁编造人物或事件；如果已读片段较短，只提取已明确登场的人物。\n3. 必须严格输出合法 JSON（不要包裹在多余解释里），字段结构如下：\n{\"headline\":\"一句不超过24字的温柔文学标题\",\"story_summary\":\"140至240字的前情提要，温和有文学质感，帮助读者3秒找回故事脉络\",\"open_threads\":[\"截至目前已埋下但尚未揭晓的悬念或伏笔1\",\"悬念2\"],\"resume_moment\":\"40至80字，精准描述读者上次停下来的画面或瞬间\",\"characters\":[{\"name\":\"人物姓名\",\"aliases\":[\"简称或外号\"],\"role\":\"一句话身份定位\",\"traits\":[\"关键词1\",\"关键词2\"],\"status\":\"截至当前的处境或心境\",\"first_seen\":\"登场章节或场景\"}],\"relations\":[{\"from\":\"人物A\",\"to\":\"人物B\",\"label\":\"简短关系词（2-8字）\",\"tone\":\"bond|family|conflict|mystery\"}]}\n4. characters 最多返回 8 位最核心人物，relations 最多返回 10 条最清晰关系，确保 relations 的 from 和 to 都来自 characters 中的 name。";

pub struct Config {
    provider: String,
    base_url: String,
    model: String,
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

pub fn generate_passage(passage: &str, book_title: &str) -> Result<EchoResult> {
    Config::load()?.generate_passage(passage, book_title)
}

pub fn generate_with_settings(
    settings: &AiSettings,
    secrets: &LocalSecrets,
    text: &str,
    mood: &str,
) -> Result<EchoResult> {
    Config::from_settings(settings, secrets)?.generate(text, mood)
}

pub fn generate_passage_with_settings(
    settings: &AiSettings,
    secrets: &LocalSecrets,
    passage: &str,
    book_title: &str,
) -> Result<EchoResult> {
    Config::from_settings(settings, secrets)?.generate_passage(passage, book_title)
}

pub fn generate_book_companion(
    settings: &AiSettings,
    secrets: &LocalSecrets,
    ctx: &SpoilerSafeContext,
) -> Result<BookCompanionCache> {
    Config::from_settings(settings, secrets)?.generate_companion(ctx)
}

#[derive(Debug, Clone)]
pub struct ConnectionCheck {
    pub summary: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedEndpoints {
    use_responses: bool,
    completion_url: String,
    models_url: String,
    ollama_tags_url: Option<String>,
}

fn resolve_endpoints(provider: &str, raw_url: &str) -> Result<ResolvedEndpoints> {
    let trimmed = raw_url.trim().trim_end_matches('/');
    ensure!(
        trimmed.starts_with("http://") || trimmed.starts_with("https://"),
        "大模型 URL 必须以 http:// 或 https:// 开头。"
    );
    ensure!(
        !trimmed.chars().any(char::is_whitespace),
        "大模型 URL 不能包含空格或换行符。"
    );

    let lower = trimmed.to_ascii_lowercase();
    let looks_ollama = provider == "ollama" || lower.contains(":11434");

    // 1. User pasted a full Responses API endpoint directly (e.g. https://proxy.example/v1/responses)
    if lower.ends_with("/responses") {
        let base = &trimmed[..trimmed.len() - "/responses".len()];
        return Ok(ResolvedEndpoints {
            use_responses: true,
            completion_url: trimmed.to_owned(),
            models_url: format!("{base}/models"),
            ollama_tags_url: None,
        });
    }

    // 2. User pasted a full Chat Completions endpoint directly (e.g. https://proxy.example/v1/chat/completions)
    if lower.ends_with("/chat/completions") {
        let base = &trimmed[..trimmed.len() - "/chat/completions".len()];
        let ollama_tags_url = looks_ollama.then(|| {
            let root = base.trim_end_matches("/v1").trim_end_matches('/');
            format!("{root}/api/tags")
        });
        return Ok(ResolvedEndpoints {
            use_responses: false,
            completion_url: trimmed.to_owned(),
            models_url: format!("{base}/models"),
            ollama_tags_url,
        });
    }

    // 3. Official OpenAI Base URL defaults to OpenAI Responses API
    if provider == "openai" && lower.contains("api.openai.com") {
        let base = if lower.ends_with("/v1") {
            trimmed.to_owned()
        } else {
            format!("{trimmed}/v1")
        };
        return Ok(ResolvedEndpoints {
            use_responses: true,
            completion_url: format!("{base}/responses"),
            models_url: format!("{base}/models"),
            ollama_tags_url: None,
        });
    }

    // 4. Ollama root URL without /v1 (e.g. http://127.0.0.1:11434 or http://192.168.1.8:11434)
    if looks_ollama {
        let root = trimmed
            .trim_end_matches("/api/chat")
            .trim_end_matches("/api/tags")
            .trim_end_matches("/v1")
            .trim_end_matches('/');
        let base = format!("{root}/v1");
        return Ok(ResolvedEndpoints {
            use_responses: false,
            completion_url: format!("{base}/chat/completions"),
            models_url: format!("{base}/models"),
            ollama_tags_url: Some(format!("{root}/api/tags")),
        });
    }

    // 5. Generic OpenAI-compatible Base URL (e.g. https://api.deepseek.com/v1,
    //    https://openrouter.ai/api/v1, https://generativelanguage.googleapis.com/v1beta/openai,
    //    http://127.0.0.1:8080/v1, https://my-oneapi.example.com)
    let after_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let has_path = after_scheme.contains('/');
    let base = if has_path {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/v1")
    };
    Ok(ResolvedEndpoints {
        use_responses: false,
        completion_url: format!("{base}/chat/completions"),
        models_url: format!("{base}/models"),
        ollama_tags_url: None,
    })
}

/// Test connectivity to the configured AI provider / custom URL and discover available models.
pub fn test_connection(settings: &AiSettings, secrets: &LocalSecrets) -> Result<ConnectionCheck> {
    let raw_url = normalized_base_url(settings);
    let endpoints = resolve_endpoints(&settings.provider, &raw_url)?;
    let client = build_http_client(15)?;

    if let Some(tags_url) = &endpoints.ollama_tags_url {
        let resp = client.get(tags_url).send().map_err(|_| {
            anyhow!("未检测到 Ollama 离线服务（{tags_url}）。请确认已启动 Ollama，或检查局域网地址。")
        })?;
        ensure!(
            resp.status().is_success(),
            "Ollama 服务返回状态 {}，请检查本机服务状态。",
            resp.status().as_u16()
        );
        let value: Value = resp.json().unwrap_or(Value::Null);
        let models: Vec<String> = value["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["name"].as_str().map(str::to_owned))
            .take(16)
            .collect();
        if models.is_empty() {
            return Ok(ConnectionCheck {
                summary:
                    "已连上 Ollama 离线服务，但尚未下载模型。可在终端运行：ollama pull qwen2.5:7b"
                        .into(),
                models: vec![],
            });
        }
        let sample = models
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("、");
        return Ok(ConnectionCheck {
            summary: format!(
                "Ollama 离线连接成功！检测到 {} 个可用模型（点击下方标签可一键选用）：{sample}",
                models.len()
            ),
            models,
        });
    }

    let cfg = Config::from_settings(settings, secrets)?;
    let mut req = client.get(&endpoints.models_url);
    if !cfg.key.is_empty() {
        req = req.bearer_auth(&cfg.key);
    }
    if let Ok(resp) = req.send()
        && resp.status().is_success()
    {
        let value: Value = resp.json().unwrap_or(Value::Null);
        let models: Vec<String> = value["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .take(16)
            .collect();
        let summary = if models.is_empty() {
            format!(
                "连接成功！当前引擎：{} · 模型：{}",
                provider_display_name(&cfg.provider),
                cfg.model
            )
        } else {
            format!(
                "{} 连接成功！发现 {} 个可用模型（可点击下方标签快速切换）。",
                provider_display_name(&cfg.provider),
                models.len()
            )
        };
        return Ok(ConnectionCheck { summary, models });
    }

    // Fallback when relay/provider disables GET /models: run a minimal 1-sentence live probe
    let probe = cfg.complete_text(
        "你是一个连接测试助手。只回复：连接正常。",
        "请确认连接。",
        32,
    )?;
    let preview = probe.chars().take(24).collect::<String>();
    Ok(ConnectionCheck {
        summary: format!(
            "{} 连接正常（模型 {} 回复：{preview}）",
            provider_display_name(&cfg.provider),
            cfg.model
        ),
        models: vec![],
    })
}

pub fn provider_display_name(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "DeepSeek 深度求索",
        "qwen" => "通义千问 DashScope",
        "siliconflow" => "硅基流动 SiliconFlow",
        "ollama" => "Ollama 本地离线大模型",
        "lmstudio" => "LM Studio 本地离线大模型",
        "llamacpp" => "llama.cpp 本地离线服务",
        "custom" => "自定义大模型 URL",
        _ => "OpenAI",
    }
}

fn normalized_base_url(settings: &AiSettings) -> String {
    let trimmed = settings.base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        AiSettings::for_preset(&settings.provider).base_url
    } else {
        trimmed.to_owned()
    }
}

fn build_http_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| anyhow!("无法初始化安全连接。"))
}

impl Config {
    pub fn from_settings(settings: &AiSettings, secrets: &LocalSecrets) -> Result<Self> {
        let provider = if settings.provider.trim().is_empty() {
            "openai".to_owned()
        } else {
            settings.provider.trim().to_owned()
        };
        let base_url = normalized_base_url(settings);
        let model = if settings.model.trim().is_empty() {
            AiSettings::for_preset(&provider).model
        } else {
            settings.model.trim().to_owned()
        };

        let is_local = settings.is_local();
        let key_optional = is_local || provider == "custom";
        let _ = resolve_endpoints(&provider, &base_url)?;
        let saved_key = secrets.ai_api_key.trim().to_owned();
        let key = if !saved_key.is_empty() {
            saved_key
        } else if key_optional {
            String::new()
        } else if provider == "openai" {
            // Seamlessly fall back to OPENAI_API_KEY / .env.local for existing setups
            Self::load_env_openai_key().map_err(|_| {
                anyhow!("尚未配置 AI 密钥。请前往「主题与设置」填写 DeepSeek / 通义千问 / OpenAI 密钥，或切换至自定义 URL / 本机离线大模型。")
            })?
        } else {
            return Err(anyhow!(
                "尚未填写 {} 的 API Key，请前往「主题与设置 → AI 共读引擎」中保存密钥。",
                provider_display_name(&provider)
            ));
        };

        if !key.is_empty() {
            ensure!(
                !key.chars().any(char::is_whitespace),
                "API 密钥包含非法空白字符，请重新检查。"
            );
        }

        Ok(Self {
            provider,
            base_url,
            model,
            key,
        })
    }

    fn load_env_openai_key() -> Result<String> {
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
        Ok(key)
    }

    pub fn load() -> Result<Self> {
        let key = Self::load_env_openai_key()?;
        Ok(Self {
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: MODEL.into(),
            key,
        })
    }

    fn endpoints(&self) -> Result<ResolvedEndpoints> {
        resolve_endpoints(&self.provider, &self.base_url)
    }

    pub fn generate(&self, text: &str, mood: &str) -> Result<EchoResult> {
        ensure!(text.chars().count() <= 1000, "心情文字请控制在 1000 字以内");
        let ep = self.endpoints()?;
        if ep.use_responses {
            self.post_responses_json(
                &request_with_model(&self.model, text, mood),
                &ep.completion_url,
            )
        } else {
            let user_payload =
                json!({"selected_mood": mood, "written_text": text.trim()}).to_string();
            let reply = self.complete_chat_at(&ep.completion_url, INSTRUCTIONS, &user_payload, 900)?;
            Ok(EchoResult {
                quotes: vec![],
                reply,
                demo: false,
            })
        }
    }

    pub fn generate_passage(&self, passage: &str, book_title: &str) -> Result<EchoResult> {
        let trimmed = passage.trim();
        ensure!(!trimmed.is_empty(), "请先选择一段要共读的文字");
        ensure!(trimmed.chars().count() <= 2000, "选文请控制在 2000 字以内");
        let ep = self.endpoints()?;
        if ep.use_responses {
            self.post_responses_json(
                &passage_request_with_model(&self.model, trimmed, book_title),
                &ep.completion_url,
            )
        } else {
            let user_payload =
                json!({"book_title": book_title.trim(), "selected_passage": trimmed}).to_string();
            let reply =
                self.complete_chat_at(&ep.completion_url, PASSAGE_INSTRUCTIONS, &user_payload, 900)?;
            Ok(EchoResult {
                quotes: vec![],
                reply,
                demo: false,
            })
        }
    }

    pub fn generate_companion(&self, ctx: &SpoilerSafeContext) -> Result<BookCompanionCache> {
        let user_prompt = format!(
            "书名：《{}》\n作者：{}\n当前阅读进度：{}%（已读至「{}」，后续 {} 个未读章节已严格截断，禁止剧透）\n\n【已读正文摘录】\n{}",
            ctx.book_title,
            ctx.book_author,
            ctx.progress_percent,
            ctx.cutoff_chapter_title,
            ctx.skipped_future_chapters,
            ctx.excerpt
        );
        let raw = self.complete_text(COMPANION_INSTRUCTIONS, &user_prompt, 2200)?;
        parse_companion_json(&raw, ctx)
    }

    fn complete_text(&self, system: &str, user: &str, max_tokens: u32) -> Result<String> {
        let ep = self.endpoints()?;
        if ep.use_responses {
            let payload = json!({
                "model": self.model,
                "store": false,
                "max_output_tokens": max_tokens,
                "instructions": system,
                "input": [{"role": "user", "content": user}]
            });
            Ok(self.post_responses_json(&payload, &ep.completion_url)?.reply)
        } else {
            self.complete_chat_at(&ep.completion_url, system, user, max_tokens)
        }
    }

    fn complete_chat_at(
        &self,
        endpoint: &str,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String> {
        let payload = json!({
            "model": self.model,
            "temperature": 0.65,
            "max_tokens": max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        });
        let is_local = AiSettings {
            provider: self.provider.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
        }
        .is_local();
        let timeout_secs = if is_local { 150 } else { 90 };
        let client = build_http_client(timeout_secs)?;
        let mut req = client.post(endpoint).json(&payload);
        if !self.key.is_empty() {
            req = req.bearer_auth(&self.key);
        }
        let mut response = req.send().map_err(|error| {
            if matches!(self.provider.as_str(), "ollama" | "lmstudio" | "llamacpp") {
                anyhow!(
                    "无法连接离线大模型服务（{}），请确认本地模型服务已启动且模型名称正确。",
                    endpoint
                )
            } else if error.is_timeout() {
                anyhow!("AI 回复等待超时，请稍后重试。")
            } else {
                anyhow!(
                    "暂时无法连接 {}（{}），请检查网络或大模型 URL 后重试。",
                    provider_display_name(&self.provider),
                    endpoint
                )
            }
        })?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow!("回复接收中断，请重试。"))?;
        ensure!(bytes.len() <= 2 * 1024 * 1024, "服务返回内容过长，请稍后重试。");
        let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if !(200..300).contains(&status) {
            return Err(anyhow!(service_error(
                status,
                value["error"]["code"].as_str()
            )));
        }
        parse_chat_completion(&value)
    }

    #[cfg(test)]
    fn generate_at(&self, text: &str, mood: &str, endpoint: &str) -> Result<EchoResult> {
        ensure!(text.chars().count() <= 1000, "心情文字请控制在 1000 字以内");
        self.post_responses_json(&request_with_model(&self.model, text, mood), endpoint)
    }

    #[cfg(test)]
    fn generate_passage_at(
        &self,
        passage: &str,
        book_title: &str,
        endpoint: &str,
    ) -> Result<EchoResult> {
        let trimmed = passage.trim();
        ensure!(!trimmed.is_empty(), "请先选择一段要共读的文字");
        ensure!(trimmed.chars().count() <= 2000, "选文请控制在 2000 字以内");
        self.post_responses_json(
            &passage_request_with_model(&self.model, trimmed, book_title),
            endpoint,
        )
    }

    fn post_responses_json(&self, payload: &Value, endpoint: &str) -> Result<EchoResult> {
        let client = build_http_client(75)?;
        let mut req = client.post(endpoint).json(payload);
        if !self.key.is_empty() {
            req = req.bearer_auth(&self.key);
        }
        let mut response = req.send().map_err(|error| {
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
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow!("回复接收中断，文字已保留，请重试。"))?;
        ensure!(bytes.len() <= 2 * 1024 * 1024, "服务返回内容过长，请稍后重试。");
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

fn request_with_model(model: &str, text: &str, mood: &str) -> Value {
    json!({"model": model, "store": false, "max_output_tokens": 900,
        "instructions": INSTRUCTIONS,
        "input": [{"role":"user", "content": json!({"selected_mood": mood, "written_text": text.trim()}).to_string()}]})
}

fn passage_request_with_model(model: &str, passage: &str, book_title: &str) -> Value {
    json!({"model": model, "store": false, "max_output_tokens": 900,
        "instructions": PASSAGE_INSTRUCTIONS,
        "input": [{"role":"user", "content": json!({"book_title": book_title.trim(), "selected_passage": passage.trim()}).to_string()}]})
}

#[cfg(test)]
fn request(text: &str, mood: &str) -> Value {
    request_with_model(MODEL, text, mood)
}

fn parse_chat_completion(value: &Value) -> Result<String> {
    let content = value["choices"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|choice| choice["message"]["content"].as_str())
        .map(str::trim)
        .unwrap_or("");
    // Strip optional <think>...</think> reasoning blocks emitted by local distilled models
    let cleaned = strip_think_tags(content);
    let cleaned = cleaned.trim();
    ensure!(
        !cleaned.is_empty() && cleaned.chars().count() <= 12_000,
        "本次没有收到可显示的回复，请重试。"
    );
    Ok(cleaned.to_owned())
}

fn strip_think_tags(input: &str) -> String {
    let mut text = input.to_owned();
    while let Some(start) = text.find("<think>") {
        if let Some(end) = text[start..].find("</think>") {
            text.replace_range(start..start + end + "</think>".len(), "");
        } else {
            break;
        }
    }
    text
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
        !reply.is_empty() && reply.chars().count() <= 12_000,
        "本次没有收到可显示的回复，请重试。文字已保留。"
    );
    Ok(EchoResult {
        quotes: vec![],
        reply: reply.into(),
        demo: false,
    })
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (end > start).then_some(&trimmed[start..=end])
}

pub fn parse_companion_json(raw: &str, ctx: &SpoilerSafeContext) -> Result<BookCompanionCache> {
    let json_slice = extract_json_object(raw).unwrap_or(raw.trim());
    let parsed: Value = match serde_json::from_str(json_slice) {
        Ok(v) => v,
        Err(_) => {
            // Graceful fallback when a tiny local model returns prose instead of strict JSON
            let clean = raw.trim();
            ensure!(!clean.is_empty(), "AI 未返回有效内容，请重试。");
            return Ok(BookCompanionCache {
                generated_at: now(),
                progress_percent: ctx.progress_percent,
                cutoff_chapter: ctx.cutoff_chapter_title.clone(),
                skipped_future_chapters: ctx.skipped_future_chapters,
                is_ai: true,
                headline: format!("读至 {}% · 防剧透前情回顾", ctx.progress_percent),
                story_summary: clean.chars().take(600).collect(),
                open_threads: vec![],
                resume_moment: ctx.recent_tail.chars().take(90).collect(),
                characters: vec![],
                relations: vec![],
            });
        }
    };

    let headline = parsed["headline"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("读至 {}% · 故事前情回顾", ctx.progress_percent));
    let story_summary = parsed["story_summary"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| "已为你整理当前进度前的故事脉络。".into());
    let open_threads: Vec<String> = parsed["open_threads"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::trim))
        .filter(|s| !s.is_empty())
        .take(5)
        .map(str::to_owned)
        .collect();
    let resume_moment = parsed["resume_moment"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| ctx.recent_tail.chars().take(90).collect());

    let mut characters: Vec<CharacterNode> = parsed["characters"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = item["name"].as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(CharacterNode {
                name: name.to_owned(),
                aliases: item["aliases"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(str::trim))
                    .filter(|s| !s.is_empty())
                    .take(4)
                    .map(str::to_owned)
                    .collect(),
                role: item["role"].as_str().unwrap_or("书中人物").trim().to_owned(),
                traits: item["traits"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(str::trim))
                    .filter(|s| !s.is_empty())
                    .take(4)
                    .map(str::to_owned)
                    .collect(),
                status: item["status"].as_str().unwrap_or("").trim().to_owned(),
                first_seen: item["first_seen"]
                    .as_str()
                    .unwrap_or(&ctx.cutoff_chapter_title)
                    .trim()
                    .to_owned(),
            })
        })
        .take(8)
        .collect();

    // Deduplicate characters by name
    let mut seen_names = std::collections::HashSet::new();
    characters.retain(|c| seen_names.insert(c.name.clone()));

    let relations: Vec<CharacterRelation> = parsed["relations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let from = item["from"].as_str()?.trim();
            let to = item["to"].as_str()?.trim();
            let label = item["label"].as_str()?.trim();
            if from.is_empty()
                || to.is_empty()
                || from == to
                || label.is_empty()
                || !seen_names.contains(from)
                || !seen_names.contains(to)
            {
                return None;
            }
            let tone = match item["tone"].as_str().unwrap_or("bond").trim() {
                "family" => "family",
                "conflict" => "conflict",
                "mystery" => "mystery",
                _ => "bond",
            };
            Some(CharacterRelation {
                from: from.to_owned(),
                to: to.to_owned(),
                label: label.to_owned(),
                tone: tone.to_owned(),
            })
        })
        .take(12)
        .collect();

    Ok(BookCompanionCache {
        generated_at: now(),
        progress_percent: ctx.progress_percent,
        cutoff_chapter: ctx.cutoff_chapter_title.clone(),
        skipped_future_chapters: ctx.skipped_future_chapters,
        is_ai: true,
        headline,
        story_summary,
        open_threads,
        resume_moment,
        characters,
        relations,
    })
}

fn service_error(status: u16, code: Option<&str>) -> &'static str {
    match (status, code) {
        (_, Some("insufficient_quota")) => {
            "API 余额或额度不足，请在对应模型平台补充额度后重试。文字已保留。"
        }
        (401, _) => "API 密钥无效或已过期，请在「主题与设置」重新配置。文字已保留。",
        (403, _) => "当前账号没有模型调用权限，请检查平台设置。文字已保留。",
        (429, _) => "请求过于频繁，请稍等片刻再试。文字已保留。",
        (400 | 404, _) => "当前接口或模型名称不可用，请检查模型名称与 Base URL。文字已保留。",
        _ => "AI 服务暂时未能生成回复，请稍后重试。文字已保留。",
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
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: MODEL.into(),
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

    #[test]
    fn passage_companion_sends_single_excerpt_without_storage() {
        let config = Config {
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: MODEL.into(),
            key: "sk-test-only".into(),
        };
        let (endpoint, server) = server(
            "200 OK",
            json!({"status":"completed", "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"这段风声与灯光把节奏轻轻放慢了，读到这里时你最先想起谁呢？"}]}]}),
        );
        let reply = config
            .generate_passage_at("傍晚的风从树梢滑下来。", "在林间，慢慢来", &endpoint)
            .unwrap();
        assert!(!reply.demo);
        assert!(reply.reply.contains("放慢"));
        let body = server.join().unwrap();
        assert_eq!(body["store"], false);
        assert_eq!(body["input"].as_array().unwrap().len(), 1);
        let content = body["input"][0]["content"].as_str().unwrap();
        assert!(content.contains("在林间，慢慢来"));
        assert!(content.contains("傍晚的风从树梢滑下来。"));
        assert!(config.generate_passage_at("   ", "书", &endpoint).is_err());
    }

    #[test]
    fn openai_compatible_chat_and_companion_json_parser_work() {
        let (endpoint, handle) = server(
            "200 OK",
            json!({
                "choices": [{
                    "message": {
                        "content": "<think>思考中</think>```json\n{\"headline\":\"马孔多的雨季前夜\",\"story_summary\":\"家族初建马孔多，冰块与磁铁开启了奇幻序幕。\",\"open_threads\":[\"羊皮卷的预言究竟写了什么\"],\"resume_moment\":\"上校站在行刑队前想起冰块。\",\"characters\":[{\"name\":\"布恩迪亚\",\"aliases\":[\"老布恩迪亚\"],\"role\":\"马孔多建城者\",\"traits\":[\"执着\",\"好奇\"],\"status\":\"痴迷炼金术\",\"first_seen\":\"第一章\"},{\"name\":\"乌尔苏拉\",\"aliases\":[],\"role\":\"家族支柱\",\"traits\":[\"坚韧\",\"务实\"],\"status\":\"守护家业\",\"first_seen\":\"第一章\"}],\"relations\":[{\"from\":\"布恩迪亚\",\"to\":\"乌尔苏拉\",\"label\":\"夫妻\",\"tone\":\"family\"}]}\n```"
                    }
                }]
            }),
        );
        let base_url = endpoint.trim_end_matches("/responses").to_owned();
        let settings = AiSettings {
            provider: "ollama".into(),
            base_url,
            model: "qwen2.5:7b".into(),
        };
        let ctx = SpoilerSafeContext {
            book_title: "百年孤独".into(),
            book_author: "马尔克斯".into(),
            cutoff_chapter_index: 0,
            total_chapters: 20,
            cutoff_chapter_title: "第一章".into(),
            progress_percent: 5,
            skipped_future_chapters: 19,
            excerpt: "多年以后，面对行刑队...".into(),
            recent_tail: "面对行刑队...".into(),
        };
        let cache = generate_book_companion(&settings, &LocalSecrets::default(), &ctx).unwrap();
        let req_body = handle.join().unwrap();
        assert_eq!(req_body["model"], "qwen2.5:7b");
        assert!(cache.is_ai);
        assert_eq!(cache.characters.len(), 2);
        assert_eq!(cache.relations.len(), 1);
        assert_eq!(cache.relations[0].tone, "family");
        assert_eq!(cache.skipped_future_chapters, 19);
    }

    #[test]
    fn resolves_custom_llm_urls_and_local_endpoints_accurately() {
        let full_chat =
            resolve_endpoints("custom", "https://my-gateway.example/api/v1/chat/completions")
                .unwrap();
        assert!(!full_chat.use_responses);
        assert_eq!(
            full_chat.completion_url,
            "https://my-gateway.example/api/v1/chat/completions"
        );
        assert_eq!(full_chat.models_url, "https://my-gateway.example/api/v1/models");

        let full_resp =
            resolve_endpoints("custom", "https://my-proxy.example/v1/responses").unwrap();
        assert!(full_resp.use_responses);
        assert_eq!(full_resp.completion_url, "https://my-proxy.example/v1/responses");

        let ollama_root = resolve_endpoints("ollama", "http://192.168.1.50:11434").unwrap();
        assert_eq!(
            ollama_root.completion_url,
            "http://192.168.1.50:11434/v1/chat/completions"
        );
        assert_eq!(
            ollama_root.ollama_tags_url.as_deref(),
            Some("http://192.168.1.50:11434/api/tags")
        );

        let custom_root = resolve_endpoints("custom", "https://relay.example.com").unwrap();
        assert_eq!(
            custom_root.completion_url,
            "https://relay.example.com/v1/chat/completions"
        );

        assert!(resolve_endpoints("custom", "ftp://bad.example").is_err());

        // Custom URL allows optional API key (for LAN llama.cpp / vLLM / keyless relays)
        let custom_local = AiSettings {
            provider: "custom".into(),
            base_url: "http://127.0.0.1:8080/v1".into(),
            model: "qwen2.5-7b".into(),
        };
        assert!(custom_local.is_local());
        assert!(Config::from_settings(&custom_local, &LocalSecrets::default()).is_ok());
    }
}
